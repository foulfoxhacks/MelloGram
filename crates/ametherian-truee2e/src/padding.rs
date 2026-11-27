//! Length-hiding padding applied to plaintext before encryption.
//!
//! Format: `u32 big-endian real length || data || zero bytes`. The total is
//! rounded up to the next bucket in [`PAD_BUCKETS`]; beyond the largest
//! bucket it is rounded up to a multiple of the largest bucket.

use crate::error::{Error, Result};

/// Padding buckets (total padded size, including the 4-byte length prefix).
pub const PAD_BUCKETS: [usize; 4] = [64, 256, 1024, 4096];

const LEN_PREFIX: usize = 4;

/// Size of the padded buffer for a plaintext of `len` bytes.
pub fn padded_len(len: usize) -> usize {
    let total = len.saturating_add(LEN_PREFIX);
    for b in PAD_BUCKETS {
        if total <= b {
            return b;
        }
    }
    let step = 4096;
    total.div_ceil(step).saturating_mul(step)
}

/// Encode `data` with the length prefix. With `bucket == true` it is padded
/// to [`padded_len`]; otherwise only the prefix is added.
pub(crate) fn encode(data: &[u8], bucket: bool) -> Result<Vec<u8>> {
    let len = u32::try_from(data.len()).map_err(|_| Error::PlaintextTooLarge)?;
    let total = if bucket {
        padded_len(data.len())
    } else {
        data.len() + LEN_PREFIX
    };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
    out.resize(total, 0);
    Ok(out)
}

/// Pad `data` to its bucket (see module docs).
pub fn pad(data: &[u8]) -> Result<Vec<u8>> {
    encode(data, true)
}

/// Reverse [`pad`]. Rejects inconsistent lengths and non-zero padding.
pub fn unpad(padded: &[u8]) -> Result<Vec<u8>> {
    let (prefix, rest) = padded
        .split_at_checked(LEN_PREFIX)
        .ok_or(Error::BadPadding)?;
    let mut len_bytes = [0u8; LEN_PREFIX];
    len_bytes.copy_from_slice(prefix);
    let len = usize::try_from(u32::from_be_bytes(len_bytes)).map_err(|_| Error::BadPadding)?;
    let (data, pad) = rest.split_at_checked(len).ok_or(Error::BadPadding)?;
    if pad.iter().any(|b| *b != 0) {
        return Err(Error::BadPadding);
    }
    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets() {
        assert_eq!(padded_len(0), 64);
        assert_eq!(padded_len(60), 64);
        assert_eq!(padded_len(61), 256);
        assert_eq!(padded_len(1020), 1024);
        assert_eq!(padded_len(4092), 4096);
        assert_eq!(padded_len(4093), 8192);
        for n in [0usize, 1, 59, 60, 61, 300, 5000] {
            let data = vec![7u8; n];
            let p = pad(&data).unwrap();
            assert_eq!(p.len(), padded_len(n));
            assert_eq!(unpad(&p).unwrap(), data);
        }
    }

    #[test]
    fn bad_padding() {
        assert_eq!(unpad(&[]), Err(Error::BadPadding));
        assert_eq!(unpad(&[0, 0, 0, 5, 1]), Err(Error::BadPadding));
        assert_eq!(unpad(&[0, 0, 0, 1, 1, 9]), Err(Error::BadPadding));
        assert_eq!(unpad(&[0xFF, 0xFF, 0xFF, 0xFF]), Err(Error::BadPadding));
    }
}
