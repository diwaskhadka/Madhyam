package dev.madhyam

/**
 * Free channels from iptv-org (github.com/iptv-org/iptv): public streams collected by the
 * community. Added as an ordinary M3U playlist named "Free TV" when turned on in Settings.
 * Mirrors windows/Madhyam.Windows/FreeTv.cs.
 */
object FreeTv {
    const val NAME = "Free TV"

    /** Every channel, grouped by category (News, Sports, Kids…). */
    const val ALL_URL = "https://iptv-org.github.io/iptv/index.category.m3u"

    /** The same channels grouped by country; used only to tag channels with their countries. */
    const val COUNTRY_INDEX_URL = "https://iptv-org.github.io/iptv/index.country.m3u"

    /** Item field holding a channel's countries (';'-separated). */
    const val COUNTRY_FIELD = "country"

    fun isFreeTv(location: String?) = location.equals(ALL_URL, ignoreCase = true)
}
