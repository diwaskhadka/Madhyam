package dev.madhyam

import java.security.MessageDigest
import java.security.SecureRandom

/**
 * Parental controls (1.1.1, the same rules on Windows and Linux): a 4-digit PIN kept as a salted
 * SHA-256 hash, locked categories, adult names locked automatically, an age limit for films and
 * series, and a lock on Settings. The rules are here; MainViewModel asks for the PIN.
 */
object Parental {
    const val PIN_LENGTH = 4
    const val MAX_TRIES = 5
    const val WAIT_MS = 30_000L
    /** The PIN stays entered this long after the last action that needed it. */
    const val UNLOCK_MS = 10 * 60_000L

    val AGE_LIMITS = listOf(0, 7, 12, 16, 18)

    /** Words that mark a category or channel as adult (whole words, case-insensitive). */
    val ADULT_WORDS = listOf(
        "xxx", "adult", "adults", "18+", "+18", "porn", "erotic", "erotica",
        "playboy", "hustler", "brazzers", "redlight", "private",
    )

    // A word counts when it is not glued to other letters or digits: "XXX", "|XXX|", "Adults: …",
    // "18+ Movies", "FR+18" (separators: anything that is not a letter or digit).
    private val ADULT = Regex(
        ADULT_WORDS.joinToString("|", prefix = "(?<![\\p{L}\\p{N}])(?:", postfix = ")(?![\\p{L}\\p{N}])") { Regex.escape(it) },
        RegexOption.IGNORE_CASE,
    )

    fun looksAdult(name: String): Boolean = ADULT.containsMatchIn(name)

    fun newSalt(): String = ByteArray(16).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) }

    fun hash(salt: String, pin: String): String =
        MessageDigest.getInstance("SHA-256").digest((salt + pin).toByteArray()).joinToString("") { "%02x".format(it) }

    /** The settings key of a category's kind: episodes belong to their series' categories. */
    fun kindKey(kind: String): String = when (kind) {
        "episode" -> "series"
        Item.CATCHUP -> "live"
        else -> kind
    }

    /**
     * An age from a provider's rating: G / PG → 7, PG-13 → 12, R → 16, NC-17 → 18, and numbers
     * as given ("12+", "FSK 16", "16"). Null when there is none or it can't be read.
     */
    fun age(rating: String?): Int? {
        val r = rating?.trim()?.uppercase()?.ifEmpty { null } ?: return null
        when (r.replace(" ", "")) {
            "G", "PG" -> return 7
            "PG-13", "PG13" -> return 12
            "R" -> return 16
            "NC-17", "NC17" -> return 18
        }
        return Regex("""\d{1,2}""").find(r)?.value?.toIntOrNull()?.takeIf { it in 0..21 }
    }
}
