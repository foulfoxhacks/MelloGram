//! Privacy plugins.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mellogram_core::{Permission, Category, OutgoingMessage, Plugin, PluginContext, PluginManifest, SendDecision, SettingDef};

/// Stops the client from broadcasting "typing…".
pub struct SilentTyping;

impl Plugin for SilentTyping {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Don't show others that you're typing.",
            inspired_by: Some("Vencord: SilentTyping"),
            permissions: &[],
            ..PluginManifest::new("silent-typing", "Silent Typing", Category::Privacy)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::list("allow_in", "Still show typing in these chats", &[])
            .describe("Chat ids, one per line (right-click a chat → Copy ID).")]
    }

    fn allow_typing(&self, ctx: &PluginContext, chat_id: i64) -> bool {
        ctx.list("allow_in").iter().any(|id| id.trim().parse() == Ok(chat_id))
    }
}

// ---------------------------------------------------------------------------

/// Renames uploads so file names don't leak information.
pub struct AnonymiseUploads;

impl AnonymiseUploads {
    pub fn new_name(original: &str, method: &str, length: usize) -> String {
        let ext = Path::new(original)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let stem = match method {
            "timestamp" => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis().to_string())
                .unwrap_or_default(),
            "fixed" => "file".to_owned(),
            _ => random_stem(length.clamp(4, 32)),
        };
        format!("{stem}{ext}")
    }
}

fn random_stem(len: usize) -> String {
    // Not cryptographic; it only needs to be unpredictable enough to hide the
    // original name. Seeded from the clock plus a counter, mixed with splitmix64.
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
        ^ COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    let mut state = seed;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    (0..len)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            ALPHABET[(z % ALPHABET.len() as u64) as usize] as char
        })
        .collect()
}

impl Plugin for AnonymiseUploads {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Replaces the names of files you upload with random ones.",
            inspired_by: Some("Vencord: AnonymiseFileNames"),
            permissions: &[Permission::ModifyOutgoing],
            ..PluginManifest::new("anonymise-uploads", "Anonymise Uploads", Category::Privacy)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![
            SettingDef::select(
                "method",
                "Naming method",
                "random",
                &[("random", "Random characters"), ("timestamp", "Timestamp"), ("fixed", "Always “file”")],
            ),
            SettingDef::number("length", "Random name length", 8.0, 4.0, 32.0),
        ]
    }

    fn before_send(&self, ctx: &PluginContext, msg: &mut OutgoingMessage) -> SendDecision {
        for file in &mut msg.attachments {
            file.file_name = Self::new_name(&file.file_name, ctx.str("method"), ctx.number("length") as usize);
        }
        SendDecision::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mellogram_core::OutgoingFile;

    #[test]
    fn silent_typing_allowlist() {
        let p = SilentTyping;
        let mut ctx = PluginContext::with_defaults(&p);
        assert!(!p.allow_typing(&ctx, 42));
        ctx.set("allow_in", serde_json::json!(["42"]));
        assert!(p.allow_typing(&ctx, 42));
        assert!(!p.allow_typing(&ctx, 7));
    }

    #[test]
    fn anonymises_names_keeping_extension() {
        let p = AnonymiseUploads;
        let ctx = PluginContext::with_defaults(&p);
        let mut m = OutgoingMessage::text(1, "");
        m.attachments.push(OutgoingFile { path: "/home/me/Passport Scan.PNG".into(), file_name: "Passport Scan.PNG".into() });
        p.before_send(&ctx, &mut m);
        let name = &m.attachments[0].file_name;
        assert!(name.ends_with(".PNG") && name.len() == 12, "{name}");
        assert_ne!(AnonymiseUploads::new_name("a", "random", 8), AnonymiseUploads::new_name("a", "random", 8));
        assert_eq!(AnonymiseUploads::new_name("x.tar", "fixed", 8), "file.tar");
    }
}
