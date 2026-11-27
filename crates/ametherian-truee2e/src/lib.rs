//! # ametherian-truee2e
//!
//! An end-to-end encryption "privacy overlay" that runs on top of ordinary
//! Telegram chats between two MelloGram users. Telegram (and anyone without
//! the keys) only ever sees ciphertext carried inside normal text messages;
//! non-MelloGram recipients see an opaque armored blob.
//!
//! This crate is a pure library: no networking, no Telegram code, no UI.
//! The protocol is specified in `PROTOCOL.md` next to this crate's
//! `Cargo.toml`.
//!
//! ## Quick tour
//!
//! ```
//! use ametherian_truee2e::{
//!     decrypt, encrypt, private_chat_conversation_id, respond, IdentityKeyPair, PublicBundle,
//!     Session,
//! };
//!
//! # fn main() -> Result<(), ametherian_truee2e::Error> {
//! let alice = IdentityKeyPair::generate()?;
//! let bob = IdentityKeyPair::generate()?;
//!
//! // Bob posts his bundle in the chat; Alice imports it (signature verified).
//! let bob_bundle = PublicBundle::from_armored(&bob.public_bundle().to_armored())?;
//!
//! // Both sides must agree on the conversation id; for private chats use the
//! // order-independent helper over the two Telegram user ids.
//! let chat = private_chat_conversation_id(1111, 2222);
//!
//! let mut a = Session::initiate(&alice, &bob_bundle)?;
//! let wire = encrypt(&mut a, b"hi bob", chat)?;
//!
//! let (mut b, first) = respond(&bob, &wire, chat)?;
//! assert_eq!(first, b"hi bob");
//!
//! let reply = encrypt(&mut b, b"hi alice", chat)?;
//! assert_eq!(decrypt(&mut a, &reply, chat)?, b"hi alice");
//! assert_eq!(a.safety_number(), b.safety_number());
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]

mod crypto;
mod error;
mod keys;
mod padding;
mod ratchet;
mod safety;
mod session;
mod wire;

pub use error::{Error, Result};
pub use keys::{IdentityKeyPair, IdentityPublic, PublicBundle, BUNDLE_ARMOR_PREFIX};
pub use padding::{pad, padded_len, unpad, PAD_BUCKETS};
pub use safety::{SafetyNumber, SAFETY_QR_PREFIX};
pub use session::{
    decrypt, encrypt, encrypt_with, is_armored, is_initial_message, private_chat_conversation_id,
    respond, EncryptOptions, Session, SessionState,
};
pub use wire::{MAX_ARMORED_LEN, MAX_PLAINTEXT_LEN, MESSAGE_ARMOR_PREFIX};

/// Protocol version carried in every bundle, message and persisted state.
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum number of message keys skipped in a single receiving chain step.
pub const MAX_SKIP: u32 = 1000;

/// Maximum number of skipped message keys retained per session (oldest are
/// evicted first).
pub const MAX_SKIPPED_KEYS: usize = 1000;
