//! Appearance Studio: structured visual settings compiled to CSS.
//!
//! Users get sliders, pickers and switches instead of writing CSS; the result
//! is ordinary CSS variables and rules layered *after* themes and *before*
//! plugin CSS and QuickCSS, so power users can still override anything.
//!
//! Every user-supplied value is validated before it reaches CSS, so a crafted
//! settings file or shared profile cannot inject arbitrary rules.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BubbleStyle {
    #[default]
    Rounded,
    Squared,
    Compact,
    Glass,
    Transparent,
    /// No bubbles; IRC-like lines.
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AvatarShape {
    #[default]
    Circle,
    Rounded,
    Square,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MessageAnimation {
    None,
    #[default]
    Fade,
    Slide,
    Pop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TimestampMode {
    #[default]
    Inline,
    Hover,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum QuoteStyle {
    #[default]
    Bar,
    Box,
    Minimal,
}

/// A colour fill: solid or a two-stop linear gradient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Fill {
    pub enabled: bool,
    pub from: String,
    pub to: String,
    /// Degrees; ignored when `to` is empty (solid fill).
    pub angle: u16,
}

impl Fill {
    fn css(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let from = safe_color(&self.from)?;
        Some(match safe_color(&self.to) {
            Some(to) => format!("linear-gradient({}deg, {from}, {to})", self.angle % 360),
            None => from.to_owned(),
        })
    }
}

/// Overrides for one chat (per-chat wallpaper and bubble colours).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChatAppearance {
    pub background: Fill,
    pub bubble_in: String,
    pub bubble_out: String,
    pub accent: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub enabled: bool,

    // Messages
    pub bubble_style: BubbleStyle,
    pub bubble_radius: u8,
    pub message_spacing: u8,
    pub max_bubble_width: u8,
    pub outgoing_fill: Fill,
    pub incoming_fill: Fill,
    pub timestamps: TimestampMode,
    pub quote_style: QuoteStyle,
    pub show_sender_names: bool,
    pub message_animation: MessageAnimation,

    // Chat
    pub chat_background: Fill,
    pub per_chat: BTreeMap<String, ChatAppearance>,

    // Typography
    pub font_family: String,
    pub mono_font_family: String,
    pub font_size: u8,
    pub line_height: u8,
    pub letter_spacing: i8,

    // Interface
    pub accent: String,
    pub density: Density,
    pub avatar_shape: AvatarShape,
    pub avatar_size: u8,
    pub chat_list_previews: bool,
    pub sidebar_width: u16,
    pub ui_scale: u16,
    pub reduced_motion: bool,
    pub glass_panels: bool,
    pub custom_scrollbars: bool,
    pub scrollbar_color: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            enabled: true,
            bubble_style: BubbleStyle::Rounded,
            bubble_radius: 10,
            message_spacing: 3,
            max_bubble_width: 72,
            outgoing_fill: Fill::default(),
            incoming_fill: Fill::default(),
            timestamps: TimestampMode::Inline,
            quote_style: QuoteStyle::Bar,
            show_sender_names: true,
            message_animation: MessageAnimation::Fade,
            chat_background: Fill::default(),
            per_chat: BTreeMap::new(),
            font_family: String::new(),
            mono_font_family: String::new(),
            font_size: 14,
            line_height: 145,
            letter_spacing: 0,
            accent: String::new(),
            density: Density::Normal,
            avatar_shape: AvatarShape::Circle,
            avatar_size: 42,
            chat_list_previews: true,
            sidebar_width: 320,
            ui_scale: 100,
            reduced_motion: false,
            glass_panels: false,
            custom_scrollbars: true,
            scrollbar_color: String::new(),
        }
    }
}

/// Accepts hex, rgb()/hsl() and named colours; rejects anything that could
/// close a declaration or a block.
pub fn safe_color(v: &str) -> Option<&str> {
    let v = v.trim();
    let ok = !v.is_empty()
        && v.len() <= 64
        && v.chars().all(|c| c.is_ascii_alphanumeric() || "#(),.% -/".contains(c))
        && !v.contains("url")
        && !v.contains("expression");
    ok.then_some(v)
}

/// Font stacks: letters, digits, spaces, commas, hyphens and quotes only.
pub fn safe_font(v: &str) -> Option<String> {
    let v = v.trim();
    let ok = !v.is_empty() && v.len() <= 200 && v.chars().all(|c| c.is_alphanumeric() || " ,-_'\"".contains(c));
    ok.then(|| v.replace('"', "'"))
}

fn chat_selector(id: &str) -> Option<String> {
    let ok = !id.is_empty() && id.len() <= 24 && id.chars().all(|c| c.is_ascii_digit() || c == '-');
    ok.then(|| format!(".mg-chat[data-chat-id=\"{id}\"]"))
}

impl Appearance {
    /// Compiles the settings to a stylesheet. Empty when disabled.
    pub fn to_css(&self) -> String {
        if !self.enabled {
            return String::new();
        }
        let mut vars = String::new();
        let mut rules = String::new();
        let mut var = |name: &str, value: String| {
            let _ = writeln!(vars, "  {name}: {value};");
        };

        var("--mg-radius", format!("{}px", self.bubble_radius.min(32)));
        var("--mg-message-gap", format!("{}px", self.message_spacing.min(24)));
        var("--mg-bubble-max", format!("{}%", self.max_bubble_width.clamp(40, 100)));
        var(
            "--mg-density",
            match self.density {
                Density::Compact => "0.65",
                Density::Normal => "1",
                Density::Comfortable => "1.4",
            }
            .into(),
        );
        var("--mg-avatar", format!("{}px", self.avatar_size.clamp(20, 72)));
        var("--mg-font-size", format!("{}px", self.font_size.clamp(10, 24)));
        var("--mg-line-height", format!("{:.2}", f32::from(self.line_height.clamp(100, 220)) / 100.0));
        var("--mg-letter-spacing", format!("{:.2}px", f32::from(self.letter_spacing.clamp(-2, 4)) / 2.0));
        var("--mg-sidebar-width", format!("{}px", self.sidebar_width.clamp(200, 520)));
        var("--mg-ui-scale", format!("{:.2}", f32::from(self.ui_scale.clamp(70, 160)) / 100.0));
        if let Some(c) = safe_color(&self.accent) {
            var("--mg-accent", c.into());
        }
        if let Some(f) = safe_font(&self.font_family) {
            var("--mg-font", f);
        }
        if let Some(f) = safe_font(&self.mono_font_family) {
            var("--mg-mono", f);
        }
        if let Some(fill) = self.outgoing_fill.css() {
            var("--mg-bubble-out", fill);
        }
        if let Some(fill) = self.incoming_fill.css() {
            var("--mg-bubble-in", fill);
        }
        if let Some(fill) = self.chat_background.css() {
            var("--mg-chat-bg", fill);
        }
        if let Some(c) = safe_color(&self.scrollbar_color) {
            var("--mg-scrollbar", c.into());
        }

        rules.push_str(match self.bubble_style {
            BubbleStyle::Rounded => "",
            BubbleStyle::Squared => ".mg-message { border-radius: 3px; }\n",
            BubbleStyle::Compact => ".mg-message { padding: 2px 8px; border-radius: 6px; }\n",
            BubbleStyle::Glass => ".mg-message { backdrop-filter: blur(12px) saturate(1.4); background: color-mix(in srgb, var(--mg-bubble-in) 55%, transparent); border: 1px solid rgba(255,255,255,.08); }\n.mg-message.mg-out { background: color-mix(in srgb, var(--mg-bubble-out) 60%, transparent); }\n",
            BubbleStyle::Transparent => ".mg-message, .mg-message.mg-out { background: transparent; border: 1px solid var(--mg-border); color: var(--mg-text); }\n",
            BubbleStyle::Flat => ".mg-message, .mg-message.mg-out { background: none; align-self: stretch; max-width: none; border-radius: 0; padding-inline: 0; color: var(--mg-text); }\n.mg-message.mg-out .mg-sender { display: block; }\n",
        });
        rules.push_str(match self.timestamps {
            TimestampMode::Inline => "",
            TimestampMode::Hover => ".mg-message .mg-time { opacity: 0; transition: opacity .15s; }\n.mg-message:hover .mg-time { opacity: 1; }\n",
            TimestampMode::Hidden => ".mg-message .mg-time { display: none; }\n",
        });
        rules.push_str(match self.quote_style {
            QuoteStyle::Bar => "",
            QuoteStyle::Box => ".mg-reply-preview { border: 1px solid var(--mg-accent); border-radius: 6px; padding: 3px 6px; }\n",
            QuoteStyle::Minimal => ".mg-reply-preview { border: 0; padding: 0; font-style: italic; }\n",
        });
        if !self.show_sender_names {
            rules.push_str(".mg-sender { display: none; }\n");
        }
        rules.push_str(match self.avatar_shape {
            AvatarShape::Circle => "",
            AvatarShape::Rounded => ".mg-avatar { border-radius: 28%; }\n",
            AvatarShape::Square => ".mg-avatar { border-radius: 3px; }\n",
            AvatarShape::Hidden => ".mg-avatar { display: none; }\n",
        });
        if !self.chat_list_previews {
            rules.push_str(".mg-chat-last { display: none; }\n");
        }
        if self.glass_panels {
            rules.push_str(".mg-sidebar, .mg-titlebar, .mg-chat-header, .mg-composer { background: color-mix(in srgb, var(--mg-bg-2) 70%, transparent); backdrop-filter: blur(18px); }\n");
        }
        if !self.custom_scrollbars {
            rules.push_str("::-webkit-scrollbar { width: auto; height: auto; }\n");
        }
        let animation = if self.reduced_motion { MessageAnimation::None } else { self.message_animation };
        rules.push_str(match animation {
            MessageAnimation::None => "",
            MessageAnimation::Fade => ".mg-message.mg-new { animation: mg-fade .25s ease-out; }\n",
            MessageAnimation::Slide => ".mg-message.mg-new { animation: mg-slide .25s ease-out; }\n",
            MessageAnimation::Pop => ".mg-message.mg-new { animation: mg-pop .22s cubic-bezier(.3,1.6,.6,1); }\n",
        });
        if self.reduced_motion {
            rules.push_str("*, *::before, *::after { animation-duration: 0s !important; transition-duration: 0s !important; }\n");
        }

        for (id, chat) in &self.per_chat {
            let Some(sel) = chat_selector(id) else { continue };
            let mut decls = String::new();
            if let Some(bg) = chat.background.css() {
                let _ = write!(decls, "--mg-chat-bg: {bg}; ");
            }
            if let Some(c) = safe_color(&chat.bubble_in) {
                let _ = write!(decls, "--mg-bubble-in: {c}; ");
            }
            if let Some(c) = safe_color(&chat.bubble_out) {
                let _ = write!(decls, "--mg-bubble-out: {c}; ");
            }
            if let Some(c) = safe_color(&chat.accent) {
                let _ = write!(decls, "--mg-accent: {c}; ");
            }
            if !decls.is_empty() {
                let _ = writeln!(rules, "{sel} {{ {decls}}}");
            }
        }

        format!(":root {{\n{vars}}}\n{rules}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_compiles_to_variables_only() {
        let css = Appearance::default().to_css();
        assert!(css.contains("--mg-radius: 10px;"));
        assert!(css.contains("mg-fade"));
        assert!(!css.contains("--mg-accent"));
    }

    #[test]
    fn disabled_is_empty() {
        let a = Appearance { enabled: false, ..Default::default() };
        assert_eq!(a.to_css(), "");
    }

    #[test]
    fn gradients_and_per_chat() {
        let mut a = Appearance::default();
        a.outgoing_fill = Fill { enabled: true, from: "#8b7cf6".into(), to: "#ff8fab".into(), angle: 135 };
        a.per_chat.insert("-100123".into(), ChatAppearance { accent: "teal".into(), ..Default::default() });
        a.per_chat.insert("x\"]{}body{".into(), ChatAppearance { accent: "red".into(), ..Default::default() });
        let css = a.to_css();
        assert!(css.contains("--mg-bubble-out: linear-gradient(135deg, #8b7cf6, #ff8fab);"));
        assert!(css.contains(".mg-chat[data-chat-id=\"-100123\"] { --mg-accent: teal; }"));
        assert!(!css.contains("body{"));
    }

    #[test]
    fn rejects_injection() {
        let a = Appearance {
            accent: "red; } * { display:none".into(),
            font_family: "x; } body { color: red".into(),
            chat_background: Fill { enabled: true, from: "url(http://evil)".into(), ..Default::default() },
            ..Default::default()
        };
        let css = a.to_css();
        assert!(!css.contains("display:none") && !css.contains("evil") && !css.contains("color: red"));
    }

    #[test]
    fn reduced_motion_overrides_animation() {
        let a = Appearance { reduced_motion: true, message_animation: MessageAnimation::Pop, ..Default::default() };
        let css = a.to_css();
        assert!(!css.contains("mg-pop") && css.contains("animation-duration: 0s"));
    }
}
