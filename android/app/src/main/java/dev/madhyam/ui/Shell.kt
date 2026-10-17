package dev.madhyam.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.LiveTv
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.automirrored.filled.PlaylistPlay
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.VideoLibrary
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.DenseListItem
import androidx.tv.material3.Icon
import androidx.tv.material3.ListItem
import androidx.tv.material3.Text
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

private val RailCollapsed = 72.dp
private val RailExpanded = 230.dp

fun Section.icon(): ImageVector = when (this) {
    Section.Home -> Icons.Filled.Home
    Section.Live -> Icons.Filled.LiveTv
    Section.Movies -> Icons.Filled.Movie
    Section.Series -> Icons.Filled.VideoLibrary
    Section.Favorites -> Icons.Filled.Star
    Section.Recent -> Icons.Filled.History
    Section.Search -> Icons.Filled.Search
    Section.Settings -> Icons.Filled.Settings
}

/**
 * The main page: a navigation rail on the left that widens to show labels while it has the
 * focus (drawn over the page, so the page never jumps), and the selected section beside it.
 */
@Composable
fun MainShell(vm: MainViewModel) {
    var railFocused by remember { mutableStateOf(false) }
    val railFocus = remember { FocusRequester() }
    var pickPlaylist by remember { mutableStateOf(false) }

    LaunchedEffect(vm.message) {
        if (vm.message != null) {
            delay(5000)
            vm.dismissMessage()
        }
    }

    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().padding(start = RailCollapsed)) {
            CompositionLocalProvider(LocalRailFocus provides railFocus) {
            when (vm.section) {
                Section.Home -> if (vm.details != null) DetailsScreen(vm) else HomeScreen(vm)
                Section.Live -> LiveScreen(vm)
                Section.Settings -> SettingsScreen(vm)
                else -> BrowseScreen(vm)
            }
            }
        }
        Rail(vm, railFocus, onFocus = { railFocused = it }, expanded = railFocused, onPlaylists = { pickPlaylist = true })

        // Status line (busy / messages), bottom right, over everything.
        val status = vm.busy ?: vm.message
        if (status != null) {
            Box(
                Modifier.align(androidx.compose.ui.Alignment.BottomEnd).padding(24.dp)
                    .background(Panel.copy(alpha = 0.95f), RoundedCornerShape(10.dp))
                    .padding(horizontal = 18.dp, vertical = 12.dp),
            ) {
                Text(status, color = if (vm.busy != null) Accent else Color.White, fontSize = 15.sp)
            }
        }
    }

    if (pickPlaylist) PlaylistDialog(vm) { pickPlaylist = false }
}

@Suppress("DEPRECATION") // FocusProperties.enter
@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun Rail(vm: MainViewModel, selected: FocusRequester, onFocus: (Boolean) -> Unit, expanded: Boolean, onPlaylists: () -> Unit) {
    val width by animateDpAsState(if (expanded) RailExpanded else RailCollapsed, label = "rail")
    Column(
        Modifier
            .width(width)
            .fillMaxHeight()
            .background(if (expanded) Panel else Background)
            .onFocusChanged { onFocus(it.hasFocus) }
            // Entering the rail always lands on the current page, not the nearest entry.
            .focusProperties { enter = { selected } }
            .focusGroup()
            .padding(horizontal = 10.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        RailEntry(
            icon = Icons.AutoMirrored.Filled.PlaylistPlay,
            label = vm.source?.name ?: "Playlists",
            sub = if (vm.sources.size > 1) "Switch playlist" else "Playlists",
            expanded = expanded,
            selected = false,
            onClick = onPlaylists,
        )
        Spacer(Modifier.height(10.dp))
        for (s in Section.entries) {
            if (s == Section.Settings) Spacer(Modifier.weight(1f))
            RailEntry(
                icon = s.icon(),
                label = s.label,
                expanded = expanded,
                selected = vm.section == s,
                modifier = if (vm.section == s) Modifier.focusRequester(selected) else Modifier,
                onClick = { if (vm.section != s) vm.selectSection(s) },
            )
        }
    }
}

@Composable
private fun RailEntry(
    icon: ImageVector,
    label: String,
    expanded: Boolean,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    sub: String? = null,
) {
    // Dense: nine entries have to fit a 540 dp tall TV screen.
    DenseListItem(
        selected = selected,
        onClick = onClick,
        modifier = modifier,
        leadingContent = { Icon(icon, contentDescription = label, modifier = Modifier.size(24.dp)) },
        headlineContent = {
            if (expanded) Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        supportingContent = if (expanded && sub != null) {
            { Text(sub, fontSize = 11.sp, color = Muted, maxLines = 1) }
        } else {
            null
        },
    )
}

/** Choose the playlist the pages show, or add another one. */
@Composable
private fun PlaylistDialog(vm: MainViewModel, onDismiss: () -> Unit) {
    // Start on the current playlist, and ignore the key-up of the OK that opened the dialog.
    val current = remember { FocusRequester() }
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(80)
        runCatching { current.requestFocus() }
    }
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier.width(460.dp).background(Panel, RoundedCornerShape(16.dp)).padding(20.dp)
                .onPreviewKeyEvent { if (it.type == KeyEventType.KeyDown) armed = true; false },
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("Playlists", fontSize = 22.sp, modifier = Modifier.padding(bottom = 8.dp))
            LazyColumn(Modifier.fillMaxWidth().height((vm.sources.size.coerceAtMost(6) * 64).dp)) {
                items(vm.sources, key = { it.id }) { s ->
                    ListItem(
                        selected = s.id == vm.source?.id,
                        onClick = {
                            if (!armed) return@ListItem
                            vm.selectSource(s)
                            onDismiss()
                        },
                        modifier = if (s.id == vm.source?.id) Modifier.focusRequester(current) else Modifier,
                        headlineContent = { Text(s.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = {
                            Text(
                                when {
                                    s.isFreeTv -> "Free TV · iptv-org"
                                    s.isXtream -> "Xtream Codes · ${s.username}"
                                    else -> "M3U playlist"
                                },
                                fontSize = 12.sp,
                                color = Muted,
                            )
                        },
                    )
                }
            }
            ListItem(
                selected = false,
                onClick = {
                    onDismiss()
                    vm.showSetup()
                },
                leadingContent = { Icon(Icons.Filled.Add, contentDescription = null) },
                headlineContent = { Text("Add playlist") },
            )
        }
    }
}

@Composable
fun SectionTitle(title: String, sub: String? = null) {
    Column(Modifier.padding(bottom = 10.dp)) {
        Text(title, fontSize = 24.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (sub != null) Text(sub, color = Muted, fontSize = 12.sp, maxLines = 1)
    }
}

@Composable
fun EmptyNote(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = androidx.compose.ui.Alignment.Center) {
        Text(text, color = Muted)
    }
}

/** The rail's entry point, so a page can send Left from its first control straight to the rail. */
val LocalRailFocus = androidx.compose.runtime.staticCompositionLocalOf { FocusRequester.Default }
