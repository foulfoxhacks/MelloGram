//! Themes are plain `.css` files in the themes directory, optionally starting
//! with a metadata comment:
//!
//! ```css
//! /**
//!  * @name Midnight Mello
//!  * @author you
//!  * @version 1.0.0
//!  * @description Deep blue with soft purple accents.
//!  */
//! :root { --mg-accent: #8b7cf6; }
//! ```
//!
//! The same header format is used by other client-mod theme ecosystems, so
//! many existing themes keep their metadata when dropped in (their selectors
//! will of course target a different DOM).
//!
//! Custom properties declared in the theme's first `:root { … }` block are
//! surfaced as editable variables; user overrides are appended after the
//! theme so they win.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings::{write_atomic, ThemeSettings};
use crate::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// File name inside the themes directory; the theme's identity.
    pub file: String,
    pub meta: ThemeMeta,
    /// `--name: value` pairs from the first `:root` block, in order.
    pub variables: Vec<(String, String)>,
    #[serde(skip)]
    pub css: String,
}

impl Theme {
    pub fn parse(file: impl Into<String>, css: impl Into<String>) -> Self {
        let css = css.into();
        Self {
            file: file.into(),
            meta: parse_meta(&css),
            variables: parse_root_variables(&css),
            css,
        }
    }

    pub fn display_name(&self) -> &str {
        self.meta.name.as_deref().unwrap_or(&self.file)
    }
}

pub fn parse_meta(css: &str) -> ThemeMeta {
    let mut meta = ThemeMeta::default();
    let trimmed = css.trim_start();
    let Some(rest) = trimmed.strip_prefix("/*") else {
        return meta;
    };
    let Some(end) = rest.find("*/") else {
        return meta;
    };
    for line in rest[..end].lines() {
        let line = line.trim().trim_start_matches('*').trim();
        let Some(tag) = line.strip_prefix('@') else { continue };
        let (key, value) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        let value = Some(value.trim().to_owned()).filter(|v| !v.is_empty());
        match key {
            "name" => meta.name = value,
            "author" => meta.author = value,
            "version" => meta.version = value,
            "description" => meta.description = value,
            "source" | "website" => meta.source = value,
            _ => {}
        }
    }
    meta
}

pub fn parse_root_variables(css: &str) -> Vec<(String, String)> {
    let css = strip_comments(css);
    let Some(start) = css.find(":root") else {
        return Vec::new();
    };
    let after = &css[start..];
    let (Some(open), Some(close)) = (after.find('{'), after.find('}')) else {
        return Vec::new();
    };
    if close < open {
        return Vec::new();
    }
    after[open + 1..close]
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim();
            name.starts_with("--")
                .then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Returns a safe `*.css` file name, or `None` if the input cannot be one.
pub fn sanitize_file_name(name: &str) -> Option<String> {
    let base: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "-_. ".contains(c) { c } else { '-' })
        .collect();
    let base = base.trim().trim_matches('.').to_owned();
    if base.is_empty() || base.contains("..") {
        return None;
    }
    Some(if base.ends_with(".css") { base } else { format!("{base}.css") })
}

/// Manages the themes directory.
#[derive(Debug)]
pub struct ThemeManager {
    dir: PathBuf,
}

impl ThemeManager {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// All `.css` files, sorted by file name.
    pub fn list(&self) -> Result<Vec<Theme>> {
        let mut themes = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "css") {
                let file = path.file_name().unwrap().to_string_lossy().into_owned();
                themes.push(Theme::parse(file, fs::read_to_string(&path)?));
            }
        }
        themes.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(themes)
    }

    pub fn get(&self, file: &str) -> Result<Theme> {
        let safe = sanitize_file_name(file).filter(|f| f == file).ok_or_else(|| Error::UnknownTheme(file.into()))?;
        let css = fs::read_to_string(self.dir.join(&safe)).map_err(|_| Error::UnknownTheme(file.into()))?;
        Ok(Theme::parse(safe, css))
    }

    /// Writes a theme. The file name comes from `@name` when present.
    pub fn install(&self, suggested_name: &str, css: &str) -> Result<Theme> {
        let meta = parse_meta(css);
        let wanted = meta.name.as_deref().unwrap_or(suggested_name);
        let file = sanitize_file_name(wanted)
            .ok_or_else(|| Error::InvalidSetting { key: "theme".into(), reason: format!("bad name `{wanted}`") })?;
        write_atomic(&self.dir.join(&file), css.as_bytes())?;
        Ok(Theme::parse(file, css))
    }

    pub fn remove(&self, file: &str, settings: &mut ThemeSettings) -> Result<()> {
        let theme = self.get(file)?;
        fs::remove_file(self.dir.join(&theme.file))?;
        settings.enabled.retain(|f| f != file);
        settings.variables.remove(file);
        Ok(())
    }

    /// The final stylesheet: enabled themes in order, each followed by the
    /// user's variable overrides, then plugin CSS, then QuickCSS last.
    pub fn compose(&self, settings: &ThemeSettings, plugin_css: &[(String, String)]) -> String {
        let mut out = String::new();
        for file in &settings.enabled {
            let Ok(theme) = self.get(file) else {
                tracing::warn!("enabled theme `{file}` is missing");
                continue;
            };
            out.push_str(&format!("/* theme: {} */\n{}\n", theme.file, theme.css));
            if let Some(vars) = settings.variables.get(file) {
                out.push_str(&variables_block(vars));
            }
        }
        for (plugin, css) in plugin_css {
            out.push_str(&format!("/* plugin: {plugin} */\n{css}\n"));
        }
        if settings.quick_css_enabled && !settings.quick_css.trim().is_empty() {
            out.push_str("/* quickcss */\n");
            out.push_str(&settings.quick_css);
            out.push('\n');
        }
        out
    }
}

fn variables_block(vars: &BTreeMap<String, String>) -> String {
    let body: String = vars
        .iter()
        .filter(|(k, v)| k.starts_with("--") && !v.contains(['{', '}', ';']))
        .map(|(k, v)| format!("  {k}: {v};\n"))
        .collect();
    if body.is_empty() {
        String::new()
    } else {
        format!(":root {{\n{body}}}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSS: &str = "/**\n * @name Midnight Mello\n * @author tester\n * @version 2.1\n */\n\
        /* palette */\n:root { --mg-accent: #8b7cf6; --mg-radius: 8px; color: red }\nbody{}";

    #[test]
    fn parses_header_and_variables() {
        let t = Theme::parse("x.css", CSS);
        assert_eq!(t.meta.name.as_deref(), Some("Midnight Mello"));
        assert_eq!(t.meta.author.as_deref(), Some("tester"));
        assert_eq!(t.meta.version.as_deref(), Some("2.1"));
        assert_eq!(
            t.variables,
            vec![("--mg-accent".into(), "#8b7cf6".into()), ("--mg-radius".into(), "8px".into())]
        );
    }

    #[test]
    fn sanitises_names() {
        assert_eq!(sanitize_file_name("Midnight Mello").as_deref(), Some("Midnight Mello.css"));
        assert_eq!(sanitize_file_name("../etc/passwd").as_deref(), Some("-etc-passwd.css"));
        assert_eq!(sanitize_file_name("..").as_deref(), None);
    }

    #[test]
    fn composes_in_order_with_overrides() {
        let dir = std::env::temp_dir().join(format!("mellogram-themes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let tm = ThemeManager::new(&dir).unwrap();
        let a = tm.install("ignored", CSS).unwrap();
        let b = tm.install("second", "body{background:black}").unwrap();
        assert_eq!(a.file, "Midnight Mello.css");

        let mut s = ThemeSettings {
            enabled: vec![b.file.clone(), a.file.clone()],
            quick_css: ".x{}".into(),
            ..Default::default()
        };
        s.variables.insert(a.file.clone(), BTreeMap::from([("--mg-accent".into(), "hotpink".into())]));
        let css = tm.compose(&s, &[("p".into(), ".p{}".into())]);
        let ib = css.find("background:black").unwrap();
        let ia = css.find("Midnight").unwrap();
        let iv = css.find("hotpink").unwrap();
        let ip = css.find(".p{}").unwrap();
        let iq = css.find(".x{}").unwrap();
        assert!(ib < ia && ia < iv && iv < ip && ip < iq);

        tm.remove(&a.file, &mut s).unwrap();
        assert_eq!(s.enabled, vec![b.file]);
        assert!(tm.get("../settings.json").is_err());
    }
}
