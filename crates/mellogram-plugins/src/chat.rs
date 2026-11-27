//! Composer and outgoing-message plugins.

use std::sync::OnceLock;

use mellogram_core::{Permission, 
    Category, CommandOutcome, OutgoingMessage, Plugin, PluginContext, PluginManifest, SendDecision,
    SettingDef,
};
use regex::Regex;
use url::Url;

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"https?://[^\s<>"'`]+"#).unwrap())
}

// ---------------------------------------------------------------------------

/// Strips tracking parameters from links before sending.
pub struct LinkScrubber;

const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "twclid", "ttclid",
    "igshid", "igsh", "mc_cid", "mc_eid", "_hsenc", "_hsmi", "mkt_tok", "ref_src", "ref_url",
    "vero_id", "oly_enc_id", "oly_anon_id", "rb_clickid", "s_cid", "spm",
];

/// Parameters only stripped on hosts where they are known to be trackers.
const HOST_PARAMS: &[(&str, &[&str])] = &[
    ("youtube.com", &["si", "pp", "feature"]),
    ("youtu.be", &["si", "feature"]),
    ("spotify.com", &["si", "context"]),
    ("twitter.com", &["s", "t"]),
    ("x.com", &["s", "t"]),
    ("instagram.com", &["igsh", "img_index"]),
    ("tiktok.com", &["_r", "_t", "is_from_webapp", "sender_device"]),
    ("amazon.", &["ref", "psc", "pd_rd_w", "pd_rd_r", "pf_rd_p", "pf_rd_r", "content-id"]),
];

impl LinkScrubber {
    pub fn scrub(url: &str, extra: &[&str]) -> String {
        // Keep trailing punctuation that belongs to the sentence, not the URL.
        let trimmed = url.trim_end_matches(['.', ',', ')', '!', '?', ';', ':']);
        let tail = &url[trimmed.len()..];
        let Ok(mut parsed) = Url::parse(trimmed) else {
            return url.to_owned();
        };
        if parsed.query().is_none() {
            return url.to_owned();
        }
        let host = parsed.host_str().unwrap_or("").trim_start_matches("www.").to_owned();
        let host_params: Vec<&str> = HOST_PARAMS
            .iter()
            .filter(|(h, _)| host == *h || host.ends_with(&format!(".{h}")) || (h.ends_with('.') && host.contains(h)))
            .flat_map(|(_, p)| p.iter().copied())
            .collect();
        let is_tracker = |k: &str| {
            let k = k.to_ascii_lowercase();
            k.starts_with("utm_")
                || TRACKING_PARAMS.contains(&k.as_str())
                || host_params.contains(&k.as_str())
                || extra.iter().any(|e| e.eq_ignore_ascii_case(&k))
        };
        let kept: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !is_tracker(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            parsed.set_query(None);
        } else {
            parsed.query_pairs_mut().clear().extend_pairs(kept);
        }
        format!("{parsed}{tail}")
    }
}

impl Plugin for LinkScrubber {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Removes tracking parameters (utm_*, fbclid, si, …) from links you send.",
            enabled_by_default: true,
            inspired_by: Some("Vencord: ClearURLs"),
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("link-scrubber", "Link Scrubber", Category::Privacy)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::list("extra_params", "Extra parameters to remove", &[])
            .describe("Query parameter names stripped from every link.")]
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        let extra = ctx.list("extra_params");
        msg.text = url_regex()
            .replace_all(&msg.text, |c: &regex::Captures| Self::scrub(&c[0], &extra))
            .into_owned();
        SendDecision::Continue
    }
}

// ---------------------------------------------------------------------------

/// User-defined find/replace rules applied to outgoing text.
pub struct TextReplace;

impl TextReplace {
    /// Rule syntax: `find => replace`, or `re:pattern => replacement` for a
    /// regular expression (`$1` style groups). Invalid rules are skipped.
    pub fn apply(text: &str, rules: &[&str]) -> String {
        let mut out = text.to_owned();
        for rule in rules {
            let Some((find, replace)) = rule.split_once("=>") else { continue };
            let (find, replace) = (find.trim(), replace.trim());
            if find.is_empty() {
                continue;
            }
            if let Some(pattern) = find.strip_prefix("re:") {
                if let Ok(re) = Regex::new(pattern) {
                    out = re.replace_all(&out, replace).into_owned();
                }
            } else {
                out = out.replace(find, replace);
            }
        }
        out
    }
}

impl Plugin for TextReplace {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Automatically replaces text in messages you send using your own rules.",
            inspired_by: Some("Vencord: TextReplace"),
            priority: -5,
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("text-replace", "Text Replace", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::list("rules", "Rules", &["teh => the"])
            .describe("One per line: `find => replace`, or `re:regex => replacement`.")]
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        msg.text = Self::apply(&msg.text, &ctx.list("rules"));
        SendDecision::Continue
    }
}

// ---------------------------------------------------------------------------

/// Text-snippet slash commands, built-in and user-defined.
pub struct QuickCommands;

const BUILTIN_SNIPPETS: &[(&str, &str, &str)] = &[
    ("shrug", "¯\\_(ツ)_/¯", "Appends a shrug"),
    ("tableflip", "(╯°□°)╯︵ ┻━┻", "Appends a table flip"),
    ("unflip", "┬─┬ ノ( ゜-゜ノ)", "Puts the table back"),
    ("lenny", "( ͡° ͜ʖ ͡°)", "Appends a lenny face"),
];

impl QuickCommands {
    fn custom<'a>(ctx: &'a PluginContext) -> impl Iterator<Item = (&'a str, &'a str)> {
        ctx.list("custom")
            .into_iter()
            .filter_map(|r| r.split_once("=>"))
            .map(|(n, t)| (n.trim().trim_start_matches('/'), t.trim()))
            .filter(|(n, _)| !n.is_empty())
    }
}

impl Plugin for QuickCommands {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Adds /shrug, /tableflip, /lenny and your own text-snippet commands.",
            enabled_by_default: true,
            inspired_by: Some("Vencord: CustomCommands"),
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("quick-commands", "Quick Commands", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::list("custom", "Custom commands", &["sig => — sent from MelloGram"])
            .describe("One per line: `name => text`. Typing `/name extra` sends `extra text`.")]
    }

    fn commands(&self, ctx: &PluginContext) -> Vec<(String, String)> {
        BUILTIN_SNIPPETS
            .iter()
            .map(|(n, _, d)| ((*n).to_owned(), (*d).to_owned()))
            .chain(Self::custom(ctx).map(|(n, t)| (n.to_owned(), format!("Inserts “{t}”"))))
            .collect()
    }

    fn run_command(&self, ctx: &PluginContext, name: &str, msg: &mut OutgoingMessage) -> CommandOutcome {
        let snippet = Self::custom(ctx)
            .find(|(n, _)| *n == name)
            .map(|(_, t)| t.to_owned())
            .or_else(|| BUILTIN_SNIPPETS.iter().find(|(n, ..)| *n == name).map(|(_, t, _)| (*t).to_owned()));
        match snippet {
            Some(s) if msg.text.is_empty() => {
                msg.text = s;
                CommandOutcome::Send
            }
            Some(s) => {
                msg.text = format!("{} {s}", msg.text);
                CommandOutcome::Send
            }
            None => CommandOutcome::NotHandled,
        }
    }
}

// ---------------------------------------------------------------------------

/// Removes common leading indentation inside ``` code blocks.
pub struct Unindent;

impl Unindent {
    pub fn apply(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("```") {
            let (before, after) = rest.split_at(start + 3);
            out.push_str(before);
            let Some(end) = after.find("```") else {
                rest = after;
                break;
            };
            out.push_str(&dedent(&after[..end]));
            out.push_str("```");
            rest = &after[end + 3..];
        }
        out.push_str(rest);
        out
    }
}

fn dedent(block: &str) -> String {
    // First line holds the optional language tag; leave it alone.
    let (head, body) = block.split_once('\n').unwrap_or(("", block));
    let indent = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let body: Vec<&str> = body.lines().map(|l| if l.len() >= indent { &l[indent..] } else { l.trim_start() }).collect();
    let mut out = format!("{head}\n{}", body.join("\n"));
    if block.ends_with('\n') {
        out.push('\n');
    }
    out
}

impl Plugin for Unindent {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Trims shared indentation from code blocks you paste and send.",
            enabled_by_default: true,
            inspired_by: Some("Vencord: Unindent"),
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("unindent", "Unindent", Category::Chat)
        }
    }

    fn before_send(&self, _: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        msg.text = Self::apply(&msg.text);
        SendDecision::Continue
    }
}

// ---------------------------------------------------------------------------

/// Send messages without a notification: `/silent text`, a composer toggle,
/// or always.
pub struct SilentSend;

impl Plugin for SilentSend {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Adds a bell toggle and /silent to send messages without a notification sound.",
            enabled_by_default: true,
            inspired_by: Some("Vencord: SilentMessageToggle"),
            permissions: &[Permission::ModifyOutgoing, Permission::Ui],
            ..PluginManifest::new("silent-send", "Silent Send", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::boolean("always", "Always send silently", false),
            SettingDef::boolean("persist", "Keep the toggle on after sending", false),
        ]
    }

    fn commands(&self, _: &PluginContext) -> Vec<(String, String)> {
        vec![("silent".into(), "Send this message without a notification".into())]
    }

    fn run_command(&self, _: &PluginContext, name: &str, msg: &mut OutgoingMessage) -> CommandOutcome {
        if name != "silent" {
            return CommandOutcome::NotHandled;
        }
        msg.silent = true;
        CommandOutcome::Send
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        msg.silent |= ctx.bool("always");
        SendDecision::Continue
    }

    fn ui_script(&self) -> Option<&'static str> {
        Some(include_str!("../ui/silent-send.js"))
    }
}

// ---------------------------------------------------------------------------

/// Controls Telegram's automatic link previews.
pub struct LinkPreviews;

impl Plugin for LinkPreviews {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Choose whether links you send get a preview card. Wrap a link in <…> to skip its preview.",
            inspired_by: Some("Vencord: UnsuppressEmbeds (inverted for Telegram)"),
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("link-previews", "Link Preview Control", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::select(
            "mode",
            "Link previews",
            "angle",
            &[("always", "Always"), ("angle", "Unless the link is wrapped in <…>"), ("never", "Never")],
        )]
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        match ctx.str("mode") {
            "never" => msg.link_preview = false,
            "angle" => {
                static RE: OnceLock<Regex> = OnceLock::new();
                let re = RE.get_or_init(|| Regex::new(r"<(https?://[^\s>]+)>").unwrap());
                if re.is_match(&msg.text) {
                    msg.link_preview = false;
                    msg.text = re.replace_all(&msg.text, "$1").into_owned();
                }
            }
            _ => {}
        }
        SendDecision::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(p: &dyn Plugin, ctx: &PluginContext, text: &str) -> OutgoingMessage {
        let mut m = OutgoingMessage::text(1, text);
        assert_eq!(p.before_send(ctx, &mut m), SendDecision::Continue);
        m
    }

    #[test]
    fn scrubs_tracking_params_only() {
        let p = LinkScrubber;
        let ctx = PluginContext::with_defaults(&p);
        let m = send(
            &p,
            &ctx,
            "look https://example.com/a?utm_source=x&id=7&fbclid=abc. and https://youtu.be/xyz?si=TRACK&t=42",
        );
        assert_eq!(m.text, "look https://example.com/a?id=7. and https://youtu.be/xyz?t=42");
        // `si` is only a tracker on known hosts.
        assert_eq!(LinkScrubber::scrub("https://example.org/?si=1", &[]), "https://example.org/?si=1");
        assert_eq!(LinkScrubber::scrub("https://example.org/?utm_medium=1", &[]), "https://example.org/");
        assert_eq!(LinkScrubber::scrub("https://example.org/?q=1&ref=2", &["ref"]), "https://example.org/?q=1");
    }

    #[test]
    fn text_replace_rules() {
        let rules = ["teh => the", "re:(\\d+)c => $1°C", "broken", "re:([ => x"];
        assert_eq!(TextReplace::apply("teh temp is 20c", &rules), "the temp is 20°C");
    }

    #[test]
    fn quick_commands() {
        let p = QuickCommands;
        let ctx = PluginContext::with_defaults(&p);
        let mut m = OutgoingMessage::text(1, "oh well");
        assert_eq!(p.run_command(&ctx, "shrug", &mut m), CommandOutcome::Send);
        assert_eq!(m.text, "oh well ¯\\_(ツ)_/¯");
        let mut m = OutgoingMessage::text(1, "");
        p.run_command(&ctx, "sig", &mut m);
        assert_eq!(m.text, "— sent from MelloGram");
        assert!(p.commands(&ctx).iter().any(|(n, _)| n == "sig"));
        assert_eq!(p.run_command(&ctx, "nope", &mut m), CommandOutcome::NotHandled);
    }

    #[test]
    fn unindents_code_blocks() {
        let input = "see:\n```rust\n    fn a() {\n        b();\n    }\n```\n  kept indent";
        assert_eq!(Unindent::apply(input), "see:\n```rust\nfn a() {\n    b();\n}\n```\n  kept indent");
        assert_eq!(Unindent::apply("no blocks"), "no blocks");
        assert_eq!(Unindent::apply("``` unterminated"), "``` unterminated");
    }

    #[test]
    fn link_preview_modes() {
        let p = LinkPreviews;
        let mut ctx = PluginContext::with_defaults(&p);
        let m = send(&p, &ctx, "x <https://a.b/c> y");
        assert!(!m.link_preview);
        assert_eq!(m.text, "x https://a.b/c y");
        assert!(send(&p, &ctx, "https://a.b").link_preview);
        ctx.set("mode", serde_json::json!("never"));
        assert!(!send(&p, &ctx, "https://a.b").link_preview);
    }

    #[test]
    fn silent_send() {
        let p = SilentSend;
        let mut ctx = PluginContext::with_defaults(&p);
        let mut m = OutgoingMessage::text(1, "hi");
        assert_eq!(p.run_command(&ctx, "silent", &mut m), CommandOutcome::Send);
        assert!(m.silent);
        ctx.set("always", serde_json::json!(true));
        assert!(send(&p, &ctx, "x").silent);
    }
}
