# Follow-ups, with the steps that are left

This is the working list from the hand-over. Items 5 and 7 are done in code;
items 1 and 2 need credentials only you can provide; the rest are designs plus
the places in the tree where they land.

## 1. Publish the first release

There are no GitHub releases yet, so `/download/` buttons point at files that do
not exist. One tag fixes that:

```bash
gh auth login                                # a token with repo + workflow scope
git checkout main && git pull                # or merge claude/site-design-next first
scripts/release.sh ci v0.1.0                 # tags, pushes, waits, downloads
```

The workflow's `release` job runs only for `v*` tags, needs the three desktop
packages, the Android packages and the Linux smoke test, then attaches everything
plus `SHA256SUMS` to the tag. `scripts/release.sh publish v0.1.0` does the same
by hand if you prefer to upload from your machine.

What must be true before the tag: the version in `Cargo.toml` is the version you
want to ship (`MelloGram 0.1.0` today), and CI is green on the branch you tag.

## 2. Signatures for distribution

The workflow reads these repository secrets. All are optional; when absent the
step skips itself and the build stays reproducible but unsigned.

| Secret | What it is | Used for |
| --- | --- | --- |
| `WINDOWS_CERT_PFX_BASE64` | base64 of a `.pfx` code-signing certificate | Authenticode on the `.exe` and `.msi` |
| `WINDOWS_CERT_PASSWORD` | that certificate's password | the same |
| `APPLE_CERTIFICATE` | base64 of a `.p12` Developer ID Application certificate | signing the macOS app during `cargo tauri build` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password | the same |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` | the same |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Apple ID, app-specific password, team id | notarisation and stapling by Tauri |
| `ANDROID_KEYSTORE_BASE64` | base64 of the release `.jks` | APK signing |
| `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` | keystore credentials | APK signing |

Add them under Settings → Secrets and variables → Actions. Without the Android
keystore the job warns and signs with a throwaway debug key, which is why the
current APK installs but Android calls it an unknown developer.

The MSI contains the installer, not only the app: signing the MSI after the fact
signs the container, while the bundled `.exe` inside is covered by the Inno
Setup installer that is signed separately. If you need both files signed
end-to-end, sign `target/release/mellogram.exe` in the build step, before the
bundles are produced.

Remaining work after the secrets exist: verify one release by hand on each OS
(SmartScreen, Gatekeeper `spctl -a -vv`, `apksigner verify --print-certs`) and
record the results in `docs/RELEASING.md`.

## 3. Where a browser overlay secret lives

Not implemented: this is the design that has to be accepted before the web client
may persist overlay state. The demo deliberately keeps everything in memory.

Constraints:

- The secret must never be readable by page script after first use, or any XSS
  on `wc.` reads every message.
- The desktop app and the browser are different devices. Sharing one private key
  between them is a downgrade: compromising the browser would compromise the
  desktop identity.
- Nothing may be stored server-side. The Worker has no database and should not
  gain one for secrets.

Proposal:

1. **Per-device identity.** `WasmIdentity::generate()` once per browser profile.
   The secret is exported to a `CryptoKey` that is generated as
   `extractable: false`, AES-GCM, and used to seal `secretBytes()` in
   IndexedDB (`mellogram.overlay.identity.v1`). Page script can ask the browser
   to decrypt during a session but cannot export the wrapping key.
2. **Session keys in memory only.** Ratchet state is sealed the same way and
   written after each message, with the message id as associated data so a
   rollback is detectable.
3. **Pairing, not copying.** The browser shows a QR/one-time link carrying its
   *public* bundle. The desktop app scans it and starts a normal pairwise
   session, so the browser becomes a peer, not a clone. A lost browser profile
   is then a lost peer, not a lost account.
4. **Locked by default.** The sealed key is only unwrapped after an explicit
   "unlock this device" action; there is no silent restore on page load.
5. **Fail closed.** If IndexedDB or WebCrypto is unavailable (private mode,
   blocked storage), the overlay stays unavailable and says why, rather than
   falling back to `localStorage`.

Where it lands: `services/web-client/src/overlay-store.js` (new), the wasm
binding gains nothing; the UI gains an unlock step. Tests: a Node test with a
fake IndexedDB for the sealing format, plus one Playwright test on the deployed
preview for the happy path and the blocked-storage path.

## 4. Split the envelope layer into a shared crate

Today `mellogram-e2e` owns the wire envelopes (`mg-e2e:v1:`,
`mg-e2e-key:v1:`, `mg-e2e-groupkey:v1:`, `mg-e2e-groupkeyreq:v1:`) and the sender
key state, while `ametherian-truee2e` owns the ratchet. The browser build can
only use the latter, which is why the demo is pairwise only.

Target layout:

```
crates/mellogram-envelopes/     # no Tauri, no telegram, no filesystem
  src/envelope.rs               # parse/serialize every mg-e2e: prefix, versioned
  src/group.rs                  # sender keys, distribution records, key requests
  tests/                        # the group tests that live in mellogram-e2e today
crates/mellogram-e2e/           # keeps Telegram glue + app state, depends on the above
crates/ametherian-truee2e-wasm/ # binds envelopes + group keys as well
```

Steps, in order, each a commit:

1. Move the envelope constants and parsers into the new crate with tests; have
   `mellogram-e2e` re-export them so nothing else changes.
2. Move sender-key state and the distribution/request records, keeping the
   existing group tests in the new crate.
3. Bind `group_share_envelopes`, `group_open`, `group_request` in the wasm crate.
4. Wire the web client's demo panel to a group round trip with three simulated
   members, still in memory.

The safety property to preserve: a request is answered only for a peer in the
remembered member list, and a truncated roster is never compared.

## 5. Groups past 200 members — done

`mellogram-telegram::chat_members_all(chat_id, cap)` follows the participant
pager and reports completeness; the sweep reads up to 5,000 members and only
compares when the read is complete. Sharing and key requests cover 500 members
per pass and return `truncated` so the UI can say the rest come next time.

What is still open is cost, not correctness — see the next item.

## 6. Cheaper group sharing

Sharing is one direct message per member per rotation. At 500 members that is 500
messages every rotation, which Telegram rate-limits and users notice.

Options, cheapest first:

1. **Share on demand only.** Keep the key on the sender side and let members ask
   for it when they actually write (the request path already exists). Rotations
   then cost nothing until someone speaks.
2. **Prioritise reachable peers.** Share to members who already have a pairwise
   session and are online first, and let the rest ask later.
3. **A relay.** Publish the sender key encrypted to a rotating group-wide secret
   in a self-hosted relay service; this moves trust off Telegram but adds a
   server, which contradicts "no new server for secrets" today.

Recommendation: implement (1) and (2) first; they remove the burst without new
infrastructure. Revisit a relay only if groups above a few hundred become real.

## 7. Headless smoke test — done, needs one green run

`smoke-linux` downloads the AppImage artifact, installs WebKitGTK and Xvfb, runs
the app for 40 seconds and treats "timeout killed a still-running process" as
success. It also fails on a `panicked at` log line. It gates the `release` job,
so a package that cannot start is never published.

## 8. Site polish — mostly done

Done: Platforms footer, web-client card, directory-index routing in the local
Worker, revalidating styles/scripts, checksum instructions on `/download/`,
per-platform OS detection (already in `site/src/assets/site.js`).

Left, in value order:

1. **Content-hashed asset names** (`site.css?v=<hash>` is not enough for
   immutable caching; hash the file name in `build.mjs` and rewrite the
   references). Then `/assets/*` can be immutable again.
2. **Changelog page** generated from releases at build time
   (`gh release list --json` in CI, written to `site/src/releases.json`), with
   `/releases/` listing notes per version.
3. **Real screenshots** in the spotlight rows, captured from the app on each
   release so the site cannot show a UI that no longer exists.

## 9. macOS runner latency

The workflow already uses `macos-latest` (macos-14 was retired mid-run). The
queue is GitHub's Apple Silicon pool, not the label. Options: keep it and accept
release latency, pay for larger macOS runners, or build the universal DMG on a
self-hosted Mac. The matrix needs no code change for either.

## 10. Web client beyond the shell

The honest order:

1. **Sign-in.** Telegram's web login is MTProto over WebSocket; doing it
   properly means shipping the protocol and a session store in the browser, or
   pairing with the desktop app as a companion (same QR flow as the overlay).
   The companion route reuses work already planned in item 3 and avoids storing
   a Telegram session in a page.
2. **Chat list and messages**, read-only first, from the companion link.
3. **IndexedDB cache** with an explicit "clear this device" action.
4. **Service worker** for offline shell and queued sends, after the send path
   exists.

Do not start at (4); offline sends without a session are a demo, not a feature.
