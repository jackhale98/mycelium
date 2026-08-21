package com.mycelium.plugins.folderpicker

import android.app.Activity
import android.content.res.Configuration
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

/**
 * Draws the app behind the system bars and tells the web layer how much room
 * they take.
 *
 * Android 15 makes edge-to-edge the default for apps targeting SDK 35, so this
 * is not decoration: without it the header would sit under the status bar and
 * the bottom navigation under the gesture bar. iOS solves this with the
 * `env(safe-area-inset-*)` CSS variables, which an Android WebView does not
 * populate, so the same numbers are pushed in as custom properties and the
 * stylesheet prefers them where present.
 *
 * The bar icons are also flipped to suit the theme — dark icons on a light app,
 * light on a dark one — which is the difference between looking native and
 * looking like a web page in a frame.
 */
class SystemInsets(private val activity: Activity) {

    private var installed = false

    fun install(webView: WebView) {
        if (installed) return
        installed = true

        val window = activity.window
        WindowCompat.setDecorFitsSystemWindows(window, false)

        val night = activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        WindowInsetsControllerCompat(window, window.decorView).apply {
            // Light *appearance* means dark icons, for a light background.
            isAppearanceLightStatusBars = !night
            isAppearanceLightNavigationBars = !night
        }

        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            // The bars and the display cutout together: a notch eats room the
            // status bar inset alone does not account for in landscape.
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            val density = activity.resources.displayMetrics.density
            fun px(value: Int) = (value / density).toInt()

            publish(webView, px(bars.top), px(bars.right), px(bars.bottom), px(bars.left))
            insets
        }
        ViewCompat.requestApplyInsets(webView)
    }

    private fun publish(webView: WebView, top: Int, right: Int, bottom: Int, left: Int) {
        val script = """
            (function () {
              var s = document.documentElement.style;
              s.setProperty('--android-inset-top', '${top}px');
              s.setProperty('--android-inset-right', '${right}px');
              s.setProperty('--android-inset-bottom', '${bottom}px');
              s.setProperty('--android-inset-left', '${left}px');
            })();
        """.trimIndent()
        webView.post { webView.evaluateJavascript(script, null) }
    }
}
