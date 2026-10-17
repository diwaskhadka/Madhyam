package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Button
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay

/**
 * OpenSubtitles search for the playing film or episode: results by popularity; OK downloads
 * the file and adds it to the stream's subtitles. The query can be edited and searched again.
 */
@Composable
fun SubtitleSearchDialog(vm: MainViewModel) {
    var query by remember { mutableStateOf(vm.subtitleQuery) }
    // Ignore the key-up of the OK that opened the dialog (it would download the first result).
    var armed by remember { mutableStateOf(false) }
    val first = remember { FocusRequester() }
    val results = vm.subtitleResults.orEmpty()
    LaunchedEffect(results.isNotEmpty()) {
        if (results.isNotEmpty()) {
            delay(80)
            runCatching { first.requestFocus() }
        }
    }
    Dialog(onDismissRequest = { vm.closeSubtitleSearch() }) {
        Column(
            Modifier.width(640.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)
                .onPreviewKeyEvent { if (it.type == KeyEventType.KeyDown) armed = true; false },
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text("Find subtitles", fontSize = 22.sp)
            Text("From opensubtitles.com · languages: ${vm.settings.osLanguages.ifBlank { "any" }}", color = Muted, fontSize = 12.sp)
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    label = { androidx.compose.material3.Text("Title") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { vm.searchSubtitles(query) }),
                    modifier = Modifier.weight(1f).tvTextField { query.isEmpty() },
                )
                Spacer(Modifier.width(10.dp))
                Button(onClick = { if (armed) vm.searchSubtitles(query) }) { Text("Search") }
            }
            when {
                vm.subtitleSearching -> Text("Searching…", color = Accent, fontSize = 14.sp)
                vm.subtitleError != null -> Text(vm.subtitleError!!, color = ErrorRed, fontSize = 14.sp)
                results.isEmpty() -> Text("No subtitles found. Try a shorter title.", color = Muted, fontSize = 14.sp)
            }
            LazyColumn(Modifier.heightIn(max = 380.dp)) {
                items(results, key = { it.fileId }) { hit ->
                    ListItem(
                        selected = false,
                        onClick = { if (armed) vm.useSubtitle(hit) },
                        modifier = if (hit === results.first()) Modifier.focusRequester(first) else Modifier,
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
}
