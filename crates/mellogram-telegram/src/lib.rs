//! Telegram backend. Wraps a grammers [`Client`] and converts everything into
//! the backend-neutral types from `mellogram-core`, so the UI and plugins never
//! touch MTProto types directly.
//!
//! You need your own `api_id` / `api_hash` from <https://my.telegram.org>;
//! MelloGram ships none.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use grammers_client::message::InputMessage;
use grammers_client::peer::Peer;
use grammers_client::sender::{SenderPool, SenderPoolHandle, UpdatesConfiguration};
use grammers_client::session::storages::SqliteSession;
use grammers_client::session::types::{PeerId, PeerInfo, PeerKind, PeerRef, UpdateState, UpdatesState};
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

/// The phone number Telegram accepts: E.164, digits only after the `+`.
///
/// The UI already cleans numbers up, but the command can also be called from
/// plugins and older builds. An invalid number must fail before
/// `auth.sendCode`, otherwise it burns the per-number flood budget without
/// ever sending a code.
fn normalize_phone(raw: &str) -> Option<String> {
    let mut number: String = raw
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n' | '(' | ')' | '-' | '.'))
        .collect();
    if let Some(rest) = number.strip_prefix("00") {
        number = format!("+{rest}");
    }
    if !number.starts_with('+') {
        number.insert(0, '+');
    }
    let digits = number.strip_prefix('+')?;
    if !(7..=15).contains(&digits.len())
        || digits.starts_with('0')
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some(number)
}

fn code_type_name(kind: &tl::enums::auth::CodeType) -> &'static str {
    match kind {
        tl::enums::auth::CodeType::Sms => "sms",
        tl::enums::auth::CodeType::Call => "call",
        tl::enums::auth::CodeType::FlashCall => "flash-call",
        tl::enums::auth::CodeType::MissedCall => "missed-call",
        tl::enums::auth::CodeType::FragmentSms => "fragment",
    }
}

/// Turn Telegram's delivery choice into something the UI can explain.
fn delivery_from(sent: &tl::types::auth::SentCode) -> CodeDelivery {
    use tl::enums::auth::SentCodeType as T;
    let mut delivery = CodeDelivery {
        via: "unknown",
        length: None,
        email: None,
        url: None,
        pattern: None,
        prefix: None,
        next: sent.next_type.as_ref().map(code_type_name),
        timeout: sent.timeout,
    };
    match &sent.r#type {
        T::App(x) => { delivery.via = "app"; delivery.length = Some(x.length); }
        T::Sms(x) => { delivery.via = "sms"; delivery.length = Some(x.length); }
        T::Call(x) => { delivery.via = "call"; delivery.length = Some(x.length); }
        T::FlashCall(x) => { delivery.via = "flash-call"; delivery.pattern = Some(x.pattern.clone()); }
        T::MissedCall(x) => {
            delivery.via = "missed-call";
            delivery.prefix = Some(x.prefix.clone());
            delivery.length = Some(x.length);
        }
        T::EmailCode(x) => {
            delivery.via = "email";
            delivery.email = Some(x.email_pattern.clone());
            delivery.length = Some(x.length);
        }
        T::SetUpEmailRequired(_) => delivery.via = "email-setup",
        T::FragmentSms(x) => {
            delivery.via = "fragment";
            delivery.url = Some(x.url.clone());
            delivery.length = Some(x.length);
        }
        T::FirebaseSms(x) => { delivery.via = "firebase"; delivery.length = Some(x.length); }
        T::SmsWord(x) => { delivery.via = "sms"; delivery.pattern = x.beginning.clone(); }
        T::SmsPhrase(x) => { delivery.via = "sms"; delivery.pattern = x.beginning.clone(); }
    }
    delivery
}

/// Translate the RPC errors people actually hit into a next step.
fn login_rpc(error: grammers_client::InvocationError) -> TgError {
    let grammers_client::InvocationError::Rpc(rpc) = &error else {
        return TgError::Rpc(error);
    };
    let message = match rpc.name.as_str() {
        "PHONE_NUMBER_INVALID" => "Telegram says that phone number is not valid. Use international format, for example +1 555 123 4567.".to_owned(),
        "PHONE_NUMBER_BANNED" => "Telegram has banned that phone number.".to_owned(),
        "PHONE_NUMBER_APP_SIGNUP_FORBIDDEN" => "Telegram does not let this app sign up new accounts. Sign up in an official Telegram app first.".to_owned(),
        "PHONE_NUMBER_FLOOD" | "PHONE_PASSWORD_FLOOD" | "FLOOD_WAIT" | "FLOOD_PREMIUM_WAIT" => {
            let wait = rpc.value.map(|v| format!(" Telegram asks you to wait {v} seconds.")).unwrap_or_default();
            format!("Telegram is rate-limiting code requests for that number.{wait} Repeated attempts make the wait longer.")
        }
        "PHONE_PASSWORD_PROTECTED" => "That phone number is password-protected in Telegram.".to_owned(),
        "API_ID_INVALID" => "Telegram rejected the API ID. Check the API ID and hash from my.telegram.org.".to_owned(),
        "API_ID_PUBLISHED_FLOOD" => "Telegram blocked this API ID because it was published. Create your own API ID at my.telegram.org.".to_owned(),
        "AUTH_RESTART" => "Telegram asked to restart the login. Try again.".to_owned(),
        "SMS_CODE_CREATE_FAILED" => "Telegram could not create an SMS code for that number. Try again, or use another number.".to_owned(),
        "UPDATE_APP_TO_LOGIN" => "Telegram requires a newer client for this login. Update MelloGram and try again.".to_owned(),
        "PHONE_CODE_INVALID" | "PHONE_CODE_EMPTY" | "PHONE_CODE_EXPIRED" => "That login code is not valid or has expired.".to_owned(),
        "PASSWORD_HASH_INVALID" => "Wrong two-step verification password.".to_owned(),
        _ => return TgError::Rpc(error),
    };
    TgError::Login(message)
}

fn payment_error(required: &tl::types::auth::SentCodePaymentRequired) -> TgError {
    TgError::Login(format!(
        "Telegram requires a paid login for this number ({} {}). Sign in with an official Telegram app first.",
        required.amount, required.currency
    ))
}

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: PathBuf,
    /// MTProto obfuscated transport (anti-fingerprinting).
    pub obfuscated: bool,
    /// Optional SOCKS5 proxy URL (already validated).
    pub proxy_url: Option<String>,
    /// Shown in Telegram's active-sessions list, e.g. "0.1.0".
    pub app_version: String,
}

/// How this device introduces itself to Telegram: MelloGram, the real OS,
/// no hardware identifiers.
fn connection_params(config: &TelegramConfig) -> grammers_client::sender::ConnectionParams {
    let os = match std::env::consts::OS { "windows" => "Windows", "macos" => "macOS", "linux" => "Linux", "android" => "Android", "ios" => "iOS", other => other };
    let obfuscated = config.obfuscated;
    grammers_client::sender::ConnectionParams {
        device_model: format!("MelloGram on {os}"),
        system_version: format!("{os} {}", std::env::consts::ARCH),
        app_version: format!("{} MelloGram", config.app_version),
        system_lang_code: "en".into(),
        lang_code: "en".into(),
        proxy_url: config.proxy_url.clone(),
        transport: Box::new(move || -> Box<dyn grammers_mtproto::transport::Transport + Send> {
            if obfuscated {
                Box::new(grammers_mtproto::transport::Obfuscated::new(grammers_mtproto::transport::Intermediate::new()))
            } else {
                Box::new(grammers_mtproto::transport::Full::new())
            }
        }),
        ..Default::default()
    }
}

/// How Telegram delivered a login code.
///
/// Telegram chooses the channel, and for third-party apps it can refuse SMS
/// entirely: the code may only arrive as a service message in another Telegram
/// session, by email, or through Fragment. The UI needs this to tell the user
/// where to look instead of assuming every code is an SMS.
#[derive(Debug, Clone, Serialize)]
pub struct CodeDelivery {
    /// `app`, `sms`, `call`, `flash-call`, `missed-call`, `email`,
    /// `fragment`, `firebase`, `email-setup`, or `unknown`.
    pub via: &'static str,
    /// Number of digits in the code, when Telegram says.
    pub length: Option<i32>,
    /// Masked address for an email code.
    pub email: Option<String>,
    /// Fragment URL for a Fragment code.
    pub url: Option<String>,
    /// Flash-call pattern.
    pub pattern: Option<String>,
    /// Missed-call prefix.
    pub prefix: Option<String>,
    /// Method `auth.resendCode` will use next, when Telegram says.
    pub next: Option<&'static str>,
    /// Seconds Telegram suggests waiting before resending.
    pub timeout: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LoginState {
    NeedPhone,
    NeedCode { phone: String, delivery: CodeDelivery },
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
    /// For one-to-one chats: the other person's online status.
    pub presence: Option<Presence>,
    pub verified: bool,
    pub bot: bool,
    /// Current profile photo id, as a string (it does not fit a JS number).
    /// `None` when the chat has no photo; changes when the photo changes, so
    /// it doubles as a cache key.
    pub photo: Option<String>,
}

/// A user's online status as precisely as their privacy settings allow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Presence {
    /// Online now; `until` is when Telegram will consider them offline.
    Online { until: i64 },
    /// Exact last-seen time (unix seconds).
    LastSeen { at: i64 },
    /// Hidden by privacy settings; only an approximation is available.
    Recently,
    WithinWeek,
    WithinMonth,
    /// Long ago, or hidden completely.
    LongAgo,
    Bot,
}

pub fn presence_of(status: &tl::enums::UserStatus) -> Presence {
    match status {
        tl::enums::UserStatus::Online(s) => Presence::Online { until: i64::from(s.expires) },
        tl::enums::UserStatus::Offline(s) => Presence::LastSeen { at: i64::from(s.was_online) },
        tl::enums::UserStatus::Recently(_) => Presence::Recently,
        tl::enums::UserStatus::LastWeek(_) => Presence::WithinWeek,
        tl::enums::UserStatus::LastMonth(_) => Presence::WithinMonth,
        tl::enums::UserStatus::Empty => Presence::LongAgo,
    }
}

fn user_presence(u: &grammers_client::peer::User) -> Presence {
    if u.is_bot() { Presence::Bot } else { presence_of(u.status()) }
}

/// Full profile of a user, group or channel for the info panel.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ChatInfo {
    pub id: i64,
    pub kind: &'static str,
    pub title: String,
    pub username: Option<String>,
    pub about: Option<String>,
    pub presence: Option<Presence>,
    pub phone: Option<String>,
    pub bot: bool,
    pub verified: bool,
    pub scam: bool,
    pub members: Option<i32>,
    pub online: Option<i32>,
    pub admins: Option<i32>,
    pub banned: Option<i32>,
    pub kicked: Option<i32>,
    pub linked_chat_id: Option<i64>,
    pub slow_mode_seconds: Option<i32>,
    pub pinned_message_id: Option<i32>,
    pub auto_delete_seconds: Option<i32>,
    pub invite_link: Option<String>,
    pub common_chats: Option<i32>,
    pub can_view_members: bool,
    pub members_hidden: bool,
    pub blocked: bool,
    pub birthday: Option<String>,
    /// Current profile photo id (string: it does not fit a JS number).
    pub photo: Option<String>,
    pub premium: bool,
    /// In your contacts / they have you in theirs too.
    pub contact: bool,
    pub mutual_contact: bool,
    /// Active collectible / extra usernames besides the main one.
    pub extra_usernames: Vec<String>,
    /// Channel shown on the user's profile (Bot-API id).
    pub personal_channel_id: Option<i64>,
    pub business_location: Option<String>,
    pub business_intro: Option<String>,
    pub business_timezone: Option<String>,
    pub business_open_now: Option<bool>,
    /// Bot description ("What can this bot do?").
    pub bot_description: Option<String>,
    pub gifts: Option<i32>,
    pub language: Option<String>,
    pub calls_available: bool,
    /// Location-based groups.
    pub location: Option<String>,
    pub join_requests: Option<i32>,
    pub boosts: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub username: Option<String>,
    pub role: &'static str,
    pub presence: Presence,
}

/// Events pushed from Telegram to the app.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TgEvent {
    NewMessage(IncomingMessage),
    Edited(IncomingMessage),
    Deleted { chat_id: Option<i64>, ids: Vec<i32> },
    /// A ban, removal, restriction, join or admin change.
    Membership(mellogram_core::relationships::MembershipEvent),
    /// A channel/supergroup's state changed (often: you were banned or removed).
    ChatChanged { chat_id: i64 },
    /// A user's online status changed.
    Presence { user_id: i64, presence: Presence },
    Disconnected { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct SendResult {
    pub message: IncomingMessage,
    pub latency_ms: u64,
}

enum Pending {
    None,
    Code { phone: String, hash: String, delivery: CodeDelivery },
    Password(grammers_client::client::PasswordToken),
}

impl Pending {
    fn matches_code(&self, expected_phone: &str, expected_hash: &str) -> bool {
        matches!(self, Self::Code { phone, hash, .. } if phone == expected_phone && hash == expected_hash)
    }
}

pub struct Telegram {
    client: Client,
    sender: SenderPoolHandle,
    session: Arc<SqliteSession>,
    api_id: i32,
    api_hash: String,
    pending: Mutex<Pending>,
    updates: Mutex<Option<mpsc::Receiver<grammers_client::session::updates::UpdatesLike>>>,
    /// Your own user id once known (0 until then).
    self_id: std::sync::atomic::AtomicI64,
}

impl Telegram {
    /// Opens the session file and starts the network runner. Does not log in.
    pub async fn connect(config: TelegramConfig) -> Result<Arc<Self>> {
        if let Some(dir) = config.session_path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let session = Arc::new(SqliteSession::open(&config.session_path).await.map_err(session_err)?);
        let SenderPool { runner, updates, handle } = SenderPool::with_configuration(Arc::clone(&session), config.api_id, connection_params(&config));
        tokio::spawn(runner.run());
        Ok(Arc::new(Self {
            client: Client::new(handle.clone()),
            sender: handle.thin,
            session,
            api_id: config.api_id,
            api_hash: config.api_hash,
            pending: Mutex::new(Pending::None),
            updates: Mutex::new(Some(updates)),
            self_id: std::sync::atomic::AtomicI64::new(0),
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
            Pending::Code { phone, delivery, .. } => LoginState::NeedCode {
                phone: phone.clone(),
                delivery: delivery.clone(),
            },
            Pending::Password(t) => LoginState::NeedPassword { hint: t.hint().map(str::to_owned) },
        })
    }

    /// Ask Telegram for a login code and remember how it chose to deliver it.
    ///
    /// `Client::request_login_code` throws that information away (and panics on
    /// `auth.sentCodeSuccess`), so the raw method is used here. Knowing the
    /// channel matters: a third-party app can be refused SMS and receive the
    /// code only as a service message in another Telegram session, by email, or
    /// through Fragment. Without it the UI says "we sent a code" and nothing
    /// arrives.
    pub async fn request_code(&self, phone: &str) -> Result<LoginState> {
        let phone = normalize_phone(phone).ok_or_else(|| {
            TgError::Login(
                "That is not a complete international phone number. Include the country code, for example +1 555 123 4567."
                    .into(),
            )
        })?;
        match self.send_code(&phone).await? {
            tl::enums::auth::SentCode::Code(sent) => {
                let delivery = delivery_from(&sent);
                if delivery.via == "email-setup" {
                    return Err(TgError::Login(
                        "Telegram will not send a code to this device until you add a login email. Add one in an official Telegram app, then try again."
                            .into(),
                    ));
                }
                *self.pending.lock().await = Pending::Code {
                    phone: phone.clone(),
                    hash: sent.phone_code_hash,
                    delivery: delivery.clone(),
                };
                Ok(LoginState::NeedCode { phone, delivery })
            }
            tl::enums::auth::SentCode::Success(done) => self.finish_sent_code(done.authorization).await,
            tl::enums::auth::SentCode::PaymentRequired(required) => Err(payment_error(&required)),
        }
    }

    /// Resend the code through the method Telegram nominated as `next_type`.
    pub async fn resend_code(&self) -> Result<LoginState> {
        let (phone, hash) = {
            let pending = self.pending.lock().await;
            let Pending::Code { phone, hash, .. } = &*pending else {
                return Err(TgError::Login("no login code was requested".into()));
            };
            (phone.clone(), hash.clone())
        };
        let sent = self
            .client
            .invoke(&tl::functions::auth::ResendCode {
                phone_number: phone.clone(),
                phone_code_hash: hash.clone(),
                reason: None,
            })
            .await
            .map_err(login_rpc)?;
        // A delayed resend must not restore a code after the user chose a
        // different number or after a newer resend replaced this request.
        let mut pending = self.pending.lock().await;
        if !pending.matches_code(&phone, &hash) {
            return Err(TgError::Login("This code request is no longer active.".into()));
        }
        match sent {
            tl::enums::auth::SentCode::Code(sent) => {
                let delivery = delivery_from(&sent);
                *pending = Pending::Code {
                    phone: phone.clone(),
                    hash: sent.phone_code_hash,
                    delivery: delivery.clone(),
                };
                Ok(LoginState::NeedCode { phone, delivery })
            }
            tl::enums::auth::SentCode::Success(done) => {
                drop(pending);
                self.finish_sent_code(done.authorization).await
            }
            tl::enums::auth::SentCode::PaymentRequired(required) => Err(payment_error(&required)),
        }
    }

    /// Forget the pending code and tell Telegram to invalidate it.
    pub async fn cancel_code(&self) -> Result<()> {
        let pending = std::mem::replace(&mut *self.pending.lock().await, Pending::None);
        if let Pending::Code { phone, hash, .. } = pending {
            // Best effort: the local state is already reset, so a network
            // failure here must not trap the user on the code screen.
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                self.client.invoke(&tl::functions::auth::CancelCode {
                    phone_number: phone,
                    phone_code_hash: hash,
                }),
            ).await;
        }
        Ok(())
    }

    pub async fn submit_code(&self, code: &str) -> Result<LoginState> {
        let mut pending = self.pending.lock().await;
        let Pending::Code { phone, hash, .. } = &*pending else {
            return Err(TgError::Login("no login code was requested".into()));
        };
        let (phone, hash) = (phone.clone(), hash.clone());
        match self
            .client
            .invoke(&tl::functions::auth::SignIn {
                phone_number: phone,
                phone_code_hash: hash,
                phone_code: Some(code.trim().to_owned()),
                email_verification: None,
            })
            .await
        {
            Ok(tl::enums::auth::Authorization::Authorization(auth)) => {
                *pending = Pending::None;
                drop(pending);
                Ok(LoginState::Ready { me: self.finish_login(auth).await? })
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => Err(TgError::Login(
                "this number has no Telegram account; sign up with an official app first".into(),
            )),
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => {
                let password: tl::types::account::Password = self
                    .client
                    .invoke(&tl::functions::account::GetPassword {})
                    .await
                    .map_err(login_rpc)?
                    .into();
                let token = grammers_client::client::PasswordToken::new(password);
                let hint = token.hint().map(str::to_owned);
                *pending = Pending::Password(token);
                Ok(LoginState::NeedPassword { hint })
            }
            Err(e) if e.is("PHONE_CODE_*") => {
                Err(TgError::Login("that code is not valid or has expired".into()))
            }
            Err(e) => Err(login_rpc(e)),
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

    /// `auth.sendCode`, including the datacenter redirect Telegram can answer
    /// with (303 PHONE_MIGRATE_X).
    async fn send_code(&self, phone: &str) -> Result<tl::enums::auth::SentCode> {
        let request = || tl::functions::auth::SendCode {
            phone_number: phone.to_owned(),
            api_id: self.api_id,
            api_hash: self.api_hash.clone(),
            settings: tl::types::CodeSettings {
                allow_flashcall: false,
                current_number: false,
                allow_app_hash: false,
                allow_missed_call: false,
                allow_firebase: false,
                unknown_number: false,
                logout_tokens: None,
                token: None,
                app_sandbox: None,
            }
            .into(),
        };
        match self.client.invoke(&request()).await {
            Ok(sent) => Ok(sent),
            Err(grammers_client::InvocationError::Rpc(rpc)) if rpc.code == 303 => {
                let dc = rpc.value.ok_or_else(|| {
                    TgError::Login(
                        "Telegram asked to reconnect to another data centre but did not say which one."
                            .into(),
                    )
                })? as i32;
                let old_dc = self.session.home_dc_id().map_err(session_err)?;
                self.sender.disconnect_from_dc(old_dc);
                self.session.set_home_dc_id(dc).await.map_err(session_err)?;
                self.client.invoke(&request()).await.map_err(login_rpc)
            }
            Err(e) => Err(login_rpc(e)),
        }
    }

    async fn finish_sent_code(&self, authorization: tl::enums::auth::Authorization) -> Result<LoginState> {
        match authorization {
            tl::enums::auth::Authorization::Authorization(auth) => {
                *self.pending.lock().await = Pending::None;
                Ok(LoginState::Ready { me: self.finish_login(auth).await? })
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => Err(TgError::Login(
                "this number has no Telegram account; sign up with an official app first".into(),
            )),
        }
    }

    /// What `Client::sign_in` does on success. grammers keeps it private, so a
    /// raw `auth.signIn` has to cache the self peer and the update state here;
    /// otherwise the account is authorized server-side but the session cannot
    /// resolve itself or process updates.
    async fn finish_login(&self, auth: tl::types::auth::Authorization) -> Result<Me> {
        let update_state = self.client.invoke(&tl::functions::updates::GetState {}).await.ok();
        let user = grammers_client::peer::User::from_raw(&self.client, auth.user);
        let id = user.id().bare_id_unchecked();
        let peer_auth = user
            .to_ref()
            .await
            .map_err(|e| TgError::Login(format!("could not finish the Telegram login: {e}")))?
            .ok_or_else(|| TgError::Login("Telegram did not return a usable account".into()))?
            .auth;
        self.session
            .cache_peer(PeerInfo::User {
                id,
                auth: Some(peer_auth),
                bot: Some(user.is_bot()),
                is_self: Some(true),
            })
            .await
            .map_err(session_err)?;
        if let Some(tl::enums::updates::State::State(state)) = update_state {
            self.session
                .set_update_state(UpdateState::All(UpdatesState {
                    pts: state.pts,
                    qts: state.qts,
                    date: state.date,
                    seq: state.seq,
                    channels: Vec::new(),
                }))
                .await
                .map_err(session_err)?;
        }
        self.self_id.store(id, std::sync::atomic::Ordering::Relaxed);
        Ok(Me { id, name: user.full_name(), username: user.username().map(str::to_owned) })
    }

    pub async fn logout(&self) -> Result<()> {
        self.client.sign_out().await?;
        Ok(())
    }

    async fn me(&self) -> Result<Me> {
        let me = self.client.get_me().await?;
        self.self_id.store(me.id().bare_id_unchecked(), std::sync::atomic::Ordering::Relaxed);
        Ok(Me { id: me.id().bare_id_unchecked(), name: me.full_name(), username: me.username().map(str::to_owned) })
    }

    /// Our own user id, fetched once and cached. `0` only if we never logged in.
    pub async fn ensure_self_id(&self) -> Result<i64> {
        let id = self.self_id.load(std::sync::atomic::Ordering::Relaxed);
        if id != 0 {
            return Ok(id);
        }
        Ok(self.me().await?.id)
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
                presence: match peer { Peer::User(u) => Some(user_presence(u)), _ => None },
                verified: match peer { Peer::User(u) => u.verified(), _ => false },
                bot: matches!(peer, Peer::User(u) if u.is_bot()),
                photo: peer_photo_id(peer).map(|id| id.to_string()),
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
        if let Some(at) = msg.schedule_at {
            input = input.schedule_date(Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(u64::try_from(at).unwrap_or(0))));
        }
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
            let mut extra = InputMessage::new().silent(msg.silent).file(up);
            if let Some(at) = msg.schedule_at {
                extra = extra.schedule_date(Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(u64::try_from(at).unwrap_or(0))));
            }
            self.client.send_message(peer, extra).await?;
        }
        Ok(SendResult { message: convert(&sent), latency_ms })
    }

    async fn upload(&self, file: &mellogram_core::OutgoingFile) -> Result<grammers_client::media::Uploaded> {
        let mut f = tokio::fs::File::open(&file.path).await?;
        let size = f.metadata().await?.len() as usize;
        Ok(self.client.upload_stream(&mut f, size, file.file_name.clone()).await?)
    }

    /// Sends a location pin.
    pub async fn send_location(&self, chat_id: i64, lat: f64, long: f64, silent: bool) -> Result<IncomingMessage> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&long) || !lat.is_finite() || !long.is_finite() {
            return Err(TgError::Login("coordinates are out of range".into()));
        }
        let peer = self.peer_ref(chat_id).await?;
        let media = tl::types::InputMediaGeoPoint {
            geo_point: tl::enums::InputGeoPoint::Point(tl::types::InputGeoPoint { lat, long, accuracy_radius: None }),
        };
        let sent = self.client.send_message(peer, InputMessage::new().silent(silent).media(media)).await?;
        Ok(convert(&sent))
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

    /// Full information for the info panel.
    pub async fn chat_info(&self, chat_id: i64) -> Result<ChatInfo> {
        let peer = self.peer_ref(chat_id).await?;
        let invite = |e: &Option<tl::enums::ExportedChatInvite>| match e {
            Some(tl::enums::ExportedChatInvite::ChatInviteExported(i)) => Some(i.link.clone()),
            _ => None,
        };
        match peer.id.kind() {
            PeerKind::User => {
                let tl::enums::users::UserFull::Full(full) = self.client.invoke(&tl::functions::users::GetFullUser { id: (&peer).into() }).await?;
                let tl::enums::UserFull::Full(f) = full.full_user;
                let user = full.users.into_iter().find_map(|u| match u {
                    tl::enums::User::User(u) if u.id == f.id => Some(u),
                    _ => None,
                });
                let mut info = ChatInfo { id: chat_id, kind: "user", about: f.about.filter(|a| !a.is_empty()), common_chats: Some(f.common_chats_count), blocked: f.blocked, pinned_message_id: f.pinned_msg_id, auto_delete_seconds: f.ttl_period, can_view_members: false, ..Default::default() };
                if let Some(tl::enums::Birthday::Birthday(b)) = f.birthday {
                    info.birthday = Some(match b.year { Some(y) => format!("{y:04}-{:02}-{:02}", b.month, b.day), None => format!("{:02}-{:02}", b.month, b.day) });
                }
                info.personal_channel_id = f.personal_channel_id.map(|id| PeerId::channel_unchecked(id).bot_api_dialog_id_unchecked());
                if let Some(tl::enums::BusinessLocation::Location(l)) = &f.business_location {
                    info.business_location = Some(l.address.clone()).filter(|a| !a.is_empty());
                }
                if let Some(tl::enums::BusinessIntro::Intro(i)) = &f.business_intro {
                    let text = [i.title.as_str(), i.description.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" — ");
                    info.business_intro = Some(text).filter(|t| !t.is_empty());
                }
                if let Some(tl::enums::BusinessWorkHours::Hours(h)) = &f.business_work_hours {
                    info.business_timezone = Some(h.timezone_id.clone());
                    info.business_open_now = Some(h.open_now);
                }
                if let Some(tl::enums::BotInfo::Info(b)) = &f.bot_info {
                    info.bot_description = b.description.clone().filter(|d| !d.is_empty());
                }
                info.gifts = f.stargifts_count.filter(|n| *n > 0);
                info.calls_available = f.phone_calls_available;
                if let Some(u) = user {
                    info.premium = u.premium;
                    info.contact = u.contact;
                    info.mutual_contact = u.mutual_contact;
                    info.language = u.lang_code.clone();
                    info.photo = match &u.photo {
                        Some(tl::enums::UserProfilePhoto::Photo(p)) => Some(p.photo_id.to_string()),
                        _ => None,
                    };
                    info.extra_usernames = u.usernames.iter().flatten().filter_map(|n| match n {
                        tl::enums::Username::Username(n) if n.active && Some(&n.username) != u.username.as_ref() => Some(n.username.clone()),
                        _ => None,
                    }).collect();
                    let first = u.first_name.clone().unwrap_or_default();
                    let last = u.last_name.clone().unwrap_or_default();
                    info.title = format!("{first} {last}").trim().to_owned();
                    info.username = u.username.clone();
                    info.phone = u.phone.clone();
                    info.bot = u.bot;
                    info.verified = u.verified;
                    info.scam = u.scam || u.fake;
                    info.presence = Some(if u.bot { Presence::Bot } else { u.status.as_ref().map(presence_of).unwrap_or(Presence::LongAgo) });
                }
                Ok(info)
            }
            PeerKind::Chat => {
                let tl::enums::messages::ChatFull::Full(full) = self.client.invoke(&tl::functions::messages::GetFullChat { chat_id: peer.id.bare_id_unchecked() }).await?;
                let tl::enums::ChatFull::Full(f) = full.full_chat else { return Err(TgError::Login("unexpected chat type".into())) };
                let members = match &f.participants {
                    tl::enums::ChatParticipants::Participants(p) => Some(p.participants.len() as i32),
                    _ => None,
                };
                let title = full.chats.iter().find_map(|c| match c { tl::enums::Chat::Chat(c) if c.id == f.id => Some(c.title.clone()), _ => None }).unwrap_or_default();
                let photo = match &f.chat_photo { Some(tl::enums::Photo::Photo(p)) => Some(p.id.to_string()), _ => None };
                Ok(ChatInfo { id: chat_id, kind: "group", title, photo, about: Some(f.about).filter(|a| !a.is_empty()), members, pinned_message_id: f.pinned_msg_id, auto_delete_seconds: f.ttl_period, invite_link: invite(&f.exported_invite), can_view_members: true, ..Default::default() })
            }
            PeerKind::Channel => {
                let tl::enums::messages::ChatFull::Full(full) = self.client.invoke(&tl::functions::channels::GetFullChannel { channel: (&peer).into() }).await?;
                let tl::enums::ChatFull::ChannelFull(f) = full.full_chat else { return Err(TgError::Login("unexpected chat type".into())) };
                let ch = full.chats.iter().find_map(|c| match c { tl::enums::Chat::Channel(c) if c.id == f.id => Some(c.clone()), _ => None });
                let mut info = ChatInfo {
                    id: chat_id,
                    kind: if ch.as_ref().is_some_and(|c| c.broadcast) { "channel" } else { "supergroup" },
                    about: Some(f.about).filter(|a| !a.is_empty()),
                    members: f.participants_count,
                    online: f.online_count,
                    admins: f.admins_count,
                    banned: f.banned_count,
                    kicked: f.kicked_count,
                    linked_chat_id: f.linked_chat_id.map(|id| PeerId::channel_unchecked(id).bot_api_dialog_id_unchecked()),
                    slow_mode_seconds: f.slowmode_seconds,
                    pinned_message_id: f.pinned_msg_id,
                    auto_delete_seconds: f.ttl_period,
                    invite_link: invite(&f.exported_invite),
                    can_view_members: f.can_view_participants,
                    members_hidden: f.participants_hidden,
                    location: match &f.location { Some(tl::enums::ChannelLocation::Location(l)) => Some(l.address.clone()), _ => None },
                    join_requests: f.requests_pending.filter(|n| *n > 0),
                    boosts: f.boosts_applied.filter(|n| *n > 0),
                    gifts: f.stargifts_count.filter(|n| *n > 0),
                    photo: match &f.chat_photo { tl::enums::Photo::Photo(p) => Some(p.id.to_string()), _ => None },
                    ..Default::default()
                };
                if let Some(c) = ch {
                    info.title = c.title.clone();
                    info.username = c.username.clone();
                    info.verified = c.verified;
                    info.scam = c.scam || c.fake;
                }
                Ok(info)
            }
        }
    }

    /// First `limit` members of a group or channel (where you are allowed to see them).
    pub async fn chat_members(&self, chat_id: i64, limit: usize) -> Result<Vec<Member>> {
        use grammers_client::peer::Role;
        let peer = self.peer_ref(chat_id).await?;
        let mut iter = self.client.iter_participants(peer);
        let mut out = Vec::new();
        while out.len() < limit.min(500) {
            let Some(p) = iter.next().await? else { break };
            out.push(Member {
                id: p.user.id().bare_id_unchecked(),
                name: p.user.full_name(),
                username: p.user.username().map(str::to_owned),
                role: match p.role { Role::Creator(_) => "owner", Role::Admin(_) => "admin", Role::Banned(_) => "banned", Role::Left(_) => "left", _ => "member" },
                presence: user_presence(&p.user),
            });
        }
        Ok(out)
    }

    /// Every member the account may see, following the participant pager up to
    /// `max`. The bool says whether the list is complete: `false` means the cap
    /// was hit and there are more members, so it must not be treated as a full
    /// roster (a group sweep would read the missing tail as departures).
    pub async fn chat_members_all(&self, chat_id: i64, max: usize) -> Result<(Vec<Member>, bool)> {
        use grammers_client::peer::Role;
        let peer = self.peer_ref(chat_id).await?;
        let mut iter = self.client.iter_participants(peer);
        let mut out = Vec::new();
        while out.len() < max {
            let Some(p) = iter.next().await? else { return Ok((out, true)) };
            out.push(Member {
                id: p.user.id().bare_id_unchecked(),
                name: p.user.full_name(),
                username: p.user.username().map(str::to_owned),
                role: match p.role { Role::Creator(_) => "owner", Role::Admin(_) => "admin", Role::Banned(_) => "banned", Role::Left(_) => "left", _ => "member" },
                presence: user_presence(&p.user),
            });
        }
        Ok((out, false))
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
        if self.self_id.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            let _ = self.me().await;
        }
        let me = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let event = match stream.next().await {
                    Ok(Update::NewMessage(m)) => {
                        let self_id = me.self_id.load(std::sync::atomic::Ordering::Relaxed);
                        if let Some(ev) = membership_from_service(&m, self_id) {
                            let _ = tx.send(TgEvent::Membership(ev));
                        }
                        TgEvent::NewMessage(convert(&m))
                    }
                    Ok(Update::Raw(raw)) => {
                        let self_id = me.self_id.load(std::sync::atomic::Ordering::Relaxed);
                        match membership_from_raw(&raw.raw, self_id) {
                            Some(ev) => ev,
                            None => continue,
                        }
                    }
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
        sender_photo: sender.and_then(peer_photo_id).map(|id| id.to_string()),
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
        media: m.media().map(|media| media_info(&media)),
        grouped_id: m.grouped_id(),
    }
}

// ---- membership detection ------------------------------------------------------

use mellogram_core::relationships::{classify, EventSource, MembershipEvent, MembershipKind, ParticipantState};

fn banned_state(b: &tl::types::ChannelParticipantBanned) -> ParticipantState {
    let tl::enums::ChatBannedRights::Rights(r) = &b.banned_rights;
    ParticipantState::Banned { view_messages: r.view_messages, until: i64::from(r.until_date) }
}

fn channel_state(p: Option<&tl::enums::ChannelParticipant>) -> ParticipantState {
    use tl::enums::ChannelParticipant as C;
    match p {
        None => ParticipantState::Absent,
        Some(C::Participant(_) | C::ParticipantSelf(_)) => ParticipantState::Member,
        Some(C::Creator(_)) => ParticipantState::Creator,
        Some(C::Admin(_)) => ParticipantState::Admin,
        Some(C::Banned(b)) => banned_state(b),
        Some(C::Left(_)) => ParticipantState::Left,
    }
}

fn chat_state(p: Option<&tl::enums::ChatParticipant>) -> ParticipantState {
    use tl::enums::ChatParticipant as C;
    match p {
        None => ParticipantState::Absent,
        Some(C::Participant(_)) => ParticipantState::Member,
        Some(C::Creator(_)) => ParticipantState::Creator,
        Some(C::Admin(_)) => ParticipantState::Admin,
    }
}

fn until_of(state: ParticipantState) -> Option<i64> {
    match state {
        // Telegram uses 0 (or a far-future date) for "forever".
        ParticipantState::Banned { until, .. } if until > 0 && until < 32_503_680_000 => Some(until),
        _ => None,
    }
}

/// Participant-change and channel-state updates → events.
pub fn membership_from_raw(update: &tl::enums::Update, self_id: i64) -> Option<TgEvent> {
    match update {
        tl::enums::Update::ChannelParticipant(u) => {
            let (prev, new) = (channel_state(u.prev_participant.as_ref()), channel_state(u.new_participant.as_ref()));
            let kind = classify(prev, new, u.actor_id == u.user_id)?;
            Some(TgEvent::Membership(MembershipEvent {
                chat_id: PeerId::channel_unchecked(u.channel_id).bot_api_dialog_id_unchecked(),
                chat_title: None,
                user_id: u.user_id,
                user_name: None,
                actor_id: (u.actor_id != 0).then_some(u.actor_id),
                actor_name: None,
                kind,
                until: until_of(new),
                is_self: self_id != 0 && u.user_id == self_id,
                date: i64::from(u.date),
                source: EventSource::ParticipantUpdate,
            }))
        }
        tl::enums::Update::ChatParticipant(u) => {
            let (prev, new) = (chat_state(u.prev_participant.as_ref()), chat_state(u.new_participant.as_ref()));
            let kind = classify(prev, new, u.actor_id == u.user_id)?;
            Some(TgEvent::Membership(MembershipEvent {
                chat_id: PeerId::chat_unchecked(u.chat_id).bot_api_dialog_id_unchecked(),
                chat_title: None,
                user_id: u.user_id,
                user_name: None,
                actor_id: (u.actor_id != 0).then_some(u.actor_id),
                actor_name: None,
                kind,
                until: None,
                is_self: self_id != 0 && u.user_id == self_id,
                date: i64::from(u.date),
                source: EventSource::ParticipantUpdate,
            }))
        }
        tl::enums::Update::UserStatus(u) => Some(TgEvent::Presence { user_id: u.user_id, presence: presence_of(&u.status) }),
        tl::enums::Update::Channel(u) => Some(TgEvent::ChatChanged {
            chat_id: PeerId::channel_unchecked(u.channel_id).bot_api_dialog_id_unchecked(),
        }),
        _ => None,
    }
}

/// "X removed Y" / "Y left" service messages (basic groups and supergroups).
pub fn membership_from_service(m: &grammers_client::message::Message, self_id: i64) -> Option<MembershipEvent> {
    let tl::enums::MessageAction::ChatDeleteUser(a) = m.action()? else { return None };
    let actor = m.sender_id().map(|s| s.bare_id_unchecked());
    let kind = if actor == Some(a.user_id) { MembershipKind::Left } else { MembershipKind::Removed };
    Some(MembershipEvent {
        chat_id: m.peer_id().bot_api_dialog_id_unchecked(),
        chat_title: None,
        user_id: a.user_id,
        user_name: None,
        actor_id: actor,
        actor_name: m.sender().and_then(|s| s.name()).map(str::to_owned),
        kind,
        until: None,
        is_self: self_id != 0 && a.user_id == self_id,
        date: m.date().as_second(),
        source: EventSource::ServiceMessage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rights(view_messages: bool, until_date: i32) -> tl::enums::ChatBannedRights {
        tl::enums::ChatBannedRights::Rights(tl::types::ChatBannedRights {
            view_messages, send_messages: true, send_media: false, send_stickers: false, send_gifs: false, send_games: false,
            send_inline: false, embed_links: false, send_polls: false, change_info: false, invite_users: false, pin_messages: false,
            manage_topics: false, send_photos: false, send_videos: false, send_roundvideos: false, send_audios: false,
            send_voices: false, send_docs: false, send_plain: false, edit_rank: false, send_reactions: false,
            manage_linked_peers: false, until_date,
        })
    }

    fn banned(view: bool, until: i32) -> tl::enums::ChannelParticipant {
        tl::enums::ChannelParticipant::Banned(tl::types::ChannelParticipantBanned {
            left: true,
            peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 42 }),
            kicked_by: 7,
            date: 0,
            banned_rights: rights(view, until),
            rank: None,
        })
    }

    fn update(prev: Option<tl::enums::ChannelParticipant>, new: Option<tl::enums::ChannelParticipant>, user: i64, actor: i64) -> tl::enums::Update {
        tl::enums::Update::ChannelParticipant(tl::types::UpdateChannelParticipant {
            via_chatlist: false, channel_id: 1234, date: 1_700_000_000, actor_id: actor, user_id: user,
            prev_participant: prev, new_participant: new, invite: None, qts: 1,
        })
    }

    fn member() -> tl::enums::ChannelParticipant {
        tl::enums::ChannelParticipant::Participant(tl::types::ChannelParticipant { user_id: 42, date: 0, subscription_until_date: None, rank: None })
    }

    #[test]
    fn detects_self_ban_with_expiry() {
        let ev = membership_from_raw(&update(Some(member()), Some(banned(true, 1_800_000_000)), 42, 7), 42).unwrap();
        let TgEvent::Membership(ev) = ev else { panic!() };
        assert_eq!(ev.kind, MembershipKind::Banned);
        assert!(ev.is_self);
        assert_eq!(ev.chat_id, -1_000_000_001_234);
        assert_eq!(ev.until, Some(1_800_000_000));
        assert_eq!(ev.actor_id, Some(7));
    }

    #[test]
    fn restriction_kick_and_unban() {
        let TgEvent::Membership(ev) = membership_from_raw(&update(Some(member()), Some(banned(false, 0)), 42, 7), 1).unwrap() else { panic!() };
        assert_eq!((ev.kind, ev.is_self, ev.until), (MembershipKind::Restricted, false, None));
        let TgEvent::Membership(ev) = membership_from_raw(&update(Some(member()), None, 42, 7), 1).unwrap() else { panic!() };
        assert_eq!(ev.kind, MembershipKind::Removed);
        let TgEvent::Membership(ev) = membership_from_raw(&update(Some(banned(true, 0)), None, 42, 7), 1).unwrap() else { panic!() };
        assert_eq!(ev.kind, MembershipKind::Unbanned);
        assert!(membership_from_raw(&update(Some(member()), Some(member()), 42, 42), 1).is_none());
    }

    #[test]
    fn channel_change_is_reported() {
        let u = tl::enums::Update::Channel(tl::types::UpdateChannel { channel_id: 99 });
        assert!(matches!(membership_from_raw(&u, 1), Some(TgEvent::ChatChanged { chat_id: -1_000_000_000_099 })));
    }

    fn sent_code(kind: tl::enums::auth::SentCodeType) -> tl::types::auth::SentCode {
        tl::types::auth::SentCode {
            r#type: kind,
            phone_code_hash: "hash".into(),
            next_type: Some(tl::enums::auth::CodeType::Sms),
            timeout: Some(60),
        }
    }

    #[test]
    fn normalizes_phone_numbers() {
        assert_eq!(normalize_phone("+1 (555) 123-4567").as_deref(), Some("+15551234567"));
        assert_eq!(normalize_phone("00 44 20 7946 0018").as_deref(), Some("+442079460018"));
        assert_eq!(normalize_phone("+15551234567").as_deref(), Some("+15551234567"));
        assert_eq!(normalize_phone("12"), None);
        assert_eq!(normalize_phone("+1 555 CALL NOW"), None);
        assert_eq!(normalize_phone("+0123456789"), None);
    }

    #[test]
    fn stale_resend_does_not_match_a_cancelled_or_replaced_code() {
        let delivery = delivery_from(&sent_code(tl::enums::auth::SentCodeType::App(
            tl::types::auth::SentCodeTypeApp { length: 5 },
        )));
        let pending = Pending::Code { phone: "+15551234567".into(), hash: "new".into(), delivery };
        assert!(pending.matches_code("+15551234567", "new"));
        assert!(!pending.matches_code("+15551234567", "old"));
        assert!(!pending.matches_code("+15557654321", "new"));
        assert!(!Pending::None.matches_code("+15551234567", "new"));
    }

    #[test]
    fn reports_where_telegram_sent_the_code() {
        let app = delivery_from(&sent_code(tl::enums::auth::SentCodeType::App(
            tl::types::auth::SentCodeTypeApp { length: 5 },
        )));
        assert_eq!((app.via, app.length, app.next, app.timeout), ("app", Some(5), Some("sms"), Some(60)));

        let email = delivery_from(&sent_code(tl::enums::auth::SentCodeType::EmailCode(
            tl::types::auth::SentCodeTypeEmailCode {
                apple_signin_allowed: false,
                google_signin_allowed: false,
                email_pattern: "t***@example.com".into(),
                length: 6,
                reset_available_period: None,
                reset_pending_date: None,
            },
        )));
        assert_eq!(email.via, "email");
        assert_eq!(email.email.as_deref(), Some("t***@example.com"));

        let fragment = delivery_from(&sent_code(tl::enums::auth::SentCodeType::FragmentSms(
            tl::types::auth::SentCodeTypeFragmentSms { url: "https://fragment.com/".into(), length: 5 },
        )));
        assert_eq!(fragment.via, "fragment");
        assert_eq!(fragment.url.as_deref(), Some("https://fragment.com/"));

        let setup = delivery_from(&sent_code(tl::enums::auth::SentCodeType::SetUpEmailRequired(
            tl::types::auth::SentCodeTypeSetUpEmailRequired {
                apple_signin_allowed: false,
                google_signin_allowed: false,
            },
        )));
        assert_eq!(setup.via, "email-setup");
    }
}

// ---- media ---------------------------------------------------------------------

use grammers_client::media::Media;
use mellogram_core::model::{ContactInfo, GeoInfo, MediaInfo, MediaKind, PollInfo, WebPreview};

fn text_of(t: &tl::enums::TextWithEntities) -> String {
    let tl::enums::TextWithEntities::Entities(t) = t;
    t.text.clone()
}

fn geo_of(g: &grammers_client::media::Geo) -> GeoInfo {
    GeoInfo { lat: g.latitue() /* sic: grammers spelling */, long: g.longitude(), accuracy_m: g.accuracy_radius(), ..Default::default() }
}

/// Describes a message attachment for the UI.
pub fn media_info(media: &Media) -> MediaInfo {
    let mut info = MediaInfo::default();
    match media {
        Media::Photo(p) => {
            info.kind = MediaKind::Photo;
            info.mime = Some("image/jpeg".into());
            info.size = p.size().map(|s| s as u64);
            info.spoiler = p.is_spoiler();
            info.has_thumbnail = !p.thumbs().is_empty();
            if let Some(tl::enums::Photo::Photo(raw)) = &p.raw.photo {
                if let Some((w, h)) = raw.sizes.iter().filter_map(|s| match s {
                    tl::enums::PhotoSize::Size(s) => Some((s.w, s.h)),
                    tl::enums::PhotoSize::Progressive(s) => Some((s.w, s.h)),
                    _ => None,
                }).max_by_key(|(w, h)| i64::from(*w) * i64::from(*h)) {
                    info.width = u32::try_from(w).ok();
                    info.height = u32::try_from(h).ok();
                }
            }
        }
        Media::Sticker(s) => {
            let mime = s.document.mime_type().unwrap_or("");
            info.kind = if mime == "application/x-tgsticker" {
                MediaKind::AnimatedSticker
            } else if mime == "video/webm" {
                MediaKind::VideoSticker
            } else {
                MediaKind::Sticker
            };
            info.mime = Some(mime.to_owned());
            info.size = s.document.size().map(|v| v as u64);
            info.emoji = Some(s.emoji().to_owned()).filter(|e| !e.is_empty());
            if let Some((w, h)) = s.document.resolution() {
                info.width = u32::try_from(w).ok();
                info.height = u32::try_from(h).ok();
            }
        }
        Media::Document(d) => {
            info.mime = d.mime_type().map(str::to_owned);
            info.file_name = d.name().map(str::to_owned).filter(|n| !n.is_empty());
            info.size = d.size().map(|v| v as u64);
            info.duration = d.duration();
            info.title = d.audio_title().map(str::to_owned);
            info.performer = d.performer().map(str::to_owned);
            info.spoiler = d.is_spoiler();
            info.has_thumbnail = !d.thumbs().is_empty();
            if let Some((w, h)) = d.resolution() {
                info.width = u32::try_from(w).ok();
                info.height = u32::try_from(h).ok();
            }
            let attrs: Vec<tl::enums::DocumentAttribute> = match &d.raw.document {
                Some(tl::enums::Document::Document(doc)) => doc.attributes.clone(),
                _ => Vec::new(),
            };
            let animated = attrs.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Animated));
            let voice = d.raw.voice || attrs.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Audio(x) if x.voice));
            let round = d.raw.round || attrs.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Video(x) if x.round_message));
            let video = attrs.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Video(_)));
            let audio = attrs.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Audio(_)));
            info.kind = if animated {
                MediaKind::Gif
            } else if voice {
                MediaKind::Voice
            } else if round {
                MediaKind::RoundVideo
            } else if video {
                MediaKind::Video
            } else if audio {
                MediaKind::Audio
            } else if info.mime.as_deref().is_some_and(|m| m.starts_with("image/")) && info.mime.as_deref() != Some("image/svg+xml") {
                MediaKind::Photo
            } else {
                MediaKind::Document
            };
        }
        Media::Contact(c) => {
            info.kind = MediaKind::Contact;
            info.contact = Some(ContactInfo {
                name: format!("{} {}", c.raw.first_name, c.raw.last_name).trim().to_owned(),
                phone: c.raw.phone_number.clone(),
            });
        }
        Media::Poll(p) => {
            info.kind = MediaKind::Poll;
            let tl::enums::PollResults::Results(r) = &tl::enums::PollResults::Results(p.raw_results.clone());
            info.poll = Some(PollInfo {
                question: text_of(&p.raw.question),
                options: p.raw.answers.iter().filter_map(|a| match a { tl::enums::PollAnswer::Answer(a) => Some(text_of(&a.text)), _ => None }).collect(),
                voters: r.total_voters,
                closed: p.raw.closed,
                quiz: p.raw.quiz,
            });
        }
        Media::Geo(g) => {
            info.kind = MediaKind::Location;
            info.geo = Some(geo_of(g));
        }
        Media::GeoLive(g) => {
            info.kind = MediaKind::LiveLocation;
            let mut geo = g.geo.as_ref().map(geo_of).unwrap_or_default();
            geo.live_period = Some(g.period());
            info.geo = Some(geo);
        }
        Media::Venue(v) => {
            info.kind = MediaKind::Venue;
            let mut geo = v.geo.as_ref().map(geo_of).unwrap_or_default();
            geo.title = Some(v.title().to_owned());
            geo.address = Some(v.address().to_owned()).filter(|a| !a.is_empty());
            info.geo = Some(geo);
        }
        Media::Dice(d) => {
            info.kind = MediaKind::Dice;
            info.emoji = Some(d.raw.emoticon.clone());
            info.value = Some(d.raw.value);
        }
        Media::WebPage(w) => {
            info.kind = MediaKind::WebPage;
            if let tl::enums::WebPage::Page(p) = &w.raw.webpage {
                info.web = Some(WebPreview { url: p.url.clone(), site_name: p.site_name.clone(), title: p.title.clone(), description: p.description.clone() });
            }
        }
        _ => info.kind = MediaKind::Unsupported,
    }
    info
}

/// Which file to fetch for a message's media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaVariant {
    Full,
    Thumbnail,
}

/// The id of a peer's current profile photo, if it has one.
pub fn peer_photo_id(peer: &Peer) -> Option<i64> {
    match peer {
        Peer::User(u) => u.photo().map(|p| p.photo_id),
        Peer::Group(g) => g.photo().map(|p| p.photo_id),
        Peer::Channel(c) => c.photo().map(|p| p.photo_id),
        _ => None,
    }
}

impl Telegram {
    /// Downloads a chat's profile photo (small 160px, or `big` 640px) to `dest`.
    /// Uses the photo id from the chat list, so a changed photo gets a new file.
    pub async fn download_profile_photo(&self, chat_id: i64, photo_id: i64, big: bool, dest: &std::path::Path) -> Result<bool> {
        let max_bytes: u64 = if big { 4 * 1024 * 1024 } else { 512 * 1024 };
        let peer = self.peer_ref(chat_id).await?;
        let photo = grammers_client::media::ChatPhoto {
            raw: tl::enums::InputFileLocation::InputPeerPhotoFileLocation(tl::types::InputPeerPhotoFileLocation {
                big,
                peer: peer.into(),
                photo_id,
            }),
        };
        let tmp = dest.with_extension("part");
        self.client.download_media(&photo, &tmp).await?;
        let len = tokio::fs::metadata(&tmp).await.map(|m| m.len()).unwrap_or(0);
        if len == 0 || len > max_bytes {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Ok(false);
        }
        tokio::fs::rename(&tmp, dest).await?;
        Ok(true)
    }

    /// Downloads a message's media (or its thumbnail) to `dest`. Refuses files
    /// larger than `max_bytes`. Writes to a temporary file and renames it, so a
    /// partial download never looks complete. Returns `false` if there is nothing to download.
    pub async fn download_media(&self, chat_id: i64, message_id: i32, variant: MediaVariant, dest: &std::path::Path, max_bytes: u64) -> Result<bool> {
        let peer = self.peer_ref(chat_id).await?;
        let msgs = self.client.get_messages_by_id(peer, &[message_id]).await?;
        let Some(Some(msg)) = msgs.into_iter().next() else { return Ok(false) };
        let Some(media) = msg.media() else { return Ok(false) };
        let tmp = dest.with_extension("part");
        let ok = match variant {
            MediaVariant::Thumbnail => {
                let thumbs = match &media {
                    Media::Photo(p) => p.thumbs(),
                    Media::Document(d) => d.thumbs(),
                    Media::Sticker(s) => s.document.thumbs(),
                    _ => Vec::new(),
                };
                // The largest thumbnail that is still small.
                let Some(thumb) = thumbs.into_iter().filter(|t| t.size() <= 512 * 1024).max_by_key(|t| t.size()) else { return Ok(false) };
                self.client.download_media(&thumb, &tmp).await?;
                true
            }
            MediaVariant::Full => {
                let size = match &media {
                    Media::Photo(p) => p.size(),
                    Media::Document(d) => d.size(),
                    Media::Sticker(s) => s.document.size(),
                    _ => return Ok(false),
                };
                if size.is_some_and(|s| s as u64 > max_bytes) {
                    return Err(TgError::Login(format!("file is larger than the {} MB limit", max_bytes / 1024 / 1024)));
                }
                self.client.download_media(&media, &tmp).await?;
                true
            }
        };
        if !ok {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Ok(false);
        }
        if tokio::fs::metadata(&tmp).await?.len() > max_bytes {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(TgError::Login("downloaded file exceeded the size limit".into()));
        }
        tokio::fs::rename(&tmp, dest).await?;
        Ok(true)
    }
}
