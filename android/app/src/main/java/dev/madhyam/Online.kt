package dev.madhyam

import android.util.Log
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.net.URLEncoder

/** IMDb rating and Rotten Tomatoes score of a film or series. */
data class ExternalRatings(val imdb: String?, val imdbVotes: String?, val rotten: String?, val imdbId: String?)

/**
 * Looks ratings up on OMDb (omdbapi.com, free key). Nothing is shown when there is no key,
 * no match, or the service has no score for that title. Mirrors windows/.../Ratings.cs.
 */
class Ratings(private val http: OkHttpClient) {
    private val cache = HashMap<String, ExternalRatings?>()

    fun fetch(key: String, title: String, year: String?, imdbId: String?, series: Boolean): ExternalRatings? {
        if (key.isBlank()) return null
        val cacheKey = imdbId ?: "$title|$year|$series"
        synchronized(cache) { if (cacheKey in cache) return cache[cacheKey] }
        var result: ExternalRatings? = null
        try {
            val q = if (imdbId != null) {
                "i=${enc(imdbId)}"
            } else {
                "t=${enc(cleanTitle(title))}" + (if (year?.length == 4) "&y=$year" else "") + "&type=" + (if (series) "series" else "movie")
            }
            val body = http.newCall(Request.Builder().url("https://www.omdbapi.com/?apikey=${enc(key.trim())}&$q").build()).execute().use { r ->
                if (!r.isSuccessful) throw IOException("HTTP ${r.code}")
                r.body?.string().orEmpty()
            }
            val o = JSONObject(body)
            if (o.optString("Response") == "True") {
                fun v(j: JSONObject, k: String) = j.optString(k).takeIf { it.isNotBlank() && it != "N/A" }
                var rotten: String? = null
                o.optJSONArray("Ratings")?.let { list ->
                    for (i in 0 until list.length()) {
                        val x = list.optJSONObject(i) ?: continue
                        if (v(x, "Source") == "Rotten Tomatoes") rotten = v(x, "Value")
                    }
                }
                val imdb = v(o, "imdbRating")
                if (imdb != null || rotten != null) result = ExternalRatings(imdb, v(o, "imdbVotes"), rotten, v(o, "imdbID"))
            }
        } catch (e: Exception) {
            Log.w(Repository.TAG, "ratings lookup failed for $title", e)
        }
        synchronized(cache) { cache[cacheKey] = result }
        return result
    }

    companion object {
        private val PREFIX = Regex("""^\s*(?:4K-)?[A-Z0-9]{2,4}\s*[-|:]\s+""")
        private val BRACKETS = Regex("""[(\[].*?[)\]]""")

        /** "EN - The Movie (2024) [4K]" → "The Movie". */
        fun cleanTitle(name: String): String = BRACKETS.replace(PREFIX.replace(name, ""), "").trim(' ', '-', '|')

        private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    }
}

/** One subtitle file offered by OpenSubtitles. */
data class SubtitleHit(
    val fileId: Long,
    val language: String,
    val release: String,
    val fileName: String,
    val downloads: Int,
    val hearingImpaired: Boolean,
    val title: String,
)

/** A downloaded subtitle file attached to the playing stream. */
data class ExternalSubtitle(val file: File, val language: String, val label: String)

/**
 * Minimal client for the OpenSubtitles.com REST API (the service VLsub uses). Needs a free API
 * key from opensubtitles.com; logging in raises the daily download quota.
 * Mirrors windows/.../OnlineSubtitles.cs.
 */
class OpenSubtitles(private val http: OkHttpClient, private val appVersion: String) {
    private var base = "https://api.opensubtitles.com/api/v1"
    private var token: String? = null
    private val json = "application/json".toMediaType()

    private fun request(apiKey: String, path: String, body: JSONObject? = null): JSONObject {
        val b = Request.Builder()
            .url(base + path)
            .header("Api-Key", apiKey.trim())
            .header("User-Agent", "Madhyam v$appVersion")
            .header("Accept", "application/json")
        token?.let { b.header("Authorization", "Bearer $it") }
        if (body != null) b.post(body.toString().toRequestBody(json))
        http.newCall(b.build()).execute().use { r ->
            val text = r.body?.string().orEmpty()
            if (!r.isSuccessful) {
                val msg = runCatching { JSONObject(text).optString("message") }.getOrNull()?.takeIf { it.isNotBlank() } ?: "HTTP ${r.code}"
                throw IOException(
                    if (r.code == 401 || r.code == 403) "OpenSubtitles refused the request ($msg). Check the API key and login in Settings › Online services."
                    else "OpenSubtitles: $msg",
                )
            }
            return JSONObject(text)
        }
    }

    private fun ensureLogin(apiKey: String, username: String, password: String?) {
        if (token != null || username.isBlank() || password.isNullOrEmpty()) return
        val d = request(apiKey, "/login", JSONObject().put("username", username).put("password", password))
        token = d.optString("token").takeIf { it.isNotBlank() }
        d.optString("base_url").takeIf { it.isNotBlank() }?.let { base = "https://$it/api/v1" }
    }

    fun search(apiKey: String, username: String, password: String?, query: String, languages: String, season: Int?, episode: Int?): List<SubtitleHit> {
        ensureLogin(apiKey, username, password)
        val q = mutableListOf("query=" + URLEncoder.encode(query.lowercase(), "UTF-8"))
        if (languages.isNotBlank()) q += "languages=" + URLEncoder.encode(languages.replace(" ", "").lowercase(), "UTF-8")
        season?.let { q += "season_number=$it" }
        episode?.let { q += "episode_number=$it" }
        q.sort() // the API redirects unless parameters are sorted
        val d = request(apiKey, "/subtitles?" + q.joinToString("&"))
        val data = d.optJSONArray("data") ?: return emptyList()
        val hits = mutableListOf<SubtitleHit>()
        for (i in 0 until data.length()) {
            val a = data.optJSONObject(i)?.optJSONObject("attributes") ?: continue
            val title = a.optJSONObject("feature_details")?.optString("title").orEmpty()
            val files = a.optJSONArray("files") ?: continue
            for (j in 0 until files.length()) {
                val f = files.optJSONObject(j) ?: continue
                hits += SubtitleHit(
                    fileId = f.optLong("file_id"),
                    language = a.optString("language"),
                    release = a.optString("release"),
                    fileName = f.optString("file_name"),
                    downloads = a.optInt("download_count"),
                    hearingImpaired = a.optBoolean("hearing_impaired"),
                    title = title,
                )
            }
        }
        return hits
    }

    /** Download one subtitle file into [dir]; returns the file and the remaining daily quota. */
    fun download(apiKey: String, username: String, password: String?, hit: SubtitleHit, dir: File): Pair<File, String> {
        ensureLogin(apiKey, username, password)
        val d = request(apiKey, "/download", JSONObject().put("file_id", hit.fileId))
        val link = d.optString("link").takeIf { it.isNotBlank() } ?: throw IOException("OpenSubtitles gave no download link.")
        val name = d.optString("file_name").takeIf { it.isNotBlank() } ?: "${hit.fileId}.srt"
        val remaining = d.opt("remaining")?.toString() ?: "?"
        dir.mkdirs()
        val file = File(dir, name.replace(Regex("""[^\w.\- ]"""), "_"))
        http.newCall(Request.Builder().url(link).build()).execute().use { r ->
            if (!r.isSuccessful) throw IOException("HTTP ${r.code} downloading the subtitle")
            file.writeBytes(r.body?.bytes() ?: ByteArray(0))
        }
        return file to remaining
    }
}
