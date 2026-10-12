package dev.madhyam.ui

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.tv.material3.Text
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.NowNext
import dev.madhyam.Repository
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/**
 * Full-screen playback with remote-control keys:
 *  - Live:  Up/Down or Channel +/- zap through the current list, OK shows info.
 *  - Video: OK or Play/Pause toggles, Left/Right seek 10 s, FF/Rewind seek 30 s.
 *  - Back returns to the list (progress is saved).
 */
@OptIn(UnstableApi::class)
@Composable
fun PlayerScreen(vm: MainViewModel, playlist: List<Item>, index: Int) {
    val item = playlist[index]
    val context = LocalContext.current
    val player = remember { ExoPlayer.Builder(context).build() }
    val focus = remember { FocusRequester() }

    var error by remember { mutableStateOf<String?>(null) }
    var nowNext by remember { mutableStateOf<NowNext?>(null) }
    var overlayToken by remember { mutableIntStateOf(0) } // bump to show the overlay
    var overlayVisible by remember { mutableStateOf(true) }
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    var playing by remember { mutableStateOf(true) }

    // Declared first so it is disposed last (effects dispose in reverse order).
    DisposableEffect(Unit) {
        val listener = object : Player.Listener {
            override fun onPlayerError(e: PlaybackException) {
                if (e.errorCode == PlaybackException.ERROR_CODE_BEHIND_LIVE_WINDOW) {
                    // Live stream moved on while we buffered: jump back to the live edge.
                    player.seekToDefaultPosition()
                    player.prepare()
                } else {
                    error = "Playback failed (${e.errorCodeName}). Press OK to retry, Back to leave."
                }
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                playing = isPlaying
            }
        }
        player.addListener(listener)
        onDispose {
            player.removeListener(listener)
            player.release()
        }
    }

    // Save progress of the item being left (runs before the next item starts).
    DisposableEffect(item.id) {
        onDispose { vm.saveProgress(item, player.currentPosition, player.duration) }
    }

    LaunchedEffect(item.id) {
        error = null
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(item.userAgent ?: Repository.USER_AGENT)
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(15_000)
            .setReadTimeoutMs(30_000)
        item.referrer?.let { http.setDefaultRequestProperties(mapOf("Referer" to it)) }
        val source = DefaultMediaSourceFactory(context)
            .setDataSourceFactory(http)
            .createMediaSource(MediaItem.fromUri(item.url))
        player.setMediaSource(source)
        player.prepare()
        val resume = vm.resumePositionMs(item)
        if (resume > 0) player.seekTo(resume)
        player.playWhenReady = true
        vm.onPlaybackStarted(item)
        overlayToken++
        nowNext = vm.nowNextFor(item)
    }

    // Periodic progress save for films/episodes (cheap; ignored for live).
    LaunchedEffect(item.id) {
        while (isActive) {
            delay(15_000)
            vm.saveProgress(item, player.currentPosition, player.duration)
        }
    }

    // Overlay auto-hide, with a live clock while it is visible.
    LaunchedEffect(overlayToken) {
        overlayVisible = true
        val until = System.currentTimeMillis() + 4_000
        while (isActive && System.currentTimeMillis() < until) {
            position = player.currentPosition
            duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0L
            delay(250)
        }
        overlayVisible = false
    }

    LaunchedEffect(Unit) { focus.requestFocus() }
    BackHandler { vm.closePlayer() }

    fun seekBy(ms: Long) {
        val d = player.duration
        val target = (player.currentPosition + ms).coerceAtLeast(0)
        player.seekTo(if (d != C.TIME_UNSET && d > 0) target.coerceAtMost(d) else target)
        overlayToken++
    }

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(focus)
            .focusable()
            .onPreviewKeyEvent { ev ->
                if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (ev.nativeKeyEvent.keyCode) {
                    AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER -> {
                        when {
                            error != null -> {
                                error = null
                                player.prepare()
                                player.play()
                            }
                            item.isLive -> {}
                            player.isPlaying -> player.pause()
                            else -> player.play()
                        }
                        overlayToken++
                        true
                    }
                    AndroidKeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> {
                        if (player.isPlaying) player.pause() else player.play()
                        overlayToken++
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_UP, AndroidKeyEvent.KEYCODE_CHANNEL_UP ->
                        if (item.isLive) { vm.zap(-1); true } else { overlayToken++; true }
                    AndroidKeyEvent.KEYCODE_DPAD_DOWN, AndroidKeyEvent.KEYCODE_CHANNEL_DOWN ->
                        if (item.isLive) { vm.zap(1); true } else { overlayToken++; true }
                    AndroidKeyEvent.KEYCODE_DPAD_LEFT -> if (item.isLive) false else { seekBy(-10_000); true }
                    AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (item.isLive) false else { seekBy(10_000); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_REWIND -> { seekBy(-30_000); true }
                    AndroidKeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> { seekBy(30_000); true }
                    else -> false
                }
            },
    ) {
        AndroidView(
            factory = { ctx ->
                PlayerView(ctx).apply {
                    useController = false // our own TV-style overlay below
                    keepScreenOn = true
                    resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                    isFocusable = false
                    this.player = player
                }
            },
            modifier = Modifier.fillMaxSize(),
        )

        if (overlayVisible || error != null) {
            Column(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .background(Color.Black.copy(alpha = 0.65f))
                    .padding(horizontal = 48.dp, vertical = 24.dp),
            ) {
                Text(item.name, fontSize = 26.sp, color = Color.White)
                if (item.isLive) {
                    val now = System.currentTimeMillis() / 1000
                    nowNext?.now?.let {
                        Text("Now: ${it.title}", color = Color.White, fontSize = 16.sp)
                        ProgressBar(it.progress(now))
                    }
                    nowNext?.next?.let { Text("Next: ${it.title}", color = Muted, fontSize = 14.sp) }
                    if (playlist.size > 1) {
                        Text("${index + 1} / ${playlist.size}   ▲▼ change channel", color = Muted, fontSize = 12.sp)
                    }
                } else if (duration > 0) {
                    ProgressBar(position.toFloat() / duration, Modifier.fillMaxWidth())
                    Text(
                        "${if (playing) "▶" else "❚❚"}  ${fmt(position)} / ${fmt(duration)}   ◀▶ 10 s",
                        color = Muted,
                        fontSize = 14.sp,
                    )
                }
                error?.let { Text(it, color = Color(0xFFFF7A7A), fontSize = 16.sp) }
            }
        }
    }
}

private fun fmt(ms: Long): String {
    val s = ms / 1000
    val h = s / 3600
    val m = s % 3600 / 60
    val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}
