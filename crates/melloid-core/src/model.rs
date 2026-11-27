use serde::{Deserialize, Serialize};

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProfileVisibility {
    #[default]
    Public,
    Unlisted,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Social,
    Gaming,
    Media,
    Creative,
    Development,
    Community,
    Website,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeCategory {
    Verified,
    Achievement,
    Community,
    Official,
    Collectible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceKind {
    Playing,
    Listening,
    Watching,
    Streaming,
    Competing,
    Creating,
    Working,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Games,
    Music,
    Projects,
    Communities,
    Activity,
    About,
    CustomLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BioSection {
    pub id: String,
    pub heading: String,
    pub body: String,
    pub visible: bool,
}
impl Default for BioSection {
    fn default() -> Self {
        Self { id: "about".into(), heading: "About me".into(), body: String::new(), visible: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionRecord {
    pub id: String,
    pub provider: String,
    pub kind: ConnectionKind,
    pub display_name: String,
    pub profile_url: Option<String>,
    /// Ownership verification is assigned only by a trusted verifier, never
    /// by the profile editor or a user-supplied boolean.
    pub verification_issuer: Option<String>,
    pub verified_at: Option<String>,
    pub visible: bool,
}
impl Default for ConnectionRecord {
    fn default() -> Self {
        Self { id: String::new(), provider: String::new(), kind: ConnectionKind::Other,
            display_name: String::new(), profile_url: None, verification_issuer: None,
            verified_at: None, visible: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BadgeRecord {
    pub id: String,
    pub issuer: String,
    pub title: String,
    pub description: String,
    pub category: BadgeCategory,
    pub awarded_at: Option<String>,
    pub visible: bool,
    /// Badge definitions and grants are server-authoritative once synced.
    pub server_issued: bool,
}
impl Default for BadgeRecord {
    fn default() -> Self {
        Self { id: String::new(), issuer: String::new(), title: String::new(),
            description: String::new(), category: BadgeCategory::Collectible,
            awarded_at: None, visible: true, server_issued: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileWidget {
    pub id: String,
    pub kind: WidgetKind,
    pub title: String,
    pub enabled: bool,
    pub position: u32,
    pub config: serde_json::Value,
}
impl Default for ProfileWidget {
    fn default() -> Self {
        Self { id: String::new(), kind: WidgetKind::About, title: String::new(),
            enabled: true, position: 0, config: serde_json::json!({}) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresenceActivity {
    pub application: String,
    pub activity: String,
    pub details: Option<String>,
    pub state: Option<String>,
    pub kind: PresenceKind,
    pub started_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub party_current: Option<u32>,
    pub party_max: Option<u32>,
    pub join_url: Option<String>,
    pub large_image: Option<String>,
    pub small_image: Option<String>,
}
impl Default for PresenceActivity {
    fn default() -> Self {
        Self { application: String::new(), activity: String::new(), details: None,
            state: None, kind: PresenceKind::Custom, started_at: None, ends_at: None,
            party_current: None, party_max: None, join_url: None,
            large_image: None, small_image: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MelloProfile {
    pub schema_version: u32,
    /// Stable local identity key. A remote service will assign its own immutable ID.
    pub local_id: String,
    pub slug: String,
    pub display_name: String,
    pub headline: String,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub accent_color: String,
    pub profile_theme: String,
    pub avatar_frame: String,
    pub profile_effect: String,
    pub visibility: ProfileVisibility,
    pub bio_sections: Vec<BioSection>,
    pub connections: Vec<ConnectionRecord>,
    pub badges: Vec<BadgeRecord>,
    pub widgets: Vec<ProfileWidget>,
    pub featured_badges: Vec<String>,
    pub show_presence: bool,
    pub presence: Option<PresenceActivity>,
    pub created_at: String,
    pub updated_at: String,
}
impl Default for MelloProfile {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            local_id: "local-default".into(),
            slug: "your-melloid".into(),
            display_name: "Your MelloID".into(),
            headline: String::new(),
            avatar_url: None,
            banner_url: None,
            accent_color: "#35bdb1".into(),
            profile_theme: "midnight".into(),
            avatar_frame: "none".into(),
            profile_effect: "none".into(),
            visibility: ProfileVisibility::Public,
            bio_sections: vec![BioSection::default()],
            connections: Vec::new(),
            badges: Vec::new(),
            widgets: vec![ProfileWidget { id: "about".into(), kind: WidgetKind::About,
                title: "About".into(), enabled: true, position: 0,
                config: serde_json::json!({}) }],
            featured_badges: Vec::new(),
            show_presence: true,
            presence: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

impl MelloProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(format!("unsupported profile schema version {}", self.schema_version));
        }
        if self.slug.is_empty() || self.slug.len() > 32
            || !self.slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return Err("profile slug must be 1-32 lowercase letters, numbers, or hyphens".into());
        }
        if self.display_name.trim().is_empty() || self.display_name.chars().count() > 64 {
            return Err("display name must contain 1-64 characters".into());
        }
        if !["midnight", "ocean", "aurora", "ember", "rose", "mint"].contains(&self.profile_theme.as_str()) {
            return Err("unsupported profile theme".into());
        }
        if !["none", "glow", "neon", "orbit", "paw"].contains(&self.avatar_frame.as_str()) {
            return Err("unsupported avatar frame".into());
        }
        if !["none", "aurora", "stars", "pawprints", "glow"].contains(&self.profile_effect.as_str()) {
            return Err("unsupported profile effect".into());
        }
        if self.headline.chars().count() > 120 {
            return Err("headline must be 120 characters or fewer".into());
        }
        if self.bio_sections.len() > 12 || self.connections.len() > 40
            || self.badges.len() > 100 || self.widgets.len() > 24 {
            return Err("profile contains too many sections, connections, badges, or widgets".into());
        }
        if !self.accent_color.starts_with('#') || !matches!(self.accent_color.len(), 4 | 7)
            || !self.accent_color[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("accent color must be a #RGB or #RRGGBB hex color".into());
        }
        for section in &self.bio_sections {
            if section.heading.chars().count() > 48 || section.body.chars().count() > 2000 {
                return Err("bio headings are limited to 48 characters and section text to 2000".into());
            }
        }
        for badge in &self.badges {
            if badge.title.trim().is_empty() || badge.title.chars().count() > 48
                || badge.description.chars().count() > 160 {
                return Err("badge titles must be 1-48 characters and descriptions 160 characters or fewer".into());
            }
        }
        for widget in &self.widgets {
            if widget.title.trim().is_empty() || widget.title.chars().count() > 48 {
                return Err("widget titles must contain 1-48 characters".into());
            }
            let Some(config) = widget.config.as_object() else {
                return Err("widget configuration must be a JSON object".into());
            };
            if let Some(items) = config.get("items") {
                let Some(items) = items.as_array() else {
                    return Err("widget items must be a list".into());
                };
                if items.len() > 20 || items.iter().any(|item| item.as_str().map_or(true, |text| text.chars().count() > 120)) {
                    return Err("widgets support up to 20 text items of 120 characters each".into());
                }
            }
            if let Some(url) = config.get("url").and_then(serde_json::Value::as_str) {
                if !url.is_empty() && !(url.starts_with("https://") || url.starts_with("http://")) {
                    return Err("widget URLs must use HTTP or HTTPS".into());
                }
            }
        }
        for connection in &self.connections {
            if connection.provider.trim().is_empty() || connection.display_name.chars().count() > 80 {
                return Err("connections require a provider and an 80-character display name or less".into());
            }
            if let Some(url) = &connection.profile_url {
                if !(url.starts_with("https://") || url.starts_with("http://")) {
                    return Err("connection URLs must use http or https".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_profile_is_valid_and_public() {
        let p = MelloProfile::default();
        assert_eq!(p.visibility, ProfileVisibility::Public);
        assert!(p.validate().is_ok());
    }
    #[test]
    fn rejects_invalid_slug_and_oversized_bio() {
        let mut p = MelloProfile::default();
        p.slug = "../private".into();
        assert!(p.validate().is_err());
        p.slug = "valid-profile".into();
        p.bio_sections[0].body = "x".repeat(2001);
        assert!(p.validate().is_err());
    }
    #[test]
    fn profile_appearance_uses_only_known_presets() {
        let mut p = MelloProfile::default();
        p.profile_theme = "url-javascript".into();
        assert!(p.validate().is_err());
        p.profile_theme = "ocean".into();
        p.avatar_frame = "neon".into();
        p.profile_effect = "stars".into();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn rejects_empty_or_oversized_badge_labels() {
        let mut p = MelloProfile::default();
        p.badges.push(BadgeRecord {
            category: BadgeCategory::Collectible,
            title: String::new(),
            ..Default::default()
        });
        assert!(p.validate().is_err());
        p.badges[0].title = "Personal collectible".into();
        p.badges[0].description = "x".repeat(161);
        assert!(p.validate().is_err());
    }

    #[test]
    fn widget_config_is_bounded_and_uses_safe_urls() {
        let mut p = MelloProfile::default();
        p.widgets[0].config = serde_json::json!({ "items": ["x".repeat(121)] });
        assert!(p.validate().is_err());
        p.widgets[0].config = serde_json::json!({ "items": ["Game one"], "url": "javascript:alert(1)" });
        assert!(p.validate().is_err());
        p.widgets[0].config = serde_json::json!({ "items": ["Game one"], "url": "https://example.com" });
        assert!(p.validate().is_ok());
    }

    #[test]
    fn connection_urls_must_be_http() {
        let mut p = MelloProfile::default();
        p.connections.push(ConnectionRecord { profile_url: Some("javascript:alert(1)".into()), ..Default::default() });
        assert!(p.validate().is_err());
    }
}
