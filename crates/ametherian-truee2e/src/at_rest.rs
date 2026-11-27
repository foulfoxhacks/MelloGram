//! Sealing of state persisted on disk (identity keys, ratchet sessions, the
//! decrypted-message vault).
//!
//! A [`DeviceKey`] is 32 random bytes kept next to the sealed blob, so this is
//! defence in depth rather than a vault: it keeps overlay state out of plain
//! backups, sync folders and "attach the log file" bug reports. The real
//! boundary is the operating-system account and the app-private config
//! directory, exactly like the Telegram session it sits next to.
//!
//! Format: `version || nonce(24) || XChaCha20-Poly1305(key, nonce, context)`.
//! The key is HKDF-SHA256(device key, label), and the context (which file this
//! is) is authenticated as associated data, so a blob cannot be swapped for
//! another one or replayed into a different slot.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::crypto::{fill_random, NONCE_LEN};
use crate::error::{Error, Result};

/// Version byte of the sealed envelope.
pub const SEALED_FORMAT_VERSION: u8 = 1;

const LABEL: &[u8] = b"mellogram/e2e/v1/at-rest";

/// A 32-byte key that protects state stored on disk. Zeroized on drop and
/// never printed by `Debug`.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct DeviceKey([u8; 32]);

impl DeviceKey {
    /// Fresh key from the operating system CSPRNG.
    pub fn generate() -> Result<Self> {
        let mut key = DeviceKey([0u8; 32]);
        fill_random(&mut key.0)?;
        Ok(key)
    }

    /// Load a key from its 32-byte representation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let array: [u8; 32] = bytes.try_into().map_err(|_| Error::InvalidKey)?;
        Ok(DeviceKey(array))
    }

    /// 32 bytes for persistence. **Store these with owner-only permissions.**
    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0)
    }

    /// HKDF subkey for one storage slot.
    fn derive(&self, context: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        let mut okm = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(Some(LABEL), &self.0)
            .expand(context, okm.as_mut_slice())
            .map_err(|_| Error::Internal)?;
        Ok(okm)
    }
}

impl core::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DeviceKey(<redacted>)")
    }
}

fn associated_data(context: &[u8]) -> Vec<u8> {
    let mut ad = Vec::with_capacity(LABEL.len() + context.len() + 1);
    ad.extend_from_slice(LABEL);
    ad.push(SEALED_FORMAT_VERSION);
    ad.extend_from_slice(context);
    ad
}

/// Encrypt `plaintext` for storage in `context` (a stable, per-file string).
pub fn seal(key: &DeviceKey, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>> {
    let subkey = key.derive(context)?;
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*subkey));
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: &associated_data(context),
            },
        )
        .map_err(|_| Error::Internal)?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    out.push(SEALED_FORMAT_VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Reverse [`seal`]. Fails on a wrong key, a different `context`, or any
/// modification of the blob.
pub fn open(key: &DeviceKey, sealed: &[u8], context: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let (&version, rest) = sealed.split_first().ok_or(Error::Truncated)?;
    if version != SEALED_FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN).ok_or(Error::Truncated)?;
    if ciphertext.is_empty() {
        return Err(Error::Truncated);
    }
    let mut nonce_array = [0u8; NONCE_LEN];
    nonce_array.copy_from_slice(nonce);
    let subkey = key.derive(context)?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*subkey));
    cipher
        .decrypt(
            &XNonce::from(nonce_array),
            Payload {
                msg: ciphertext,
                aad: &associated_data(context),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_domain_separation() {
        let key = DeviceKey::generate().unwrap();
        let sealed = seal(&key, b"ratchet state", b"store").unwrap();
        assert_eq!(open(&key, &sealed, b"store").unwrap().as_slice(), b"ratchet state");
        // Wrong context (a blob moved to another slot) is rejected.
        assert_eq!(open(&key, &sealed, b"other").unwrap_err(), Error::DecryptionFailed);
        // Wrong key is rejected.
        let other = DeviceKey::generate().unwrap();
        assert_eq!(open(&other, &sealed, b"store").unwrap_err(), Error::DecryptionFailed);
    }

    #[test]
    fn tampering_is_rejected() {
        let key = DeviceKey::generate().unwrap();
        let mut sealed = seal(&key, b"data", b"store").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert_eq!(open(&key, &sealed, b"store").unwrap_err(), Error::DecryptionFailed);

        let mut version = seal(&key, b"data", b"store").unwrap();
        version[0] = 9;
        assert_eq!(open(&key, &version, b"store").unwrap_err(), Error::UnsupportedVersion(9));

        assert_eq!(open(&key, &[], b"store").unwrap_err(), Error::Truncated);
        assert_eq!(open(&key, &[1, 2, 3], b"store").unwrap_err(), Error::Truncated);
    }

    #[test]
    fn keys_round_trip_and_are_redacted() {
        let key = DeviceKey::generate().unwrap();
        let bytes = key.to_bytes();
        let restored = DeviceKey::from_bytes(bytes.as_slice()).unwrap();
        assert!(format!("{key:?}").contains("redacted"));
        let sealed = seal(&key, b"x", b"ctx").unwrap();
        assert_eq!(open(&restored, &sealed, b"ctx").unwrap().as_slice(), b"x");
        assert_eq!(DeviceKey::from_bytes(&[0u8; 31]).unwrap_err(), Error::InvalidKey);
    }
}
