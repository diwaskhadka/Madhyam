package dev.madhyam.ui

import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Star
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Border
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import coil.compose.AsyncImage
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.platform.LocalFocusManager

/** Sort names understood by the engine's `query` (catalog.rs Sort::parse), as on Windows. */
val SortOptions = listOf(
    "provider" to "Provider order",
    "added_desc" to "Date added (latest)",
    "added_asc" to "Date added (oldest)",
    "name_asc" to "Name A–Z",
    "name_desc" to "Name Z–A",
    "rating_desc" to "Top rated",
    "rating_asc" to "Lowest rated",
)

val RatingOptions = listOf(0, 5, 6, 7, 8, 9)

private fun ratingLabel(r: Int) = if (r <= 0) "Any rating" else "Rating $r+"

/**
 * Movies and Series (Windows layout): categories | poster grid, with sort and minimum rating
 * above it and a "Continue watching" row on top. The grid pages in as the focus moves down.
 */
@Suppress("DEPRECATION") // FocusProperties.enter, as in the rail
@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun VodScreen(vm: MainViewModel) {
    var dialog by remember { mutableStateOf<String?>(null) }
    // A new sort or rating rebuilds the grid, header included: put the focus back on the button.
    var refocus by remember { mutableStateOf<String?>(null) }
    SectionBack(vm)
    SectionLayout(vm, 230.dp) {
        // A new list (section, category, sort, rating) starts with fresh scroll and focus memory.
        if (vm.lockedGroup) {
            Column(Modifier.fillMaxSize().padding(start = 24.dp, top = 16.dp)) {
                Text(vm.group ?: "", fontSize = 24.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                LockedNote()
            }
            return@SectionLayout
        }
        // Left/Right from the categories land here (see entryIndex below).
        val entry = remember { FocusRequester() }
        var entryReady by remember { mutableStateOf(false) }
        Box(Modifier.fillMaxSize().sectionContent(vm) { if (entryReady) entry else FocusRequester.Default }) {
        key(vm.section, vm.group, vm.settings.vodSort, vm.settings.vodMinRating) {
            val headerRows = if (vm.continueItems.isNotEmpty() && vm.group == null) 2 else 1
            val openedIndex = vm.items.indexOfFirst { it.id == vm.lastOpenedId }
            // Keep the header in view unless the opened poster is further down.
            val state = rememberLazyGridState(initialFirstVisibleItemIndex = if (openedIndex >= 8) openedIndex + headerRows else 0)
            // Entering the grid lands on the opened poster, else the first one (not on Sort).
            val back = remember { FocusRequester() }
            val focusIndex = openedIndex.coerceAtLeast(0)
            val sortButton = remember { FocusRequester() }
            val ratingButton = remember { FocusRequester() }
            val wrap = rememberWrap()
            val focusManager = LocalFocusManager.current
            val scope = rememberCoroutineScope()
            // Coming from the categories (as TiviMate does): the poster last focused in this list if
            // it's on screen, else Continue watching, else the first poster on screen.
            var lastFocused by remember { mutableIntStateOf(-1) }
            // The poster that has the focus now (-1: the header, Continue watching or elsewhere).
            var focusedPoster by remember { mutableIntStateOf(-1) }
            var wrapping by remember { mutableStateOf(false) }
            val entryIndex by remember {
                derivedStateOf {
                    val visible = state.layoutInfo.visibleItemsInfo.map { it.index }
                    when {
                        lastFocused >= 0 && lastFocused in visible -> lastFocused
                        headerRows == 2 && 1 in visible -> 1
                        else -> visible.firstOrNull { it >= headerRows } ?: -1
                    }
                }
            }
            // Read here, in composition, so the flag follows the list as it lays out (a yes/no
            // state: scrolling doesn't recompose the grid for it).
            val ready by remember { derivedStateOf { entryIndex >= 0 } }
            val readyNow = ready // read in composition (not only inside the effect), so it updates
            SideEffect { entryReady = readyNow }
            fun entryAt(index: Int): Modifier = if (index == entryIndex) Modifier.focusRequester(entry) else Modifier
            LaunchedEffect(Unit) {
                val target = refocus ?: return@LaunchedEffect
                refocus = null
                // Twice: when the dialog's window closes, the page's window restores its own focus.
                val button = if (target == "rating") ratingButton else sortButton
                delay(150)
                runCatching { button.requestFocus() }
                delay(350)
                runCatching { button.requestFocus() }
            }
            // Back from a details page: the focus returns to the poster that was opened.
            LaunchedEffect(Unit) {
                if (openedIndex >= 0) {
                    delay(100)
                    runCatching { back.requestFocus() }
                }
            }
            LazyVerticalGrid(
                columns = GridCells.Adaptive(coverWidth(vm.settings.coverSize)), // Settings › Appearance › Cover size
                state = state,
                modifier = Modifier
                    .fillMaxSize()
                    // Right at the end of a row: the first poster of the next row (held Right walks
                    // through the whole list); on the last poster it does nothing.
                    .onPreviewKeyEvent { ev ->
                        if (ev.type != KeyEventType.KeyDown || ev.nativeKeyEvent.keyCode != android.view.KeyEvent.KEYCODE_DPAD_RIGHT) return@onPreviewKeyEvent false
                        val i = focusedPoster
                        if (i < 0) return@onPreviewKeyEvent false
                        if (wrapping) return@onPreviewKeyEvent true // a held key while the focus is still moving
                        if (focusManager.moveFocus(FocusDirection.Right)) return@onPreviewKeyEvent true
                        val next = i + 1
                        if (next < vm.items.size) {
                            wrapping = true
                            wrap.focus {
                                val cell = headerRows + next
                                if (state.layoutInfo.visibleItemsInfo.none { it.index == cell }) state.scrollToItem(cell)
                                cell
                            }
                            scope.launch { delay(120); wrapping = false }
                        } else if (!vm.allLoaded) {
                            vm.loadMore()
                        }
                        true
                    }
                    // Up past the header: the last poster (all of them loaded); Down past the last:
                    // the first row of content (Continue watching when it shows).
                    .wrapColumn(
                        onTop = {
                            wrap.focus {
                                vm.loadAll()
                                if (vm.items.isEmpty()) -1 else (headerRows + vm.items.lastIndex).also { state.scrollToItem(it) }
                            }
                        },
                        onBottom = {
                            if (vm.allLoaded) {
                                wrap.focus { state.scrollToItem(0); if (headerRows == 2) 1 else headerRows }
                            } else {
                                vm.loadMore()
                            }
                        },
                    ),
                contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 32.dp),
                horizontalArrangement = Arrangement.spacedBy(18.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "\u0000header") {
                    GridHeader(vm, sortButton, ratingButton, onSort = { dialog = "sort" }, onRating = { dialog = "rating" })
                }
                if (headerRows == 2) {
                    item(span = { GridItemSpan(maxLineSpan) }, key = "\u0000continue") { ContinueRow(vm, wrap.at(1).then(entryAt(1)).onFocusChanged { if (it.hasFocus) lastFocused = 1 }) }
                }
                itemsIndexed(vm.items, key = { _, it -> it.id }) { i, item ->
                    PosterCard(
                        vm,
                        item,
                        modifier = (if (i == focusIndex) Modifier.focusRequester(back) else Modifier).then(wrap.at(headerRows + i)).then(entryAt(headerRows + i))
                            .onFocusChanged { if (it.hasFocus) focusedPoster = i else if (focusedPoster == i) focusedPoster = -1 },
                        onFocused = {
                            lastFocused = headerRows + i
                            if (i > vm.items.size - 30) vm.loadMore()
                        },
                    )
                }
            }
        }
        }
    }
    when (dialog) {
        "sort" -> OptionDialog("Sort by", SortOptions, vm.settings.vodSort, { refocus = "sort"; vm.setVodSort(it) }) { dialog = null }
        "rating" -> OptionDialog("Minimum rating", RatingOptions.map { it to ratingLabel(it) }, vm.settings.vodMinRating, { refocus = "rating"; vm.setVodMinRating(it) }) {
            dialog = null
        }
    }
}

@Composable
private fun GridHeader(vm: MainViewModel, sortButton: FocusRequester, ratingButton: FocusRequester, onSort: () -> Unit, onRating: () -> Unit) {
    val noun = if (vm.section == Section.Series) "series" else "movies"
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(vm.group ?: vm.categories.firstOrNull()?.name ?: vm.section.label, fontSize = 24.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                if (vm.total == 0 && vm.busy == null) "Nothing here${if (vm.settings.vodMinRating > 0) " with this rating" else ""}."
                else "%,d %s".format(vm.total, noun),
                color = Muted,
                fontSize = 12.sp,
            )
        }
        SectionSearchButton(vm, Modifier.padding(end = 8.dp))
        val pad = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
        OutlinedButton(onClick = onSort, contentPadding = pad, modifier = Modifier.focusRequester(sortButton)) {
            Text(SortOptions.firstOrNull { it.first == vm.settings.vodSort }?.second ?: "Provider order", fontSize = 13.sp)
        }
        OutlinedButton(onClick = onRating, contentPadding = pad, modifier = Modifier.padding(start = 8.dp).focusRequester(ratingButton)) {
            Text(ratingLabel(vm.settings.vodMinRating), fontSize = 13.sp)
        }
    }
}

/** Films (Movies) or episodes (Series) started but not finished; OK resumes. */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ContinueRow(vm: MainViewModel, modifier: Modifier = Modifier) {
    // Left on the first card goes back to the categories, not down to the poster a little to its left.
    var first by remember { mutableStateOf(false) }
    Column(modifier.focusGroup()) {
        Text("Continue watching", fontSize = 17.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(bottom = 8.dp))
        LazyRow(
            Modifier.focusRestorer().onPreviewKeyEvent {
                if (first && it.type == KeyEventType.KeyDown && it.nativeKeyEvent.keyCode == android.view.KeyEvent.KEYCODE_DPAD_LEFT) {
                    vm.openCategories()
                    true
                } else {
                    false
                }
            },
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            contentPadding = PaddingValues(vertical = 8.dp, horizontal = 4.dp),
        ) {
            itemsIndexed(vm.continueItems, key = { _, it -> it.id }) { i, item ->
                PosterCard(vm, item, Modifier.onFocusChanged { f -> if (f.isFocused || f.hasFocus) first = i == 0 }, width = 116.dp, onClick = { vm.resume(item) })
            }
        }
    }
}

/** A poster that grows when focused, with the name under it; rating, favorite and progress on it. */
@Composable
fun PosterCard(
    vm: MainViewModel,
    item: Item,
    modifier: Modifier = Modifier,
    width: Dp? = null,
    onFocused: () -> Unit = {},
    onClick: () -> Unit = { vm.open(item) },
) {
    var focused by remember { mutableStateOf(false) }
    Column(if (width != null) modifier.width(width) else modifier.fillMaxWidth()) {
        Surface(
            onClick = { vm.pick(item, onClick) },
            onLongClick = { vm.showOptions(item) },
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(2f / 3f)
                .onFocusChanged {
                    focused = it.isFocused
                    if (it.isFocused) onFocused()
                },
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
            colors = ClickableSurfaceDefaults.colors(containerColor = Panel, focusedContainerColor = Panel),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1.1f),
            border = ClickableSurfaceDefaults.border(
                focusedBorder = Border(androidx.compose.foundation.BorderStroke(3.dp, TextColor), shape = RoundedCornerShape(10.dp)),
            ),
        ) {
            Box(Modifier.fillMaxSize()) {
                Poster(item.logo, item.name, Modifier.fillMaxSize(), small = true)
                SelectedOverlay(vm, item)
                item.ratingLabel?.let {
                    Badge("★ $it", Modifier.align(Alignment.TopEnd).padding(6.dp))
                }
                // Providers list a film once per version ("4K-TOP - …", "TOP - …", "EN - …"):
                // without titles under the covers, the tag tells the look-alike posters apart.
                if (!item.isEpisode && !vm.settings.titlesUnderCovers) item.nameTag?.let {
                    Badge(it, Modifier.align(Alignment.BottomStart).padding(start = 6.dp, bottom = 12.dp))
                }
                if (vm.isFavorite(item)) {
                    Icon(Icons.Filled.Star, null, tint = Color(0xFFFFC94D), modifier = Modifier.align(Alignment.TopStart).padding(6.dp).size(18.dp))
                }
                if (item.isWatched) {
                    WatchedMark(Modifier.align(Alignment.BottomEnd).padding(6.dp))
                } else if (item.isStarted) {
                    ProgressBar(item.watchedRatio, Modifier.align(Alignment.BottomCenter).padding(horizontal = 8.dp, vertical = 4.dp))
                }
            }
        }
        // Settings › Appearance › Titles under covers (the title still shows on the focused cover).
        if (!vm.settings.titlesUnderCovers && !focused) return@Column
        val sub = if (item.isEpisode) item.seasonEpisode else item.year
        Text(
            if (item.isEpisode) item.episodeTitle.ifBlank { item.name } else item.name,
            fontSize = 13.sp,
            lineHeight = 16.sp, // the theme's line height leaves a gap between two lines this small
            maxLines = if (focused) 2 else 1,
            overflow = TextOverflow.Ellipsis,
            color = if (focused) TextColor else TextColor.copy(alpha = 0.8f),
            modifier = Modifier.padding(top = 12.dp),
        )
        if (!sub.isNullOrBlank()) Text(sub, fontSize = 11.sp, color = Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/**
 * Remote image filling [modifier]; the title on a plain card when there is none (or it fails).
 * [small]: a grid poster, for which a smaller TMDB picture is enough.
 *
 * A plain AsyncImage, not SubcomposeAsyncImage: subcomposing every poster made scrolling a big
 * grid stutter on TVs.
 */
@Composable
fun Poster(url: String?, title: String, modifier: Modifier, crop: Boolean = true, small: Boolean = false) {
    var failed by remember(url) { mutableStateOf(false) }
    if (url.isNullOrBlank() || failed) {
        Box(modifier.background(Accent.copy(alpha = 0.16f)), contentAlignment = Alignment.Center) {
            Text(title, fontSize = 14.sp, textAlign = TextAlign.Center, maxLines = 4, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(10.dp))
        }
    } else {
        AsyncImage(
            model = if (small) smallPoster(url) else url,
            contentDescription = null,
            contentScale = if (crop) ContentScale.Crop else ContentScale.Fit,
            modifier = modifier,
            onError = { failed = true },
        )
    }
}

private val TmdbSize = Regex("""(image\.tmdb\.org/t/p/)[^/]+/""")

/** TMDB posters come as 600×900 or larger; a grid cover is under 350 pixels wide. */
private fun smallPoster(url: String): String = if ("image.tmdb.org" in url) TmdbSize.replaceFirst(url, "$1w342/") else url

@Composable
fun Badge(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        fontSize = 11.sp,
        fontWeight = FontWeight.SemiBold,
        color = Color.White,
        modifier = modifier.background(Color.Black.copy(alpha = 0.7f), RoundedCornerShape(5.dp)).padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

@Composable
fun WatchedMark(modifier: Modifier = Modifier) {
    Box(modifier.size(24.dp).clip(RoundedCornerShape(12.dp)).background(Accent), contentAlignment = Alignment.Center) {
        Icon(Icons.Filled.Check, contentDescription = "Watched", tint = Color.White, modifier = Modifier.size(16.dp))
    }
}

/** The tick on an item chosen with Select multiple (hold OK). */
@Composable
fun SelectedMark(modifier: Modifier = Modifier) {
    Box(modifier.size(28.dp).clip(RoundedCornerShape(14.dp)).background(Color.White), contentAlignment = Alignment.Center) {
        Icon(Icons.Filled.Check, contentDescription = "Selected", tint = Accent, modifier = Modifier.size(20.dp))
    }
}

/** A selected item's picture: tinted, with the tick on it. */
@Composable
fun androidx.compose.foundation.layout.BoxScope.SelectedOverlay(vm: MainViewModel, item: Item) {
    if (!vm.isSelected(item)) return
    Box(Modifier.matchParentSize().background(Accent.copy(alpha = 0.35f)))
    SelectedMark(Modifier.align(Alignment.Center))
}

/** A list of choices in a dialog; the current one is highlighted and gets the focus. */
@Composable
fun <T> OptionDialog(title: String, options: List<Pair<T, String>>, current: T, onPick: (T) -> Unit, onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.width(380.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)) {
            Text(title, fontSize = 22.sp, modifier = Modifier.padding(bottom = 8.dp))
            val first = remember { FocusRequester() }
            val wrap = rememberWrap()
            Column(Modifier.wrapColumn(onTop = { wrap.focus { options.lastIndex } }, onBottom = { wrap.focus { 0 } })) {
                for ((i, pair) in options.withIndex()) {
                    val (value, label) = pair
                    ListItem(
                        selected = value == current,
                        onClick = {
                            onPick(value)
                            onDismiss()
                        },
                        modifier = (if (value == current) Modifier.focusRequester(first) else Modifier).then(wrap.at(i)),
                        headlineContent = { Text(label) },
                    )
                }
            }
            LaunchedEffect(Unit) {
                delay(50)
                runCatching { first.requestFocus() }
            }
        }
    }
}

/** Poster column width for Settings › Appearance › Cover size. */
fun coverWidth(size: String): Dp = when (size) {
    "Tiny" -> 76.dp
    "Extra small" -> 92.dp
    "Small" -> 112.dp
    "Large" -> 172.dp
    else -> 136.dp
}
