// Kotlin binding for Android / Android TV, via JNA.
//
// Compiled by the Android app in ../../android (it adds this folder as a source
// directory), so CI builds it on every push.
//
// Gradle dependencies:
//     implementation("net.java.dev.jna:jna:5.15.0@aar")
//     implementation("com.squareup.okhttp3:okhttp:4.12.0")
//
// The native libraries go in app/src/main/jniLibs/<abi>/libmadhyam_core_ffi.so
// (built by `cargo ndk`, see README).

package dev.madhyam.core

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.NativeLong
import com.sun.jna.Pointer
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject

/** Mirror of include/madhyam_core.h. JNA generates the implementation at runtime. */
internal interface NativeLib : Library {
    fun iptv_core_new(): Pointer
    fun iptv_core_free(core: Pointer)
    fun iptv_call(core: Pointer, method: String, args: String?): Pointer
    // size_t == C `long` on Android (32- and 64-bit), so NativeLong is correct here.
    // It is NOT correct on 64-bit Windows; use the C# binding there.
    fun iptv_import(core: Pointer, method: String, data: ByteArray, len: NativeLong, args: String?): Pointer
    fun iptv_string_free(s: Pointer)
    fun iptv_version(): String
}

class IptvException(val code: String, message: String) : Exception("$code: $message")

/** Build a JSONObject from pairs: `args("source" to 1, "kind" to "live")`. Null values are skipped. */
fun args(vararg pairs: Pair<String, Any?>): JSONObject {
    val o = JSONObject()
    for ((k, v) in pairs) if (v != null) o.put(k, v)
    return o
}

/**
 * One instance per app (it holds the whole catalog). Thread-safe, but every
 * call blocks: never call it on the main thread.
 */
class MadhyamCore : AutoCloseable {
    private val lib: NativeLib = Native.load(
        "madhyam_core_ffi",
        NativeLib::class.java,
        mapOf(Library.OPTION_STRING_ENCODING to "UTF-8"),
    )

    @Volatile
    private var handle: Pointer? = lib.iptv_core_new()

    val version: String get() = lib.iptv_version()

    private fun unwrap(p: Pointer): Any? {
        val text = try {
            p.getString(0, "UTF-8")
        } finally {
            lib.iptv_string_free(p)
        }
        val env = JSONObject(text)
        env.optJSONObject("error")?.let { throw IptvException(it.optString("code"), it.optString("message")) }
        return if (env.isNull("ok")) null else env.get("ok")
    }

    private fun h(): Pointer = handle ?: throw IllegalStateException("MadhyamCore is closed")

    /** Low-level call. Result is JSONObject, JSONArray, String, Number, Boolean or null. */
    fun call(method: String, a: JSONObject = JSONObject()): Any? = unwrap(lib.iptv_call(h(), method, a.toString()))

    fun obj(method: String, a: JSONObject = JSONObject()): JSONObject = call(method, a) as? JSONObject ?: JSONObject()
    fun arr(method: String, a: JSONObject = JSONObject()): JSONArray = call(method, a) as? JSONArray ?: JSONArray()

    fun importBytes(method: String, data: ByteArray, a: JSONObject = JSONObject()): Any? =
        unwrap(lib.iptv_import(h(), method, data, NativeLong(data.size.toLong()), a.toString()))

    /**
     * Full Xtream refresh. The core lists the URLs; OkHttp downloads them.
     * Returns `{"items": n, "account": {...}, "warnings": [...]}`. Blocking.
     */
    fun syncXtream(source: Int, http: OkHttpClient, progress: ((part: String, bytes: Long) -> Unit)? = null): JSONObject {
        val reqs = arr("xtream_requests", args("source" to source))
        for (i in 0 until reqs.length()) {
            val r = reqs.getJSONObject(i)
            val part = r.getString("part")
            progress?.invoke(part, 0)
            val body = http.newCall(Request.Builder().url(r.getString("url")).build()).execute().use { resp ->
                if (!resp.isSuccessful) throw IptvException("api", "HTTP ${resp.code} while downloading $part")
                val input = resp.body?.byteStream() ?: return@use ByteArray(0)
                val out = java.io.ByteArrayOutputStream()
                val chunk = ByteArray(64 * 1024)
                var total = 0L
                var reported = 0L
                while (true) {
                    val n = input.read(chunk)
                    if (n < 0) break
                    out.write(chunk, 0, n)
                    total += n
                    if (total - reported >= 256 * 1024) {
                        reported = total
                        progress?.invoke(part, total)
                    }
                }
                progress?.invoke(part, total)
                out.toByteArray()
            }
            importBytes("xtream_part", body, args("source" to source, "part" to part))
        }
        progress?.invoke("commit", 0)
        return obj("xtream_commit", args("source" to source))
    }

    override fun close() {
        val p = handle ?: return
        handle = null
        lib.iptv_core_free(p)
    }
}
