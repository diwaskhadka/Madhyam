package dev.madhyam.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme

val Accent = Color(0xFF4F8CFF)
val Background = Color(0xFF0E1020)
val Panel = Color(0xFF171A2E)
val Muted = Color(0xFF9AA0B8)

/**
 * TV Material for focusable widgets, plus the phone Material 3 theme because
 * text fields only exist there. Both are dark: TVs are watched in dim rooms.
 */
@Composable
fun MadhyamTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Accent,
            background = Background,
            surface = Panel,
        ),
    ) {
        androidx.compose.material3.MaterialTheme(
            colorScheme = androidx.compose.material3.darkColorScheme(
                primary = Accent,
                background = Background,
                surface = Panel,
            ),
            content = content,
        )
    }
}
