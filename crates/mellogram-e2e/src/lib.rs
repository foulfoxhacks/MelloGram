//! The in-app end-to-end privacy overlay.
//!
//! Every MelloGram install gets one long-term identity. A one-to-one chat can
//! then be switched to the overlay: the message text is encrypted with
//! ametherian-truee2e (X3DH-style setup + Double Ratchet over
//! XChaCha20-Poly1305) and sent as an ordinary `mg-e2e:v1:` text message, so
//! Telegram's servers only ever see ciphertext.
//!
//! State (identity secrets, ratchet sessions and a small plaintext vault so
//! history can be shown again after a restart) lives in `e2e/store.bin`,
//! sealed with a per-install device key (`e2e/device.key`). Both files are
//! created owner-only under the same app-private directory as the Telegram
//! session.
//!
//! Threat model and limitations: `docs/E2E.md`.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, MutexGuard};

use ametherian_truee2e::{
    decrypt, encrypt_with, is_armored, is_initial_message, open_at_rest, private_chat_conversation_id,
    respond, seal_at_rest, DeviceKey, EncryptOptions, Error as OverlayError, IdentityKeyPair,
    IdentityPublic, PublicBundle, SafetyNumber, Session,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use mellogram_core::{IncomingMessage, MessageBadge};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};



/// Version of the on-disk store.
const STORE_VERSION: u16 = 1;
/// Associated data for the sealed store.
const STORE_CONTEXT: &[u8] = b"mellogram/e2e/store/v1";

/// Largest plaintext we accept for one encrypted message. The armored text has
/// to stay under Telegram's 4096-character message limit, which leaves room for
/// about this many UTF-8 bytes.
const MAX_ENCRYPTED_PLAINTEXT: usize = 2600;
/// Above the largest padding bucket we send unpadded text (still over the
/// 4-byte length prefix) rather than blowing past Telegram's size limit.
const PADDED_MAX: usize = 1020;
/// How many decrypted messages we keep so history renders again after a
/// restart without touching the ratchet.
const PLAINTEXT_VAULT_MAX: usize = 5000;

#[derive(Clone, Default)]
struct Peer {
    bundle: Option<PublicBundle>,
    session: Option<Session>,
    verified: bool,
}

struct E2eStore {
    dir: PathBuf,
    key: Option<DeviceKey>,
    identity: Option<IdentityKeyPair>,
    peers: BTreeMap<i64, Peer>,
    /// chat id -> overlay requested
    chats: BTreeMap<i64, bool>,
    /// (chat id, message id) -> plaintext
    plaintext: BTreeMap<(i64, i32), String>,
}

#[derive(Serialize, Deserialize, Default)]
struct Persisted {
    version: u16,
    #[serde(default)]
    identity: Option<String>,
    #[serde(default)]
    peers: BTreeMap<i64, PersistedPeer>,
    #[serde(default)]
    chats: BTreeMap<i64, bool>,
    /// "chat:message" -> plaintext.
    #[serde(default)]
    plaintext: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedPeer {
    #[serde(default)]
    bundle: Option<String>,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    verified: bool,
}

// ---- views handed to the UI -------------------------------------------------

/// What the settings page shows.
#[derive(Debug, Clone, Serialize, Default)]
pub struct E2eStatus {
    pub has_identity: bool,
    pub fingerprint: Option<String>,
    pub key: Option<String>,
    pub peers: Vec<PeerView>,
    pub decrypted_messages: usize,
}

/// One contact's overlay state.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PeerView {
    pub peer_id: i64,
    pub verified: bool,
    pub has_session: bool,
    pub fingerprint: Option<String>,
    pub safety_number: Option<String>,
}

/// Overlay state for the chat that is open.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ChatView {
    pub chat_id: i64,
    pub enabled: bool,
    pub ready: bool,
    pub peer_key_known: bool,
    pub verified: bool,
    pub safety_number: Option<String>,
}

// ---- store ------------------------------------------------------------------

fn io_error(what: &str, e: std::io::Error) -> String {
    format!("{what}: {e}")
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) {}

/// Writes through a temporary file and renames, so a crash never leaves a
/// half-written store.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| io_error("could not write overlay state", e))?;
    restrict(&tmp, 0o600);
    std::fs::rename(&tmp, path).map_err(|e| io_error("could not replace overlay state", e))?;
    Ok(())
}

fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| "decrypted message is not valid UTF-8".to_string())
}

fn fingerprint(identity: &IdentityPublic) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"mellogram-e2e-fingerprint-v1");
    hasher.update(identity.to_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .take(8)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.join(""))
        .collect::<Vec<_>>()
        .join("-")
}

impl E2eStore {
    fn open(dir: &Path) -> E2eStore {
        let dir = dir.join("e2e");
        let mut store = E2eStore {
            dir,
            key: None,
            identity: None,
            peers: BTreeMap::new(),
            chats: BTreeMap::new(),
            plaintext: BTreeMap::new(),
        };
        if let Err(e) = store.load() {
            tracing::warn!("encrypted overlay state could not be loaded: {e}");
        }
        store
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join("device.key")
    }

    fn store_path(&self) -> PathBuf {
        self.dir.join("store.bin")
    }

    fn load(&mut self) -> Result<(), String> {
        let key_bytes = match std::fs::read(self.key_path()) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(io_error("could not read the overlay device key", e)),
        };
        let key = DeviceKey::from_bytes(&key_bytes)
            .map_err(|_| "the overlay device key file is corrupt".to_string())?;
        let sealed = match std::fs::read(self.store_path()) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                self.key = Some(key);
                return Ok(());
            }
            Err(e) => return Err(io_error("could not read the overlay state", e)),
        };
        let plain = open_at_rest(&key, &sealed, STORE_CONTEXT)
            .map_err(|_| "the overlay state could not be decrypted on this device".to_string())?;
        let persisted: Persisted = serde_json::from_slice(plain.as_slice())
            .map_err(|_| "the overlay state is corrupt".to_string())?;
        if persisted.version != STORE_VERSION {
            return Err("the overlay state was written by a newer MelloGram".into());
        }
        self.identity = match persisted.identity {
            Some(text) => match STANDARD.decode(text) {
                Ok(bytes) => IdentityKeyPair::from_bytes(&bytes).ok(),
                Err(_) => None,
            },
            None => None,
        };
        for (peer_id, record) in persisted.peers {
            let bundle = record
                .bundle
                .and_then(|text| PublicBundle::from_armored(&text).ok());
            let session = record.session.and_then(|text| {
                STANDARD
                    .decode(text)
                    .ok()
                    .and_then(|bytes| Session::from_bytes(&bytes).ok())
            });
            // A session is useless without the bundle that pins the identity.
            let session = if bundle.is_some() { session } else { None };
            self.peers.insert(
                peer_id,
                Peer {
                    bundle,
                    session,
                    verified: record.verified,
                },
            );
        }
        self.chats = persisted.chats;
        for (key, text) in persisted.plaintext {
            if let Some((chat, id)) = key.split_once(':') {
                if let (Ok(chat), Ok(id)) = (chat.parse::<i64>(), id.parse::<i32>()) {
                    self.plaintext.insert((chat, id), text);
                }
            }
        }
        self.key = Some(key);
        Ok(())
    }

    fn ensure_key(&mut self) -> Result<(), String> {
        if self.key.is_some() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| io_error("could not create the overlay directory", e))?;
        restrict(&self.dir, 0o700);
        let key = DeviceKey::generate().map_err(|_| "could not generate an overlay key".to_string())?;
        write_atomic(&self.key_path(), key.to_bytes().as_slice())?;
        self.key = Some(key);
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let Some(key) = self.key.as_ref() else {
            return Err("the overlay device key is missing".into());
        };
        let mut peers = BTreeMap::new();
        for (id, peer) in &self.peers {
            peers.insert(
                *id,
                PersistedPeer {
                    bundle: peer.bundle.as_ref().map(PublicBundle::to_armored),
                    session: peer.session.as_ref().and_then(|s| {
                        s.to_bytes().ok().map(|bytes| STANDARD.encode(bytes.as_slice()))
                    }),
                    verified: peer.verified,
                },
            );
        }
        let mut plaintext = BTreeMap::new();
        for ((chat, id), text) in &self.plaintext {
            plaintext.insert(format!("{chat}:{id}"), text.clone());
        }
        let persisted = Persisted {
            version: STORE_VERSION,
            identity: self
                .identity
                .as_ref()
                .map(|i| STANDARD.encode(i.to_bytes().as_slice())),
            peers,
            chats: self.chats.clone(),
            plaintext,
        };
        let json = serde_json::to_vec(&persisted)
            .map_err(|e| format!("could not encode the overlay state: {e}"))?;
        let sealed = seal_at_rest(key, &json, STORE_CONTEXT)
            .map_err(|_| "could not encrypt the overlay state".to_string())?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| io_error("could not create the overlay directory", e))?;
        restrict(&self.dir, 0o700);
        write_atomic(&self.store_path(), &sealed)
    }

    fn persist(&mut self) -> Result<(), String> {
        self.ensure_key()?;
        self.save()
    }

    fn local_identity(&self) -> Option<IdentityPublic> {
        self.identity.as_ref().map(IdentityKeyPair::identity)
    }

    fn safety(&self, peer_id: i64) -> Option<String> {
        let local = self.local_identity()?;
        let peer = self.peers.get(&peer_id)?.bundle.as_ref()?;
        Some(SafetyNumber::compute(&local, &peer.identity()).display())
    }

    fn peer_view(&self, peer_id: i64) -> PeerView {
        let peer = self.peers.get(&peer_id);
        PeerView {
            peer_id,
            verified: peer.is_some_and(|p| p.verified),
            has_session: peer.is_some_and(|p| p.session.is_some()),
            fingerprint: peer
                .and_then(|p| p.bundle.as_ref())
                .map(|b| fingerprint(&b.identity())),
            safety_number: self.safety(peer_id),
        }
    }

    fn chat_view(&self, chat_id: i64) -> ChatView {
        let peer = self.peers.get(&chat_id);
        ChatView {
            chat_id,
            enabled: self.chats.get(&chat_id).copied().unwrap_or(false),
            ready: peer.is_some_and(|p| p.session.is_some()),
            peer_key_known: peer.is_some_and(|p| p.bundle.is_some()),
            verified: peer.is_some_and(|p| p.verified),
            safety_number: self.safety(chat_id),
        }
    }

    fn remember(&mut self, chat_id: i64, message_id: i32, text: &str) {
        self.plaintext.insert((chat_id, message_id), text.to_string());
        while self.plaintext.len() > PLAINTEXT_VAULT_MAX {
            let Some(oldest) = self.plaintext.keys().next().copied() else {
                break;
            };
            self.plaintext.remove(&oldest);
        }
    }
}

// ---- shared state -----------------------------------------------------------

/// Overlay state owned by [`AppState`].
pub struct E2eState {
    store: Mutex<E2eStore>,
    /// Our own Telegram user id (0 until we learn it).
    self_id: AtomicI64,
}

impl E2eState {
    /// Loads the overlay from the app config directory. Never fails: a broken
    /// store is logged and the overlay starts empty rather than blocking the
    /// app from opening.
    pub fn open(dir: &Path) -> Self {
        E2eState {
            store: Mutex::new(E2eStore::open(dir)),
            self_id: AtomicI64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, E2eStore> {
        self.store.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Called once the Telegram login tells us who we are.
    pub fn set_self_id(&self, id: i64) {
        if id > 0 {
            self.self_id.store(id, Ordering::Relaxed);
        }
    }

    /// Our own user id, if known.
    pub fn self_id(&self) -> i64 {
        self.self_id.load(Ordering::Relaxed)
    }

    /// Whether the open chat asked for the overlay.
    pub fn chat_enabled(&self, chat_id: i64) -> bool {
        self.lock().chats.get(&chat_id).copied().unwrap_or(false)
    }

    /// Writes pending state to disk (called on shutdown).
    pub fn flush(&self) {
        let mut store = self.lock();
        if store.key.is_some() {
            if let Err(e) = store.persist() {
                tracing::warn!("could not save the encrypted overlay: {e}");
            }
        }
    }

    /// Identity summary for the settings page.
    pub fn status(&self) -> E2eStatus {
        let store = self.lock();
        let identity = store.local_identity();
        E2eStatus {
            has_identity: identity.is_some(),
            fingerprint: identity.as_ref().map(fingerprint),
            key: store
                .identity
                .as_ref()
                .map(|i| i.public_bundle().to_armored()),
            peers: store.peers.keys().map(|id| store.peer_view(*id)).collect(),
            decrypted_messages: store.plaintext.len(),
        }
    }

    /// Our public bundle (creating the identity on first use).
    pub fn identity_bundle(&self) -> Result<String, String> {
        let mut store = self.lock();
        if store.identity.is_none() {
            let identity = IdentityKeyPair::generate()
                .map_err(|_| "could not generate your overlay key".to_string())?;
            store.identity = Some(identity);
            store.persist()?;
        }
        store
            .identity
            .as_ref()
            .map(|i| i.public_bundle().to_armored())
            .ok_or_else(|| "your overlay key is missing".to_string())
    }

    /// Throws away the identity, every session and the decrypted vault.
    pub fn reset_identity(&self) -> Result<(), String> {
        let mut store = self.lock();
        store.identity = None;
        for peer in store.peers.values_mut() {
            peer.session = None;
            peer.verified = false;
        }
        store.chats.clear();
        store.plaintext.clear();
        store.persist()
    }

    /// Remembers a contact's public key, pinning their identity.
    pub fn import_peer(&self, peer_id: i64, key: &str) -> Result<PeerView, String> {
        if peer_id <= 0 {
            return Err("Overlay keys belong to people: open a one-to-one chat first.".into());
        }
        let bundle = PublicBundle::from_armored(key.trim())
            .map_err(|e| format!("that is not a valid MelloGram overlay key ({e})"))?;
        let mut store = self.lock();
        if store
            .local_identity()
            .is_some_and(|local| local == bundle.identity())
        {
            return Err("That is your own key — ask the other person for theirs.".into());
        }
        let peer = store.peers.entry(peer_id).or_default();
        if peer
            .bundle
            .as_ref()
            .is_some_and(|existing| existing.identity() != bundle.identity())
        {
            // Their device changed (or someone swapped the key): start clean
            // and require the user to compare safety numbers again.
            peer.session = None;
            peer.verified = false;
        }
        peer.bundle = Some(bundle);
        store.persist()?;
        Ok(store.peer_view(peer_id))
    }

    /// Drops a contact's key and session.
    pub fn forget_peer(&self, peer_id: i64) -> Result<(), String> {
        let mut store = self.lock();
        store.peers.remove(&peer_id);
        store.chats.remove(&peer_id);
        store.persist()
    }

    /// Per-chat overlay state.
    pub fn chat_state(&self, chat_id: i64) -> ChatView {
        self.lock().chat_view(chat_id)
    }

    /// Turns the overlay on or off for a chat, starting the session on first
    /// use. Requires our identity and the contact's key.
    pub fn set_chat(&self, chat_id: i64, enabled: bool) -> Result<ChatView, String> {
        if chat_id <= 0 {
            return Err(
                "The encrypted overlay works in one-to-one chats. Group and channel support arrives separately.".into(),
            );
        }
        let mut store = self.lock();
        if enabled {
            if store.identity.is_none() {
                return Err("Create your overlay key first (Settings → End-to-End Overlay).".into());
            }
            let bundle = store
                .peers
                .get(&chat_id)
                .and_then(|p| p.bundle.clone())
                .ok_or_else(|| "Add the other person's overlay key first.".to_string())?;
            let needs_session = store
                .peers
                .get(&chat_id)
                .is_some_and(|p| p.session.is_none());
            if needs_session {
                let session = {
                    let identity = store
                        .identity
                        .as_ref()
                        .ok_or_else(|| "your overlay key is missing".to_string())?;
                    Session::initiate(identity, &bundle)
                        .map_err(|_| "could not start an encrypted session".to_string())?
                };
                if let Some(peer) = store.peers.get_mut(&chat_id) {
                    peer.session = Some(session);
                }
            }
        }
        store.chats.insert(chat_id, enabled);
        store.persist()?;
        Ok(store.chat_view(chat_id))
    }

    /// Whether the typed digits match the contact's safety number; marks the
    /// contact verified when they do.
    pub fn verify_peer(&self, peer_id: i64, digits: &str) -> Result<bool, String> {
        let mut store = self.lock();
        let local = store
            .local_identity()
            .ok_or_else(|| "your overlay key is missing".to_string())?;
        let bundle = store
            .peers
            .get(&peer_id)
            .and_then(|p| p.bundle.as_ref())
            .ok_or_else(|| "this contact has no overlay key yet".to_string())?;
        let number = SafetyNumber::compute(&local, &bundle.identity());
        if !number.matches_digits(digits) {
            return Ok(false);
        }
        if let Some(peer) = store.peers.get_mut(&peer_id) {
            peer.verified = true;
        }
        store.persist()?;
        Ok(true)
    }

    /// Encrypts `text` for the chat. Returns the armored text to send.
    pub fn encrypt_for_chat(&self, chat_id: i64, text: &str) -> Result<String, String> {
        let me = self.self_id();
        if me <= 0 {
            return Err("Sign in to Telegram before using the encrypted overlay.".into());
        }
        if chat_id <= 0 {
            return Err("The encrypted overlay works in one-to-one chats only.".into());
        }
        let bytes = text.as_bytes();
        if bytes.len() > MAX_ENCRYPTED_PLAINTEXT {
            return Err(format!(
                "Encrypted messages are limited to {MAX_ENCRYPTED_PLAINTEXT} bytes of text."
            ));
        }
        let mut store = self.lock();
        if !store.chats.get(&chat_id).copied().unwrap_or(false) {
            return Err("Encryption is off for this chat.".into());
        }
        let conversation = private_chat_conversation_id(me, chat_id);
        let options = EncryptOptions {
            pad: bytes.len() <= PADDED_MAX,
        };
        let (armored, backup) = {
            let peer = store
                .peers
                .get_mut(&chat_id)
                .ok_or_else(|| "the other person's key is missing".to_string())?;
            let session = peer
                .session
                .as_mut()
                .ok_or_else(|| "no encrypted session with this person yet".to_string())?;
            let backup = session.clone();
            match encrypt_with(session, bytes, conversation, options) {
                Ok(armored) => (armored, backup),
                Err(e) => return Err(e.to_string()),
            }
        };
        if let Err(e) = store.persist() {
            // Never send a message whose ratchet state we could not store: the
            // next message would be undecryptable.
            if let Some(peer) = store.peers.get_mut(&chat_id) {
                peer.session = Some(backup);
            }
            return Err(e);
        }
        Ok(armored)
    }

    /// Remembers a message we sent so the echo and history show the text.
    pub fn remember_sent(&self, chat_id: i64, message_id: i32, text: &str) {
        let mut store = self.lock();
        store.remember(chat_id, message_id, text);
        if let Err(e) = store.persist() {
            tracing::warn!("could not save the decrypted message: {e}");
        }
    }

    fn cached_plaintext(&self, chat_id: i64, message_id: i32) -> Option<String> {
        self.lock().plaintext.get(&(chat_id, message_id)).cloned()
    }

    /// Decrypts an incoming armored message, accepting a new session from an
    /// initial message (trust on first use, with the identity pinned).
    fn decrypt_incoming(
        &self,
        chat_id: i64,
        peer_id: i64,
        message_id: i32,
        armored: &str,
    ) -> Result<(String, bool), String> {
        let me = self.self_id();
        if me <= 0 {
            return Err("sign in to Telegram to read encrypted messages".into());
        }
        if peer_id <= 0 {
            return Err("encrypted group messages are not supported yet".into());
        }
        let conversation = private_chat_conversation_id(me, peer_id);
        let mut store = self.lock();

        if let Some(peer) = store.peers.get_mut(&peer_id) {
            if let Some(session) = peer.session.as_mut() {
                let backup = session.clone();
                match decrypt(session, armored, conversation) {
                    Ok(plain) => {
                        let text = utf8(plain)?;
                        let verified = peer.verified;
                        store.remember(chat_id, message_id, &text);
                        if let Err(e) = store.persist() {
                            if let Some(peer) = store.peers.get_mut(&peer_id) {
                                peer.session = Some(backup);
                            }
                            return Err(e);
                        }
                        return Ok((text, verified));
                    }
                    Err(OverlayError::NewSessionOffered) if is_initial_message(armored) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
        }

        if !is_initial_message(armored) {
            return Err("no encrypted session with this person yet".into());
        }
        let (session, plain) = {
            let identity = store
                .identity
                .as_ref()
                .ok_or_else(|| "this message was encrypted to an older key of yours".to_string())?;
            respond(identity, armored, conversation).map_err(|e| e.to_string())?
        };
        let offered = session.peer_bundle().clone();
        let entry = store.peers.entry(peer_id).or_default();
        if entry
            .bundle
            .as_ref()
            .is_some_and(|existing| existing.identity() != offered.identity())
        {
            return Err("their identity key changed — verify the safety number first".into());
        }
        entry.bundle = Some(offered);
        entry.session = Some(session);
        let verified = entry.verified;
        let text = utf8(plain)?;
        store.remember(chat_id, message_id, &text);
        if let Err(e) = store.persist() {
            tracing::warn!("could not save the new encrypted session: {e}");
        }
        Ok((text, verified))
    }

    /// Rewrites an armored message in place when we can read it. Returns the
    /// reason when we could not, for logging; the ciphertext stays visible with
    /// a lock badge either way.
    pub fn apply_incoming(&self, message: &mut IncomingMessage) -> Option<String> {
        if !is_armored(&message.text) {
            return None;
        }
        message.add_class("mg-e2e");
        let own = message.outgoing;
        if let Some(text) = self.cached_plaintext(message.chat_id, message.id) {
            message.text = text;
            overlay_badge(message, true, own);
            return None;
        }
        if own {
            overlay_badge(message, true, true);
            return None;
        }
        let peer_id = if message.chat_id > 0 {
            message.chat_id
        } else {
            message.sender_id.unwrap_or(0)
        };
        match self.decrypt_incoming(message.chat_id, peer_id, message.id, &message.text) {
            Ok((text, verified)) => {
                message.text = text;
                overlay_badge(message, verified, false);
                None
            }
            Err(reason) => {
                message.add_class("mg-e2e-locked");
                overlay_badge(message, false, false);
                Some(reason)
            }
        }
    }

    /// Forgets the decrypted-message vault. Returns how many messages were
    /// dropped.
    pub fn clear_history(&self) -> Result<usize, String> {
        let mut store = self.lock();
        let dropped = store.plaintext.len();
        store.plaintext.clear();
        store.persist()?;
        Ok(dropped)
    }
}

/// True when `text` is an armored overlay message (`mg-e2e:v1:`). The chat
/// list uses it to avoid showing ciphertext as the last message.
pub fn is_overlay_text(text: &str) -> bool {
    is_armored(text)
}

fn overlay_badge(message: &mut IncomingMessage, trusted: bool, own: bool) {
    let label = if trusted { "🔒" } else { "🔓" };
    let tooltip = if own {
        "Sent with the MelloGram end-to-end overlay"
    } else if trusted {
        "End-to-end encrypted · verified contact"
    } else {
        "End-to-end encrypted · safety number not verified yet"
    };
    message.badges.push(MessageBadge {
        plugin: "e2e".into(),
        label: label.into(),
        tooltip: Some(tooltip.into()),
    });
}

