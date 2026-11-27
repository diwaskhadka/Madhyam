package dev.madhyam.ui

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
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
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
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
import androidx.tv.material3.Button
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.Category
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import androidx.compose.ui.platform.LocalFocusManager
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
    SectionBack(vm)
    SectionLayout(
        vm,
        210.dp,
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
        // Coming in from the side lands in the channels, not on the button at the same height.
        val channels = remember { FocusRequester() }
        // From the categories: the playing channel, else the last one focused, else the first.
        Row(Modifier.fillMaxSize().sectionContent(vm) { vm.channelEntry ?: if (!vm.lockedGroup && vm.items.isNotEmpty()) channels else FocusRequester.Default }) {
            // On the preview the channels shrink to their logos and the picture gets the room.
            ChannelList(vm, now, channels, if (vm.previewFocused) Modifier.width(88.dp) else Modifier.weight(1f))
            InfoPane(vm, now, if (vm.previewFocused) Modifier.weight(1f) else Modifier.width(380.dp))
        }
    }
}

/**
 * The part of a section beside the category column. Focus inside it closes the column, so the
 * content gets the room; Left from its left edge opens the column again.
 */
@Suppress("DEPRECATION") // FocusProperties.exit, as enter in the rail
@kotlin.OptIn(ExperimentalComposeUiApi::class)
fun Modifier.sectionContent(vm: MainViewModel, entry: () -> FocusRequester = { FocusRequester.Default }): Modifier = this
    .focusProperties {
        // Left/Right from the categories land on the content's own first or remembered item (as
        // TiviMate does), never on a header button at the same height. Up/Down: the nearest.
        enter = { direction ->
            if (!wrapMoving && (direction == FocusDirection.Right || direction == FocusDirection.Left)) entry() else FocusRequester.Default
        }
        exit = { direction ->
            if (direction == FocusDirection.Left && !vm.categoriesOpen) {
                vm.openCategories()
                FocusRequester.Cancel
            } else {
                FocusRequester.Default
            }
        }
    }
    .onFocusChanged { if (it.hasFocus) vm.categoriesOpen = false }
    .focusGroup()

private val StripWidth = 56.dp

/**
 * Category column over [content]. Opening it slides the column in and pushes the content aside at
 * its own size, past the right edge: moved as layers, so nothing is laid out again per frame.
 */
@Composable
fun SectionLayout(vm: MainViewModel, panelWidth: androidx.compose.ui.unit.Dp, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val open = androidx.compose.animation.core.animateFloatAsState(if (vm.categoriesOpen) 1f else 0f, panelMotion(), label = "categories")
    val showPanel by remember { androidx.compose.runtime.derivedStateOf { vm.categoriesOpen || open.value > 0.01f } }
    val showStrip by remember { androidx.compose.runtime.derivedStateOf { !vm.categoriesOpen || open.value < 0.99f } }
    Box(modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().padding(start = StripWidth).graphicsLayer { translationX = open.value * (panelWidth - StripWidth).toPx() }) {
            content()
        }
        if (showStrip) CategoryStrip(vm, Modifier.graphicsLayer { alpha = 1f - open.value })
        if (showPanel) {
            CategoryPanel(
                vm,
                Modifier.width(panelWidth).graphicsLayer {
                    translationX = -(1f - open.value) * (panelWidth - StripWidth).toPx()
                    alpha = open.value
                }.background(Background),
            )
        }
    }
}

/** Short label for the slim column: the prefix before "|", else the initials of two words. */
private fun categoryShort(c: Category): String {
    if (c.isAll) return "All"
    val prefix = c.name.substringBefore('|', "").trim()
    if (prefix.isNotEmpty()) return prefix.take(3).uppercase()
    val words = c.name.split(' ', '-', '_', '/').map { w -> w.filter { it.isLetterOrDigit() } }.filter { it.isNotEmpty() }
    return when {
        words.isEmpty() -> "•"
        words.size == 1 -> words[0].take(2).uppercase()
        else -> "${words[0][0]}${words[1][0]}".uppercase()
    }
}

/**
 * The category column while closed, as the rail is: narrow, the same categories in short, the
 * chosen one marked. Not focusable: Left from the content opens the full column.
 */
@Composable
fun CategoryStrip(vm: MainViewModel, modifier: Modifier = Modifier) {
    val selectedIndex = vm.categories.indexOfFirst { if (it.isAll) vm.group == null else vm.group == it.name }.coerceAtLeast(0)
    LazyColumn(
        modifier = modifier.width(StripWidth).fillMaxHeight().background(Panel.copy(alpha = 0.55f)),
        state = rememberLazyListState(initialFirstVisibleItemIndex = (selectedIndex - 3).coerceAtLeast(0)),
        userScrollEnabled = false,
        contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 16.dp, horizontal = 6.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        itemsIndexed(vm.categories, key = { _, c -> if (c.isAll) "\u0000all" else c.name }) { i, c ->
            val on = i == selectedIndex
            Box(
                Modifier.fillMaxWidth().height(40.dp)
                    .background(if (on) TextColor.copy(alpha = 0.16f) else Color.Transparent, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    categoryShort(c),
                    fontSize = 12.sp,
                    fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (on) TextColor else Muted,
                    maxLines = 1,
                )
            }
        }
    }
}

/** Category column shared by Live TV, Movies and Series. */
@Suppress("DEPRECATION") // FocusProperties.enter, as in the rail
@kotlin.OptIn(ExperimentalComposeUiApi::class)
@Composable
fun CategoryPanel(vm: MainViewModel, modifier: Modifier = Modifier) {
    val wrap = rememberWrap()
    val scope = rememberCoroutineScope()
    val focusManager = LocalFocusManager.current
    var entering by remember { mutableStateOf(false) }
    val list = rememberLazyListState(initialFirstVisibleItemIndex = (vm.categories.indexOfFirst { if (it.isAll) vm.group == null else vm.group == it.name } - 3).coerceAtLeast(0))
    Column(
        modifier.fillMaxHeight().background(Panel.copy(alpha = 0.55f)).padding(vertical = 16.dp, horizontal = 8.dp)
            .wrapColumn(
                onTop = { wrap.focus { vm.categories.lastIndex.also { if (it >= 0) list.scrollToItem(it) } } },
                onBottom = { wrap.focus { list.scrollToItem(0); 0 } },
            ),
    ) {
        if (vm.section == Section.Live && vm.source?.isFreeTv == true) {
            Row(Modifier.padding(start = 4.dp, bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ToggleChip("Categories", vm.liveGroupBy == "category") { vm.groupLiveBy("category") }
                ToggleChip("Countries", vm.liveGroupBy == "country") { vm.groupLiveBy("country") }
            }
        }
        // Open scrolled to the chosen category, and entering the list (Right from the rail) lands
        // on it: landing on the category at the same height would open that one instead.
        val selectedIndex = vm.categories.indexOfFirst { if (it.isAll) vm.group == null else vm.group == it.name }.coerceAtLeast(0)
        val current = remember { FocusRequester() }
        // Reopened with Left from the content: the focus comes back to the chosen category.
        LaunchedEffect(vm.focusCategories) {
            if (vm.focusCategories) {
                withFrameNanos { }
                withFrameNanos { }
                runCatching { current.requestFocus() }
                vm.focusCategories = false
            }
        }
        LazyColumn(
            state = list,
            // Right while the focused category is still loading: wait for it, then go in (going in
            // at once would land in the old list, which is then replaced under the focus).
            modifier = Modifier.onPreviewKeyEvent {
                if (it.type != KeyEventType.KeyDown || it.nativeKeyEvent.keyCode != android.view.KeyEvent.KEYCODE_DPAD_RIGHT) return@onPreviewKeyEvent false
                if (entering) return@onPreviewKeyEvent true
                if (!vm.categoryPending) return@onPreviewKeyEvent false
                entering = true
                scope.launch {
                    try {
                        vm.awaitCategory()
                        withFrameNanos { }
                        withFrameNanos { }
                        focusManager.moveFocus(FocusDirection.Right)
                    } finally {
                        entering = false
                    }
                }
                true
            }.focusProperties {
                enter = { if (!wrap.active && list.layoutInfo.visibleItemsInfo.any { it.index == selectedIndex }) current else FocusRequester.Default }
            }.focusGroup(),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsIndexed(vm.categories, key = { _, c -> (if (c.isAll) "\u0000all" else c.name) }) { i, c ->
                CategoryRow(vm, c, (if (i == selectedIndex) Modifier.focusRequester(current) else Modifier).then(wrap.at(i)))
            }
        }
    }
}

@Composable
private fun CategoryRow(vm: MainViewModel, c: Category, modifier: Modifier = Modifier) {
    val selected = if (c.isAll) vm.group == null else vm.group == c.name
    val focusManager = LocalFocusManager.current
    val scope = rememberCoroutineScope()
    val src = vm.source
    val kind = vm.section.kind
    // Parental controls: a lock (and "Adult" when it was locked for its name).
    val lockReason = if (c.isAll || src == null || kind == null) null else vm.categoryLockReason(src.id, kind, c.name)
    DenseListItem(
        selected = selected,
        // OK on a category goes into its channels or posters (a locked one asks for the PIN first).
        onClick = {
            if (lockReason != null && vm.isCategoryLocked(c.name)) {
                vm.selectCategory(c, fromClick = true)
            } else {
                vm.selectCategory(c)
                // Into its list once its items are in (see CategoryPanel's Right).
                scope.launch {
                    vm.awaitCategory()
                    withFrameNanos { }
                    withFrameNanos { }
                    focusManager.moveFocus(FocusDirection.Right)
                }
            }
        },
        onLongClick = if (c.isAll) null else ({ vm.showOptions(c) }),
        // Resting the focus on a category shows its contents, as on most TV apps (passing over
        // categories with Down loads nothing).
        modifier = modifier.onFocusChanged { if (it.isFocused) vm.previewCategory(c) },
        headlineContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (lockReason != null) CategoryLockIcon(!vm.unlocked, Modifier.padding(end = 6.dp))
                Text(c.name, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp,
                    fontWeight = if (c.isAll) FontWeight.SemiBold else FontWeight.Normal)
            }
        },
        supportingContent = if (lockReason == "Adult") {
            { Text("Adult", color = RowMuted, fontSize = 11.sp) }
        } else {
            null
        },
    )
}

/**
 * A filter or tab chip: on is filled with the accent colour and ticked, off is outlined. One
 * button for both states: swapping two kinds of button on OK dropped the focus to the rail.
 */
@Composable
fun ToggleChip(label: String, on: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val pad = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)
    Button(
        onClick = onClick,
        modifier = modifier,
        contentPadding = pad,
        colors = androidx.tv.material3.ButtonDefaults.colors(
            containerColor = if (on) Accent else Color.Transparent,
            contentColor = if (on) Color.White else TextColor,
            focusedContainerColor = TextColor,
            focusedContentColor = Background,
        ),
        border = androidx.tv.material3.ButtonDefaults.border(
            border = if (on) {
                androidx.tv.material3.Border.None
            } else {
                androidx.tv.material3.Border(androidx.compose.foundation.BorderStroke(1.dp, Muted.copy(alpha = 0.7f)))
            },
        ),
    ) {
        if (on) Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(14.dp))
        Text(if (on) " $label" else label, fontSize = 12.sp)
    }
}

@kotlin.OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ChannelList(vm: MainViewModel, now: Long, channels: FocusRequester, modifier: Modifier) {
    val strip = vm.settings.stripPrefixes
    val compact = vm.previewFocused
    val wrap = rememberWrap()
    // Filled in below; the column's wrap-around scrolls it.
    var listRef: androidx.compose.foundation.lazy.LazyListState? = null
    Column(
        modifier.fillMaxHeight().padding(horizontal = if (compact) 4.dp else 12.dp, vertical = 16.dp)
            .wrapColumn(
                onTop = { wrap.focus { vm.loadAll(); vm.items.lastIndex.also { if (it >= 0) listRef?.scrollToItem(it) } } },
                onBottom = { if (vm.allLoaded) wrap.focus { listRef?.scrollToItem(0); 0 } else vm.loadMore() },
            ),
    ) {
        val title = vm.group ?: vm.categories.firstOrNull()?.name ?: "Live TV"
        if (!compact) Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) { SectionTitle(title, "%,d channels".format(vm.total), subLines = 2) }
            if (!vm.lockedGroup) SectionSearchButton(vm, Modifier.padding(end = 8.dp))
            ToggleChip("TV guide", false) { vm.openGuide() }
        }
        if (vm.lockedGroup) {
            LockedNote()
            return@Column
        }
        if (vm.items.isEmpty()) {
            EmptyNote(if (vm.busy != null) "" else "No channels here.")
            return@Column
        }
        // Coming to this list puts the focus on the playing channel, else the last one focused
        // (back from the player or the guide), else the first; not on the rail. Once per visit:
        // moving through the categories must not pull the focus into each list they open.
        var focusedOnce by remember { mutableStateOf(false) }
        // Each category is a list of its own: its scroll and its remembered row start fresh.
        key(vm.listGroup) {
        val start = remember { FocusRequester() }
        val startIndex = remember {
            val id = vm.playback.current?.id ?: vm.focused?.id
            vm.items.indexOfFirst { it.id == id }.coerceAtLeast(0)
        }
        val listState = rememberLazyListState(initialFirstVisibleItemIndex = startIndex)
        listRef = listState
        LaunchedEffect(Unit) {
            if (focusedOnce) return@LaunchedEffect
            focusedOnce = true
            // Not when the rail is moving in (it lands on the categories).
            if (vm.skipChannelFocus) {
                vm.skipChannelFocus = false
                return@LaunchedEffect
            }
            delay(100)
            // The saved scroll position can have that row off screen, and then it can't take focus.
            if (listState.layoutInfo.visibleItemsInfo.none { it.index == startIndex }) listState.scrollToItem(startIndex)
            androidx.compose.runtime.withFrameNanos { }
            vm.channelEntry = start
            runCatching { start.requestFocus() }
            vm.channelEntry = null
        }
        LazyColumn(
            // Rows scrolling out fade under the header instead of being cut in half.
            Modifier.focusRequester(channels).focusRestorer().fadeEdges(16.dp),
            state = listState,
            contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsIndexed(vm.items, key = { _, it -> it.id }) { i, item ->
                ChannelRow(
                    modifier = (if (i == startIndex) Modifier.focusRequester(start) else Modifier).then(wrap.at(i)),
                    vm = vm,
                    number = i + 1,
                    item = item,
                    now = now,
                    strip = strip,
                    compact = compact,
                    onFocused = {
                        vm.focus(item)
                        if (i > vm.items.size - 30) vm.loadMore()
                    },
                )
            }
        }
        }
    }
}

@Composable
private fun ChannelRow(modifier: Modifier, vm: MainViewModel, number: Int, item: Item, now: Long, strip: Boolean, compact: Boolean, onFocused: () -> Unit) {
    val nn = vm.nowNext[item.id]
    val playing = vm.playback.current?.id == item.id
    var focused by remember { mutableStateOf(false) }
    DenseListItem(
        selected = playing,
        onClick = { vm.pick(item) { vm.open(item) } },
        onLongClick = { vm.showOptions(item) },
        modifier = modifier.height(if (nn?.now != null && !compact) 58.dp else 46.dp).onFocusChanged {
            focused = it.isFocused
            if (it.isFocused) onFocused()
        },
        leadingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (vm.isSelected(item)) SelectedMark(Modifier.padding(end = 8.dp))
                if (vm.settings.showNumbers && !compact) {
                    // Wide enough for the list's longest number (53,154 channels: five digits).
                    val digits = maxOf(vm.total, number).toString().length
                    Text("$number", color = RowMuted, fontSize = 12.sp, maxLines = 1, softWrap = false, modifier = Modifier.width(maxOf(34, digits * 8 + 4).dp))
                }
                if (compact) Logo(item, 40.dp, 26.dp, strip) else Logo(item, 44.dp, 28.dp, strip)
            }
        },
        headlineContent = {
            if (!compact) Row(verticalAlignment = Alignment.CenterVertically) {
                if (playing) Icon(Icons.Filled.PlayArrow, null, tint = RowAccent, modifier = Modifier.size(16.dp).padding(end = 2.dp))
                // The column is narrow: the focused name scrolls to show all of it.
                Text(
                    item.displayName(strip),
                    maxLines = 1,
                    softWrap = false,
                    overflow = if (focused) TextOverflow.Clip else TextOverflow.Ellipsis,
                    fontSize = 15.sp,
                    modifier = Modifier.weight(1f, fill = false)
                        .then(if (focused) Modifier.basicMarquee(iterations = Int.MAX_VALUE, initialDelayMillis = 900) else Modifier),
                )
                if (vm.isFavorite(item)) Icon(Icons.Filled.Star, null, tint = Color(0xFFFFC94D), modifier = Modifier.padding(start = 4.dp).size(14.dp))
            }
        },
        supportingContent = if (compact) null else nn?.now?.let { cur ->
            {
                Column {
                    Text(cur.title, color = RowMuted, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
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
        // On the preview, the info below is about what plays in it, not the last row focused
        // (which may be a separator row the focus passed on the way).
        modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f)
            .onFocusChanged { f ->
                vm.previewFocused = f.isFocused
                if (f.isFocused) cur?.let { vm.focus(it) }
            },
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
        colors = ClickableSurfaceDefaults.colors(containerColor = Color.Black, focusedContainerColor = Color.Black),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.02f),
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            if (cur != null) {
                VideoSurface(vm.playback.player, Modifier.fillMaxSize(), style = vm.settings)
                vm.playback.error?.let {
                    Column(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.6f)).padding(14.dp), verticalArrangement = Arrangement.Center) {
                        Text(it, color = ErrorRed, fontSize = 13.sp)
                        Text(
                            "OK on the channel to try again" + (vm.playback.errorDetail?.let { d -> "  ·  $d" } ?: ""),
                            color = Color.White.copy(alpha = 0.7f), fontSize = 11.sp, modifier = Modifier.padding(top = 6.dp),
                        )
                    }
                }
            } else {
                Text("Press OK on a channel to watch it here.\nOK again for full screen.", color = Muted, fontSize = 13.sp)
            }
        }
    }
}

/**
 * A Media3 PlayerView showing [player]; several can exist, the newest one gets the picture.
 * [subtitleLift] raises subtitles by that fraction of the height, above an overlay at the bottom.
 * [style]: Settings › Playback › Subtitle style, applied to every kind of subtitle.
 */
@OptIn(UnstableApi::class)
@Composable
fun VideoSurface(
    player: Player,
    modifier: Modifier = Modifier,
    resizeMode: Int = AspectRatioFrameLayout.RESIZE_MODE_FIT,
    subtitleLift: Float = 0f,
    style: dev.madhyam.AppSettings? = null,
) {
    // Read here so a style change redraws the view (the settings are snapshot state).
    val styleKey = style?.let {
        listOf(it.subFontPt, it.subFont, it.subTextColor, it.subTextOpacity, it.subBackColor, it.subBackOpacity,
            it.subWindowColor, it.subWindowOpacity, it.subEdge, it.subPosition, it.subOffset)
    }
    // Settings › Subtitle style › Position: when the user moved subtitles, streams' own cue
    // positions (CEA-608 rows, WebVTT "line") give way. Added after the PlayerView's own listener,
    // so these cues are the ones shown.
    val view = remember { arrayOfNulls<PlayerView>(1) }
    if (style != null) {
        androidx.compose.runtime.DisposableEffect(player) {
            val listener = object : Player.Listener {
                override fun onCues(cueGroup: androidx.media3.common.text.CueGroup) {
                    if (dev.madhyam.SubtitleStyle.movesCues(style)) {
                        view[0]?.subtitleView?.setCues(dev.madhyam.SubtitleStyle.placeCues(cueGroup.cues))
                    }
                }
            }
            player.addListener(listener)
            onDispose { player.removeListener(listener) }
        }
    }
    AndroidView(
        factory = { ctx ->
            PlayerView(ctx).also { view[0] = it }.apply {
                useController = false // our own TV-style overlay
                keepScreenOn = true
                this.resizeMode = resizeMode
                isFocusable = false
                setShutterBackgroundColor(android.graphics.Color.BLACK)
                this.player = player
                // The text size follows the picture's height (preview window, full screen).
                subtitleView?.addOnLayoutChangeListener { v, _, top, _, bottom, _, oldTop, _, oldBottom ->
                    val s = v.tag as? dev.madhyam.AppSettings ?: return@addOnLayoutChangeListener
                    if (bottom - top != oldBottom - oldTop) dev.madhyam.SubtitleStyle.apply(v as androidx.media3.ui.SubtitleView, s, bottom - top)
                }
            }
        },
        update = {
            it.resizeMode = resizeMode
            // Move the whole view: broadcast captions (CEA-608) place themselves and ignore padding.
            it.subtitleView?.translationY = -subtitleLift * it.height
            @Suppress("UNUSED_EXPRESSION") styleKey
            it.subtitleView?.let { sv ->
                sv.tag = style
                if (style != null) dev.madhyam.SubtitleStyle.apply(sv, style)
            }
        },
        onRelease = { it.player = null },
        modifier = modifier,
    )
}
