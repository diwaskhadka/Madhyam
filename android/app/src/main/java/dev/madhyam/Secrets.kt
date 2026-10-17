package dev.madhyam

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import org.json.JSONObject
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Xtream passwords, encrypted with a key that never leaves the Android Keystore (the Android
 * counterpart of DPAPI on Windows). The catalog snapshot is then written with the passwords
 * blanked out, and they are handed back to the engine after it loads.
 *
 * The Keystore's AES keys need Android 6 (API 23). On Android 5 boxes [available] is false
 * and the passwords simply stay in the (app-private) snapshot, as before 0.5.0.
 */
class Secrets(private val file: File) {
    private val map = mutableMapOf<Int, String>() // source id → base64(iv + ciphertext)

    val available: Boolean get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.M

    init {
        if (file.exists()) {
            try {
                val o = JSONObject(file.readText())
                for (k in o.keys()) map[k.toInt()] = o.getString(k)
            } catch (e: Exception) {
                Log.w(Repository.TAG, "secrets unreadable", e)
            }
        }
    }

    fun get(source: Int): String? = map[source]?.let { decrypt(it) }

    /** Store (or replace) a password; returns true when the file changed. */
    fun put(source: Int, password: String): Boolean {
        if (!available || password.isEmpty()) return false
        if (get(source) == password) return false
        map[source] = encrypt(password) ?: return false
        return true
    }

    /** Forget sources that no longer exist; returns true when the file changed. */
    fun retain(ids: Set<Int>): Boolean = map.keys.retainAll { it < 0 || it in ids } // negative ids are app logins

    /** Forget one stored password; returns true when the file changed. */
    fun remove(id: Int): Boolean = map.remove(id) != null

    fun save() {
        val o = JSONObject()
        for ((k, v) in map) o.put(k.toString(), v)
        val tmp = File(file.path + ".tmp")
        tmp.writeText(o.toString())
        if (!tmp.renameTo(file)) {
            tmp.copyTo(file, overwrite = true)
            tmp.delete()
        }
    }

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return gen.generateKey()
    }

    private fun encrypt(plain: String): String? = try {
        val c = Cipher.getInstance(TRANSFORMATION)
        c.init(Cipher.ENCRYPT_MODE, key())
        Base64.encodeToString(c.iv + c.doFinal(plain.toByteArray()), Base64.NO_WRAP)
    } catch (e: Exception) {
        Log.w(Repository.TAG, "could not encrypt a password", e)
        null
    }

    private fun decrypt(stored: String): String? = try {
        val bytes = Base64.decode(stored, Base64.NO_WRAP)
        val c = Cipher.getInstance(TRANSFORMATION)
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes, 0, IV_LEN))
        String(c.doFinal(bytes, IV_LEN, bytes.size - IV_LEN))
    } catch (e: Exception) {
        // A restored backup on another device, or a wiped Keystore: the key is gone.
        Log.w(Repository.TAG, "could not decrypt a password", e)
        null
    }

    private companion object {
        const val ALIAS = "madhyam-passwords"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val IV_LEN = 12
    }
}
