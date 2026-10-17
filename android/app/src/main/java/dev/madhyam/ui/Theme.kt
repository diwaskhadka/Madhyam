package dev.madhyam.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme
import androidx.tv.material3.lightColorScheme

/** The app's own colours; Settings › Appearance picks the dark or the light set (like Windows). */
data class Palette(
    val accent: Color,
    val background: Color,
    val panel: Color,
    val muted: Color,
    val text: Color,
    val dark: Boolean,
)

private val DarkPalette = Palette(
    accent = Color(0xFF4F8CFF),
    background = Color(0xFF0E1020),
    panel = Color(0xFF171A2E),
    muted = Color(0xFF9AA0B8),
    text = Color(0xFFECEEF6),
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
 * TV Material for focusable widgets, plus the phone Material 3 theme because text fields only
 * exist there. [theme] is "Dark", "Light" or "System" (follow the TV).
 */
@Composable
fun MadhyamTheme(theme: String = "Dark", content: @Composable () -> Unit) {
    val dark = when (theme) {
        "Light" -> false
        "System" -> isSystemInDarkTheme()
        else -> true
    }
    val p = if (dark) DarkPalette else LightPalette
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
