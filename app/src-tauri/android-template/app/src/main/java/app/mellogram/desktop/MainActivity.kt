package app.mellogram.desktop

import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

/**
 * MelloGram's Android activity (copied over the Tauri-generated one by
 * scripts/configure-android.py).
 *
 * The app draws edge to edge, but the WebView itself is padded by the system
 * bars, display cutout and on-screen keyboard. That keeps the header out of
 * the status bar, the composer above the gesture bar, and the composer above
 * the keyboard on every Android version, without relying on the WebView's
 * env(safe-area-inset-*) support (which older WebViews lack).
 */
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Matches --mg-bg-2 so the bar areas blend into the header and composer.
    val barColor = Color.parseColor("#1E1D29")
    window.decorView.setBackgroundColor(barColor)
    WindowCompat.getInsetsController(window, window.decorView).apply {
      isAppearanceLightStatusBars = false
      isAppearanceLightNavigationBars = false
    }

    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }
}
