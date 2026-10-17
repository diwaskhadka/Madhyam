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

    // -------------------------------------------------------------- guide
    var epgRefreshHours = 12

    // --------------------------------------------------------- categories
    /** "source:kind" (or "source:live:country") → hidden category names. */
    val hiddenCategories = mutableMapOf<String, MutableSet<String>>()

    // ------------------------------------------------------------ free tv
    var freeTvSourceId: Int? = null
    var freeTvGroupBy = "category"          // category | country

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
            .put("epgRefreshHours", epgRefreshHours)
            .put("freeTvSourceId", freeTvSourceId ?: JSONObject.NULL)
            .put("freeTvGroupBy", freeTvGroupBy)
            .put("hiddenCategories", JSONObject().apply {
                for ((k, v) in hiddenCategories) if (v.isNotEmpty()) put(k, JSONArray(v.sorted()))
            })
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
            epgRefreshHours = o.optInt("epgRefreshHours", epgRefreshHours)
            freeTvSourceId = o.optIntOrNull("freeTvSourceId")
            freeTvGroupBy = o.optString("freeTvGroupBy", freeTvGroupBy)
            o.optJSONObject("hiddenCategories")?.let { h ->
                for (k in h.keys()) {
                    val a = h.optJSONArray(k) ?: continue
                    hiddenCategories[k] = (0 until a.length()).map { a.getString(it) }.toMutableSet()
                }
            }
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
