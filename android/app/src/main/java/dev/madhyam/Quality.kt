package dev.madhyam

import java.text.Normalizer

/**
 * Picture quality guessed from the tags in an item's name and its category, because playlists
 * don't send the real resolution until playback ("4K-TOP - …", "TOP MOVIES ⁴ᴷ ³⁸⁴⁰ᴾ …", "… FHD").
 */
enum class Quality(val label: String) {
    UHD("4K"),
    FHD("Full HD"),
    HD("HD"),
    Other("Other");

    companion object {
        private val TAGS: Map<String, Quality> = buildMap {
            listOf("4K", "UHD", "2160P", "3840P", "8K", "4320P").forEach { put(it, UHD) }
            listOf("FHD", "1080P", "1080I", "BLURAY", "BLURAYRIP", "BDRIP", "BRRIP", "REMUX").forEach { put(it, FHD) }
            listOf("HD", "720P", "HDRIP").forEach { put(it, HD) }
        }

        // Words of letters and digits; "BLU-RAY" and "FULL HD" are joined first.
        private val WORD = Regex("[A-Z0-9]+")

        /** The best quality named in [text], or null when it names none. */
        fun inText(text: String): Quality? {
            if (text.isEmpty()) return null
            // NFKC turns the small "⁴ᴷ ³⁸⁴⁰ᴾ" and "ᴴᴰ" some playlists use into plain 4K 3840P HD.
            val t = Normalizer.normalize(text, Normalizer.Form.NFKC).uppercase()
                .replace("BLU-RAY", "BLURAY").replace("BLU RAY", "BLURAY")
                .replace("FULL HD", "FHD").replace("FULLHD", "FHD").replace("ULTRA HD", "UHD")
            var best: Quality? = null
            for (m in WORD.findAll(t)) {
                val q = TAGS[m.value] ?: continue
                if (best == null || q.ordinal < best.ordinal) best = q
                if (best == UHD) break
            }
            return best
        }

        /** The item's own name wins over its category's ("4K-TOP" in a plain "Movies" category). */
        fun of(item: Item): Quality = inText(item.name) ?: inText(item.group) ?: Other
    }
}
