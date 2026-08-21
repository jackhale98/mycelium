package com.mycelium.plugins.folderpicker

import android.graphics.Color

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
