package com.mycelium.plugins.folderpicker

import android.app.Activity
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback

/**
 * Makes the system back gesture navigate the app instead of leaving it.
 *
 * Without this, back closes Mycelium from wherever you are — which is the
 * single most un-Android thing a WebView app does. Back is expected to retrace
 * your steps and only exit from the top.
 *
 * The decision belongs to the web layer, which is the only side that knows
 * where the user is, so each press asks it and acts on the answer. When it says
 * it did not handle the press we are at the root: the callback disables itself
 * and hands the press back to the system, which closes the app as it should.
 *
 * Registered through OnBackPressedDispatcher rather than by overriding
 * onBackPressed, which is deprecated and does not participate in the predictive
 * back animation on Android 13 and later.
 */
class BackHandler(private val activity: Activity) {

    private var callback: OnBackPressedCallback? = null

    fun install(webView: WebView) {
        if (callback != null) return
        val owner = activity as? ComponentActivity ?: return

        val handler = object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                webView.evaluateJavascript(BACK_SCRIPT) { result ->
                    if (result == "true") return@evaluateJavascript
                    // Nothing left to go back to: step aside and let the system
                    // finish the activity. Re-enabling afterwards would be racy
                    // and pointless, since the activity is on its way out.
                    isEnabled = false
                    owner.onBackPressedDispatcher.onBackPressed()
                }
            }
        }
        owner.onBackPressedDispatcher.addCallback(owner, handler)
        callback = handler
    }

    private companion object {
        /**
         * Returns "true" when the web layer consumed the press. Guarded because
         * a back press can arrive before the page that defines the hook has
         * loaded, and a missing hook should mean "not handled", not a crash.
         */
        const val BACK_SCRIPT =
            "(function(){try{return !!(window.__myceliumBack && window.__myceliumBack())}" +
                "catch(e){return false}})()"
    }
}
