#!/usr/bin/env bash
# Build MelloGram installers and get them onto GitHub Releases.
#
#   scripts/release.sh local [linux|macos|windows|android]   build on this machine
#   scripts/release.sh ci v0.2.0 [--no-wait]                  tag, push, then fetch CI packages
#   scripts/release.sh assets v0.2.0                          download packages for an existing tag
#   scripts/release.sh publish v0.2.0                         create/update the GitHub release
#
# Add --dry-run to any command to print the exact commands without running them.
#
# Platforms that a single machine cannot build (Windows installers on Linux, a
# universal macOS DMG on Windows, Android on macOS without a JDK/SDK) go through
# CI: the workflow builds every target on a native runner and the `release` job
# attaches all of them to the tag. `local` exists for quick single-platform
# builds and for machines that already have the Tauri toolchain installed.
#
# Requirements:
#   local linux/macos/windows   Rust 1.85+, cargo-tauri 2, the Tauri OS
#                               dependencies, and Inno Setup for the Windows .exe
#   local android               JDK 17, Android SDK/NDK, cargo-tauri with the
#                               Android targets
#   ci / assets / publish       git and the GitHub CLI (gh), authenticated
#
# Nothing here writes credentials or uploads anything unless you run `publish`
# or `ci` (and `ci` only pushes the tag you name).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_DEFAULT="foulfoxhacks/MelloGram"
REPO="${MELLOGRAM_REPO:-$REPO_DEFAULT}"
DRY_RUN=0
WAIT=1
PLATFORM="auto"
WORKFLOW="mellogram.yml"

say()  { printf '\033[1;35m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

run() {
  if [ "$DRY_RUN" = 1 ]; then
    printf '    %s\n' "$*"
  else
    "$@"
  fi
}

need() {
  [ "$DRY_RUN" = 1 ] && return 0
  command -v "$1" >/dev/null 2>&1 || die "$1 is required for this command"
}

usage() {
  sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
}

version_of() {
  grep -m1 '^version[[:space:]]*=' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/'
}

stage_dir() { echo "$ROOT/dist/release-$1"; }

host_platform() {
  case "$(uname -s)" in
    Linux*) echo linux ;;
    Darwin*) echo macos ;;
    MINGW*|MSYS*|CYGWIN*) echo windows ;;
    *) die "unsupported host: $(uname -s)" ;;
  esac
}

# ---- local builds ----------------------------------------------------------

build_linux() {
  need cargo
  say "Building Linux packages (.deb, .rpm, AppImage)"
  ( cd "$ROOT/app/src-tauri" && run cargo tauri build --bundles deb,rpm,appimage )
  local out="$ROOT/target/release/bundle"
  run mkdir -p "$1"
  run cp "$out"/deb/*.deb              "$1/MelloGram-linux-amd64.deb"
  run cp "$out"/rpm/*.rpm              "$1/MelloGram-linux-x86_64.rpm"
  run cp "$out"/appimage/*.AppImage    "$1/MelloGram-linux-x86_64.AppImage"
}

build_macos() {
  need cargo
  say "Building the universal macOS app and DMG"
  ( cd "$ROOT/app/src-tauri" && run cargo tauri build --target universal-apple-darwin --bundles app,dmg )
  run mkdir -p "$1"
  run cp "$ROOT"/target/universal-apple-darwin/release/bundle/dmg/*.dmg "$1/MelloGram-macos-universal.dmg"
}

build_windows() {
  need cargo
  say "Building the MSI and the Inno Setup installer"
  ( cd "$ROOT/app/src-tauri" && run cargo tauri build --bundles msi )
  local iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
  if [ "$DRY_RUN" != 1 ] && [ ! -x "$iscc" ]; then
    die "Inno Setup 6 is not installed; install it from https://jrsoftware.org/isdl.php (the MSI alone is in target/release/bundle/msi)"
  fi
  local version
  version="$(version_of)"
  ( cd "$ROOT/app/src-tauri" && run "$iscc" "/DAppVersion=$version" "/DSourceDir=$(pwd -W 2>/dev/null || pwd)\\target\\release" installer/windows/mellogram.iss )
  run mkdir -p "$1"
  run cp "$ROOT/app/src-tauri/installer/windows/Output/MelloGram-Setup-x64.exe" "$1/MelloGram-Setup-x64.exe"
  run cp "$ROOT/app/src-tauri/target/release/bundle/msi/"*.msi "$1/MelloGram-windows-x64.msi"
}

build_android() {
  need cargo
  [ "$DRY_RUN" = 1 ] || [ -n "${ANDROID_HOME:-}${ANDROID_SDK_ROOT:-}" ] || die "set ANDROID_HOME (or ANDROID_SDK_ROOT) to the Android SDK"
  say "Building the Android APK and AAB"
  ( cd "$ROOT/app/src-tauri" && run cargo tauri android build --apk --aab )
  local gen="$ROOT/app/src-tauri/gen/android/app/build/outputs"
  run mkdir -p "$1"
  run cp "$gen"/apk/universal/release/*.apk "$1/MelloGram-android.apk"
  run cp "$gen"/bundle/release/*.aab      "$1/MelloGram-android-unsigned.aab"
}

cmd_local() {
  local platform="$PLATFORM"
  [ "$platform" = auto ] && platform="$(host_platform)"
  local dest
  dest="$(stage_dir "$(version_of)")"
  say "MelloGram $(version_of) · $platform · $dest"
  case "$platform" in
    linux) build_linux "$dest" ;;
    macos) build_macos "$dest" ;;
    windows) build_windows "$dest" ;;
    android) build_android "$dest" ;;
    *) die "unknown platform: $platform (linux, macos, windows, android)" ;;
  esac
  list_assets "$dest"
}

# ---- CI builds -------------------------------------------------------------

cmd_ci() {
  local version="$1"
  need git
  [ "$DRY_RUN" = 1 ] || git -C "$ROOT" diff --quiet || die "commit your changes before tagging (git status)"
  say "Tagging $version and pushing it; the workflow publishes every platform to the release"
  run git -C "$ROOT" tag -a "$version" -m "MelloGram $version"
  run git -C "$ROOT" push origin "refs/tags/$version"
  if [ "$DRY_RUN" = 1 ]; then
    say "Would wait for the run, download assets and leave publishing to the release job"
    return
  fi
  if [ "$WAIT" = 1 ]; then
    wait_for_run "$version"
    cmd_assets "$version"
  else
    say "Not waiting. Fetch later with: scripts/release.sh assets $version"
  fi
}

wait_for_run() {
  need gh
  local version="$1" run_id="" i
  say "Waiting for the workflow run for $version"
  for i in $(seq 1 120); do
    run_id="$(gh run list --repo "$REPO" --workflow "$WORKFLOW" --branch "$version" --limit 1 --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)"
    [ -n "$run_id" ] && break
    sleep 10
  done
  [ -n "$run_id" ] || die "no workflow run appeared for $version; check 'gh run list --repo $REPO'"
  gh run watch "$run_id" --repo "$REPO" --exit-status || die "the workflow for $version did not succeed; inspect it with 'gh run view $run_id --repo $REPO'"
}

cmd_assets() {
  local version="$1"
  local dest
  dest="$(stage_dir "$version")"
  if [ "$DRY_RUN" = 1 ]; then
    say "Downloading the $version artifacts into $dest"
    printf '    gh run download --repo %s --dir %s <run-id>\n' "$REPO" "$dest"
    return
  fi
  need gh
  run mkdir -p "$dest"
  local run_id
  run_id="$(gh run list --repo "$REPO" --workflow "$WORKFLOW" --branch "$version" --limit 1 --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)"
  [ -n "$run_id" ] || die "no workflow run found for $version; pass a tag that was pushed through this script"
  say "Downloading artifacts from run $run_id"
  gh run download "$run_id" --repo "$REPO" --dir "$dest"
  list_assets "$dest"
}

# ---- publishing ------------------------------------------------------------

cmd_publish() {
  local version="$1"
  local dest
  dest="$(stage_dir "$version")"
  [ -d "$dest" ] || die "no staged packages in $dest — run 'ci $version' or 'assets $version' first"
  local files=()
  while IFS= read -r file; do files+=("$file"); done < <(find "$dest" -type f \( -name '*.exe' -o -name '*.msi' -o -name '*.dmg' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.apk' -o -name '*.aab' \) | sort)
  [ "${#files[@]}" -gt 0 ] || die "no installer files found in $dest"
  need gh
  if gh release view "$version" --repo "$REPO" >/dev/null 2>&1; then
    say "Uploading ${#files[@]} file(s) to the existing $version release"
    run gh release upload "$version" "${files[@]}" --repo "$REPO" --clobber
  else
    say "Creating the $version release with ${#files[@]} file(s)"
    run gh release create "$version" "${files[@]}" --repo "$REPO" --title "MelloGram $version" --generate-notes
  fi
}

list_assets() {
  local dest="$1"
  [ "$DRY_RUN" = 1 ] && return 0
  say "Packages in $dest"
  if command -v du >/dev/null 2>&1; then
    find "$dest" -type f -size +0 -printf '%10s  %p\n' 2>/dev/null || find "$dest" -type f -exec ls -lh {} +
  fi
}

# ---- arguments -------------------------------------------------------------

COMMAND="${1:-help}"
[ $# -gt 0 ] && shift || true
ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --platform) PLATFORM="${2:-}"; shift 2 ;;
    --platform=*) PLATFORM="${1#*=}"; shift ;;
    --no-wait) WAIT=0; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --repo) REPO="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) ARGS+=("$1"); shift ;;
  esac
done

case "$COMMAND" in
  local) PLATFORM="${ARGS[0]:-$PLATFORM}"; cmd_local ;;
  ci) [ "${#ARGS[@]}" -gt 0 ] || die "usage: scripts/release.sh ci v0.2.0"; cmd_ci "${ARGS[0]}" ;;
  assets) [ "${#ARGS[@]}" -gt 0 ] || die "usage: scripts/release.sh assets v0.2.0"; cmd_assets "${ARGS[0]}" ;;
  publish) [ "${#ARGS[@]}" -gt 0 ] || die "usage: scripts/release.sh publish v0.2.0"; cmd_publish "${ARGS[0]}" ;;
  help|-h|--help) usage ;;
  *) usage; die "unknown command: $COMMAND" ;;
esac
