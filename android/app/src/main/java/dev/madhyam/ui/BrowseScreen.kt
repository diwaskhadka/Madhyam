package dev.madhyam.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.ListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Text
import coil.compose.AsyncImage
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.NowNext
import dev.madhyam.Section
import kotlinx.coroutines.delay

@Composable
fun BrowseScreen(vm: MainViewModel) {
    BackHandler(enabled = vm.openSeries != null) { vm.closeSeries() }
    LaunchedEffect(vm.message) {
        if (vm.message != null) {
            delay(6000)
            vm.dismissMessage()
        }
    }

    Row(Modifier.fillMaxSize()) {
        SideMenu(vm)
        if (vm.section.kind != null && vm.openSeries == null) {
            GroupList(vm)
        }
        Column(Modifier.weight(1f).fillMaxHeight().padding(horizontal = 24.dp, vertical = 20.dp)) {
            Header(vm)
            vm.busy?.let { Text(it, color = Accent, modifier = Modifier.padding(vertical = 4.dp)) }
            vm.message?.let { Text(it, color = Color(0xFFFF7A7A), modifier = Modifier.padding(vertical = 4.dp)) }
            ItemList(vm)
        }
    }
}

@Composable
private fun SideMenu(vm: MainViewModel) {
    Column(
        Modifier.width(240.dp).fillMaxHeight().background(Panel).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text("Madhyam", fontSize = 24.sp, color = Accent)
        Text(vm.source?.name ?: "", color = Muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(12.dp))
        for (s in Section.entries) {
            ListItem(
                selected = vm.section == s,
                onClick = { vm.selectSection(s) },
                headlineContent = { Text(s.label) },
            )
        }
        Spacer(Modifier.weight(1f))
        if (vm.sources.size > 1) {
            OutlinedButton(onClick = {
                val i = vm.sources.indexOfFirst { it.id == vm.source?.id }
                vm.selectSource(vm.sources[(i + 1) % vm.sources.size])
            }) { Text("Next source", fontSize = 13.sp) }
        }
        OutlinedButton(onClick = { vm.refreshSource() }, enabled = vm.busy == null) { Text("Refresh", fontSize = 13.sp) }
        OutlinedButton(onClick = { vm.showSetup() }) { Text("Add source", fontSize = 13.sp) }
        OutlinedButton(onClick = { vm.removeSource() }, enabled = vm.busy == null) { Text("Remove source", fontSize = 13.sp) }
    }
}

@Composable
private fun GroupList(vm: MainViewModel) {
    LazyColumn(
        Modifier.width(280.dp).fillMaxHeight().background(Panel.copy(alpha = 0.6f)).padding(vertical = 16.dp, horizontal = 8.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        items(vm.groups, key = { it.name }) { g ->
            ListItem(
                selected = vm.group == g.name,
                onClick = { vm.selectGroup(g.name) },
                // Moving focus over a category shows its channels, as on most TV apps.
                modifier = Modifier.onFocusChanged { if (it.isFocused) vm.selectGroup(g.name) },
                headlineContent = { Text(g.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                trailingContent = { Text("${g.count}", color = Muted, fontSize = 12.sp) },
            )
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
            modifier = Modifier.fillMaxWidth(),
        )
    } else {
        val title = vm.openSeries?.name ?: vm.group ?: vm.section.label
        Text(title, fontSize = 26.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(
            if (vm.openSeries != null) "Back to return to the series list" else "Hold OK on an item to add or remove a favorite",
            color = Muted,
            fontSize = 12.sp,
        )
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun ItemList(vm: MainViewModel) {
    val now = System.currentTimeMillis() / 1000
    if (vm.items.isEmpty() && vm.busy == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(
                when (vm.section) {
                    Section.Favorites -> "No favorites yet."
                    Section.Recent -> "Nothing watched yet."
                    Section.Search -> if (vm.query.isBlank()) "Type to search." else "No results."
                    else -> "Nothing here."
                },
                color = Muted,
            )
        }
        return
    }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        items(vm.items, key = { it.id }) { item ->
            ItemRow(item, vm.nowNext[item.id], now, onClick = { vm.open(item) }, onLongClick = { vm.toggleFavorite(item) })
        }
    }
}

@Composable
private fun ItemRow(item: Item, nn: NowNext?, now: Long, onClick: () -> Unit, onLongClick: () -> Unit) {
    ListItem(
        selected = false,
        onClick = onClick,
        onLongClick = onLongClick,
        leadingContent = { Logo(item) },
        headlineContent = { Text(item.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            val cur = nn?.now
            when {
                item.isLive && cur != null -> Column {
                    Text(cur.title, color = Muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    ProgressBar(cur.progress(now))
                    nn.next?.let { Text("Next: ${it.title}", color = Muted, fontSize = 11.sp, maxLines = 1) }
                }
                !item.plot.isNullOrBlank() -> Text(item.plot, color = Muted, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                else -> Text(item.group, color = Muted, fontSize = 12.sp, maxLines = 1)
            }
        },
    )
}

@Composable
private fun Logo(item: Item) {
    val m = Modifier.size(width = 64.dp, height = 44.dp).clip(RoundedCornerShape(6.dp))
    if (item.logo.isBlank()) {
        Box(m.background(Accent.copy(alpha = 0.25f)), contentAlignment = Alignment.Center) {
            Text(item.name.take(2).uppercase(), fontSize = 14.sp)
        }
    } else {
        AsyncImage(model = item.logo, contentDescription = null, contentScale = ContentScale.Fit, modifier = m)
    }
}

@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    Box(modifier.padding(vertical = 3.dp).width(220.dp).height(3.dp).background(Muted.copy(alpha = 0.3f))) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction).background(Accent))
    }
}
