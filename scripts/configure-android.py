#!/usr/bin/env python3
"""Patch the Tauri-generated Android project for MelloGram.

Run after `cargo tauri android init` (CI does this on every build; the
generated project in app/src-tauri/gen/android is not committed):

  * launcher icons from app/src-tauri/icons/android (MelloGram logo, adaptive
    and themed icons) replace the Tauri defaults;
  * AndroidManifest.xml gets exactly the permissions MelloGram uses
    (see docs/ANDROID.md) and an IME-aware main activity;
  * MainActivity.kt is replaced by the MelloGram one, which keeps the WebView
    clear of the status bar, gesture bar, display cutout and keyboard.

Idempotent: running it twice leaves the project unchanged.
"""
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]
TAURI = ROOT / "app/src-tauri"
ANDROID = TAURI / "gen/android"
MAIN = ANDROID / "app/src/main"
MANIFEST = MAIN / "AndroidManifest.xml"
PACKAGE_DIR = Path("java/app/mellogram/desktop")

TOOLS_NS = 'xmlns:tools="http://schemas.android.com/tools"'

# Permissions MelloGram requests. Location is only asked for at runtime when
# the user shares a location; notifications are asked for once on Android 13+.
PERMISSIONS = [
    "android.permission.INTERNET",
    "android.permission.POST_NOTIFICATIONS",
    "android.permission.ACCESS_COARSE_LOCATION",
    "android.permission.ACCESS_FINE_LOCATION",
]
# Merged in by tauri-plugin-notification for re-scheduling notifications after
# a reboot. MelloGram never schedules notifications, so drop it.
REMOVED_PERMISSIONS = [
    "android.permission.RECEIVE_BOOT_COMPLETED",
]


def fail(message: str) -> None:
    raise SystemExit(f"configure-android: {message}")


def copy_icons() -> int:
    source = TAURI / "icons/android"
    if not source.is_dir():
        fail(f"{source} is missing; run branding/make_android_icons.py")
    res = MAIN / "res"
    count = 0
    for file in sorted(source.rglob("*")):
        if file.is_file():
            target = res / file.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, target)
            count += 1
    return count


def patch_manifest(text: str) -> str:
    if "xmlns:tools=" not in text:
        text = re.sub(r"<manifest\b", f"<manifest {TOOLS_NS}", text, count=1)

    lines = []
    for perm in PERMISSIONS:
        if f'android:name="{perm}"' not in text:
            lines.append(f'    <uses-permission android:name="{perm}" />')
    for perm in REMOVED_PERMISSIONS:
        if f'android:name="{perm}"' not in text:
            lines.append(f'    <uses-permission android:name="{perm}" tools:node="remove" />')
    if 'android.hardware.location"' not in text:
        # Location permissions imply a required GPS feature; keep the app
        # installable on devices without one.
        lines.append('    <uses-feature android:name="android.hardware.location" android:required="false" />')
        lines.append('    <uses-feature android:name="android.hardware.location.gps" android:required="false" />')
        lines.append('    <uses-feature android:name="android.hardware.location.network" android:required="false" />')
    if lines:
        text = re.sub(r"(<manifest\b[^>]*>\n?)", lambda m: m.group(1) + "\n".join(lines) + "\n", text, count=1)

    if "android:roundIcon" not in text:
        text = text.replace('android:icon="@mipmap/ic_launcher"', 'android:icon="@mipmap/ic_launcher"\n        android:roundIcon="@mipmap/ic_launcher_round"', 1)

    # Resize the activity for the soft keyboard (MainActivity also pads the
    # WebView by the IME inset for edge-to-edge on Android 11+).
    if "android:windowSoftInputMode" not in text:
        text, n = re.subn(r'(android:name="\.MainActivity")', r'\1\n            android:windowSoftInputMode="adjustResize"', text, count=1)
        if n != 1:
            fail("could not find .MainActivity in AndroidManifest.xml")
    return text


def main() -> None:
    if not MANIFEST.exists():
        fail("Tauri Android project is missing; run `cargo tauri android init --ci` first")

    icons = copy_icons()

    text = MANIFEST.read_text(encoding="utf-8")
    MANIFEST.write_text(patch_manifest(text), encoding="utf-8")

    activity_src = TAURI / "android-template/app/src/main" / PACKAGE_DIR / "MainActivity.kt"
    activity_dst = MAIN / PACKAGE_DIR / "MainActivity.kt"
    if not activity_dst.exists():
        fail(f"generated {activity_dst.relative_to(ROOT)} not found; did the app identifier change?")
    shutil.copy2(activity_src, activity_dst)

    print(f"configure-android: copied {icons} icon files, patched AndroidManifest.xml and MainActivity.kt")


if __name__ == "__main__":
    main()
