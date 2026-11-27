//! Session setup (X3DH-style) and the public encrypt / decrypt API.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::crypto::{self, dh, fill_random, label, x3dh_root_key, SecretBytes, NONCE_LEN};
use crate::error::{Error, Result};
use crate::keys::{IdentityKeyPair, IdentityPublic, PublicBundle};
use crate::padding;
use crate::ratchet::Ratchet;
use crate::safety::SafetyNumber;
use crate::wire::{self, InitialSection, WireMessage, MAX_PLAINTEXT_LEN, MESSAGE_ARMOR_PREFIX};
use crate::PROTOCOL_VERSION;

/// Version of the persisted [`Session`] format.
const SESSION_FORMAT_VERSION: u16 = 1;

/// Cap on [`Session::from_bytes`] input.
const MAX_SESSION_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
struct PendingInitial {
    ephemeral: [u8; 32],
    recipient_prekey: [u8; 32],
}

/// A pairwise end-to-end session between the local device and one peer.
///
/// Contains Double Ratchet secrets: they are zeroized on drop and never
/// printed by `Debug`. Persist with [`Session::to_bytes`] and **encrypt the
/// result at rest**.
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    format_version: u16,
    local: PublicBundle,
    peer: PublicBundle,
    initiator: bool,
    /// Initiator only: set until the first authenticated reply arrives;
    /// while set, every outgoing message carries the session-setup section.
    pending_initial: Option<PendingInitial>,
    /// Responder only: the initiator's ephemeral key, used to recognise
    /// re-sent setup messages of *this* session.
    initiator_ephemeral: Option<[u8; 32]>,
    ratchet: Ratchet,
}

/// Alias matching the persistence terminology used in the docs.
pub type SessionState = Session;

// The only secret-bearing field (`ratchet`) zeroizes itself on drop.
impl ZeroizeOnDrop for Session {}

impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Session")
            .field("local", &self.local.identity())
            .field("peer", &self.peer.identity())
            .field("initiator", &self.initiator)
            .field("awaiting_reply", &self.pending_initial.is_some())
            .field("ratchet", &self.ratchet)
            .finish()
    }
}

/// Options for [`encrypt_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncryptOptions {
    /// Pad the plaintext to a size bucket to hide its length. Default `true`.
    pub pad: bool,
}

impl Default for EncryptOptions {
    fn default() -> Self {
        EncryptOptions { pad: true }
    }
}

fn associated_data(
    sender: &IdentityPublic,
    recipient: &IdentityPublic,
    chat_id: i64,
    prefix: &[u8],
) -> Vec<u8> {
    let mut ad = Vec::with_capacity(label::AD.len() + 1 + 128 + 8 + prefix.len());
    ad.extend_from_slice(label::AD);
    ad.push(PROTOCOL_VERSION);
    ad.extend_from_slice(&sender.to_bytes());
    ad.extend_from_slice(&recipient.to_bytes());
    ad.extend_from_slice(&chat_id.to_be_bytes());
    ad.extend_from_slice(prefix);
    ad
}

impl Session {
    /// Start a session with a peer whose (already verified) bundle we hold.
    /// The first messages sent on the returned session carry the setup data
    /// the peer needs to call [`respond`].
    pub fn initiate(identity: &IdentityKeyPair, their: &PublicBundle) -> Result<Session> {
        let local = identity.public_bundle();
        if local.identity().ct_eq(&their.identity()) {
            return Err(Error::ReflectedIdentity);
        }
        let ek = SecretBytes::random()?;
        let their_spk = their.signed_prekey();
        let dh1 = dh(identity.x25519_secret(), &their_spk)?;
        let dh2 = dh(&ek, &their.identity().x25519_bytes())?;
        let dh3 = dh(&ek, &their_spk)?;
        let sk = x3dh_root_key(
            &dh1,
            &dh2,
            &dh3,
            &local.identity().to_bytes(),
            &their.identity().to_bytes(),
        )?;
        let ratchet = Ratchet::init_initiator(sk, their_spk)?;
        Ok(Session {
            format_version: SESSION_FORMAT_VERSION,
            local,
            peer: their.clone(),
            initiator: true,
            pending_initial: Some(PendingInitial {
                ephemeral: crypto::x25519_public(&ek),
                recipient_prekey: their_spk,
            }),
            initiator_ephemeral: None,
            ratchet,
        })
    }

    /// The peer's identity. Integrators should pin it (trust on first use)
    /// and warn the user if it ever changes.
    pub fn peer_identity(&self) -> IdentityPublic {
        self.peer.identity()
    }

    /// The peer's full public bundle as learned during setup.
    pub fn peer_bundle(&self) -> &PublicBundle {
        &self.peer
    }

    /// The local identity this session belongs to.
    pub fn local_identity(&self) -> IdentityPublic {
        self.local.identity()
    }

    /// Whether this side started the session.
    pub fn is_initiator(&self) -> bool {
        self.initiator
    }

    /// True while the initiator has not yet received any reply (outgoing
    /// messages still carry the setup section).
    pub fn awaiting_reply(&self) -> bool {
        self.pending_initial.is_some()
    }

    /// Safety number for out-of-band verification of both identities.
    pub fn safety_number(&self) -> SafetyNumber {
        SafetyNumber::compute(&self.local.identity(), &self.peer.identity())
    }

    /// Serialize for persistence (versioned serde/JSON encoding). The output
    /// contains ratchet secrets: **encrypt it at rest**. The buffer zeroizes
    /// itself on drop.
    pub fn to_bytes(&self) -> Result<Zeroizing<Vec<u8>>> {
        let mut buf = Zeroizing::new(Vec::with_capacity(256 * 1024));
        serde_json::to_writer(&mut *buf, self).map_err(|_| Error::InvalidState)?;
        Ok(buf)
    }

    /// Restore from [`Session::to_bytes`] output; validates version and
    /// invariants.
    pub fn from_bytes(bytes: &[u8]) -> Result<Session> {
        if bytes.len() > MAX_SESSION_BYTES {
            return Err(Error::TooLarge);
        }
        let s: Session = serde_json::from_slice(bytes).map_err(|_| Error::InvalidState)?;
        if s.format_version != SESSION_FORMAT_VERSION {
            return Err(Error::InvalidState);
        }
        if s.local.identity().ct_eq(&s.peer.identity())
            || (s.initiator && s.initiator_ephemeral.is_some())
            || (!s.initiator && s.pending_initial.is_some())
        {
            return Err(Error::InvalidState);
        }
        s.ratchet.validate()?;
        Ok(s)
    }

    fn decrypt_parsed(&mut self, msg: &WireMessage, chat_id: i64) -> Result<Vec<u8>> {
        if let Some(init) = &msg.initial {
            let ours = self
                .initiator_ephemeral
                .is_some_and(|e| bool::from(e.ct_eq(&init.ephemeral)))
                && init.sender.identity().ct_eq(&self.peer.identity());
            if !ours {
                return Err(Error::NewSessionOffered);
            }
        }
        let ad = associated_data(
            &self.peer.identity(),
            &self.local.identity(),
            chat_id,
            &msg.prefix,
        );
        let mut next = self.ratchet.clone();
        let padded = next.decrypt_step(&msg.header, |mk| {
            crypto::open(mk, &msg.nonce, &msg.ciphertext, &ad)
        })?;
        let plaintext = padding::unpad(&padded)?;
        // Commit only after full success.
        self.ratchet = next;
        if self.ratchet.has_received() {
            self.pending_initial = None;
        }
        Ok(plaintext)
    }

    fn encrypt_inner(
        &mut self,
        plaintext: &[u8],
        chat_id: i64,
        opts: EncryptOptions,
    ) -> Result<String> {
        if plaintext.len() > MAX_PLAINTEXT_LEN {
            return Err(Error::PlaintextTooLarge);
        }
        let padded = Zeroizing::new(padding::encode(plaintext, opts.pad)?);
        let mut nonce = [0u8; NONCE_LEN];
        fill_random(&mut nonce)?;
        let mut next = self.ratchet.clone();
        let (header, mk) = next.encrypt_step()?;
        let initial = self.pending_initial.as_ref().map(|p| InitialSection {
            sender: self.local.clone(),
            ephemeral: p.ephemeral,
            recipient_prekey: p.recipient_prekey,
        });
        let prefix = wire::encode_prefix(initial.as_ref(), &header);
        let ad = associated_data(
            &self.local.identity(),
            &self.peer.identity(),
            chat_id,
            &prefix,
        );
        let ct = crypto::seal(&mk, &nonce, &padded, &ad)?;
        self.ratchet = next;
        Ok(wire::armor(&prefix, &nonce, &ct))
    }
}

/// Encrypt `plaintext` for the session peer, bound to conversation `chat_id`.
/// Pads to a size bucket (see [`crate::PAD_BUCKETS`]). Returns the armored
/// text (`mg-e2e:v1:...`) to send as an ordinary Telegram message.
///
/// Fails only on oversize input, RNG failure or counter exhaustion; the
/// session is unchanged on failure.
pub fn encrypt(session: &mut Session, plaintext: &[u8], chat_id: i64) -> Result<String> {
    session.encrypt_inner(plaintext, chat_id, EncryptOptions::default())
}

/// [`encrypt`] with explicit options (e.g. to disable padding).
pub fn encrypt_with(
    session: &mut Session,
    plaintext: &[u8],
    chat_id: i64,
    opts: EncryptOptions,
) -> Result<String> {
    session.encrypt_inner(plaintext, chat_id, opts)
}

/// Decrypt an armored message from the session peer received in `chat_id`.
///
/// The session is only modified if the message authenticates and decodes.
/// Duplicates / replays return [`Error::Replay`]; an initial message for a
/// *different* session returns [`Error::NewSessionOffered`].
pub fn decrypt(session: &mut Session, armored: &str, chat_id: i64) -> Result<Vec<u8>> {
    let msg = wire::parse_armored(armored)?;
    session.decrypt_parsed(&msg, chat_id)
}

/// Accept a session-setup message (the first message(s) from an initiator)
/// and decrypt it. Returns the new session and the plaintext.
///
/// The peer identity is taken from the message (its bundle signature is
/// verified); integrators must compare [`Session::peer_identity`] with any
/// pinned key for that contact before trusting it.
pub fn respond(
    identity: &IdentityKeyPair,
    armored: &str,
    chat_id: i64,
) -> Result<(Session, Vec<u8>)> {
    let msg = wire::parse_armored(armored)?;
    let init = msg.initial.as_ref().ok_or(Error::NotInitialMessage)?;
    let local = identity.public_bundle();
    if !bool::from(init.recipient_prekey.ct_eq(&local.signed_prekey())) {
        return Err(Error::UnknownPrekey);
    }
    let sender = init.sender.identity();
    if sender.ct_eq(&local.identity()) {
        return Err(Error::ReflectedIdentity);
    }
    let dh1 = dh(identity.prekey_secret(), &sender.x25519_bytes())?;
    let dh2 = dh(identity.x25519_secret(), &init.ephemeral)?;
    let dh3 = dh(identity.prekey_secret(), &init.ephemeral)?;
    let sk = x3dh_root_key(
        &dh1,
        &dh2,
        &dh3,
        &sender.to_bytes(),
        &local.identity().to_bytes(),
    )?;
    let mut session = Session {
        format_version: SESSION_FORMAT_VERSION,
        local,
        peer: init.sender.clone(),
        initiator: false,
        pending_initial: None,
        initiator_ephemeral: Some(init.ephemeral),
        ratchet: Ratchet::init_responder(sk, identity.prekey_secret()),
    };
    let plaintext = session.decrypt_parsed(&msg, chat_id)?;
    Ok((session, plaintext))
}

/// Returns true if `text` looks like an armored encrypted message
/// (`mg-e2e:v1:` prefix). Does not validate the payload.
pub fn is_armored(text: &str) -> bool {
    text.trim_start().starts_with(MESSAGE_ARMOR_PREFIX)
}

/// Returns true if `text` decodes to a session-setup message (route it to
/// [`respond`] when no session with the sender exists). Does not verify it.
pub fn is_initial_message(text: &str) -> bool {
    wire::peek_is_initial(text)
}

/// Canonical, order-independent conversation id for a Telegram *private*
/// chat. In private chats each side sees a different chat id (the other
/// user's id), so both must instead use this value derived from the two user
/// ids. For groups / channels use the shared chat id directly.
pub fn private_chat_conversation_id(user_a: i64, user_b: i64) -> i64 {
    let (lo, hi) = if user_a <= user_b {
        (user_a, user_b)
    } else {
        (user_b, user_a)
    };
    let mut h = Sha256::new();
    h.update(label::PRIVATE_CHAT);
    h.update(lo.to_be_bytes());
    h.update(hi.to_be_bytes());
    let out = h.finalize();
    let mut b = [0u8; 8];
    for (dst, src) in b.iter_mut().zip(out.as_slice()) {
        *dst = *src;
    }
    i64::from_be_bytes(b)
}
