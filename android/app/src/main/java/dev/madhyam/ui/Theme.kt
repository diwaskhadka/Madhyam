package dev.madhyam.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme
import androidx.tv.material3.lightColorScheme
import kotlin.math.roundToInt

/** The app's own colours; Settings › Appearance picks the dark or the light set (like Windows). */
data class Palette(
    val accent: Color,
    val background: Color,
    val panel: Color,
    val muted: Color,
    val text: Color,
    val dark: Boolean,
)

/** Dark theme, "Navy & Crimson": deep navy surfaces, crimson highlights (brand colours, branding/README.md). */
private val DarkPalette = Palette(
    accent = Color(0xFFF2506B),
    background = Color(0xFF0B1024),
    panel = Color(0xFF131C40),
    muted = Color(0xFF97A0C3),
    text = Color(0xFFECEEF7),
    dark = true,
)

private val LightPalette = Palette(
    accent = Color(0xFF2563EB),
    background = Color(0xFFF3F4F8),
    panel = Color(0xFFE2E5EE),
    muted = Color(0xFF5B6275),
    text = Color(0xFF151827),
    dark = false,
)

val LocalPalette = staticCompositionLocalOf { DarkPalette }

val Accent: Color @Composable @ReadOnlyComposable get() = LocalPalette.current.accent
val Background: Color @Composable @ReadOnlyComposable get() = LocalPalette.current.background
val Panel: Color @Composable @ReadOnlyComposable get() = LocalPalette.current.panel
val Muted: Color @Composable @ReadOnlyComposable get() = LocalPalette.current.muted
/** Main text colour (white on dark, near-black on light). */
val TextColor: Color @Composable @ReadOnlyComposable get() = LocalPalette.current.text

/**
 * Whether we are inside a focused list row or button, whose highlight is the opposite of the
 * page (a light bar on the dark theme): its content colour is then dark on dark pages.
 */
private val onHighlight: Boolean
    @Composable @ReadOnlyComposable get() = (LocalContentColor.current.luminance() < 0.5f) == LocalPalette.current.dark

/** Secondary text in a list row: [Muted], or the row's own colour, dimmed, on a focused row. */
val RowMuted: Color
    @Composable @ReadOnlyComposable get() = if (onHighlight) LocalContentColor.current.copy(alpha = 0.72f) else Muted

/** A value shown in a list row: [Accent], or the row's own colour on a focused row. */
val RowAccent: Color
    @Composable @ReadOnlyComposable get() = if (onHighlight) LocalContentColor.current else Accent

/**
 * TV Material for focusable widgets, plus the phone Material 3 theme because text fields only
 * exist there. [theme] is "Dark" (Navy & Crimson), "Light", "System" (follow the TV) or
 * "Custom" (built from [customColor] on a dark or, with [customLight], a light base).
 */
@Composable
fun MadhyamTheme(
    theme: String = "Dark",
    customColor: String = CRIMSON,
    customLight: Boolean = false,
    content: @Composable () -> Unit,
) {
    val p = when (theme) {
        "Light" -> LightPalette
        "System" -> if (isSystemInDarkTheme()) DarkPalette else LightPalette
        "Custom" -> customPalette(customColor, customLight)
        else -> DarkPalette
    }
    val dark = p.dark
    val tv = if (dark) {
        darkColorScheme(primary = p.accent, background = p.background, surface = p.panel)
    } else {
        lightColorScheme(primary = p.accent, background = p.background, surface = p.panel)
    }
    val m3 = if (dark) {
        androidx.compose.material3.darkColorScheme(primary = p.accent, background = p.background, surface = p.panel)
    } else {
        androidx.compose.material3.lightColorScheme(primary = p.accent, background = p.background, surface = p.panel)
    }
    MaterialTheme(colorScheme = tv) {
        androidx.compose.material3.MaterialTheme(colorScheme = m3) {
            // Text outside a TV Surface takes LocalContentColor, which defaults to black.
            CompositionLocalProvider(LocalPalette provides p, LocalContentColor provides p.text, content = content)
        }
    }
}

// ------------------------------------------------------------------ custom theme

const val CRIMSON = "#DC143C"
const val ROYAL_BLUE = "#003893"

/** Colour picker presets with the names shown for them (same list as Windows). */
val THEME_PRESETS = listOf(
    "Crimson" to CRIMSON, "Royal blue" to ROYAL_BLUE, "Vermilion" to "#E4572E", "Saffron" to "#F2A541",
    "Green" to "#2BA84A", "Teal" to "#12A4A4", "Sky blue" to "#2F6FEB", "Violet" to "#6C4AD8",
    "Purple" to "#B43FB4", "Rose" to "#E0457B", "Slate" to "#5E6B7D", "Brown" to "#8B5E3C",
)

/** "Crimson" for a preset colour, otherwise its hex code. */
fun colorName(hex: String): String = THEME_PRESETS.firstOrNull { it.second.equals(hex, ignoreCase = true) }?.first ?: hex.uppercase()

/**
 * A whole palette from one colour: surfaces take its hue, the highlight is the colour itself,
 * brightened or darkened just enough to stay readable (same rules as Windows' ThemeColors).
 */
fun customPalette(hex: String, light: Boolean): Palette {
    val c = parseColor(hex) ?: parseColor(CRIMSON)!!
    val (h, s, l) = toHsl(c)
    val tint = minOf(s, 0.55f) // strong colours give tinted greys, not neon surfaces
    return if (!light) {
        Palette(
            accent = hsl(h, maxOf(s, 0.55f), maxOf(l, 0.64f)),
            background = hsl(h, tint * 0.8f, 0.07f),
            panel = hsl(h, tint * 0.75f, 0.12f),
            muted = hsl(h, tint * 0.3f, 0.68f),
            text = hsl(h, tint * 0.25f, 0.94f),
            dark = true,
        )
    } else {
        Palette(
            accent = hsl(h, maxOf(s, 0.6f), minOf(l, 0.42f)),
            background = hsl(h, tint * 0.5f, 0.965f),
            panel = hsl(h, tint * 0.45f, 0.90f),
            muted = hsl(h, tint * 0.25f, 0.40f),
            text = hsl(h, tint * 0.4f, 0.12f),
            dark = false,
        )
    }
}

/** "#RRGGBB" (the # is optional), or null. */
fun parseColor(hex: String?): Color? {
    val v = hex?.trim()?.removePrefix("#") ?: return null
    if (v.length != 6) return null
    return v.toLongOrNull(16)?.let { Color(0xFF000000 or it) }
}

fun Color.toHex(): String = "#%02X%02X%02X".format(
    (red * 255).roundToInt(), (green * 255).roundToInt(), (blue * 255).roundToInt(),
)

fun toHsl(c: Color): Triple<Float, Float, Float> {
    val max = maxOf(c.red, c.green, c.blue)
    val min = minOf(c.red, c.green, c.blue)
    val l = (max + min) / 2
    if (max == min) return Triple(0f, 0f, l)
    val d = max - min
    val s = if (l > 0.5f) d / (2 - max - min) else d / (max + min)
    val h = when (max) {
        c.red -> (c.green - c.blue) / d + (if (c.green < c.blue) 6 else 0)
        c.green -> (c.blue - c.red) / d + 2
        else -> (c.red - c.green) / d + 4
    }
    return Triple(h * 60, s, l)
}

fun hsl(h: Float, s: Float, l: Float): Color = Color.hsl(((h % 360) + 360) % 360, s.coerceIn(0f, 1f), l.coerceIn(0f, 1f))
