package dev.madhyam.ui

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay

/** In-app speech recognition; [listening] is true between start and the final result. */
@Stable
private class Speech(val recognizer: SpeechRecognizer?) {
    var listening by mutableStateOf(false)
    val available get() = recognizer != null

    fun start() {
        val r = recognizer ?: return
        listening = true
        r.startListening(
            Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true),
        )
    }

    fun stop() {
        if (listening) recognizer?.cancel()
        listening = false
    }
}

@Composable
private fun rememberSpeech(onText: (String, Boolean) -> Unit): Speech {
    val context = LocalContext.current
    val latest by rememberUpdatedState(onText)
    val speech = remember {
        Speech(if (SpeechRecognizer.isRecognitionAvailable(context)) SpeechRecognizer.createSpeechRecognizer(context) else null)
    }
    DisposableEffect(speech) {
        speech.listen { text, final -> latest(text, final) }
        onDispose {
            speech.stop()
            speech.recognizer?.destroy()
        }
    }
    return speech
}

private fun Speech.listen(onText: (String, Boolean) -> Unit) {
    recognizer?.setRecognitionListener(object : RecognitionListener {
        private fun first(b: Bundle?) = b?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()?.takeIf { it.isNotBlank() }
        override fun onResults(b: Bundle?) {
            listening = false
            first(b)?.let { onText(it, true) }
        }
        override fun onPartialResults(b: Bundle?) {
            first(b)?.let { onText(it, false) }
        }
        override fun onError(error: Int) {
            listening = false
        }
        override fun onReadyForSpeech(p: Bundle?) = Unit
        override fun onBeginningOfSpeech() = Unit
        override fun onRmsChanged(v: Float) = Unit
        override fun onBufferReceived(b: ByteArray?) = Unit
        override fun onEndOfSpeech() = Unit
        override fun onEvent(t: Int, p: Bundle?) = Unit
    })
}

/**
 * Search as one full page over the app: the microphone first (it listens as the page opens), the
 * box beside it, earlier searches under them, then results. Back returns to where it was opened.
 */
@Composable
fun SearchPage(vm: MainViewModel) {
    Dialog(
        onDismissRequest = { vm.closeSearch() },
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        val context = LocalContext.current
        val focusManager = LocalFocusManager.current
        val keyboard = LocalSoftwareKeyboardController.current
        val orb = remember { FocusRequester() }
        val speech = rememberSpeech { text, final ->
            vm.search(text)
            if (final) vm.addSearchHistory(text)
        }
        // Without the microphone permission the TV's own voice screen does the listening.
        val systemVoice = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
            r.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.takeIf { it.isNotBlank() }?.let {
                vm.search(it)
                vm.addSearchHistory(it)
            }
        }
        fun openSystemVoice() {
            try {
                systemVoice.launch(Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM))
            } catch (_: android.content.ActivityNotFoundException) {
                android.widget.Toast.makeText(context, "Voice search isn't available on this TV", android.widget.Toast.LENGTH_LONG).show()
            }
        }
        val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted && speech.available) speech.start() else openSystemVoice()
        }
        val hasMic = { context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED }
        LaunchedEffect(Unit) {
            delay(80)
            runCatching { orb.requestFocus() }
            if (hasMic() && speech.available) speech.start()
        }
        var filters by remember { mutableStateOf(false) }
        val where = if (vm.searchScoped) "Search in ${vm.group ?: vm.section.label}" else "Search"

        Column(Modifier.fillMaxSize().background(Background).padding(horizontal = 40.dp, vertical = 24.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Surface(
                    onClick = {
                        when {
                            speech.listening -> speech.stop()
                            !speech.available -> openSystemVoice()
                            hasMic() -> speech.start()
                            else -> permission.launch(Manifest.permission.RECORD_AUDIO)
                        }
                    },
                    modifier = Modifier.size(56.dp).focusRequester(orb),
                    shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(28.dp)),
                    colors = ClickableSurfaceDefaults.colors(
                        containerColor = if (speech.listening) Accent else Panel,
                        contentColor = if (speech.listening) Color.White else TextColor,
                        focusedContainerColor = if (speech.listening) Accent else TextColor,
                        focusedContentColor = if (speech.listening) Color.White else Background,
                    ),
                    scale = ClickableSurfaceDefaults.scale(focusedScale = 1.08f),
                ) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Icon(Icons.Filled.Mic, contentDescription = "Voice search", modifier = Modifier.size(26.dp))
                    }
                }
                val field = TextColor.copy(alpha = 0.82f)
                OutlinedTextField(
                    value = vm.query,
                    onValueChange = vm::search,
                    singleLine = true,
                    textStyle = TextStyle(fontSize = 16.sp, color = Background),
                    placeholder = {
                        androidx.compose.material3.Text(if (speech.listening) "Speak to search" else where, fontSize = 16.sp, color = Background.copy(alpha = 0.6f), maxLines = 1)
                    },
                    shape = RoundedCornerShape(6.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedContainerColor = TextColor,
                        unfocusedContainerColor = field,
                        focusedBorderColor = Color.Transparent,
                        unfocusedBorderColor = Color.Transparent,
                        cursorColor = Background,
                    ),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = {
                        vm.addSearchHistory(vm.query)
                        keyboard?.hide()
                        focusManager.moveFocus(FocusDirection.Down)
                    }),
                    modifier = Modifier.padding(start = 32.dp).width(600.dp)
                        .onFocusChanged { if (it.hasFocus) speech.stop() }
                        .tvTextField(atStart = { vm.query.isEmpty() }),
                )
                Spacer(Modifier.weight(1f))
                if (!vm.searchScoped) SearchIcon("", "Search settings", Modifier, Icons.Filled.Settings) { filters = true }
            }
            Spacer(Modifier.height(20.dp))
            when {
                vm.query.isBlank() -> SearchHistory(vm)
                vm.searchResults.isNotEmpty() -> SearchCards(vm, vm.searchResults)
                vm.searchedFor == vm.query -> EmptyNote("No results.")
            }
        }
        if (filters) SearchFilters(vm) { filters = false }
    }
}

/** Earlier searches in two columns, as quiet pills; OK searches again. */
@Composable
private fun SearchHistory(vm: MainViewModel) {
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    val history = vm.settings.searchHistory
    if (history.isEmpty()) return
    Column(Modifier.padding(start = 88.dp).width(600.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Search history", color = Muted, fontSize = 13.sp, modifier = Modifier.weight(1f))
            SearchIcon("", "Clear search history", Modifier, Icons.Filled.Delete) { vm.clearSearchHistory() }
        }
        LazyColumn(contentPadding = PaddingValues(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            items(history.chunked(2)) { pair ->
                Row {
                    for (q in pair) Box(Modifier.weight(1f)) { HistoryPill(q) { vm.search(q) } }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun HistoryPill(text: String, onClick: () -> Unit) {
    val round = RoundedCornerShape(16.dp)
    Surface(
        onClick = onClick,
        shape = ClickableSurfaceDefaults.shape(round),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = Panel,
            contentColor = Muted,
            focusedContainerColor = TextColor,
            focusedContentColor = Background,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Text(text, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp))
    }
}

/** What the whole-app search looks in (the gear on the search page). */
@Composable
private fun SearchFilters(vm: MainViewModel, onDismiss: () -> Unit) {
    val first = remember { FocusRequester() }
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    val st = vm.settings
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { first.requestFocus() }
    }
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.background(Panel, RoundedCornerShape(16.dp)).padding(24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("Search in", fontSize = 20.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ToggleChip("Live TV", st.searchLive, Modifier.focusRequester(first)) { vm.changeSearchFilter { searchLive = !searchLive } }
                ToggleChip("Movies", st.searchMovies) { vm.changeSearchFilter { searchMovies = !searchMovies } }
                ToggleChip("Series", st.searchSeries) { vm.changeSearchFilter { searchSeries = !searchSeries } }
                if (vm.sources.size > 1) {
                    ToggleChip("All playlists", st.searchAllSources) { vm.changeSearchFilter { searchAllSources = !searchAllSources } }
                }
            }
        }
    }
}
