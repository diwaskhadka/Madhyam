package dev.madhyam.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Button
import androidx.tv.material3.ListItem
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Switch
import androidx.tv.material3.Text
import dev.madhyam.Account
import dev.madhyam.FreeTv
import dev.madhyam.Group
import dev.madhyam.MainViewModel
import dev.madhyam.Source

private enum class Page(val label: String) {
    Playlists("Playlists"),
    FreeTv("Free TV"),
    Live("Live TV"),
    Categories("Categories"),
    Playback("Playback"),
    Guide("Guide"),
    About("About"),
}

/**
 * Settings (same style as Windows): sections on the left, rows of
 * "title + description … value" on the right. OK on a row toggles or cycles its value.
 */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun SettingsScreen(vm: MainViewModel) {
    var page by remember { mutableStateOf(Page.Playlists) }
    var openSource by remember { mutableStateOf<Source?>(null) }
    var editing by remember { mutableStateOf(false) }
    // Back from a playlist's page: focus its row again (the page that had the focus is gone).
    val returnTo = remember { FocusRequester() }
    var lastOpened by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(openSource) {
        if (openSource == null && lastOpened != null) {
            kotlinx.coroutines.delay(60)
            runCatching { returnTo.requestFocus() }
        }
        lastOpened = openSource?.id ?: lastOpened
    }
    val currentPage = remember { FocusRequester() }
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion // recompose when settings change

    BackHandler(enabled = openSource != null) {
        if (editing) editing = false else openSource = null
    }

    Row(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.width(220.dp).fillMaxHeight().background(Panel.copy(alpha = 0.55f)).padding(vertical = 16.dp, horizontal = 8.dp)
                .focusRestorer(currentPage), // first entry lands on the open page, not the nearest row
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            item { Text("Settings", fontSize = 24.sp, modifier = Modifier.padding(start = 12.dp, bottom = 12.dp)) }
            items(Page.entries) { p ->
                ListItem(
                    selected = page == p,
                    onClick = { page = p },
                    modifier = (if (page == p) Modifier.focusRequester(currentPage) else Modifier).onFocusChanged {
                        if (it.isFocused && page != p) {
                            page = p
                            openSource = null
                            editing = false
                        }
                    },
                    headlineContent = { Text(p.label) },
                )
            }
        }
        Column(Modifier.weight(1f).fillMaxHeight().padding(horizontal = 28.dp, vertical = 16.dp)) {
            val src = openSource?.let { o -> vm.sources.firstOrNull { it.id == o.id } }
            when {
                page == Page.Playlists && src != null && editing -> EditSource(vm, src) { editing = false }
                page == Page.Playlists && src != null -> SourceDetails(vm, src, onEdit = { editing = true }, onRemoved = { openSource = null })
                else -> LazyColumn(Modifier.focusRestorer(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    when (page) {
                        Page.Playlists -> playlists(vm, returnTo, lastOpened) { openSource = it }
                        Page.FreeTv -> freeTv(vm)
                        Page.Live -> live(vm)
                        Page.Categories -> item { CategoriesPage(vm) }
                        Page.Playback -> playback(vm)
                        Page.Guide -> guide(vm)
                        Page.About -> about(vm)
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------- rows

@Composable
private fun SwitchRow(title: String, desc: String, on: Boolean, onToggle: (Boolean) -> Unit) {
    ListItem(
        selected = false,
        onClick = { onToggle(!on) },
        headlineContent = { Text(title) },
        supportingContent = { Text(desc, color = Muted, fontSize = 12.sp) },
        trailingContent = { Switch(checked = on, onCheckedChange = null) },
    )
}

/** OK cycles through [options] (value → label). */
@Composable
private fun <T> ChoiceRow(title: String, desc: String, value: T, options: List<Pair<T, String>>, onPick: (T) -> Unit) {
    val i = options.indexOfFirst { it.first == value }.coerceAtLeast(0)
    ListItem(
        selected = false,
        onClick = { onPick(options[(i + 1) % options.size].first) },
        headlineContent = { Text(title) },
        supportingContent = { Text(desc, color = Muted, fontSize = 12.sp) },
        trailingContent = { Text(options[i].second, color = Accent) },
    )
}

@Composable
private fun ActionRow(title: String, desc: String? = null, value: String? = null, modifier: Modifier = Modifier, onClick: () -> Unit) {
    ListItem(
        selected = false,
        onClick = onClick,
        modifier = modifier,
        headlineContent = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = desc?.let { { Text(it, color = Muted, fontSize = 12.sp, maxLines = 2) } },
        trailingContent = value?.let { { Text(it, color = Muted, fontSize = 13.sp) } },
    )
}

private fun Source.describe(): String = when {
    isFreeTv -> "Free TV · iptv-org"
    isXtream -> "Xtream Codes · $username"
    else -> "M3U · $location"
}

// ------------------------------------------------------------------ pages

private fun LazyListScope.playlists(vm: MainViewModel, returnTo: FocusRequester, lastOpened: Int?, open: (Source) -> Unit) {
    item { SectionTitle("Playlists", "OK on a playlist to refresh, edit or remove it") }
    items(vm.sources, key = { it.id }) { s ->
        ActionRow(
            s.name, s.describe(), if (s.updatedAt > 0) "Updated ${date(s.updatedAt)}" else null,
            modifier = if (s.id == lastOpened) Modifier.focusRequester(returnTo) else Modifier,
        ) { open(s) }
    }
    item { ActionRow("Add playlist", "Xtream Codes login or M3U playlist URL") { vm.showSetup() } }
}

@Composable
private fun SourceDetails(vm: MainViewModel, s: Source, onEdit: () -> Unit, onRemoved: () -> Unit) {
    var account by remember(s.id) { mutableStateOf<Account?>(null) }
    var accountError by remember(s.id) { mutableStateOf<String?>(null) }
    var confirmRemove by remember(s.id) { mutableStateOf(false) }
    // The row that opened this page is gone: put the focus on the first action, not back on the rail.
    val first = remember { FocusRequester() }
    LaunchedEffect(s.id) {
        kotlinx.coroutines.delay(60)
        runCatching { first.requestFocus() }
    }
    if (s.isXtream) {
        LaunchedEffect(s.id) {
            try {
                account = vm.account(s)
            } catch (e: Exception) {
                accountError = e.message ?: "Account information unavailable."
            }
        }
    }
    LazyColumn(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item { SectionTitle(s.name, s.describe()) }
        if (s.isXtream) {
            item {
                val a = account
                Text(
                    when {
                        a != null -> buildString {
                            append("Account ${a.status.ifBlank { "unknown" }}")
                            if (a.isTrial) append(" (trial)")
                            a.expires?.let { append(" · expires ${date(it)}") }
                            if (a.maxConnections > 0) append(" · connections ${a.activeConnections} / ${a.maxConnections}")
                        }
                        accountError != null -> accountError!!
                        else -> "Checking the account…"
                    },
                    color = if (accountError != null) ErrorRed else Muted,
                    fontSize = 13.sp,
                    modifier = Modifier.padding(start = 12.dp, bottom = 8.dp),
                )
            }
        }
        item { ActionRow("Refresh playlist", "Download channels, films and series again", modifier = Modifier.focusRequester(first)) { vm.refreshSource(s) } }
        item { ActionRow("Update guide", "Download the programme guide (EPG) again") { vm.refreshGuide(s) } }
        if (!s.isFreeTv) item { ActionRow("Edit", "Name, address and login") { onEdit() } }
        item {
            ActionRow(
                if (confirmRemove) "Press OK again to remove ${s.name}" else "Remove playlist",
                "Removes its channels, favorites stay for when it comes back",
            ) {
                if (confirmRemove) {
                    vm.removeSource(s)
                    onRemoved()
                } else {
                    confirmRemove = true
                }
            }
        }
    }
}

@Composable
private fun EditSource(vm: MainViewModel, s: Source, onDone: () -> Unit) {
    var name by remember(s.id) { mutableStateOf(s.name) }
    var location by remember(s.id) { mutableStateOf(s.location) }
    var server by remember(s.id) { mutableStateOf(s.location) }
    var user by remember(s.id) { mutableStateOf(s.username) }
    var pass by remember(s.id) { mutableStateOf("") }
    // Focus the Save button rather than a field (a focused field would open the keyboard at once).
    val save = remember { FocusRequester() }
    LaunchedEffect(s.id) {
        kotlinx.coroutines.delay(60)
        runCatching { save.requestFocus() }
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SectionTitle("Edit ${s.name}", "Leave the password empty to keep the current one")
        Field("Name", name) { name = it }
        if (s.isXtream) {
            Field("Server", server, KeyboardType.Uri) { server = it }
            Field("Username", user) { user = it }
            Field("Password (unchanged if empty)", pass, KeyboardType.Password, secret = true) { pass = it }
        } else {
            Field("Playlist URL", location, KeyboardType.Uri) { location = it }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(modifier = Modifier.focusRequester(save), onClick = {
                if (s.isXtream) vm.updateSource(s, name, null, server, user, pass) else vm.updateSource(s, name, location, null, null, null)
                onDone()
            }) { Text("Save and refresh") }
            OutlinedButton(onClick = onDone) { Text("Cancel") }
        }
    }
}

@Composable
fun Field(label: String, value: String, type: KeyboardType = KeyboardType.Text, secret: Boolean = false, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { androidx.compose.material3.Text(label) },
        singleLine = true,
        visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = type, imeAction = ImeAction.Next),
        modifier = Modifier.width(560.dp),
    )
}

private fun LazyListScope.freeTv(vm: MainViewModel) {
    val on = vm.sources.any { it.isFreeTv }
    item {
        SectionTitle("Free TV", "Public channels collected by iptv-org (github.com/iptv-org/iptv)")
    }
    item {
        SwitchRow("Free TV channels", "Adds a \"${FreeTv.NAME}\" playlist with thousands of free channels", on) { vm.setFreeTv(it) }
    }
    if (on) {
        item {
            ChoiceRow(
                "Group channels by", "Also switchable at the top of the category list in Live TV",
                vm.settings.freeTvGroupBy, listOf("category" to "Categories", "country" to "Countries"),
            ) { by ->
                vm.changeSettings { freeTvGroupBy = by }
                if (vm.source?.isFreeTv == true) vm.groupLiveBy(by)
            }
        }
        item { Text("To hide countries, open Settings › Categories and choose \"Countries\".", color = Muted, fontSize = 12.sp, modifier = Modifier.padding(12.dp)) }
    }
}

private fun LazyListScope.live(vm: MainViewModel) {
    val st = vm.settings
    item { SectionTitle("Live TV") }
    item {
        SwitchRow("Play in the preview first", "OK plays a channel in the small window; OK again goes full screen. Off: straight to full screen", st.previewOnSelect) {
            vm.changeSettings { previewOnSelect = it }
        }
    }
    item { SwitchRow("Channel numbers", "Show the position of each channel in the list", st.showNumbers) { vm.changeSettings { showNumbers = it } } }
    item {
        SwitchRow("Sort channels A–Z", "Off keeps the provider's order", st.liveSortAz) {
            vm.changeSettings { liveSortAz = it }
            vm.selectSection(vm.section)
        }
    }
    item { SwitchRow("Hide name prefixes", "\"UK: BBC One\" becomes \"BBC One\"", st.stripPrefixes) { vm.changeSettings { stripPrefixes = it } } }
    item {
        ChoiceRow(
            "Start in", "The page Madhyam opens with",
            st.startupSection,
            listOf("Last" to "Last page", "Live" to "Live TV", "Movies" to "Movies", "Series" to "Series", "Favorites" to "Favorites", "Recent" to "Recent"),
        ) { vm.changeSettings { startupSection = it } }
    }
}

/** Show or hide the categories of the current playlist. */
@Composable
private fun CategoriesPage(vm: MainViewModel) {
    val s = vm.source ?: run {
        Text("Add a playlist first.", color = Muted)
        return
    }
    val kinds = buildList {
        add("live" to "Live TV")
        add("movie" to "Movies")
        add("series" to "Series")
        if (s.isFreeTv) add("live:country" to "Countries")
    }
    var kind by remember(s.id) { mutableStateOf("live") }
    var groups by remember(s.id, kind) { mutableStateOf<List<Group>>(emptyList()) }
    LaunchedEffect(s.id, kind) {
        groups = if (kind == "live:country") vm.groupsOf(s, "live", FreeTv.COUNTRY_FIELD) else vm.groupsOf(s, kind)
    }
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    val hidden = vm.settings.hidden(s.id, kind)
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        SectionTitle("Categories of ${s.name}", "Hidden categories disappear from the list and from \"All\"")
        ChoiceRow("Show categories of", "OK to switch", kind, kinds) { kind = it }
        if (groups.isEmpty()) Text("No categories.", color = Muted, modifier = Modifier.padding(12.dp))
        for (g in groups) {
            val shown = g.name !in hidden
            SwitchRow(g.name, "${g.count} items", shown) { on ->
                val realKind = if (kind == "live:country") "live:country" else kind
                vm.setHidden(s, realKind, g.name, !on)
            }
        }
    }
}

private fun LazyListScope.playback(vm: MainViewModel) {
    val st = vm.settings
    item { SectionTitle("Playback") }
    item { SwitchRow("Resume films and episodes", "Continue where you stopped", st.resumePlayback) { vm.changeSettings { resumePlayback = it } } }
    item {
        SwitchRow("Subtitles", "Show subtitles when the stream has them", st.showSubtitles) {
            vm.changeSettings { showSubtitles = it }
            vm.playback.applyTrackPreferences()
        }
    }
    item {
        PickRow("Preferred audio language", "Used when a stream has several audio tracks", st.audioLanguage, LANGUAGES) {
            vm.changeSettings { audioLanguage = it }
            vm.playback.applyTrackPreferences()
        }
    }
    item {
        PickRow("Preferred subtitle language", "Used when a stream has several subtitle tracks", st.subtitleLanguage, LANGUAGES) {
            vm.changeSettings { subtitleLanguage = it }
            vm.playback.applyTrackPreferences()
        }
    }
    item {
        Text(
            "During playback press Menu (or OK on live TV, Down on films) for audio, subtitles, speed and picture size.",
            color = Muted, fontSize = 12.sp, modifier = Modifier.padding(12.dp),
        )
    }
}

/** Language choices for the preferred-track settings: ISO 639-1 code → name. */
private val LANGUAGES = listOf(
    "" to "Stream default", "en" to "English", "es" to "Spanish", "fr" to "French", "de" to "German",
    "it" to "Italian", "pt" to "Portuguese", "nl" to "Dutch", "pl" to "Polish", "tr" to "Turkish",
    "ru" to "Russian", "uk" to "Ukrainian", "ar" to "Arabic", "hi" to "Hindi", "ne" to "Nepali",
    "bn" to "Bengali", "ur" to "Urdu", "zh" to "Chinese", "ja" to "Japanese", "ko" to "Korean",
)

private fun LazyListScope.guide(vm: MainViewModel) {
    val st = vm.settings
    item { SectionTitle("Guide (EPG)") }
    item {
        ChoiceRow(
            "Update guides every", "Checked when Madhyam starts",
            st.epgRefreshHours, listOf(6 to "6 hours", 12 to "12 hours", 24 to "24 hours", 48 to "2 days"),
        ) { vm.changeSettings { epgRefreshHours = it } }
    }
    items(vm.sources, key = { it.id }) { s -> ActionRow("Update the guide of ${s.name}") { vm.refreshGuide(s) } }
}

private fun LazyListScope.about(vm: MainViewModel) {
    item { AboutRows(vm) }
}

@Composable
private fun AboutRows(vm: MainViewModel) {
    val ctx = LocalContext.current
    val version = remember {
        try {
            ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName ?: "?"
        } catch (_: Exception) {
            "?"
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        SectionTitle("About Madhyam")
        ActionRow("Version", value = version) {}
        ActionRow("Engine (madhyam-core)", value = vm.engineVersion) {}
        ActionRow("Clear watch history", "Empties the Recent list") { vm.clearHistory() }
        Text(
            "Madhyam plays your own playlists and ships with no channels. Free TV channels come from iptv-org.",
            color = Muted, fontSize = 12.sp, modifier = Modifier.padding(12.dp),
        )
    }
}

/** OK opens a list of [options] to pick from (for long lists such as languages). */
@Composable
private fun <T> PickRow(title: String, desc: String, value: T, options: List<Pair<T, String>>, onPick: (T) -> Unit) {
    var open by remember { mutableStateOf(false) }
    ListItem(
        selected = false,
        onClick = { open = true },
        headlineContent = { Text(title) },
        supportingContent = { Text(desc, color = Muted, fontSize = 12.sp) },
        trailingContent = { Text(options.firstOrNull { it.first == value }?.second ?: value.toString(), color = Accent) },
    )
    if (open) PickerDialog(title, options, value, onPick) { open = false }
}
