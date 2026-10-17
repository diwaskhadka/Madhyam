package dev.madhyam.ui

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AspectRatio
import androidx.compose.material.icons.filled.Audiotrack
import androidx.compose.material.icons.filled.ClosedCaption
import androidx.compose.material.icons.filled.Forward10
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Replay10
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Speed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Icon
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import dev.madhyam.PictureSize
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

private enum class Picker { Audio, Subtitles, Speed, Picture }

/** One button of the options row. */
private class Control(val icon: ImageVector, val label: String, val action: () -> Unit)

/**
 * Full-screen playback with remote-control keys:
 *  - Live:  Up/Down or Channel +/- zap through the channel list, OK opens the options row.
 *  - Video: OK or Play/Pause toggles, Left/Right seek 10 s, FF/Rewind seek 30 s, Down opens the options row.
 *  - Menu (any): options row with audio, subtitles, speed and picture size (and the TV guide on live).
 *  - Back closes the options row, then returns to the page (live keeps playing in the preview).
 *
 * The player keeps the focus the whole time; the options row is driven from its key handler
 * (Left/Right move the highlight, OK runs it), which avoids handing focus into the video view.
 */
@Composable
fun PlayerScreen(vm: MainViewModel) {
    val pb = vm.playback
    val item = pb.current ?: run {
        LaunchedEffect(Unit) { vm.closePlayer() }
        return
    }
    val focus = remember { FocusRequester() }
    var overlayToken by remember { mutableIntStateOf(0) } // bump to show the overlay
    var overlayVisible by remember { mutableStateOf(true) }
    var controls by remember { mutableStateOf(false) }
    var selected by remember { mutableIntStateOf(0) }
    var picker by remember { mutableStateOf<Picker?>(null) }
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    val seekable = !item.isLive

    val row: List<Control> = buildList {
        if (seekable) {
            add(Control(if (pb.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow, if (pb.isPlaying) "Pause" else "Play") { pb.togglePause() })
            add(Control(Icons.Filled.Replay10, "−10 s") { pb.seekBy(-10_000) })
            add(Control(Icons.Filled.Forward10, "+10 s") { pb.seekBy(10_000) })
        }
        add(Control(Icons.Filled.Audiotrack, "Audio") { picker = Picker.Audio })
        add(Control(Icons.Filled.ClosedCaption, "Subtitles") { picker = Picker.Subtitles })
        if (seekable && !item.isCatchup && vm.subtitleSearchAvailable) {
            add(Control(Icons.Filled.Search, "Find subtitles") { vm.openSubtitleSearch() })
        }
        if (seekable) add(Control(Icons.Filled.Speed, "Speed ${speedLabel(pb.speed)}") { picker = Picker.Speed })
        add(Control(Icons.Filled.AspectRatio, pb.pictureSize.label) { picker = Picker.Picture })
        if (item.isLive) {
            add(Control(Icons.Filled.GridView, "TV guide") {
                vm.closePlayer()
                vm.openGuide()
            })
        }
    }

    fun openControls() {
        controls = true
        selected = 0
        overlayToken++
    }

    fun closeControls() {
        controls = false
        picker = null
    }

    // Back on the options row (same button highlighted) after a picker closes.
    fun pickerClosed() {
        picker = null
        controls = true
        overlayToken++
    }

    // A new channel or film shows the overlay.
    LaunchedEffect(item.id) { overlayToken++ }

    // Periodic progress save for films/episodes (ignored for live and catch-up).
    LaunchedEffect(item.id) {
        while (isActive) {
            delay(15_000)
            vm.saveProgress(item)
        }
    }

    // Overlay auto-hide (longer while the options row is open), with a live clock while visible.
    LaunchedEffect(overlayToken, controls, picker) {
        overlayVisible = true
        val until = System.currentTimeMillis() + if (controls || picker != null) 10_000 else 4_000
        while (isActive && System.currentTimeMillis() < until) {
            position = pb.positionMs
            duration = pb.durationMs
            delay(250)
        }
        overlayVisible = false
        if (picker == null && controls) closeControls()
    }

    // Back on the options row when the subtitle search closes.
    val searching = vm.subtitleResults != null
    LaunchedEffect(searching) {
        if (!searching && controls) {
            overlayToken++
            delay(120)
            runCatching { focus.requestFocus() }
        }
    }
    // Ask for focus once the page we replaced has let go of it (else the first key press is lost),
    // and again when a picker dialog closes.
    LaunchedEffect(picker) {
        if (picker == null) {
            delay(120)
            runCatching { focus.requestFocus() }
        }
    }
    BackHandler { if (controls) closeControls() else vm.closePlayer() }

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(focus)
            .focusable()
            .onPreviewKeyEvent { ev ->
                if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                val code = ev.nativeKeyEvent.keyCode
                overlayToken++ // any key keeps the overlay up
                when (code) {
                    AndroidKeyEvent.KEYCODE_MENU -> {
                        if (controls) closeControls() else openControls()
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, AndroidKeyEvent.KEYCODE_MEDIA_PLAY, AndroidKeyEvent.KEYCODE_MEDIA_PAUSE -> {
                        pb.togglePause()
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_REWIND -> {
                        pb.seekBy(-30_000)
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> {
                        pb.seekBy(30_000)
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_CHANNEL_UP -> {
                        if (item.isLive) vm.zap(-1)
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_CHANNEL_DOWN -> {
                        if (item.isLive) vm.zap(1)
                        return@onPreviewKeyEvent true
                    }
                }
                if (controls) {
                    // The options row: Left/Right move, OK runs, Up/Down close it.
                    when (code) {
                        AndroidKeyEvent.KEYCODE_DPAD_LEFT -> selected = (selected - 1).coerceAtLeast(0)
                        AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> selected = (selected + 1).coerceAtMost(row.lastIndex)
                        AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> row.getOrNull(selected)?.action?.invoke()
                        AndroidKeyEvent.KEYCODE_DPAD_UP, AndroidKeyEvent.KEYCODE_DPAD_DOWN -> closeControls()
                        else -> return@onPreviewKeyEvent false
                    }
                    return@onPreviewKeyEvent true
                }
                when (code) {
                    AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> {
                        when {
                            pb.error != null -> pb.retry()
                            item.isLive -> openControls()
                            else -> pb.togglePause()
                        }
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_UP -> {
                        if (item.isLive) vm.zap(-1)
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_DOWN -> {
                        if (item.isLive) vm.zap(1) else openControls()
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_LEFT -> {
                        if (seekable) pb.seekBy(-10_000)
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> {
                        if (seekable) pb.seekBy(10_000)
                        true
                    }
                    else -> false
                }
            },
    ) {
        VideoSurface(pb.player, Modifier.fillMaxSize(), pb.pictureSize.resizeMode)

        if (overlayVisible || controls || pb.error != null) {
            val strip = vm.settings.stripPrefixes
            Column(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.85f))))
                    .padding(start = 48.dp, end = 48.dp, top = 48.dp, bottom = 28.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (item.isLive || item.isCatchup) {
                        Logo(item, 72.dp, 44.dp, strip)
                        Spacer(Modifier.width(12.dp))
                    }
                    Column {
                        Text(
                            if (item.isEpisode) "${item.seasonEpisode}  ${item.name}" else item.displayName(strip),
                            fontSize = 26.sp, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                        when {
                            item.isCatchup -> Text("Catch-up · recorded ${date(item.catchupStart)} ${hhmm(item.catchupStart)}", color = Accent, fontSize = 12.sp)
                            item.isLive && pb.playlist.size > 1 ->
                                Text("${pb.index + 1} / ${pb.playlist.size}   ▲▼ change channel   OK options", color = Muted, fontSize = 12.sp)
                        }
                    }
                }
                if (item.isLive) {
                    val now = System.currentTimeMillis() / 1000
                    val nn = vm.nowNext[item.id]
                    nn?.now?.let {
                        Text("${hhmm(it.start)} – ${hhmm(it.stop)}   ${it.title}", color = Color.White, fontSize = 16.sp, maxLines = 1)
                        ProgressBar(it.progress(now), width = 520.dp)
                    }
                    nn?.next?.let { Text("Next  ${hhmm(it.start)}   ${it.title}", color = Muted, fontSize = 14.sp, maxLines = 1) }
                } else if (duration > 0) {
                    ProgressBar(position.toFloat() / duration, Modifier.padding(top = 8.dp))
                    Row {
                        Text(
                            "${if (pb.isPlaying) "▶" else "❚❚"}  ${fmt(position)} / ${fmt(duration)}",
                            color = Color.White,
                            fontSize = 14.sp,
                            modifier = Modifier.width(260.dp),
                        )
                        Text(
                            buildString {
                                append("◀ ▶ 10 s   ⏪ ⏩ 30 s   ▼ options")
                                if (pb.speed != 1f) append("   speed ${speedLabel(pb.speed)}")
                            },
                            color = Muted, fontSize = 14.sp,
                        )
                    }
                }
                pb.error?.let { Text(it, color = ErrorRed, fontSize = 16.sp) }
                if (controls) {
                    Spacer(Modifier.height(14.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        // Films have up to eight options: then only the highlighted one shows its label.
                        val labels = row.size <= 5
                        row.forEachIndexed { i, c -> ControlChip(c, i == selected, showLabel = labels) }
                    }
                }
            }
        }
    }

    @Suppress("UNUSED_EXPRESSION") pb.tracksVersion // re-read the track lists when they change
    when (picker) {
        Picker.Audio -> {
            val tracks = pb.audioTracks()
            PickerDialog("Audio", tracks.map { it to it.label }, tracks.firstOrNull { it.selected }, { pb.select(it) }) { pickerClosed() }
        }
        Picker.Subtitles -> {
            val tracks = pb.subtitleTracks()
            val off = "off"
            val options: List<Pair<Any, String>> = listOf<Pair<Any, String>>(off to "Off") + tracks.map { it to it.label }
            val current: Any = if (pb.subtitlesOff) off else tracks.firstOrNull { it.selected } ?: off
            PickerDialog("Subtitles", options, current, { choice ->
                when (choice) {
                    off -> pb.turnSubtitlesOff()
                    is dev.madhyam.TrackChoice -> pb.select(choice)
                }
            }) { pickerClosed() }
        }
        Picker.Speed -> PickerDialog("Speed", SPEEDS.map { it to speedLabel(it) }, pb.speed, { pb.changeSpeed(it) }) { pickerClosed() }
        Picker.Picture -> PickerDialog("Picture size", PictureSize.entries.map { it to it.label }, pb.pictureSize, { pb.pictureSize = it }) { pickerClosed() }
        null -> {}
    }
    if (searching) SubtitleSearchDialog(vm)
}

private val SPEEDS = listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f)

private fun speedLabel(s: Float) = if (s == s.toInt().toFloat()) "${s.toInt()}×" else "${s}×"

@Composable
private fun ControlChip(c: Control, highlighted: Boolean, showLabel: Boolean = true) {
    Row(
        Modifier
            .background(if (highlighted) Color.White else Color.White.copy(alpha = 0.12f), RoundedCornerShape(50))
            .padding(horizontal = 16.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val fg = if (highlighted) Color.Black else Color.White
        Icon(c.icon, contentDescription = null, tint = fg, modifier = Modifier.size(20.dp))
        if (showLabel || highlighted) {
            Spacer(Modifier.width(6.dp))
            Text(c.label, fontSize = 13.sp, color = fg, maxLines = 1, softWrap = false)
        }
    }
}
