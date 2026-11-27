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
    let dir = std::env::temp_dir().join(format!("mg-e2e-group-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn device(tag: &str, id: i64) -> E2eState {
    let state = E2eState::open(&temp(tag));
    state.set_self_id(id);
    state
}

/// Delivers Alice's group key to Bob: the envelope travels inside their
/// pairwise session, so Bob's client installs it on receipt.
fn deliver_key(alice: &E2eState, bob: &E2eState, group: i64, message_id: i32) {
    let envelopes = alice.group_distributions(group, &[bob.self_id()]).unwrap();
    assert_eq!(envelopes.len(), 1, "expected one envelope for the reachable member");
    let armored = alice.encrypt_for_peer(bob.self_id(), &envelopes[0].text).unwrap();
    let mut incoming = msg(alice.self_id(), message_id, alice.self_id(), armored, false);
    assert_eq!(bob.apply_incoming(&mut incoming), None);
    assert!(incoming.text.starts_with("🔑"), "got {:?}", incoming.text);
}

#[test]
fn group_sender_keys_round_trip() {
    const GROUP: i64 = -1001234567890;
    let alice = device("alice", 1);
    let bob = device("bob", 2);
    let carol = device("carol", 3);

    let bob_key = bob.identity_bundle().unwrap();
    let alice_key = alice.identity_bundle().unwrap();
    carol.identity_bundle().unwrap();

    // Only Bob has a pairwise overlay key with Alice; Carol is in the group but
    // not reachable yet.
    alice.import_peer(2, &bob_key).unwrap();
    alice.set_chat(2, true).unwrap();
    bob.import_peer(1, &alice_key).unwrap();

    let view = alice.group_enable(GROUP).unwrap();
    assert!(view.enabled && view.has_sender_key);
    assert_eq!(view.pairwise_peers, 1);
    assert!(alice.group_distributions(GROUP, &[2, 3]).unwrap().len() == 1);

    deliver_key(&alice, &bob, GROUP, 5);
    assert_eq!(bob.group_state(GROUP).senders.len(), 1);

    let armored = alice.group_encrypt(GROUP, "hello group").unwrap();
    assert!(armored.starts_with("mg-e2e-g:v1:"));
    let mut incoming = msg(GROUP, 10, 1, armored.clone(), false);
    assert_eq!(bob.apply_incoming(&mut incoming), None);
    assert_eq!(incoming.text, "hello group");

    // Carol never received the key: the ciphertext stays locked, not garbage.
    let mut blocked = msg(GROUP, 10, 1, armored, false);
    assert!(carol.apply_incoming(&mut blocked).is_some());
    assert!(blocked.classes.iter().any(|c| c == "mg-e2e-locked"));

    // A group message in a one-to-one chat is refused.
    let mut misuse = msg(2, 11, 1, alice.group_encrypt(GROUP, "x").unwrap(), false);
    assert!(alice.apply_incoming(&mut misuse).is_some());
}

#[test]
fn membership_change_forces_a_rotation() {
    const GROUP: i64 = -1009876543210;
    let alice = device("alice2", 1);
    let bob = device("bob2", 2);
    let bob_key = bob.identity_bundle().unwrap();
    alice.identity_bundle().unwrap();
    bob.identity_bundle().unwrap();
    alice.import_peer(2, &bob_key).unwrap();
    alice.set_chat(2, true).unwrap();
    alice.group_enable(GROUP).unwrap();
    deliver_key(&alice, &bob, GROUP, 5);

    let first = alice.group_encrypt(GROUP, "before").unwrap();
    let mut incoming = msg(GROUP, 20, 1, first, false);
    assert_eq!(bob.apply_incoming(&mut incoming), None);
    assert_eq!(incoming.text, "before");

    // Bob is removed: Alice must rotate before sending again.
    alice.group_mark_rotation(GROUP);
    assert!(alice.group_state(GROUP).rotation_needed);
    let refused = alice.group_encrypt(GROUP, "should not send");
    assert!(refused.unwrap_err().contains("rotate"));

    let rotated = alice.group_rotate(GROUP).unwrap();
    assert!(!rotated.rotation_needed);

    // Bob still holds the old chain, so the new key is unreadable to him until
    // it is delivered again.
    let stale = alice.group_encrypt(GROUP, "new key").unwrap();
    let mut stale_in = msg(GROUP, 21, 1, stale, false);
    assert!(bob.apply_incoming(&mut stale_in).is_some());
    assert!(stale_in.classes.iter().any(|c| c == "mg-e2e-locked"));

    deliver_key(&alice, &bob, GROUP, 6);
    let mut fresh = msg(GROUP, 22, 1, alice.group_encrypt(GROUP, "readable again").unwrap(), false);
    assert_eq!(bob.apply_incoming(&mut fresh), None);
    assert_eq!(fresh.text, "readable again");

    alice.group_disable(GROUP).unwrap();
    assert!(!alice.group_state(GROUP).enabled);
    assert!(alice.group_encrypt(GROUP, "off").is_err());
}

#[test]
fn group_messages_reject_tampering_and_persist() {
    const GROUP: i64 = -1005550001111;
    let dir = temp("persist");
    let alice = E2eState::open(&dir);
    alice.set_self_id(1);
    let bob_dir = temp("bob3");
    let bob = E2eState::open(&bob_dir);
    bob.set_self_id(2);
    let bob_key = bob.identity_bundle().unwrap();
    alice.identity_bundle().unwrap();
    bob.identity_bundle().unwrap();
    alice.import_peer(2, &bob_key).unwrap();
    alice.set_chat(2, true).unwrap();
    alice.group_enable(GROUP).unwrap();
    deliver_key(&alice, &bob, GROUP, 5);

    let good = alice.group_encrypt(GROUP, "payload").unwrap();
    let mut tampered = good.clone().into_bytes();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(tampered).unwrap();
    let mut bad = msg(GROUP, 30, 1, tampered, false);
    assert!(bob.apply_incoming(&mut bad).is_some());
    let mut real = msg(GROUP, 30, 1, good, false);
    assert_eq!(bob.apply_incoming(&mut real), None);

    // Restart Bob: the receiver chain, the group and the key come back.
    drop(bob);
    let bob = E2eState::open(&bob_dir);
    bob.set_self_id(2);
    assert_eq!(bob.group_state(GROUP).senders.len(), 1);
    let next = alice.group_encrypt(GROUP, "after restart").unwrap();
    let mut restart = msg(GROUP, 31, 1, next, false);
    assert_eq!(bob.apply_incoming(&mut restart), None);
    assert_eq!(restart.text, "after restart");

    // Alice's store never contains the plaintext.
    let store = std::fs::read(dir.join("e2e/store.bin")).unwrap();
    assert!(!store.windows(7).any(|w| w == b"payload"));
}

#[test]
fn sweep_notices_members_who_left_without_an_event() {
    const GROUP: i64 = -1002223334445;
    let alice = device("sweep", 1);
    alice.identity_bundle().unwrap();
    alice.group_enable(GROUP).unwrap();

    // First observation only records, it cannot know what came before.
    let first = alice.group_note_members(GROUP, &[1, 2, 3], true);
    assert!(first.removed.is_empty() && first.added.is_empty());
    assert!(!first.rotation_needed);
    assert_eq!(alice.group_state(GROUP).members, 3);

    // Bob is gone in the second observation: the key must rotate.
    let second = alice.group_note_members(GROUP, &[1, 3], true);
    assert_eq!(second.removed, vec![2]);
    assert!(second.rotation_needed);
    assert!(alice.group_state(GROUP).rotation_needed);
    assert!(alice.group_encrypt(GROUP, "x").unwrap_err().contains("rotate"));

    // After rotating, a new member is recorded without forcing another rotation.
    let rotated = alice.group_rotate(GROUP).unwrap();
    assert!(!rotated.rotation_needed);
    let third = alice.group_note_members(GROUP, &[1, 3, 4], true);
    assert_eq!(third.added, vec![4]);
    assert!(!third.rotation_needed);
    assert!(alice.group_encrypt(GROUP, "ok").is_ok());
}

#[test]
fn a_member_can_ask_for_the_group_key() {
    const GROUP: i64 = -1007778889990;
    let alice = device("ask-a", 1);
    let bob = device("ask-b", 2);
    let carol = device("ask-c", 3);
    let bob_key = bob.identity_bundle().unwrap();
    let alice_key = alice.identity_bundle().unwrap();
    carol.identity_bundle().unwrap();
    alice.import_peer(2, &bob_key).unwrap();
    alice.set_chat(2, true).unwrap();
    bob.import_peer(1, &alice_key).unwrap();
    bob.set_chat(1, true).unwrap();
    alice.group_enable(GROUP).unwrap();
    // Alice knows the member list, which is what lets her answer key requests.
    alice.group_note_members(GROUP, &[1, 2, 3], true);

    // Bob asks Alice, inside their pairwise session.
    let request = bob.request_group_key(GROUP).unwrap();
    assert!(request.starts_with("mg-e2e-groupkeyreq:v1:"));
    let armored = bob.encrypt_for_peer(1, &request).unwrap();
    let mut incoming = msg(2, 80, 2, armored, false);
    assert_eq!(alice.apply_incoming(&mut incoming), None);
    assert_eq!(incoming.text, "🔑 Asked for a group encryption key");

    // Alice answers from the queue, and Bob installs the key.
    let replies = alice.take_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].peer_id, 2);
    let reply = alice.encrypt_for_peer(2, &replies[0].text).unwrap();
    let mut key = msg(1, 81, 1, reply, false);
    assert_eq!(bob.apply_incoming(&mut key), None);
    assert_eq!(bob.group_state(GROUP).senders.len(), 1);
    let mut group_message = msg(GROUP, 82, 1, alice.group_encrypt(GROUP, "for bob").unwrap(), false);
    assert_eq!(bob.apply_incoming(&mut group_message), None);
    assert_eq!(group_message.text, "for bob");

    // Carol holds no key, so asking produces no reply.
    let mut carol_request = msg(3, 83, 3, carol.request_group_key(GROUP).unwrap(), false);
    carol.apply_incoming(&mut carol_request);
    assert_eq!(alice.take_replies().len(), 0);
}
