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

## Turning it on in MelloGram

1. **Settings → End-to-End Overlay → Create my key**, then **Copy my key** and
   send it to the other person through a channel you trust.
2. Open the one-to-one chat and click the **lock button** in the composer,
   paste their key and confirm. Your next message starts the encrypted
   session; on their device it is decrypted automatically, and their client
   pins your key from that first message.
3. Compare the 60-digit safety number (lock button) over a call or in person
   and press **Check digits**. Verified contacts show a closed lock; everyone
   else shows an open one.

The lock button shows the state at a glance: grey = off, amber = on but
waiting for the other person's key, green = on with an open session.

## What MelloGram stores

- Your identity, the ratchet sessions and a bounded cache of decrypted
  messages live in `e2e/store.bin` in the config directory, sealed with
  XChaCha20-Poly1305 under a per-install device key (`e2e/device.key`). Both
  files are created owner-only. The device key sits beside the store, so this
  keeps overlay state out of backups, sync folders and bug reports — it is not
  protection against someone who already has your account.
- The decrypted cache exists so history renders again after a restart without
  rewinding the ratchet. **Forget decrypted messages** clears it.
- Deleting `e2e/` means a new identity: contacts must verify the new safety
  number. MelloGram warns when a stored contact key changes and never
  silently replaces a pinned one; the new key has to be imported explicitly.

## Limits in this build

- One-to-one text only. Group and channel overlays are separate work.
- Encrypted messages are limited to about 2600 bytes of UTF-8, because the
  armored text has to fit Telegram's 4096-character message limit.
- Attachments and edits are refused while the overlay is on: files are not
  covered yet, and an edit would no longer match the ciphertext stored by
  Telegram.
- Messages sent before the overlay was enabled stay in plain text.
