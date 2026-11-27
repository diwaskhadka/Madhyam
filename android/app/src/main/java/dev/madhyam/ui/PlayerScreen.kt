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
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.Speed
import androidx.compose.material.icons.filled.Timer
import androidx.compose.material.icons.filled.Tune
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
import androidx.media3.common.Player
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

private enum class Picker { Audio, Subtitles, Speed, Picture, AudioDelay }

/** One button of the options row. */
private class Control(val icon: ImageVector, val label: String, val action: () -> Unit)

/**
 * Full-screen playback with remote-control keys:
 *  - Live:  Up/Down or Channel +/- zap through the channel list, OK opens the options row.
 *  - Video: OK or Down opens the options row (Play/Pause first), Play/Pause toggles, Left/Right seek 10 s,
 *    FF/Rewind seek 30 s.
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
    var styling by remember { mutableStateOf(false) } // Subtitles › Subtitle style…
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    var badges by remember { mutableStateOf(emptyList<String>()) } // "3840x2160", "24 FPS", "STEREO"
    val seekable = !item.isLive
    // Up next: the last 30 s of an episode offer the next one (OK plays it, Back hides the card,
    // and it starts by itself when this one ends).
    val next = vm.nextEpisode
    var secondsLeft by remember { mutableIntStateOf(-1) } // -1: unknown
    var ended by remember { mutableStateOf(false) }
    var upNextHidden by remember(item.id) { mutableStateOf(false) }
    LaunchedEffect(item.id) {
        secondsLeft = -1
        ended = false
        if (!item.isEpisode) return@LaunchedEffect
        while (isActive) {
            val d = pb.durationMs
            ended = pb.player.playbackState == Player.STATE_ENDED
            secondsLeft = if (ended) 0 else if (d > 0) ((d - pb.positionMs).coerceAtLeast(0) / 1000).toInt() else -1
            delay(500)
        }
    }
    val upNext = next != null && !upNextHidden && picker == null && !styling && (ended || secondsLeft in 0..30)
    LaunchedEffect(upNext, ended) { if (upNext && ended) vm.playNextEpisode() }

    val row: List<Control> = buildList {
        if (seekable) {
            add(Control(if (pb.paused) Icons.Filled.PlayArrow else Icons.Filled.Pause, if (pb.paused) "Play" else "Pause") { pb.togglePause() })
            add(Control(Icons.Filled.Replay10, "−10 s") { pb.seekBy(-10_000) })
            add(Control(Icons.Filled.Forward10, "+10 s") { pb.seekBy(10_000) })
            if (next != null) add(Control(Icons.Filled.SkipNext, "Next episode") { controls = false; vm.playNextEpisode() })
        }
        add(Control(Icons.Filled.Audiotrack, "Audio") { picker = Picker.Audio })
        add(Control(Icons.Filled.ClosedCaption, "Subtitles") { picker = Picker.Subtitles })
        add(Control(Icons.Filled.Tune, "Audio delay ${delayLabel(pb.audioDelayMs)}") { picker = Picker.AudioDelay })
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

    // Overlay auto-hide (longer while the options row is open; never while paused), with a live
    // clock while visible.
    LaunchedEffect(overlayToken, controls, picker, pb.paused) {
        overlayVisible = true
        val until = System.currentTimeMillis() + if (controls || picker != null) 10_000 else 4_000
        while (isActive && (pb.paused || System.currentTimeMillis() < until)) {
            position = pb.positionMs
            duration = pb.durationMs
            badges = pb.streamBadges()
            delay(250)
        }
        overlayVisible = false
        if (picker == null && controls) closeControls()
    }

    // Ask for focus once the page we replaced has let go of it (else the first key press is lost),
    // and again when a picker dialog closes.
    LaunchedEffect(picker) {
        if (picker == null) {
            delay(120)
            runCatching { focus.requestFocus() }
        }
    }
    BackHandler {
        when {
            controls -> closeControls()
            upNext -> upNextHidden = true
            else -> vm.closePlayer()
        }
    }

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
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> {
                        pb.togglePause()
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY -> {
                        pb.play()
                        return@onPreviewKeyEvent true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PAUSE -> {
                        pb.pause()
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
                if (upNext && !controls && (code == AndroidKeyEvent.KEYCODE_DPAD_CENTER || code == AndroidKeyEvent.KEYCODE_ENTER)) {
                    vm.playNextEpisode()
                    return@onPreviewKeyEvent true
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
                        // The first OK only shows the controls (as on most TV players); OK again
                        // on the highlighted Play/Pause pauses.
                        if (pb.error != null) pb.retry() else openControls()
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
        // Keep subtitles above the title and options row while they show.
        val lift = when {
            controls -> 0.24f
            overlayVisible -> 0.14f
            else -> 0f
        }
        VideoSurface(pb.player, Modifier.fillMaxSize(), pb.pictureSize.resizeMode, subtitleLift = lift, style = vm.settings, playback = pb)

        if (overlayVisible || controls || pb.error != null) {
            val strip = vm.settings.stripPrefixes
            Column(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.85f))))
                    .padding(start = 48.dp, end = 48.dp, top = 48.dp, bottom = 28.dp),
            ) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    if (item.isLive || item.isCatchup) {
                        Logo(item, 72.dp, 44.dp, strip)
                        Spacer(Modifier.width(12.dp))
                    }
                    Column(Modifier.weight(1f)) {
                        Text(
                            if (item.isEpisode) item.episodeLabel else item.displayName(strip),
                            fontSize = 26.sp, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                        when {
                            item.isCatchup -> Text("Catch-up · recorded ${date(item.catchupStart)} ${hhmm(item.catchupStart)}", color = Accent, fontSize = 12.sp)
                            item.isLive && pb.playlist.size > 1 ->
                                Text("${pb.index + 1} / ${pb.playlist.size}   ▲▼ change channel   OK options", color = Muted, fontSize = 12.sp)
                        }
                    }
                    // Picture size, frame rate and sound, as TiviMate shows them.
                    badges.forEach { b ->
                        Spacer(Modifier.width(8.dp))
                        Text(
                            b, color = Color.Black, fontSize = 12.sp, maxLines = 1, softWrap = false,
                            modifier = Modifier.background(Color.White.copy(alpha = 0.72f), RoundedCornerShape(4.dp)).padding(horizontal = 6.dp, vertical = 2.dp),
                        )
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
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        // The state, not the action: "Paused" with its icon while paused.
                        Row(Modifier.width(260.dp), verticalAlignment = Alignment.CenterVertically) {
                            Icon(
                                if (pb.paused) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                                contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp),
                            )
                            Text(
                                "  ${if (pb.paused) "Paused  " else ""}${fmt(position)} / ${fmt(duration)}",
                                color = Color.White,
                                fontSize = 14.sp,
                            )
                        }
                        Text(
                            buildString {
                                // Plain characters: the ⏪ ⏩ symbols turn into coloured emoji on Android.
                                append("◀ ▶ 10 s   ◀◀ ▶▶ 30 s   OK options")
                                if (pb.speed != 1f) append("   speed ${speedLabel(pb.speed)}")
                            },
                            color = Muted, fontSize = 14.sp,
                        )
                    }
                }
                pb.error?.let {
                    Text(it, color = ErrorRed, fontSize = 16.sp)
                    Text("OK to try again" + (pb.errorDetail?.let { d -> "  ·  $d" } ?: ""), color = Muted, fontSize = 12.sp)
                }
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
        if (upNext && next != null) UpNextCard(next, secondsLeft, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = if (controls || overlayVisible) 170.dp else 48.dp))
    }

    @Suppress("UNUSED_EXPRESSION") pb.tracksVersion // re-read the track lists when they change
    when (picker) {
        Picker.Audio -> {
            val tracks = pb.audioTracks()
            PickerDialog("Audio", tracks.map { it to it.label }, tracks.firstOrNull { it.selected }, { pb.select(it) }) { pickerClosed() }
        }
        // One panel for tracks, online search, delay and style.
        Picker.Subtitles -> SubtitlesPanel(
            vm,
            canSearch = seekable && !item.isCatchup && vm.subtitleSearchAvailable,
            onStyle = {
                styling = true
                closeControls() // the style page shows the subtitles where they really sit
            },
            onDismiss = { pickerClosed() },
        )
        Picker.Speed -> PickerDialog("Speed", SPEEDS.map { it to speedLabel(it) }, pb.speed, { pb.changeSpeed(it) }) { pickerClosed() }
        Picker.Picture -> PickerDialog("Picture size", PictureSize.entries.map { it to it.label }, pb.pictureSize, { pb.pictureSize = it }) { pickerClosed() }
        Picker.AudioDelay -> AudioDelayDialog({ pb.audioDelayMs }, { pb.changeAudioDelay(it) }) { pickerClosed() }
        null -> {}
    }
    if (styling) SubtitleStyleDialog(vm) { styling = false }
    LaunchedEffect(styling) {
        if (!styling) {
            delay(120)
            runCatching { focus.requestFocus() }
        }
    }
}

private val SPEEDS = listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f)

/** 250 → "+0.25 s", -1500 → "−1.5 s". */
internal fun delayLabel(ms: Int): String {
    if (ms == 0) return "0 s"
    val s = java.math.BigDecimal(kotlin.math.abs(ms)).movePointLeft(3).stripTrailingZeros().toPlainString()
    return (if (ms > 0) "+" else "−") + s + " s"
}

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

/** The card in the last seconds of an episode: what plays next, and when. */
@Composable
private fun UpNextCard(next: dev.madhyam.Item, secondsLeft: Int, modifier: Modifier) {
    Column(
        modifier
            .width(360.dp)
            .background(Color.Black.copy(alpha = 0.82f), RoundedCornerShape(14.dp))
            .padding(18.dp),
    ) {
        Text("UP NEXT", color = Accent, fontSize = 12.sp)
        Text(next.episodeLabel, color = Color.White, fontSize = 20.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Row(
                Modifier.background(Color.White, RoundedCornerShape(50)).padding(horizontal = 16.dp, vertical = 9.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.Black, modifier = Modifier.size(20.dp))
                Text(" Play now  OK", color = Color.Black, fontSize = 14.sp)
            }
            Spacer(Modifier.width(12.dp))
            Column {
                Text(if (secondsLeft > 0) "Starts in $secondsLeft s" else "Starting…", color = Color.White, fontSize = 13.sp)
                Text("Back hides this", color = Muted, fontSize = 12.sp)
            }
        }
    }
}
