//! Telegram backend. Wraps a grammers [`Client`] and converts everything into
//! the backend-neutral types from `mellogram-core`, so the UI and plugins never
//! touch MTProto types directly.
//!
//! You need your own `api_id` / `api_hash` from <https://my.telegram.org>;
//! MelloGram ships none.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use grammers_client::message::InputMessage;
use grammers_client::peer::Peer;
use grammers_client::sender::{SenderPool, UpdatesConfiguration};
use grammers_client::session::storages::SqliteSession;
use grammers_client::session::types::{PeerId, PeerRef};
use grammers_client::session::Session as _;
use grammers_client::tl;
use grammers_client::update::Update;
use grammers_client::{Client, SignInError};
use mellogram_core::{IncomingMessage, OutgoingMessage};
use serde::Serialize;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum TgError {
    #[error("Telegram error: {0}")]
    Rpc(#[from] grammers_client::InvocationError),
    #[error("session error: {0}")]
    Session(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown chat {0}; open it from the chat list first")]
    UnknownChat(i64),
    #[error("{0}")]
    Login(String),
}

pub type Result<T> = std::result::Result<T, TgError>;

fn session_err(e: impl std::fmt::Debug) -> TgError {
    TgError::Session(format!("{e:?}"))
}

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LoginState {
    NeedPhone,
    NeedCode { phone: String },
    NeedPassword { hint: Option<String> },
    Ready { me: Me },
}

#[derive(Debug, Clone, Serialize)]
pub struct Me {
    pub id: i64,
    pub name: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSummary {
    pub id: i64,
    pub title: String,
    pub username: Option<String>,
    pub kind: &'static str,
    pub unread: i32,
    pub pinned: bool,
    pub last_text: Option<String>,
    pub last_date: Option<i64>,
}

/// Events pushed from Telegram to the app.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TgEvent {
    NewMessage(IncomingMessage),
    Edited(IncomingMessage),
    Deleted { chat_id: Option<i64>, ids: Vec<i32> },
    Disconnected { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct SendResult {
    pub message: IncomingMessage,
    pub latency_ms: u64,
}

enum Pending {
    None,
    Code { phone: String, token: grammers_client::client::LoginToken },
    Password(grammers_client::client::PasswordToken),
}

pub struct Telegram {
    client: Client,
    session: Arc<SqliteSession>,
    api_hash: String,
    pending: Mutex<Pending>,
    updates: Mutex<Option<mpsc::Receiver<grammers_client::session::updates::UpdatesLike>>>,
}

impl Telegram {
    /// Opens the session file and starts the network runner. Does not log in.
    pub async fn connect(config: TelegramConfig) -> Result<Arc<Self>> {
        if let Some(dir) = config.session_path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let session = Arc::new(SqliteSession::open(&config.session_path).await.map_err(session_err)?);
        let SenderPool { runner, updates, handle } = SenderPool::new(Arc::clone(&session), config.api_id);
        tokio::spawn(runner.run());
        Ok(Arc::new(Self {
            client: Client::new(handle),
            session,
            api_hash: config.api_hash,
            pending: Mutex::new(Pending::None),
            updates: Mutex::new(Some(updates)),
        }))
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    // ---- login -------------------------------------------------------------

    pub async fn login_state(&self) -> Result<LoginState> {
        if self.client.is_authorized().await? {
            return Ok(LoginState::Ready { me: self.me().await? });
        }
        Ok(match &*self.pending.lock().await {
            Pending::None => LoginState::NeedPhone,
            Pending::Code { phone, .. } => LoginState::NeedCode { phone: phone.clone() },
            Pending::Password(t) => LoginState::NeedPassword { hint: t.hint().map(str::to_owned) },
        })
    }

    pub async fn request_code(&self, phone: &str) -> Result<LoginState> {
        let phone = phone.trim().to_owned();
        let token = self.client.request_login_code(&phone, &self.api_hash).await?;
        *self.pending.lock().await = Pending::Code { phone: phone.clone(), token };
        Ok(LoginState::NeedCode { phone })
    }

    pub async fn submit_code(&self, code: &str) -> Result<LoginState> {
        let mut pending = self.pending.lock().await;
        let Pending::Code { token, .. } = &*pending else {
            return Err(TgError::Login("no login code was requested".into()));
        };
        match self.client.sign_in(token, code.trim()).await {
            Ok(_) => {
                *pending = Pending::None;
                drop(pending);
                Ok(LoginState::Ready { me: self.me().await? })
            }
            Err(SignInError::PasswordRequired(t)) => {
                let hint = t.hint().map(str::to_owned);
                *pending = Pending::Password(t);
                Ok(LoginState::NeedPassword { hint })
            }
            Err(SignInError::InvalidCode) => Err(TgError::Login("that code is not valid".into())),
            Err(SignInError::SignUpRequired) => Err(TgError::Login(
                "this number has no Telegram account; sign up with an official app first".into(),
            )),
            Err(e) => Err(TgError::Login(e.to_string())),
        }
    }

    pub async fn submit_password(&self, password: &str) -> Result<LoginState> {
        let mut pending = self.pending.lock().await;
        let Pending::Password(token) = std::mem::replace(&mut *pending, Pending::None) else {
            return Err(TgError::Login("no password was requested".into()));
        };
        match self.client.check_password(token, password).await {
            Ok(_) => {
                drop(pending);
                Ok(LoginState::Ready { me: self.me().await? })
            }
            Err(SignInError::InvalidPassword(t)) => {
                *pending = Pending::Password(t);
                Err(TgError::Login("wrong password".into()))
            }
            Err(e) => Err(TgError::Login(e.to_string())),
        }
    }

    pub async fn logout(&self) -> Result<()> {
        self.client.sign_out().await?;
        Ok(())
    }

    async fn me(&self) -> Result<Me> {
        let me = self.client.get_me().await?;
        Ok(Me { id: me.id().bare_id_unchecked(), name: me.full_name(), username: me.username().map(str::to_owned) })
    }

    // ---- chats & messages ----------------------------------------------------

    async fn peer_ref(&self, chat_id: i64) -> Result<PeerRef> {
        let id = PeerId::from_bot_api_dialog_id(chat_id).ok_or(TgError::UnknownChat(chat_id))?;
        self.session.peer_ref(id).await.map_err(session_err)?.ok_or(TgError::UnknownChat(chat_id))
    }

    pub async fn dialogs(&self, limit: usize) -> Result<Vec<ChatSummary>> {
        let mut iter = self.client.iter_dialogs();
        let mut out = Vec::new();
        while out.len() < limit {
            let Some(dialog) = iter.next().await? else { break };
            let (unread, pinned) = match &dialog.raw {
                tl::enums::Dialog::Dialog(d) => (d.unread_count, d.pinned),
                _ => (0, false),
            };
            let peer = dialog.peer();
            out.push(ChatSummary {
                id: dialog.peer_id().bot_api_dialog_id_unchecked(),
                title: peer.name().unwrap_or("Deleted account").to_owned(),
                username: peer.username().map(str::to_owned),
                kind: peer_kind(peer),
                unread,
                pinned,
                last_text: dialog.last_message.as_ref().map(|m| preview(m.text(), m.media().is_some())),
                last_date: dialog.last_message.as_ref().map(|m| m.date().as_second()),
            });
        }
        Ok(out)
    }

    /// Newest-first page of history; pass the oldest id you have as `before`.
    pub async fn history(&self, chat_id: i64, limit: usize, before: Option<i32>) -> Result<Vec<IncomingMessage>> {
        let peer = self.peer_ref(chat_id).await?;
        let mut iter = self.client.iter_messages(peer).limit(limit);
        if let Some(id) = before {
            iter = iter.offset_id(id);
        }
        let mut out = Vec::new();
        while let Some(m) = iter.next().await? {
            out.push(convert(&m));
        }
        out.reverse();
        Ok(out)
    }

    pub async fn send(&self, msg: &OutgoingMessage) -> Result<SendResult> {
        let peer = self.peer_ref(msg.chat_id).await?;
        let mut input = InputMessage::new()
            .text(msg.text.clone())
            .silent(msg.silent)
            .reply_to(msg.reply_to)
            .link_preview(msg.link_preview);
        // Telegram sends one file per message here; extra files go as follow-ups.
        let mut files = msg.attachments.iter();
        if let Some(first) = files.next() {
            input = input.file(self.upload(first).await?);
        }
        let started = Instant::now();
        let sent = self.client.send_message(peer, input).await?;
        let latency_ms = started.elapsed().as_millis() as u64;
        for file in files {
            let up = self.upload(file).await?;
            self.client.send_message(peer, InputMessage::new().silent(msg.silent).file(up)).await?;
        }
        Ok(SendResult { message: convert(&sent), latency_ms })
    }

    async fn upload(&self, file: &mellogram_core::OutgoingFile) -> Result<grammers_client::media::Uploaded> {
        let mut f = tokio::fs::File::open(&file.path).await?;
        let size = f.metadata().await?.len() as usize;
        Ok(self.client.upload_stream(&mut f, size, file.file_name.clone()).await?)
    }

    pub async fn send_typing(&self, chat_id: i64) -> Result<()> {
        let peer = self.peer_ref(chat_id).await?;
        self.client
            .action(peer)
            .oneshot(tl::enums::SendMessageAction::SendMessageTypingAction)
            .await?;
        Ok(())
    }

    pub async fn mark_read(&self, chat_id: i64) -> Result<()> {
        let peer = self.peer_ref(chat_id).await?;
        self.client.mark_as_read(peer).await?;
        Ok(())
    }

    /// Marks every dialog with unread messages as read. Returns how many.
    pub async fn mark_all_read(&self) -> Result<usize> {
        let mut iter = self.client.iter_dialogs();
        let mut n = 0;
        while let Some(dialog) = iter.next().await? {
            if matches!(&dialog.raw, tl::enums::Dialog::Dialog(d) if d.unread_count > 0 || d.unread_mark) {
                self.client.mark_as_read(dialog.peer_ref()).await?;
                n += 1;
            }
        }
        Ok(n)
    }

    pub async fn delete(&self, chat_id: i64, ids: &[i32]) -> Result<()> {
        let peer = self.peer_ref(chat_id).await?;
        self.client.delete_messages(peer, ids).await?;
        Ok(())
    }

    pub async fn edit(&self, chat_id: i64, id: i32, text: &str) -> Result<()> {
        let peer = self.peer_ref(chat_id).await?;
        self.client.edit_message(peer, id, InputMessage::new().text(text)).await?;
        Ok(())
    }

    /// Round-trip time of an MTProto ping.
    pub async fn ping(&self) -> Result<std::time::Duration> {
        let started = Instant::now();
        let ping_id = started.elapsed().as_nanos() as i64 ^ 0x4d47_5049_4e47;
        self.client.invoke(&tl::functions::Ping { ping_id }).await?;
        Ok(started.elapsed())
    }

    pub async fn is_authorized(&self) -> bool {
        self.client.is_authorized().await.unwrap_or(false)
    }

    // ---- updates ---------------------------------------------------------------

    /// Starts streaming updates into `tx`. Call once, after login.
    pub async fn start_updates(self: &Arc<Self>, tx: mpsc::UnboundedSender<TgEvent>) -> Result<()> {
        let Some(updates) = self.updates.lock().await.take() else {
            return Ok(()); // already running
        };
        let mut stream = self
            .client
            .stream_updates(updates, UpdatesConfiguration { catch_up: true, ..Default::default() })
            .await
            .map_err(|e| TgError::Session(e.to_string()))?;
        tokio::spawn(async move {
            loop {
                let event = match stream.next().await {
                    Ok(Update::NewMessage(m)) => TgEvent::NewMessage(convert(&m)),
                    Ok(Update::MessageEdited(m)) => TgEvent::Edited(convert(&m)),
                    Ok(Update::MessageDeleted(d)) => TgEvent::Deleted {
                        chat_id: d.channel_id().map(|c| PeerId::channel_unchecked(c).bot_api_dialog_id_unchecked()),
                        ids: d.messages().to_vec(),
                    },
                    Ok(_) => continue,
                    Err(e) => {
                        let _ = tx.send(TgEvent::Disconnected { reason: e.to_string() });
                        break;
                    }
                };
                if tx.send(event).is_err() {
                    break;
                }
            }
            let _ = stream.sync_update_state().await;
        });
        Ok(())
    }
}

fn peer_kind(peer: &Peer) -> &'static str {
    match peer {
        Peer::User(_) => "user",
        Peer::Group(_) => "group",
        Peer::Channel(_) => "channel",
        Peer::Community(_) => "community",
    }
}

fn preview(text: &str, has_media: bool) -> String {
    let t: String = text.chars().take(80).collect();
    match (t.is_empty(), has_media) {
        (true, true) => "📎 Media".into(),
        _ => t.replace('\n', " "),
    }
}

/// grammers message → backend-neutral message.
pub fn convert(m: &grammers_client::message::Message) -> IncomingMessage {
    let sender = m.sender();
    IncomingMessage {
        chat_id: m.peer_id().bot_api_dialog_id_unchecked(),
        id: m.id(),
        sender_id: m.sender_id().map(|s| s.bot_api_dialog_id_unchecked()),
        sender_name: sender.and_then(|s| s.name()).map(str::to_owned),
        sender_username: sender.and_then(|s| s.username()).map(str::to_owned),
        display_name: None,
        text: m.text().to_owned(),
        date: m.date().as_second(),
        edit_date: m.edit_date().map(|d| d.as_second()),
        outgoing: m.outgoing(),
        silent: m.silent(),
        reply_to: m.reply_to_message_id(),
        has_media: m.media().is_some(),
        badges: Vec::new(),
        edit_history: Vec::new(),
        deleted: false,
        classes: Vec::new(),
        highlight: None,
    }
}
