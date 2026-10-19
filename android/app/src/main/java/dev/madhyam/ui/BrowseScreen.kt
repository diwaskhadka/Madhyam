package dev.madhyam.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

/** Movies and Series (poster grid), Favorites, Recent and Search (lists); a film's or series' details page over them. */
@Composable
fun BrowseScreen(vm: MainViewModel) {
    when {
        vm.details != null -> DetailsScreen(vm)
        vm.section.kind != null -> VodScreen(vm)
        else -> Column(Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 16.dp)) {
            Header(vm)
            ItemList(vm)
        }
    }
}

@Composable
private fun Header(vm: MainViewModel) {
    if (vm.section == Section.Search) {
        // Opening Search puts the focus on the box (not on a filter chip). Until OK is pressed the
        // box is a plain bar, not a text field: some TVs open the keyboard as soon as a text field
        // has the focus, and the next OK then typed the keyboard's first key. OK on the bar swaps
        // in the text field with the keyboard; leaving the field (Down, Up) brings the bar back.
        // With results showing (back from one of them), the list keeps the focus.
        val box = remember { FocusRequester() }
        val field = remember { FocusRequester() }
        // Down from the box goes to the filter chips under it, not to a poster further down.
        val chips = remember { FocusRequester() }
        var editing by remember { mutableStateOf(false) }
        val keyboard = androidx.compose.ui.platform.LocalSoftwareKeyboardController.current
        LaunchedEffect(Unit) {
            if (vm.query.isBlank()) {
                delay(100)
                runCatching { box.requestFocus() }
            }
        }
        if (editing) {
            var hadFocus by remember { mutableStateOf(false) }
            LaunchedEffect(Unit) {
                delay(50)
                runCatching { field.requestFocus() }
                keyboard?.show()
            }
            OutlinedTextField(
                value = vm.query,
                onValueChange = vm::search,
                label = { androidx.compose.material3.Text("Search channels, movies, series") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp).focusRequester(field).focusProperties { down = chips }
                    .onFocusChanged { if (it.hasFocus) hadFocus = true else if (hadFocus) editing = false }
                    .tvTextField(atStart = { vm.query.isEmpty() }),
            )
        } else {
            SearchBar(vm.query, Modifier.fillMaxWidth().padding(bottom = 10.dp).focusRequester(box).focusProperties { down = chips }) { editing = true }
        }
        // Which kinds to search, and whether to look in every playlist (Windows advanced search).
        val st = vm.settings
        @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
        Row(Modifier.padding(bottom = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ToggleChip("Live TV", st.searchLive, Modifier.focusRequester(chips)) { vm.changeSearchFilter { searchLive = !searchLive } }
            ToggleChip("Movies", st.searchMovies) { vm.changeSearchFilter { searchMovies = !searchMovies } }
            ToggleChip("Series", st.searchSeries) { vm.changeSearchFilter { searchSeries = !searchSeries } }
            if (vm.sources.size > 1) {
                ToggleChip("All playlists", st.searchAllSources) { vm.changeSearchFilter { searchAllSources = !searchAllSources } }
            }
        }
        return
    }
    val recent = vm.section == Section.Recent
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            SectionTitle(
                vm.section.label,
                if (recent) "${vm.items.size} items · hold OK to remove one" else "${vm.items.size} items · hold OK to remove a favorite",
            )
        }
        if (recent && vm.items.isNotEmpty()) ToggleChip("Clear history", false) { vm.clearHistoryHere() }
    }
    Row(Modifier.padding(bottom = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        ToggleChip("All", vm.pageKind == null) { vm.changePageKind(null) }
        ToggleChip("Live TV", vm.pageKind == "live") { vm.changePageKind("live") }
        ToggleChip("Movies and series", vm.pageKind == "vod") { vm.changePageKind("vod") }
    }
}

/** The search box while it is not being typed in: looks like the text field, OK opens it. */
@Composable
private fun SearchBar(query: String, modifier: Modifier, onOpen: () -> Unit) {
    androidx.tv.material3.Surface(
        onClick = onOpen,
        modifier = modifier.height(64.dp),
        shape = androidx.tv.material3.ClickableSurfaceDefaults.shape(androidx.compose.foundation.shape.RoundedCornerShape(6.dp)),
        colors = androidx.tv.material3.ClickableSurfaceDefaults.colors(
            containerColor = Color.Transparent,
            focusedContainerColor = Color.Transparent,
            contentColor = TextColor,
            focusedContentColor = TextColor,
        ),
        scale = androidx.tv.material3.ClickableSurfaceDefaults.scale(focusedScale = 1f),
        border = androidx.tv.material3.ClickableSurfaceDefaults.border(
            border = androidx.tv.material3.Border(androidx.compose.foundation.BorderStroke(1.dp, Muted)),
            focusedBorder = androidx.tv.material3.Border(androidx.compose.foundation.BorderStroke(2.dp, Accent)),
        ),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Search, contentDescription = null, tint = Muted, modifier = Modifier.size(22.dp))
            Text(
                query.ifBlank { "Search channels, movies, series" },
                color = if (query.isBlank()) Muted else TextColor,
                fontSize = 16.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(start = 12.dp),
            )
        }
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ItemList(vm: MainViewModel) {
    if (vm.items.isEmpty()) {
        if (vm.busy == null) {
            EmptyNote(
                when (vm.section) {
                    Section.Favorites -> "No favorites yet. Hold OK on a channel or film to add one."
                    Section.Recent -> "Nothing watched yet."
                    Section.Search -> if (vm.query.isBlank()) "Type to search every playlist." else "No results."
                    else -> "Nothing here."
                },
            )
        }
        return
    }
    if (vm.section == Section.Search) {
        SearchCards(vm)
        return
    }
    val now = System.currentTimeMillis() / 1000
    val strip = vm.settings.stripPrefixes
    // The "All" filter groups results by kind under a heading (Windows pages do the same).
    val groups: List<Pair<String, List<Item>>> = listOf(
        "Live TV" to vm.items.filter { it.isLive },
        "Movies" to vm.items.filter { it.kind == "movie" },
        "Series" to vm.items.filter { it.isSeries },
        "Episodes" to vm.items.filter { it.isEpisode },
    ).filter { it.second.isNotEmpty() }
    LazyColumn(Modifier.focusRestorer(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        for ((label, list) in groups) {
            if (groups.size > 1) {
                item(key = "header:$label") {
                    Text("$label  ${list.size}", color = Accent, fontSize = 14.sp, modifier = Modifier.padding(top = 10.dp, bottom = 2.dp))
                }
            }
            itemsIndexed(list, key = { _, it -> it.id }) { _, item ->
                ItemRow(vm, item, now, strip, onFocused = {})
            }
        }
    }
}

/** Search results as rows of cards, like Home: channel cards, then posters for each kind. */
@Composable
private fun SearchCards(vm: MainViewModel) {
    val live = vm.items.filter { it.isLive }
    val movies = vm.items.filter { it.kind == "movie" }
    val series = vm.items.filter { it.isSeries }
    val episodes = vm.items.filter { it.isEpisode }
    LazyColumn(
        Modifier.focusRestorer(),
        contentPadding = PaddingValues(top = 6.dp, bottom = 40.dp),
        verticalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        if (live.isNotEmpty()) item(key = "live") { ChannelRow(vm, "Live TV", live) }
        if (movies.isNotEmpty()) item(key = "movies") { PosterRow(vm, "Movies", movies) }
        if (series.isNotEmpty()) item(key = "series") { PosterRow(vm, "Series", series) }
        if (episodes.isNotEmpty()) item(key = "episodes") { PosterRow(vm, "Episodes", episodes) }
    }
}

@Composable
private fun ItemRow(vm: MainViewModel, item: Item, now: Long, strip: Boolean, onFocused: () -> Unit) {
    val nn = vm.nowNext[item.id]
    ListItem(
        selected = false,
        onClick = { vm.open(item) },
        onLongClick = { if (vm.section == Section.Recent) vm.removeFromHistory(item) else vm.toggleFavorite(item) },
        modifier = Modifier.onFocusChanged { if (it.isFocused) onFocused() },
        leadingContent = { if (item.isLive) Logo(item, 64.dp, 40.dp, strip) else Logo(item, 44.dp, 64.dp) },
        headlineContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(item.displayName(strip), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                if (vm.isFavorite(item)) Icon(Icons.Filled.Star, null, tint = Color(0xFFFFC94D), modifier = Modifier.padding(start = 6.dp).size(14.dp))
            }
        },
        supportingContent = {
            val cur = nn?.now
            val meta = listOfNotNull(if (item.isEpisode) item.seasonEpisode else null, item.year, item.ratingLabel?.let { "★ $it" })
                .joinToString("  ·  ")
            when {
                item.isLive && cur != null -> Column {
                    Text(cur.title, color = RowMuted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    ProgressBar(cur.progress(now), width = 220.dp)
                }
                !item.plot.isNullOrBlank() -> Column {
                    if (meta.isNotEmpty()) Text(meta, color = RowAccent, fontSize = 12.sp)
                    Text(item.plot, color = RowMuted, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                meta.isNotEmpty() -> Text(meta, color = RowMuted, fontSize = 12.sp)
                else -> Text(item.group, color = RowMuted, fontSize = 12.sp, maxLines = 1)
            }
        },
    )
}
