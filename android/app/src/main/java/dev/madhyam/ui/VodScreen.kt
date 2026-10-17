package dev.madhyam.ui

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
import coil.compose.SubcomposeAsyncImage
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

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
    Row(Modifier.fillMaxSize()) {
        CategoryPanel(vm, Modifier.width(230.dp))
        // A new list (section, category, sort, rating) starts with fresh scroll and focus memory.
        key(vm.section, vm.group, vm.settings.vodSort, vm.settings.vodMinRating) {
            val headerRows = if (vm.continueItems.isNotEmpty() && vm.group == null) 2 else 1
            val openedIndex = vm.items.indexOfFirst { it.id == vm.lastOpenedId }
            // Keep the header in view unless the opened poster is further down.
            val state = rememberLazyGridState(initialFirstVisibleItemIndex = if (openedIndex >= 8) openedIndex + headerRows else 0)
            // Entering the grid lands on the opened poster, else the first one (not on Sort).
            val back = remember { FocusRequester() }
            val focusIndex = openedIndex.coerceAtLeast(0)
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
                    .weight(1f)
                    .fillMaxHeight()
                    .focusProperties {
                        val target = focusIndex + headerRows
                        enter = { if (state.layoutInfo.visibleItemsInfo.any { it.index == target }) back else FocusRequester.Default }
                    }
                    .focusGroup(),
                contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 32.dp),
                horizontalArrangement = Arrangement.spacedBy(18.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "\u0000header") {
                    GridHeader(vm, onSort = { dialog = "sort" }, onRating = { dialog = "rating" })
                }
                if (headerRows == 2) {
                    item(span = { GridItemSpan(maxLineSpan) }, key = "\u0000continue") { ContinueRow(vm) }
                }
                itemsIndexed(vm.items, key = { _, it -> it.id }) { i, item ->
                    PosterCard(
                        vm,
                        item,
                        modifier = if (i == focusIndex) Modifier.focusRequester(back) else Modifier,
                        onFocused = { if (i > vm.items.size - 30) vm.loadMore() },
                    )
                }
            }
        }
    }
    when (dialog) {
        "sort" -> OptionDialog("Sort by", SortOptions, vm.settings.vodSort, { vm.setVodSort(it) }) { dialog = null }
        "rating" -> OptionDialog("Minimum rating", RatingOptions.map { it to ratingLabel(it) }, vm.settings.vodMinRating, { vm.setVodMinRating(it) }) {
            dialog = null
        }
    }
}

@Composable
private fun GridHeader(vm: MainViewModel, onSort: () -> Unit, onRating: () -> Unit) {
    val noun = if (vm.section == Section.Series) "series" else "movies"
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(vm.group ?: vm.categories.firstOrNull()?.name ?: vm.section.label, fontSize = 24.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                if (vm.total == 0 && vm.busy == null) "Nothing here${if (vm.settings.vodMinRating > 0) " with this rating" else ""}."
                else "%,d %s · hold OK for favorite".format(vm.total, noun),
                color = Muted,
                fontSize = 12.sp,
            )
        }
        val pad = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
        OutlinedButton(onClick = onSort, contentPadding = pad) {
            Text(SortOptions.firstOrNull { it.first == vm.settings.vodSort }?.second ?: "Provider order", fontSize = 13.sp)
        }
        OutlinedButton(onClick = onRating, contentPadding = pad, modifier = Modifier.padding(start = 8.dp)) {
            Text(ratingLabel(vm.settings.vodMinRating), fontSize = 13.sp)
        }
    }
}

/** Films (Movies) or episodes (Series) started but not finished; OK resumes. */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ContinueRow(vm: MainViewModel) {
    Column {
        Text("Continue watching", fontSize = 17.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(bottom = 8.dp))
        LazyRow(
            Modifier.focusRestorer(),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            contentPadding = PaddingValues(vertical = 8.dp, horizontal = 4.dp),
        ) {
            items(vm.continueItems, key = { it.id }) { item ->
                PosterCard(vm, item, width = 116.dp, onClick = { vm.resume(item) })
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
            onClick = onClick,
            onLongClick = { vm.toggleFavorite(item) },
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
                Poster(item.logo, item.name, Modifier.fillMaxSize())
                item.rating?.let {
                    Badge("★ $it", Modifier.align(Alignment.TopEnd).padding(6.dp))
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
            item.name,
            fontSize = 13.sp,
            maxLines = if (focused) 2 else 1,
            overflow = TextOverflow.Ellipsis,
            color = if (focused) TextColor else TextColor.copy(alpha = 0.8f),
            modifier = Modifier.padding(top = 12.dp),
        )
        if (!sub.isNullOrBlank()) Text(sub, fontSize = 11.sp, color = Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Remote image filling [modifier]; the title on a plain card when there is none (or it fails). */
@Composable
fun Poster(url: String?, title: String, modifier: Modifier, crop: Boolean = true) {
    val fallback: @Composable () -> Unit = {
        Box(modifier.background(Accent.copy(alpha = 0.16f)), contentAlignment = Alignment.Center) {
            Text(title, fontSize = 14.sp, textAlign = TextAlign.Center, maxLines = 4, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(10.dp))
        }
    }
    if (url.isNullOrBlank()) {
        fallback()
    } else {
        SubcomposeAsyncImage(
            model = url,
            contentDescription = null,
            contentScale = if (crop) ContentScale.Crop else ContentScale.Fit,
            modifier = modifier,
            error = { fallback() },
        )
    }
}

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

/** A list of choices in a dialog; the current one is highlighted and gets the focus. */
@Composable
fun <T> OptionDialog(title: String, options: List<Pair<T, String>>, current: T, onPick: (T) -> Unit, onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.width(380.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)) {
            Text(title, fontSize = 22.sp, modifier = Modifier.padding(bottom = 8.dp))
            val first = remember { FocusRequester() }
            Column {
                for ((value, label) in options) {
                    ListItem(
                        selected = value == current,
                        onClick = {
                            onPick(value)
                            onDismiss()
                        },
                        modifier = if (value == current) Modifier.focusRequester(first) else Modifier,
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
    "Small" -> 112.dp
    "Large" -> 172.dp
    else -> 136.dp
}
