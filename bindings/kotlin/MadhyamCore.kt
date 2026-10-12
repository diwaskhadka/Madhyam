// Kotlin binding for Android / Android TV, via JNA.
//
// NOT compiled in the sandbox this was written in (no Maven access) — the C ABI
// it calls is tested end-to-end by bindings/python/e2e_test.py.
//
// Gradle (app module):
//     implementation("net.java.dev.jna:jna:5.17.0@aar")
//     implementation("com.squareup.okhttp3:okhttp:4.12.0")      // for the sync example
//     implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
//
// Put the cross-compiled libraries here (see README "Building for Android"):
//     app/src/main/jniLibs/arm64-v8a/libmadhyam_core_ffi.so
//     app/src/main/jniLibs/armeabi-v7a/libmadhyam_core_ffi.so    // many TV boxes are still 32-bit
//     app/src/main/jniLibs/x86_64/libmadhyam_core_ffi.so         // emulator

package dev.madhyam.core

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.NativeLong
import com.sun.jna.Pointer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

private interface NativeLib : Library {
    fun iptv_core_new(): Pointer
    fun iptv_core_free(core: Pointer)
    fun iptv_call(core: Pointer, method: String, args: String?): Pointer
    // size_t == C long on Android (both 32- and 64-bit), so NativeLong is correct here.
    // (It is NOT correct on 64-bit Windows; use the C# binding there.)
    fun iptv_import(core: Pointer, method: String, data: ByteArray, len: NativeLong, args: String?): Pointer
    fun iptv_string_free(s: Pointer)
    fun iptv_version(): String
}

class IptvException(val code: String, message: String) : Exception("$code: $message")

/**
 * One instance per app (it holds the whole catalog). All methods block:
 * use the `suspend` helpers, or call from a background thread.
 */
class MadhyamCore : AutoCloseable {
    private val lib: NativeLib = Native.load(
        "madhyam_core_ffi",
        NativeLib::class.java,
        mapOf(Library.OPTION_STRING_ENCODING to "UTF-8"),
    )
    private var handle: Pointer? = lib.iptv_core_new()

    val version: String get() = lib.iptv_version()

    private fun unwrap(p: Pointer): Any? {
        val text = try { p.getString(0, "UTF-8") } finally { lib.iptv_string_free(p) }
        val env = JSONObject(text)
        env.optJSONObject("error")?.let { throw IptvException(it.getString("code"), it.getString("message")) }
        return env.opt("ok")
    }

    /** Low-level call. Result is JSONObject, JSONArray, String, Number, Boolean or null. */
    fun call(method: String, args: JSONObject = JSONObject()): Any? =
        unwrap(lib.iptv_call(checkNotNull(handle) { "closed" }, method, args.toString()))

    fun importBytes(method: String, data: ByteArray, args: JSONObject = JSONObject()): Any? =
        unwrap(lib.iptv_import(checkNotNull(handle) { "closed" }, method, data, NativeLong(data.size.toLong()), args.toString()))

    override fun close() {
        handle?.let(lib::iptv_core_free)
        handle = null
    }

    // ---------------------------------------------------------- typed helpers

    suspend fun search(query: String, kind: String? = null, limit: Int = 50): JSONArray = io {
        call("search", JSONObject().put("query", query).put("limit", limit).apply { kind?.let { put("kind", it) } }) as JSONArray
    }

    suspend fun nowNext(itemId: String): JSONObject = io { call("now_next", JSONObject().put("id", itemId)) as JSONObject }

    suspend fun groups(source: Int, kind: String): JSONArray =
        io { call("groups", JSONObject().put("source", source).put("kind", kind)) as JSONArray }

    suspend fun list(source: Int, kind: String, group: String?, offset: Int, limit: Int): JSONArray = io {
        val a = JSONObject().put("source", source).put("kind", kind).put("offset", offset).put("limit", limit)
        group?.let { a.put("group", it) }
        call("list", a) as JSONArray
    }

    suspend fun setProgress(itemId: String, positionSec: Double, durationSec: Double) = io {
        call("set_progress", JSONObject().put("id", itemId).put("position", positionSec).put("duration", durationSec))
    }

    /**
     * Full Xtream refresh. The core says what to download; OkHttp downloads it.
     * Returns `{"items": n, "account": {...}, "warnings": [...]}`.
     */
    suspend fun syncXtream(source: Int, http: OkHttpClient): JSONObject = io {
        val reqs = call("xtream_requests", JSONObject().put("source", source)) as JSONArray
        for (i in 0 until reqs.length()) {
            val r = reqs.getJSONObject(i)
            val body = http.newCall(Request.Builder().url(r.getString("url")).build()).execute().use { resp ->
                if (!resp.isSuccessful) throw IptvException("api", "HTTP ${resp.code} for ${r.getString("part")}")
                resp.body!!.bytes()
            }
            importBytes("xtream_part", body, JSONObject().put("source", source).put("part", r.getString("part")))
        }
        call("xtream_commit", JSONObject().put("source", source)) as JSONObject
    }

    /** Download every guide for a source. Big guides: stream to a cache file and use import_xmltv_file instead. */
    suspend fun refreshEpg(source: Int, http: OkHttpClient) = io {
        val urls = call("epg_fetch_urls", JSONObject().put("source", source)) as JSONArray
        for (i in 0 until urls.length()) {
            http.newCall(Request.Builder().url(urls.getString(i)).build()).execute().use { resp ->
                if (resp.isSuccessful) {
                    val raw = resp.body!!.byteStream()
                    // Hosts decompress; the core only reads plain XML.
                    val stream = if (urls.getString(i).endsWith(".gz")) java.util.zip.GZIPInputStream(raw) else raw
                    importBytes("xmltv", stream.readBytes())
                }
            }
        }
    }

    private suspend fun <T> io(block: () -> T): T = withContext(Dispatchers.IO) { block() }

    companion object {
        /** Helper for code that wants plain Kotlin types. */
        fun parse(json: String): Any? = JSONTokener(json).nextValue()
    }
}
