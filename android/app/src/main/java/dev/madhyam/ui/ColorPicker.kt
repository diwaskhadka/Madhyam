package dev.madhyam.ui

import android.view.KeyEvent as AndroidKeyEvent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.DialogProperties
import androidx.tv.material3.Button
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.DenseListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Surface
import androidx.tv.material3.Text

/**
 * Settings › Appearance › Custom colour, made for a remote: named presets (Crimson, Royal blue…),
 * Left/Right on Hue, Saturation and Lightness, a hex code, and a dark or light base.
 * The preview shows the palette the app builds from the colour.
 */
@Composable
fun ColorPickerDialog(color: String, light: Boolean, onDone: (String, Boolean) -> Unit, onDismiss: () -> Unit) {
    val start = toHsl(parseColor(color) ?: parseColor(CRIMSON)!!)
    var hue by remember { mutableFloatStateOf(start.first) }
    var sat by remember { mutableFloatStateOf(start.second) }
    var lig by remember { mutableFloatStateOf(start.third) }
    var isLight by remember { mutableStateOf(light) }
    var editHex by remember { mutableStateOf(false) }
    // The OK that opened the dialog lifts its key inside it: accept clicks only after a key went down here.
    var armed by remember { mutableStateOf(false) }
    val first = remember { FocusRequester() }
    val use = remember { FocusRequester() }
    var focusedPreset by remember { mutableStateOf<String?>(null) }
    val current = hsl(hue, sat, lig)
    val p = customPalette(current.toHex(), isLight)

    fun set(c: Color) {
        val (h, s, l) = toHsl(c)
        hue = h
        sat = s
        lig = l
    }

    // Two columns, so the whole dialog fits a 540 dp high TV screen.
    androidx.compose.ui.window.Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(
            Modifier.width(800.dp).heightIn(max = 500.dp).background(Panel, RoundedCornerShape(16.dp)).padding(horizontal = 22.dp, vertical = 16.dp)
                .onPreviewKeyEvent { if (it.type == KeyEventType.KeyDown) armed = true; false }
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text("Custom theme colour", fontSize = 20.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                Column(Modifier.width(320.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    // Preview: a mini screen in the palette this colour produces.
                    Row(
                        Modifier.background(p.background, RoundedCornerShape(12.dp)).padding(12.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Box(Modifier.size(36.dp, 56.dp).background(p.panel, RoundedCornerShape(8.dp)))
                        Column(
                            Modifier.width(236.dp).background(p.panel, RoundedCornerShape(8.dp))
                                .border(2.dp, p.accent, RoundedCornerShape(8.dp)).padding(horizontal = 14.dp, vertical = 8.dp),
                        ) {
                            Text("Live TV", color = p.text, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                            Text("Now: Evening News", color = p.muted, fontSize = 12.sp)
                            Spacer(Modifier.height(4.dp))
                            Box(Modifier.height(4.dp).width(160.dp).background(p.accent, RoundedCornerShape(2.dp)))
                        }
                    }

                    val name = colorName(current.toHex())
                    Text(if (name.startsWith("#")) name else "$name · ${current.toHex()}", fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                    Text(focusedPreset?.let { "Presets: $it" } ?: "Presets", color = Muted, fontSize = 12.sp)
                    THEME_PRESETS.chunked(6).forEachIndexed { row, presets ->
                        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            presets.forEachIndexed { i, (presetName, hex) ->
                                val c = parseColor(hex)!!
                                Surface(
                                    onClick = { if (armed) set(c) },
                                    shape = ClickableSurfaceDefaults.shape(CircleShape),
                                    colors = ClickableSurfaceDefaults.colors(containerColor = c, focusedContainerColor = c),
                                    border = ClickableSurfaceDefaults.border(
                                        focusedBorder = androidx.tv.material3.Border(androidx.compose.foundation.BorderStroke(3.dp, TextColor)),
                                    ),
                                    modifier = Modifier.size(40.dp)
                                        .onFocusChanged { if (it.isFocused) focusedPreset = presetName else if (focusedPreset == presetName) focusedPreset = null }
                                        .then(if (row == 0 && i == 0) Modifier.focusRequester(first) else Modifier),
                                ) {}
                            }
                        }
                    }
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    StepRow("Hue", "${hue.toInt()}°") { hue = ((hue + it * 10) % 360 + 360) % 360 }
                    StepRow("Saturation", "${(sat * 100).toInt()} %") { sat = (sat + it * 0.05f).coerceIn(0f, 1f) }
                    StepRow("Lightness", "${(lig * 100).toInt()} %") { lig = (lig + it * 0.05f).coerceIn(0.1f, 0.9f) }
                    DenseListItem(
                        selected = false,
                        onClick = { if (armed) editHex = true },
                        headlineContent = { Text("Hex code") },
                        supportingContent = { Text("OK to type one, like #DC143C", color = Muted, fontSize = 11.sp) },
                        trailingContent = { Text(current.toHex(), color = Accent) },
                    )
                    DenseListItem(
                        selected = false,
                        onClick = { if (armed) isLight = !isLight },
                        // Down goes to "Use this theme", not to whichever button is nearest.
                        modifier = Modifier.focusProperties { down = use },
                        headlineContent = { Text("Base") },
                        supportingContent = { Text("OK switches dark / light", color = Muted, fontSize = 11.sp) },
                        trailingContent = { Text(if (isLight) "Light" else "Dark", color = Accent) },
                    )
                    Spacer(Modifier.height(4.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Button(onClick = { if (armed) { onDone(current.toHex(), isLight); onDismiss() } }, modifier = Modifier.focusRequester(use)) { Text("Use this theme") }
                        OutlinedButton(onClick = onDismiss) { Text("Cancel") }
                    }
                }
            }
        }
    }
    if (editHex) {
        TextEditDialog("Hex code (#RRGGBB)", current.toHex(), KeyboardType.Ascii, secret = false, onDone = { parseColor(it)?.let(::set) }) {
            editHex = false
        }
    }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(80)
        runCatching { first.requestFocus() }
    }
}

/** A row the remote's Left/Right steps through (−1 / +1); its value shows on the right. */
@Composable
private fun StepRow(title: String, value: String, onStep: (Int) -> Unit) {
    DenseListItem(
        selected = false,
        onClick = {},
        headlineContent = { Text(title) },
        trailingContent = { Text("◀  $value  ▶", color = Accent) },
        modifier = Modifier.onPreviewKeyEvent {
            if (it.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
            when (it.nativeKeyEvent.keyCode) {
                AndroidKeyEvent.KEYCODE_DPAD_LEFT -> { onStep(-1); true }
                AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> { onStep(1); true }
                else -> false
            }
        },
    )
}
