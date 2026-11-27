//! Typed errors. No variant ever carries secret material.

/// Errors returned by this crate. Every parse / decrypt path returns one of
/// these instead of panicking on hostile input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The text does not start with the expected armor prefix.
    #[error("not an armored MelloGram E2E payload")]
    NotArmored,
    /// The input exceeds the configured size cap.
    #[error("input too large")]
    TooLarge,
    /// The armor body is not valid unpadded base64url.
    #[error("invalid base64url encoding")]
    Encoding,
    /// The binary payload ended before a required field.
    #[error("truncated payload")]
    Truncated,
    /// The binary payload has unexpected bytes after the last field.
    #[error("trailing data in payload")]
    TrailingData,
    /// A version byte / field this implementation does not understand.
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u8),
    /// Unknown message type byte.
    #[error("unknown message type {0}")]
    UnknownMessageType(u8),
    /// A public key failed to decode or is a weak / small-order key.
    #[error("invalid or weak public key")]
    InvalidKey,
    /// A bundle signature did not verify.
    #[error("bundle signature verification failed")]
    BadSignature,
    /// A Diffie-Hellman output was all-zero (peer sent a low-order point).
    #[error("non-contributory key exchange")]
    NonContributory,
    /// The initial message targets a signed prekey we do not hold.
    #[error("initial message references an unknown signed prekey")]
    UnknownPrekey,
    /// The peer presented our own identity key.
    #[error("peer identity equals local identity")]
    ReflectedIdentity,
    /// Authentication failed: wrong key, wrong chat, tampering, or forgery.
    #[error("decryption failed")]
    DecryptionFailed,
    /// The message was already received (or is older than the replay window).
    #[error("duplicate or replayed message")]
    Replay,
    /// The header asks us to skip more message keys than allowed.
    #[error("too many skipped messages")]
    TooManySkipped,
    /// An initial (session-setup) message arrived for an existing session
    /// with a different ephemeral key: the peer started a new session.
    /// No state was changed; the integrator may call [`crate::respond`].
    #[error("peer offered a new session")]
    NewSessionOffered,
    /// [`crate::respond`] was given a message that is not an initial message.
    #[error("not a session-setup message")]
    NotInitialMessage,
    /// The plaintext is larger than [`crate::MAX_PLAINTEXT_LEN`].
    #[error("plaintext too large")]
    PlaintextTooLarge,
    /// Decrypted padding is malformed.
    #[error("invalid padding")]
    BadPadding,
    /// A 32-bit message counter would overflow.
    #[error("message counter exhausted")]
    CounterExhausted,
    /// The operating system random number generator failed.
    #[error("random number generator failure")]
    Rng,
    /// Persisted state failed to (de)serialize or violates invariants.
    #[error("invalid persisted state")]
    InvalidState,
    /// Internal cryptographic library error (should not happen).
    #[error("internal cryptographic error")]
    Internal,
}

/// Crate result alias.
pub type Result<T> = core::result::Result<T, Error>;
