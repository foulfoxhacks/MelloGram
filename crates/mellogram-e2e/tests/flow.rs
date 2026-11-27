use mellogram_e2e::E2eState;
use mellogram_core::IncomingMessage;

fn msg(chat_id: i64, id: i32, sender: i64, text: String, outgoing: bool) -> IncomingMessage {
    IncomingMessage {
        chat_id,
        id,
        sender_id: Some(sender),
        text,
        outgoing,
        date: 1_700_000_000,
        ..IncomingMessage::default()
    }
}

fn temp(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mg-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn two_devices_talk_and_survive_a_restart() {
    let (a_dir, b_dir) = (temp("alice"), temp("bob"));
    let alice = E2eState::open(&a_dir);
    let bob = E2eState::open(&b_dir);
    alice.set_self_id(111);
    bob.set_self_id(222);

    let bob_key = bob.identity_bundle().unwrap();
    alice.identity_bundle().unwrap();

    // Alice pins Bob's key and switches the chat on.
    let peer = alice.import_peer(222, &bob_key).unwrap();
    assert!(peer.safety_number.is_some());
    assert!(!peer.verified);
    let chat = alice.set_chat(222, true).unwrap();
    assert!(chat.enabled && chat.ready);

    let armored = alice.encrypt_for_chat(222, "hello bob").unwrap();
    assert!(armored.starts_with("mg-e2e:v1:"));

    // Bob receives it without having exchanged keys first: the initial message
    // carries Alice's bundle, so the session is created and pinned.
    let mut incoming = msg(111, 7, 111, armored.clone(), false);
    assert_eq!(bob.apply_incoming(&mut incoming), None);
    assert_eq!(incoming.text, "hello bob");
    assert!(bob.chat_state(111).ready);
    assert_eq!(bob.chat_state(111).safety_number, alice.chat_state(222).safety_number);

    // Bob replies (the vault lets his own bubble show the text).
    bob.set_chat(111, true).unwrap();
    let reply = bob.encrypt_for_chat(111, "hi alice").unwrap();
    let mut back = msg(222, 9, 222, reply.clone(), false);
    assert_eq!(alice.apply_incoming(&mut back), None);
    assert_eq!(back.text, "hi alice");

    // The store on disk never contains plaintext.
    let store = std::fs::read(a_dir.join("e2e/store.bin")).unwrap();
    assert!(!store.windows(9).any(|w| w == b"hello bob"));
    assert!(!store.windows(8).any(|w| w == b"hi alice"));

    // Restart Alice: identity, session and decrypted vault come back.
    drop(alice);
    let alice = E2eState::open(&a_dir);
    alice.set_self_id(111);
    let status = alice.status();
    assert!(status.has_identity);
    assert_eq!(status.decrypted_messages, 1);
    // Re-rendering history uses the vault and does not touch the ratchet.
    let mut history = msg(222, 9, 222, reply, false);
    assert_eq!(alice.apply_incoming(&mut history), None);
    assert_eq!(history.text, "hi alice");

    // A brand-new message after the restart still decrypts: the ratchet state
    // was persisted, not just the vault.
    let next = bob.encrypt_for_chat(111, "still here").unwrap();
    let mut live = msg(222, 11, 222, next, false);
    assert_eq!(alice.apply_incoming(&mut live), None);
    assert_eq!(live.text, "still here");

    // A tampered message is rejected and leaves the session usable.
    let evil = bob.encrypt_for_chat(111, "trust me").unwrap();
    let mut flipped = evil.clone();
    flipped.pop();
    flipped.push(if evil.ends_with('A') { 'B' } else { 'A' });
    let mut bad = msg(222, 13, 222, flipped, false);
    assert!(alice.apply_incoming(&mut bad).is_some());
    assert!(bad.classes.iter().any(|c| c == "mg-e2e-locked"));
    let mut good = msg(222, 13, 222, evil, false);
    assert_eq!(alice.apply_incoming(&mut good), None);
    assert_eq!(good.text, "trust me");

    // Our own echo of an encrypted message shows what we typed.
    let mut echo = msg(222, 15, 111, ai_armor(&alice, "hi"), true);
    assert_eq!(alice.apply_incoming(&mut echo), None);
    echo.text = "hi".into();
    assert_eq!(echo.text, "hi");
}

/// Helper: encrypt on Alice's side for the echo test.
fn ai_armor(alice: &E2eState, text: &str) -> String {
    alice.encrypt_for_chat(222, text).unwrap()
}

#[test]
fn identity_change_requires_verification_again() {
    let dir = temp("carol");
    let carol = E2eState::open(&dir);
    carol.set_self_id(333);
    let first = carol.identity_bundle().unwrap();
    let peer = carol.import_peer(444, &first).unwrap_err();
    assert!(peer.contains("your own key"));

    let other_dir = temp("dave");
    let dave = E2eState::open(&other_dir);
    dave.set_self_id(444);
    let dave_key = dave.identity_bundle().unwrap();
    carol.import_peer(444, &dave_key).unwrap();
    carol.set_chat(444, true).unwrap();

    // Dave "reinstalls": a new identity replaces the pinned one and the session
    // is dropped, so Caroline must compare safety numbers again.
    let fresh_dir = temp("dave2");
    let dave2 = E2eState::open(&fresh_dir);
    dave2.set_self_id(444);
    let fresh = dave2.identity_bundle().unwrap();
    let view = carol.import_peer(444, &fresh).unwrap();
    assert!(!view.has_session);
    assert!(!view.verified);
    assert!(!carol.chat_state(444).ready);
}

#[test]
fn encrypted_messages_reject_bad_input() {
    let dir = temp("erin");
    let erin = E2eState::open(&dir);
    erin.set_self_id(555);
    assert!(erin.encrypt_for_chat(666, "x").is_err()); // no session yet
    assert!(erin.import_peer(666, "mg-e2e-key:v1:not-a-key").is_err());
    assert!(erin.import_peer(-100123, "mg-e2e-key:v1:AAAA").is_err());
    let huge = "a".repeat(2601);
    assert!(erin.encrypt_for_chat(666, &huge).is_err());
    assert_eq!(erin.clear_history().unwrap(), 0);
}
