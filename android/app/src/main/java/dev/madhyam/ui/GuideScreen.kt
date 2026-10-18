package dev.madhyam.ui

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.History
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Programme
import kotlinx.coroutines.delay

private val ChannelColumn = 200.dp
private val RowHeight = 54.dp

/**
 * TV guide: the channels of the current category against a 2½-hour window. Left/Right past
 * the edge moves the window by an hour; OK on what is on now watches the channel, OK on a
 * recorded past programme (↺, Xtream catch-up) plays the recording.
 */
@Composable
fun GuideScreen(vm: MainViewModel) {
    BackHandler { vm.closeGuide() }
    var now by remember { mutableLongStateOf(System.currentTimeMillis() / 1000) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(30_000)
            now = System.currentTimeMillis() / 1000
        }
    }
    var selected by remember { mutableStateOf<Pair<Item, Programme?>?>(null) }
    val channels = vm.items.filter { it.isLive }
    val from = vm.guideFrom
    val to = vm.guideTo
    val strip = vm.settings.stripPrefixes

    Column(Modifier.fillMaxSize().padding(start = 16.dp, end = 24.dp, top = 16.dp, bottom = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("TV guide · ${vm.group ?: vm.categories.firstOrNull()?.name ?: "Live TV"}", fontSize = 22.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("${dayLabel(from)}  ${hhmm(from)} – ${hhmm(to)}   ·   ◀ ▶ past the edge moves an hour   ·   ↺ recording", color = Muted, fontSize = 12.sp)
            }
            val pad = PaddingValues(horizontal = 12.dp, vertical = 4.dp)
            OutlinedButton(onClick = { vm.shiftGuide(-120) }, contentPadding = pad) { Text("◀ Earlier", fontSize = 12.sp) }
            Spacer(Modifier.width(6.dp))
            OutlinedButton(onClick = { vm.guideNow() }, contentPadding = pad) { Text("Now", fontSize = 12.sp) }
            Spacer(Modifier.width(6.dp))
            OutlinedButton(onClick = { vm.shiftGuide(120) }, contentPadding = pad) { Text("Later ▶", fontSize = 12.sp) }
        }
        Details(vm, selected, now)
        if (channels.isEmpty()) {
            EmptyNote("No channels in this category.")
            return@Column
        }
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val timeline = maxWidth - ChannelColumn
            val perSec = timeline / (to - from).toFloat()
            Column {
                TimeRuler(from, to, perSec)
                // The guide opens at the channel that was playing (or focused), a couple of rows down.
                val start = remember { FocusRequester() }
                val startIndex = remember { channels.indexOfFirst { it.id == vm.guideStartId }.coerceAtLeast(0) }
                LaunchedEffect(Unit) {
                    delay(150)
                    runCatching { start.requestFocus() }
                }
                LazyColumn(
                    state = rememberLazyListState(initialFirstVisibleItemIndex = (startIndex - 2).coerceAtLeast(0)),
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    itemsIndexed(channels, key = { _, c -> c.id }) { i, ch ->
                        GuideRow(
                            vm, ch, vm.guideData[ch.id].orEmpty(), from, to, now, perSec, strip,
                            focusFirst = if (i == startIndex) start else null,
                            onFocus = { p ->
                                selected = ch to p
                                if (i > channels.size - 20) vm.loadMore()
                            },
                        )
                    }
                }
            }
            // The "now" line across all rows.
            if (now in from until to) {
                Box(
                    Modifier.offset(x = ChannelColumn + perSec * (now - from).toFloat(), y = 22.dp)
                        .width(2.dp).fillMaxHeight().background(Accent.copy(alpha = 0.8f)),
                )
            }
        }
    }
}

@Composable
private fun Details(vm: MainViewModel, selected: Pair<Item, Programme?>?, now: Long) {
    Column(Modifier.fillMaxWidth().height(78.dp).padding(vertical = 6.dp)) {
        val (ch, p) = selected ?: return@Column
        val strip = vm.settings.stripPrefixes
        if (p == null) {
            Text(ch.displayName(strip), fontSize = 16.sp)
            Text("No guide information · OK to watch", color = Muted, fontSize = 12.sp)
            return@Column
        }
        val action = when {
            p.start <= now && now < p.stop -> "OK to watch"
            p.stop <= now && vm.canCatchup(ch, p) -> "OK to play the recording"
            p.stop <= now -> "Not recorded"
            else -> "Starts at ${hhmm(p.start)}"
        }
        Text("${p.title}", fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text("${ch.displayName(strip)} · ${dayLabel(p.start)} ${hhmm(p.start)} – ${hhmm(p.stop)} · $action", color = Accent, fontSize = 12.sp, maxLines = 1)
        if (p.desc.isNotBlank()) Text(p.desc, color = Muted, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun TimeRuler(from: Long, to: Long, perSec: Dp) {
    Box(Modifier.fillMaxWidth().height(22.dp)) {
        var t = from - from % 1800
        while (t < to) {
            if (t >= from) {
                Text(hhmm(t), color = Muted, fontSize = 11.sp, modifier = Modifier.offset(x = ChannelColumn + perSec * (t - from).toFloat()))
            }
            t += 1800
        }
    }
}

@Composable
private fun GuideRow(
    vm: MainViewModel,
    ch: Item,
    programmes: List<Programme>,
    from: Long,
    to: Long,
    now: Long,
    perSec: Dp,
    strip: Boolean,
    focusFirst: FocusRequester?,
    onFocus: (Programme?) -> Unit,
) {
    Row(Modifier.fillMaxWidth().height(RowHeight)) {
        Row(
            Modifier.width(ChannelColumn).fillMaxHeight().padding(end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Logo(ch, 48.dp, 30.dp, strip)
            Spacer(Modifier.width(8.dp))
            Text(ch.displayName(strip), fontSize = 13.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Box(Modifier.weight(1f).fillMaxHeight()) {
            val visible = programmes.filter { it.stop > from && it.start < to }.sortedBy { it.start }
            if (visible.isEmpty()) {
                Cell(vm, ch, null, from, to, from, to, now, perSec, focusFirst, onFocus)
            } else {
                // The first cell of the row to focus on entry: what is on now, else the first one.
                val first = visible.firstOrNull { it.start <= now && now < it.stop } ?: visible.first()
                for (p in visible) {
                    androidx.compose.runtime.key(p.start) {
                        Cell(vm, ch, p, maxOf(p.start, from), minOf(p.stop, to), from, to, now, perSec,
                            if (p === first) focusFirst else null, onFocus)
                    }
                }
            }
        }
    }
}

@Composable
private fun Cell(
    vm: MainViewModel,
    ch: Item,
    p: Programme?,
    start: Long,
    stop: Long,
    from: Long,
    to: Long,
    now: Long,
    perSec: Dp,
    requester: FocusRequester?,
    onFocus: (Programme?) -> Unit,
) {
    val onNow = p == null || (p.start <= now && now < p.stop)
    val past = p != null && p.stop <= now
    val recorded = past && vm.canCatchup(ch, p)
    var m = Modifier
        .offset(x = perSec * (start - from).toFloat())
        .width((perSec * (stop - start).toFloat() - 3.dp).coerceAtLeast(6.dp))
        .fillMaxHeight()
        .onFocusChanged { if (it.isFocused) onFocus(p) }
        .onPreviewKeyEvent { ev ->
            if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
            when (ev.nativeKeyEvent.keyCode) {
                // At the edge of the window: move it instead of leaving the row.
                AndroidKeyEvent.KEYCODE_DPAD_LEFT -> if (start <= from) { vm.shiftGuide(-60); true } else false
                AndroidKeyEvent.KEYCODE_DPAD_RIGHT -> if (stop >= to) { vm.shiftGuide(60); true } else false
                else -> false
            }
        }
    if (requester != null) m = m.focusRequester(requester)
    Surface(
        onClick = { vm.openFromGuide(ch, p) },
        modifier = m,
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(6.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = when {
                onNow -> Accent.copy(alpha = 0.22f)
                past -> Panel.copy(alpha = 0.6f)
                else -> Panel
            },
            contentColor = if (past && !recorded) Muted else TextColor,
            focusedContainerColor = TextColor,
            focusedContentColor = Background,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (recorded) {
                Icon(Icons.Filled.History, contentDescription = "Recorded", modifier = Modifier.size(14.dp))
                Spacer(Modifier.width(4.dp))
            }
            Column {
                Text(p?.title ?: "No information", fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (p != null) Text("${hhmm(p.start)} – ${hhmm(p.stop)}", fontSize = 10.sp, maxLines = 1)
            }
        }
    }
}
