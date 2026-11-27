//! Bond Watch: alerts for bans, removals, restrictions and lost access.

use mellogram_core::relationships::{MembershipEvent, MembershipKind};
use mellogram_core::{Category, Permission, Plugin, PluginContext, PluginManifest, SettingDef};

pub struct BondWatch;

impl BondWatch {
    /// Whether this event should raise an alert under the current settings.
    pub fn wants(ctx: &PluginContext, e: &MembershipEvent) -> bool {
        use MembershipKind as K;
        if e.is_self {
            return match e.kind {
                K::Banned | K::Removed | K::Restricted | K::Unbanned => ctx.bool("alert_self"),
                K::AccessLost => ctx.bool("lost_access"),
                K::Promoted | K::Demoted => ctx.bool("admin_changes"),
                K::Left | K::Joined => false, // your own actions
            };
        }
        match e.kind {
            K::Banned | K::Removed | K::Restricted | K::Unbanned => ctx.bool("alert_moderation"),
            K::Left => ctx.bool("include_leaves"),
            K::Joined => ctx.bool("include_joins"),
            K::Promoted | K::Demoted => ctx.bool("admin_changes"),
            K::AccessLost => false,
        }
    }
}

impl Plugin for BondWatch {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Alerts you the moment you are banned, removed or restricted, when you lose access to a chat, and when members are banned in groups you manage. Keeps a local history.",
            enabled_by_default: true,
            permissions: &[Permission::Ui],
            ..PluginManifest::new("bond-watch", "Bond Watch", Category::Notifications)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::boolean("alert_self", "When I am banned, removed, restricted or unbanned", true),
            SettingDef::boolean("lost_access", "When a chat disappears from my list", true),
            SettingDef::boolean("alert_moderation", "Bans and removals in groups I manage", true),
            SettingDef::boolean("admin_changes", "Admin promotions and demotions", true),
            SettingDef::boolean("include_leaves", "Members leaving", false),
            SettingDef::boolean("include_joins", "Members joining", false),
        ]
    }

    fn on_membership(&self, ctx: &PluginContext, e: &MembershipEvent) -> Option<String> {
        Self::wants(ctx, e).then(|| e.describe())
    }

    fn ui_script(&self) -> Option<&'static str> {
        Some(include_str!("../ui/bond-watch.js"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mellogram_core::relationships::EventSource;

    fn ev(kind: MembershipKind, is_self: bool) -> MembershipEvent {
        MembershipEvent { chat_id: -1, chat_title: Some("Den".into()), user_id: 5, user_name: Some("Kai".into()), actor_id: None, actor_name: None, kind, until: None, is_self, date: 0, source: EventSource::ParticipantUpdate }
    }

    #[test]
    fn filters_follow_settings() {
        let p = BondWatch;
        let mut ctx = PluginContext::with_defaults(&p);
        assert_eq!(p.on_membership(&ctx, &ev(MembershipKind::Banned, true)).as_deref(), Some("You were banned from Den"));
        assert!(p.on_membership(&ctx, &ev(MembershipKind::AccessLost, true)).is_some());
        assert!(p.on_membership(&ctx, &ev(MembershipKind::Removed, false)).is_some());
        assert!(p.on_membership(&ctx, &ev(MembershipKind::Left, false)).is_none());
        assert!(p.on_membership(&ctx, &ev(MembershipKind::Left, true)).is_none(), "own actions never alert");
        ctx.set("alert_self", serde_json::json!(false));
        assert!(p.on_membership(&ctx, &ev(MembershipKind::Banned, true)).is_none());
    }
}
