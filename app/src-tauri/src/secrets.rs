//! Small sealed secret store for third-party API keys (e.g. WakaTime).
//!
//! Each secret is sealed with XChaCha20-Poly1305 under a per-install device
//! key (`secrets/device.key`, created with owner-only permissions on Unix),
//! with the secret's name bound as associated data so a sealed file cannot be
//! swapped for another. Values never appear in settings, logs or IPC replies.

use std::path::{Path, PathBuf};

use ametherian_truee2e::{open_at_rest, seal_at_rest, DeviceKey};
use zeroize::Zeroizing;

pub struct Secrets {
    dir: PathBuf,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

impl Secrets {
    pub fn new(config_dir: &Path) -> Self {
        Self { dir: config_dir.join("secrets") }
    }

    fn device_key(&self) -> Result<DeviceKey, String> {
        let path = self.dir.join("device.key");
        if let Ok(bytes) = std::fs::read(&path) {
            return DeviceKey::from_bytes(&bytes).map_err(|_| "the secrets key file is damaged".to_string());
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let key = DeviceKey::generate().map_err(|e| e.to_string())?;
        write_private(&path, &key.to_bytes()[..])?;
        Ok(key)
    }

    pub fn set(&self, name: &str, value: &str) -> Result<(), String> {
        if !valid_name(name) {
            return Err("invalid secret name".into());
        }
        let key = self.device_key()?;
        let sealed = seal_at_rest(&key, value.as_bytes(), name.as_bytes()).map_err(|e| e.to_string())?;
        write_private(&self.dir.join(format!("{name}.sealed")), &sealed)
    }

    pub fn get(&self, name: &str) -> Option<Zeroizing<String>> {
        if !valid_name(name) {
            return None;
        }
        let sealed = std::fs::read(self.dir.join(format!("{name}.sealed"))).ok()?;
        let key = self.device_key().ok()?;
        let plain = open_at_rest(&key, &sealed, name.as_bytes()).ok()?;
        String::from_utf8(plain.to_vec()).ok().map(Zeroizing::new)
    }

    pub fn has(&self, name: &str) -> bool {
        valid_name(name) && self.dir.join(format!("{name}.sealed")).is_file()
    }

    pub fn delete(&self, name: &str) {
        if valid_name(name) {
            let _ = std::fs::remove_file(self.dir.join(format!("{name}.sealed")));
        }
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_name_binding() {
        let dir = std::env::temp_dir().join(format!("mg-secrets-{}", std::process::id()));
        let s = Secrets::new(&dir);
        s.set("wakatime", "waka_abc").unwrap();
        assert_eq!(s.get("wakatime").unwrap().as_str(), "waka_abc");
        // A sealed file renamed to another secret does not open.
        std::fs::copy(dir.join("secrets/wakatime.sealed"), dir.join("secrets/other.sealed")).unwrap();
        assert!(s.get("other").is_none());
        assert!(s.set("../x", "v").is_err());
        s.delete("wakatime");
        assert!(!s.has("wakatime"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
