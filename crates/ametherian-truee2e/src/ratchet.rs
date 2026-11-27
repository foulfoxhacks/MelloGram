//! Double Ratchet (Signal specification, revision 1), with plaintext headers.
//!
//! The state machine below operates in place; callers that need atomicity
//! (all of them) clone the state, run the step on the clone and commit only
//! on success, so a forged or corrupted message never changes the session.

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::{dh, kdf_ck, kdf_rk, x25519_public, SecretBytes};
use crate::error::{Error, Result};
use crate::wire::Header;
use crate::{MAX_SKIP, MAX_SKIPPED_KEYS};

/// How many previous receiving ratchet public keys we remember, so a replay
/// of a message from an already-retired chain is reported as [`Error::Replay`].
const MAX_RETIRED_DHR: usize = 64;

#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub(crate) struct SkippedKey {
    dh: [u8; 32],
    n: u32,
    mk: SecretBytes,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub(crate) struct Ratchet {
    dhs: SecretBytes,
    dhs_pub: [u8; 32],
    dhr: Option<[u8; 32]>,
    rk: SecretBytes,
    cks: Option<SecretBytes>,
    ckr: Option<SecretBytes>,
    ns: u32,
    nr: u32,
    pn: u32,
    /// Insertion-ordered; oldest evicted first.
    skipped: Vec<SkippedKey>,
    retired_dhr: Vec<[u8; 32]>,
}

impl core::fmt::Debug for Ratchet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ratchet")
            .field("ns", &self.ns)
            .field("nr", &self.nr)
            .field("pn", &self.pn)
            .field("skipped", &self.skipped.len())
            .field("keys", &"<redacted>")
            .finish()
    }
}

impl Ratchet {
    /// Alice: knows Bob's signed prekey, which serves as his first ratchet key.
    pub(crate) fn init_initiator(sk: SecretBytes, their_ratchet: [u8; 32]) -> Result<Self> {
        let dhs = SecretBytes::random()?;
        let (rk, cks) = kdf_rk(&sk, &dh(&dhs, &their_ratchet)?)?;
        Ok(Ratchet {
            dhs_pub: x25519_public(&dhs),
            dhs,
            dhr: Some(their_ratchet),
            rk,
            cks: Some(cks),
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: Vec::new(),
            retired_dhr: Vec::new(),
        })
    }

    /// Bob: his signed prekey pair is his first ratchet key pair.
    pub(crate) fn init_responder(sk: SecretBytes, spk: &SecretBytes) -> Self {
        Ratchet {
            dhs_pub: x25519_public(spk),
            dhs: spk.clone(),
            dhr: None,
            rk: sk,
            cks: None,
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: Vec::new(),
            retired_dhr: Vec::new(),
        }
    }

    pub(crate) fn has_received(&self) -> bool {
        self.ckr.is_some()
    }

    /// Advance the sending chain: returns the header and the message key.
    pub(crate) fn encrypt_step(&mut self) -> Result<(Header, SecretBytes)> {
        let ck = self.cks.as_ref().ok_or(Error::InvalidState)?;
        let next_ns = self.ns.checked_add(1).ok_or(Error::CounterExhausted)?;
        let (next, mk) = kdf_ck(ck)?;
        let header = Header {
            dh: self.dhs_pub,
            pn: self.pn,
            n: self.ns,
        };
        self.cks = Some(next);
        self.ns = next_ns;
        Ok((header, mk))
    }

    /// Derive the message key for `header` (performing DH ratchet steps and
    /// storing skipped keys as needed) and run `open` with it.
    pub(crate) fn decrypt_step<T>(
        &mut self,
        header: &Header,
        open: impl FnOnce(&SecretBytes) -> Result<T>,
    ) -> Result<T> {
        if let Some(idx) = self
            .skipped
            .iter()
            .position(|k| k.dh == header.dh && k.n == header.n)
        {
            let key = self.skipped.remove(idx);
            return open(&key.mk);
        }
        if self.dhr != Some(header.dh) {
            if self.retired_dhr.contains(&header.dh) {
                return Err(Error::Replay);
            }
            self.skip_until(header.pn)?;
            self.dh_ratchet(&header.dh)?;
        } else if header.n < self.nr {
            return Err(Error::Replay);
        }
        self.skip_until(header.n)?;
        let ck = self.ckr.as_ref().ok_or(Error::InvalidState)?;
        let next_nr = self.nr.checked_add(1).ok_or(Error::CounterExhausted)?;
        let (next, mk) = kdf_ck(ck)?;
        self.ckr = Some(next);
        self.nr = next_nr;
        open(&mk)
    }

    fn skip_until(&mut self, until: u32) -> Result<()> {
        let Some(ck) = self.ckr.as_ref() else {
            return Ok(());
        };
        if until <= self.nr {
            return Ok(());
        }
        if until - self.nr > MAX_SKIP {
            return Err(Error::TooManySkipped);
        }
        let dhr = self.dhr.ok_or(Error::InvalidState)?;
        let mut ck = ck.clone();
        while self.nr < until {
            let (next, mk) = kdf_ck(&ck)?;
            self.skipped.push(SkippedKey {
                dh: dhr,
                n: self.nr,
                mk,
            });
            if self.skipped.len() > MAX_SKIPPED_KEYS {
                self.skipped.remove(0);
            }
            ck = next;
            self.nr += 1;
        }
        self.ckr = Some(ck);
        Ok(())
    }

    fn dh_ratchet(&mut self, their: &[u8; 32]) -> Result<()> {
        if let Some(old) = self.dhr {
            self.retired_dhr.push(old);
            if self.retired_dhr.len() > MAX_RETIRED_DHR {
                self.retired_dhr.remove(0);
            }
        }
        self.pn = self.ns;
        self.ns = 0;
        self.nr = 0;
        self.dhr = Some(*their);
        let (rk, ckr) = kdf_rk(&self.rk, &dh(&self.dhs, their)?)?;
        self.ckr = Some(ckr);
        self.dhs = SecretBytes::random()?;
        self.dhs_pub = x25519_public(&self.dhs);
        let (rk, cks) = kdf_rk(&rk, &dh(&self.dhs, their)?)?;
        self.rk = rk;
        self.cks = Some(cks);
        Ok(())
    }

    /// Invariant check after deserialization.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.skipped.len() > MAX_SKIPPED_KEYS
            || self.retired_dhr.len() > MAX_RETIRED_DHR
            || x25519_public(&self.dhs) != self.dhs_pub
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn skipped_len(&self) -> usize {
        self.skipped.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeroize_clears_ratchet_secrets() {
        let sk = SecretBytes::random().unwrap();
        let spk = SecretBytes::random().unwrap();
        let mut r = Ratchet::init_initiator(sk, x25519_public(&spk)).unwrap();
        assert!(!r.rk.is_zero());
        r.zeroize();
        assert!(r.rk.is_zero());
        assert!(r.dhs.is_zero());
        assert!(r.cks.is_none());
        assert!(r.skipped.is_empty());
    }

    #[test]
    fn skipped_key_storage_is_bounded() {
        let sk = SecretBytes::random().unwrap();
        let spk = SecretBytes::random().unwrap();
        let mut a = Ratchet::init_initiator(sk.clone(), x25519_public(&spk)).unwrap();
        let mut b = Ratchet::init_responder(sk, &spk);
        let ok = |_: &SecretBytes| Ok(());
        for _round in 0..3 {
            let mut last = None;
            for _ in 0..600 {
                last = Some(a.encrypt_step().unwrap().0);
            }
            b.decrypt_step(&last.unwrap(), ok).unwrap();
            assert!(b.skipped_len() <= MAX_SKIPPED_KEYS);
            let (h, _) = b.encrypt_step().unwrap();
            a.decrypt_step(&h, ok).unwrap();
        }
        assert_eq!(b.skipped_len(), MAX_SKIPPED_KEYS);

        // A gap larger than MAX_SKIP is refused.
        let mut last = None;
        for _ in 0..(MAX_SKIP + 2) {
            last = Some(a.encrypt_step().unwrap().0);
        }
        assert!(matches!(
            b.decrypt_step(&last.unwrap(), ok),
            Err(Error::TooManySkipped)
        ));
    }

    #[test]
    fn secret_bytes_zeroize_and_redacted_debug() {
        let mut s = SecretBytes::random().unwrap();
        assert!(!format!("{s:?}").contains(&format!("{:?}", s.0)));
        assert!(format!("{s:?}").contains("redacted"));
        s.zeroize();
        assert!(s.is_zero());
    }
}
