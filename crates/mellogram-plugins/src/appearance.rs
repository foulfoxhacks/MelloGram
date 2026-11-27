//! Appearance and interface plugins.

use mellogram_core::{Permission, Category, IncomingMessage, Plugin, PluginContext, PluginManifest, SettingDef};

/// Choose how sender names are displayed.
pub struct NameDisplay;

impl NameDisplay {
    pub fn format(mode: &str, name: Option<&str>, username: Option<&str>) -> Option<String> {
        let user = username.map(|u| format!("@{u}"));
        match (mode, name, user) {
            ("username", _, Some(u)) => Some(u),
            ("name-username", Some(n), Some(u)) => Some(format!("{n} ({u})")),
            ("username-name", Some(n), Some(u)) => Some(format!("{u} ({n})")),
            (_, Some(n), _) => Some(n.to_owned()),
            (_, None, u) => u,
        }
    }
}

impl Plugin for NameDisplay {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Show @usernames next to (or instead of) display names.",
            inspired_by: Some("Vencord: ShowMeYourName"),
            permissions: &[Permission::ReadMessages],
            ..PluginManifest::new("name-display", "Name Display", Category::Appearance)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::select(
            "mode",
            "Format",
            "name-username",
            &[
                ("name-username", "Name (@username)"),
                ("username-name", "@username (Name)"),
                ("username", "@username only"),
                ("name", "Name only"),
            ],
        )]
    }

    fn on_message(&self, ctx: &PluginContext, msg: &mut IncomingMessage) {
        msg.display_name = Self::format(ctx.str("mode"), msg.sender_name.as_deref(), msg.sender_username.as_deref());
    }

    fn on_edit(&self, ctx: &PluginContext, _: Option<&IncomingMessage>, msg: &mut IncomingMessage) {
        self.on_message(ctx, msg);
    }
}

/// Recolour the client without writing a theme.
pub struct ClientColors;

fn safe_color(v: &str) -> Option<&str> {
    let v = v.trim();
    let ok = !v.is_empty() && v.len() < 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || "#(),.% -".contains(c));
    ok.then_some(v)
}

impl Plugin for ClientColors {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Pick accent and background colours for the whole client.",
            inspired_by: Some("Vencord: ClientTheme"),
            permissions: &[Permission::Styles],
            ..PluginManifest::new("client-colors", "Client Colors", Category::Appearance)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::text("accent", "Accent colour", "#8b7cf6").describe("Any CSS colour, e.g. #ff8fab."),
            SettingDef::text("background", "Background colour", "").describe("Leave empty to keep the theme's."),
            SettingDef::number("radius", "Corner radius (px)", 10.0, 0.0, 24.0),
        ]
    }

    fn css(&self, ctx: &PluginContext) -> Option<String> {
        let mut vars = String::new();
        if let Some(c) = safe_color(ctx.str("accent")) {
            vars.push_str(&format!("--mg-accent:{c};"));
        }
        if let Some(c) = safe_color(ctx.str("background")) {
            vars.push_str(&format!("--mg-bg:{c};"));
        }
        vars.push_str(&format!("--mg-radius:{}px;", ctx.number("radius").clamp(0.0, 24.0)));
        Some(format!(":root{{{vars}}}"))
    }
}

ui_plugin!(
    CharCounter, "char-counter", "Character Counter", Chat,
    "Shows how many characters you've typed out of Telegram's 4096 limit.",
    "Vencord: CharacterCounter", true, "char-counter.js", [Permission::Ui],
    [SettingDef::number("show_after", "Only show after this many characters", 0.0, 0.0, 4096.0)]
);

ui_plugin!(
    PreciseTimestamps, "precise-timestamps", "Precise Timestamps", Appearance,
    "Choose how message times are shown, down to the second.",
    "Vencord: DontRoundMyTimestamps / ReplyTimestamp", false, "precise-timestamps.js", [Permission::Ui],
    [SettingDef::select("format", "Format", "seconds", &[
        ("seconds", "14:03:27"),
        ("datetime", "2026-11-26 14:03"),
        ("relative", "5 minutes ago"),
    ])]
);

ui_plugin!(
    PinChats, "pin-chats", "Local Pins", Profiles,
    "Pin chats to the top of your list on this device only, beyond Telegram's own pin limit.",
    "Vencord: PinDMs", false, "pin-chats.js", [Permission::Ui],
    [SettingDef::list("pinned", "Pinned chat ids", &[])]
);

ui_plugin!(
    ViewRaw, "view-raw", "View Raw", Developer,
    "Adds “View raw” to the message menu, showing the message exactly as the client sees it.",
    "Vencord: ViewRaw", false, "view-raw.js", [Permission::Ui, Permission::ReadMessages],
    []
);

ui_plugin!(
    ReadAll, "read-all", "Read All", Notifications,
    "Adds a button that marks every chat as read.",
    "Vencord: ReadAllNotificationsButton", false, "read-all.js", [Permission::Ui],
    []
);

ui_plugin!(
    SendLatency, "send-latency", "Send Latency", Developer,
    "Shows how long Telegram took to accept each message you send.",
    "Vencord: MessageLatency", false, "send-latency.js", [Permission::Ui],
    [SettingDef::number("warn_ms", "Highlight when slower than (ms)", 1500.0, 100.0, 60000.0)]
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_formats() {
        assert_eq!(NameDisplay::format("name-username", Some("Mel"), Some("mel")).as_deref(), Some("Mel (@mel)"));
        assert_eq!(NameDisplay::format("username", Some("Mel"), None).as_deref(), Some("Mel"));
        assert_eq!(NameDisplay::format("username-name", None, Some("mel")).as_deref(), Some("@mel"));
        assert_eq!(NameDisplay::format("name", None, None), None);
    }

    #[test]
    fn client_colors_rejects_css_injection() {
        let p = ClientColors;
        let mut ctx = PluginContext::with_defaults(&p);
        ctx.set("accent", serde_json::json!("red;} body{display:none"));
        let css = p.css(&ctx).unwrap();
        assert!(!css.contains("display"));
        ctx.set("accent", serde_json::json!("rgb(1, 2, 3)"));
        assert!(p.css(&ctx).unwrap().contains("--mg-accent:rgb(1, 2, 3);"));
    }
}
