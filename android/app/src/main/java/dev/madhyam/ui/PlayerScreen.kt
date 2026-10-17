package dev.madhyam.ui

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/**
 * Full-screen playback with remote-control keys:
 *  - Live:  Up/Down or Channel +/- zap through the channel list, OK shows the info bar.
 *  - Video: OK or Play/Pause toggles, Left/Right seek 10 s, FF/Rewind seek 30 s.
 *  - Back returns to the page (live keeps playing in the preview; progress is saved).
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
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }

    // A new channel or film shows the overlay.
    LaunchedEffect(item.id) { overlayToken++ }

    // Periodic progress save for films/episodes (ignored for live).
    LaunchedEffect(item.id) {
        while (isActive) {
            delay(15_000)
            vm.saveProgress(item)
        }
    }

    // Overlay auto-hide, with a live clock while it is visible.
    LaunchedEffect(overlayToken) {
        overlayVisible = true
        val until = System.currentTimeMillis() + 4_000
        while (isActive && System.currentTimeMillis() < until) {
            position = pb.positionMs
            duration = pb.durationMs
            delay(250)
        }
        overlayVisible = false
    }

    LaunchedEffect(Unit) { focus.requestFocus() }
    BackHandler { vm.closePlayer() }

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(focus)
            .focusable()
            .onPreviewKeyEvent { ev ->
                if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                val handled = when (ev.nativeKeyEvent.keyCode) {
                    AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> {
                        when {
                            pb.error != null -> pb.retry()
                            item.isLive -> {}
                            else -> pb.togglePause()
                        }
                        true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, AndroidKeyEvent.KEYCODE_MEDIA_PLAY, AndroidKeyEvent.KEYCODE_MEDIA_PAUSE -> {
                        pb.togglePause()
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_UP, AndroidKeyEvent.KEYCODE_CHANNEL_UP -> {
                        if (item.isLive) vm.zap(-1)
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_DOWN, AndroidKeyEvent.KEYCODE_CHANNEL_DOWN -> {
                        if (item.isLive) vm.zap(1)
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_LEFT -> if (item.isLive) true else { pb.seekBy(-10_000); true }
                    AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (item.isLive) true else { pb.seekBy(10_000); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_REWIND -> { pb.seekBy(-30_000); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> { pb.seekBy(30_000); true }
                    else -> false
                }
                if (handled) overlayToken++
                handled
            },
    ) {
        VideoSurface(pb.player, Modifier.fillMaxSize())

        if (overlayVisible || pb.error != null) {
            val strip = vm.settings.stripPrefixes
            Column(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.85f))))
                    .padding(start = 48.dp, end = 48.dp, top = 48.dp, bottom = 28.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (item.isLive) {
                        Logo(item, 72.dp, 44.dp, strip)
                        Text("  ", fontSize = 12.sp)
                    }
                    Column {
                        Text(if (item.isEpisode) "${item.seasonEpisode}  ${item.name}" else item.displayName(strip), fontSize = 26.sp, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (item.isLive && pb.playlist.size > 1) {
                            Text("${pb.index + 1} / ${pb.playlist.size}   ▲▼ change channel", color = Muted, fontSize = 12.sp)
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
                        Text("◀ ▶ 10 s    ⏪ ⏩ 30 s", color = Muted, fontSize = 14.sp)
                    }
                }
                pb.error?.let { Text(it, color = ErrorRed, fontSize = 16.sp) }
            }
        }
    }
}
