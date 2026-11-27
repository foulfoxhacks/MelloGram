//! Background operation policy: when a new message should raise a native
//! notification and what it may show. Pure logic so it is easy to test; the
//! app supplies window focus and the local hour.

use crate::model::IncomingMessage;
use crate::settings::NotificationSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationContent {
    pub title: String,
    pub body: String,
}

/// `true` when `hour` (0–23) falls inside the quiet window. Handles windows
/// that wrap midnight; an empty window (start == end) is never quiet.
pub fn in_quiet_hours(start: u8, end: u8, hour: u8) -> bool {
    let (s, e, h) = (start % 24, end % 24, hour % 24);
    if s == e {
        false
    } else if s < e {
        (s..e).contains(&h)
    } else {
        h >= s || h < e
    }
}

/// Decides whether to notify, and builds privacy-respecting content.
pub fn notification_for(
    settings: &NotificationSettings,
    msg: &IncomingMessage,
    chat_title: Option<&str>,
    window_focused: bool,
    local_hour: u8,
) -> Option<NotificationContent> {
    if !settings.desktop || settings.dnd || window_focused {
        return None;
    }
    if msg.outgoing && !settings.notify_own {
        return None;
    }
    if msg.silent || settings.muted_chats.contains(&msg.chat_id) {
        return None;
    }
    if settings.quiet_hours && in_quiet_hours(settings.quiet_start, settings.quiet_end, local_hour) {
        return None;
    }
    if !settings.show_previews {
        return Some(NotificationContent { title: "MelloGram".into(), body: "New message".into() });
    }
    let sender = msg.display_name.as_deref().or(msg.sender_name.as_deref());
    let title = match (chat_title, sender) {
        (Some(chat), Some(who)) if chat != who => format!("{who} · {chat}"),
        (Some(chat), _) => chat.to_owned(),
        (None, Some(who)) => who.to_owned(),
        (None, None) => "MelloGram".to_owned(),
    };
    let mut body: String = msg.text.chars().take(180).collect();
    if msg.text.chars().count() > 180 {
        body.push('…');
    }
    if body.trim().is_empty() {
        body = if msg.has_media { "📎 Media".into() } else { "New message".into() };
    }
    Some(NotificationContent { title, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(text: &str) -> IncomingMessage {
        IncomingMessage { chat_id: 7, text: text.into(), sender_name: Some("Ava".into()), ..Default::default() }
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        assert!(in_quiet_hours(22, 7, 23) && in_quiet_hours(22, 7, 3) && !in_quiet_hours(22, 7, 12));
        assert!(in_quiet_hours(9, 17, 9) && !in_quiet_hours(9, 17, 17));
        assert!(!in_quiet_hours(5, 5, 5));
    }

    #[test]
    fn respects_focus_dnd_mute_silent_and_quiet_hours() {
        let s = NotificationSettings::default();
        assert!(notification_for(&s, &msg("hi"), Some("Chat"), false, 12).is_some());
        assert!(notification_for(&s, &msg("hi"), Some("Chat"), true, 12).is_none(), "focused");
        assert!(notification_for(&NotificationSettings { dnd: true, ..s.clone() }, &msg("hi"), None, false, 12).is_none());
        assert!(notification_for(&NotificationSettings { muted_chats: vec![7], ..s.clone() }, &msg("hi"), None, false, 12).is_none());
        assert!(notification_for(&s, &IncomingMessage { silent: true, ..msg("hi") }, None, false, 12).is_none());
        assert!(notification_for(&s, &IncomingMessage { outgoing: true, ..msg("hi") }, None, false, 12).is_none());
        let quiet = NotificationSettings { quiet_hours: true, ..s.clone() };
        assert!(notification_for(&quiet, &msg("hi"), None, false, 23).is_none());
        assert!(notification_for(&quiet, &msg("hi"), None, false, 12).is_some());
    }

    #[test]
    fn content_and_privacy() {
        let s = NotificationSettings::default();
        let n = notification_for(&s, &msg("hello there"), Some("Mello Devs"), false, 12).unwrap();
        assert_eq!((n.title.as_str(), n.body.as_str()), ("Ava · Mello Devs", "hello there"));
        let n = notification_for(&s, &msg(""), Some("Ava"), false, 12).unwrap();
        assert_eq!((n.title.as_str(), n.body.as_str()), ("Ava", "New message"));
        let private = NotificationSettings { show_previews: false, ..s };
        let n = notification_for(&private, &msg("secret plans"), Some("Ava"), false, 12).unwrap();
        assert!(!n.title.contains("Ava") && !n.body.contains("secret"));
        let long = "x".repeat(400);
        assert_eq!(notification_for(&NotificationSettings::default(), &msg(&long), None, false, 1).unwrap().body.chars().count(), 181);
    }
}
