# End-to-end encrypted chats (privacy overlay)

MelloGram can add an extra layer of end-to-end encryption on top of ordinary
Telegram chats when **both** people use MelloGram. Messages are encrypted on
your device and only decrypted on your contact's device. Telegram's servers
only ever store and relay scrambled text.

Technical details: [`crates/ametherian-truee2e/PROTOCOL.md`](../crates/ametherian-truee2e/PROTOCOL.md).

## What it protects

- **Message content**, end to end between MelloGram users. Telegram, anyone
  with access to Telegram's servers, and anyone else in between cannot read it.
- **Past and future messages after a key leak.** Keys change with every
  exchange of messages (forward secrecy and post-compromise security), so one
  stolen key does not unlock the whole history.
- **Message integrity.** A tampered message, or one copied into a different
  chat, is rejected rather than shown.
- **Message length (roughly).** Messages are padded to a few fixed sizes by
  default.

## What it does NOT protect

- **Metadata.** Telegram still sees who talks to whom, when, how often, and
  roughly how large each message is. Your profile, contacts and group
  membership are unchanged.
- **People not using MelloGram.** If your contact uses another Telegram app,
  they only see an unreadable block of text starting with `mg-e2e:v1:`. They
  cannot read it.
- **Your devices.** Anyone who can unlock your phone or computer, malware on
  either device, screenshots, screen recordings, copy-paste, or the other
  person forwarding what you said are all outside what encryption can stop.
- **Media and calls.** The overlay covers text messages.

## Verifying your contact

When you first turn on encryption with someone, your apps exchange keys in the
chat. To make sure nobody swapped those keys in transit:

1. Open the chat's encryption info. You both see the same **safety number**:
   60 digits in 12 groups of five, and a QR code.
2. Compare them over a channel you trust: in person, scan each other's QR
   code, or read the digits aloud on a call.
3. If they match, mark the contact as verified.

If a contact's safety number ever changes, MelloGram warns you. That can
happen because they reinstalled the app or changed devices, but it can also
mean someone is intercepting the chat. Verify again before sending anything
sensitive.
