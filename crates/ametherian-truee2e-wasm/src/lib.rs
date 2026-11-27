//! WebAssembly bindings for the MelloGram overlay protocol — a spike.
//!
//! Purpose: prove the protocol crate runs in a browser (wasm32-unknown-unknown,
//! Web Crypto randomness) and give the web client a working primitive it can
//! build on. The surface is deliberately small:
//!
//! * [`WasmIdentity`] — key generation, export/restore, public bundle, safety
//!   numbers.
//! * [`WasmSession`] — start a session from a peer bundle, encrypt, decrypt,
//!   export/restore state.
//! * [`accept`] — the responder half of the handshake (session + reply), which
//!   the caller sends back to the initiator.
//!
//! What is **not** here yet: persistence (the web client must seal `secretBytes`
//! and session state itself), the group sender keys, and the app's envelope
//! format (`mg-e2e-groupkey:` and friends). Those are app-layer concerns the
//! desktop crate already owns; binding them is a later step, not a protocol
//! change.
//!
//! Build:
//!
//! ```text
//! rustup target add wasm32-unknown-unknown
//! cargo build -p ametherian-truee2e-wasm --target wasm32-unknown-unknown
//! # then, for the JS glue: wasm-bindgen target/web/pkg \
//! #   target/wasm32-unknown-unknown/debug/ametherian_truee2e_wasm.wasm
//! ```
//!
//! Native `cargo test -p ametherian-truee2e-wasm` runs the same code paths, so
//! the bindings are testable without a browser.

use ametherian_truee2e::{
    decrypt, encrypt, respond, IdentityKeyPair, PublicBundle, SafetyNumber, Session,
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsError;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// A device identity. The secret bytes never leave the caller's control; keep
/// them in browser storage that is encrypted, never in `localStorage`.
#[wasm_bindgen]
pub struct WasmIdentity {
    inner: IdentityKeyPair,
}

#[wasm_bindgen]
impl WasmIdentity {
    /// Fresh identity from Web Crypto randomness.
    #[wasm_bindgen(js_name = generate)]
    pub fn generate() -> Result<WasmIdentity, JsError> {
        IdentityKeyPair::generate()
            .map(|inner| WasmIdentity { inner })
            .map_err(js_err)
    }

    /// Restore an identity from [`WasmIdentity::secretBytes`].
    #[wasm_bindgen(js_name = fromSecret)]
    pub fn from_secret(bytes: &[u8]) -> Result<WasmIdentity, JsError> {
        IdentityKeyPair::from_bytes(bytes)
            .map(|inner| WasmIdentity { inner })
            .map_err(js_err)
    }

    /// Secret key material. Encrypt it at rest before storing it.
    #[wasm_bindgen(js_name = secretBytes)]
    pub fn secret_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes().to_vec()
    }

    /// Armored public bundle, safe to send to a contact.
    pub fn bundle(&self) -> String {
        self.inner.public_bundle().to_armored()
    }

    /// 60-digit safety number against a peer's armored bundle, grouped in fives.
    #[wasm_bindgen(js_name = safetyNumber)]
    pub fn safety_number(&self, peer_bundle: &str) -> Result<String, JsError> {
        let peer = PublicBundle::from_armored(peer_bundle).map_err(js_err)?;
        let number = SafetyNumber::compute(&self.inner.identity(), &peer.identity());
        Ok(number.groups().join(" "))
    }
}

/// One side of a pairwise conversation. State is serializable so the caller can
/// persist it (encrypted); losing it means the ratchet cannot be resumed.
#[wasm_bindgen]
pub struct WasmSession {
    inner: Session,
}

#[wasm_bindgen]
impl WasmSession {
    /// Start a session as the initiator. The first ciphertext carries the
    /// handshake, so it can be sent as an ordinary message.
    pub fn start(
        identity: &WasmIdentity,
        peer_bundle: &str,
        chat_id: i64,
    ) -> Result<WasmSession, JsError> {
        let peer = PublicBundle::from_armored(peer_bundle).map_err(js_err)?;
        Session::initiate(&identity.inner, &peer)
            .map(|inner| WasmSession { inner })
            .map_err(js_err)
        .map(|session| {
            let _ = chat_id;
            session
        })
    }

    /// Restore a session from [`WasmSession::state`].
    #[wasm_bindgen(js_name = fromState)]
    pub fn from_state(state: &[u8]) -> Result<WasmSession, JsError> {
        Session::from_bytes(state)
            .map(|inner| WasmSession { inner })
            .map_err(js_err)
    }

    /// Serialized ratchet state. Encrypt it at rest.
    pub fn state(&self) -> Result<Vec<u8>, JsError> {
        self.inner.to_bytes().map(|b| b.to_vec()).map_err(js_err)
    }

    /// Seals text and returns the armored message to send.
    pub fn encrypt(&mut self, text: &str, chat_id: i64) -> Result<String, JsError> {
        encrypt(&mut self.inner, text.as_bytes(), chat_id).map_err(js_err)
    }

    /// Opens an armored message from the peer.
    pub fn decrypt(&mut self, armored: &str, chat_id: i64) -> Result<String, JsError> {
        let plain = decrypt(&mut self.inner, armored, chat_id).map_err(js_err)?;
        String::from_utf8(plain).map_err(js_err)
    }

    /// The safety number for this conversation.
    #[wasm_bindgen(js_name = safetyNumber)]
    pub fn safety_number(&self) -> String {
        self.inner.safety_number().groups().join(" ")
    }

    /// True while the initiator waits for the responder's first reply.
    #[wasm_bindgen(js_name = awaitingReply)]
    pub fn awaiting_reply(&self) -> bool {
        self.inner.awaiting_reply()
    }
}

/// Responder half of the handshake. Returns JSON:
/// `{"state":"<base64>","plaintext":"<the message that arrived>"}`.
/// Store the state; reply with [`WasmSession::encrypt`] on that session.
#[wasm_bindgen]
pub fn accept(identity: &WasmIdentity, initial: &str, chat_id: i64) -> Result<String, JsError> {
    let (session, plaintext) = respond(&identity.inner, initial, chat_id).map_err(js_err)?;
    let state = session.to_bytes().map_err(js_err)?;
    Ok(json!({
        "state": B64.encode(state.as_slice()),
        "plaintext": String::from_utf8(plaintext).map_err(js_err)?,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_restore_and_agree_on_a_safety_number() {
        let alice = WasmIdentity::generate().unwrap();
        let bob = WasmIdentity::generate().unwrap();
        let restored = WasmIdentity::from_secret(&alice.secret_bytes()).unwrap();
        assert_eq!(
            alice.safety_number(&bob.bundle()).unwrap(),
            restored.safety_number(&bob.bundle()).unwrap()
        );
        assert_eq!(
            alice.safety_number(&bob.bundle()).unwrap(),
            bob.safety_number(&alice.bundle()).unwrap()
        );
        // Error paths construct JsError, which only exists in a browser.
    }

    #[test]
    fn a_pairwise_conversation_works_through_the_bindings() {
        let alice = WasmIdentity::generate().unwrap();
        let bob = WasmIdentity::generate().unwrap();
        let chat = 424242;

        let mut out = WasmSession::start(&alice, &bob.bundle(), chat).unwrap();
        assert!(out.awaiting_reply());
        let hello = out.encrypt("hello from wasm", chat).unwrap();
        assert!(hello.starts_with("mg-e2e:v1:"));

        let accepted: serde_json::Value = serde_json::from_str(&accept(&bob, &hello, chat).unwrap()).unwrap();
        assert_eq!(accepted["plaintext"], "hello from wasm");
        let state = B64.decode(accepted["state"].as_str().unwrap()).unwrap();
        let mut inbox = WasmSession::from_state(&state).unwrap();
        out.decrypt(&inbox.encrypt("hi back", chat).unwrap(), chat).unwrap();
        assert!(!out.awaiting_reply());
        let answer = inbox.decrypt(&out.encrypt("and back", chat).unwrap(), chat).unwrap();
        assert_eq!(answer, "and back");
        assert_eq!(out.safety_number(), inbox.safety_number());
    }

    #[test]
    fn session_state_survives_a_restart() {
        let alice = WasmIdentity::generate().unwrap();
        let bob = WasmIdentity::generate().unwrap();
        let chat = 7;
        let mut session = WasmSession::start(&alice, &bob.bundle(), chat).unwrap();
        let first = session.encrypt("before restart", chat).unwrap();
        let saved = session.state().unwrap();
        let _accepted = accept(&bob, &first, chat).unwrap();
        let mut resumed = WasmSession::from_state(&saved).unwrap();
        // The saved state is the pre-restart ratchet: it can still produce the
        // next message and the peer can open it.
        let next = resumed.encrypt("after restart", chat).unwrap();
        assert!(next.starts_with("mg-e2e:v1:"));
    }
}
