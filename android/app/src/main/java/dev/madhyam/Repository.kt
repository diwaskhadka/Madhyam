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

data class Source(val id: Int, val name: String, val kind: String)

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
) {
    val isLive get() = kind == "live"
    val isSeries get() = kind == "series"
}

data class Programme(val title: String, val start: Long, val stop: Long, val desc: String) {
    /** 0..1 progress of the programme at `now` (Unix seconds). */
    fun progress(now: Long): Float =
        if (stop <= start) 0f else ((now - start).toFloat() / (stop - start)).coerceIn(0f, 1f)
}

data class NowNext(val now: Programme?, val next: Programme?)

private fun JSONObject.str(k: String): String? = optString(k, "").takeIf { it.isNotEmpty() && !isNull(k) }

private fun parseItem(o: JSONObject): Item {
    val extra = o.optJSONObject("extra") ?: JSONObject()
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
    )
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

    /** Age of the cached guide in ms, or null if none is cached. */
    fun epgAgeMs(source: Int): Long? =
        epgFiles(source).minOfOrNull { it.lastModified() }?.let { System.currentTimeMillis() - it }

    suspend fun hasEpg(source: Int): Boolean = io { core.arr("epg_fetch_urls", args("source" to source)).length() > 0 }

    private fun save() {
        try {
            core.call("save_snapshot", args("path" to snapshot.path))
        } catch (e: IptvException) {
            Log.w(TAG, "could not save snapshot", e)
        }
    }

    suspend fun sources(): List<Source> = io {
        val a = core.arr("sources")
        (0 until a.length()).map {
            val o = a.getJSONObject(it)
            Source(o.getInt("id"), o.optString("name"), o.optString("kind"))
        }
    }

    suspend fun addM3u(name: String, url: String): Int {
        val id = io { core.obj("add_m3u_source", args("name" to name, "location" to url)).getInt("id") }
        try {
            refresh(id)
        } catch (e: Exception) {
            io { core.call("remove_source", args("source" to id)) }
            throw e
        }
        return id
    }

    suspend fun addXtream(name: String, server: String, user: String, pass: String): Int {
        val id = io {
            core.obj(
                "add_xtream_source",
                args("name" to name, "server" to server, "username" to user, "password" to pass),
            ).getInt("id")
        }
        try {
            refresh(id)
        } catch (e: Exception) {
            io { core.call("remove_source", args("source" to id)) }
            throw e
        }
        return id
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
                    val file = download(src.getString("location"), "playlist")
                    try {
                        core.call("import_m3u_file", args("source" to id, "path" to file.path))
                    } finally {
                        file.delete()
                    }
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

    suspend fun groups(source: Int, kind: String): List<Group> = io {
        val a = core.arr("groups", args("source" to source, "kind" to kind))
        (0 until a.length()).map { a.getJSONObject(it).let { g -> Group(g.optString("name"), g.optInt("count")) } }
    }

    suspend fun items(source: Int, kind: String, group: String?, limit: Int = 5000): List<Item> = io {
        core.arr("list", args("source" to source, "kind" to kind, "group" to group, "limit" to limit)).items()
    }

    suspend fun search(query: String, kind: String? = null): List<Item> = io {
        if (query.isBlank()) emptyList() else core.arr("search", args("query" to query, "kind" to kind, "limit" to 200)).items()
    }

    suspend fun favorites(): List<Item> = io { core.arr("favorites").items() }

    suspend fun recent(): List<Item> = io { core.arr("recent", args("limit" to 30)).items() }

    suspend fun isFavorite(id: String): Boolean = io { core.obj("item", args("id" to id)).optBoolean("favorite") }

    suspend fun setFavorite(id: String, on: Boolean) = io {
        core.call("set_favorite", args("id" to id, "on" to on))
        save()
    }

    /** Episodes of a series; downloaded from the panel the first time. */
    suspend fun episodes(series: Item): List<Item> = io {
        var eps = core.arr("episodes", args("id" to series.id)).items()
        if (eps.isEmpty()) {
            val url = core.call("series_info_url", args("id" to series.id)) as String
            val body = http.newCall(Request.Builder().url(url).build()).execute().use { r ->
                if (!r.isSuccessful) throw IOException("HTTP ${r.code} for series info")
                r.body?.bytes() ?: ByteArray(0)
            }
            core.importBytes("series_info", body, args("id" to series.id))
            eps = core.arr("episodes", args("id" to series.id)).items()
            save()
        }
        eps
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

    suspend fun persist() = io { save() }

    companion object {
        const val TAG = "Madhyam"
        const val USER_AGENT = "Mozilla/5.0 (Linux; Android TV) Madhyam/0.1"
    }
}
