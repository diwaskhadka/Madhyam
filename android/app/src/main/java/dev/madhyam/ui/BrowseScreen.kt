package dev.madhyam.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
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

/** Movies and Series (categories + list), Favorites, Recent and Search. */
@Composable
fun BrowseScreen(vm: MainViewModel) {
    BackHandler(enabled = vm.openSeries != null) { vm.closeSeries() }
    Row(Modifier.fillMaxSize()) {
        if (vm.section.kind != null && vm.openSeries == null) CategoryPanel(vm, Modifier.width(230.dp))
        Column(Modifier.weight(1f).fillMaxHeight().padding(horizontal = 24.dp, vertical = 16.dp)) {
            Header(vm)
            ItemList(vm)
        }
    }
}

@Composable
private fun Header(vm: MainViewModel) {
    if (vm.section == Section.Search) {
        OutlinedTextField(
            value = vm.query,
            onValueChange = vm::search,
            label = { androidx.compose.material3.Text("Search channels, movies, series") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp),
        )
        return
    }
    val title = vm.openSeries?.name ?: when {
        vm.section.kind != null -> vm.group ?: vm.categories.firstOrNull()?.name ?: vm.section.label
        else -> vm.section.label
    }
    val sub = when {
        vm.openSeries != null -> "${vm.items.size} episodes · Back to return"
        vm.section.kind != null -> "${vm.total} titles · hold OK for favorite"
        else -> "Hold OK to add or remove a favorite"
    }
    SectionTitle(title, sub)
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
    val now = System.currentTimeMillis() / 1000
    val strip = vm.settings.stripPrefixes
    LazyColumn(Modifier.focusRestorer(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        itemsIndexed(vm.items, key = { _, it -> it.id }) { i, item ->
            ItemRow(
                vm,
                item,
                now,
                strip,
                onFocused = { if (vm.section.kind != null && i > vm.items.size - 30) vm.loadMore() },
            )
        }
    }
}

@Composable
private fun ItemRow(vm: MainViewModel, item: Item, now: Long, strip: Boolean, onFocused: () -> Unit) {
    val nn = vm.nowNext[item.id]
    ListItem(
        selected = false,
        onClick = { vm.open(item) },
        onLongClick = { vm.toggleFavorite(item) },
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
            val meta = listOfNotNull(item.year, item.rating?.let { "★ $it" }).joinToString("  ·  ")
            when {
                item.isLive && cur != null -> Column {
                    Text(cur.title, color = Muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    ProgressBar(cur.progress(now), width = 220.dp)
                }
                !item.plot.isNullOrBlank() -> Column {
                    if (meta.isNotEmpty()) Text(meta, color = Accent, fontSize = 12.sp)
                    Text(item.plot, color = Muted, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                meta.isNotEmpty() -> Text(meta, color = Muted, fontSize = 12.sp)
                else -> Text(item.group, color = Muted, fontSize = 12.sp, maxLines = 1)
            }
        },
    )
}
