#!/usr/bin/env python3
"""Patch the Tauri-generated Android project with MelloGram FCM support."""
from pathlib import Path
import re
import shutil

root = Path(__file__).resolve().parents[1]
android = root / "app/src-tauri/gen/android"
manifest = android / "app/src/main/AndroidManifest.xml"
gradle = next((p for p in (android / "app/build.gradle.kts", android / "app/build.gradle") if p.exists()), android / "app/build.gradle.kts")

if not manifest.exists() or not gradle.exists():
    raise SystemExit("Tauri Android project is missing; run `cargo tauri android init --ci` first")

manifest_text = manifest.read_text(encoding="utf-8")
service = """        <service
            android:name="app.mellogram.desktop.MelloGramMessagingService"
            android:exported="false">
            <intent-filter>
                <action android:name="com.google.firebase.MESSAGING_EVENT" />
            </intent-filter>
        </service>
"""
if "MelloGramMessagingService" not in manifest_text:
    manifest_text = manifest_text.replace("</application>", service + "    </application>")
manifest.write_text(manifest_text, encoding="utf-8")

gradle_text = gradle.read_text(encoding="utf-8")
if "firebase-messaging" not in gradle_text:
    if gradle.suffix == ".kts":
        dependency = '\n    implementation(platform("com.google.firebase:firebase-bom:33.7.0"))\n    implementation("com.google.firebase:firebase-messaging")\n'
    else:
        dependency = "\n    implementation platform('com.google.firebase:firebase-bom:33.7.0')\n    implementation 'com.google.firebase:firebase-messaging'\n"
    match = re.search(r"dependencies\s*\{", gradle_text)
    if not match:
        raise SystemExit("Could not find dependencies block in generated app/build.gradle")
    insert_at = match.end()
    gradle_text = gradle_text[:insert_at] + dependency + gradle_text[insert_at:]
gradle.write_text(gradle_text, encoding="utf-8")

source = root / "app/src-tauri/android-template/app/src/main/java/app/mellogram/desktop/MelloGramMessagingService.java"
destination = android / "app/src/main/java/app/mellogram/desktop/MelloGramMessagingService.java"
destination.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(source, destination)
print("Configured MelloGram FirebaseMessagingService and Firebase Messaging dependency")
