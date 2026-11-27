//! Safety numbers for out-of-band identity verification.
//!
//! Each party's half is 30 digits derived from 5200 iterations of SHA-512
//! over a versioned label and its identity public key (ed25519 || x25519),
//! following the construction of Signal's numeric fingerprints. The two
//! halves are sorted, so both users see the same 60 digits regardless of
//! who computes them.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;

use crate::crypto::label;
use crate::keys::IdentityPublic;

/// Prefix of the QR / copy-paste verification string.
pub const SAFETY_QR_PREFIX: &str = "mg-e2e-verify:v1:";

const ITERATIONS: usize = 5200;
const FINGERPRINT_VERSION: u16 = 1;

/// A 60-digit, order-independent safety number for a pair of identities.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SafetyNumber {
    digits: String,
    qr: String,
}

fn half(id: &IdentityPublic) -> String {
    let key = id.to_bytes();
    let mut h = Sha512::new();
    h.update(label::SAFETY);
    h.update(FINGERPRINT_VERSION.to_be_bytes());
    h.update(key);
    let mut out = h.finalize();
    for _ in 1..ITERATIONS {
        let mut h = Sha512::new();
        h.update(out.as_slice());
        h.update(key);
        out = h.finalize();
    }
    let mut s = String::with_capacity(30);
    for chunk in out.as_slice().chunks_exact(5).take(6) {
        let v = chunk.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
        s.push_str(&format!("{:05}", v % 100_000));
    }
    s
}

impl SafetyNumber {
    /// Compute the safety number for two identities (argument order does not
    /// matter).
    pub fn compute(a: &IdentityPublic, b: &IdentityPublic) -> Self {
        let (ha, hb) = (half(a), half(b));
        let digits = if ha <= hb { ha + &hb } else { hb + &ha };
        let (lo, hi) = if a.to_bytes() <= b.to_bytes() {
            (a, b)
        } else {
            (b, a)
        };
        let mut raw = Vec::with_capacity(128);
        raw.extend_from_slice(&lo.to_bytes());
        raw.extend_from_slice(&hi.to_bytes());
        let qr = format!("{SAFETY_QR_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw));
        SafetyNumber { digits, qr }
    }

    /// The 60 digits without separators.
    pub fn digits(&self) -> &str {
        &self.digits
    }

    /// 12 groups of 5 digits.
    pub fn groups(&self) -> Vec<String> {
        self.digits
            .as_bytes()
            .chunks(5)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect()
    }

    /// Human-readable form: 12 groups of 5 digits separated by spaces.
    pub fn display(&self) -> String {
        self.groups().join(" ")
    }

    /// QR-friendly string encoding both identity keys (sorted). Scanning the
    /// peer's QR and calling [`SafetyNumber::matches_qr`] verifies the keys.
    pub fn qr_payload(&self) -> &str {
        &self.qr
    }

    /// Constant-time comparison against a scanned QR payload.
    pub fn matches_qr(&self, scanned: &str) -> bool {
        let s = scanned.trim();
        s.len() == self.qr.len() && bool::from(s.as_bytes().ct_eq(self.qr.as_bytes()))
    }

    /// Compare against digits typed / read aloud by the user; whitespace and
    /// dashes are ignored.
    pub fn matches_digits(&self, input: &str) -> bool {
        let cleaned: String = input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        cleaned.len() == self.digits.len()
            && bool::from(cleaned.as_bytes().ct_eq(self.digits.as_bytes()))
    }
}

impl core::fmt::Display for SafetyNumber {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.display())
    }
}
