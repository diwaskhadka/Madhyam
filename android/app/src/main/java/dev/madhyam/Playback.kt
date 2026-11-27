package dev.madhyam

import android.content.Context
import android.os.Looper
import androidx.annotation.OptIn
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.ForwardingRenderer
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.ForwardingAudioSink
import androidx.media3.exoplayer.text.TextOutput
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import java.util.Locale
import androidx.media3.common.text.Cue
import androidx.media3.extractor.text.CuesWithTiming
import androidx.media3.extractor.text.DefaultSubtitleParserFactory
import androidx.media3.extractor.text.SubtitleParser
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

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
    // Audio / subtitle delay (Windows: VLC audio-delay and spu-delay). Media3 has no such
    // setting, so the audio clock and the text renderer see shifted positions instead.
    @Volatile private var audioDelayUs = 0L
    @Volatile private var subtitleDelayUs = 0L

    private val renderers = object : DefaultRenderersFactory(context) {
        override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink =
            object : ForwardingAudioSink(super.buildAudioSink(context, enableFloatOutput, enableAudioTrackPlaybackParams)!!) {
                // The video is timed to the audio clock: a clock ahead by d shows pictures d early,
                // which is the same as hearing the sound d late (positive delay = sound later).
                override fun getCurrentPositionUs(sourceEnded: Boolean): Long {
                    val pos = super.getCurrentPositionUs(sourceEnded)
                    return if (pos == AudioSink.CURRENT_POSITION_NOT_SET) pos else pos + audioDelayUs
                }
            }

        override fun buildTextRenderers(context: Context, output: TextOutput, outputLooper: Looper, extensionRendererMode: Int, out: ArrayList<Renderer>) {
            val start = out.size
            super.buildTextRenderers(context, output, outputLooper, extensionRendererMode, out)
            for (i in start until out.size) {
                out[i] = object : ForwardingRenderer(out[i]) {
                    // Positive delay = subtitles later: look up cues for an earlier time.
                    override fun render(positionUs: Long, elapsedRealtimeUs: Long) =
                        super.render(positionUs - subtitleDelayUs, elapsedRealtimeUs)
                }
            }
        }
    }

    /**
     * Settings › Playback › Buffer: playback starts once this much is buffered (applies at start-up).
     * The buffer also stops at a quarter of the app's Java heap: Media3's own limit for video is
     * 128 MB, more than a 1–2 GB TV can spare beside the catalog.
     */
    private val loadControl = DefaultLoadControl.Builder()
        .setBufferDurationsMs(
            maxOf(15_000, settings.bufferMs * 4),
            maxOf(50_000, settings.bufferMs * 8),
            settings.bufferMs,
            settings.bufferMs * 2,
        )
        .setTargetBufferBytes((Runtime.getRuntime().maxMemory() / 4).coerceIn(16L shl 20, 64L shl 20).toInt())
        .build()

    val player: ExoPlayer = ExoPlayer.Builder(context, renderers).setLoadControl(loadControl).build()

    var audioDelayMs by mutableIntStateOf(0)
        private set
    var subtitleDelayMs by mutableIntStateOf(0)
        private set

    fun changeAudioDelay(ms: Int) {
        audioDelayMs = ms
        audioDelayUs = ms * 1000L
    }

    fun changeSubtitleDelay(ms: Int) {
        subtitleDelayMs = ms
        subtitleDelayUs = ms * 1000L
    }

    /** What is loaded in the player, or null when stopped. */
    var current by mutableStateOf<Item?>(null)
        private set
    /** What went wrong, as a sentence for the screen; [errorDetail] holds the player's code. */
    var error by mutableStateOf<String?>(null)
        private set
    var errorDetail by mutableStateOf<String?>(null)
        private set
    var isPlaying by mutableStateOf(false)
        private set
    /** Paused by the user (as opposed to playing or buffering). */
    var paused by mutableStateOf(false)
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
                    error = describe(e)
                    errorDetail = e.errorCodeName.removePrefix("ERROR_CODE_")
                }
            }

            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                paused = !playWhenReady
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
        errorDetail = null
        current = item
        externalSubtitles = emptyList()
        activeExternal = null
        externalTimed = emptyList()
        parsed.clear()
        changeAudioDelay(0)
        changeSubtitleDelay(0)
        // Per-stream choices start over; the Settings preferences apply again.
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon().clearOverrides().build()
        applyTrackPreferences()
        changeSpeed(1f)
        prepare(item, resumeMs)
    }

    private fun prepare(item: Item, positionMs: Long) {
        val ua = item.userAgent ?: settings.sourceUserAgent[item.sourceId]?.takeIf { it.isNotBlank() } ?: Repository.USER_AGENT
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(ua)
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(15_000)
            .setReadTimeoutMs(30_000)
        item.referrer?.let { http.setDefaultRequestProperties(mapOf("Referer" to it)) }
        val media = MediaItem.Builder()
            .setUri(streamUrl(item))
            .build()
        // Downloaded subtitles are not part of the stream: the app draws them (see [addSubtitle]).
        val source = DefaultMediaSourceFactory(context)
            .setDataSourceFactory(DefaultDataSource.Factory(context, http))
            .createMediaSource(media)
        player.setMediaSource(source)
        player.prepare()
        if (positionMs > 0) player.seekTo(positionMs)
        player.playWhenReady = true
    }

    /** Subtitle files downloaded for the playing film or episode (OpenSubtitles). */
    var externalSubtitles by mutableStateOf<List<ExternalSubtitle>>(emptyList())
        private set

    /**
     * The downloaded subtitle on screen, or null. The app draws it over the picture (the video
     * view asks [externalCuesNow]): adding it to the stream would restart the stream.
     */
    var activeExternal by mutableStateOf<ExternalSubtitle?>(null)
        private set
    private var externalTimed: List<CuesWithTiming> = emptyList()
    private val parsed = HashMap<File, List<CuesWithTiming>>()

    /** Read a downloaded subtitle and show it, without touching the stream. */
    suspend fun addSubtitle(sub: ExternalSubtitle) {
        if (current == null) return
        parsed[sub.file] = withContext(Dispatchers.IO) { parseSubtitle(sub.file) }
        externalSubtitles = externalSubtitles.filter { it.file != sub.file } + sub
        useExternal(sub)
    }

    /** Show a downloaded subtitle instead of the stream's own. */
    fun useExternal(sub: ExternalSubtitle) {
        externalTimed = parsed[sub.file] ?: return
        activeExternal = sub
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
        tracksVersion++
    }

    /** The lines of the downloaded subtitle at the playing position (after the subtitle delay). */
    fun externalCuesNow(): List<Cue> {
        val list = externalTimed
        if (activeExternal == null || list.isEmpty()) return emptyList()
        val t = player.currentPosition * 1000 - subtitleDelayUs
        // The last line that starts by now, then the few before it that may still be showing.
        var lo = 0
        var hi = list.size - 1
        var at = -1
        while (lo <= hi) {
            val mid = (lo + hi) ushr 1
            if (list[mid].startTimeUs <= t) { at = mid; lo = mid + 1 } else hi = mid - 1
        }
        val out = ArrayList<Cue>()
        var i = at
        while (i >= 0 && i > at - 20) {
            val c = list[i]
            val end = if (c.durationUs == C.TIME_UNSET) Long.MAX_VALUE else c.endTimeUs
            if (t < end) out.addAll(0, c.cues)
            i--
        }
        return out
    }

    private fun parseSubtitle(file: File): List<CuesWithTiming> {
        val format = Format.Builder().setSampleMimeType(subtitleMime(file.name)).build()
        val factory = DefaultSubtitleParserFactory()
        if (!factory.supportsFormat(format)) return emptyList()
        val out = ArrayList<CuesWithTiming>()
        factory.create(format).parse(file.readBytes(), SubtitleParser.OutputOptions.allCues()) { out.add(it) }
        return out.sortedBy { it.startTimeUs }
    }

    /**
     * Settings › Playback › Live stream format: Xtream live URLs end in .ts or .m3u8 and the
     * server offers both; switch the extension unless it is Auto (as the Windows app does).
     */
    private fun streamUrl(item: Item): String {
        val url = item.url
        if (!item.isLive || settings.liveFormat == "Auto" || !url.contains("/live/")) return url
        val (from, to) = if (settings.liveFormat == "HLS") ".ts" to ".m3u8" else ".m3u8" to ".ts"
        return if (url.endsWith(from, ignoreCase = true)) url.dropLast(from.length) + to else url
    }

    private fun subtitleMime(name: String): String = when (name.substringAfterLast('.').lowercase()) {
        "vtt" -> MimeTypes.TEXT_VTT
        "ass", "ssa" -> MimeTypes.TEXT_SSA
        "ttml", "dfxp", "xml" -> MimeTypes.APPLICATION_TTML
        else -> MimeTypes.APPLICATION_SUBRIP
    }

    /** Next/previous channel in [playlist]; returns the new item. */
    /** The next (or previous) channel of the list, passing over the ones [skip] rejects (parental locks). */
    fun zap(delta: Int, skip: (Item) -> Boolean = { false }): Item? {
        val n = playlist.size
        if (n < 2) return null
        var i = index
        repeat(n - 1) {
            i = ((i + delta) % n + n) % n
            if (!skip(playlist[i])) {
                index = i
                return playlist[i].also { load(it, 0) }
            }
        }
        return null
    }

    fun retry() {
        error = null
        errorDetail = null
        player.prepare()
        player.play()
    }

    fun togglePause() {
        if (player.playWhenReady) player.pause() else player.play()
    }

    fun play() = player.play()

    fun pause() = player.pause()

    /** What the stream really is, as TiviMate shows it beside the title: "3840x2160", "24 FPS", "STEREO". */
    @androidx.annotation.OptIn(UnstableApi::class)
    fun streamBadges(): List<String> = buildList {
        val v = player.videoFormat
        val w = v?.width?.takeIf { it > 0 } ?: player.videoSize.width
        val h = v?.height?.takeIf { it > 0 } ?: player.videoSize.height
        if (w > 0 && h > 0) add("${w}x$h")
        v?.frameRate?.takeIf { it > 0f }?.let { add("${Math.round(it)} FPS") }
        // High dynamic range, only when the stream has it.
        when {
            v == null -> {}
            v.sampleMimeType == MimeTypes.VIDEO_DOLBY_VISION || v.codecs?.startsWith("dv") == true -> add("DOLBY VISION")
            v.colorInfo?.colorTransfer == C.COLOR_TRANSFER_ST2084 -> add("HDR10")
            v.colorInfo?.colorTransfer == C.COLOR_TRANSFER_HLG -> add("HLG")
        }
        val a = player.audioFormat
        a?.channelCount?.takeIf { it > 0 }?.let {
            add(when (it) { 1 -> "MONO"; 2 -> "STEREO"; 6 -> "5.1"; 8 -> "7.1"; else -> "$it CH" })
        }
        // The sound format, when it says more than plain stereo/surround.
        when (a?.sampleMimeType) {
            MimeTypes.AUDIO_E_AC3_JOC -> "DOLBY ATMOS"
            MimeTypes.AUDIO_TRUEHD -> "DOLBY TRUEHD"
            MimeTypes.AUDIO_E_AC3 -> "DOLBY DIGITAL+"
            MimeTypes.AUDIO_AC3 -> "DOLBY DIGITAL"
            MimeTypes.AUDIO_DTS_HD -> "DTS-HD"
            MimeTypes.AUDIO_DTS_X -> "DTS:X"
            MimeTypes.AUDIO_DTS, MimeTypes.AUDIO_DTS_EXPRESS -> "DTS"
            MimeTypes.AUDIO_AAC -> "AAC"
            MimeTypes.AUDIO_AC4 -> "DOLBY AC-4"
            else -> null
        }?.let { add(it) }
    }

    /** A plain sentence for a playback error (the code goes to [errorDetail]). */
    private fun describe(e: PlaybackException): String {
        val what = if (current?.isLive == true) "this channel" else "this stream"
        val http = (e.cause as? androidx.media3.datasource.HttpDataSource.InvalidResponseCodeException)?.responseCode
        return when (e.errorCode) {
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT ->
                "Couldn't connect to $what. The provider may be offline, or your account may be using all its connections."
            PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS -> when (http) {
                401, 403 -> "The provider refused $what. Your account may have expired or be using all its connections."
                404, 410 -> "The provider no longer has $what."
                else -> "The provider couldn't send $what${http?.let { " (error $it)" } ?: ""}. Try again in a moment."
            }
            PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> "The provider no longer has $what."
            PlaybackException.ERROR_CODE_IO_INVALID_HTTP_CONTENT_TYPE,
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED,
            PlaybackException.ERROR_CODE_PARSING_MANIFEST_MALFORMED ->
                "The provider sent something that isn't a video for $what."
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED,
            PlaybackException.ERROR_CODE_PARSING_MANIFEST_UNSUPPORTED,
            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
            PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED,
            PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
            PlaybackException.ERROR_CODE_DECODING_FORMAT_EXCEEDS_CAPABILITIES ->
                "This TV can't play the format of $what."
            else -> "Playback of $what stopped with an error."
        }
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
        if (t.group.type == C.TRACK_TYPE_TEXT) activeExternal = null
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(t.group.type, false)
            .setOverrideForType(TrackSelectionOverride(t.group.mediaTrackGroup, t.index))
            .build()
    }

    fun turnSubtitlesOff() {
        activeExternal = null
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
        errorDetail = null
    }

    /**
     * Stop decoding and close the stream (the audio track, the decoders and the provider's
     * connection go with it), but keep [current] so [resume] can start it again.
     */
    fun suspend() {
        player.stop()
    }

    /** Start the stream [suspend] stopped: live from the live edge, films from where they were. */
    fun resume() {
        val item = current ?: return
        if (player.playbackState != Player.STATE_IDLE) return
        if (item.isLive) player.seekToDefaultPosition()
        player.prepare()
        player.playWhenReady = true
    }

    fun release() = player.release()
}
