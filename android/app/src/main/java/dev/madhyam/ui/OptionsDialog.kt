package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.Category
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

/** The menu that holding OK opens on a channel, film, series, episode or category. */
@Composable
fun OptionsDialog(vm: MainViewModel) {
    val item = vm.optionsItem
    val category = vm.optionsCategory
    when {
        item != null -> OptionsList(item.displayName(vm.settings.stripPrefixes), itemOptions(vm, item), vm::closeOptions)
        category != null -> OptionsList(category.name, categoryOptions(vm, category), vm::closeOptions)
    }
}

private fun itemOptions(vm: MainViewModel, item: Item): List<Pair<String, () -> Unit>> = buildList {
    // An episode can only be a favorite from before; let it be removed.
    if (!item.isEpisode || vm.isFavorite(item)) {
        add((if (vm.isFavorite(item)) "Remove from favorites" else "Add to favorites") to { vm.toggleFavorite(item) })
    }
    when {
        item.isLive -> add("Watch" to { vm.watchFromOptions(item) })
        item.isSeries -> if (vm.details?.id != item.id) add("Open" to { vm.showDetails(item) })
        else -> add((if (item.isStarted) "Resume" else "Play") to { vm.resume(item) })
    }
    if (item.kind == "movie" && vm.details?.id != item.id) add("Details" to { vm.showDetails(item) })
    if (item.kind == "movie" || item.isEpisode) {
        add((if (item.isWatched) "Mark as not watched" else "Mark as watched") to { vm.toggleWatched(item) })
    }
    if (vm.section == Section.Recent && !vm.searchOpen) add("Remove from Recent" to { vm.removeFromHistory(item) })
}

private fun categoryOptions(vm: MainViewModel, c: Category): List<Pair<String, () -> Unit>> = buildList {
    add("Hide category" to { vm.hideCategory(c.name) })
    if (vm.parentalActive) {
        add((if (vm.categoryIsLockedSetting(c.name)) "Unlock category" else "Lock category") to { vm.toggleCategoryLock(c.name) })
    }
}

@Composable
private fun OptionsList(title: String, options: List<Pair<String, () -> Unit>>, onDismiss: () -> Unit) {
    val first = remember { FocusRequester() }
    // Holding OK repeats its key-down and then lifts it in here: only a fresh press may choose.
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { first.requestFocus() }
    }
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier.width(420.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)
                .onPreviewKeyEvent {
                    if (it.type == KeyEventType.KeyDown && it.nativeKeyEvent.repeatCount == 0) armed = true
                    false
                },
        ) {
            Text(title, fontSize = 20.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(bottom = 10.dp))
            options.forEachIndexed { i, (label, action) ->
                ListItem(
                    selected = false,
                    onClick = {
                        if (armed) {
                            onDismiss() // first: the action may open its own dialog (the PIN)
                            action()
                        }
                    },
                    modifier = if (i == 0) Modifier.focusRequester(first) else Modifier,
                    headlineContent = { Text(label, maxLines = 1) },
                )
            }
        }
    }
}
