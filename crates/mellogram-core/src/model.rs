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
