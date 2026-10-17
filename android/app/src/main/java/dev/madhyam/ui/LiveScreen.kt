package dev.madhyam.ui

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.DenseListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Button
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.Category
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

/**
 * Live TV (same layout as Windows): categories | channels | preview + guide.
 * OK on a channel plays it in the preview; OK again (or on the preview) goes full screen.
 */
@Composable
fun LiveScreen(vm: MainViewModel) {
    var now by remember { mutableLongStateOf(System.currentTimeMillis() / 1000) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(30_000)
            now = System.currentTimeMillis() / 1000
        }
    }
    if (vm.guideOpen) {
        GuideScreen(vm)
        return
    }
    Row(
        Modifier.fillMaxSize().onPreviewKeyEvent { ev ->
            // Menu opens the TV guide from anywhere in Live TV.
            if (ev.type == KeyEventType.KeyDown && ev.nativeKeyEvent.keyCode == android.view.KeyEvent.KEYCODE_MENU) {
                vm.openGuide()
                true
            } else {
                false
            }
        },
    ) {
        CategoryPanel(vm, Modifier.width(210.dp))
        ChannelList(vm, now, Modifier.weight(1f))
        InfoPane(vm, now, Modifier.width(380.dp))
    }
}

/** Category column shared by Live TV, Movies and Series. */
@kotlin.OptIn(ExperimentalComposeUiApi::class)
@Composable
fun CategoryPanel(vm: MainViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxHeight().background(Panel.copy(alpha = 0.55f)).padding(vertical = 16.dp, horizontal = 8.dp)) {
        if (vm.section == Section.Live && vm.source?.isFreeTv == true) {
            Row(Modifier.padding(start = 4.dp, bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ToggleChip("Categories", vm.liveGroupBy == "category") { vm.groupLiveBy("category") }
                ToggleChip("Countries", vm.liveGroupBy == "country") { vm.groupLiveBy("country") }
            }
        }
        val list = rememberLazyListState()
        LazyColumn(state = list, modifier = Modifier.focusRestorer(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            itemsIndexed(vm.categories, key = { _, c -> (if (c.isAll) "\u0000all" else c.name) }) { _, c ->
                CategoryRow(vm, c)
            }
        }
    }
}

@Composable
private fun CategoryRow(vm: MainViewModel, c: Category) {
    val selected = if (c.isAll) vm.group == null else vm.group == c.name
    DenseListItem(
        selected = selected,
        onClick = { vm.selectCategory(c) },
        // Moving the focus over a category shows its contents, as on most TV apps.
        modifier = Modifier.onFocusChanged { if (it.isFocused) vm.selectCategory(c) },
        headlineContent = {
            Text(c.name, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp,
                fontWeight = if (c.isAll) FontWeight.SemiBold else FontWeight.Normal)
        },
        trailingContent = { Text("${c.count}", color = Muted, fontSize = 11.sp) },
    )
}

@Composable
fun ToggleChip(label: String, on: Boolean, onClick: () -> Unit) {
    val content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = { Text(label, fontSize = 12.sp) }
    val pad = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)
    if (on) Button(onClick = onClick, contentPadding = pad, content = content)
    else OutlinedButton(onClick = onClick, contentPadding = pad, content = content)
}

@kotlin.OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ChannelList(vm: MainViewModel, now: Long, modifier: Modifier) {
    val strip = vm.settings.stripPrefixes
    Column(modifier.fillMaxHeight().padding(horizontal = 12.dp, vertical = 16.dp)) {
        val title = vm.group ?: vm.categories.firstOrNull()?.name ?: "Live TV"
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) { SectionTitle(title, "${vm.total} channels · hold OK for favorite") }
            ToggleChip("TV guide", false) { vm.openGuide() }
        }
        if (vm.items.isEmpty()) {
            EmptyNote(if (vm.busy != null) "" else "No channels here.")
            return@Column
        }
        // Entering Live TV puts the focus on the playing channel (or the first one), not on the rail.
        val start = remember { FocusRequester() }
        val startIndex = vm.items.indexOfFirst { it.id == vm.playback.current?.id }.coerceAtLeast(0)
        LaunchedEffect(Unit) {
            delay(100)
            runCatching { start.requestFocus() }
        }
        LazyColumn(
            Modifier.focusRestorer(),
            state = rememberLazyListState(initialFirstVisibleItemIndex = startIndex),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsIndexed(vm.items, key = { _, it -> it.id }) { i, item ->
                ChannelRow(
                    modifier = if (i == startIndex) Modifier.focusRequester(start) else Modifier,
                    vm = vm,
                    number = i + 1,
                    item = item,
                    now = now,
                    strip = strip,
                    onFocused = {
                        vm.focus(item)
                        if (i > vm.items.size - 30) vm.loadMore()
                    },
                )
            }
        }
    }
}

@Composable
private fun ChannelRow(modifier: Modifier, vm: MainViewModel, number: Int, item: Item, now: Long, strip: Boolean, onFocused: () -> Unit) {
    val nn = vm.nowNext[item.id]
    val playing = vm.playback.current?.id == item.id
    DenseListItem(
        selected = playing,
        onClick = { vm.open(item) },
        onLongClick = { vm.toggleFavorite(item) },
        modifier = modifier.onFocusChanged { if (it.isFocused) onFocused() },
        leadingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (vm.settings.showNumbers) {
                    Text("$number", color = Muted, fontSize = 12.sp, modifier = Modifier.width(34.dp))
                }
                Logo(item, 56.dp, 34.dp, strip)
            }
        },
        headlineContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (playing) Icon(Icons.Filled.PlayArrow, null, tint = Accent, modifier = Modifier.size(16.dp).padding(end = 2.dp))
                Text(item.displayName(strip), maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 15.sp, modifier = Modifier.weight(1f, fill = false))
                if (vm.isFavorite(item)) Icon(Icons.Filled.Star, null, tint = Color(0xFFFFC94D), modifier = Modifier.padding(start = 4.dp).size(14.dp))
            }
        },
        supportingContent = nn?.now?.let { cur ->
            {
                Column {
                    Text(cur.title, color = Muted, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    ProgressBar(cur.progress(now), width = 160.dp)
                }
            }
        },
    )
}

@Composable
private fun InfoPane(vm: MainViewModel, now: Long, modifier: Modifier) {
    val playing = vm.playback.current
    val item = vm.focused ?: playing
    Column(modifier.fillMaxHeight().padding(top = 16.dp, end = 20.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Preview(vm)
        if (item == null) {
            Text("Move to a channel to see what's on.", color = Muted, fontSize = 13.sp)
            return@Column
        }
        val strip = vm.settings.stripPrefixes
        Text(item.displayName(strip), fontSize = 20.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        val nn = vm.nowNext[item.id]
        val cur = nn?.now
        if (cur == null) {
            Text("No guide information for this channel.", color = Muted, fontSize = 13.sp)
        } else {
            Text("${hhmm(cur.start)} – ${hhmm(cur.stop)}  ${cur.title}", fontSize = 15.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
            ProgressBar(cur.progress(now))
            if (cur.desc.isNotBlank()) Text(cur.desc, color = Muted, fontSize = 12.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
        val later = vm.upcoming.filter { it.start > now && (vm.focused?.id ?: playing?.id) == item.id }
        if (later.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Text("Later", color = Accent, fontSize = 13.sp)
            for (p in later.take(6)) {
                Row {
                    Text(hhmm(p.start), color = Muted, fontSize = 12.sp, modifier = Modifier.width(48.dp))
                    Text(p.title, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        } else if (cur != null) {
            nn.next?.let { Text("Next  ${hhmm(it.start)}  ${it.title}", color = Muted, fontSize = 12.sp, maxLines = 1) }
        }
    }
}

/** The preview window: the shared player, focusable so OK on it goes full screen. */
@Composable
private fun Preview(vm: MainViewModel) {
    val cur = vm.playback.current
    Surface(
        onClick = { vm.enterFullScreen() },
        modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f),
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
        colors = ClickableSurfaceDefaults.colors(containerColor = Color.Black, focusedContainerColor = Color.Black),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.02f),
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            if (cur != null) {
                VideoSurface(vm.playback.player, Modifier.fillMaxSize())
                vm.playback.error?.let {
                    Text(it, color = ErrorRed, fontSize = 13.sp, modifier = Modifier.padding(12.dp))
                }
            } else {
                Text("Press OK on a channel to watch it here.\nOK again for full screen.", color = Muted, fontSize = 13.sp)
            }
        }
    }
}

/** A Media3 PlayerView showing [player]; several can exist, the newest one gets the picture. */
@OptIn(UnstableApi::class)
@Composable
fun VideoSurface(player: Player, modifier: Modifier = Modifier, resizeMode: Int = AspectRatioFrameLayout.RESIZE_MODE_FIT) {
    AndroidView(
        factory = { ctx ->
            PlayerView(ctx).apply {
                useController = false // our own TV-style overlay
                keepScreenOn = true
                this.resizeMode = resizeMode
                isFocusable = false
                setShutterBackgroundColor(android.graphics.Color.BLACK)
                this.player = player
            }
        },
        update = { it.resizeMode = resizeMode },
        onRelease = { it.player = null },
        modifier = modifier,
    )
}
