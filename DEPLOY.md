# Deploy MelloGram to GitHub

The repository contents in this archive are laid out for the repository root.

## Push the project

```powershell
git clone https://github.com/foulfoxhacks/MelloGram.git
cd MelloGram
# Extract the contents of MelloGram-standalone.zip into this directory,
# allowing the archive's MelloGram/ folder contents to populate the repo root.
git add -A
git commit -m "Initial standalone MelloGram project"
git push -u origin main
```

## Build outputs

Every push and pull request runs the Rust tests and platform build jobs. GitHub
Actions artifacts contain Linux packages, Windows installers, macOS disk images,
and Android debug/release APKs plus an AAB. Pushing a `v*` tag additionally
creates a GitHub Release with the generated artifacts.

See [docs/FIREBASE_ANDROID.md](docs/FIREBASE_ANDROID.md) for Firebase setup notes.

Telegram API defaults are built into the application; no repository Actions secrets are required.
