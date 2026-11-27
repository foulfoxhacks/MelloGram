//! Group messaging with sender keys.
//!
//! A group cannot use the pairwise Double Ratchet (one chain per pair), so each
//! member publishes a single **sender key** for the group: a symmetric chain
//! that every other member holds and advances once per message. The key is
//! delivered inside the existing pairwise encrypted session (see
//! [`crate::SenderKey::to_armored`] and the app's distribution envelope), so a
//! member can only receive it from the person it belongs to.
//!
//! Security properties:
//!
//! - Confidentiality against Telegram: messages are XChaCha20-Poly1305 sealed
//!   under a key Telegram never sees.
//! - Per-message forward secrecy inside a chain: every message uses a fresh
//!   message key from the HMAC chain, and old keys are dropped.
//! - Authenticity is *group-internal*: every member holds every sender key, so
//!   a hostile member can forge a message as another member. This is inherent
//!   to sender-key groups (Signal has the same property); group encryption
//!   trusts its members. The associated data still binds the key id, the group
//!   id, the sender identity and the iteration, so a message cannot be moved to
//!   another group, replayed, or reused after a key rotation.
//! - Membership changes require a rotation: the app drops the old chain and
//!   re-distributes, so a removed member cannot read anything sent afterwards.
//!
//! The state machine is atomic: [`GroupReceiver::decrypt`] runs on a clone and
//! commits only after the message authenticates, so forged input never burns a
//! message key.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::crypto::{fill_random, kdf_ck, label, open, seal, SecretBytes, NONCE_LEN};
use crate::error::{Error, Result};
use crate::keys::IdentityPublic;
use crate::padding::{encode as pad_encode, unpad};
use crate::wire::{Reader, MAX_ARMORED_LEN};
use crate::PROTOCOL_VERSION;

/// Armor prefix for a group message.
pub const GROUP_MESSAGE_PREFIX: &str = "mg-e2e-g:v1:";

/// Armor prefix for a sender key (only ever send this inside an encrypted
/// pairwise message, never in a group).
pub const SENDER_KEY_PREFIX: &str = "mg-e2e-sk:v1:";

/// Length of a sender-key identifier.
pub const SENDER_KEY_ID_LEN: usize = 16;

/// How many message keys we derive ahead of a gap before refusing.
pub const MAX_GROUP_SKIP: u32 = 500;

/// How many out-of-order message keys a receiver keeps.
pub const MAX_GROUP_SKIPPED_KEYS: usize = 500;

fn group_ad(
    sender: &IdentityPublic,
    group_id: i64,
    key_id: &[u8; SENDER_KEY_ID_LEN],
    iteration: u32,
) -> Vec<u8> {
    let mut ad = Vec::with_capacity(label::GROUP_AD.len() + 1 + 64 + 8 + SENDER_KEY_ID_LEN + 4);
    ad.extend_from_slice(label::GROUP_AD);
    ad.push(PROTOCOL_VERSION);
    ad.extend_from_slice(&sender.to_bytes());
    ad.extend_from_slice(&group_id.to_be_bytes());
    ad.extend_from_slice(key_id);
    ad.extend_from_slice(&iteration.to_be_bytes());
    ad
}

fn armor(prefix: &str, bin: &[u8]) -> String {
    let mut s = String::with_capacity(prefix.len() + bin.len().div_ceil(3) * 4);
    s.push_str(prefix);
    s.push_str(&URL_SAFE_NO_PAD.encode(bin));
    s
}

fn decode(prefix: &str, text: &str) -> Result<Vec<u8>> {
    if text.len() > MAX_ARMORED_LEN {
        return Err(Error::TooLarge);
    }
    let body = text.trim().strip_prefix(prefix).ok_or(Error::NotArmored)?;
    URL_SAFE_NO_PAD.decode(body).map_err(|_| Error::Encoding)
}

/// Returns true when `text` is a group message (`mg-e2e-g:v1:`).
pub fn is_group_message(text: &str) -> bool {
    text.trim_start().starts_with(GROUP_MESSAGE_PREFIX)
}

/// Returns true when `text` is a bare sender key (`mg-e2e-sk:v1:`).
pub fn is_sender_key(text: &str) -> bool {
    text.trim_start().starts_with(SENDER_KEY_PREFIX)
}

/// A sender's chain for one group. Only ever shared inside an encrypted
/// pairwise message.
#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct SenderKey {
    id: [u8; SENDER_KEY_ID_LEN],
    chain: SecretBytes,
    iteration: u32,
}

impl core::fmt::Debug for SenderKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SenderKey")
            .field("iteration", &self.iteration)
            .field("chain", &"<redacted>")
            .finish()
    }
}

impl SenderKey {
    /// Fresh random key id and chain, starting at iteration 0.
    pub fn generate() -> Result<Self> {
        let mut id = [0u8; SENDER_KEY_ID_LEN];
        fill_random(&mut id)?;
        Ok(SenderKey {
            id,
            chain: SecretBytes::random()?,
            iteration: 0,
        })
    }

    /// Identifier of this key (changes on every rotation).
    pub fn id(&self) -> [u8; SENDER_KEY_ID_LEN] {
        self.id
    }

    /// Next iteration that will be used.
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// `mg-e2e-sk:v1:` armor for distribution over a pairwise session.
    pub fn to_armored(&self) -> String {
        let mut bin = Vec::with_capacity(1 + SENDER_KEY_ID_LEN + 32 + 4);
        bin.push(PROTOCOL_VERSION);
        bin.extend_from_slice(&self.id);
        bin.extend_from_slice(self.chain.as_bytes());
        bin.extend_from_slice(&self.iteration.to_be_bytes());
        armor(SENDER_KEY_PREFIX, &bin)
    }

    /// Parse a distributed key. Bounds-checked and version-gated.
    pub fn from_armored(text: &str) -> Result<Self> {
        let bin = decode(SENDER_KEY_PREFIX, text)?;
        let mut r = Reader::new(&bin);
        let version = r.u8()?;
        if version != PROTOCOL_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let id = r.array::<SENDER_KEY_ID_LEN>()?;
        let chain = SecretBytes(r.array::<32>()?);
        let iteration = r.u32()?;
        r.finish()?;
        Ok(SenderKey {
            id,
            chain,
            iteration,
        })
    }

    /// Encrypt one group message and advance the chain.
    pub fn encrypt(
        &mut self,
        sender: &IdentityPublic,
        group_id: i64,
        plaintext: &[u8],
    ) -> Result<String> {
        let iteration = self.iteration;
        let next = iteration.checked_add(1).ok_or(Error::CounterExhausted)?;
        let (chain, mk) = kdf_ck(&self.chain)?;
        let mut nonce = [0u8; NONCE_LEN];
        fill_random(&mut nonce)?;
        let padded = Zeroizing::new(pad_encode(plaintext, true)?);
        let ad = group_ad(sender, group_id, &self.id, iteration);
        let ciphertext = seal(&mk, &nonce, &padded, &ad)?;

        let mut bin = Vec::with_capacity(1 + SENDER_KEY_ID_LEN + 4 + NONCE_LEN + ciphertext.len());
        bin.push(PROTOCOL_VERSION);
        bin.extend_from_slice(&self.id);
        bin.extend_from_slice(&iteration.to_be_bytes());
        bin.extend_from_slice(&nonce);
        bin.extend_from_slice(&ciphertext);

        self.chain = chain;
        self.iteration = next;
        Ok(armor(GROUP_MESSAGE_PREFIX, &bin))
    }

    /// The receiving half for a member who just received this key.
    pub fn receiver(&self) -> GroupReceiver {
        GroupReceiver {
            id: self.id,
            chain: self.chain.clone(),
            iteration: self.iteration,
            skipped: Vec::new(),
        }
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct SkippedGroupKey {
    iteration: u32,
    mk: SecretBytes,
}

/// One member's view of a sender's chain.
#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct GroupReceiver {
    id: [u8; SENDER_KEY_ID_LEN],
    chain: SecretBytes,
    iteration: u32,
    /// Insertion-ordered; oldest evicted first.
    skipped: Vec<SkippedGroupKey>,
}

impl core::fmt::Debug for GroupReceiver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupReceiver")
            .field("iteration", &self.iteration)
            .field("skipped", &self.skipped.len())
            .field("chain", &"<redacted>")
            .finish()
    }
}

impl GroupReceiver {
    /// Sender key id this receiver is bound to.
    pub fn id(&self) -> [u8; SENDER_KEY_ID_LEN] {
        self.id
    }

    /// How many out-of-order keys are buffered (tests / diagnostics).
    pub fn skipped_len(&self) -> usize {
        self.skipped.len()
    }

    /// Decrypt a group message from `sender` for `group_id`.
    pub fn decrypt(
        &mut self,
        sender: &IdentityPublic,
        group_id: i64,
        armored: &str,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let bin = decode(GROUP_MESSAGE_PREFIX, armored)?;
        let mut r = Reader::new(&bin);
        let version = r.u8()?;
        if version != PROTOCOL_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let key_id = r.array::<SENDER_KEY_ID_LEN>()?;
        if key_id != self.id {
            return Err(Error::UnknownSenderKey);
        }
        let iteration = r.u32()?;
        let nonce = r.array::<NONCE_LEN>()?;
        let ciphertext = r.rest();
        if ciphertext.len() < 16 {
            return Err(Error::Truncated);
        }

        let mut next = self.clone();
        let mk = next.key_for(iteration)?;
        let ad = group_ad(sender, group_id, &self.id, iteration);
        let padded = open(&mk, &nonce, ciphertext, &ad)?;
        let plaintext = unpad(&padded)?;
        *self = next;
        Ok(Zeroizing::new(plaintext))
    }

    /// Message key for `iteration`, buffering skipped keys on the way.
    fn key_for(&mut self, iteration: u32) -> Result<SecretBytes> {
        if let Some(index) = self.skipped.iter().position(|k| k.iteration == iteration) {
            // `SkippedGroupKey` zeroizes on drop, so copy the key out instead of
            // moving it.
            let key = self.skipped.remove(index);
            return Ok(key.mk.clone());
        }
        if iteration < self.iteration {
            // Already passed this point and the key is gone: replay or a gap
            // that was never delivered.
            return Err(Error::Replay);
        }
        if iteration - self.iteration > MAX_GROUP_SKIP {
            return Err(Error::TooManySkipped);
        }
        while self.iteration < iteration {
            let (chain, mk) = kdf_ck(&self.chain)?;
            self.skipped.push(SkippedGroupKey {
                iteration: self.iteration,
                mk,
            });
            if self.skipped.len() > MAX_GROUP_SKIPPED_KEYS {
                self.skipped.remove(0);
            }
            self.chain = chain;
            self.iteration += 1;
        }
        let (chain, mk) = kdf_ck(&self.chain)?;
        self.chain = chain;
        self.iteration = self.iteration.checked_add(1).ok_or(Error::CounterExhausted)?;
        Ok(mk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IdentityKeyPair;

    fn setup() -> (IdentityKeyPair, SenderKey, GroupReceiver, i64) {
        let alice = IdentityKeyPair::generate().unwrap();
        let mut key = SenderKey::generate().unwrap();
        assert_eq!(key.iteration(), 0);
        let receiver = key.receiver();
        // Advance so the receiver really starts mid-chain, like a real member
        // joining after some messages.
        key.encrypt(&alice.identity(), -42, b"before").unwrap();
        (alice, key, receiver, -42)
    }

    #[test]
    fn round_trip_and_forward_ratchet() {
        let (alice, mut key, mut bob, group) = setup();
        let one = key.encrypt(&alice.identity(), group, b"hello group").unwrap();
        assert!(is_group_message(&one));
        assert_eq!(
            bob.decrypt(&alice.identity(), group, &one).unwrap().as_slice(),
            b"hello group"
        );
        let two = key.encrypt(&alice.identity(), group, b"second").unwrap();
        assert_eq!(
            bob.decrypt(&alice.identity(), group, &two).unwrap().as_slice(),
            b"second"
        );
        assert_ne!(one, two);
    }

    #[test]
    fn out_of_order_and_replay() {
        let (alice, mut key, mut bob, group) = setup();
        let first = key.encrypt(&alice.identity(), group, b"1").unwrap();
        let second = key.encrypt(&alice.identity(), group, b"2").unwrap();
        let third = key.encrypt(&alice.identity(), group, b"3").unwrap();
        // Delivered backwards.
        assert_eq!(bob.decrypt(&alice.identity(), group, &third).unwrap().as_slice(), b"3");
        assert_eq!(bob.decrypt(&alice.identity(), group, &second).unwrap().as_slice(), b"2");
        assert_eq!(bob.decrypt(&alice.identity(), group, &first).unwrap().as_slice(), b"1");
        assert_eq!(
            bob.decrypt(&alice.identity(), group, &first).unwrap_err(),
            Error::Replay
        );
        assert!(bob.skipped_len() <= MAX_GROUP_SKIPPED_KEYS);
    }

    #[test]
    fn wrong_group_sender_or_key_is_rejected() {
        let (alice, mut key, mut bob, group) = setup();
        let armored = key.encrypt(&alice.identity(), group, b"scoped").unwrap();
        assert_eq!(
            bob.decrypt(&alice.identity(), group + 1, &armored).unwrap_err(),
            Error::DecryptionFailed
        );
        let mallory = IdentityKeyPair::generate().unwrap();
        assert_eq!(
            bob.decrypt(&mallory.identity(), group, &armored).unwrap_err(),
            Error::DecryptionFailed
        );
        // A different key id is refused before any crypto runs.
        let other = SenderKey::generate().unwrap();
        let receiver_backup = bob.clone();
        let _ = receiver_backup;
        let foreign = {
            let mut k = other.clone();
            k.encrypt(&alice.identity(), group, b"x").unwrap()
        };
        assert_eq!(
            bob.decrypt(&alice.identity(), group, &foreign).unwrap_err(),
            Error::UnknownSenderKey
        );
    }

    #[test]
    fn tampering_does_not_consume_keys() {
        let (alice, mut key, mut bob, group) = setup();
        let good = key.encrypt(&alice.identity(), group, b"payload").unwrap();
        let mut bad = good.clone().into_bytes();
        let last = bad.len() - 1;
        bad[last] = if bad[last] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bad).unwrap();
        assert!(bob.decrypt(&alice.identity(), group, &bad).is_err());
        // The real message still decrypts: the failed attempt changed nothing.
        assert_eq!(bob.decrypt(&alice.identity(), group, &good).unwrap().as_slice(), b"payload");
    }

    #[test]
    fn sender_key_armor_round_trip_and_bounds() {
        let key = SenderKey::generate().unwrap();
        let text = key.to_armored();
        assert!(is_sender_key(&text));
        let parsed = SenderKey::from_armored(&text).unwrap();
        assert_eq!(parsed.id(), key.id());
        assert_eq!(parsed.iteration(), key.iteration());
        assert_eq!(SENDER_KEY_PREFIX, "mg-e2e-sk:v1:");
        assert_eq!(
            SenderKey::from_armored("mg-e2e-sk:v1:!!!!").unwrap_err(),
            Error::Encoding
        );
        let wrong_version = format!("mg-e2e-sk:v1:{}", URL_SAFE_NO_PAD.encode([9u8; 53]));
        assert_eq!(
            SenderKey::from_armored(&wrong_version).unwrap_err(),
            Error::UnsupportedVersion(9)
        );
        assert!(!is_group_message(&text));
        assert!(!format!("{key:?}").contains(&URL_SAFE_NO_PAD.encode(key.chain.as_bytes())));
    }

    #[test]
    fn skipping_is_bounded() {
        let (alice, mut key, mut bob, group) = setup();
        let mut last = String::new();
        for _ in 0..(MAX_GROUP_SKIP + 2) {
            last = key.encrypt(&alice.identity(), group, b"x").unwrap();
        }
        assert_eq!(
            bob.decrypt(&alice.identity(), group, &last).unwrap_err(),
            Error::TooManySkipped
        );
    }
}
