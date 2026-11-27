use std::fs;
use std::path::{Path, PathBuf};

use crate::{MelloProfile, Result};

/// Local profile cache. It is not a public hosting service; syncing profiles
/// requires a future authenticated MelloID API.
pub struct MelloIdStore {
    path: PathBuf,
    profile: MelloProfile,
}
impl MelloIdStore {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("melloid-profile.json");
        let profile = match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => MelloProfile::default(),
            Err(error) => return Err(error.into()),
        };
        profile.validate().map_err(crate::IdentityError::Invalid)?;
        Ok(Self { path, profile })
    }

    pub fn profile(&self) -> &MelloProfile { &self.profile }

    pub fn save(&mut self, mut profile: MelloProfile) -> Result<()> {
        profile.validate().map_err(crate::IdentityError::Invalid)?;
        profile.created_at = self.profile.created_at.clone();
        profile.updated_at = chrono::Utc::now().to_rfc3339();
        let bytes = serde_json::to_vec_pretty(&profile)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &self.path)?;
        self.profile = profile;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_and_reopens_profile() {
        let dir = std::env::temp_dir().join(format!("melloid-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut store = MelloIdStore::open(&dir).unwrap();
        let mut profile = store.profile().clone();
        profile.display_name = "Mello Tester".into();
        store.save(profile).unwrap();
        let reopened = MelloIdStore::open(&dir).unwrap();
        assert_eq!(reopened.profile().display_name, "Mello Tester");
        let _ = fs::remove_dir_all(dir);
    }
}
