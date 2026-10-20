package dev.madhyam

import android.graphics.Color
import android.graphics.Typeface
import android.util.TypedValue
import androidx.annotation.OptIn
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView

/**
 * Subtitle style (1.1.1, like YouTube's caption options; the same options on Windows and Linux).
 * Sizes are typographic points at 1080p: 1 pt = 1/72 inch at 96 dpi, so 32 pt is 42.7 pixels
 * on a 1080-pixel-high picture, scaled with the picture (a smaller window gets smaller text).
 */
object SubtitleStyle {
    const val DEFAULT_PT = 32 // TV; Windows and Linux use 24 pt

    val SIZES = listOf(12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72)
    val FONTS = listOf("Sans-serif", "Serif", "Monospace", "Casual", "Small caps")
    val TEXT_COLORS = listOf("White", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Red", "Black")
    val BACK_COLORS = listOf("Black", "White", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Red")
    val TEXT_OPACITIES = listOf(25, 50, 75, 100)
    val OPACITIES = listOf(0, 25, 50, 75, 100)
    val EDGES = listOf("None", "Drop shadow", "Raised", "Depressed", "Outline")
    val POSITIONS = listOf("Bottom", "Lower third", "Middle", "Top")
    const val OFFSET_STEP = 2
    const val OFFSET_MAX = 40

    private fun rgb(name: String): Int = when (name) {
        "Yellow" -> 0xFFFF00
        "Green" -> 0x00FF00
        "Cyan" -> 0x00FFFF
        "Blue" -> 0x0000FF
        "Magenta" -> 0xFF00FF
        "Red" -> 0xFF0000
        "Black" -> 0x000000
        else -> 0xFFFFFF
    }

    /** A named colour with an opacity in percent, as an ARGB int. */
    fun color(name: String, opacity: Int): Int = (opacity.coerceIn(0, 100) * 255 / 100 shl 24) or rgb(name)

    private fun typeface(name: String): Typeface = when (name) {
        "Serif" -> Typeface.SERIF
        "Monospace" -> Typeface.MONOSPACE
        // Android ships these families; on a device without them create() falls back to sans-serif.
        "Casual" -> Typeface.create("casual", Typeface.NORMAL)
        "Small caps" -> Typeface.create("sans-serif-smallcaps", Typeface.NORMAL)
        else -> Typeface.SANS_SERIF
    }

    private fun edge(name: String): Int = when (name) {
        "None" -> CaptionStyleCompat.EDGE_TYPE_NONE
        "Raised" -> CaptionStyleCompat.EDGE_TYPE_RAISED
        "Depressed" -> CaptionStyleCompat.EDGE_TYPE_DEPRESSED
        "Outline" -> CaptionStyleCompat.EDGE_TYPE_OUTLINE
        else -> CaptionStyleCompat.EDGE_TYPE_DROP_SHADOW
    }

    /** Text height in pixels for [pt] on a picture [heightPx] pixels high. */
    fun pixels(pt: Int, heightPx: Int): Float = pt * 96f / 72f * heightPx / 1080f

    /** Where the bottom of the subtitles sits, as a fraction of the picture height from the bottom. */
    fun bottomFraction(position: String, offsetPercent: Int): Float {
        val base = when (position) {
            "Lower third" -> 0.30f
            "Middle" -> 0.45f
            "Top" -> 0.82f
            else -> 0.08f
        }
        return (base + offsetPercent / 100f).coerceIn(0f, 0.95f)
    }

    /** Apply the user's style to [view] (embedded styles and sizes of the stream are ignored). */
    @OptIn(UnstableApi::class)
    fun apply(view: SubtitleView, s: AppSettings, heightPx: Int = view.height) {
        view.setApplyEmbeddedStyles(false)
        view.setApplyEmbeddedFontSizes(false)
        val style = CaptionStyleCompat(
            color(s.subTextColor, s.subTextOpacity),
            color(s.subBackColor, s.subBackOpacity),
            color(s.subWindowColor, s.subWindowOpacity),
            edge(s.subEdge),
            Color.BLACK,
            typeface(s.subFont),
        )
        view.setStyle(style)
        if (heightPx > 0) view.setFixedTextSize(TypedValue.COMPLEX_UNIT_PX, pixels(s.subFontPt, heightPx))
        view.setBottomPaddingFraction(bottomFraction(s.subPosition, s.subOffset))
    }

    /** Whether the user moved subtitles from the default place (then streams' own positions give way). */
    fun movesCues(s: AppSettings): Boolean = s.subPosition != "Bottom" || s.subOffset != 0

    /**
     * Text cues placed by the user's Position: the lines on screen together (in their order from
     * top to bottom) as one block without a line position of its own, so the bottom padding puts
     * it where the user chose. Picture subtitles keep their places.
     */
    fun placeCues(cues: List<Cue>): List<Cue> {
        val text = cues.filter { it.bitmap == null && it.text != null }
        if (text.isEmpty()) return cues
        val ordered = text.sortedBy { if (it.line == Cue.DIMEN_UNSET) Float.MAX_VALUE else if (it.lineType == Cue.LINE_TYPE_NUMBER && it.line < 0) 1000f + it.line else it.line }
        val joined = android.text.SpannableStringBuilder()
        for ((i, c) in ordered.withIndex()) {
            if (i > 0) joined.append("\n")
            joined.append(c.text)
        }
        val block = ordered.first().buildUpon()
            .setText(joined)
            .setLine(Cue.DIMEN_UNSET, Cue.TYPE_UNSET)
            .setLineAnchor(Cue.TYPE_UNSET)
            .setPosition(Cue.DIMEN_UNSET)
            .setPositionAnchor(Cue.TYPE_UNSET)
            .setSize(Cue.DIMEN_UNSET)
            .build()
        return cues.filter { it.bitmap != null } + block
    }

    fun offsetLabel(p: Int): String = when {
        p > 0 -> "$p% higher"
        p < 0 -> "${-p}% lower"
        else -> "No offset"
    }
}
