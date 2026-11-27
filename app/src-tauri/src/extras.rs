//! Commands for the customisation platform: Appearance Studio, workspaces,
//! profile sharing, app lock, plugin storage, diagnostics and safe mode.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mellogram_core::manager::PackageReport;
use mellogram_core::settings::BackupTheme;
use mellogram_core::{Appearance, OutgoingMessage, Permission, ProfilePackage, WorkspaceSummary};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ---- Appearance Studio ----------------------------------------------------------

#[tauri::command]
pub fn get_appearance(state: S<'_>) -> Appearance {
    state.store.lock().unwrap().settings.appearance.clone()
}

#[tauri::command]
pub fn set_appearance(state: S<'_>, appearance: Appearance) -> R<()> {
    state.store.lock().unwrap().settings.appearance = appearance;
    state.save()?;
    state.refresh_styles();
    Ok(())
}

// ---- Workspaces -------------------------------------------------------------------

#[tauri::command]
pub fn list_workspaces(state: S<'_>) -> Vec<WorkspaceSummary> {
    state.store.lock().unwrap().settings.workspace_summaries()
}

#[tauri::command]
pub fn create_workspace(state: S<'_>, name: String, icon: String) -> R<String> {
    if name.trim().is_empty() {
        return Err("give the workspace a name".into());
    }
    let id = state.store.lock().unwrap().settings.create_workspace(&name, &icon);
    state.save()?;
    Ok(id)
}

#[tauri::command]
pub fn switch_workspace(state: S<'_>, id: String) -> R<()> {
    state.store.lock().unwrap().settings.switch_workspace(&id).map_err(err)?;
    state.save()?;
    state.reload_plugins();
    Ok(())
}

#[tauri::command]
pub fn duplicate_workspace(state: S<'_>, id: String) -> R<String> {
    let new_id = state.store.lock().unwrap().settings.duplicate_workspace(&id).map_err(err)?;
    state.save()?;
    Ok(new_id)
}

#[tauri::command]
pub fn update_workspace(state: S<'_>, id: String, name: String, icon: String, description: String) -> R<()> {
    state.store.lock().unwrap().settings.update_workspace_meta(&id, &name, &icon, &description).map_err(err)?;
    state.save()
}

#[tauri::command]
pub fn delete_workspace(state: S<'_>, id: String) -> R<()> {
    state.store.lock().unwrap().settings.delete_workspace(&id);
    state.save()
}

// ---- Profile sharing ------------------------------------------------------------------

/// Exports a workspace (or the live configuration when `id` is empty) with
/// the theme files it enables.
#[tauri::command]
pub fn export_profile(state: S<'_>, id: Option<String>, name: String, author: String, description: String) -> R<String> {
    let mut store = state.store.lock().unwrap();
    store.settings.save_active_workspace();
    let mut ws = match id.filter(|i| !i.is_empty()) {
        Some(id) => store
            .settings
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or("unknown workspace")?,
        None => store.settings.snapshot(String::new(), name.clone()),
    };
    drop(store);
    if !name.trim().is_empty() {
        ws.name = name.trim().to_owned();
    }
    ws.description = description.trim().to_owned();
    let themes = ws
        .themes
        .enabled
        .iter()
        .filter_map(|f| state.themes.get(f).ok())
        .map(|t| BackupTheme { file: t.file, css: t.css })
        .collect();
    serde_json::to_string_pretty(&ProfilePackage::new(ws, &author, themes)).map_err(err)
}

#[tauri::command]
pub fn inspect_profile(state: S<'_>, json: String) -> R<PackageReport> {
    let pkg = ProfilePackage::parse(&json).map_err(err)?;
    Ok(state.plugins.lock().unwrap().inspect_package(&pkg))
}

/// Imports a package as a new, inactive workspace. Themes are installed only
/// if no theme with that file name exists, so imports never overwrite yours.
#[tauri::command]
pub fn import_profile(state: S<'_>, json: String) -> R<Value> {
    let pkg = ProfilePackage::parse(&json).map_err(err)?;
    let mut skipped = Vec::new();
    for t in &pkg.themes {
        match state.themes.get(&t.file) {
            Ok(existing) if existing.css != t.css => skipped.push(t.file.clone()),
            Ok(_) => {}
            Err(_) => {
                std::fs::write(state.themes.dir().join(&t.file), &t.css).map_err(err)?;
            }
        }
    }
    let id = state.store.lock().unwrap().settings.add_imported_workspace(pkg.workspace);
    state.save()?;
    Ok(json!({ "id": id, "skipped_themes": skipped }))
}

// ---- App lock -------------------------------------------------------------------------

const PIN_ITERATIONS: u32 = 120_000;

fn hash_pin(pin: &str, salt: &str) -> String {
    let mut digest = Sha256::new().chain_update(salt.as_bytes()).chain_update(pin.as_bytes()).finalize();
    for _ in 1..PIN_ITERATIONS {
        digest = Sha256::new().chain_update(digest).chain_update(salt.as_bytes()).finalize();
    }
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn check_pin(state: &AppState, pin: &str) -> R<bool> {
    {
        let failures = state.lock_failures.lock().unwrap();
        if let Some(until) = failures.1 {
            if Instant::now() < until {
                return Err(format!("Too many attempts. Try again in {}s.", (until - Instant::now()).as_secs() + 1));
            }
        }
    }
    let (hash, salt) = {
        let s = &state.store.lock().unwrap().settings.privacy;
        (s.lock_pin_hash.clone(), s.lock_salt.clone())
    };
    let (Some(hash), Some(salt)) = (hash, salt) else { return Ok(true) };
    let ok = constant_time_eq(&hash_pin(pin, &salt), &hash);
    let mut failures = state.lock_failures.lock().unwrap();
    if ok {
        *failures = (0, None);
    } else {
        failures.0 += 1;
        if failures.0 >= 5 {
            let wait = 30u64 << (failures.0 - 5).min(5);
            failures.1 = Some(Instant::now() + Duration::from_secs(wait));
        }
    }
    Ok(ok)
}

#[tauri::command]
pub fn lock_status(state: S<'_>) -> Value {
    let p = &state.store.lock().unwrap().settings.privacy;
    json!({ "has_pin": p.lock_pin_hash.is_some(), "lock_after_minutes": p.lock_after_minutes, "lock_on_start": p.lock_on_start })
}

#[tauri::command]
pub async fn set_lock_pin(state: S<'_>, current: Option<String>, pin: String) -> R<()> {
    if pin.chars().count() < 4 || pin.len() > 64 {
        return Err("use 4–64 characters".into());
    }
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        if !check_pin(&state, current.as_deref().unwrap_or(""))? {
            return Err("current PIN is wrong".to_string());
        }
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).map_err(err)?;
        let salt: String = salt.iter().map(|b| format!("{b:02x}")).collect();
        let hash = hash_pin(&pin, &salt);
        {
            let mut store = state.store.lock().unwrap();
            store.settings.privacy.lock_pin_hash = Some(hash);
            store.settings.privacy.lock_salt = Some(salt);
        }
        state.save()
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn clear_lock_pin(state: S<'_>, pin: String) -> R<()> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        if !check_pin(&state, &pin)? {
            return Err("wrong PIN".to_string());
        }
        {
            let mut store = state.store.lock().unwrap();
            store.settings.privacy.lock_pin_hash = None;
            store.settings.privacy.lock_salt = None;
        }
        state.save()
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn verify_lock_pin(state: S<'_>, pin: String) -> R<bool> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || check_pin(&state, &pin)).await.map_err(err)?
}

#[tauri::command]
pub fn set_lock_options(state: S<'_>, lock_after_minutes: u32, lock_on_start: bool) -> R<()> {
    {
        let mut store = state.store.lock().unwrap();
        store.settings.privacy.lock_after_minutes = lock_after_minutes.min(24 * 60);
        store.settings.privacy.lock_on_start = lock_on_start;
    }
    state.save()
}

// ---- Plugin storage ---------------------------------------------------------------------

const PLUGIN_DATA_LIMIT: usize = 2 * 1024 * 1024;

fn storage_path(state: &AppState, id: &str) -> R<std::path::PathBuf> {
    let plugins = state.plugins.lock().unwrap();
    let info = plugins
        .list(&state.store.lock().unwrap().settings)
        .into_iter()
        .find(|p| p.manifest.id == id)
        .ok_or_else(|| format!("unknown plugin `{id}`"))?;
    if !info.manifest.permissions.contains(&Permission::Storage) {
        return Err(format!("plugin `{id}` did not declare the storage permission"));
    }
    if !info.enabled {
        return Err(format!("plugin `{id}` is not enabled"));
    }
    let dir = state.dir.join("plugin-data");
    std::fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir.join(format!("{}.json", info.manifest.id)))
}

#[tauri::command]
pub fn plugin_storage_get(state: S<'_>, id: String) -> R<Value> {
    let path = storage_path(&state, &id)?;
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(err),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(err(e)),
    }
}

#[tauri::command]
pub fn plugin_storage_set(state: S<'_>, id: String, value: Value) -> R<()> {
    let path = storage_path(&state, &id)?;
    let bytes = serde_json::to_vec(&value).map_err(err)?;
    if bytes.len() > PLUGIN_DATA_LIMIT {
        return Err(format!("plugin data is limited to {} MB", PLUGIN_DATA_LIMIT / 1024 / 1024));
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(tmp, path).map_err(err)
}

// ---- Diagnostics & testing center ---------------------------------------------------------

fn dir_size(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

fn memory_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb * 1024)
}

#[tauri::command]
pub async fn get_diagnostics(state: S<'_>) -> R<Value> {
    let tg = state.tg.lock().await.clone();
    let (connected, authorized, ping_ms) = match &tg {
        Some(tg) => {
            let authorized = tg.is_authorized().await;
            let ping = tokio::time::timeout(Duration::from_secs(5), tg.ping()).await;
            let ping_ms = match ping {
                Ok(Ok(d)) => Some(d.as_secs_f64() * 1000.0),
                _ => None,
            };
            (true, authorized, ping_ms)
        }
        None => (false, false, None),
    };
    let c = &state.counters;
    let plugins = {
        let store = state.store.lock().unwrap();
        state.plugins.lock().unwrap().list(&store.settings)
    };
    let enabled = plugins.iter().filter(|p| p.enabled).count();
    let plugin_stats: Vec<Value> = plugins
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            json!({
                "id": p.manifest.id, "name": p.manifest.name, "calls": p.stats.calls,
                "avg_us": if p.stats.calls > 0 { p.stats.total_us / p.stats.calls } else { 0 },
                "max_us": p.stats.max_us, "start_us": p.stats.start_us, "errors": p.errors.len(),
            })
        })
        .collect();
    let themes = state.themes.list().map(|t| t.len()).unwrap_or(0);
    Ok(json!({
        "uptime_s": state.started.elapsed().as_secs(),
        "safe_mode": state.safe_mode,
        "unclean_shutdown": state.unclean_shutdown,
        "telegram": { "configured": connected, "authorized": authorized, "ping_ms": ping_ms },
        "counters": {
            "messages_received": c.messages.load(Ordering::Relaxed),
            "history_loads": c.history_loads.load(Ordering::Relaxed),
            "sent": c.sends.load(Ordering::Relaxed),
            "send_failures": c.send_failures.load(Ordering::Relaxed),
            "sends_stopped_by_plugins": c.cancelled_sends.load(Ordering::Relaxed),
            "disconnects": c.disconnects.load(Ordering::Relaxed),
        },
        "plugins": { "installed": plugins.len(), "enabled": enabled, "stats": plugin_stats },
        "themes": themes,
        "storage_bytes": dir_size(&state.dir),
        "memory_bytes": memory_bytes(),
        "platform": format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub name: &'static str,
    /// "pass", "warn", "fail" or "skip".
    pub status: &'static str,
    pub detail: String,
    pub ms: f64,
}

fn test(name: &'static str, f: impl FnOnce() -> (&'static str, String)) -> TestResult {
    let t = Instant::now();
    let (status, detail) = f();
    TestResult { name, status, detail, ms: t.elapsed().as_secs_f64() * 1000.0 }
}

fn braces_balanced(css: &str) -> bool {
    let mut depth = 0i32;
    let mut in_comment = false;
    let b = css.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if in_comment {
            if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                in_comment = false;
                i += 1;
            }
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            in_comment = true;
            i += 1;
        } else if b[i] == b'{' {
            depth += 1;
        } else if b[i] == b'}' {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
        i += 1;
    }
    depth == 0 && !in_comment
}

#[tauri::command]
pub async fn run_tests(state: S<'_>) -> R<Vec<TestResult>> {
    let mut out = Vec::new();
    let dir = state.dir.clone();

    out.push(test("Local storage", || {
        let probe = dir.join(".write-test");
        match std::fs::write(&probe, b"ok").and_then(|_| std::fs::read(&probe)) {
            Ok(b) if b == b"ok" => {
                let _ = std::fs::remove_file(probe);
                ("pass", format!("{} is writable", dir.display()))
            }
            Ok(_) => ("fail", "read back different data".into()),
            Err(e) => ("fail", e.to_string()),
        }
    }));

    out.push(test("Settings integrity", || match std::fs::read_to_string(dir.join("settings.json")) {
        Ok(text) => match serde_json::from_str::<mellogram_core::Settings>(&text) {
            Ok(_) => ("pass", format!("{} bytes, valid", text.len())),
            Err(e) => ("fail", format!("settings.json does not parse: {e}")),
        },
        Err(_) => ("warn", "no settings saved yet".into()),
    }));

    let themes = state.themes.list().unwrap_or_default();
    out.push(test("Theme validation", || {
        let bad: Vec<String> = themes.iter().filter(|t| !braces_balanced(&t.css)).map(|t| t.file.clone()).collect();
        let unnamed = themes.iter().filter(|t| t.meta.name.is_none()).count();
        if !bad.is_empty() {
            ("fail", format!("unbalanced braces in {}", bad.join(", ")))
        } else if unnamed > 0 {
            ("warn", format!("{} theme(s) have no @name header", unnamed))
        } else {
            ("pass", format!("{} theme(s) valid", themes.len()))
        }
    }));

    let quick_css = state.store.lock().unwrap().settings.themes.quick_css.clone();
    out.push(test("QuickCSS", || {
        if braces_balanced(&quick_css) {
            ("pass", format!("{} bytes", quick_css.len()))
        } else {
            ("warn", "QuickCSS has unbalanced braces; rules after the error are ignored".into())
        }
    }));

    let (plugins, settings) = {
        let store = state.store.lock().unwrap();
        (state.plugins.lock().unwrap().list(&store.settings), store.settings.clone())
    };
    out.push(test("Plugin health", || {
        let failing: Vec<&str> = plugins.iter().filter(|p| !p.errors.is_empty()).map(|p| p.manifest.name).collect();
        if failing.is_empty() {
            ("pass", format!("{} enabled, no errors", plugins.iter().filter(|p| p.enabled).count()))
        } else {
            ("fail", format!("errors in: {}", failing.join(", ")))
        }
    }));

    out.push(test("Plugin dependencies", || {
        let missing: Vec<String> = plugins
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| {
                p.manifest
                    .dependencies
                    .iter()
                    .filter(|d| !plugins.iter().any(|q| q.manifest.id == **d && q.enabled))
                    .map(move |d| format!("{} → {d}", p.manifest.id))
            })
            .collect();
        if missing.is_empty() { ("pass", "all satisfied".into()) } else { ("fail", missing.join(", ")) }
    }));

    out.push(test("Plugin permissions", || {
        let outgoing: Vec<&str> = plugins
            .iter()
            .filter(|p| p.enabled && p.manifest.permissions.contains(&Permission::ModifyOutgoing))
            .map(|p| p.manifest.name)
            .collect();
        ("pass", format!("{} enabled plugin(s) may change outgoing messages: {}", outgoing.len(), outgoing.join(", ")))
    }));

    out.push(test("Plugin performance", || {
        let slow: Vec<String> = plugins
            .iter()
            .filter(|p| p.stats.calls > 0 && (p.stats.total_us / p.stats.calls > 5_000 || p.stats.max_us > 100_000))
            .map(|p| format!("{} (avg {}µs, max {}µs)", p.manifest.name, p.stats.total_us / p.stats.calls, p.stats.max_us))
            .collect();
        if slow.is_empty() { ("pass", "no slow hooks this session".into()) } else { ("warn", slow.join(", ")) }
    }));

    {
        let plugins = state.plugins.lock().unwrap();
        out.push(test("Send pipeline benchmark", || {
            let sample = OutgoingMessage::text(0, "teh quick test https://example.com/?utm_source=x ```\n    code\n```");
            let n = 500;
            let t = Instant::now();
            for _ in 0..n {
                let _ = plugins.process_outgoing(sample.clone());
            }
            let per = t.elapsed().as_micros() as f64 / n as f64;
            (if per < 1000.0 { "pass" } else { "warn" }, format!("{per:.1} µs per message through all enabled plugins"))
        }));
    }

    out.push(test("Workspace integrity", || match &settings.active_workspace {
        Some(id) if !settings.workspaces.iter().any(|w| &w.id == id) => ("fail", format!("active workspace `{id}` is missing")),
        Some(id) => ("pass", format!("active: {id}, {} total", settings.workspaces.len())),
        None => ("pass", format!("no active workspace, {} saved", settings.workspaces.len())),
    }));

    out.push(test("Shutdown state", || {
        if state.safe_mode {
            ("warn", "running in safe mode".into())
        } else if state.unclean_shutdown {
            ("warn", "the previous session did not exit cleanly".into())
        } else {
            ("pass", "previous session exited cleanly".into())
        }
    }));

    out.push(test("Telegram configuration", || match (settings.telegram.api_id, &settings.telegram.api_hash) {
        (Some(_), Some(_)) => ("pass", "API credentials saved".into()),
        _ => ("fail", "no API ID / hash yet".into()),
    }));

    let t = Instant::now();
    let dns = tauri::async_runtime::spawn_blocking(|| {
        use std::net::ToSocketAddrs;
        "api.telegram.org:443".to_socket_addrs().map(|a| a.count())
    })
    .await
    .map_err(err)?;
    out.push(TestResult {
        name: "DNS resolution",
        status: if dns.is_ok() { "pass" } else { "fail" },
        detail: match dns {
            Ok(n) => format!("api.telegram.org → {n} address(es)"),
            Err(e) => e.to_string(),
        },
        ms: t.elapsed().as_secs_f64() * 1000.0,
    });

    let tg = state.tg.lock().await.clone();
    let t = Instant::now();
    match tg {
        None => out.push(TestResult { name: "Telegram connectivity", status: "skip", detail: "not configured".into(), ms: 0.0 }),
        Some(tg) => {
            let authorized = tg.is_authorized().await;
            out.push(TestResult {
                name: "Telegram authorization",
                status: if authorized { "pass" } else { "warn" },
                detail: if authorized { "signed in".into() } else { "not signed in".into() },
                ms: t.elapsed().as_secs_f64() * 1000.0,
            });
            let t = Instant::now();
            let ping = tokio::time::timeout(Duration::from_secs(8), tg.ping()).await;
            out.push(TestResult {
                name: "Telegram connectivity",
                status: match &ping {
                    Ok(Ok(d)) if d.as_millis() < 1500 => "pass",
                    Ok(Ok(_)) => "warn",
                    _ => "fail",
                },
                detail: match ping {
                    Ok(Ok(d)) => format!("MTProto ping {} ms", d.as_millis()),
                    Ok(Err(e)) => e.to_string(),
                    Err(_) => "timed out after 8 s".into(),
                },
                ms: t.elapsed().as_secs_f64() * 1000.0,
            });
        }
    }

    out.push(test("Disk usage", || {
        let bytes = dir_size(&dir);
        (if bytes < 500 * 1024 * 1024 { "pass" } else { "warn" }, format!("{:.1} MB in the config folder", bytes as f64 / 1_048_576.0))
    }));

    Ok(out)
}

/// Everything useful for a bug report, with secrets removed.
#[tauri::command]
pub async fn support_bundle(state: S<'_>) -> R<String> {
    let diagnostics = get_diagnostics(state.clone()).await?;
    let tests = run_tests(state.clone()).await?;
    let mut settings = serde_json::to_value(&state.store.lock().unwrap().settings).map_err(err)?;
    settings["telegram"] = json!({ "redacted": true });
    settings["privacy"]["lock_pin_hash"] = json!(null);
    settings["privacy"]["lock_salt"] = json!(null);
    let plugins: Vec<Value> = {
        let store = state.store.lock().unwrap();
        state
            .plugins
            .lock()
            .unwrap()
            .list(&store.settings)
            .into_iter()
            .map(|p| json!({ "id": p.manifest.id, "version": p.manifest.version, "enabled": p.enabled, "errors": p.errors }))
            .collect()
    };
    serde_json::to_string_pretty(&json!({
        "format": "mellogram-support-bundle",
        "generated_at": chrono_now(),
        "diagnostics": diagnostics,
        "tests": tests,
        "plugins": plugins,
        "settings": settings,
        "note": "Telegram credentials, session and app-lock PIN are never included.",
    }))
    .map_err(err)
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("unix:{secs}")
}

// ---- Safe mode & restart ---------------------------------------------------------------

#[tauri::command]
pub fn startup_status(state: S<'_>) -> Value {
    let lock = &state.store.lock().unwrap().settings.privacy;
    json!({
        "safe_mode": state.safe_mode,
        "unclean_shutdown": state.unclean_shutdown,
        "lock_on_start": lock.lock_on_start && lock.lock_pin_hash.is_some(),
        "lock_after_minutes": if lock.lock_pin_hash.is_some() { lock.lock_after_minutes } else { 0 },
        "mobile": cfg!(any(target_os = "android", target_os = "ios")),
    })
}

/// Restarts the app, optionally into safe mode.
#[tauri::command]
pub fn restart_app(state: S<'_>, safe_mode: bool) -> R<()> {
    if safe_mode {
        std::fs::write(state.dir.join("safe-mode"), b"1").map_err(err)?;
    }
    state.shutdown();
    state.app.restart();
}
