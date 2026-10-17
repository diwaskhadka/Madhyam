package dev.madhyam

import android.content.Context
import android.util.Log
import dev.madhyam.core.MadhyamCore
import dev.madhyam.core.IptvException
import dev.madhyam.core.args
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.util.concurrent.TimeUnit
import java.util.zip.GZIPInputStream

// ----------------------------------------------------------------- models

data class Source(
    val id: Int,
    val name: String,
    val kind: String,
    val location: String = "",
    val username: String = "",
    val updatedAt: Long = 0,
) {
    val isXtream get() = kind == "xtream"
    val isFreeTv get() = FreeTv.isFreeTv(location)
}

data class Group(val name: String, val count: Int)

data class Item(
    val id: String,
    val kind: String,
    val name: String,
    val group: String,
    val logo: String,
    val url: String,
    val userAgent: String?,
    val referrer: String?,
    val archiveDays: Int,
    val plot: String?,
    val year: String? = null,
    val rating: String? = null,
    val genre: String? = null,
    /** Episodes: season and episode numbers, and the id of the series item. */
    val season: String? = null,
    val episode: String? = null,
    val seriesId: String? = null,
    /** Episode length announced by the panel, in seconds (0 = unknown). */
    val durationHint: Double = 0.0,
    /** Saved resume position and length, in seconds (0 = never played). */
    val positionSec: Double = 0.0,
    val durationSec: Double = 0.0,
) {
    val isLive get() = kind == "live"
    val isSeries get() = kind == "series"
    val isEpisode get() = kind == "episode"

    val watchedRatio: Float get() = if (durationSec > 0) (positionSec / durationSec).toFloat().coerceIn(0f, 1f) else 0f
    val isWatched get() = watchedRatio >= 0.95f
    /** Started but not finished. */
    val isStarted get() = positionSec > 0 && !isWatched

    /** "S01E03". */
    val seasonEpisode: String
        get() = "S%02dE%02d".format(season?.toIntOrNull() ?: 1, episode?.toIntOrNull() ?: 0)

    /** Name as shown, optionally without a "UK: " / "|EN| " style prefix (live channels only). */
    fun displayName(stripPrefix: Boolean): String = if (stripPrefix && isLive) stripPrefix(name) else name

    companion object {
        private val PREFIX = Regex("""^\s*(\|[^|]{1,12}\||\[[^]]{1,12}]|[A-Za-z0-9]{1,6}\s*[:|\-]\s+)\s*""")

        fun stripPrefix(name: String): String = PREFIX.replace(name, "").ifBlank { name }
    }
}

data class Programme(val title: String, val start: Long, val stop: Long, val desc: String) {
    /** 0..1 progress of the programme at `now` (Unix seconds). */
    fun progress(now: Long): Float =
        if (stop <= start) 0f else ((now - start).toFloat() / (stop - start)).coerceIn(0f, 1f)
}

data class NowNext(val now: Programme?, val next: Programme?)

data class Page(val total: Int, val items: List<Item>)

data class Account(
    val status: String,
    val expires: Long?,
    val maxConnections: Int,
    val activeConnections: Int,
    val isTrial: Boolean,
)

private fun JSONObject.str(k: String): String? = optString(k, "").takeIf { it.isNotEmpty() && !isNull(k) }

private fun parseItem(o: JSONObject): Item {
    val extra = o.optJSONObject("extra") ?: JSONObject()
    val progress = o.optJSONObject("progress")
    return Item(
        id = o.getString("id"),
        kind = o.optString("kind"),
        name = o.optString("name"),
        group = o.optString("group"),
        logo = o.optString("logo"),
        url = o.optString("url"),
        userAgent = extra.str("user_agent"),
        referrer = extra.str("referrer"),
        archiveDays = o.optInt("archive_days"),
        plot = extra.str("plot"),
        year = extra.str("year"),
        rating = extra.str("rating")?.takeIf { it != "0" && it != "0.0" },
        genre = extra.str("genre"),
        season = extra.str("season"),
        episode = extra.str("episode"),
        seriesId = o.str("series"),
        durationHint = extra.str("duration_secs")?.toDoubleOrNull() ?: 0.0,
        positionSec = progress?.optDouble("position", 0.0) ?: 0.0,
        durationSec = progress?.optDouble("duration", 0.0) ?: 0.0,
    )
}

/** Film or series details from the Xtream panel (get_vod_info / get_series_info "info"). */
data class DetailsInfo(
    val plot: String? = null,
    val cast: String? = null,
    val director: String? = null,
    val genre: String? = null,
    val released: String? = null,
    val duration: String? = null,
    val rating: String? = null,
    val country: String? = null,
    val language: String? = null,
    val poster: String? = null,
    val backdrop: String? = null,
) {
    companion object {
        /** Parse the "info" object; values may be strings or numbers, empty means absent. */
        fun parse(info: JSONObject?): DetailsInfo {
            if (info == null) return DetailsInfo()
            fun one(k: String): String? =
                info.opt(k)?.takeIf { it != JSONObject.NULL && it !is JSONObject && it !is JSONArray }?.toString()?.trim()?.takeIf { it.isNotEmpty() }
            fun any(vararg keys: String): String? = keys.firstNotNullOfOrNull { one(it) }
            val backdrop = when (val b = info.opt("backdrop_path")) {
                is JSONArray -> (0 until b.length()).map { b.optString(it) }.firstOrNull { it.isNotBlank() }
                is String -> b.takeIf { it.isNotBlank() }
                else -> null
            }
            val secs = one("duration_secs")?.toDoubleOrNull() ?: 0.0
            var duration = if (secs > 0) clock(secs) else any("duration", "episode_run_time")
            duration?.toIntOrNull()?.let { if (it in 1..599) duration = "$it min" }
            return DetailsInfo(
                plot = any("plot", "description"),
                cast = any("cast", "actors"),
                director = any("director"),
                genre = any("genre"),
                released = any("releasedate", "releaseDate", "release_date"),
                duration = duration,
                rating = any("rating", "rating_5based")?.takeIf { it != "0" && it != "0.0" },
                country = any("country"),
                language = info.optJSONObject("audio")?.optString("language")?.takeIf { it.isNotBlank() } ?: any("language"),
                poster = any("movie_image", "cover_big", "cover"),
                backdrop = backdrop,
            )
        }

        /** 5400 → "1:30:00". */
        fun clock(seconds: Double): String {
            val s = seconds.toLong()
            return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s % 3600 / 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
        }
    }
}

private fun parseProgramme(o: JSONObject?): Programme? = o?.let {
    Programme(it.optString("title"), it.optLong("start"), it.optLong("stop"), it.optString("desc"))
}

private fun JSONArray.items(): List<Item> = (0 until length()).map { parseItem(getJSONObject(it)) }

// ------------------------------------------------------------- repository

/**
 * Everything the UI needs, as suspend functions on the IO dispatcher.
 * The Rust core does parsing, indexing and search; this class only moves
 * bytes (OkHttp) and converts JSON into Kotlin data classes.
 */
class Repository(context: Context) {
    private val core = MadhyamCore()
    private val snapshot = File(context.filesDir, "catalog.jsonl")
    private val cache = context.cacheDir
    private val secrets = Secrets(File(context.filesDir, "secrets.json"))
    val settings: AppSettings = AppSettings.load(context)

    /** Downloaded guides, re-imported at start-up (the snapshot deliberately excludes EPG). */
    private val epgDir = File(context.filesDir, "epg")
    private fun epgFile(source: Int, index: Int) = File(epgDir, "source-$source-$index.xml")
    private fun epgFiles(source: Int): List<File> =
        epgDir.listFiles { f -> f.name.startsWith("source-$source-") && f.name.endsWith(".xml") }?.toList() ?: emptyList()

    val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .followRedirects(true)
        .followSslRedirects(true)
        .addInterceptor { chain ->
            // Some panels reject requests without a browser-like user agent.
            chain.proceed(chain.request().newBuilder().header("User-Agent", USER_AGENT).build())
        }
        .build()

    private suspend fun <T> io(block: () -> T): T = withContext(Dispatchers.IO) { block() }

    val version: String get() = core.version

    /** Restore the last catalog so the app starts instantly, even offline. */
    suspend fun init() = io {
        if (snapshot.exists()) {
            try {
                core.call("load_snapshot", args("path" to snapshot.path))
                restoreSecrets()
                // Before 0.5.0 the password was written in plain text: rewrite the file without it.
                if (secrets.available && snapshotHasPasswords()) save()
            } catch (e: IptvException) {
                Log.w(TAG, "snapshot unreadable, starting empty", e)
                snapshot.delete()
            }
        }
        epgDir.listFiles { f -> f.name.endsWith(".xml") }?.forEach { f ->
            try {
                core.call("import_xmltv_file", args("path" to f.path))
            } catch (e: IptvException) {
                Log.w(TAG, "cached guide ${f.name} unreadable", e)
            }
        }
    }

    private fun snapshotHasPasswords(): Boolean = snapshot.useLines { lines ->
        lines.take(50).any { it.contains("\"t\":\"source\"") && it.contains("\"kind\":\"xtream\"") && !it.contains("\"password\":\"\"") }
    }

    /** Age of the cached guide in ms, or null if none is cached. */
    fun epgAgeMs(source: Int): Long? =
        epgFiles(source).minOfOrNull { it.lastModified() }?.let { System.currentTimeMillis() - it }

    suspend fun hasEpg(source: Int): Boolean = io { core.arr("epg_fetch_urls", args("source" to source)).length() > 0 }

    // ------------------------------------------------------------ secrets

    private fun xtreamIds(): List<Int> {
        val a = core.arr("sources")
        return (0 until a.length()).map { a.getJSONObject(it) }.filter { it.optString("kind") == "xtream" }.map { it.getInt("id") }
    }

    private fun enginePassword(id: Int): String = core.call("source_password", args("source" to id)) as? String ?: ""

    /** Copy every Xtream password from the engine into the encrypted store, dropping removed sources. */
    private fun syncSecrets() {
        if (!secrets.available) return
        val ids = xtreamIds()
        var changed = secrets.retain(ids.toSet())
        for (id in ids) changed = secrets.put(id, enginePassword(id)) || changed
        if (changed) secrets.save()
    }

    /** After loading a (redacted) snapshot, give the engine its passwords back. */
    private fun restoreSecrets() {
        for (id in xtreamIds()) {
            if (enginePassword(id).isNotEmpty()) continue // an older, unredacted file: migrated on the next save
            secrets.get(id)?.let { core.call("update_source", args("source" to id, "password" to it)) }
        }
    }

    private fun save() {
        try {
            syncSecrets()
            core.call("save_snapshot", args("path" to snapshot.path, "redact_passwords" to secrets.available))
        } catch (e: Exception) {
            Log.w(TAG, "could not save snapshot", e)
        }
    }

    // ------------------------------------------------------------ sources

    suspend fun sources(): List<Source> = io {
        val a = core.arr("sources")
        (0 until a.length()).map {
            val o = a.getJSONObject(it)
            Source(
                o.getInt("id"),
                o.optString("name"),
                o.optString("kind"),
                o.optString("location"),
                o.optString("username"),
                o.optLong("updated_at"),
            )
        }
    }

    suspend fun addM3u(name: String, url: String): Int {
        val id = io { core.obj("add_m3u_source", args("name" to name, "location" to url)).getInt("id") }
        return refreshOrRemove(id)
    }

    suspend fun addXtream(name: String, server: String, user: String, pass: String): Int {
        val id = io {
            core.obj(
                "add_xtream_source",
                args("name" to name, "server" to server, "username" to user, "password" to pass),
            ).getInt("id")
        }
        return refreshOrRemove(id)
    }

    /** Add iptv-org's Free TV playlist (or return the one already added). */
    suspend fun addFreeTv(): Int {
        sources().firstOrNull { it.isFreeTv }?.let { return it.id }
        return addM3u(FreeTv.NAME, FreeTv.ALL_URL)
    }

    private suspend fun refreshOrRemove(id: Int): Int {
        try {
            refresh(id)
        } catch (e: Exception) {
            io { core.call("remove_source", args("source" to id)) }
            throw e
        }
        return id
    }

    /** Change a source's name and connection details (blank = keep), then re-download it. */
    suspend fun updateSource(id: Int, name: String, location: String?, server: String?, username: String?, password: String?) {
        io {
            core.call(
                "update_source",
                args(
                    "source" to id,
                    "name" to name.trim().ifBlank { null },
                    "location" to location?.trim()?.ifBlank { null },
                    "server" to server?.trim()?.ifBlank { null },
                    "username" to username?.trim()?.ifBlank { null },
                    "password" to password?.ifEmpty { null },
                ),
            )
        }
        refresh(id)
    }

    suspend fun renameSource(id: Int, name: String) = io {
        core.call("rename_source", args("source" to id, "name" to name))
        save()
    }

    suspend fun removeSource(id: Int) = io {
        core.call("remove_source", args("source" to id))
        epgFiles(id).forEach { it.delete() }
        save()
    }

    /** Re-download a source's catalog, then (best effort) its guide. */
    suspend fun refresh(id: Int) {
        io {
            val src = core.arr("sources").let { a ->
                (0 until a.length()).map { a.getJSONObject(it) }.first { it.getInt("id") == id }
            }
            when (src.optString("kind")) {
                "xtream" -> core.syncXtream(id, http)
                else -> {
                    val location = src.getString("location")
                    val file = download(location, "playlist")
                    try {
                        core.call("import_m3u_file", args("source" to id, "path" to file.path))
                    } finally {
                        file.delete()
                    }
                    if (FreeTv.isFreeTv(location)) tagCountries(id)
                }
            }
            save()
        }
        try {
            refreshEpg(id)
        } catch (e: Exception) {
            // A missing guide must never block watching TV.
            Log.w(TAG, "EPG refresh failed for source $id", e)
        }
    }

    /**
     * Tag Free TV channels with their countries (iptv-org's index by country), so Live TV can
     * switch to countries. Best effort: categories still work without it.
     */
    private fun tagCountries(id: Int) {
        try {
            val file = download(FreeTv.COUNTRY_INDEX_URL, "countries")
            try {
                val n = core.importBytes("m3u_tags", file.readBytes(), args("source" to id, "field" to FreeTv.COUNTRY_FIELD))
                Log.i(TAG, "free tv: $n channels tagged with a country")
            } finally {
                file.delete()
            }
        } catch (e: Exception) {
            Log.w(TAG, "free tv country index failed", e)
        }
    }

    suspend fun refreshEpg(id: Int) = io {
        val urls = core.arr("epg_fetch_urls", args("source" to id))
        epgDir.mkdirs()
        var index = 0
        for (i in 0 until urls.length()) {
            val file = try {
                download(urls.getString(i), "guide")
            } catch (e: IOException) {
                Log.w(TAG, "guide download failed", e)
                continue
            }
            try {
                core.call("import_xmltv_file", args("path" to file.path))
                // Keep it for the next start (already decompressed).
                val dest = epgFile(id, index++)
                if (!file.renameTo(dest)) file.copyTo(dest, overwrite = true)
            } finally {
                if (file.exists()) file.delete()
            }
        }
        val keep = (0 until index).map { epgFile(id, it).name }.toSet()
        epgFiles(id).filter { it.name !in keep }.forEach { it.delete() }
        save()
    }

    /** Xtream account status (expiry, connections). */
    suspend fun account(source: Int): Account = io {
        val url = core.call("account_url", args("source" to source)) as String
        val body = get(url)
        val root = JSONObject(String(body))
        val u = root.optJSONObject("user_info") ?: throw IptvException("api", "The server did not return account information.")
        val auth = u.opt("auth")?.toString().let { it == "1" || it == "true" }
        if (!auth) throw IptvException("api", "Login rejected: check the username and password.")
        Account(
            status = u.optString("status"),
            expires = u.optString("exp_date").toLongOrNull()?.takeIf { it > 0 },
            maxConnections = u.optString("max_connections").toIntOrNull() ?: 0,
            activeConnections = u.optString("active_cons").toIntOrNull() ?: 0,
            isTrial = u.optString("is_trial") == "1",
        )
    }

    private fun get(url: String): ByteArray =
        http.newCall(Request.Builder().url(url).build()).execute().use { r ->
            if (!r.isSuccessful) throw IOException("HTTP ${r.code}")
            r.body?.bytes() ?: ByteArray(0)
        }

    /**
     * Stream a URL (or local path) to a cache file, transparently gunzipping
     * `.gz` guides. Streaming keeps a 200 MB guide from ever sitting in RAM.
     */
    private fun download(location: String, name: String): File {
        val out = File.createTempFile(name, ".tmp", cache)
        try {
            if (location.startsWith("/") || location.startsWith("file://")) {
                File(location.removePrefix("file://")).inputStream().use { copyMaybeGzip(it, out) }
                return out
            }
            http.newCall(Request.Builder().url(location).build()).execute().use { resp ->
                if (!resp.isSuccessful) throw IOException("HTTP ${resp.code} for $name")
                val body = resp.body ?: throw IOException("empty response for $name")
                body.byteStream().use { copyMaybeGzip(it, out) }
            }
            return out
        } catch (e: Exception) {
            out.delete()
            throw e
        }
    }

    private fun copyMaybeGzip(input: InputStream, out: File) {
        val buffered = BufferedInputStream(input, 64 * 1024)
        buffered.mark(2)
        val magic = ByteArray(2)
        val n = buffered.read(magic)
        buffered.reset()
        val src: InputStream =
            if (n == 2 && magic[0] == 0x1f.toByte() && magic[1] == 0x8b.toByte()) GZIPInputStream(buffered, 64 * 1024) else buffered
        out.outputStream().use { src.copyTo(it, 64 * 1024) }
    }

    // ------------------------------------------------------------ browsing

    /** Groups of a source; [groupField] groups by an item field instead (Free TV "country"). */
    suspend fun groups(source: Int, kind: String, groupField: String? = null): List<Group> = io {
        val a = core.arr("groups", args("source" to source, "kind" to kind, "group_field" to groupField))
        (0 until a.length()).map { a.getJSONObject(it).let { g -> Group(g.optString("name"), g.optInt("count")) } }
    }

    /**
     * Sorted, filtered, paged listing. [group] = null means every category of the kind
     * except [hidden] ones.
     */
    suspend fun query(
        source: Int,
        kind: String,
        group: String?,
        hidden: Collection<String> = emptyList(),
        sort: String = "provider",
        offset: Int = 0,
        limit: Int = 200,
        groupField: String? = null,
        minRating: Double = 0.0,
    ): Page = io {
        val r = core.obj(
            "query",
            args(
                "source" to source,
                "kind" to kind,
                "group" to group,
                "group_field" to groupField,
                "hidden_groups" to JSONArray(hidden.toList()),
                "sort" to sort,
                "min_rating" to minRating,
                "offset" to offset,
                "limit" to limit,
            ),
        )
        Page(r.optInt("total"), (r.optJSONArray("items") ?: JSONArray()).items())
    }

    suspend fun search(query: String, kind: String? = null): List<Item> = io {
        if (query.isBlank()) emptyList() else core.arr("search", args("query" to query, "kind" to kind, "limit" to 200)).items()
    }

    suspend fun favorites(kind: String? = null): List<Item> = io { core.arr("favorites", args("kind" to kind)).items() }

    suspend fun recent(kind: String? = null): List<Item> = io { core.arr("recent", args("kind" to kind, "limit" to 60)).items() }

    suspend fun favoriteIds(): Set<String> = io { core.arr("favorites").items().map { it.id }.toSet() }

    suspend fun setFavorite(id: String, on: Boolean) = io {
        core.call("set_favorite", args("id" to id, "on" to on))
        save()
    }

    /** One item with its saved progress, or null if it no longer exists. */
    suspend fun item(id: String): Item? = io {
        try {
            parseItem(core.obj("item", args("id" to id)))
        } catch (e: IptvException) {
            null
        }
    }

    /** Films and episodes started but not finished, most recent first. */
    suspend fun continueWatching(limit: Int = 20): List<Item> =
        io { core.arr("continue_watching", args("limit" to limit)).items() }

    // A get_series_info answer carries both the details and the episodes: fetch it once per session.
    private val seriesBodies = mutableMapOf<String, ByteArray>()
    private val detailsCache = mutableMapOf<String, DetailsInfo>()

    private fun seriesBody(series: Item): ByteArray = seriesBodies.getOrPut(series.id) {
        get(core.call("series_info_url", args("id" to series.id)) as String)
    }

    /** Episodes of a series; downloaded from the panel the first time. */
    suspend fun episodes(series: Item): List<Item> = io {
        var eps = core.arr("episodes", args("id" to series.id)).items()
        if (eps.isEmpty()) {
            core.importBytes("series_info", seriesBody(series), args("id" to series.id))
            eps = core.arr("episodes", args("id" to series.id)).items()
            save()
        }
        eps
    }

    /**
     * Plot, cast, backdrop… of a film or series (cached for the session). Null for M3U items,
     * which have no details endpoint, and when the panel does not answer.
     */
    suspend fun details(item: Item): DetailsInfo? = io {
        detailsCache[item.id]?.let { return@io it }
        try {
            val body = if (item.isSeries) seriesBody(item) else get(core.call("vod_info_url", args("id" to item.id)) as String)
            DetailsInfo.parse(JSONObject(String(body)).optJSONObject("info")).also { detailsCache[item.id] = it }
        } catch (e: IptvException) {
            null // not an Xtream item
        } catch (e: Exception) {
            Log.w(TAG, "details failed for ${item.name}", e)
            null
        }
    }

    /** Mark films or episodes as watched (or not), with one save for all of them. */
    suspend fun setWatched(items: List<Item>, watched: Boolean) = io {
        for (it in items) {
            val dur = listOf(it.durationSec, it.durationHint).firstOrNull { d -> d > 0 } ?: 1.0
            core.call("set_progress", args("id" to it.id, "position" to if (watched) dur else 0.0, "duration" to dur))
        }
        save()
    }

    // ----------------------------------------------------------------- EPG

    suspend fun nowNext(ids: List<String>): Map<String, NowNext> = io {
        if (ids.isEmpty()) return@io emptyMap()
        val res = core.obj("now_next_many", args("ids" to JSONArray(ids)))
        ids.associateWith { id ->
            val o = res.optJSONObject(id)
            NowNext(parseProgramme(o?.optJSONObject("now")), parseProgramme(o?.optJSONObject("next")))
        }
    }

    /** A channel's programmes from now until [hours] ahead (for the guide under the preview). */
    suspend fun upcoming(id: String, hours: Int = 24): List<Programme> = io {
        val now = System.currentTimeMillis() / 1000
        val r = core.obj("guide", args("ids" to JSONArray(listOf(id)), "from" to now, "to" to now + hours * 3600L))
        val a = r.optJSONArray(id) ?: JSONArray()
        (0 until a.length()).mapNotNull { parseProgramme(a.optJSONObject(it)) }.filter { it.stop > now }
    }

    // ------------------------------------------------------- playback state

    suspend fun recordPlay(id: String) = io { core.call("record_play", args("id" to id)) }

    /** Saved resume position in milliseconds, or 0. */
    suspend fun resumePositionMs(id: String): Long = io {
        val p = core.obj("item", args("id" to id)).optJSONObject("progress") ?: return@io 0L
        val pos = p.optDouble("position", 0.0)
        val dur = p.optDouble("duration", 0.0)
        // Near the end means "finished": start over.
        if (dur > 0 && pos / dur > 0.95) 0L else (pos * 1000).toLong()
    }

    suspend fun setProgress(id: String, positionMs: Long, durationMs: Long) = io {
        core.call("set_progress", args("id" to id, "position" to positionMs / 1000.0, "duration" to durationMs / 1000.0))
    }

    suspend fun clearHistory() = io {
        core.call("clear_history")
        save()
    }

    suspend fun persist() = io { save() }

    companion object {
        const val TAG = "Madhyam"
        const val USER_AGENT = "Mozilla/5.0 (Linux; Android TV) Madhyam/0.6"
    }
}
