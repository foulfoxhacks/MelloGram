# Android Firebase Messaging

The Android build workflow adds `MelloGramMessagingService` to the generated
Android manifest and adds the Firebase Messaging SDK dependency after Tauri
creates the native Android project. The service receives FCM message and token
refresh callbacks; it currently logs event metadata only and does not display
notifications or send tokens to a backend.

To enable live Firebase delivery, configure a Firebase project for the Android
application ID `app.mellogram.desktop` and provide its `google-services.json`.
The Google Services Gradle plugin and Firebase project configuration are not
committed because they are project-specific credentials/configuration. Token
registration, notification presentation, and backend delivery remain separate
integration work.
