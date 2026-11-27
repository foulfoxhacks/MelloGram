package app.mellogram.desktop;

import android.util.Log;
import com.google.firebase.messaging.FirebaseMessagingService;
import com.google.firebase.messaging.RemoteMessage;

/** Receives Firebase Cloud Messaging events for MelloGram. */
public final class MelloGramMessagingService extends FirebaseMessagingService {
    private static final String TAG = "MelloGramFCM";

    @Override
    public void onMessageReceived(RemoteMessage message) {
        super.onMessageReceived(message);
        Log.i(TAG, "Received FCM message; messageId=" + message.getMessageId());
        if (message.getNotification() != null) {
            Log.i(TAG, "Notification payload received");
        }
        if (!message.getData().isEmpty()) {
            Log.i(TAG, "Data payload received with " + message.getData().size() + " fields");
        }
    }

    @Override
    public void onNewToken(String token) {
        super.onNewToken(token);
        // Token registration with the MelloGram account backend is intentionally
        // left to the application layer; do not log or persist the token here.
        Log.i(TAG, "FCM registration token refreshed");
    }
}
