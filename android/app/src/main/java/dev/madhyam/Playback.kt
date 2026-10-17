package dev.madhyam

import android.content.Context
import androidx.annotation.OptIn
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import java.util.Locale

/** One audio or subtitle track the stream offers. */
data class TrackChoice(val group: Tracks.Group, val index: Int, val label: String, val selected: Boolean)

/** How the picture fills the screen. */
enum class PictureSize(val label: String, val resizeMode: Int) {
    @OptIn(UnstableApi::class) Fit("Fit", AspectRatioFrameLayout.RESIZE_MODE_FIT),
    @OptIn(UnstableApi::class) Fill("Stretch", AspectRatioFrameLayout.RESIZE_MODE_FILL),
    @OptIn(UnstableApi::class) Zoom("Zoom", AspectRatioFrameLayout.RESIZE_MODE_ZOOM),
}

/**
 * The one ExoPlayer of the app. Live TV's preview window and the full-screen player show the
 * same instance, so going full screen never re-buffers the stream (one video surface moved
 * between pages, as in the Windows app).
 */
@OptIn(UnstableApi::class)
class Playback(private val context: Context, private val settings: AppSettings) {
    val player: ExoPlayer = ExoPlayer.Builder(context).build()

    /** What is loaded in the player, or null when stopped. */
    var current by mutableStateOf<Item?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var isPlaying by mutableStateOf(false)
        private set

    /** Channels that Up/Down (or Channel +/-) zap through, and the position of [current] in it. */
    var playlist by mutableStateOf<List<Item>>(emptyList())
        private set
    var index by mutableStateOf(0)
        private set

    var speed by mutableFloatStateOf(1f)
        private set
    var pictureSize by mutableStateOf(PictureSize.Fit)
    /** Bumped when the stream's tracks change, so track menus re-read them. */
    var tracksVersion by mutableIntStateOf(0)
        private set

    init {
        player.addListener(object : Player.Listener {
            override fun onPlayerError(e: PlaybackException) {
                if (e.errorCode == PlaybackException.ERROR_CODE_BEHIND_LIVE_WINDOW) {
                    // Live stream moved on while we buffered: jump back to the live edge.
                    player.seekToDefaultPosition()
                    player.prepare()
                } else {
                    error = "Playback failed (${e.errorCodeName}). Press OK to retry."
                }
            }

            override fun onIsPlayingChanged(playing: Boolean) {
                isPlaying = playing
                if (playing) error = null
            }

            override fun onTracksChanged(tracks: Tracks) {
                tracksVersion++
            }
        })
        applyTrackPreferences()
    }

    /** Subtitles on/off and the preferred audio and subtitle languages from Settings. */
    fun applyTrackPreferences() {
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, !settings.showSubtitles)
            .setPreferredAudioLanguage(settings.audioLanguage.ifBlank { null })
            .setPreferredTextLanguage(settings.subtitleLanguage.ifBlank { null })
            .build()
    }

    /** Start [item] (from [resumeMs]); [list] is what zapping moves through. */
    fun play(item: Item, list: List<Item> = listOf(item), resumeMs: Long = 0) {
        playlist = list.ifEmpty { listOf(item) }
        index = playlist.indexOfFirst { it.id == item.id }.coerceAtLeast(0)
        load(item, resumeMs)
    }

    private fun load(item: Item, resumeMs: Long) {
        error = null
        current = item
        // Per-stream choices start over; the Settings preferences apply again.
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon().clearOverrides().build()
        applyTrackPreferences()
        changeSpeed(1f)
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
        if (resumeMs > 0) player.seekTo(resumeMs)
        player.playWhenReady = true
    }

    /** Next/previous channel in [playlist]; returns the new item. */
    fun zap(delta: Int): Item? {
        val n = playlist.size
        if (n < 2) return null
        index = ((index + delta) % n + n) % n
        return playlist[index].also { load(it, 0) }
    }

    fun retry() {
        error = null
        player.prepare()
        player.play()
    }

    fun togglePause() {
        if (player.isPlaying) player.pause() else player.play()
    }

    fun seekBy(ms: Long) {
        val d = player.duration
        val target = (player.currentPosition + ms).coerceAtLeast(0)
        player.seekTo(if (d != C.TIME_UNSET && d > 0) target.coerceAtMost(d) else target)
    }

    fun changeSpeed(s: Float) {
        speed = s
        player.setPlaybackSpeed(s)
    }

    // -------------------------------------------------------------- tracks

    fun audioTracks(): List<TrackChoice> = tracks(C.TRACK_TYPE_AUDIO)

    fun subtitleTracks(): List<TrackChoice> = tracks(C.TRACK_TYPE_TEXT)

    /** Whether subtitles are switched off for this stream. */
    val subtitlesOff: Boolean get() = player.trackSelectionParameters.disabledTrackTypes.contains(C.TRACK_TYPE_TEXT)

    private fun tracks(type: Int): List<TrackChoice> {
        val out = mutableListOf<TrackChoice>()
        var n = 0
        for (g in player.currentTracks.groups) {
            if (g.type != type) continue
            for (i in 0 until g.length) {
                if (!g.isTrackSupported(i)) continue
                n++
                out += TrackChoice(g, i, label(g.getTrackFormat(i), type, n), g.isTrackSelected(i))
            }
        }
        return out
    }

    fun select(t: TrackChoice) {
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(t.group.type, false)
            .setOverrideForType(TrackSelectionOverride(t.group.mediaTrackGroup, t.index))
            .build()
    }

    fun turnSubtitlesOff() {
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
    }

    private fun label(f: Format, type: Int, n: Int): String {
        val lang = f.language?.takeIf { it.isNotBlank() && it != "und" }?.let {
            Locale.forLanguageTag(it).displayLanguage.ifBlank { it }
        }
        val parts = listOfNotNull(
            f.label?.takeIf { it.isNotBlank() && it != lang },
            lang,
            if (type == C.TRACK_TYPE_AUDIO && f.channelCount > 0) channels(f.channelCount) else null,
        )
        return parts.joinToString(" · ").ifBlank { (if (type == C.TRACK_TYPE_AUDIO) "Audio " else "Subtitles ") + n }
    }

    private fun channels(n: Int) = when (n) {
        1 -> "mono"
        2 -> "stereo"
        6 -> "5.1"
        8 -> "7.1"
        else -> "$n ch"
    }

    val positionMs: Long get() = player.currentPosition
    val durationMs: Long get() = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0L

    fun stop() {
        player.stop()
        player.clearMediaItems()
        current = null
        error = null
    }

    fun release() = player.release()
}
