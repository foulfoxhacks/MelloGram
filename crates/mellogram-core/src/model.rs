//! Message types exposed to plugins.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A message the user is about to send. `before_send` hooks may rewrite it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// Bot-API style dialog id (users > 0, groups < 0, channels -100…).
    pub chat_id: i64,
    pub text: String,
    /// Deliver without a notification sound.
    pub silent: bool,
    pub reply_to: Option<i32>,
    /// Ask Telegram to generate a link preview.
    pub link_preview: bool,
    pub attachments: Vec<OutgoingFile>,
    /// Unix time to deliver at (Telegram scheduled message). `None` sends now.
    #[serde(default)]
    pub schedule_at: Option<i64>,
}

impl OutgoingMessage {
    pub fn text(chat_id: i64, text: impl Into<String>) -> Self {
        Self {
            chat_id,
            text: text.into(),
            link_preview: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutgoingFile {
    pub path: PathBuf,
    /// Name shown to recipients. Plugins may rename it.
    pub file_name: String,
}

/// A message as displayed in the client.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub chat_id: i64,
    pub id: i32,
    pub sender_id: Option<i64>,
    /// Display name as Telegram reports it. Plugins may rewrite `display_name`.
    pub sender_name: Option<String>,
    pub sender_username: Option<String>,
    /// Sender's current profile photo id (string: does not fit a JS number).
    #[serde(default)]
    pub sender_photo: Option<String>,
    pub display_name: Option<String>,
    pub text: String,
    /// Unix seconds.
    pub date: i64,
    pub edit_date: Option<i64>,
    pub outgoing: bool,
    pub silent: bool,
    pub reply_to: Option<i32>,
    pub has_media: bool,
    /// Plugin-provided decorations rendered next to the message.
    pub badges: Vec<MessageBadge>,
    /// Previous versions of the text, oldest first (filled by history plugins).
    pub edit_history: Vec<String>,
    /// Set when the message was deleted but a plugin asked to keep showing it.
    pub deleted: bool,
    /// Extra CSS classes (sanitised to `[a-z0-9-]`) added by plugins.
    pub classes: Vec<String>,
    /// Highlight colour from a highlighting plugin (already validated).
    pub highlight: Option<String>,
    /// What the attachment is, so the UI can render it.
    pub media: Option<MediaInfo>,
    /// Messages sharing this id form one album.
    pub grouped_id: Option<i64>,
}

/// Backend-neutral description of a message attachment.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub kind: MediaKind,
    pub mime: Option<String>,
    pub file_name: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Seconds (audio/video).
    pub duration: Option<f64>,
    pub title: Option<String>,
    pub performer: Option<String>,
    /// Sticker emoji / dice emoji.
    pub emoji: Option<String>,
    pub has_thumbnail: bool,
    pub spoiler: bool,
    pub geo: Option<GeoInfo>,
    pub web: Option<WebPreview>,
    pub contact: Option<ContactInfo>,
    pub poll: Option<PollInfo>,
    /// Dice value.
    pub value: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Photo,
    Video,
    /// Looping silent animation (GIF / MP4).
    Gif,
    RoundVideo,
    Voice,
    Audio,
    Sticker,
    /// Lottie (TGS) sticker.
    AnimatedSticker,
    /// WebM sticker.
    VideoSticker,
    Document,
    Location,
    LiveLocation,
    Venue,
    Contact,
    Poll,
    Dice,
    WebPage,
    #[default]
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeoInfo {
    pub lat: f64,
    pub long: f64,
    pub accuracy_m: Option<i32>,
    pub title: Option<String>,
    pub address: Option<String>,
    /// Live location: seconds it is shared for.
    pub live_period: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebPreview {
    pub url: String,
    pub site_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactInfo {
    pub name: String,
    pub phone: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollInfo {
    pub question: String,
    pub options: Vec<String>,
    pub voters: Option<i32>,
    pub closed: bool,
    pub quiz: bool,
}

impl IncomingMessage {
    /// Adds a CSS class, dropping anything that is not `[a-z0-9-]`.
    pub fn add_class(&mut self, class: &str) {
        let clean: String = class.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-').collect();
        if !clean.is_empty() && !self.classes.contains(&clean) {
            self.classes.push(clean);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageBadge {
    pub plugin: String,
    pub label: String,
    pub tooltip: Option<String>,
}
