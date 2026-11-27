//! Message history: keep deleted messages on screen and record edits.
//!
//! Everything stays on this device and in memory; nothing is uploaded and the
//! log is cleared when the client closes.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use mellogram_core::{Permission, Category, IncomingMessage, MessageBadge, Plugin, PluginContext, PluginManifest, SettingDef};

#[derive(Default)]
struct Cache {
    /// (chat_id, message_id) → sender is self
    seen: HashMap<(i64, i32), bool>,
    order: VecDeque<(i64, i32)>,
}

pub struct MessageHistory {
    cache: Mutex<Cache>,
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self { cache: Mutex::new(Cache::default()) }
    }
}

impl MessageHistory {
    fn remember(&self, ctx: &PluginContext, msg: &IncomingMessage) {
        let mut c = self.cache.lock().unwrap();
        let key = (msg.chat_id, msg.id);
        if c.seen.insert(key, msg.outgoing).is_none() {
            c.order.push_back(key);
        }
        let max = ctx.number("max_cached").max(100.0) as usize;
        while c.order.len() > max {
            if let Some(old) = c.order.pop_front() {
                c.seen.remove(&old);
            }
        }
    }

    fn badge(label: &str, tooltip: Option<String>) -> MessageBadge {
        MessageBadge { plugin: "message-history".into(), label: label.into(), tooltip }
    }
}

impl Plugin for MessageHistory {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Keeps deleted messages visible (struck through) and shows edit history. Stored only in memory on this device.",
            inspired_by: Some("Vencord: MessageLogger"),
            permissions: &[Permission::ReadMessages, Permission::Styles],
            ..PluginManifest::new("message-history", "Message History", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::boolean("keep_deleted", "Keep deleted messages", true),
            SettingDef::boolean("log_edits", "Record edit history", true),
            SettingDef::boolean("ignore_self", "Ignore your own messages", false),
            SettingDef::number("max_cached", "Messages to track", 5000.0, 100.0, 100000.0),
        ]
    }

    fn on_message(&self, ctx: &PluginContext, msg: &mut IncomingMessage) {
        self.remember(ctx, msg);
    }

    fn on_edit(&self, ctx: &PluginContext, previous: Option<&IncomingMessage>, msg: &mut IncomingMessage) {
        self.remember(ctx, msg);
        if !ctx.bool("log_edits") || (msg.outgoing && ctx.bool("ignore_self")) {
            return;
        }
        let Some(prev) = previous else { return };
        if prev.text == msg.text {
            return; // reaction or media-only update
        }
        msg.edit_history = prev.edit_history.clone();
        msg.edit_history.push(prev.text.clone());
        msg.badges.retain(|b| b.plugin != "message-history");
        let n = msg.edit_history.len();
        msg.badges.push(Self::badge(
            "edited",
            Some(format!("{n} earlier version{}", if n == 1 { "" } else { "s" })),
        ));
    }

    fn on_delete(&self, ctx: &PluginContext, chat_id: Option<i64>, ids: &[i32]) -> bool {
        if !ctx.bool("keep_deleted") {
            return false;
        }
        if !ctx.bool("ignore_self") {
            return true;
        }
        // Keep unless every deleted message we know about was our own.
        let c = self.cache.lock().unwrap();
        ids.iter().any(|id| {
            let own = match chat_id {
                Some(chat) => c.seen.get(&(chat, *id)).copied(),
                None => c.seen.iter().find(|((_, mid), _)| mid == id).map(|(_, own)| *own),
            };
            own != Some(true)
        })
    }

    fn css(&self, _: &PluginContext) -> Option<String> {
        Some(
            ".mg-message.mg-deleted .mg-text { text-decoration: line-through; opacity: .6; }\n\
             .mg-message.mg-deleted { background: color-mix(in srgb, var(--mg-danger) 10%, transparent); }\n\
             .mg-edit-history { font-size: .85em; opacity: .6; border-left: 2px solid var(--mg-border); padding-left: 6px; }"
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: i32, text: &str, outgoing: bool) -> IncomingMessage {
        IncomingMessage { chat_id: 5, id, text: text.into(), outgoing, ..Default::default() }
    }

    #[test]
    fn records_edits() {
        let p = MessageHistory::default();
        let ctx = PluginContext::with_defaults(&p);
        let mut first = msg(1, "helo", false);
        p.on_message(&ctx, &mut first);
        let mut second = msg(1, "hello", false);
        p.on_edit(&ctx, Some(&first), &mut second);
        let mut third = msg(1, "hello!", false);
        p.on_edit(&ctx, Some(&second), &mut third);
        assert_eq!(third.edit_history, vec!["helo", "hello"]);
        assert_eq!(third.badges.len(), 1);
        assert_eq!(third.badges[0].tooltip.as_deref(), Some("2 earlier versions"));
    }

    #[test]
    fn keep_deleted_respects_ignore_self() {
        let p = MessageHistory::default();
        let mut ctx = PluginContext::with_defaults(&p);
        p.on_message(&ctx, &mut msg(1, "mine", true));
        p.on_message(&ctx, &mut msg(2, "theirs", false));
        assert!(p.on_delete(&ctx, None, &[1]));
        ctx.set("ignore_self", serde_json::json!(true));
        assert!(!p.on_delete(&ctx, None, &[1]));
        assert!(p.on_delete(&ctx, Some(5), &[1, 2]));
        ctx.set("keep_deleted", serde_json::json!(false));
        assert!(!p.on_delete(&ctx, None, &[2]));
    }

    #[test]
    fn cache_is_bounded() {
        let p = MessageHistory::default();
        let mut ctx = PluginContext::with_defaults(&p);
        ctx.set("max_cached", serde_json::json!(100));
        for i in 0..250 {
            p.on_message(&ctx, &mut msg(i, "x", false));
        }
        assert_eq!(p.cache.lock().unwrap().order.len(), 100);
    }
}
