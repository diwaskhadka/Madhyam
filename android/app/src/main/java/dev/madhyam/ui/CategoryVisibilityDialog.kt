package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Button
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.Group
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay

/**
 * Hold OK on a category › Show and hide categories: every category of the page, hidden ones too.
 * OK ticks; Hide N hides the ticked ones that show, Show N brings back the ticked hidden ones.
 * Up from the first row reaches the buttons; Up from the buttons wraps to the last row.
 */
@Composable
fun CategoryVisibilityDialog(vm: MainViewModel) {
    if (!vm.editingCategories) return
    var groups by remember { mutableStateOf<List<Group>?>(null) }
    val ticked = remember { mutableStateListOf<String>() }
    val firstRow = remember { FocusRequester() }
    val buttons = remember { FocusRequester() }
    val list = rememberLazyListState()
    val wrap = rememberWrap()
    // Opened with OK: the release of that key must not tick the first row.
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        groups = vm.allCategoriesHere()
        delay(80)
        runCatching { firstRow.requestFocus() }
    }
    val version = vm.settingsVersion // re-read the hidden marks when they change
    fun hidden(name: String) = version.let { vm.isCategoryHidden(name) }
    val toHide = ticked.filter { !hidden(it) }
    val toShow = ticked.filter { hidden(it) }
    val rows = groups.orEmpty()

    Dialog(onDismissRequest = { vm.editingCategories = false }) {
        Column(
            Modifier.width(520.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)
                .onPreviewKeyEvent {
                    if (it.type == KeyEventType.KeyDown && it.nativeKeyEvent.repeatCount == 0) armed = true
                    false
                },
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text("Show and hide categories", fontSize = 20.sp)
            Text(
                "OK ticks · ${ticked.size} ticked · Up for the buttons · Back closes",
                color = Muted, fontSize = 12.sp,
            )
            // The buttons are the top of the column: Up from them wraps to the last row.
            Row(
                Modifier.onPreviewKeyEvent { ev ->
                    if (ev.type == KeyEventType.KeyDown && ev.nativeKeyEvent.keyCode == android.view.KeyEvent.KEYCODE_DPAD_UP && rows.isNotEmpty()) {
                        wrap.focus { rows.lastIndex.also { list.scrollToItem(it) } }
                        true
                    } else {
                        false
                    }
                },
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Button(
                    onClick = { if (armed && toHide.isNotEmpty()) vm.applyCategoryChoice(toHide, emptyList()) },
                    modifier = Modifier.focusRequester(buttons),
                ) { Text("Hide ${toHide.size}") }
                Button(onClick = { if (armed && toShow.isNotEmpty()) vm.applyCategoryChoice(emptyList(), toShow) }) {
                    Text("Show ${toShow.size}")
                }
                Button(onClick = { if (armed) vm.editingCategories = false }) { Text("Close") }
            }
            if (groups == null) Text("Loading categories…", color = Muted, fontSize = 14.sp)
            LazyColumn(
                Modifier.heightIn(max = 400.dp).wrapColumn(
                    onTop = { runCatching { buttons.requestFocus() } },
                    onBottom = { wrap.focus { list.scrollToItem(0); 0 } },
                ),
                state = list,
            ) {
                itemsIndexed(rows, key = { _, g -> g.name }) { i, g ->
                    val on = g.name in ticked
                    val isHidden = hidden(g.name)
                    ListItem(
                        selected = false,
                        onClick = {
                            if (armed) {
                                if (on) ticked.remove(g.name) else ticked.add(g.name)
                            }
                        },
                        modifier = (if (i == 0) Modifier.focusRequester(firstRow) else Modifier).then(wrap.at(i)),
                        leadingContent = {
                            Icon(
                                if (on) Icons.Filled.CheckBox else Icons.Filled.CheckBoxOutlineBlank,
                                contentDescription = if (on) "Ticked" else null,
                                modifier = Modifier.size(20.dp),
                            )
                        },
                        headlineContent = {
                            Text(
                                g.name, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp,
                                color = if (isHidden) Muted else Color.Unspecified,
                            )
                        },
                        trailingContent = {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                if (isHidden) Text("Hidden  ", color = Accent, fontSize = 12.sp)
                                Text("${g.count}", color = Muted, fontSize = 12.sp)
                            }
                        },
                    )
                }
            }
        }
    }
}
