//! Productivity, security and local-privacy plugins.

use std::sync::OnceLock;

use mellogram_core::appearance::safe_color;
use mellogram_core::{
    Category, CommandOutcome, IncomingMessage, OutgoingMessage, Permission, Plugin, PluginContext,
    PluginManifest, SendDecision, SettingDef,
};
use regex::Regex;

// ---------------------------------------------------------------------------

/// Colours messages that contain keywords you care about.
pub struct MessageHighlight;

impl MessageHighlight {
    /// Returns the colour of the first matching rule.
    pub fn matching_color(text: &str, rules: &[&str], default: &str) -> Option<String> {
        let lower = text.to_lowercase();
        rules.iter().find_map(|rule| {
            let (word, color) = match rule.split_once("=>") {
                Some((w, c)) => (w.trim(), c.trim()),
                None => (rule.trim(), default),
            };
            if word.is_empty() || !lower.contains(&word.to_lowercase()) {
                return None;
            }
            Some(safe_color(color).or(safe_color(default)).unwrap_or("var(--mg-accent)").to_owned())
        })
    }
}

impl Plugin for MessageHighlight {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Highlights messages containing your keywords, each with its own colour.",
            inspired_by: Some("Spec: MessageHighlight / keyword colouring"),
            permissions: &[Permission::ReadMessages, Permission::Styles],
            ..PluginManifest::new("message-highlight", "Message Highlight", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::list("rules", "Keywords", &["urgent => #ef5b6a", "release"])
                .describe("One per line: `keyword` or `keyword => colour`. Case-insensitive."),
            SettingDef::text("default_color", "Default colour", "#f2b84b"),
            SettingDef::boolean("ignore_own", "Ignore your own messages", true),
        ]
    }

    fn on_message(&self, ctx: &PluginContext, msg: &mut IncomingMessage) {
        if msg.outgoing && ctx.bool("ignore_own") {
            return;
        }
        if let Some(color) = Self::matching_color(&msg.text, &ctx.list("rules"), ctx.str("default_color")) {
            msg.highlight = Some(color);
            msg.add_class("mg-highlighted");
        }
    }

    fn on_edit(&self, ctx: &PluginContext, _: Option<&IncomingMessage>, msg: &mut IncomingMessage) {
        self.on_message(ctx, msg);
    }

    fn css(&self, _: &PluginContext) -> Option<String> {
        Some(".mg-message.mg-highlighted { box-shadow: inset 3px 0 0 var(--mg-hl, var(--mg-accent)), 0 0 0 1px color-mix(in srgb, var(--mg-hl, var(--mg-accent)) 45%, transparent); }".into())
    }
}

// ---------------------------------------------------------------------------

/// Stops you from accidentally pasting secrets into a chat.
pub struct SecretGuard;

/// Prefix that sends anyway.
pub const SECRET_BYPASS: &str = "!! ";

fn secret_patterns() -> &'static [(&'static str, Regex)] {
    static P: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    P.get_or_init(|| {
        [
            ("a private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
            ("an AWS access key", r"\b(AKIA|ASIA)[0-9A-Z]{16}\b"),
            ("a GitHub token", r"\b(gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{60,})\b"),
            ("a Slack token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
            ("a Stripe secret key", r"\b[sr]k_live_[A-Za-z0-9]{20,}"),
            ("a Telegram bot token", r"\b\d{8,10}:AA[A-Za-z0-9_-]{33}\b"),
            ("a JSON Web Token", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
            ("an API key", r"\bsk-(proj-|ant-)?[A-Za-z0-9_-]{32,}"),
        ]
        .into_iter()
        .map(|(n, p)| (n, Regex::new(p).unwrap()))
        .collect()
    })
}

fn luhn_card(text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap());
    re.find_iter(text).any(|m| {
        let digits: Vec<u32> = m.as_str().chars().filter_map(|c| c.to_digit(10)).collect();
        if !(13..=19).contains(&digits.len()) || digits.iter().all(|d| *d == digits[0]) {
            return false;
        }
        let sum: u32 = digits
            .iter()
            .rev()
            .enumerate()
            .map(|(i, &d)| if i % 2 == 1 { let x = d * 2; if x > 9 { x - 9 } else { x } } else { d })
            .sum();
        sum % 10 == 0
    })
}

impl SecretGuard {
    pub fn detect(text: &str, check_cards: bool) -> Option<&'static str> {
        secret_patterns()
            .iter()
            .find(|(_, re)| re.is_match(text))
            .map(|(name, _)| *name)
            .or_else(|| (check_cards && luhn_card(text)).then_some("a payment card number"))
    }
}

impl Plugin for SecretGuard {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Blocks messages that look like passwords, API keys, private keys or card numbers. Start a message with “!! ” to send anyway.",
            inspired_by: Some("Spec: ClipboardWarning"),
            enabled_by_default: true,
            priority: 100, // run last, on the final text
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("secret-guard", "Secret Guard", Category::Privacy)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::boolean("cards", "Check for payment card numbers", true)]
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        if let Some(rest) = msg.text.strip_prefix(SECRET_BYPASS) {
            msg.text = rest.to_owned();
            return SendDecision::Continue;
        }
        match Self::detect(&msg.text, ctx.bool("cards")) {
            Some(what) => SendDecision::Cancel(format!(
                "Not sent: this looks like {what}. Start the message with “!! ” if you really mean to share it."
            )),
            None => SendDecision::Continue,
        }
    }
}

// ---------------------------------------------------------------------------

/// `/calc 2 * (3 + 4)` → `2 * (3 + 4) = 14`
pub struct Calculator;

/// Small recursive-descent evaluator: + - * / % ^, parentheses, unary minus,
/// and sqrt/abs/round/floor/ceil. No variables, no allocation-heavy parsing.
pub fn evaluate(expr: &str) -> Result<f64, String> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        depth: u32,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
        }
        fn eat(&mut self, c: u8) -> bool {
            self.ws();
            if self.s.get(self.i) == Some(&c) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn expr(&mut self) -> Result<f64, String> {
            let mut v = self.term()?;
            loop {
                if self.eat(b'+') { v += self.term()?; }
                else if self.eat(b'-') { v -= self.term()?; }
                else { return Ok(v); }
            }
        }
        fn term(&mut self) -> Result<f64, String> {
            let mut v = self.power()?;
            loop {
                if self.eat(b'*') || self.eat(b'x') { v *= self.power()?; }
                else if self.eat(b'/') {
                    let d = self.power()?;
                    if d == 0.0 { return Err("division by zero".into()); }
                    v /= d;
                }
                else if self.eat(b'%') { v %= self.power()?; }
                else { return Ok(v); }
            }
        }
        fn power(&mut self) -> Result<f64, String> {
            let base = self.unary()?;
            if self.eat(b'^') { Ok(base.powf(self.power()?)) } else { Ok(base) }
        }
        fn unary(&mut self) -> Result<f64, String> {
            if self.eat(b'-') { return Ok(-self.unary()?); }
            if self.eat(b'+') { return self.unary(); }
            self.atom()
        }
        fn atom(&mut self) -> Result<f64, String> {
            self.depth += 1;
            if self.depth > 64 { return Err("too deeply nested".into()); }
            self.ws();
            let r = if self.eat(b'(') {
                let v = self.expr()?;
                if !self.eat(b')') { return Err("missing )".into()); }
                Ok(v)
            } else if self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
                let start = self.i;
                while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) { self.i += 1; }
                let name = std::str::from_utf8(&self.s[start..self.i]).unwrap().to_ascii_lowercase();
                if name == "pi" { return Ok(std::f64::consts::PI); }
                if !self.eat(b'(') { return Err(format!("unknown name `{name}`")); }
                let arg = self.expr()?;
                if !self.eat(b')') { return Err("missing )".into()); }
                match name.as_str() {
                    "sqrt" => Ok(arg.sqrt()),
                    "abs" => Ok(arg.abs()),
                    "round" => Ok(arg.round()),
                    "floor" => Ok(arg.floor()),
                    "ceil" => Ok(arg.ceil()),
                    _ => Err(format!("unknown function `{name}`")),
                }
            } else {
                let start = self.i;
                while self.s.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == b'.' || *c == b'_' || *c == b',') { self.i += 1; }
                let raw: String = std::str::from_utf8(&self.s[start..self.i]).unwrap().chars().filter(|c| *c != '_' && *c != ',').collect();
                raw.parse::<f64>().map_err(|_| "expected a number".to_string())
            };
            self.depth -= 1;
            r
        }
    }
    let mut p = P { s: expr.as_bytes(), i: 0, depth: 0 };
    let v = p.expr()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("unexpected `{}`", &expr[p.i..]));
    }
    if !v.is_finite() {
        return Err("result is not a finite number".into());
    }
    Ok(v)
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.10}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

impl Plugin for Calculator {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Adds /calc: type an expression and send it with its result.",
            inspired_by: Some("Spec: QuickCalculator"),
            enabled_by_default: true,
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("calculator", "Calculator", Category::Utility)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::boolean("result_only", "Send only the result", false)]
    }

    fn commands(&self, _: &PluginContext) -> Vec<(String, String)> {
        vec![("calc".into(), "Calculate, e.g. /calc 12.5 * 4 + sqrt(9)".into())]
    }

    fn run_command(&self, ctx: &PluginContext, name: &str, msg: &mut OutgoingMessage) -> CommandOutcome {
        if name != "calc" {
            return CommandOutcome::NotHandled;
        }
        match evaluate(&msg.text) {
            Ok(v) if ctx.bool("result_only") => {
                msg.text = format_number(v);
                CommandOutcome::Send
            }
            Ok(v) => {
                msg.text = format!("{} = {}", msg.text.trim(), format_number(v));
                CommandOutcome::Send
            }
            Err(e) => CommandOutcome::Consumed(Some(format!("Can't calculate that: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------

/// Local display privacy: blur or hide things on *your* screen.
pub struct PrivacyMode;

impl Plugin for PrivacyMode {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Blur chat previews, names, avatars or messages on your screen, plus a privacy screen when the window loses focus. Ctrl+Shift+H toggles a full blur.",
            inspired_by: Some("Spec: LocalPrivacyMode / HideChatPreviews / HideAvatars / ScreenshotPrivacyOverlay"),
            permissions: &[Permission::Styles, Permission::Ui],
            ..PluginManifest::new("privacy-mode", "Privacy Mode", Category::Privacy)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::boolean("blur_previews", "Blur chat-list previews", true),
            SettingDef::boolean("blur_names", "Blur names", false),
            SettingDef::boolean("hide_avatars", "Hide avatars", false),
            SettingDef::boolean("blur_messages", "Blur messages until hovered", false),
            SettingDef::boolean("blur_media", "Blur media until hovered", true),
            SettingDef::boolean("privacy_screen", "Blur everything when the window loses focus", false),
            SettingDef::number("blur_px", "Blur strength (px)", 6.0, 2.0, 20.0),
        ]
    }

    fn css(&self, ctx: &PluginContext) -> Option<String> {
        let px = ctx.number("blur_px").clamp(2.0, 20.0);
        let blur = format!("filter: blur({px}px); transition: filter .15s;");
        let mut css = format!(
            "body.mg-privacy-screen .mg-app {{ filter: blur(14px) grayscale(.4); }}\n\
             body.mg-panic .mg-app {{ filter: blur(20px); }}\n"
        );
        let mut rule = |on: bool, sel: &str, hover: &str| {
            if on {
                css.push_str(&format!("{sel} {{ {blur} }}\n{hover} {{ filter: none; }}\n"));
            }
        };
        rule(ctx.bool("blur_previews"), ".mg-chat-last", ".mg-chat-item:hover .mg-chat-last");
        rule(ctx.bool("blur_names"), ".mg-chat-name, .mg-sender, .mg-chat-title", ".mg-chat-item:hover .mg-chat-name, .mg-message:hover .mg-sender, .mg-chat-title:hover");
        rule(ctx.bool("blur_messages"), ".mg-message .mg-text", ".mg-message:hover .mg-text");
        rule(ctx.bool("blur_media"), ".mg-media", ".mg-message:hover .mg-media");
        if ctx.bool("hide_avatars") {
            css.push_str(".mg-avatar { visibility: hidden; width: 0; margin: 0; }\n");
        }
        Some(css)
    }

    fn ui_script(&self) -> Option<&'static str> {
        Some(include_str!("../ui/privacy-mode.js"))
    }
}

// ---------------------------------------------------------------------------

ui_plugin!(
    ChatColorizer, "chat-colorizer", "Chat Colorizer", Appearance,
    "Gives every person a consistent name colour, with your own overrides per contact.",
    "Vencord: IrcColors / Spec: ChatColorizer", false, "chat-colorizer.js",
    [Permission::Ui],
    [
        SettingDef::number("saturation", "Saturation (%)", 65.0, 20.0, 100.0),
        SettingDef::number("lightness", "Lightness (%)", 68.0, 30.0, 85.0),
        SettingDef::list("overrides", "Per-contact colours", &[]).describe("One per line: `user id => colour`."),
        SettingDef::boolean("color_bubbles", "Tint message bubbles too", false),
    ]
);

ui_plugin!(
    QuickNotes, "quick-notes", "Quick Notes", Productivity,
    "A private notes panel for each chat, kept only on this device.",
    "Spec: QuickNotes / ContactNotes", false, "quick-notes.js",
    [Permission::Ui, Permission::Storage],
    []
);

ui_plugin!(
    Bookmarks, "bookmarks", "Bookmarks", Productivity,
    "Bookmark messages and jump back to them from any chat.",
    "Spec: MessageBookmarks / ConversationBookmarks", false, "bookmarks.js",
    [Permission::Ui, Permission::Storage, Permission::ReadMessages],
    []
);

ui_plugin!(
    MessageTemplates, "message-templates", "Message Templates", Productivity,
    "Reusable message templates with {date}, {time}, {chat} and {cursor} placeholders.",
    "Spec: MessageTemplates / CustomSnippets", false, "message-templates.js",
    [Permission::Ui],
    [SettingDef::list("templates", "Templates", &[
        "Greeting => Hi! Hope you're having a good {weekday}.",
        "Away => I'm away right now and will reply after {time}.",
        "Meeting => Meeting notes ({date}):\n- {cursor}",
    ]).describe("One per line: `name => text`. Use \\n for new lines.")]
);

ui_plugin!(
    ChatStatistics, "chat-stats", "Chat Statistics", Productivity,
    "Statistics for the loaded part of a chat: messages per person, busiest hours, top words.",
    "Spec: MessageStatistics / GroupActivityDashboard", false, "chat-stats.js",
    [Permission::Ui, Permission::ReadMessages],
    []
);

ui_plugin!(
    ChatExport, "chat-export", "Chat Export", Productivity,
    "Export the loaded messages of a chat as text, Markdown or JSON.",
    "Spec: MessageExport", false, "chat-export.js",
    [Permission::Ui, Permission::ReadMessages],
    []
);

ui_plugin!(
    LinkCollection, "link-collection", "Link Collection", Productivity,
    "Lists every link shared in the loaded part of a chat.",
    "Spec: LinkCollection / ReadingList", false, "link-collection.js",
    [Permission::Ui, Permission::ReadMessages],
    []
);

ui_plugin!(
    CopyEnhancer, "copy-enhancer", "Copy Enhancer", Chat,
    "Extra copy options: as a quote, with sender and time, or as Markdown.",
    "Spec: MessageCopyEnhancer", false, "copy-enhancer.js",
    [Permission::Ui, Permission::Clipboard],
    []
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rules() {
        let rules = ["urgent => red", "release", "bad => url(x)"];
        assert_eq!(MessageHighlight::matching_color("This is URGENT", &rules, "gold").as_deref(), Some("red"));
        assert_eq!(MessageHighlight::matching_color("new release out", &rules, "gold").as_deref(), Some("gold"));
        assert_eq!(MessageHighlight::matching_color("bad things", &rules, "gold").as_deref(), Some("gold"));
        assert_eq!(MessageHighlight::matching_color("nothing", &rules, "gold"), None);

        let p = MessageHighlight;
        let ctx = PluginContext::with_defaults(&p);
        let mut m = IncomingMessage { text: "URGENT: deploy".into(), ..Default::default() };
        p.on_message(&ctx, &mut m);
        assert_eq!(m.highlight.as_deref(), Some("#ef5b6a"));
        assert_eq!(m.classes, vec!["mg-highlighted"]);
    }

    #[test]
    fn secret_guard() {
        assert_eq!(SecretGuard::detect("key AKIAIOSFODNN7EXAMPLE ok", false), Some("an AWS access key"));
        assert_eq!(SecretGuard::detect("-----BEGIN OPENSSH PRIVATE KEY-----", false), Some("a private key"));
        assert_eq!(SecretGuard::detect("card 4111 1111 1111 1111", true), Some("a payment card number"));
        assert_eq!(SecretGuard::detect("card 4111 1111 1111 1112", true), None);
        assert_eq!(SecretGuard::detect("call me at 5551234567", true), None);
        assert_eq!(SecretGuard::detect("just a normal message", true), None);

        let p = SecretGuard;
        let ctx = PluginContext::with_defaults(&p);
        let mut m = OutgoingMessage::text(1, "ghp_abcdefghijklmnopqrstuvwxyz0123456789AB");
        assert!(matches!(p.before_send(&ctx, &mut m), SendDecision::Cancel(_)));
        let mut m = OutgoingMessage::text(1, "!! ghp_abcdefghijklmnopqrstuvwxyz0123456789AB");
        assert_eq!(p.before_send(&ctx, &mut m), SendDecision::Continue);
        assert!(m.text.starts_with("ghp_"));
    }

    #[test]
    fn calculator() {
        assert_eq!(evaluate("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(evaluate("(2 + 3) * 4").unwrap(), 20.0);
        assert_eq!(evaluate("-2 ^ 2").unwrap(), 4.0); // unary minus binds tighter, like most calculators
        assert_eq!(evaluate("2^3^2").unwrap(), 512.0);
        assert_eq!(evaluate("sqrt(16) + abs(-2)").unwrap(), 6.0);
        assert_eq!(evaluate("1,000 * 3").unwrap(), 3000.0);
        assert!(evaluate("1/0").is_err());
        assert!(evaluate("2 +").is_err());
        assert!(evaluate("rm -rf").is_err());
        assert!(evaluate(&"(".repeat(200)).is_err());
        assert_eq!(format_number(0.1 + 0.2), "0.3");

        let p = Calculator;
        let ctx = PluginContext::with_defaults(&p);
        let mut m = OutgoingMessage::text(1, "12.5 * 4");
        assert_eq!(p.run_command(&ctx, "calc", &mut m), CommandOutcome::Send);
        assert_eq!(m.text, "12.5 * 4 = 50");
        let mut m = OutgoingMessage::text(1, "2 +");
        assert!(matches!(p.run_command(&ctx, "calc", &mut m), CommandOutcome::Consumed(Some(_))));
    }

    #[test]
    fn privacy_css() {
        let p = PrivacyMode;
        let mut ctx = PluginContext::with_defaults(&p);
        ctx.set("blur_px", serde_json::json!(999));
        let css = p.css(&ctx).unwrap();
        assert!(css.contains("blur(20px)") && css.contains(".mg-chat-last"));
    }
}

ui_plugin!(
    SendLater, "send-later", "Send Later", Productivity,
    "Schedule a message with Telegram's own scheduled messages, so it is delivered at the chosen time even if MelloGram is closed.",
    "Spec: messageScheduler", true, "send-later.js",
    [Permission::Ui, Permission::ModifyOutgoing],
    []
);

ui_plugin!(
    TrafficLog, "traffic-log", "Traffic Log", Privacy,
    "A live, redacted list of every network exchange MelloGram makes: direction, host or API method, and size. Never message text, numbers or tokens.",
    "Spec: traffic log", true, "traffic-log.js",
    [Permission::Ui],
    []
);

ui_plugin!(
    ActivityTracker, "activity-tracker", "Activity Tracker", Productivity,
    "Time spent in MelloGram per day and per chat, kept on this device, with optional WakaTime heartbeats (your key is stored sealed).",
    "Vencord: vencord-wakatime", false, "activity-tracker.js",
    [Permission::Ui, Permission::Storage, Permission::Network],
    [SettingDef::boolean("wakatime", "Send WakaTime heartbeats", false)]
);

ui_plugin!(
    GlobalBadges, "global-badges", "Global Badges", Appearance,
    "Shows the badges of people who publish a public MelloID profile next to their name, in every MelloGram install.",
    "Domis-Vencord-Plugins: GlobalBadges", true, "global-badges.js",
    [Permission::Ui, Permission::Network, Permission::ReadMessages],
    []
);
