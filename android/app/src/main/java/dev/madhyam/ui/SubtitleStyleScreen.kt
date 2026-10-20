package dev.madhyam.ui

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.SubtitleView
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import dev.madhyam.SubtitleStyle
import android.view.KeyEvent as AndroidKeyEvent

/** The subtitle style options, as rows of a settings list. */
internal fun LazyListScope.subtitleStyleRows(vm: MainViewModel, first: FocusRequester? = null) {
    val st = vm.settings
    item {
        Box(if (first != null) Modifier.focusRequester(first) else Modifier) {
            PickRow("Font size", "In points at full screen (1080p)", st.subFontPt, SubtitleStyle.SIZES.map { it to "$it pt" }) {
                vm.changeSettings { subFontPt = it }
            }
        }
    }
    item { ChoiceRow("Font family", "OK to switch", st.subFont, SubtitleStyle.FONTS.map { it to it }) { vm.changeSettings { subFont = it } } }
    item { ChoiceRow("Text colour", "OK to switch", st.subTextColor, SubtitleStyle.TEXT_COLORS.map { it to it }) { vm.changeSettings { subTextColor = it } } }
    item { ChoiceRow("Text opacity", "OK to switch", st.subTextOpacity, SubtitleStyle.TEXT_OPACITIES.map { it to "$it%" }) { vm.changeSettings { subTextOpacity = it } } }
    item { ChoiceRow("Background colour", "Behind each line of text", st.subBackColor, SubtitleStyle.BACK_COLORS.map { it to it }) { vm.changeSettings { subBackColor = it } } }
    item { ChoiceRow("Background opacity", "OK to switch", st.subBackOpacity, SubtitleStyle.OPACITIES.map { it to "$it%" }) { vm.changeSettings { subBackOpacity = it } } }
    item { ChoiceRow("Window colour", "A box around the whole caption area", st.subWindowColor, SubtitleStyle.BACK_COLORS.map { it to it }) { vm.changeSettings { subWindowColor = it } } }
    item { ChoiceRow("Window opacity", "OK to switch", st.subWindowOpacity, SubtitleStyle.OPACITIES.map { it to "$it%" }) { vm.changeSettings { subWindowOpacity = it } } }
    item { ChoiceRow("Edge style", "Shadow or outline around the letters", st.subEdge, SubtitleStyle.EDGES.map { it to it }) { vm.changeSettings { subEdge = it } } }
    item {
        ChoiceRow("Position", "Where subtitles sit on the picture", st.subPosition, SubtitleStyle.POSITIONS.map { it to it }) {
            vm.changeSettings { subPosition = it }
        }
    }
    item {
        ActionRow("Move up", "2% higher", SubtitleStyle.offsetLabel(st.subOffset)) {
            vm.changeSettings { subOffset = (subOffset + SubtitleStyle.OFFSET_STEP).coerceAtMost(SubtitleStyle.OFFSET_MAX) }
        }
    }
    item {
        ActionRow("Move down", "2% lower", SubtitleStyle.offsetLabel(st.subOffset)) {
            vm.changeSettings { subOffset = (subOffset - SubtitleStyle.OFFSET_STEP).coerceAtLeast(-SubtitleStyle.OFFSET_MAX) }
        }
    }
    item { ActionRow("Reset to defaults", "32 pt, Sans-serif, white on 50% black, drop shadow, at the bottom") { vm.changeSettings { resetSubtitleStyle() } } }
}

/**
 * Settings › Playback › Subtitle style: the options on the left, a live preview over a dark
 * frame on the right. The preview takes a share of the page width and stays put while the
 * options scroll.
 */
@Composable
internal fun SubtitleStylePage(vm: MainViewModel) {
    // The row that opened the page is gone: put the focus on the first option.
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(80)
        runCatching { first.requestFocus() }
    }
    Column {
        SectionTitle("Subtitle style", "Every change shows at once in the preview and in the player. Back returns to Playback")
        androidx.compose.foundation.layout.Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
            LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                subtitleStyleRows(vm, first)
            }
            StylePreview(vm, Modifier.weight(1.15f).aspectRatio(16f / 9f).clip(RoundedCornerShape(10.dp)))
        }
    }
}

/**
 * A miniature of the screen: a sample subtitle drawn by Media3's own subtitle renderer with the
 * user's style, scaled to the frame as it would be to the picture.
 */
@OptIn(UnstableApi::class)
@Composable
private fun StylePreview(vm: MainViewModel, modifier: Modifier) {
    val st = vm.settings
    val key = listOf(st.subFontPt, st.subFont, st.subTextColor, st.subTextOpacity, st.subBackColor, st.subBackOpacity,
        st.subWindowColor, st.subWindowOpacity, st.subEdge, st.subPosition, st.subOffset)
    Box(
        modifier.background(Brush.verticalGradient(listOf(Color(0xFF3A4A5E), Color(0xFF1B2533), Color(0xFF0B0F16)))),
    ) {
        AndroidView(
            factory = { ctx ->
                SubtitleView(ctx).apply {
                    setCues(listOf(Cue.Builder().setText("This is how subtitles will look.\nThe second line of a subtitle.").build()))
                    addOnLayoutChangeListener { v, _, top, _, bottom, _, oldTop, _, oldBottom ->
                        if (bottom - top != oldBottom - oldTop) SubtitleStyle.apply(v as SubtitleView, st, bottom - top)
                    }
                }
            },
            update = {
                @Suppress("UNUSED_EXPRESSION") key
                SubtitleStyle.apply(it, st)
            },
            modifier = Modifier.fillMaxSize(),
        )
        Text("Preview", color = Color.White.copy(alpha = 0.6f), fontSize = 11.sp, modifier = Modifier.align(Alignment.TopStart).padding(8.dp))
    }
}

/**
 * The player's "Subtitle style…": the options in a panel on the right, without dimming the
 * picture, so the real subtitles show each change.
 */
@Composable
fun SubtitleStyleDialog(vm: MainViewModel, onDismiss: () -> Unit) {
    // The OK that opened the panel can come back up in here: ignore OK until a key went down in it.
    var armed by remember { mutableStateOf(false) }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(100)
        runCatching { first.requestFocus() }
    }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        (LocalView.current.parent as? DialogWindowProvider)?.window?.setDimAmount(0f)
        Box(Modifier.fillMaxSize()) {
            Column(
                Modifier.align(Alignment.CenterEnd).width(440.dp).fillMaxHeight()
                    .background(Panel.copy(alpha = 0.94f))
                    .padding(horizontal = 16.dp, vertical = 20.dp)
                    .onPreviewKeyEvent { ev ->
                        val code = ev.nativeKeyEvent.keyCode
                        val ok = code == AndroidKeyEvent.KEYCODE_DPAD_CENTER || code == AndroidKeyEvent.KEYCODE_ENTER
                        if (ev.type == KeyEventType.KeyDown) armed = true
                        ok && ev.type == KeyEventType.KeyUp && !armed
                    },
            ) {
                Text("Subtitle style", fontSize = 22.sp, modifier = Modifier.padding(bottom = 4.dp))
                Text("Changes show on the subtitles at once. Back to close.", color = Muted, fontSize = 12.sp, modifier = Modifier.padding(bottom = 8.dp))
                LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    subtitleStyleRows(vm, first)
                }
            }
        }
    }
}
