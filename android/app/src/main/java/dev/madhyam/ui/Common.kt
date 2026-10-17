package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import coil.compose.SubcomposeAsyncImage
import dev.madhyam.Item
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

val ErrorRed = Color(0xFFFF7A7A)

/** Channel logo or poster; the first letters of the name when there is none (or it fails to load). */
@Composable
fun Logo(item: Item, width: Dp = 64.dp, height: Dp = 40.dp, stripPrefix: Boolean = false) {
    val shape = Modifier.size(width, height).clip(RoundedCornerShape(6.dp))
    val initials: @Composable () -> Unit = {
        Box(shape.background(Accent.copy(alpha = 0.22f)), contentAlignment = Alignment.Center) {
            Text(item.displayName(stripPrefix).take(2).uppercase(), fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        }
    }
    if (item.logo.isBlank()) {
        initials()
    } else {
        SubcomposeAsyncImage(
            model = item.logo,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = shape,
            error = { initials() },
        )
    }
}

/** Thin progress line; fills the available width unless [width] is given. */
@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier, width: Dp? = null) {
    val sized = if (width != null) modifier.width(width) else modifier.fillMaxWidth()
    Box(sized.padding(vertical = 3.dp).height(3.dp).clip(RoundedCornerShape(2.dp)).background(Muted.copy(alpha = 0.3f))) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).background(Accent))
    }
}

private val clock = SimpleDateFormat("HH:mm", Locale.getDefault())
private val day = SimpleDateFormat("EEE d MMM yyyy", Locale.getDefault())

fun hhmm(unixSeconds: Long): String = clock.format(Date(unixSeconds * 1000))
fun date(unixSeconds: Long): String = day.format(Date(unixSeconds * 1000))

fun fmt(ms: Long): String {
    val s = ms / 1000
    val h = s / 3600
    val m = s % 3600 / 60
    val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}
