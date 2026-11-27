# Telegram API credentials

MelloGram includes the project's Telegram API ID and API hash as built-in defaults.
Fresh installations are preconfigured automatically; users do not need to enter
these values or configure GitHub Actions secrets.

For development or an alternate distribution, compile-time environment variables
may override the built-in defaults:

```powershell
$env:MELLOGRAM_TELEGRAM_API_ID = "<your-api-id>"
$env:MELLOGRAM_TELEGRAM_API_HASH = "<your-api-hash>"
cd app/src-tauri
cargo tauri build
```

Existing per-user Telegram settings take precedence over the built-in defaults.
Telegram credentials are not included in MelloGram backups or workspace profiles.
