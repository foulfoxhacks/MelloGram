//! Thin, misuse-resistant wrappers around audited RustCrypto / dalek
//! primitives. Nothing in here implements a primitive itself.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{Error, Result};

/// Domain-separation labels. Every KDF / signature context is versioned.
pub(crate) mod label {
    pub const BUNDLE_SIG: &[u8] = b"mellogram/e2e/v1/bundle-signature";
    pub const X3DH_SALT: &[u8] = b"mellogram/e2e/v1/x3dh/salt";
    pub const X3DH_INFO: &[u8] = b"mellogram/e2e/v1/x3dh/root-key";
    pub const RATCHET_ROOT: &[u8] = b"mellogram/e2e/v1/ratchet/root";
    pub const MESSAGE_KEY: &[u8] = b"mellogram/e2e/v1/ratchet/message-key";
    pub const AD: &[u8] = b"mellogram/e2e/v1/associated-data";
    pub const GROUP_AD: &[u8] = b"mellogram/e2e/v1/group-associated-data";
    pub const SAFETY: &[u8] = b"mellogram/e2e/v1/safety-number";
    pub const PRIVATE_CHAT: &[u8] = b"mellogram/e2e/v1/private-chat-id";
}

/// 32 bytes of secret key material. Zeroized on drop, redacted in `Debug`,
/// compared in constant time.
#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SecretBytes(pub(crate) [u8; 32]);

impl SecretBytes {
    pub(crate) fn random() -> Result<Self> {
        let mut s = SecretBytes([0u8; 32]);
        fill_random(&mut s.0)?;
        Ok(s)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn is_zero(&self) -> bool {
        self.0.iter().all(|b| *b == 0)
    }
}

impl core::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SecretBytes(<redacted>)")
    }
}

impl PartialEq for SecretBytes {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}
impl Eq for SecretBytes {}

pub(crate) fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| Error::Rng)
}

/// X25519 public key bytes from a secret.
pub(crate) fn x25519_public(secret: &SecretBytes) -> [u8; 32] {
    let s = StaticSecret::from(*secret.as_bytes());
    PublicKey::from(&s).to_bytes()
}

/// X25519 Diffie-Hellman. Rejects non-contributory (all-zero) outputs, which
/// arise when the peer supplies a low-order point.
pub(crate) fn dh(secret: &SecretBytes, their_public: &[u8; 32]) -> Result<SecretBytes> {
    let s = StaticSecret::from(*secret.as_bytes());
    let shared = s.diffie_hellman(&PublicKey::from(*their_public));
    if !shared.was_contributory() {
        return Err(Error::NonContributory);
    }
    Ok(SecretBytes(shared.to_bytes()))
}

fn hkdf_expand(salt: &[u8], ikm: &[u8], info: &[u8], okm: &mut [u8]) -> Result<()> {
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, okm)
        .map_err(|_| Error::Internal)
}

/// X3DH-style master secret -> root key.
pub(crate) fn x3dh_root_key(
    dh1: &SecretBytes,
    dh2: &SecretBytes,
    dh3: &SecretBytes,
    initiator: &[u8; 64],
    responder: &[u8; 64],
) -> Result<SecretBytes> {
    let mut ikm = Zeroizing::new([0u8; 32 * 4]);
    let (f, rest) = ikm.split_at_mut(32);
    f.fill(0xFF);
    let (a, rest) = rest.split_at_mut(32);
    a.copy_from_slice(dh1.as_bytes());
    let (b, c) = rest.split_at_mut(32);
    b.copy_from_slice(dh2.as_bytes());
    c.copy_from_slice(dh3.as_bytes());

    let mut info = Vec::with_capacity(label::X3DH_INFO.len() + 128);
    info.extend_from_slice(label::X3DH_INFO);
    info.extend_from_slice(initiator);
    info.extend_from_slice(responder);

    let mut out = SecretBytes([0u8; 32]);
    hkdf_expand(label::X3DH_SALT, ikm.as_slice(), &info, &mut out.0)?;
    Ok(out)
}

/// KDF_RK: (root key, DH output) -> (new root key, chain key).
pub(crate) fn kdf_rk(rk: &SecretBytes, dh_out: &SecretBytes) -> Result<(SecretBytes, SecretBytes)> {
    let mut okm = Zeroizing::new([0u8; 64]);
    hkdf_expand(
        rk.as_bytes(),
        dh_out.as_bytes(),
        label::RATCHET_ROOT,
        okm.as_mut_slice(),
    )?;
    let mut new_rk = SecretBytes([0u8; 32]);
    let mut ck = SecretBytes([0u8; 32]);
    let (l, r) = okm.split_at(32);
    new_rk.0.copy_from_slice(l);
    ck.0.copy_from_slice(r);
    Ok((new_rk, ck))
}

fn hmac_byte(key: &SecretBytes, byte: u8) -> Result<SecretBytes> {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key.as_bytes()).map_err(|_| Error::Internal)?;
    mac.update(&[byte]);
    let tag = mac.finalize().into_bytes();
    let mut out = SecretBytes([0u8; 32]);
    if tag.len() != 32 {
        return Err(Error::Internal);
    }
    out.0.copy_from_slice(&tag);
    Ok(out)
}

/// KDF_CK (Signal spec): message key = HMAC(ck, 0x01), next ck = HMAC(ck, 0x02).
pub(crate) fn kdf_ck(ck: &SecretBytes) -> Result<(SecretBytes, SecretBytes)> {
    let mk = hmac_byte(ck, 0x01)?;
    let next = hmac_byte(ck, 0x02)?;
    Ok((next, mk))
}

fn aead_key(mk: &SecretBytes) -> Result<Zeroizing<[u8; 32]>> {
    let mut k = Zeroizing::new([0u8; 32]);
    hkdf_expand(
        &[0u8; 32],
        mk.as_bytes(),
        label::MESSAGE_KEY,
        k.as_mut_slice(),
    )?;
    Ok(k)
}

pub(crate) const NONCE_LEN: usize = 24;
pub(crate) const TAG_LEN: usize = 16;

pub(crate) fn seal(
    mk: &SecretBytes,
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    ad: &[u8],
) -> Result<Vec<u8>> {
    let k = aead_key(mk)?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*k));
    cipher
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad: ad,
            },
        )
        .map_err(|_| Error::Internal)
}

pub(crate) fn open(
    mk: &SecretBytes,
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    ad: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let k = aead_key(mk)?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*k));
    cipher
        .decrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: ciphertext,
                aad: ad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::DecryptionFailed)
}
