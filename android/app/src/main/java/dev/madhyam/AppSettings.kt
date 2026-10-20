package dev.madhyam

import android.content.Context
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * User preferences, kept in settings.json in app-private storage (the same idea as the
 * Windows app's settings.json). The simple properties are snapshot state, so the screens that
 * show them recompose when they change; the collections are not (screens that show them read
 * MainViewModel.settingsVersion). Call [save] after changing anything.
 */
class AppSettings private constructor(private val file: File) {
    // ------------------------------------------------------------ general
    var startupSection by mutableStateOf("Last")            // Last | Live | Movies | Series | Favorites | Recent
    var lastSection by mutableStateOf("Live")
    var lastSourceId by mutableStateOf<Int?>(null)

    // ----------------------------------------------------------- live tv
    var stripPrefixes by mutableStateOf(false)               // "UK: BBC One" → "BBC One"
    var showNumbers by mutableStateOf(true)
    var liveSortAz by mutableStateOf(false)
    var previewOnSelect by mutableStateOf(true)              // play the selected channel in the preview window

    // ---------------------------------------------------------- playback
    var resumePlayback by mutableStateOf(true)
    var showSubtitles by mutableStateOf(true)
    var audioLanguage by mutableStateOf("")                   // ISO 639 code; "" = the stream's default
    var subtitleLanguage by mutableStateOf("")

    // ------------------------------------------------- online services
    var omdbApiKey by mutableStateOf("")                      // omdbapi.com: IMDb and Rotten Tomatoes ratings
    var osApiKey by mutableStateOf("")                        // opensubtitles.com API key
    var osUsername by mutableStateOf("")                      // optional; the password is kept in Secrets
    var osLanguages by mutableStateOf("en")                   // comma-separated ISO 639-1 codes

    // ---------------------------------------------------- movies, series
    var vodSort by mutableStateOf("provider")                 // a catalog Sort name (provider, added_desc, name_asc, rating_desc…)
    var vodMinRating by mutableStateOf(0)                     // 0 = any rating

    // ---------------------------------------------------- home, search
    var dashHero by mutableStateOf(true)
    var dashContinue by mutableStateOf(true)
    var dashLiveFavorites by mutableStateOf(true)
    var dashRecentLive by mutableStateOf(true)
    var dashFavoriteVod by mutableStateOf(true)
    var dashRecentlyAdded by mutableStateOf(true)
    var searchLive by mutableStateOf(true)
    var searchMovies by mutableStateOf(true)
    var searchSeries by mutableStateOf(true)
    var searchAllSources by mutableStateOf(true)

    // -------------------------------------------------------------- guide
    var epgRefreshHours by mutableStateOf(12)

    // --------------------------------------------------------- categories
    /** "source:kind" (or "source:live:country") → hidden category names. */
    val hiddenCategories = mutableMapOf<String, MutableSet<String>>()

    // ------------------------------------------------------------ free tv
    var freeTvSourceId by mutableStateOf<Int?>(null)
    var freeTvGroupBy by mutableStateOf("category")          // category | country
    // Free TV comes preinstalled: added once on the first start, never again after the user removes it.
    var freeTvPreinstalled by mutableStateOf(false)
    /** How long the catalog took to load last time, for the start-up progress bar. */
    var lastLoadMs by mutableStateOf(0L)

    // ------------------------------------------- 0.9.0: Windows 0.4.11 parity
    var theme by mutableStateOf("Dark")                      // Dark (Navy & Crimson) | Light | System | Custom
    var customColor by mutableStateOf("#DC143C")             // Custom theme: the picked colour
    var customLight by mutableStateOf(false)                 // Custom theme on a light base
    var coverSize by mutableStateOf("Medium")                // Tiny | Extra small | Small | Medium | Large
    var titlesUnderCovers by mutableStateOf(true)
    var epgOffsetMinutes by mutableStateOf(0)                // positive = programmes shown later
    val extraEpgUrls = mutableListOf<String>()
    /** Playlist id → user agent sent for its downloads and streams ("" = the default). */
    val sourceUserAgent = mutableMapOf<Int, String>()
    var liveFormat by mutableStateOf("Auto")                 // Auto | TS | HLS (Xtream live streams)
    var bufferMs by mutableStateOf(1500)                     // playback starts after this much is buffered
    /** Kind ("live", "movie", "series") → "provider" | "name_asc" | "name_desc". */
    val categorySort = mutableMapOf<String, String>()

    // ------------------------------------------------ 1.1.1: parental controls
    var parentalOn by mutableStateOf(false)
    /** SHA-256 of salt + PIN (hex) and the salt; never the PIN itself. "" = no PIN set. */
    var parentalPinHash by mutableStateOf("")
    var parentalPinSalt by mutableStateOf("")
    var parentalAutoAdult by mutableStateOf(true)            // treat adult-looking categories and channels as locked
    var parentalLockSettings by mutableStateOf(false)
    var parentalMaxAge by mutableStateOf(0)                  // 0 = any; else 7 | 12 | 16 | 18
    /** "source:kind" → categories locked by the user. */
    val lockedCategories = mutableMapOf<String, MutableSet<String>>()
    /** "source:kind" → categories the user unlocked although they look adult (wins over auto-lock). */
    val unlockedCategories = mutableMapOf<String, MutableSet<String>>()

    // --------------------------------------------------- 1.1.1: subtitle style
    var subFontPt by mutableStateOf(SubtitleStyle.DEFAULT_PT)
    var subFont by mutableStateOf("Sans-serif")
    var subTextColor by mutableStateOf("White")
    var subTextOpacity by mutableStateOf(100)
    var subBackColor by mutableStateOf("Black")
    var subBackOpacity by mutableStateOf(50)
    var subWindowColor by mutableStateOf("Black")
    var subWindowOpacity by mutableStateOf(0)
    var subEdge by mutableStateOf("Drop shadow")
    var subPosition by mutableStateOf("Bottom")
    var subOffset by mutableStateOf(0)                       // percent of the height, + = higher, in steps of 2

    fun resetSubtitleStyle() {
        subFontPt = SubtitleStyle.DEFAULT_PT
        subFont = "Sans-serif"
        subTextColor = "White"
        subTextOpacity = 100
        subBackColor = "Black"
        subBackOpacity = 50
        subWindowColor = "Black"
        subWindowOpacity = 0
        subEdge = "Drop shadow"
        subPosition = "Bottom"
        subOffset = 0
    }

    fun hidden(source: Int, kind: String): MutableSet<String> =
        hiddenCategories.getOrPut(hiddenKey(source, kind)) { mutableSetOf() }

    /** Whether a category is hidden, without adding an empty set (safe to call while composing). */
    fun isHidden(source: Int, kind: String, name: String): Boolean = hiddenCategories[hiddenKey(source, kind)]?.contains(name) == true

    /**
     * Writes settings.json. The JSON is built here (the caller's thread, which owns the collections);
     * the file is written on a background thread, the newest text winning when changes come quickly.
     */
    fun save() {
        synchronized(this) { pending = toJson() }
        writer.execute {
            val text = synchronized(this) { pending.also { pending = null } } ?: return@execute
            write(text)
        }
    }

    private var pending: String? = null
    private val writer = java.util.concurrent.Executors.newSingleThreadExecutor { r -> Thread(r, "settings-writer").apply { isDaemon = true } }

    private fun toJson(): String {
        val o = JSONObject()
            .put("startupSection", startupSection)
            .put("lastSection", lastSection)
            .put("lastSourceId", lastSourceId ?: JSONObject.NULL)
            .put("stripPrefixes", stripPrefixes)
            .put("showNumbers", showNumbers)
            .put("liveSortAz", liveSortAz)
            .put("previewOnSelect", previewOnSelect)
            .put("resumePlayback", resumePlayback)
            .put("showSubtitles", showSubtitles)
            .put("audioLanguage", audioLanguage)
            .put("subtitleLanguage", subtitleLanguage)
            .put("omdbApiKey", omdbApiKey)
            .put("osApiKey", osApiKey)
            .put("osUsername", osUsername)
            .put("osLanguages", osLanguages)
            .put("vodSort", vodSort)
            .put("vodMinRating", vodMinRating)
            .put("dashHero", dashHero)
            .put("dashContinue", dashContinue)
            .put("dashLiveFavorites", dashLiveFavorites)
            .put("dashRecentLive", dashRecentLive)
            .put("dashFavoriteVod", dashFavoriteVod)
            .put("dashRecentlyAdded", dashRecentlyAdded)
            .put("searchLive", searchLive)
            .put("searchMovies", searchMovies)
            .put("searchSeries", searchSeries)
            .put("searchAllSources", searchAllSources)
            .put("epgRefreshHours", epgRefreshHours)
            .put("freeTvSourceId", freeTvSourceId ?: JSONObject.NULL)
            .put("freeTvGroupBy", freeTvGroupBy)
            .put("freeTvPreinstalled", freeTvPreinstalled)
            .put("lastLoadMs", lastLoadMs)
            .put("hiddenCategories", JSONObject().apply {
                for ((k, v) in hiddenCategories) if (v.isNotEmpty()) put(k, JSONArray(v.sorted()))
            })
            .put("theme", theme)
            .put("customColor", customColor)
            .put("customLight", customLight)
            .put("coverSize", coverSize)
            .put("titlesUnderCovers", titlesUnderCovers)
            .put("epgOffsetMinutes", epgOffsetMinutes)
            .put("extraEpgUrls", JSONArray(extraEpgUrls))
            .put("sourceUserAgent", JSONObject().apply { for ((k, v) in sourceUserAgent) if (v.isNotBlank()) put(k.toString(), v) })
            .put("liveFormat", liveFormat)
            .put("bufferMs", bufferMs)
            .put("categorySort", JSONObject().apply { for ((k, v) in categorySort) put(k, v) })
            .put("parentalOn", parentalOn)
            .put("parentalPinHash", parentalPinHash)
            .put("parentalPinSalt", parentalPinSalt)
            .put("parentalAutoAdult", parentalAutoAdult)
            .put("parentalLockSettings", parentalLockSettings)
            .put("parentalMaxAge", parentalMaxAge)
            .put("lockedCategories", setsToJson(lockedCategories))
            .put("unlockedCategories", setsToJson(unlockedCategories))
            .put("subFontPt", subFontPt)
            .put("subFont", subFont)
            .put("subTextColor", subTextColor)
            .put("subTextOpacity", subTextOpacity)
            .put("subBackColor", subBackColor)
            .put("subBackOpacity", subBackOpacity)
            .put("subWindowColor", subWindowColor)
            .put("subWindowOpacity", subWindowOpacity)
            .put("subEdge", subEdge)
            .put("subPosition", subPosition)
            .put("subOffset", subOffset)
        return o.toString(2)
    }

    private fun write(text: String) {
        try {
            val tmp = File(file.path + ".tmp")
            tmp.writeText(text)
            if (!tmp.renameTo(file)) {
                tmp.copyTo(file, overwrite = true)
                tmp.delete()
            }
        } catch (e: Exception) {
            Log.w(Repository.TAG, "could not save settings", e)
        }
    }

    private fun load() {
        if (!file.exists()) return
        try {
            val o = JSONObject(file.readText())
            startupSection = o.optString("startupSection", startupSection)
            lastSection = o.optString("lastSection", lastSection)
            lastSourceId = o.optIntOrNull("lastSourceId")
            stripPrefixes = o.optBoolean("stripPrefixes", stripPrefixes)
            showNumbers = o.optBoolean("showNumbers", showNumbers)
            liveSortAz = o.optBoolean("liveSortAz", liveSortAz)
            previewOnSelect = o.optBoolean("previewOnSelect", previewOnSelect)
            resumePlayback = o.optBoolean("resumePlayback", resumePlayback)
            showSubtitles = o.optBoolean("showSubtitles", showSubtitles)
            audioLanguage = o.optString("audioLanguage", audioLanguage)
            subtitleLanguage = o.optString("subtitleLanguage", subtitleLanguage)
            omdbApiKey = o.optString("omdbApiKey", omdbApiKey)
            osApiKey = o.optString("osApiKey", osApiKey)
            osUsername = o.optString("osUsername", osUsername)
            osLanguages = o.optString("osLanguages", osLanguages)
            vodSort = o.optString("vodSort", vodSort)
            vodMinRating = o.optInt("vodMinRating", vodMinRating)
            dashHero = o.optBoolean("dashHero", dashHero)
            dashContinue = o.optBoolean("dashContinue", dashContinue)
            dashLiveFavorites = o.optBoolean("dashLiveFavorites", dashLiveFavorites)
            dashRecentLive = o.optBoolean("dashRecentLive", dashRecentLive)
            dashFavoriteVod = o.optBoolean("dashFavoriteVod", dashFavoriteVod)
            dashRecentlyAdded = o.optBoolean("dashRecentlyAdded", dashRecentlyAdded)
            searchLive = o.optBoolean("searchLive", searchLive)
            searchMovies = o.optBoolean("searchMovies", searchMovies)
            searchSeries = o.optBoolean("searchSeries", searchSeries)
            searchAllSources = o.optBoolean("searchAllSources", searchAllSources)
            epgRefreshHours = o.optInt("epgRefreshHours", epgRefreshHours)
            freeTvSourceId = o.optIntOrNull("freeTvSourceId")
            freeTvGroupBy = o.optString("freeTvGroupBy", freeTvGroupBy)
            freeTvPreinstalled = o.optBoolean("freeTvPreinstalled", freeTvPreinstalled)
            lastLoadMs = o.optLong("lastLoadMs", lastLoadMs)
            o.optJSONObject("hiddenCategories")?.let { h ->
                for (k in h.keys()) {
                    val a = h.optJSONArray(k) ?: continue
                    hiddenCategories[k] = (0 until a.length()).map { a.getString(it) }.toMutableSet()
                }
            }
            theme = o.optString("theme", theme)
            customColor = o.optString("customColor", customColor)
            customLight = o.optBoolean("customLight", customLight)
            coverSize = o.optString("coverSize", coverSize)
            titlesUnderCovers = o.optBoolean("titlesUnderCovers", titlesUnderCovers)
            epgOffsetMinutes = o.optInt("epgOffsetMinutes", epgOffsetMinutes)
            o.optJSONArray("extraEpgUrls")?.let { a -> for (i in 0 until a.length()) extraEpgUrls += a.getString(i) }
            o.optJSONObject("sourceUserAgent")?.let { m -> for (k in m.keys()) k.toIntOrNull()?.let { sourceUserAgent[it] = m.getString(k) } }
            liveFormat = o.optString("liveFormat", liveFormat)
            bufferMs = o.optInt("bufferMs", bufferMs)
            o.optJSONObject("categorySort")?.let { m -> for (k in m.keys()) categorySort[k] = m.getString(k) }
            parentalOn = o.optBoolean("parentalOn", parentalOn)
            parentalPinHash = o.optString("parentalPinHash", parentalPinHash)
            parentalPinSalt = o.optString("parentalPinSalt", parentalPinSalt)
            parentalAutoAdult = o.optBoolean("parentalAutoAdult", parentalAutoAdult)
            parentalLockSettings = o.optBoolean("parentalLockSettings", parentalLockSettings)
            parentalMaxAge = o.optInt("parentalMaxAge", parentalMaxAge)
            setsFromJson(o.optJSONObject("lockedCategories"), lockedCategories)
            setsFromJson(o.optJSONObject("unlockedCategories"), unlockedCategories)
            subFontPt = o.optInt("subFontPt", subFontPt)
            subFont = o.optString("subFont", subFont)
            subTextColor = o.optString("subTextColor", subTextColor)
            subTextOpacity = o.optInt("subTextOpacity", subTextOpacity)
            subBackColor = o.optString("subBackColor", subBackColor)
            subBackOpacity = o.optInt("subBackOpacity", subBackOpacity)
            subWindowColor = o.optString("subWindowColor", subWindowColor)
            subWindowOpacity = o.optInt("subWindowOpacity", subWindowOpacity)
            subEdge = o.optString("subEdge", subEdge)
            subPosition = o.optString("subPosition", subPosition)
            subOffset = o.optInt("subOffset", subOffset)
        } catch (e: Exception) {
            Log.w(Repository.TAG, "settings unreadable, using defaults", e)
        }
    }

    companion object {
        fun hiddenKey(source: Int, kind: String) = "$source:$kind"

        fun load(context: Context): AppSettings = AppSettings(File(context.filesDir, "settings.json")).apply { load() }

        private fun JSONObject.optIntOrNull(k: String): Int? = if (has(k) && !isNull(k)) optInt(k) else null

        private fun setsToJson(m: Map<String, Set<String>>) = JSONObject().apply {
            for ((k, v) in m) if (v.isNotEmpty()) put(k, JSONArray(v.sorted()))
        }

        private fun setsFromJson(o: JSONObject?, into: MutableMap<String, MutableSet<String>>) {
            o ?: return
            for (k in o.keys()) {
                val a = o.optJSONArray(k) ?: continue
                into[k] = (0 until a.length()).map { a.getString(it) }.toMutableSet()
            }
        }
    }
}
