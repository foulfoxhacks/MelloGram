//! Long-term identity keys and the signed public bundle.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use subtle::ConstantTimeEq;
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::crypto::{label, x25519_public, SecretBytes};
use crate::error::{Error, Result};
use crate::wire::Reader;
use crate::PROTOCOL_VERSION;

/// Armor prefix for a public bundle posted in a chat.
pub const BUNDLE_ARMOR_PREFIX: &str = "mg-e2e-key:v1:";

/// Encoded length of a [`PublicBundle`]: version + ed25519 + x25519 + SPK + sig.
pub(crate) const BUNDLE_LEN: usize = 1 + 32 + 32 + 32 + 64;

/// Max accepted armored bundle text length (generous; real ones are ~230 chars).
const MAX_ARMORED_BUNDLE_LEN: usize = 1024;

/// The public identity of a device: its Ed25519 signing key and its X25519
/// identity key. This is what safety numbers are computed over and what is
/// bound into every message's associated data.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityPublic {
    ed25519: [u8; 32],
    x25519: [u8; 32],
}

impl IdentityPublic {
    /// Ed25519 identity (signing) public key.
    pub fn ed25519_bytes(&self) -> [u8; 32] {
        self.ed25519
    }

    /// X25519 identity public key.
    pub fn x25519_bytes(&self) -> [u8; 32] {
        self.x25519
    }

    /// `ed25519 || x25519` (64 bytes).
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        let (a, b) = out.split_at_mut(32);
        a.copy_from_slice(&self.ed25519);
        b.copy_from_slice(&self.x25519);
        out
    }

    pub(crate) fn ct_eq(&self, other: &Self) -> bool {
        self.to_bytes().ct_eq(&other.to_bytes()).into()
    }
}

impl core::fmt::Debug for IdentityPublic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "IdentityPublic({})",
            URL_SAFE_NO_PAD.encode(self.to_bytes())
        )
    }
}

/// A device's signed public bundle. Can only be constructed from a local
/// [`IdentityKeyPair`] or by parsing, and parsing always verifies the
/// signature, so every `PublicBundle` value has a valid signature.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicBundle {
    identity: IdentityPublic,
    signed_prekey: [u8; 32],
    signature: [u8; 64],
}

fn signed_message(x25519_identity: &[u8; 32], signed_prekey: &[u8; 32]) -> Vec<u8> {
    let mut m = Vec::with_capacity(label::BUNDLE_SIG.len() + 1 + 64);
    m.extend_from_slice(label::BUNDLE_SIG);
    m.push(PROTOCOL_VERSION);
    m.extend_from_slice(x25519_identity);
    m.extend_from_slice(signed_prekey);
    m
}

impl PublicBundle {
    /// The identity this bundle belongs to.
    pub fn identity(&self) -> IdentityPublic {
        self.identity
    }

    /// The X25519 signed prekey.
    pub fn signed_prekey(&self) -> [u8; 32] {
        self.signed_prekey
    }

    /// Compact binary encoding (161 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BUNDLE_LEN);
        out.push(PROTOCOL_VERSION);
        out.extend_from_slice(&self.identity.ed25519);
        out.extend_from_slice(&self.identity.x25519);
        out.extend_from_slice(&self.signed_prekey);
        out.extend_from_slice(&self.signature);
        out
    }

    /// Parse and verify a binary bundle. Rejects trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let b = Self::read(&mut r)?;
        r.finish()?;
        Ok(b)
    }

    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self> {
        let version = r.u8()?;
        if version != PROTOCOL_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let ed25519 = r.array::<32>()?;
        let x25519 = r.array::<32>()?;
        let signed_prekey = r.array::<32>()?;
        let signature = r.array::<64>()?;
        let bundle = PublicBundle {
            identity: IdentityPublic { ed25519, x25519 },
            signed_prekey,
            signature,
        };
        bundle.verify()?;
        Ok(bundle)
    }

    fn verify(&self) -> Result<()> {
        let vk = VerifyingKey::from_bytes(&self.identity.ed25519).map_err(|_| Error::InvalidKey)?;
        if vk.is_weak() {
            return Err(Error::InvalidKey);
        }
        let sig = Signature::from_bytes(&self.signature);
        vk.verify_strict(
            &signed_message(&self.identity.x25519, &self.signed_prekey),
            &sig,
        )
        .map_err(|_| Error::BadSignature)
    }

    /// Text armor suitable for posting in a chat: `mg-e2e-key:v1:<base64url>`.
    pub fn to_armored(&self) -> String {
        let mut s = String::from(BUNDLE_ARMOR_PREFIX);
        s.push_str(&URL_SAFE_NO_PAD.encode(self.to_bytes()));
        s
    }

    /// Parse and verify an armored bundle. Surrounding whitespace is ignored.
    pub fn from_armored(text: &str) -> Result<Self> {
        if text.len() > MAX_ARMORED_BUNDLE_LEN {
            return Err(Error::TooLarge);
        }
        let body = text
            .trim()
            .strip_prefix(BUNDLE_ARMOR_PREFIX)
            .ok_or(Error::NotArmored)?;
        let bytes = URL_SAFE_NO_PAD.decode(body).map_err(|_| Error::Encoding)?;
        Self::from_bytes(&bytes)
    }

    /// Returns true if `text` looks like an armored public bundle.
    pub fn is_armored(text: &str) -> bool {
        text.trim_start().starts_with(BUNDLE_ARMOR_PREFIX)
    }
}

impl core::fmt::Debug for PublicBundle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicBundle")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl Serialize for PublicBundle {
    fn serialize<S: Serializer>(&self, s: S) -> core::result::Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicBundle {
    fn deserialize<D: Deserializer<'de>>(d: D) -> core::result::Result<Self, D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        PublicBundle::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

/// A device's long-term secret keys: Ed25519 identity signing key, X25519
/// identity key and X25519 signed prekey. All secrets are zeroized on drop
/// and never printed by `Debug`.
pub struct IdentityKeyPair {
    signing: SigningKey,
    x25519: SecretBytes,
    prekey: SecretBytes,
}

/// Length of [`IdentityKeyPair::to_bytes`] output.
const IDENTITY_SECRET_LEN: usize = 1 + 32 * 3;

impl IdentityKeyPair {
    /// Generate fresh keys from the OS CSPRNG.
    pub fn generate() -> Result<Self> {
        let ed = SecretBytes::random()?;
        Ok(IdentityKeyPair {
            signing: SigningKey::from_bytes(ed.as_bytes()),
            x25519: SecretBytes::random()?,
            prekey: SecretBytes::random()?,
        })
    }

    /// Public identity of this device.
    pub fn identity(&self) -> IdentityPublic {
        IdentityPublic {
            ed25519: self.signing.verifying_key().to_bytes(),
            x25519: x25519_public(&self.x25519),
        }
    }

    /// Build (and sign) the public bundle to share with peers.
    pub fn public_bundle(&self) -> PublicBundle {
        let identity = self.identity();
        let signed_prekey = x25519_public(&self.prekey);
        let signature = self
            .signing
            .sign(&signed_message(&identity.x25519, &signed_prekey))
            .to_bytes();
        PublicBundle {
            identity,
            signed_prekey,
            signature,
        }
    }

    /// Replace the signed prekey with a fresh one. Existing sessions are not
    /// affected; initial messages addressed to the old prekey that are still
    /// in flight will fail with [`Error::UnknownPrekey`]. Re-publish the
    /// bundle afterwards.
    pub fn rotate_signed_prekey(&mut self) -> Result<()> {
        self.prekey = SecretBytes::random()?;
        Ok(())
    }

    pub(crate) fn x25519_secret(&self) -> &SecretBytes {
        &self.x25519
    }

    pub(crate) fn prekey_secret(&self) -> &SecretBytes {
        &self.prekey
    }

    /// Serialize the secret keys for persistence. **The caller must encrypt
    /// this at rest** (e.g. with the platform keystore). The returned buffer
    /// zeroizes itself on drop.
    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(IDENTITY_SECRET_LEN));
        out.push(PROTOCOL_VERSION);
        let ed = Zeroizing::new(self.signing.to_bytes());
        out.extend_from_slice(ed.as_slice());
        out.extend_from_slice(self.x25519.as_bytes());
        out.extend_from_slice(self.prekey.as_bytes());
        out
    }

    /// Restore from [`IdentityKeyPair::to_bytes`] output.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let version = r.u8()?;
        if version != PROTOCOL_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let ed = SecretBytes(r.array::<32>()?);
        let x25519 = SecretBytes(r.array::<32>()?);
        let prekey = SecretBytes(r.array::<32>()?);
        r.finish()?;
        Ok(IdentityKeyPair {
            signing: SigningKey::from_bytes(ed.as_bytes()),
            x25519,
            prekey,
        })
    }
}

// Every field zeroizes itself on drop (SigningKey via ed25519-dalek's
// `zeroize` feature, SecretBytes via derive).
impl ZeroizeOnDrop for IdentityKeyPair {}

impl core::fmt::Debug for IdentityKeyPair {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdentityKeyPair")
            .field("identity", &self.identity())
            .field("secrets", &"<redacted>")
            .finish()
    }
}
