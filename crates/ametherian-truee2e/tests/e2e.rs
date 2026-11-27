#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ametherian_truee2e::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

const CHAT: i64 = -1001234567890;

struct Pair {
    alice: IdentityKeyPair,
    bob: IdentityKeyPair,
    a: Session,
    b: Session,
}

/// Full handshake: bundle exchange via armor, first message, reply.
fn setup() -> Pair {
    let alice = IdentityKeyPair::generate().unwrap();
    let bob = IdentityKeyPair::generate().unwrap();
    let bob_bundle = PublicBundle::from_armored(&bob.public_bundle().to_armored()).unwrap();
    let mut a = Session::initiate(&alice, &bob_bundle).unwrap();
    let m = encrypt(&mut a, b"hello", CHAT).unwrap();
    assert!(is_armored(&m));
    assert!(is_initial_message(&m));
    let (b, pt) = respond(&bob, &m, CHAT).unwrap();
    assert_eq!(pt, b"hello");
    assert_eq!(b.peer_identity(), alice.identity());
    assert_eq!(a.peer_identity(), bob.identity());
    Pair { alice, bob, a, b }
}

fn unarmor(s: &str) -> Vec<u8> {
    URL_SAFE_NO_PAD
        .decode(s.strip_prefix(MESSAGE_ARMOR_PREFIX).unwrap())
        .unwrap()
}

fn rearmor(b: &[u8]) -> String {
    format!("{MESSAGE_ARMOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(b))
}

/// Deterministic xorshift for fuzz loops.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

#[test]
fn handshake_and_bidirectional_conversation() {
    let mut p = setup();
    assert!(p.a.awaiting_reply());
    for i in 0..20u32 {
        let msg = format!("a->b {i}");
        let ct = encrypt(&mut p.a, msg.as_bytes(), CHAT).unwrap();
        assert!(!ct.contains(&msg));
        assert_eq!(decrypt(&mut p.b, &ct, CHAT).unwrap(), msg.as_bytes());
        // Several messages in a row from Bob, then alternate.
        for j in 0..(i % 3 + 1) {
            let msg = format!("b->a {i}.{j}");
            let ct = encrypt(&mut p.b, msg.as_bytes(), CHAT).unwrap();
            assert!(!is_initial_message(&ct));
            assert_eq!(decrypt(&mut p.a, &ct, CHAT).unwrap(), msg.as_bytes());
        }
        assert!(!p.a.awaiting_reply());
        let ct = encrypt(&mut p.a, b"", CHAT).unwrap();
        assert!(!is_initial_message(&ct));
        assert_eq!(decrypt(&mut p.b, &ct, CHAT).unwrap(), b"");
    }
}

#[test]
fn initial_messages_resent_until_reply() {
    let alice = IdentityKeyPair::generate().unwrap();
    let bob = IdentityKeyPair::generate().unwrap();
    let mut a = Session::initiate(&alice, &bob.public_bundle()).unwrap();
    let m: Vec<String> = (0..5)
        .map(|i| encrypt(&mut a, format!("m{i}").as_bytes(), CHAT).unwrap())
        .collect();
    assert!(m.iter().all(|x| is_initial_message(x)));
    // Out of order, starting with m2.
    let (mut b, pt) = respond(&bob, &m[2], CHAT).unwrap();
    assert_eq!(pt, b"m2");
    for i in [0usize, 4, 1, 3] {
        assert_eq!(
            decrypt(&mut b, &m[i], CHAT).unwrap(),
            format!("m{i}").as_bytes()
        );
    }
    for x in &m {
        assert_eq!(decrypt(&mut b, x, CHAT), Err(Error::Replay));
    }
    let r = encrypt(&mut b, b"reply", CHAT).unwrap();
    assert_eq!(decrypt(&mut a, &r, CHAT).unwrap(), b"reply");
    assert!(!is_initial_message(&encrypt(&mut a, b"x", CHAT).unwrap()));
}

#[test]
fn out_of_order_across_ratchet_steps() {
    let mut p = setup();
    let r = encrypt(&mut p.b, b"r", CHAT).unwrap();
    decrypt(&mut p.a, &r, CHAT).unwrap();

    let a0 = encrypt(&mut p.a, b"a0", CHAT).unwrap();
    let a1 = encrypt(&mut p.a, b"a1", CHAT).unwrap();
    let b0 = encrypt(&mut p.b, b"b0", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.a, &b0, CHAT).unwrap(), b"b0");
    let a2 = encrypt(&mut p.a, b"a2", CHAT).unwrap(); // new chain
    let a3 = encrypt(&mut p.a, b"a3", CHAT).unwrap();

    assert_eq!(decrypt(&mut p.b, &a3, CHAT).unwrap(), b"a3");
    assert_eq!(decrypt(&mut p.b, &a1, CHAT).unwrap(), b"a1");
    assert_eq!(decrypt(&mut p.b, &a2, CHAT).unwrap(), b"a2");
    assert_eq!(decrypt(&mut p.b, &a0, CHAT).unwrap(), b"a0");
    for x in [&a0, &a1, &a2, &a3] {
        assert_eq!(decrypt(&mut p.b, x, CHAT), Err(Error::Replay));
    }
}

#[test]
fn replay_rejected() {
    let mut p = setup();
    let m = encrypt(&mut p.a, b"once", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.b, &m, CHAT).unwrap(), b"once");
    assert_eq!(decrypt(&mut p.b, &m, CHAT), Err(Error::Replay));

    // Replay of a message from a retired chain after several ratchet steps.
    for _ in 0..3 {
        let r = encrypt(&mut p.b, b"r", CHAT).unwrap();
        decrypt(&mut p.a, &r, CHAT).unwrap();
        let s = encrypt(&mut p.a, b"s", CHAT).unwrap();
        decrypt(&mut p.b, &s, CHAT).unwrap();
    }
    assert_eq!(decrypt(&mut p.b, &m, CHAT), Err(Error::Replay));

    // Replaying the very first (setup) message into the live session.
    let first = {
        let alice = IdentityKeyPair::generate().unwrap();
        let mut a = Session::initiate(&alice, &p.bob.public_bundle()).unwrap();
        let f = encrypt(&mut a, b"x", CHAT).unwrap();
        let (mut b, _) = respond(&p.bob, &f, CHAT).unwrap();
        assert_eq!(decrypt(&mut b, &f, CHAT), Err(Error::Replay));
        f
    };
    // ...and into an unrelated session: treated as a new session offer.
    assert_eq!(
        decrypt(&mut p.b, &first, CHAT),
        Err(Error::NewSessionOffered)
    );
}

#[test]
fn tampering_any_byte_is_rejected_and_state_untouched() {
    for initial in [true, false] {
        let mut p = setup();
        if !initial {
            let r = encrypt(&mut p.b, b"r", CHAT).unwrap();
            decrypt(&mut p.a, &r, CHAT).unwrap();
        }
        let m = encrypt(&mut p.a, b"attack at dawn", CHAT).unwrap();
        assert_eq!(is_initial_message(&m), initial && p.a.awaiting_reply());
        let bin = unarmor(&m);
        for i in 0..bin.len() {
            for flip in [0x01u8, 0x80] {
                let mut t = bin.clone();
                t[i] ^= flip;
                assert!(
                    decrypt(&mut p.b, &rearmor(&t), CHAT).is_err(),
                    "byte {i} flip {flip:#x}"
                );
            }
        }
        // Truncations and extensions.
        for n in 0..bin.len() {
            assert!(decrypt(&mut p.b, &rearmor(&bin[..n]), CHAT).is_err());
        }
        let mut ext = bin.clone();
        ext.push(0);
        assert!(decrypt(&mut p.b, &rearmor(&ext), CHAT).is_err());
        // Session still accepts the genuine message.
        assert_eq!(decrypt(&mut p.b, &m, CHAT).unwrap(), b"attack at dawn");
    }
}

#[test]
fn wrong_chat_id_rejected() {
    let mut p = setup();
    let m = encrypt(&mut p.a, b"for this chat only", CHAT).unwrap();
    assert_eq!(
        decrypt(&mut p.b, &m, CHAT + 1),
        Err(Error::DecryptionFailed)
    );
    assert_eq!(decrypt(&mut p.b, &m, 0), Err(Error::DecryptionFailed));
    assert_eq!(decrypt(&mut p.b, &m, CHAT).unwrap(), b"for this chat only");

    // The setup message is also bound to the chat.
    let alice = IdentityKeyPair::generate().unwrap();
    let mut a = Session::initiate(&alice, &p.bob.public_bundle()).unwrap();
    let f = encrypt(&mut a, b"x", CHAT).unwrap();
    assert_eq!(
        respond(&p.bob, &f, CHAT + 1).unwrap_err(),
        Error::DecryptionFailed
    );
}

#[test]
fn ciphertext_not_reattributable_to_other_identity() {
    // Mallory swaps the sender bundle inside Alice's setup message for her
    // own (validly signed) bundle: the AD / X3DH binding makes it fail.
    let p = setup();
    let mallory = IdentityKeyPair::generate().unwrap();
    let alice2 = IdentityKeyPair::generate().unwrap();
    let mut a = Session::initiate(&alice2, &p.bob.public_bundle()).unwrap();
    let m = encrypt(&mut a, b"x", CHAT).unwrap();
    let mut bin = unarmor(&m);
    let mb = mallory.public_bundle().to_bytes();
    bin[2..2 + mb.len()].copy_from_slice(&mb);
    assert!(respond(&p.bob, &rearmor(&bin), CHAT).is_err());
    // And a third party cannot decrypt at all.
    let carol = IdentityKeyPair::generate().unwrap();
    assert_eq!(respond(&carol, &m, CHAT).unwrap_err(), Error::UnknownPrekey);
    let _ = (&p.alice, &p.a);
}

#[test]
fn bundle_signature_forgery_rejected() {
    let bob = IdentityKeyPair::generate().unwrap();
    let eve = IdentityKeyPair::generate().unwrap();
    let good = bob.public_bundle().to_bytes();
    assert!(PublicBundle::from_bytes(&good).is_ok());

    // Flip every byte of the signature / keys.
    for i in 1..good.len() {
        let mut t = good.clone();
        t[i] ^= 0x01;
        assert!(PublicBundle::from_bytes(&t).is_err(), "byte {i}");
    }
    // Substitute Eve's prekey under Bob's signature.
    let mut t = good.clone();
    t[65..97].copy_from_slice(&eve.public_bundle().signed_prekey());
    assert_eq!(PublicBundle::from_bytes(&t), Err(Error::BadSignature));
    // Eve's signature claiming Bob's identity.
    let eb = eve.public_bundle().to_bytes();
    let mut t = good.clone();
    t[97..].copy_from_slice(&eb[97..]);
    assert_eq!(PublicBundle::from_bytes(&t), Err(Error::BadSignature));
    // Weak (identity point) ed25519 key.
    let mut t = good.clone();
    t[1..33].copy_from_slice(&{
        let mut k = [0u8; 32];
        k[0] = 1;
        k
    });
    assert!(PublicBundle::from_bytes(&t).is_err());
    // Version / length.
    let mut t = good.clone();
    t[0] = 2;
    assert_eq!(
        PublicBundle::from_bytes(&t),
        Err(Error::UnsupportedVersion(2))
    );
    assert_eq!(
        PublicBundle::from_bytes(&good[..100]),
        Err(Error::Truncated)
    );
    let mut t = good.clone();
    t.push(0);
    assert_eq!(PublicBundle::from_bytes(&t), Err(Error::TrailingData));
    // Armor.
    assert_eq!(PublicBundle::from_armored("hello"), Err(Error::NotArmored));
    assert_eq!(
        PublicBundle::from_armored("mg-e2e-key:v1:!!"),
        Err(Error::Encoding)
    );
    assert!(PublicBundle::is_armored(&bob.public_bundle().to_armored()));
}

#[test]
fn malformed_armor_returns_errors() {
    let mut p = setup();
    assert_eq!(decrypt(&mut p.b, "", CHAT), Err(Error::NotArmored));
    assert_eq!(
        decrypt(&mut p.b, "hello world", CHAT),
        Err(Error::NotArmored)
    );
    assert_eq!(decrypt(&mut p.b, "mg-e2e:v1:", CHAT), Err(Error::Truncated));
    assert_eq!(
        decrypt(&mut p.b, "mg-e2e:v1:***", CHAT),
        Err(Error::Encoding)
    );
    assert_eq!(
        decrypt(&mut p.b, "mg-e2e:v1:AAAA====", CHAT),
        Err(Error::Encoding)
    );
    let big = format!("{MESSAGE_ARMOR_PREFIX}{}", "A".repeat(MAX_ARMORED_LEN));
    assert_eq!(decrypt(&mut p.b, &big, CHAT), Err(Error::TooLarge));
    assert_eq!(respond(&p.bob, &big, CHAT).unwrap_err(), Error::TooLarge);
    assert_eq!(
        decrypt(&mut p.b, &rearmor(&[9, 1]), CHAT),
        Err(Error::UnsupportedVersion(9))
    );
    assert_eq!(
        decrypt(&mut p.b, &rearmor(&[1, 7]), CHAT),
        Err(Error::UnknownMessageType(7))
    );
    let normal = encrypt(&mut p.b, b"x", CHAT).unwrap();
    assert_eq!(
        respond(&p.alice, &normal, CHAT).unwrap_err(),
        Error::NotInitialMessage
    );
    assert_eq!(
        encrypt(&mut p.a, &vec![0u8; MAX_PLAINTEXT_LEN + 1], CHAT),
        Err(Error::PlaintextTooLarge)
    );
    // Max-size plaintext still round-trips (and stays under the armor cap).
    let big_pt = vec![0x5Au8; MAX_PLAINTEXT_LEN];
    let fresh = setup();
    let mut a = fresh.a;
    let ct = encrypt(&mut a, &big_pt, CHAT).unwrap();
    assert!(ct.len() <= MAX_ARMORED_LEN);
    let (_, pt) = respond(&fresh.bob, &ct, CHAT).unwrap();
    assert_eq!(pt, big_pt);
}

#[test]
fn fuzz_random_inputs_never_panic() {
    let mut p = setup();
    let valid_initial = {
        let mut a = Session::initiate(&p.alice, &p.bob.public_bundle()).unwrap();
        unarmor(&encrypt(&mut a, b"seed", CHAT).unwrap())
    };
    let valid_normal = unarmor(&encrypt(&mut p.b, b"seed", CHAT).unwrap());
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in 0..4000 {
        let len = (rng.next() % 400) as usize;
        let mut bytes = rng.bytes(len);
        // Make some inputs structurally plausible to reach deeper parsing.
        match i % 4 {
            0 if !bytes.is_empty() => bytes[0] = 1,
            1 if bytes.len() > 1 => {
                bytes[0] = 1;
                bytes[1] = 1 + (i as u8 % 2);
            }
            2 => {
                bytes = if i % 8 == 2 {
                    valid_initial.clone()
                } else {
                    valid_normal.clone()
                };
                let n = bytes.len();
                for _ in 0..(1 + rng.next() % 4) {
                    let at = (rng.next() as usize) % n;
                    bytes[at] = rng.next() as u8;
                }
                bytes.truncate((rng.next() as usize) % (n + 1));
            }
            _ => {}
        }
        let s = rearmor(&bytes);
        let _ = decrypt(&mut p.a, &s, CHAT);
        let _ = decrypt(&mut p.b, &s, CHAT);
        let _ = respond(&p.bob, &s, CHAT);
        let _ = is_initial_message(&s);
        let _ = PublicBundle::from_bytes(&bytes);
        let _ = PublicBundle::from_armored(&format!(
            "{BUNDLE_ARMOR_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(&bytes)
        ));
        let _ = IdentityKeyPair::from_bytes(&bytes);
        let _ = Session::from_bytes(&bytes);
        let _ = unpad(&bytes);
        // Raw (possibly non-UTF-8-ish) text after the prefix.
        let raw = format!("{MESSAGE_ARMOR_PREFIX}{}", String::from_utf8_lossy(&bytes));
        let _ = decrypt(&mut p.b, &raw, CHAT);
    }
    // Sessions survived intact.
    let m = encrypt(&mut p.a, b"still ok", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.b, &m, CHAT).unwrap(), b"still ok");
}

#[test]
fn session_persistence_round_trip_mid_conversation() {
    let mut p = setup();
    let r = encrypt(&mut p.b, b"r", CHAT).unwrap();
    decrypt(&mut p.a, &r, CHAT).unwrap();
    // Leave skipped keys pending on Bob's side.
    let a0 = encrypt(&mut p.a, b"a0", CHAT).unwrap();
    let a1 = encrypt(&mut p.a, b"a1", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.b, &a1, CHAT).unwrap(), b"a1");

    let a_bytes = p.a.to_bytes().unwrap();
    let b_bytes = p.b.to_bytes().unwrap();
    drop(p.a);
    drop(p.b);
    let mut a = SessionState::from_bytes(&a_bytes).unwrap();
    let mut b = SessionState::from_bytes(&b_bytes).unwrap();

    assert_eq!(decrypt(&mut b, &a0, CHAT).unwrap(), b"a0");
    assert_eq!(decrypt(&mut b, &a1, CHAT), Err(Error::Replay));
    let x = encrypt(&mut b, b"after restore", CHAT).unwrap();
    assert_eq!(decrypt(&mut a, &x, CHAT).unwrap(), b"after restore");
    let y = encrypt(&mut a, b"and back", CHAT).unwrap();
    assert_eq!(decrypt(&mut b, &y, CHAT).unwrap(), b"and back");

    // Identity key persistence.
    let id_bytes = p.bob.to_bytes();
    let bob2 = IdentityKeyPair::from_bytes(&id_bytes).unwrap();
    assert_eq!(bob2.identity(), p.bob.identity());
    assert_eq!(bob2.public_bundle(), p.bob.public_bundle());

    // Corrupted / wrong-version state is rejected.
    assert_eq!(Session::from_bytes(b"{}").unwrap_err(), Error::InvalidState);
    let s = String::from_utf8(a_bytes.to_vec()).unwrap();
    let bumped = s.replacen("\"format_version\":1", "\"format_version\":2", 1);
    assert_ne!(s, bumped);
    assert_eq!(
        Session::from_bytes(bumped.as_bytes()).unwrap_err(),
        Error::InvalidState
    );
}

#[test]
fn safety_numbers_match_and_are_order_independent() {
    let p = setup();
    let sa = p.a.safety_number();
    let sb = p.b.safety_number();
    assert_eq!(sa, sb);
    assert_eq!(
        SafetyNumber::compute(&p.alice.identity(), &p.bob.identity()),
        SafetyNumber::compute(&p.bob.identity(), &p.alice.identity())
    );
    assert_eq!(sa.digits().len(), 60);
    assert!(sa.digits().bytes().all(|c| c.is_ascii_digit()));
    assert_eq!(sa.groups().len(), 12);
    assert!(sa.groups().iter().all(|g| g.len() == 5));
    assert_eq!(sa.display().len(), 60 + 11);
    assert!(sa.matches_digits(&sb.display()));
    assert!(sa.matches_qr(sb.qr_payload()));
    assert!(sa.qr_payload().starts_with(SAFETY_QR_PREFIX));

    let eve = IdentityKeyPair::generate().unwrap();
    let se = SafetyNumber::compute(&p.alice.identity(), &eve.identity());
    assert_ne!(se, sa);
    assert!(!sa.matches_qr(se.qr_payload()));
    assert!(!sa.matches_digits(se.digits()));
}

#[test]
fn padding_hides_length_by_default() {
    let mut p = setup();
    let short = encrypt(&mut p.a, b"k", CHAT).unwrap();
    let longer = encrypt(&mut p.a, &[b'x'; 50], CHAT).unwrap();
    assert_eq!(short.len(), longer.len());
    let o = EncryptOptions { pad: false };
    let s2 = encrypt_with(&mut p.a, b"k", CHAT, o).unwrap();
    let l2 = encrypt_with(&mut p.a, &[b'x'; 50], CHAT, o).unwrap();
    assert!(s2.len() < l2.len());
    for (m, want) in [
        (&short, &b"k"[..]),
        (&longer, &[b'x'; 50][..]),
        (&s2, b"k"),
        (&l2, &[b'x'; 50]),
    ] {
        assert_eq!(decrypt(&mut p.b, m, CHAT).unwrap(), want);
    }
}

#[test]
fn new_session_offer_and_reflection_and_prekey_rotation() {
    let mut p = setup();
    // Alice reinstalls / restarts the session.
    let mut a2 = Session::initiate(&p.alice, &p.bob.public_bundle()).unwrap();
    let m = encrypt(&mut a2, b"new", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.b, &m, CHAT), Err(Error::NewSessionOffered));
    let (_, pt) = respond(&p.bob, &m, CHAT).unwrap();
    assert_eq!(pt, b"new");
    // Initiator receiving a setup message.
    let mut b2 = Session::initiate(&p.bob, &p.alice.public_bundle()).unwrap();
    let mb = encrypt(&mut b2, b"x", CHAT).unwrap();
    assert_eq!(decrypt(&mut p.a, &mb, CHAT), Err(Error::NewSessionOffered));

    assert_eq!(
        Session::initiate(&p.alice, &p.alice.public_bundle()).unwrap_err(),
        Error::ReflectedIdentity
    );

    let mut bob = IdentityKeyPair::from_bytes(&p.bob.to_bytes()).unwrap();
    let old = bob.public_bundle();
    bob.rotate_signed_prekey().unwrap();
    assert_ne!(old.signed_prekey(), bob.public_bundle().signed_prekey());
    let mut a3 = Session::initiate(&p.alice, &old).unwrap();
    let m3 = encrypt(&mut a3, b"x", CHAT).unwrap();
    assert_eq!(respond(&bob, &m3, CHAT).unwrap_err(), Error::UnknownPrekey);
}

#[test]
fn secrets_are_redacted_in_debug() {
    let p = setup();
    let id_dbg = format!("{:?}", p.alice);
    assert!(id_dbg.contains("redacted"));
    let secret = p.alice.to_bytes();
    let hex: String = secret[1..33].iter().map(|b| format!("{b:02x}")).collect();
    assert!(!id_dbg.contains(&hex));
    assert!(!id_dbg.contains(&format!("{:?}", &secret[1..33])));
    let s_dbg = format!("{:?}", p.a);
    assert!(s_dbg.contains("redacted"));
}

#[test]
fn private_chat_id_is_order_independent() {
    assert_eq!(
        private_chat_conversation_id(5, 9),
        private_chat_conversation_id(9, 5)
    );
    assert_ne!(
        private_chat_conversation_id(5, 9),
        private_chat_conversation_id(5, 10)
    );
    // Works end to end when each side computes it with swapped arguments.
    let mut p = setup();
    let m = encrypt(&mut p.b, b"pm", private_chat_conversation_id(111, 222)).unwrap();
    assert_eq!(
        decrypt(&mut p.a, &m, private_chat_conversation_id(222, 111)).unwrap(),
        b"pm"
    );
}
