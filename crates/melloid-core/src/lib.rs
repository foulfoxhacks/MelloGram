//! MelloID's backend-neutral identity domain and local persistence foundation.
//!
//! Public profile hosting, OAuth verification and live presence transport are
//! intentionally separate services. This crate defines the versioned contract
//! shared by the client and those future services.

mod model;
mod store;

pub use model::{
    BadgeCategory, BadgeRecord, BioSection, ConnectionKind, ConnectionRecord,
    MelloProfile, PresenceActivity, PresenceKind, ProfileVisibility,
    ProfileWidget, WidgetKind, PROFILE_SCHEMA_VERSION,
};
pub use store::MelloIdStore;

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("profile I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("profile JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid profile: {0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, IdentityError>;
