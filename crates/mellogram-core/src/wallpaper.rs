//! MelloStudio wallpaper engine.
//!
//! A wallpaper is a background layer drawn behind messages (or behind the
//! whole window). It can be a solid colour, a multi-stop gradient, one of the
//! user's own images, a generated SVG pattern, or a CSS-only animated scene.
//! Blur, dim, saturation and parallax are applied to the layer only, so text
//! stays sharp.
//!
//! Wallpapers compile to CSS. Every user value is validated before it reaches
//! CSS (colours, file names, numbers), so a shared profile cannot inject
//! rules or load remote resources. Image wallpapers only ever reference files
//! inside the app's own `wallpapers/` folder through a URL the host supplies.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::appearance::safe_color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ImageFit {
    #[default]
    Cover,
    Contain,
    Tile,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternKind {
    Dots,
    Grid,
    Diagonal,
    Waves,
    Checker,
    Hearts,
    Paws,
    Stars,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnimatedKind {
    /// Slow-moving soft colour fields.
    Aurora,
    /// Drifting radial glows.
    Nebula,
    /// Rolling gradient bands.
    Waves,
    /// Gentle colour cycling.
    Breathe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WallpaperKind {
    None,
    Solid { color: String },
    Gradient {
        /// 2–6 colours, evenly spaced.
        colors: Vec<String>,
        angle: u16,
        radial: bool,
    },
    Image {
        /// File name inside the wallpapers folder.
        file: String,
        fit: ImageFit,
    },
    Pattern {
        pattern: PatternKind,
        color: String,
        background: String,
        /// Tile size in px.
        size: u16,
        /// 0–100.
        opacity: u8,
    },
    Animated {
        animation: AnimatedKind,
        /// 2–5 colours.
        colors: Vec<String>,
        /// Seconds per cycle.
        speed: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wallpaper {
    pub kind: WallpaperKind,
    /// Blur in px applied to the layer (0–40).
    pub blur: u8,
    /// Darkening overlay 0–90 %.
    pub dim: u8,
    /// 0–200 %.
    pub saturation: u16,
    /// Subtle movement following the pointer.
    pub parallax: bool,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self { kind: WallpaperKind::None, blur: 0, dim: 0, saturation: 100, parallax: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WallpaperTarget {
    /// Behind the message list only.
    #[default]
    Chat,
    /// Behind the whole window; panels become translucent.
    Window,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperSettings {
    pub enabled: bool,
    pub target: WallpaperTarget,
    pub global: Wallpaper,
    /// Used instead of `global` while the system is in dark mode.
    pub dark: Option<Wallpaper>,
    /// Per-chat wallpapers, keyed by chat id.
    pub per_chat: BTreeMap<String, Wallpaper>,
    /// User-saved presets.
    pub saved: Vec<NamedWallpaper>,
    /// Panel translucency when `target` is `Window` (0–90 %).
    pub panel_transparency: u8,
}

impl Default for WallpaperSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            target: WallpaperTarget::Chat,
            global: Wallpaper::default(),
            dark: None,
            per_chat: BTreeMap::new(),
            saved: Vec::new(),
            panel_transparency: 35,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedWallpaper {
    pub name: String,
    pub wallpaper: Wallpaper,
}

/// Accepts only names the host itself generates for imported images.
pub fn safe_image_file(name: &str) -> bool {
    let (stem, ext) = match name.rsplit_once('.') {
        Some(p) => p,
        None => return false,
    };
    !stem.is_empty()
        && stem.len() <= 80
        && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "gif")
}

/// Detects the image type from its first bytes; rejects anything else.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else {
        None
    }
}

fn colors_ok(colors: &[String], min: usize, max: usize) -> Option<Vec<&str>> {
    let c: Vec<&str> = colors.iter().filter_map(|c| safe_color(c)).collect();
    (c.len() >= min && c.len() == colors.len() && c.len() <= max).then_some(c)
}

/// Percent-encodes a colour for use inside an SVG data URI.
fn uri_color(c: &str) -> String {
    c.replace('#', "%23").replace(' ', "%20").replace('(', "%28").replace(')', "%29").replace(',', "%2C")
}

fn pattern_svg(kind: PatternKind, color: &str, size: u16, opacity: u8) -> String {
    let s = size.clamp(8, 200);
    let o = f32::from(opacity.min(100)) / 100.0;
    let c = uri_color(color);
    let body = match kind {
        PatternKind::Dots => format!("<circle cx='{h}' cy='{h}' r='{r}' fill='{c}'/>", h = s / 2, r = (s / 8).max(1)),
        PatternKind::Grid => format!("<path d='M{s} 0H0V{s}' fill='none' stroke='{c}' stroke-width='1'/>"),
        PatternKind::Diagonal => format!("<path d='M0 {s}L{s} 0' stroke='{c}' stroke-width='{w}'/>", w = (s / 10).max(1)),
        PatternKind::Waves => format!(
            "<path d='M0 {h}Q{q} {a} {h} {h}T{s} {h}' fill='none' stroke='{c}' stroke-width='{w}'/>",
            h = s / 2,
            q = s / 4,
            a = s / 4,
            w = (s / 16).max(1)
        ),
        PatternKind::Checker => format!("<rect width='{h}' height='{h}' fill='{c}'/><rect x='{h}' y='{h}' width='{h}' height='{h}' fill='{c}'/>", h = s / 2),
        PatternKind::Hearts => format!(
            "<path transform='translate({t} {t}) scale({k})' d='M10 18L2.5 10.5A4.5 4.5 0 0 1 10 4a4.5 4.5 0 0 1 7.5 6.5z' fill='{c}'/>",
            t = s / 4,
            k = f32::from(s) / 40.0
        ),
        PatternKind::Paws => format!(
            "<g transform='translate({t} {t}) scale({k})' fill='{c}'><ellipse cx='10' cy='13' rx='5' ry='4'/><circle cx='4' cy='7' r='2'/><circle cx='8' cy='4' r='2'/><circle cx='12' cy='4' r='2'/><circle cx='16' cy='7' r='2'/></g>",
            t = s / 4,
            k = f32::from(s) / 40.0
        ),
        PatternKind::Stars => format!(
            "<circle cx='{a}' cy='{b}' r='1' fill='{c}'/><circle cx='{d}' cy='{e}' r='1.5' fill='{c}'/><circle cx='{f}' cy='{g}' r='0.8' fill='{c}'/>",
            a = s / 5,
            b = s / 3,
            d = s * 2 / 3,
            e = s / 6,
            f = s / 2,
            g = s * 4 / 5
        ),
    };
    format!(
        "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='{s}' height='{s}' opacity='{o:.2}'%3E{}%3C/svg%3E\")",
        body.replace('<', "%3C").replace('>', "%3E")
    )
}

impl Wallpaper {
    /// CSS declarations for the layer, or `None` for no wallpaper / invalid
    /// input. `image_url` maps a validated file name to a URL the webview can
    /// load (the host decides; it never receives arbitrary paths).
    pub fn layer_css(&self, image_url: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        let mut decl = String::new();
        match &self.kind {
            WallpaperKind::None => return None,
            WallpaperKind::Solid { color } => {
                let _ = write!(decl, "background: {};", safe_color(color)?);
            }
            WallpaperKind::Gradient { colors, angle, radial } => {
                let c = colors_ok(colors, 2, 6)?.join(", ");
                if *radial {
                    let _ = write!(decl, "background: radial-gradient(circle at 30% 20%, {c});");
                } else {
                    let _ = write!(decl, "background: linear-gradient({}deg, {c});", angle % 360);
                }
            }
            WallpaperKind::Image { file, fit } => {
                if !safe_image_file(file) {
                    return None;
                }
                let url = image_url(file)?;
                if url.contains(['"', '\\', '\n', ')']) {
                    return None;
                }
                let (size, repeat, pos) = match fit {
                    ImageFit::Cover => ("cover", "no-repeat", "center"),
                    ImageFit::Contain => ("contain", "no-repeat", "center"),
                    ImageFit::Tile => ("auto", "repeat", "top left"),
                    ImageFit::Center => ("auto", "no-repeat", "center"),
                };
                let _ = write!(decl, "background: url(\"{url}\") {pos} / {size} {repeat}; background-color: var(--mg-bg);");
            }
            WallpaperKind::Pattern { pattern, color, background, size, opacity } => {
                let fg = safe_color(color)?;
                let bg = safe_color(background).unwrap_or("var(--mg-bg)");
                let _ = write!(decl, "background: {} repeat, {bg};", pattern_svg(*pattern, fg, *size, *opacity));
            }
            WallpaperKind::Animated { animation, colors, speed } => {
                let c = colors_ok(colors, 2, 5)?;
                let secs = (*speed).clamp(4, 240);
                let all = c.join(", ");
                match animation {
                    AnimatedKind::Aurora => {
                        let _ = write!(decl, "background: linear-gradient(120deg, {all}, {}); background-size: 400% 400%; animation: mg-wp-pan {secs}s ease-in-out infinite alternate;", c[0]);
                    }
                    AnimatedKind::Nebula => {
                        let glows: Vec<String> = c
                            .iter()
                            .enumerate()
                            .map(|(i, col)| format!("radial-gradient(circle at {}% {}%, {col} 0, transparent 45%)", 15 + i * 30 % 80, 20 + i * 37 % 70))
                            .collect();
                        let _ = write!(decl, "background: {}, var(--mg-bg); background-size: 160% 160%; animation: mg-wp-drift {secs}s ease-in-out infinite alternate;", glows.join(", "));
                    }
                    AnimatedKind::Waves => {
                        let _ = write!(decl, "background: repeating-linear-gradient(160deg, {all}, {} 40%); background-size: 200% 200%; animation: mg-wp-pan {secs}s linear infinite;", c[0]);
                    }
                    AnimatedKind::Breathe => {
                        let _ = write!(decl, "background: {}; animation: mg-wp-hue {secs}s ease-in-out infinite alternate;", c[0]);
                    }
                }
            }
        }
        let mut filters = Vec::new();
        if self.blur > 0 {
            filters.push(format!("blur({}px)", self.blur.min(40)));
        }
        if self.saturation != 100 {
            filters.push(format!("saturate({}%)", self.saturation.min(200)));
        }
        if !filters.is_empty() {
            let _ = write!(decl, " filter: {};", filters.join(" "));
        }
        // Blur bleeds transparent edges; overscan hides them.
        if self.blur > 0 {
            let _ = write!(decl, " inset: -{}px;", u16::from(self.blur.min(40)) * 2);
        }
        let _ = write!(decl, " --mg-wp-dim: {:.2};", f32::from(self.dim.min(90)) / 100.0);
        Some(decl)
    }
}

impl WallpaperSettings {
    pub fn to_css(&self, image_url: &dyn Fn(&str) -> Option<String>) -> String {
        if !self.enabled {
            return String::new();
        }
        let layer = match self.target {
            WallpaperTarget::Chat => ".mg-chat > .mg-wallpaper",
            WallpaperTarget::Window => "body > .mg-wallpaper-window",
        };
        let mut css = String::new();
        let mut any = false;
        if let Some(d) = self.global.layer_css(image_url) {
            let _ = writeln!(css, "{layer} {{ {d} }}");
            any = true;
        }
        if let Some(d) = self.dark.as_ref().and_then(|w| w.layer_css(image_url)) {
            let _ = writeln!(css, "@media (prefers-color-scheme: dark) {{ {layer} {{ {d} }} }}");
            any = true;
        }
        for (id, wp) in &self.per_chat {
            let ok = !id.is_empty() && id.len() <= 24 && id.chars().all(|c| c.is_ascii_digit() || c == '-');
            if !ok {
                continue;
            }
            if let Some(d) = wp.layer_css(image_url) {
                // Per-chat wallpapers always sit behind that chat's messages.
                let _ = writeln!(css, ".mg-chat[data-chat-id=\"{id}\"] > .mg-wallpaper {{ {d} }}");
                any = true;
            }
        }
        if !any {
            return String::new();
        }
        css.push_str("body.mg-has-wallpaper .mg-messages { background: transparent; }\n");
        if self.target == WallpaperTarget::Window {
            let alpha = 100 - u16::from(self.panel_transparency.min(90));
            let _ = writeln!(
                css,
                ".mg-sidebar, .mg-titlebar, .mg-chat-header, .mg-composer {{ background: color-mix(in srgb, var(--mg-bg-2) {alpha}%, transparent); backdrop-filter: blur(16px); }}\n.mg-app {{ background: transparent; }}"
            );
        }
        css
    }

    /// The wallpaper that applies to a chat, for previews.
    pub fn for_chat(&self, chat_id: &str) -> &Wallpaper {
        self.per_chat.get(chat_id).unwrap_or(&self.global)
    }
}

/// Built-in presets shown in the gallery.
pub fn presets() -> Vec<NamedWallpaper> {
    fn named(name: &str, kind: WallpaperKind) -> NamedWallpaper {
        NamedWallpaper { name: name.into(), wallpaper: Wallpaper { kind, ..Wallpaper::default() } }
    }
    let v = |c: &[&str]| c.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    vec![
        named("Midnight Aurora", WallpaperKind::Animated { animation: AnimatedKind::Aurora, colors: v(&["#1a1446", "#3b2a8f", "#0e5e6f", "#5b2a86"]), speed: 30 }),
        named("Nebula", WallpaperKind::Animated { animation: AnimatedKind::Nebula, colors: v(&["#8b7cf680", "#ff8fab66", "#43c59e55"]), speed: 40 }),
        named("Ocean Waves", WallpaperKind::Animated { animation: AnimatedKind::Waves, colors: v(&["#0b2545", "#13315c", "#134074"]), speed: 60 }),
        named("Breathe", WallpaperKind::Animated { animation: AnimatedKind::Breathe, colors: v(&["#2a1f4f", "#1f3b4f"]), speed: 20 }),
        named("Lavender", WallpaperKind::Gradient { colors: v(&["#b8a9ff", "#8b7cf6", "#5b4bc4"]), angle: 160, radial: false }),
        named("Sunset", WallpaperKind::Gradient { colors: v(&["#ff9a8b", "#ff6a88", "#7f53ac"]), angle: 200, radial: false }),
        named("Forest", WallpaperKind::Gradient { colors: v(&["#0f2e1d", "#1e5631", "#4c9a2a"]), angle: 170, radial: false }),
        named("Pastel", WallpaperKind::Gradient { colors: v(&["#ffd6e8", "#d6e4ff", "#d8ffe8"]), angle: 135, radial: false }),
        named("Glow", WallpaperKind::Gradient { colors: v(&["#3b2a8f", "#0f1222"]), angle: 0, radial: true }),
        named("Paw Prints", WallpaperKind::Pattern { pattern: PatternKind::Paws, color: "#8b7cf6".into(), background: "#151933".into(), size: 64, opacity: 35 }),
        named("Hearts", WallpaperKind::Pattern { pattern: PatternKind::Hearts, color: "#ff8fab".into(), background: "#2a1730".into(), size: 56, opacity: 30 }),
        named("Starfield", WallpaperKind::Pattern { pattern: PatternKind::Stars, color: "#ffffff".into(), background: "#05060f".into(), size: 90, opacity: 70 }),
        named("Blueprint", WallpaperKind::Pattern { pattern: PatternKind::Grid, color: "#4f7cff".into(), background: "#0b1a3a".into(), size: 24, opacity: 45 }),
        named("Dots", WallpaperKind::Pattern { pattern: PatternKind::Dots, color: "#9a98b3".into(), background: "#17161f".into(), size: 22, opacity: 30 }),
        named("AMOLED Black", WallpaperKind::Solid { color: "#000000".into() }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_images(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn every_preset_compiles() {
        for p in presets() {
            assert!(p.wallpaper.layer_css(&no_images).is_some(), "{}", p.name);
        }
    }

    #[test]
    fn gradient_and_filters() {
        let w = Wallpaper {
            kind: WallpaperKind::Gradient { colors: vec!["#111".into(), "red".into()], angle: 400, radial: false },
            blur: 8,
            dim: 40,
            saturation: 150,
            parallax: false,
        };
        let css = w.layer_css(&no_images).unwrap();
        assert!(css.contains("linear-gradient(40deg, #111, red)"));
        assert!(css.contains("blur(8px) saturate(150%)"));
        assert!(css.contains("inset: -16px"));
        assert!(css.contains("--mg-wp-dim: 0.40"));
    }

    #[test]
    fn rejects_injection_and_remote_images() {
        let bad = Wallpaper { kind: WallpaperKind::Solid { color: "red; } body { display:none".into() }, ..Default::default() };
        assert!(bad.layer_css(&no_images).is_none());
        let bad = Wallpaper { kind: WallpaperKind::Gradient { colors: vec!["red".into(), "url(http://x)".into()], angle: 0, radial: false }, ..Default::default() };
        assert!(bad.layer_css(&no_images).is_none());
        let img = |f: &str| Some(format!("asset://localhost/w/{f}"));
        for file in ["../../etc/passwd.png", "x.svg", "a b.png", "http://evil/x.png"] {
            let w = Wallpaper { kind: WallpaperKind::Image { file: file.into(), fit: ImageFit::Cover }, ..Default::default() };
            assert!(w.layer_css(&img).is_none(), "{file}");
        }
        let ok = Wallpaper { kind: WallpaperKind::Image { file: "wp-1a2b.jpg".into(), fit: ImageFit::Tile }, ..Default::default() };
        assert!(ok.layer_css(&img).unwrap().contains("url(\"asset://localhost/w/wp-1a2b.jpg\") top left / auto repeat"));
    }

    #[test]
    fn settings_css_per_chat_dark_and_window() {
        let mut s = WallpaperSettings::default();
        assert_eq!(s.to_css(&no_images), "", "nothing configured, nothing emitted");
        s.global = presets().into_iter().find(|p| p.name == "Lavender").unwrap().wallpaper;
        s.dark = Some(Wallpaper { kind: WallpaperKind::Solid { color: "#000".into() }, ..Default::default() });
        s.per_chat.insert("-100123".into(), Wallpaper { kind: WallpaperKind::Solid { color: "teal".into() }, ..Default::default() });
        s.per_chat.insert("x\"]{}".into(), Wallpaper { kind: WallpaperKind::Solid { color: "red".into() }, ..Default::default() });
        let css = s.to_css(&no_images);
        assert!(css.contains(".mg-chat > .mg-wallpaper { background: linear-gradient"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains(".mg-chat[data-chat-id=\"-100123\"] > .mg-wallpaper { background: teal;"));
        assert!(!css.contains("x\\\"]") && !css.contains("red;"));
        s.target = WallpaperTarget::Window;
        let css = s.to_css(&no_images);
        assert!(css.contains("body > .mg-wallpaper-window") && css.contains("backdrop-filter"));
        s.enabled = false;
        assert_eq!(s.to_css(&no_images), "");
    }

    #[test]
    fn sniffing_and_names() {
        assert_eq!(sniff_image(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("png"));
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image(b"<svg onload=alert(1)>"), None);
        assert!(safe_image_file("wp-abc_1.webp"));
        assert!(!safe_image_file("wp.svg") && !safe_image_file(".png") && !safe_image_file("a/b.png"));
    }

    #[test]
    fn pattern_colors_are_uri_encoded() {
        let w = Wallpaper { kind: WallpaperKind::Pattern { pattern: PatternKind::Dots, color: "#ff0000".into(), background: "".into(), size: 20, opacity: 50 }, ..Default::default() };
        let css = w.layer_css(&no_images).unwrap();
        assert!(css.contains("%23ff0000") && !css.contains("fill='#"));
    }
}
