package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Button
import androidx.tv.material3.Text
import kotlinx.coroutines.delay

/** The steps of the delay buttons, in ms (0 = back to no delay). */
private val DELAY_STEPS = listOf(-1000, -250, 0, 250, 1000)

/** Delays go up to ±10 s. */
private const val DELAY_LIMIT_MS = 10_000

/**
 * Audio or subtitle delay as buttons, like the player's −10 s / +10 s: each OK changes the delay at
 * once and the focus stays, so pressing again keeps going. [value] is read at each press.
 */
@Composable
fun DelayStepper(value: () -> Int, armed: () -> Boolean, onChange: (Int) -> Unit) {
    val plus = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { plus.requestFocus() }
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(top = 4.dp)) {
        Text(delayLabel(value()), fontSize = 34.sp, modifier = Modifier.padding(bottom = 14.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DELAY_STEPS.forEach { step ->
                Button(
                    onClick = {
                        if (armed()) onChange(if (step == 0) 0 else (value() + step).coerceIn(-DELAY_LIMIT_MS, DELAY_LIMIT_MS))
                    },
                    modifier = if (step == 250) Modifier.focusRequester(plus) else Modifier,
                ) {
                    Text(if (step == 0) "Reset" else delayLabel(step), fontSize = 14.sp)
                }
            }
        }
    }
}
