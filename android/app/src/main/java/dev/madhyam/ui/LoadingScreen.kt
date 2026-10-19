package dev.madhyam.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import dev.madhyam.Brand
import dev.madhyam.R

/**
 * Start-up: the logo, the name and a progress bar while the catalog loads. The engine loads the
 * snapshot in one call, so the bar follows how long the last start took ([expectedMs]) and
 * slows down near the end; without a previous start it sweeps instead.
 */
@Composable
fun LoadingScreen(expectedMs: Long) {
    val pulse = rememberInfiniteTransition(label = "pulse")
    val scale by pulse.animateFloat(
        initialValue = 0.96f,
        targetValue = 1.04f,
        animationSpec = infiniteRepeatable(tween(1100, easing = FastOutSlowInEasing), RepeatMode.Reverse),
        label = "scale",
    )
    Column(
        Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Image(
            painterResource(R.drawable.brand_logo),
            contentDescription = Brand.NAME,
            modifier = Modifier.size(150.dp).scale(scale),
        )
        Spacer(Modifier.height(28.dp))
        Text(Brand.NAME, fontSize = 40.sp, color = TextColor)
        Text(Brand.NATIVE_NAME, fontSize = 20.sp, color = Muted)
        Spacer(Modifier.height(36.dp))
        Box(
            Modifier.width(420.dp).height(6.dp).clip(RoundedCornerShape(3.dp)).background(Panel),
        ) {
            if (expectedMs > 0) EstimatedBar(expectedMs) else SweepingBar()
        }
        Spacer(Modifier.height(14.dp))
        Text("Loading your playlists…", fontSize = 14.sp, color = Muted)
    }
}

@Composable
private fun EstimatedBar(expectedMs: Long) {
    val progress = remember { Animatable(0f) }
    LaunchedEffect(expectedMs) {
        // Most of the way in the expected time, then creep: never full before it is done.
        progress.animateTo(0.9f, tween(expectedMs.coerceIn(500, 60_000).toInt(), easing = LinearEasing))
        progress.animateTo(0.98f, tween(20_000, easing = FastOutSlowInEasing))
    }
    Box(Modifier.fillMaxHeight().fillMaxWidth(progress.value).background(Accent))
}

@Composable
private fun SweepingBar() {
    val sweep = rememberInfiniteTransition(label = "sweep")
    val x by sweep.animateFloat(
        initialValue = -0.3f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(1400, easing = LinearEasing)),
        label = "x",
    )
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxHeight().width(126.dp)
                .align(Alignment.CenterStart)
                .offset(x = (420 * x).dp) // the bar is 420 dp wide
                .background(Accent),
        )
    }
}
