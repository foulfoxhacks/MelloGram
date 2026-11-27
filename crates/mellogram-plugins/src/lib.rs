//! MelloGram's built-in plugins.
//!
//! Each plugin is an independent implementation of a feature idea popularised
//! by Discord client mods such as Vencord, adapted to Telegram. No Vencord
//! code is used; `PluginManifest::inspired_by` records the idea it adapts.
#![forbid(unsafe_code)]

use std::sync::Arc;

use mellogram_core::Plugin;

/// Declares a plugin whose behaviour lives entirely in its UI script.
macro_rules! ui_plugin {
    ($ty:ident, $id:literal, $name:literal, $cat:ident, $desc:literal, $inspired:literal, $default:expr,
     $script:literal, [$($perm:expr),* $(,)?], [$($setting:expr),* $(,)?]) => {
        pub struct $ty;
        impl mellogram_core::Plugin for $ty {
            fn manifest(&self) -> mellogram_core::PluginManifest {
                mellogram_core::PluginManifest {
                    description: $desc,
                    inspired_by: Some($inspired),
                    enabled_by_default: $default,
                    permissions: &[$($perm),*],
                    ..mellogram_core::PluginManifest::new($id, $name, mellogram_core::Category::$cat)
                }
            }
            #[allow(unused_imports)]
            fn settings(&self) -> Vec<mellogram_core::SettingDef> {
                use mellogram_core::SettingDef;
                vec![$($setting),*]
            }
            fn ui_script(&self) -> Option<&'static str> {
                Some(include_str!(concat!("../ui/", $script)))
            }
        }
    };
}

pub mod appearance;
pub mod bonds;
pub mod chat;
pub mod history;
pub mod privacy;
pub mod productivity;

/// Every built-in plugin, in no particular order (the manager sorts them).
pub fn builtin() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(bonds::BondWatch),
        Arc::new(chat::LinkScrubber),
        Arc::new(chat::TextReplace),
        Arc::new(chat::QuickCommands),
        Arc::new(chat::Unindent),
        Arc::new(chat::SilentSend),
        Arc::new(chat::LinkPreviews),
        Arc::new(privacy::SilentTyping),
        Arc::new(privacy::AnonymiseUploads),
        Arc::new(history::MessageHistory::default()),
        Arc::new(appearance::NameDisplay),
        Arc::new(appearance::ClientColors),
        Arc::new(appearance::CharCounter),
        Arc::new(appearance::PreciseTimestamps),
        Arc::new(appearance::PinChats),
        Arc::new(appearance::ViewRaw),
        Arc::new(appearance::ReadAll),
        Arc::new(appearance::SendLatency),
        Arc::new(productivity::MessageHighlight),
        Arc::new(productivity::SecretGuard),
        Arc::new(productivity::Calculator),
        Arc::new(productivity::PrivacyMode),
        Arc::new(productivity::ChatColorizer),
        Arc::new(productivity::QuickNotes),
        Arc::new(productivity::Bookmarks),
        Arc::new(productivity::MessageTemplates),
        Arc::new(productivity::ChatStatistics),
        Arc::new(productivity::ChatExport),
        Arc::new(productivity::LinkCollection),
        Arc::new(productivity::CopyEnhancer),
    ]
}

#[cfg(test)]
mod tests {
    use mellogram_core::{OutgoingMessage, PluginManager, Settings};

    #[test]
    fn all_builtins_load_and_cooperate() {
        let mut m = PluginManager::new(super::builtin(), None);
        let mut s = Settings::default();
        m.init(&s);
        for p in m.list(&s) {
            if !p.enabled {
                m.enable(p.manifest.id, &mut s).unwrap();
            }
        }
        assert!(m.list(&s).iter().all(|p| p.enabled && p.errors.is_empty()));
        assert!(!m.allow_typing(1));
        let out = m
            .process_outgoing(OutgoingMessage::text(1, "/shrug teh link <https://x.com/a?utm_source=1&s=20>"))
            .unwrap();
        assert_eq!(out.text, "the link https://x.com/a ¯\\_(ツ)_/¯");
        assert!(!out.link_preview);
        assert!(m.commands().iter().any(|c| c.name == "silent"));
        assert!(!m.css().is_empty());
        assert_eq!(m.ui_scripts().len(), 17);
        // Secret Guard runs last and blocks secrets even after other rewrites.
        assert!(m.process_outgoing(OutgoingMessage::text(1, "teh key AKIAIOSFODNN7EXAMPLE")).is_err());
        assert_eq!(m.process_outgoing(OutgoingMessage::text(1, "/calc 6*7")).unwrap().text, "6*7 = 42");
    }
}
