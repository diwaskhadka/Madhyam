package dev.madhyam.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Text
import dev.madhyam.Group
import dev.madhyam.MainViewModel
import dev.madhyam.MainViewModel.PinCheck
import dev.madhyam.Parental
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import android.view.KeyEvent as AndroidKeyEvent

/**
 * Four digit boxes. Number keys type a digit; with only a D-pad, ▲▼ choose the digit of the
 * highlighted box, ◀ ▶ move between boxes and OK goes on. The fourth digit submits. [onSubmit]
 * says whether the PIN was right; a wrong one shakes the boxes and clears them.
 */
@Composable
fun PinDialog(title: String, note: String? = null, onSubmit: (String) -> PinCheck, onDismiss: () -> Unit) {
    val digits = remember { IntArray(Parental.PIN_LENGTH) { -1 } }
    var cursor by remember { mutableIntStateOf(0) }
    var version by remember { mutableIntStateOf(0) } // redraw after changing [digits]
    var error by remember { mutableStateOf<String?>(null) }
    var waitSeconds by remember { mutableIntStateOf(0) }
    // The OK that opened the dialog may come back up in here: only react to presses made in it.
    var armed by remember { mutableStateOf(false) }
    val shake = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { focus.requestFocus() }
    }
    LaunchedEffect(waitSeconds > 0) {
        while (waitSeconds > 0) {
            delay(1000)
            waitSeconds--
        }
        if (error?.startsWith("Too many") == true) error = null
    }

    fun clear() {
        digits.fill(-1)
        cursor = 0
        version++
    }

    fun submit() {
        val pin = digits.joinToString("") { it.toString() }
        when (val r = onSubmit(pin)) {
            PinCheck.Ok -> {}
            is PinCheck.Wrong -> {
                error = if (r.triesLeft == 1) "Wrong PIN. 1 try left." else "Wrong PIN. ${r.triesLeft} tries left."
                clear()
                scope.launch {
                    for (x in listOf(18f, -18f, 12f, -12f, 6f, 0f)) shake.animateTo(x, tween(45))
                }
            }
            is PinCheck.Wait -> {
                waitSeconds = r.seconds
                error = "Too many wrong PINs. Try again in a moment."
                clear()
            }
        }
    }

    fun type(d: Int) {
        if (waitSeconds > 0) return
        digits[cursor] = d
        version++
        if (cursor == Parental.PIN_LENGTH - 1) submit() else cursor++
    }

    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier.width(520.dp).background(Panel, RoundedCornerShape(16.dp)).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(Icons.Filled.Lock, contentDescription = null, tint = Accent, modifier = Modifier.size(32.dp))
            Text(title, fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
            if (note != null) Text(note, color = Muted, fontSize = 13.sp)
            @Suppress("UNUSED_EXPRESSION") version
            Row(
                Modifier
                    .offset(x = shake.value.dp)
                    .focusRequester(focus)
                    .onPreviewKeyEvent { ev ->
                        if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                        armed = true
                        val code = ev.nativeKeyEvent.keyCode
                        when (code) {
                            in AndroidKeyEvent.KEYCODE_0..AndroidKeyEvent.KEYCODE_9 -> type(code - AndroidKeyEvent.KEYCODE_0)
                            in AndroidKeyEvent.KEYCODE_NUMPAD_0..AndroidKeyEvent.KEYCODE_NUMPAD_9 -> type(code - AndroidKeyEvent.KEYCODE_NUMPAD_0)
                            AndroidKeyEvent.KEYCODE_DPAD_UP, AndroidKeyEvent.KEYCODE_DPAD_DOWN -> {
                                if (waitSeconds == 0) {
                                    val step = if (code == AndroidKeyEvent.KEYCODE_DPAD_UP) 1 else 9
                                    digits[cursor] = if (digits[cursor] < 0) 0 else (digits[cursor] + step) % 10
                                    version++
                                }
                            }
                            AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (digits[cursor] >= 0 && cursor < Parental.PIN_LENGTH - 1) cursor++
                            AndroidKeyEvent.KEYCODE_DPAD_LEFT, AndroidKeyEvent.KEYCODE_DEL -> if (cursor > 0) {
                                if (code == AndroidKeyEvent.KEYCODE_DEL) digits[cursor] = -1
                                cursor--
                                version++
                            }
                            AndroidKeyEvent.KEYCODE_DPAD_CENTER, AndroidKeyEvent.KEYCODE_ENTER, AndroidKeyEvent.KEYCODE_NUMPAD_ENTER -> {
                                if (waitSeconds == 0) {
                                    if (digits[cursor] < 0) digits[cursor] = 0
                                    if (digits.all { it >= 0 }) submit() else cursor++
                                    version++
                                }
                            }
                            AndroidKeyEvent.KEYCODE_BACK -> return@onPreviewKeyEvent false
                            else -> return@onPreviewKeyEvent false
                        }
                        true
                    }
                    .focusable(),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                for (i in 0 until Parental.PIN_LENGTH) {
                    val d = digits[i]
                    val here = i == cursor && waitSeconds == 0
                    Box(
                        Modifier.size(64.dp, 76.dp)
                            .background(Background, RoundedCornerShape(10.dp))
                            .border(if (here) 3.dp else 1.dp, if (here) Accent else Muted.copy(alpha = 0.5f), RoundedCornerShape(10.dp)),
                        contentAlignment = Alignment.Center,
                    ) {
                        // The digit being chosen shows; the others are dots.
                        Text(
                            when {
                                d < 0 -> ""
                                here -> d.toString()
                                else -> "•"
                            },
                            fontSize = 30.sp,
                        )
                    }
                }
            }
            Text(
                when {
                    waitSeconds > 0 -> "Too many wrong PINs. Try again in $waitSeconds s."
                    error != null -> error!!
                    else -> "Number keys, or ▲ ▼ to choose a digit and OK for the next"
                },
                color = if (error != null || waitSeconds > 0) ErrorRed else Muted,
                fontSize = 13.sp,
            )
            OutlinedButton(onClick = { if (armed) onDismiss() }) { Text("Cancel") }
        }
    }
}

/** The PIN the view model asks for (opening a locked category, a locked channel, Settings…). */
@Composable
fun PinHost(vm: MainViewModel) {
    val req = vm.pinRequest ?: return
    val wait = remember(req) { vm.pinWaitSeconds() }
    androidx.compose.runtime.key(req) {
        PinDialog(
            req.title,
            note = if (wait > 0) null else "Parental controls",
            onSubmit = { vm.checkPin(it) },
            onDismiss = { vm.cancelPin() },
        )
    }
}

private enum class PinStep { None, New, Again, Reset }

/** Settings › Parental controls. */
@Composable
internal fun ParentalPage(vm: MainViewModel) {
    val st = vm.settings
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    var step by remember { mutableStateOf(PinStep.None) }
    var first by remember { mutableStateOf("") }
    var turnOnAfter by remember { mutableStateOf(false) }
    var categories by remember { mutableStateOf(false) }
    val hasPin = st.parentalPinHash.isNotEmpty()

    if (categories) {
        androidx.activity.compose.BackHandler { categories = false }
        LockedCategoriesPage(vm)
        return
    }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item { SectionTitle("Parental controls", "A 4-digit PIN locks categories, adult content, films above an age and Settings") }
        item {
            SwitchRow(
                "Parental controls",
                if (hasPin) "On: the locks below need the PIN" else "Turning it on asks you to choose a PIN",
                st.parentalOn,
            ) { on ->
                when {
                    on && !hasPin -> {
                        turnOnAfter = true
                        step = PinStep.New
                    }
                    on -> vm.requirePin("Enter the PIN to turn parental controls on") { vm.setParentalOn(true) }
                    else -> vm.requirePin("Enter the PIN to turn parental controls off") { vm.setParentalOn(false) }
                }
            }
        }
        item {
            ActionRow(
                if (hasPin) "Change PIN" else "Set PIN",
                if (hasPin) "Asks for the current PIN first" else "Choose a 4-digit PIN",
            ) {
                turnOnAfter = false
                if (hasPin) vm.requirePin("Enter the current PIN") { step = PinStep.New } else step = PinStep.New
            }
        }
        item {
            SwitchRow(
                "Auto-lock adult content",
                "Categories and channels named xxx, adult, 18+, porn, erotic, playboy… are locked. Unlock one under Locked categories",
                st.parentalAutoAdult,
            ) { on -> vm.changeSettings { parentalAutoAdult = on } }
        }
        item {
            SwitchRow(
                "Lock Settings",
                "Settings, adding, editing or removing a playlist and Parental controls need the PIN",
                st.parentalLockSettings,
            ) { on -> vm.changeSettings { parentalLockSettings = on } }
        }
        item {
            ChoiceRow(
                "Allow up to",
                "Films and series rated above need the PIN. Items without an age rating are allowed",
                st.parentalMaxAge,
                Parental.AGE_LIMITS.map { it to if (it == 0) "Any" else "$it" },
            ) { vm.changeSettings { parentalMaxAge = it } }
        }
        item { ActionRow("Locked categories", "Lock or unlock categories of every playlist") { categories = true } }
        item { ActionRow("Reset parental controls", "Forgot the PIN? Removes the PIN and every lock") { step = PinStep.Reset } }
        if (!st.parentalOn) {
            item { Text("Parental controls are off: nothing is locked.", color = Muted, fontSize = 12.sp, modifier = Modifier.padding(12.dp)) }
        }
    }

    when (step) {
        PinStep.New -> PinDialog("Choose a 4-digit PIN", onSubmit = {
            first = it
            step = PinStep.Again
            PinCheck.Ok
        }, onDismiss = { step = PinStep.None })
        PinStep.Again -> androidx.compose.runtime.key("again") {
            PinDialog("Enter the PIN again", onSubmit = {
                if (it != first) {
                    vm.toast("The PINs don't match. Choose the PIN again.")
                    step = PinStep.New
                } else {
                    vm.setPin(it)
                    if (turnOnAfter) vm.setParentalOn(true)
                    vm.toast(if (turnOnAfter) "Parental controls are on." else "PIN saved.")
                    step = PinStep.None
                }
                PinCheck.Ok
            }, onDismiss = { step = PinStep.None })
        }
        PinStep.Reset -> TextEditDialog(
            "Type RESET to remove the PIN and every lock", "", KeyboardType.Text, secret = false,
            onDone = { if (it.trim().equals("RESET", ignoreCase = true)) vm.resetParental() else vm.toast("Not reset: type RESET to confirm.") },
        ) { step = PinStep.None }
        PinStep.None -> {}
    }
}

/** Every category of every playlist, with a lock that OK turns on or off. */
@Composable
private fun LockedCategoriesPage(vm: MainViewModel) {
    val sources = vm.sources
    if (sources.isEmpty()) {
        Text("Add a playlist first.", color = Muted)
        return
    }
    var sourceId by remember { mutableStateOf(vm.source?.id ?: sources.first().id) }
    val src = sources.firstOrNull { it.id == sourceId } ?: sources.first()
    var kind by remember { mutableStateOf("live") }
    var groups by remember(src.id, kind) { mutableStateOf<List<Group>?>(null) }
    LaunchedEffect(src.id, kind) { groups = vm.groupsOf(src, kind) }
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item { SectionTitle("Locked categories", "OK locks or unlocks a category. Back returns to Parental controls") }
        if (sources.size > 1) {
            item { ChoiceRow("Playlist", "OK to switch", src.id, sources.map { it.id to it.name }) { sourceId = it } }
        }
        item { ChoiceRow("Show categories of", "OK to switch", kind, listOf("live" to "Live TV", "movie" to "Movies", "series" to "Series")) { kind = it } }
        val list = groups
        when {
            list == null -> item { Text("Loading categories…", color = Muted, modifier = Modifier.padding(12.dp)) }
            list.isEmpty() -> item { Text("No categories.", color = Muted, modifier = Modifier.padding(12.dp)) }
            else -> items(list, key = { it.name }) { g ->
                // The locks are a plain map: reading settingsVersion here redraws the row when one changes.
                val reason = vm.settingsVersion.let { vm.categoryLockSetting(src.id, kind, g.name) }
                val adult = Parental.looksAdult(g.name)
                ListItem(
                    selected = false,
                    onClick = { vm.setCategoryLock(src.id, kind, g.name, reason == null) },
                    headlineContent = { Text(g.name) },
                    supportingContent = {
                        Text(
                            when {
                                reason == "Adult" -> "Adult · locked automatically"
                                adult && reason == null -> "Adult · unlocked by you"
                                reason != null -> "Locked · ${g.count} items"
                                else -> "${g.count} items"
                            },
                            color = RowMuted, fontSize = 12.sp,
                        )
                    },
                    trailingContent = {
                        Icon(
                            if (reason != null) Icons.Filled.Lock else Icons.Filled.LockOpen,
                            contentDescription = if (reason != null) "Locked" else "Not locked",
                            tint = if (reason != null) RowAccent else RowMuted.copy(alpha = 0.5f),
                        )
                    },
                )
            }
        }
    }
}

/** 🔒 for a category that needs the PIN now, an open lock while the session is unlocked. */
@Composable
fun CategoryLockIcon(locked: Boolean, modifier: Modifier = Modifier) {
    Icon(
        if (locked) Icons.Filled.Lock else Icons.Filled.LockOpen,
        contentDescription = if (locked) "Locked" else "Unlocked for now",
        tint = if (locked) RowAccent else RowMuted,
        modifier = modifier.size(14.dp),
    )
}

/** What a locked category shows instead of its items. */
@Composable
fun LockedNote() {
    Box(Modifier.padding(top = 40.dp).padding(horizontal = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Lock, null, tint = Accent, modifier = Modifier.size(22.dp))
            Text("  This category is locked. Press OK on it to enter the PIN.", color = Muted, fontSize = 14.sp)
        }
    }
}
