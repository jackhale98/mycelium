package com.mycelium.plugins.folderpicker

import android.app.Activity
import android.app.AlertDialog
import android.app.DatePickerDialog
import android.content.res.Configuration
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.Button
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale

/**
 * Toolbar colours for one theme.
 *
 * These were hardcoded to their light values, so on a dark-themed device the
 * toolbar rendered as a bright bar above the keyboard with low-contrast labels.
 * The web UI solves this with CSS custom properties, which cannot reach a view
 * built in Kotlin, so the same palette is mirrored here. Hues match the app's
 * task-state tokens: a deadline is orange in the agenda and orange here.
 *
 * Every label pair measures at least 4.64:1 against its own background.
 */
private data class ToolbarPalette(
    val background: Int,
    val separator: Int,
    val label: Int,
    val link: Int,
    val id: Int,
    val todo: Int,
    val priority: Int,
    val tag: Int,
    val deadline: Int,
    val scheduled: Int,
) {
    companion object {
        val LIGHT = ToolbarPalette(
            background = Color.parseColor("#F2F2F7"),
            separator = Color.parseColor("#C7C7CC"),
            label = Color.parseColor("#1C1C1E"),
            link = Color.parseColor("#166534"),
            id = Color.parseColor("#BE185D"),
            todo = Color.parseColor("#B91C1C"),
            priority = Color.parseColor("#6D28D9"),
            tag = Color.parseColor("#0F766E"),
            deadline = Color.parseColor("#C2410C"),
            scheduled = Color.parseColor("#1D4ED8"),
        )

        val DARK = ToolbarPalette(
            background = Color.parseColor("#1C1C1E"),
            separator = Color.parseColor("#3A3A3C"),
            label = Color.parseColor("#E6EBF1"),
            link = Color.parseColor("#4ADE80"),
            id = Color.parseColor("#F9A8D4"),
            todo = Color.parseColor("#FCA5A5"),
            priority = Color.parseColor("#C4B5FD"),
            tag = Color.parseColor("#5EEAD4"),
            deadline = Color.parseColor("#FDBA74"),
            scheduled = Color.parseColor("#93C5FD"),
        )
    }
}

@TauriPlugin
class FolderPickerPlugin(private val activity: Activity) : Plugin(activity) {
    private var webView: WebView? = null
    private var toolbarView: View? = null

    /** Resolved per build, so following the device theme costs no extra state. */
    private val palette: ToolbarPalette
        get() {
            val night = activity.resources.configuration.uiMode and
                Configuration.UI_MODE_NIGHT_MASK
            return if (night == Configuration.UI_MODE_NIGHT_YES) {
                ToolbarPalette.DARK
            } else {
                ToolbarPalette.LIGHT
            }
        }

    override fun load(webView: WebView) {
        this.webView = webView
    }

    @Command
    fun pickFolder(invoke: Invoke) {
        // Android folder picking - stub for now
        val ret = JSObject()
        ret.put("path", null as String?)
        invoke.resolve(ret)
    }

    @Command
    fun restoreAccess(invoke: Invoke) {
        val ret = JSObject()
        ret.put("path", null as String?)
        invoke.resolve(ret)
    }

    @Command
    fun setupToolbar(invoke: Invoke) {
        val wv = webView
        if (wv == null) {
            val ret = JSObject()
            ret.put("installed", false)
            invoke.resolve(ret)
            return
        }

        Handler(Looper.getMainLooper()).post {
            installToolbar(wv)
            val ret = JSObject()
            ret.put("installed", true)
            invoke.resolve(ret)
        }
    }

    // ── Keyboard Toolbar ──────────────────────────────────────

    private fun installToolbar(wv: WebView) {
        if (toolbarView != null) return // already installed

        val toolbar = createToolbarView(wv)
        toolbar.visibility = View.GONE

        val rootView = activity.findViewById<FrameLayout>(android.R.id.content)
        val params = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            dpToPx(44)
        ).apply {
            gravity = Gravity.BOTTOM
        }
        rootView.addView(toolbar, params)
        toolbarView = toolbar

        // Detect keyboard via WindowInsetsCompat
        ViewCompat.setOnApplyWindowInsetsListener(rootView) { _, insets ->
            val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val imeHeight = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val navBarHeight = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom

            // The IME inset already covers the nav bar area the toolbar sits in, so
            // subtract it; a split-screen/floating IME can report less than the nav
            // bar, which would otherwise push the toolbar off-screen.
            val offset = (imeHeight - navBarHeight).coerceAtLeast(0)

            if (imeVisible && imeHeight > 0) {
                toolbar.visibility = View.VISIBLE
                toolbar.translationY = -offset.toFloat()
            } else {
                toolbar.visibility = View.GONE
                toolbar.translationY = 0f
            }
            insets
        }

        // Position correctly if the keyboard is already up when the toolbar installs
        ViewCompat.requestApplyInsets(rootView)
    }

    private fun createToolbarView(wv: WebView): View {
        val theme = palette
        val scroll = HorizontalScrollView(activity).apply {
            isHorizontalScrollBarEnabled = false
            setBackgroundColor(theme.background)
        }

        val container = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dpToPx(4), 0, dpToPx(4), 0)
            gravity = Gravity.CENTER_VERTICAL
        }

        data class BtnDef(val label: String, val action: String, val color: Int? = null, val bold: Boolean = false)

        val buttons = listOf(
            BtnDef("Link", "link", theme.link, true),
            BtnDef("|", ""),
            BtnDef("H", "heading", bold = true),
            BtnDef("ID", "makeNode", theme.id, true),
            BtnDef("TODO", "todo", theme.todo, true),
            BtnDef("[#]", "priority", theme.priority, true),
            BtnDef("Tag", "tag", theme.tag, true),
            BtnDef("DL", "deadline", theme.deadline),
            BtnDef("SC", "scheduled", theme.scheduled),
            BtnDef("|", ""),
            BtnDef("\uD835\uDC01", "bold", bold = true),  // 𝐁
            BtnDef("\uD835\uDC3C", "italic"),              // 𝐼
            BtnDef("U\u0332", "underline"),                // U̲
            BtnDef("S\u0336", "strike"),                   // S̶
            BtnDef("⟨⟩", "code"),
            BtnDef("≡", "verbatim"),
            BtnDef("|", ""),
            BtnDef("•", "list"),
            BtnDef("☐", "checkbox"),
            BtnDef("⊞", "table"),
            BtnDef("{ }", "srcblock"),
            BtnDef("\u201C", "quote"),                     // "
            BtnDef("\uD83D\uDCC5", "timestamp"),           // 📅
        )

        for (def in buttons) {
            if (def.label == "|") {
                // Separator
                val sep = View(activity).apply {
                    setBackgroundColor(theme.separator)
                    layoutParams = LinearLayout.LayoutParams(dpToPx(1), dpToPx(24)).apply {
                        setMargins(dpToPx(2), 0, dpToPx(2), 0)
                    }
                }
                container.addView(sep)
                continue
            }

            val btn = Button(activity).apply {
                text = def.label
                isAllCaps = false
                setTextSize(TypedValue.COMPLEX_UNIT_SP, if (def.label.length > 2) 11f else 13f)
                setTextColor(def.color ?: theme.label)
                if (def.bold) {
                    setTypeface(typeface, android.graphics.Typeface.BOLD)
                }
                setBackgroundColor(Color.TRANSPARENT)
                setPadding(dpToPx(6), 0, dpToPx(6), 0)
                minimumWidth = dpToPx(32)
                minWidth = dpToPx(32)
                minimumHeight = dpToPx(36)
                minHeight = dpToPx(36)
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    dpToPx(36)
                )
                setOnClickListener { onToolbarAction(def.action, wv) }
            }
            container.addView(btn)
        }

        scroll.addView(container)
        return scroll
    }

    private fun onToolbarAction(action: String, wv: WebView) {
        when (action) {
            "todo" -> showTodoPicker(wv)
            "heading" -> showHeadingPicker(wv)
            "priority" -> showPriorityPicker(wv)
            "tag" -> showTagPicker(wv)
            "table" -> showTablePicker(wv)
            "deadline" -> showDatePicker("deadline", wv)
            "scheduled" -> showDatePicker("scheduled", wv)
            else -> {
                wv.evaluateJavascript(
                    "window.__myceliumToolbar && window.__myceliumToolbar.$action()", null
                )
            }
        }
    }

    // ── Pickers ─────────────────────────────────────────────

    private fun showTodoPicker(wv: WebView) {
        val js = "JSON.stringify({ todo: window.__myceliumOrgConfig?.todoKeywords ?? ['TODO'], done: window.__myceliumOrgConfig?.doneKeywords ?? ['DONE'] })"
        wv.evaluateJavascript(js) { result ->
            val jsonStr = result?.trim('"')?.replace("\\\"", "\"")?.replace("\\\\", "\\") ?: "{}"
            Handler(Looper.getMainLooper()).post {
                var todoKw = listOf("TODO")
                var doneKw = listOf("DONE")
                try {
                    val obj = JSONObject(jsonStr)
                    todoKw = jsonArrayToList(obj.optJSONArray("todo")) ?: todoKw
                    doneKw = jsonArrayToList(obj.optJSONArray("done")) ?: doneKw
                } catch (_: Exception) {}

                val items = mutableListOf("None")
                items.addAll(todoKw)
                items.addAll(doneKw.map { "✓ $it" })

                AlertDialog.Builder(activity)
                    .setTitle("Set TODO State")
                    .setItems(items.toTypedArray()) { dialog, which ->
                        val selected = when {
                            which == 0 -> "null"
                            which <= todoKw.size -> jsString(todoKw[which - 1])
                            else -> jsString(doneKw[which - 1 - todoKw.size])
                        }
                        wv.evaluateJavascript("window.__myceliumToolbar?.todoSet($selected)", null)
                        dialog.dismiss()
                    }
                    .setNegativeButton("Cancel", null)
                    .create().show()
            }
        }
    }

    private fun showHeadingPicker(wv: WebView) {
        Handler(Looper.getMainLooper()).post {
            val items = arrayOf("Same level (auto)", "* Heading 1", "** Heading 2", "*** Heading 3", "**** Heading 4")
            AlertDialog.Builder(activity)
                .setTitle("Insert Heading")
                .setItems(items) { dialog, which ->
                    if (which == 0) {
                        wv.evaluateJavascript("window.__myceliumToolbar?.heading()", null)
                    } else {
                        wv.evaluateJavascript("window.__myceliumToolbar?.headingLevel($which)", null)
                    }
                    dialog.dismiss()
                }
                .setNegativeButton("Cancel", null)
                .create().show()
        }
    }

    private fun showPriorityPicker(wv: WebView) {
        val js = "JSON.stringify(window.__myceliumOrgConfig?.priorities ?? ['A','B','C'])"
        wv.evaluateJavascript(js) { result ->
            val jsonStr = result?.trim('"')?.replace("\\\"", "\"")?.replace("\\\\", "\\") ?: "[]"
            Handler(Looper.getMainLooper()).post {
                var priorities = listOf("A", "B", "C")
                try {
                    priorities = jsonArrayToList(JSONArray(jsonStr)) ?: priorities
                } catch (_: Exception) {}

                val items = mutableListOf("None")
                items.addAll(priorities.map { "[#$it]" })

                AlertDialog.Builder(activity)
                    .setTitle("Set Priority")
                    .setItems(items.toTypedArray()) { dialog, which ->
                        val selected = if (which == 0) "null" else jsString(priorities[which - 1])
                        wv.evaluateJavascript("window.__myceliumToolbar?.prioritySet($selected)", null)
                        dialog.dismiss()
                    }
                    .setNegativeButton("Cancel", null)
                    .create().show()
            }
        }
    }

    private fun showTablePicker(wv: WebView) {
        Handler(Looper.getMainLooper()).post {
            val items = arrayOf(
                "2 × 2 (2 cols × 2 rows)",
                "3 × 3 (3 cols × 3 rows)",
                "4 × 3 (4 cols × 3 rows)",
                "5 × 3 (5 cols × 3 rows)",
                "2 × 4 (2 cols × 4 rows)",
                "3 × 5 (3 cols × 5 rows)"
            )
            val dims = arrayOf(
                intArrayOf(2, 2), intArrayOf(3, 3), intArrayOf(4, 3),
                intArrayOf(5, 3), intArrayOf(2, 4), intArrayOf(3, 5)
            )
            AlertDialog.Builder(activity)
                .setTitle("Insert Table")
                .setItems(items) { dialog, which ->
                    val (cols, rows) = dims[which]
                    wv.evaluateJavascript("window.__myceliumToolbar?.tableSize($rows, $cols)", null)
                    dialog.dismiss()
                }
                .setNegativeButton("Cancel", null)
                .create().show()
        }
    }

    private fun showTagPicker(wv: WebView) {
        val jsFiletags = "window.__myceliumToolbar?.getFiletags?.() ?? '[]'"
        val jsAllTags = "JSON.stringify((window.__myceliumVaultTags ?? []).map(t => t.tag || t))"
        wv.evaluateJavascript(jsFiletags) { fileResult ->
            wv.evaluateJavascript(jsAllTags) { allResult ->
                Handler(Looper.getMainLooper()).post {
                    var currentTags = mutableListOf<String>()
                    var allTags = mutableListOf<String>()
                    try {
                        val fileStr = fileResult?.trim('"')?.replace("\\\"", "\"")?.replace("\\\\", "\\") ?: "[]"
                        currentTags = jsonArrayToList(JSONArray(fileStr))?.toMutableList() ?: mutableListOf()
                    } catch (_: Exception) {}
                    try {
                        val allStr = allResult?.trim('"')?.replace("\\\"", "\"")?.replace("\\\\", "\\") ?: "[]"
                        allTags = jsonArrayToList(JSONArray(allStr))?.toMutableList() ?: mutableListOf()
                    } catch (_: Exception) {}

                    // Merge: current first, then vault tags not already present
                    val displayTags = currentTags.toMutableList()
                    for (t in allTags) {
                        if (!displayTags.contains(t)) displayTags.add(t)
                    }

                    val items = displayTags.map { tag ->
                        if (currentTags.contains(tag)) "✓ $tag" else "  $tag"
                    }.toMutableList()
                    items.add("+ Add New Tag")

                    AlertDialog.Builder(activity)
                        .setTitle("File Tags")
                        .setItems(items.toTypedArray()) { dialog, which ->
                            if (which < displayTags.size) {
                                val tag = displayTags[which]
                                wv.evaluateJavascript("window.__myceliumToolbar?.tagSet(${jsString(tag)})", null)
                            } else {
                                // Add new tag
                                val input = android.widget.EditText(activity).apply {
                                    hint = "tag name"
                                    setPadding(dpToPx(16), dpToPx(8), dpToPx(16), dpToPx(8))
                                }
                                AlertDialog.Builder(activity)
                                    .setTitle("New Tag")
                                    .setView(input)
                                    .setPositiveButton("Add") { _, _ ->
                                        val tag = input.text.toString().trim()
                                        if (tag.isNotEmpty()) {
                                            wv.evaluateJavascript("window.__myceliumToolbar?.tagSet(${jsString(tag)})", null)
                                        }
                                    }
                                    .setNegativeButton("Cancel", null)
                                    .create().show()
                            }
                            dialog.dismiss()
                        }
                        .setNegativeButton("Cancel", null)
                        .create().show()
                }
            }
        }
    }

    private fun showDatePicker(type: String, wv: WebView) {
        val jsGet = "window.__myceliumToolbar?.getExisting?.(${jsString(type)}) ?? ''"
        wv.evaluateJavascript(jsGet) { result ->
            val existingStr = result?.trim('"') ?: ""
            Handler(Looper.getMainLooper()).post {
                val cal = Calendar.getInstance()

                // Pre-select existing date if available
                if (existingStr.isNotEmpty() && existingStr != "null") {
                    try {
                        val sdf = SimpleDateFormat("yyyy-MM-dd", Locale.US)
                        sdf.parse(existingStr)?.let { cal.time = it }
                    } catch (_: Exception) {}
                }

                val dialog = DatePickerDialog(
                    activity,
                    { _, year, month, dayOfMonth ->
                        cal.set(year, month, dayOfMonth)
                        val dateFmt = SimpleDateFormat("yyyy-MM-dd", Locale.US)
                        val dayFmt = SimpleDateFormat("EEE", Locale.US)
                        val timestamp = "<${dateFmt.format(cal.time)} ${dayFmt.format(cal.time)}>"
                        val jsType = if (type == "deadline") "deadlineSet" else "scheduledSet"
                        wv.evaluateJavascript("window.__myceliumToolbar?.$jsType(${jsString(timestamp)})", null)
                    },
                    cal.get(Calendar.YEAR),
                    cal.get(Calendar.MONTH),
                    cal.get(Calendar.DAY_OF_MONTH)
                )

                // Add Remove button
                dialog.setButton(AlertDialog.BUTTON_NEUTRAL, "Remove") { d, _ ->
                    val jsType = if (type == "deadline") "deadlineSet" else "scheduledSet"
                    wv.evaluateJavascript("window.__myceliumToolbar?.$jsType(null)", null)
                    d.dismiss()
                }

                dialog.show()
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────

    private fun dpToPx(dp: Int): Int {
        return (dp * activity.resources.displayMetrics.density).toInt()
    }

    /** Encode a value as a JavaScript string literal (quotes included) so it can be
     *  interpolated into a script passed to evaluateJavascript safely. */
    private fun jsString(value: String): String {
        return JSONObject.quote(value)
            .replace("\u2028", "\\u2028")
            .replace("\u2029", "\\u2029")
    }

    private fun jsonArrayToList(arr: JSONArray?): List<String>? {
        if (arr == null) return null
        val list = mutableListOf<String>()
        for (i in 0 until arr.length()) {
            list.add(arr.getString(i))
        }
        return list
    }
}
