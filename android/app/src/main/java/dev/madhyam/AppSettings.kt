package dev.madhyam

import android.content.Context
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * User preferences, kept in settings.json in app-private storage (the same idea as the
 * Windows app's settings.json). Every property is plain state; call [save] after changing it.
 */
class AppSettings private constructor(private val file: File) {
    // ------------------------------------------------------------ general
    var startupSection = "Last"            // Last | Live | Movies | Series | Favorites | Recent
    var lastSection = "Live"
    var lastSourceId: Int? = null

    // ----------------------------------------------------------- live tv
    var stripPrefixes = false               // "UK: BBC One" → "BBC One"
    var showNumbers = true
    var liveSortAz = false
    var previewOnSelect = true              // play the selected channel in the preview window

    // ---------------------------------------------------------- playback
    var resumePlayback = true
    var showSubtitles = true
    var audioLanguage = ""                   // ISO 639 code; "" = the stream's default
    var subtitleLanguage = ""

    // ------------------------------------------------- online services
    var omdbApiKey = ""                      // omdbapi.com: IMDb and Rotten Tomatoes ratings
    var osApiKey = ""                        // opensubtitles.com API key
    var osUsername = ""                      // optional; the password is kept in Secrets
    var osLanguages = "en"                   // comma-separated ISO 639-1 codes

    // ---------------------------------------------------- movies, series
    var vodSort = "provider"                 // a catalog Sort name (provider, added_desc, name_asc, rating_desc…)
    var vodMinRating = 0                     // 0 = any rating

    // ---------------------------------------------------- home, search
    var dashHero = true
    var dashContinue = true
    var dashLiveFavorites = true
    var dashRecentLive = true
    var dashFavoriteVod = true
    var dashRecentlyAdded = true
    var searchLive = true
    var searchMovies = true
    var searchSeries = true
    var searchAllSources = true

    // -------------------------------------------------------------- guide
    var epgRefreshHours = 12

    // --------------------------------------------------------- categories
    /** "source:kind" (or "source:live:country") → hidden category names. */
    val hiddenCategories = mutableMapOf<String, MutableSet<String>>()

    // ------------------------------------------------------------ free tv
    var freeTvSourceId: Int? = null
    var freeTvGroupBy = "category"          // category | country

    // ------------------------------------------- 0.9.0: Windows 0.4.11 parity
    var theme = "Dark"                      // Dark | Light | System
    var coverSize = "Medium"                // Small | Medium | Large
    var titlesUnderCovers = true
    var epgOffsetMinutes = 0                // positive = programmes shown later
    val extraEpgUrls = mutableListOf<String>()
    /** Playlist id → user agent sent for its downloads and streams ("" = the default). */
    val sourceUserAgent = mutableMapOf<Int, String>()
    var liveFormat = "Auto"                 // Auto | TS | HLS (Xtream live streams)
    var bufferMs = 1500                     // playback starts after this much is buffered
    /** Kind ("live", "movie", "series") → "provider" | "name_asc" | "name_desc". */
    val categorySort = mutableMapOf<String, String>()

    fun hidden(source: Int, kind: String): MutableSet<String> =
        hiddenCategories.getOrPut(hiddenKey(source, kind)) { mutableSetOf() }

    fun save() {
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
            .put("hiddenCategories", JSONObject().apply {
                for ((k, v) in hiddenCategories) if (v.isNotEmpty()) put(k, JSONArray(v.sorted()))
            })
            .put("theme", theme)
            .put("coverSize", coverSize)
            .put("titlesUnderCovers", titlesUnderCovers)
            .put("epgOffsetMinutes", epgOffsetMinutes)
            .put("extraEpgUrls", JSONArray(extraEpgUrls))
            .put("sourceUserAgent", JSONObject().apply { for ((k, v) in sourceUserAgent) if (v.isNotBlank()) put(k.toString(), v) })
            .put("liveFormat", liveFormat)
            .put("bufferMs", bufferMs)
            .put("categorySort", JSONObject().apply { for ((k, v) in categorySort) put(k, v) })
        try {
            val tmp = File(file.path + ".tmp")
            tmp.writeText(o.toString(2))
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
            o.optJSONObject("hiddenCategories")?.let { h ->
                for (k in h.keys()) {
                    val a = h.optJSONArray(k) ?: continue
                    hiddenCategories[k] = (0 until a.length()).map { a.getString(it) }.toMutableSet()
                }
            }
            theme = o.optString("theme", theme)
            coverSize = o.optString("coverSize", coverSize)
            titlesUnderCovers = o.optBoolean("titlesUnderCovers", titlesUnderCovers)
            epgOffsetMinutes = o.optInt("epgOffsetMinutes", epgOffsetMinutes)
            o.optJSONArray("extraEpgUrls")?.let { a -> for (i in 0 until a.length()) extraEpgUrls += a.getString(i) }
            o.optJSONObject("sourceUserAgent")?.let { m -> for (k in m.keys()) k.toIntOrNull()?.let { sourceUserAgent[it] = m.getString(k) } }
            liveFormat = o.optString("liveFormat", liveFormat)
            bufferMs = o.optInt("bufferMs", bufferMs)
            o.optJSONObject("categorySort")?.let { m -> for (k in m.keys()) categorySort[k] = m.getString(k) }
        } catch (e: Exception) {
            Log.w(Repository.TAG, "settings unreadable, using defaults", e)
        }
    }

    companion object {
        fun hiddenKey(source: Int, kind: String) = "$source:$kind"

        fun load(context: Context): AppSettings = AppSettings(File(context.filesDir, "settings.json")).apply { load() }

        private fun JSONObject.optIntOrNull(k: String): Int? = if (has(k) && !isNull(k)) optInt(k) else null
    }
}
