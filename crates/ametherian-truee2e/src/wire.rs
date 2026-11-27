//! Binary wire format and text armor. All parsing is bounds-checked and
//! returns typed errors; nothing here can panic on hostile input.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use crate::crypto::{NONCE_LEN, TAG_LEN};
use crate::error::{Error, Result};
use crate::keys::PublicBundle;
use crate::PROTOCOL_VERSION;

/// Armor prefix for an encrypted message.
pub const MESSAGE_ARMOR_PREFIX: &str = "mg-e2e:v1:";

/// Armored messages longer than this (in bytes of text) are rejected before
/// any decoding takes place.
pub const MAX_ARMORED_LEN: usize = 64 * 1024;

/// Largest plaintext accepted by [`crate::encrypt`]. Chosen so that a padded
/// initial message still fits under [`MAX_ARMORED_LEN`].
pub const MAX_PLAINTEXT_LEN: usize = 32 * 1024;

pub(crate) const TYPE_NORMAL: u8 = 0x01;
pub(crate) const TYPE_INITIAL: u8 = 0x02;

/// Minimal bounds-checked cursor.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Reader { buf }
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (head, tail) = self.buf.split_at_checked(n).ok_or(Error::Truncated)?;
        self.buf = tail;
        Ok(head)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(u8::from_be_bytes(self.array::<1>()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array::<4>()?))
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len()
    }

    pub(crate) fn rest(&mut self) -> &'a [u8] {
        let r = self.buf;
        self.buf = &[];
        r
    }

    pub(crate) fn finish(&self) -> Result<()> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingData)
        }
    }
}

/// Double Ratchet message header (sent in the clear, authenticated as AD).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub dh: [u8; 32],
    pub pn: u32,
    pub n: u32,
}

/// Session-setup data carried by initial messages until the initiator has
/// received a reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InitialSection {
    pub sender: PublicBundle,
    pub ephemeral: [u8; 32],
    pub recipient_prekey: [u8; 32],
}

pub(crate) struct WireMessage {
    pub initial: Option<InitialSection>,
    pub header: Header,
    /// Everything before the nonce; authenticated as part of the AD.
    pub prefix: Vec<u8>,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

pub(crate) fn encode_prefix(initial: Option<&InitialSection>, header: &Header) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 225 + 40);
    out.push(PROTOCOL_VERSION);
    match initial {
        Some(init) => {
            out.push(TYPE_INITIAL);
            out.extend_from_slice(&init.sender.to_bytes());
            out.extend_from_slice(&init.ephemeral);
            out.extend_from_slice(&init.recipient_prekey);
        }
        None => out.push(TYPE_NORMAL),
    }
    out.extend_from_slice(&header.dh);
    out.extend_from_slice(&header.pn.to_be_bytes());
    out.extend_from_slice(&header.n.to_be_bytes());
    out
}

pub(crate) fn armor(prefix: &[u8], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> String {
    let mut bin = Vec::with_capacity(prefix.len() + NONCE_LEN + ciphertext.len());
    bin.extend_from_slice(prefix);
    bin.extend_from_slice(nonce);
    bin.extend_from_slice(ciphertext);
    let mut s = String::with_capacity(MESSAGE_ARMOR_PREFIX.len() + bin.len().div_ceil(3) * 4);
    s.push_str(MESSAGE_ARMOR_PREFIX);
    s.push_str(&URL_SAFE_NO_PAD.encode(&bin));
    s
}

fn decode_armor(text: &str) -> Result<Vec<u8>> {
    if text.len() > MAX_ARMORED_LEN {
        return Err(Error::TooLarge);
    }
    let body = text
        .trim()
        .strip_prefix(MESSAGE_ARMOR_PREFIX)
        .ok_or(Error::NotArmored)?;
    URL_SAFE_NO_PAD.decode(body).map_err(|_| Error::Encoding)
}

/// Cheap check of the message type without verifying anything.
pub(crate) fn peek_is_initial(text: &str) -> bool {
    decode_armor(text)
        .ok()
        .and_then(|b| b.get(1).copied())
        .is_some_and(|t| t == TYPE_INITIAL)
}

pub(crate) fn parse_armored(text: &str) -> Result<WireMessage> {
    let bin = decode_armor(text)?;
    parse_binary(&bin)
}

pub(crate) fn parse_binary(bin: &[u8]) -> Result<WireMessage> {
    let mut r = Reader::new(bin);
    let version = r.u8()?;
    if version != PROTOCOL_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let initial = match r.u8()? {
        TYPE_NORMAL => None,
        TYPE_INITIAL => {
            let sender = PublicBundle::read(&mut r)?;
            let ephemeral = r.array::<32>()?;
            let recipient_prekey = r.array::<32>()?;
            Some(InitialSection {
                sender,
                ephemeral,
                recipient_prekey,
            })
        }
        other => return Err(Error::UnknownMessageType(other)),
    };
    let header = Header {
        dh: r.array::<32>()?,
        pn: r.u32()?,
        n: r.u32()?,
    };
    let prefix_len = bin.len().saturating_sub(r.remaining());
    let prefix = bin.get(..prefix_len).ok_or(Error::Truncated)?.to_vec();
    let nonce = r.array::<NONCE_LEN>()?;
    if r.remaining() < TAG_LEN + 4 {
        return Err(Error::Truncated);
    }
    let ciphertext = r.rest().to_vec();
    Ok(WireMessage {
        initial,
        header,
        prefix,
        nonce,
        ciphertext,
    })
}
