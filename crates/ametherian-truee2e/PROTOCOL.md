# MelloGram E2E overlay protocol, version 1

This document specifies the protocol implemented by `ametherian-truee2e`.
It is an X3DH-style key agreement (without one-time prekeys) followed by the
Double Ratchet, carried as armored text inside ordinary Telegram messages.

All primitives come from audited RustCrypto / dalek crates:

| Purpose            | Primitive                         | Crate                |
|--------------------|-----------------------------------|----------------------|
| Signatures         | Ed25519 (`verify_strict`, weak keys rejected) | `ed25519-dalek` 3 |
| Key agreement      | X25519 (non-contributory outputs rejected)    | `x25519-dalek` 3  |
| KDF                | HKDF-SHA-256, HMAC-SHA-256        | `hkdf`, `hmac`, `sha2` |
| AEAD               | XChaCha20-Poly1305 (24-byte random nonce) | `chacha20poly1305` |
| Fingerprints       | SHA-512 (iterated)                | `sha2`               |
| Randomness         | OS CSPRNG                         | `getrandom`          |
| Constant-time eq.  |                                   | `subtle`             |

Integers are big-endian. `||` is concatenation. All domain-separation labels
start with `mellogram/e2e/v1/`.

## 1. Identity and public bundle

Each device holds:

* `IKsig`: Ed25519 signing key
* `IK`: X25519 identity key
* `SPK`: X25519 signed prekey (rotatable)

The **identity public key** used everywhere below is
`ID = IKsig.pub (32) || IK.pub (32)` (64 bytes).

The public bundle (161 bytes):

```
version  u8 = 0x01
IKsig.pub [32]
IK.pub    [32]
SPK.pub   [32]
sig       [64]   Ed25519 by IKsig over
                 "mellogram/e2e/v1/bundle-signature" || 0x01 || IK.pub || SPK.pub
```

The signature covers the X25519 identity key as well as the prekey, so the
Ed25519 and X25519 identities are cryptographically bound together. Every
parse verifies the signature (strict verification, small-order keys rejected);
an unverified bundle cannot be constructed through the API.

Armor for posting in a chat: `mg-e2e-key:v1:` + base64url (no padding) of the
161 bytes. Bundles may later also be served by a key directory; the same
verification applies.

## 2. Session setup (X3DH-style)

Initiator A holds B's verified bundle. A generates an ephemeral X25519 key `EK`.

```
DH1 = X25519(IK_A,  SPK_B)
DH2 = X25519(EK_A,  IK_B)
DH3 = X25519(EK_A,  SPK_B)
SK  = HKDF-SHA256(
        salt = "mellogram/e2e/v1/x3dh/salt",
        ikm  = 0xFF*32 || DH1 || DH2 || DH3,
        info = "mellogram/e2e/v1/x3dh/root-key" || ID_A || ID_B,
        L    = 32)
```

Any all-zero DH output aborts setup. A identity equal to the local identity is
rejected (reflection). B computes the same values with the mirrored keys. No
one-time prekeys are used, so the first message is replayable to a *fresh*
`respond` call by an attacker; see "Replay" below for how integrators handle it.

## 3. Double Ratchet

Exactly as in Signal's published Double Ratchet specification, with
`KDF_RK`, `KDF_CK` instantiated as:

```
KDF_RK(rk, dh_out) = HKDF-SHA256(salt = rk, ikm = dh_out,
                                 info = "mellogram/e2e/v1/ratchet/root", L = 64)
                     -> (rk' = first 32, ck = last 32)
KDF_CK(ck)         = (ck' = HMAC-SHA256(ck, 0x02), mk = HMAC-SHA256(ck, 0x01))
AEAD key           = HKDF-SHA256(salt = 0x00*32, ikm = mk,
                                 info = "mellogram/e2e/v1/ratchet/message-key", L = 32)
```

* A initializes with `DHr = SPK_B`, a fresh `DHs`, and
  `(RK, CKs) = KDF_RK(SK, DH(DHs, SPK_B))`.
* B initializes with `DHs = SPK_B` and `RK = SK`, performing the first DH
  ratchet step on receipt of A's first message.
* Header (sent **in the clear**, header encryption is not used):
  `DHs.pub [32] || PN u32 || N u32`. The header is authenticated as part of the
  associated data, so it cannot be modified, but an observer learns ratchet
  public keys and message counters (they reveal message counts per chain, not
  content).
* Skipped message keys: at most `MAX_SKIP = 1000` per chain step; at most
  `MAX_SKIPPED_KEYS = 1000` retained per session (oldest evicted first).
* Each message key is used exactly once and deleted after use.

### Initial messages

Until A has received and authenticated any message from B, every message A
sends is an *initial* message that also carries `A's bundle || EK_A.pub ||
SPK_B.pub`, so B can set up the session from whichever of them arrives first.

B's `decrypt` accepts an initial message on an existing responder session only
if its `EK` and sender identity match that session; otherwise it returns
`NewSessionOffered` without touching state (the integrator decides whether to
call `respond` and replace the session, e.g. after the peer reinstalled).
Simultaneous initiation by both sides is resolved by the integrator (suggested
rule: keep the session initiated by the identity with the lexicographically
smaller `ID`).

## 4. Associated data

```
AD = "mellogram/e2e/v1/associated-data" || 0x01 (version)
     || ID_sender (64) || ID_recipient (64)
     || conversation_id (i64, 8 bytes)
     || wire prefix (everything before the nonce: version, type,
                     initial section if present, header)
```

Ciphertext therefore cannot be moved to another chat, re-attributed to another
sender/recipient, or have its header altered.

**Conversation id.** For groups and channels it is the Telegram chat id. In
Telegram private chats each side sees a different chat id (the other user's
id), so both sides must use `private_chat_conversation_id(user_a, user_b)`:
the first 8 bytes (big-endian i64) of
`SHA-256("mellogram/e2e/v1/private-chat-id" || min(a,b) || max(a,b))`.

## 5. Wire format

Binary:

```
version u8 = 0x01
type    u8 = 0x01 normal | 0x02 initial
[initial only] bundle_A [161] || EK_A.pub [32] || SPK_B.pub [32]
header  DHs.pub [32] || PN u32 || N u32
nonce   [24]           random, XChaCha20-Poly1305
ct      [>= 20]        AEAD(plaintext_padded) incl. 16-byte tag
```

Text armor: `mg-e2e:v1:` + base64url without padding. Armor longer than
64 KiB is rejected before decoding; plaintext is capped at 32 KiB. Overhead is
82 bytes for normal and 307 bytes for initial messages (before base64).

Note Telegram's 4096-character limit for text messages: padded plaintexts up
to the 1024-byte bucket always fit in one message; larger ones exceed the
limit after base64 and must be split or sent another way by the integrator.

## 6. Padding

Plaintext is encoded as `len u32 || data || zeros`, with the total rounded up
to 64, 256, 1024 or 4096 bytes, and above that to a multiple of 4096. This is
on by default (`EncryptOptions { pad: true }`). Without bucketing only the
length prefix is added. Non-zero padding or an inconsistent length is
rejected after decryption.

## 7. Replay protection

* Within a session, the receiving counter and skipped-key cache reject any
  message whose key was already consumed (`Error::Replay`), including messages
  from chains that were already ratcheted away (the last 64 receiving ratchet
  keys are remembered).
* `decrypt` is atomic: state is updated only when a message authenticates and
  decodes. Forged, tampered or wrong-chat messages leave the session untouched.
* `respond` creates a new session from an initial message. Because there are
  no one-time prekeys, an attacker can replay a captured initial message to
  `respond`; integrators should call `respond` only when no session exists
  with that peer for that chat, and pin peer identities.

## 8. Safety numbers

For each identity: `h0 = SHA-512("mellogram/e2e/v1/safety-number" || 0x0001
|| ID)`, then `h_i = SHA-512(h_{i-1} || ID)` for 5200 total iterations. The
first 30 bytes are split into six 5-byte big-endian integers, each reduced mod
100000 and printed as 5 digits. The two 30-digit halves are sorted and
concatenated, giving 60 digits shown as 12 groups of 5. The QR payload is
`mg-e2e-verify:v1:` + base64url(sorted(ID_A, ID_B)). Both are
order-independent, so both users see the same values.

## 9. Persistence

`Session::to_bytes()` produces a versioned serde (JSON) encoding containing
`format_version = 1`, both public bundles, role flags and the full ratchet
state including secrets. `IdentityKeyPair::to_bytes()` produces
`0x01 || ed25519 seed || IK secret || SPK secret`. **Both outputs are secret:
the integrator must encrypt them at rest** (platform keystore / OS keychain).
Returned buffers are `Zeroizing<Vec<u8>>`; `from_bytes` validates versions
and invariants and caps input size (4 MiB).

## 10. Hygiene

* `#![forbid(unsafe_code)]`; `unwrap`/`expect`/indexing/`panic` denied by lint.
* All parsing is bounds-checked and returns typed errors.
* Secret-bearing types implement `ZeroizeOnDrop` and redact `Debug`.
