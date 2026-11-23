package dev.madhyam.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Button
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay

/**
 * The online page of the Subtitles panel: OpenSubtitles results for the playing film or episode,
 * by popularity; OK downloads the file and adds it to the stream's subtitles. The title can be
 * edited and searched again: Up from the first result reaches it, and an empty search lands on it. [armed]: a key went down in the panel (ignore the opening OK's key-up).
 */
@Composable
fun SubtitleSearchPage(vm: MainViewModel, armed: () -> Boolean) {
    var query by remember { mutableStateOf(vm.subtitleQuery) }
    val first = remember { FocusRequester() }
    val field = remember { FocusRequester() }
    val results = vm.subtitleResults.orEmpty()
    val wrap = rememberWrap()
    val list = rememberLazyListState()
    // Results: the focus goes to the best one. None (or an error): to the title, to fix it.
    LaunchedEffect(results.isNotEmpty(), vm.subtitleSearching) {
        if (vm.subtitleSearching) return@LaunchedEffect
        delay(80)
        runCatching { (if (results.isNotEmpty()) first else field).requestFocus() }
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Find subtitles online", fontSize = 22.sp)
        Text(
            "From opensubtitles.com · languages: ${vm.settings.osLanguages.ifBlank { "any" }} · " +
                "Up from the first result edits the title · Back returns to the list",
            color = Muted, fontSize = 12.sp,
        )
        // The title and Search are the top of the column: Up from them wraps to the last result.
        Row(
            Modifier.onPreviewKeyEvent { ev ->
                if (ev.type == KeyEventType.KeyDown && ev.nativeKeyEvent.keyCode == android.view.KeyEvent.KEYCODE_DPAD_UP && results.isNotEmpty()) {
                    wrap.focus { results.lastIndex.also { list.scrollToItem(it) } }
                    true
                } else false
            },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                label = { androidx.compose.material3.Text("Title") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { vm.searchSubtitles(query) }),
                modifier = Modifier.weight(1f).focusRequester(field).tvTextField(atStart = { query.isEmpty() }),
            )
            Spacer(Modifier.width(10.dp))
            Button(onClick = { if (armed()) vm.searchSubtitles(query) }) { Text("Search") }
        }
        when {
            vm.subtitleSearching -> Text("Searching…", color = Accent, fontSize = 14.sp)
            vm.subtitleError != null -> Text(vm.subtitleError!!, color = ErrorRed, fontSize = 14.sp)
            results.isEmpty() -> Text("No subtitles found. Try a shorter title.", color = Muted, fontSize = 14.sp)
        }
        LazyColumn(
            Modifier.heightIn(max = 380.dp).wrapColumn(
                onTop = { runCatching { field.requestFocus() } },
                onBottom = { wrap.focus { if (results.isEmpty()) -1 else 0.also { list.scrollToItem(0) } } },
            ),
            state = list,
        ) {
            itemsIndexed(results, key = { _, it -> it.fileId }) { i, hit ->
                ListItem(
                    selected = false,
                    onClick = { if (armed()) vm.useSubtitle(hit) },
                    modifier = (if (i == 0) Modifier.focusRequester(first) else Modifier).then(wrap.at(i)),
                    headlineContent = {
                        Text(
                            "${hit.language.uppercase()} · ${hit.release.ifBlank { hit.fileName }}",
                            maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp,
                        )
                    },
                    supportingContent = {
                        Text(
                            buildString {
                                append("%,d downloads".format(hit.downloads))
                                if (hit.hearingImpaired) append(" · hearing impaired")
                                if (hit.title.isNotBlank()) append(" · ${hit.title}")
                            },
                            color = Muted, fontSize = 12.sp, maxLines = 1,
                        )
                    },
                )
            }
        }
    }
}
