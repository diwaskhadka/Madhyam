package dev.madhyam.ui

import androidx.compose.foundation.focusGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Wrap-around in a column: Up past its top goes to its last item, Down past its bottom to its
 * first. Each item takes [at] with its index; the column takes [Modifier.wrapColumn].
 */
class Wrap internal constructor(private val scope: CoroutineScope) {
    private var target by mutableIntStateOf(-1)
    private val requester = FocusRequester()

    /** True while the focus is being moved: a column's own "enter" must not redirect it. */
    val active: Boolean get() = target >= 0

    fun at(i: Int): Modifier = if (i == target) Modifier.focusRequester(requester) else Modifier

    /** Focus the item [prepare] returns (after scrolling it into its list); a negative index does nothing. */
    fun focus(prepare: suspend () -> Int) {
        scope.launch {
            val i = prepare()
            if (i < 0) return@launch
            target = i
            withFrameNanos { }
            withFrameNanos { }
            wrapMoving = true
            runCatching { requester.requestFocus() }
            wrapMoving = false
            target = -1
        }
    }
}

/**
 * True while any wrap-around moves the focus: a group's "enter" (the page you're on, the remembered
 * row) must not redirect it.
 */
var wrapMoving = false
    private set

@Composable
fun rememberWrap(): Wrap {
    val scope = rememberCoroutineScope()
    return remember { Wrap(scope) }
}

/**
 * Calls [onTop] / [onBottom] when Up / Down finds nothing further in the column, instead of
 * stopping or letting the focus leave it.
 */
@OptIn(ExperimentalComposeUiApi::class)
fun Modifier.wrapColumn(onTop: () -> Unit, onBottom: () -> Unit): Modifier = composed {
    val focusManager = LocalFocusManager.current
    onPreviewKeyEvent { event ->
        if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
        when (event.nativeKeyEvent.keyCode) {
            android.view.KeyEvent.KEYCODE_DPAD_UP -> {
                if (!focusManager.moveFocus(FocusDirection.Up)) onTop()
                true
            }
            android.view.KeyEvent.KEYCODE_DPAD_DOWN -> {
                if (!focusManager.moveFocus(FocusDirection.Down)) onBottom()
                true
            }
            else -> false
        }
    }.then(stayInside())
}

/** Up and Down never take the focus out of the group. */
@OptIn(ExperimentalComposeUiApi::class)
private fun stayInside(): Modifier = Modifier.focusProperties {
    exit = { direction ->
        if (direction == FocusDirection.Up || direction == FocusDirection.Down) FocusRequester.Cancel else FocusRequester.Default
    }
}.focusGroup()

/**
 * Wrap-around for a column whose rows can't be numbered (Settings pages): the focus walks to the
 * other end one row at a time.
 */
class WrapByMoving internal constructor(private val scope: CoroutineScope, private val focusManager: FocusManager) {
    internal fun walk(direction: FocusDirection) {
        scope.launch {
            var steps = 0
            while (steps < 2000 && focusManager.moveFocus(direction)) {
                if (++steps % 25 == 0) withFrameNanos { }
            }
        }
    }
}

@Composable
fun rememberWrapByMoving(): WrapByMoving {
    val scope = rememberCoroutineScope()
    val focusManager = LocalFocusManager.current
    return remember { WrapByMoving(scope, focusManager) }
}

fun Modifier.wrapByMoving(w: WrapByMoving): Modifier =
    wrapColumn(onTop = { w.walk(FocusDirection.Down) }, onBottom = { w.walk(FocusDirection.Up) })
