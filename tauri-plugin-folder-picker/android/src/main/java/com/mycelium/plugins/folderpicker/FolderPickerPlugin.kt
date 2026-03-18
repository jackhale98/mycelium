package com.mycelium.plugins.folderpicker

import android.app.Activity
import android.app.AlertDialog
import android.app.DatePickerDialog
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

@TauriPlugin
class FolderPickerPlugin(private val activity: Activity) : Plugin(activity) {
    private var webView: WebView? = null
    private var toolbarView: View? = null

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

            if (imeVisible && imeHeight > 0) {
                toolbar.visibility = View.VISIBLE
                toolbar.translationY = -(imeHeight - navBarHeight).toFloat()
            } else {
                toolbar.visibility = View.GONE
                toolbar.translationY = 0f
            }
            insets
        }
    }

    private fun createToolbarView(wv: WebView): View {
        val scroll = HorizontalScrollView(activity).apply {
            isHorizontalScrollBarEnabled = false
            setBackgroundColor(Color.parseColor("#F2F2F7"))
        }

        val container = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dpToPx(4), 0, dpToPx(4), 0)
            gravity = Gravity.CENTER_VERTICAL
        }

        data class BtnDef(val label: String, val action: String, val color: Int? = null, val bold: Boolean = false)

        val buttons = listOf(
            BtnDef("Link", "link", Color.parseColor("#16a34a"), true),
            BtnDef("|", ""),
            BtnDef("H", "heading", bold = true),
            BtnDef("ID", "makeNode", Color.parseColor("#9333ea"), true),
            BtnDef("TODO", "todo", Color.parseColor("#dc2626"), true),
            BtnDef("[#]", "priority", Color.parseColor("#ea580c"), true),
            BtnDef("Tag", "tag", Color.parseColor("#0d9488"), true),
            BtnDef("DL", "deadline", Color.parseColor("#dc2626")),
            BtnDef("SC", "scheduled", Color.parseColor("#2563eb")),
            BtnDef("|", ""),
            BtnDef("B", "bold", bold = true),
            BtnDef("I", "italic"),
            BtnDef("U", "underline"),
            BtnDef("S", "strike"),
            BtnDef("~c~", "code"),
            BtnDef("=v=", "verbatim"),
            BtnDef("|", ""),
            BtnDef("-", "list"),
            BtnDef("☐", "checkbox"),
            BtnDef("|T|", "table"),
            BtnDef("SRC", "srcblock"),
            BtnDef("\"", "quote"),
            BtnDef("Date", "timestamp"),
        )

        for (def in buttons) {
            if (def.label == "|") {
                // Separator
                val sep = View(activity).apply {
                    setBackgroundColor(Color.parseColor("#C7C7CC"))
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
                if (def.color != null) setTextColor(def.color)
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
                            which <= todoKw.size -> "'${todoKw[which - 1]}'"
                            else -> "'${doneKw[which - 1 - todoKw.size]}'"
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
                        val selected = if (which == 0) "null" else "'${priorities[which - 1]}'"
                        wv.evaluateJavascript("window.__myceliumToolbar?.prioritySet($selected)", null)
                        dialog.dismiss()
                    }
                    .setNegativeButton("Cancel", null)
                    .create().show()
            }
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
                                wv.evaluateJavascript("window.__myceliumToolbar?.tagSet('$tag')", null)
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
                                            wv.evaluateJavascript("window.__myceliumToolbar?.tagSet('$tag')", null)
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
        val jsGet = "window.__myceliumToolbar?.getExisting?.('$type') ?? ''"
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
                        wv.evaluateJavascript("window.__myceliumToolbar?.$jsType('$timestamp')", null)
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

    private fun jsonArrayToList(arr: JSONArray?): List<String>? {
        if (arr == null) return null
        val list = mutableListOf<String>()
        for (i in 0 until arr.length()) {
            list.add(arr.getString(i))
        }
        return list
    }
}
