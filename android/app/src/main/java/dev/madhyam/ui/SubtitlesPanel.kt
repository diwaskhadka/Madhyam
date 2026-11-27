package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Timer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import dev.madhyam.TrackChoice
import kotlinx.coroutines.delay

private enum class SubPage { List, Delay, Online }

private class Row(val label: String, val icon: ImageVector?, val checked: Boolean, val divider: Boolean = false, val onPick: () -> Unit)

/**
 * Everything about subtitles in one panel: Off, the stream's tracks (and downloaded ones), Find
 * online (results open in the panel), Delay and Style. Back goes from a page to the list, then
 * closes the panel.
 */
@Composable
fun SubtitlesPanel(vm: MainViewModel, canSearch: Boolean, onStyle: () -> Unit, onDismiss: () -> Unit) {
    val pb = vm.playback
    var page by remember { mutableStateOf(SubPage.List) }
    // Holding or lifting the OK that opened the panel must not pick a row.
    var armed by remember { mutableStateOf(false) }
    // The list row to focus when the list shows again (Delay after the delay page).
    var returnTo by remember { mutableIntStateOf(-1) }
    // A downloaded result closes the search, and the panel with it.
    val searching = vm.subtitleResults != null
    LaunchedEffect(searching) { if (!searching && page == SubPage.Online) onDismiss() }

    fun back() {
        when (page) {
            SubPage.Online -> {
                page = SubPage.List
                vm.closeSubtitleSearch()
            }
            SubPage.Delay -> page = SubPage.List
            SubPage.List -> onDismiss()
        }
    }

    Dialog(onDismissRequest = ::back) {
        Column(
            Modifier.width(if (page == SubPage.Online) 640.dp else 460.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)
                .onPreviewKeyEvent {
                    if (it.type == KeyEventType.KeyDown && it.nativeKeyEvent.repeatCount == 0) armed = true
                    false
                },
        ) {
            @Suppress("UNUSED_EXPRESSION") pb.tracksVersion // re-read the tracks when they change
            when (page) {
                SubPage.List -> {
                    val tracks = pb.subtitleTracks()
                    val ext = pb.activeExternal
                    val off = ext == null && (pb.subtitlesOff || tracks.none { it.selected })
                    val rows = buildList {
                        add(Row("Off", null, off) { pb.turnSubtitlesOff(); onDismiss() })
                        tracks.forEach { t: TrackChoice -> add(Row(t.label, null, !off && ext == null && t.selected) { pb.select(t); onDismiss() }) }
                        // Downloaded with Find online (drawn by the app, so picking one never restarts the video).
                        pb.externalSubtitles.forEach { s -> add(Row(s.label, null, ext == s) { pb.useExternal(s); onDismiss() }) }
                        var divider = true
                        if (canSearch) {
                            add(Row("Find online…", Icons.Filled.Search, false, divider = true) { vm.openSubtitleSearch(); page = SubPage.Online })
                            divider = false
                        }
                        val delayRow = size
                        add(Row("Delay: ${delayLabel(pb.subtitleDelayMs)}", Icons.Filled.Timer, false, divider) { returnTo = delayRow; page = SubPage.Delay })
                        add(Row("Style…", Icons.Filled.Palette, false) { onStyle() })
                    }
                    val start = if (returnTo >= 0) returnTo else rows.indexOfFirst { it.checked }.coerceAtLeast(0)
                    Text("Subtitles", fontSize = 22.sp, modifier = Modifier.padding(bottom = 10.dp))
                    PanelList(rows.size, start, armed = { armed }, isChecked = { rows[it].checked }, divider = { rows[it].divider }, label = { rows[it].label }, icon = { rows[it].icon }) { rows[it].onPick() }
                }
                SubPage.Delay -> {
                    Text("Subtitle delay", fontSize = 22.sp)
                    Text("+ shows subtitles later · Back returns to the list", color = Muted, fontSize = 12.sp, modifier = Modifier.padding(bottom = 10.dp))
                    DelayStepper({ pb.subtitleDelayMs }, { armed }) { pb.changeSubtitleDelay(it) }
                }
                SubPage.Online -> SubtitleSearchPage(vm) { armed }
            }
        }
    }
}

/** A list of [count] rows with the focus on [start]; Up and Down wrap around. */
@Composable
private fun PanelList(
    count: Int,
    start: Int,
    armed: () -> Boolean,
    isChecked: (Int) -> Boolean,
    divider: (Int) -> Boolean,
    label: (Int) -> String,
    icon: (Int) -> ImageVector?,
    onPick: (Int) -> Unit,
) {
    val first = remember { FocusRequester() }
    val wrap = rememberWrap()
    val list = rememberLazyListState(initialFirstVisibleItemIndex = (start - 3).coerceAtLeast(0))
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { first.requestFocus() }
    }
    LazyColumn(
        Modifier.heightIn(max = 420.dp).wrapColumn(
            onTop = { wrap.focus { (count - 1).also { list.scrollToItem(it) } } },
            onBottom = { wrap.focus { list.scrollToItem(0); 0 } },
        ),
        state = list,
    ) {
        itemsIndexed(List(count) { it }) { _, i ->
            if (divider(i)) Box(Modifier.padding(vertical = 6.dp).fillMaxWidth().height(1.dp).background(Muted.copy(alpha = 0.3f)))
            ListItem(
                selected = isChecked(i),
                onClick = { if (armed()) onPick(i) },
                modifier = (if (i == start) Modifier.focusRequester(first) else Modifier).then(wrap.at(i)),
                leadingContent = icon(i)?.let { ic -> { Icon(ic, contentDescription = null, modifier = Modifier.size(20.dp)) } },
                headlineContent = { Text(label(i), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                trailingContent = if (isChecked(i)) {
                    { Icon(Icons.Filled.Check, null, tint = Accent) }
                } else {
                    null
                },
            )
        }
    }
}
