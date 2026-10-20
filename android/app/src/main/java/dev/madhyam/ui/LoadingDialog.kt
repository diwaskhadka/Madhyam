package dev.madhyam.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Text
import dev.madhyam.LoadLine
import dev.madhyam.LoadState
import kotlinx.coroutines.delay
import kotlin.math.exp

/**
 * "Adding playlist": the steps of a playlist load with a tick as each finishes, what is
 * happening now (with the size downloaded), how many live channels, movies and series came
 * in, and an overall bar, so nobody waits at a blank screen. Hide lets the load go on in the
 * background (the status line still shows it).
 */
@Composable
fun LoadingDialog(state: LoadState, onClose: () -> Unit) {
    val button = remember { FocusRequester() }
    // The OK that started the load lifts its key in here: only accept clicks after a key went down.
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { button.requestFocus() }
    }
    // Without a known size, the running step's share of the bar creeps forward so it never looks stuck.
    var bar by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(state) {
        while (true) {
            val n = state.lines.size
            val target = when {
                state.finished && state.error == null -> 1f
                state.current < 0 -> 0f
                else -> {
                    val seconds = (System.currentTimeMillis() - state.stepStarted) / 1000f
                    (state.current + 0.9f * (1 - exp(-seconds / 6f))) / n
                }
            }
            bar = maxOf(bar, target)
            delay(120)
        }
    }
    Dialog(onDismissRequest = onClose) {
        Column(
            Modifier.width(520.dp).background(Panel, RoundedCornerShape(16.dp)).padding(24.dp)
                .onPreviewKeyEvent { if (it.type == KeyEventType.KeyDown) armed = true; false },
        ) {
            Text(state.title, fontSize = 22.sp, maxLines = 1)
            val sub = when {
                state.error != null -> null
                state.finished -> "Here's what the playlist has."
                else -> "This can take a minute for a large playlist."
            }
            if (sub != null) Text(sub, color = Muted, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
            Spacer(Modifier.padding(top = 14.dp))
            Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
                state.lines.forEach { Line(it) }
            }
            ProgressBar(bar, Modifier.padding(top = 12.dp))
            state.error?.let { Text(it, color = ErrorRed, fontSize = 14.sp, modifier = Modifier.padding(top = 10.dp)) }
            Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.End) {
                androidx.tv.material3.Button(onClick = { if (armed || state.finished) onClose() }, modifier = Modifier.focusRequester(button)) {
                    Text(
                        when {
                            state.error != null -> "Close"
                            state.finished -> "Done"
                            else -> "Hide"
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun Line(line: LoadLine) {
    val pulse = rememberInfiniteTransition(label = "pulse").animateFloat(
        1f, 0.35f, infiniteRepeatable(tween(650), RepeatMode.Reverse), label = "pulse",
    )
    val none = line.detail == "None"
    Row(verticalAlignment = Alignment.CenterVertically) {
        val (mark, color) = when (line.state) {
            LoadLine.State.Waiting -> "○" to Muted
            LoadLine.State.Running -> "●" to Accent
            LoadLine.State.Done -> "✓" to Accent
            LoadLine.State.Failed -> "✕" to ErrorRed
        }
        Text(
            mark, color = color, fontSize = 15.sp,
            modifier = Modifier.width(28.dp).alpha(if (line.state == LoadLine.State.Running) pulse.value else 1f),
        )
        Text(
            line.label,
            color = if (line.state == LoadLine.State.Waiting || none) Muted else TextColor,
            fontSize = 15.sp,
            fontWeight = if (line.state == LoadLine.State.Running) FontWeight.SemiBold else FontWeight.Normal,
            modifier = Modifier.weight(1f),
        )
        Text(line.detail, color = if (none) Muted else TextColor, fontSize = 13.sp)
    }
}
