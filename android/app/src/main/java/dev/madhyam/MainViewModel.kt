package dev.madhyam

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.madhyam.core.IptvException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** Top-level pages of the app. */
sealed interface Screen {
    data object Loading : Screen
    data object Setup : Screen
    data object Main : Screen
    /** Full-screen video; Back returns to [Screen.Main]. */
    data object Player : Screen
}

/** Entries of the navigation rail. */
enum class Section(val label: String, val kind: String?) {
    Live("Live TV", "live"),
    Movies("Movies", "movie"),
    Series("Series", "series"),
    Favorites("Favorites", null),
    Recent("Recent", null),
    Search("Search", null),
    Settings("Settings", null),
}

/** A row of the category panel; [isAll] is the synthetic "All" entry (group = null). */
data class Category(val name: String, val count: Int, val isAll: Boolean = false)

class MainViewModel(app: Application) : AndroidViewModel(app) {
    private val repo = (app as App).repo
    val settings: AppSettings = repo.settings
    val playback = Playback(app, settings)

    var screen by mutableStateOf<Screen>(Screen.Loading)
        private set
    var sources by mutableStateOf<List<Source>>(emptyList())
        private set
    var source by mutableStateOf<Source?>(null)
        private set
    var section by mutableStateOf(Section.Live)
        private set

    // Category panel and the list beside it.
    var categories by mutableStateOf<List<Category>>(emptyList())
        private set
    /** Selected category name; null = all. */
    var group by mutableStateOf<String?>(null)
        private set
    var items by mutableStateOf<List<Item>>(emptyList())
        private set
    var total by mutableStateOf(0)
        private set
    /** Free TV in Live TV: "category" or "country". */
    var liveGroupBy by mutableStateOf(settings.freeTvGroupBy)
        private set

    /** Non-null while a series' episodes are shown in place of the item list. */
    var openSeries by mutableStateOf<Item?>(null)
        private set
    var query by mutableStateOf("")
        private set

    // Live TV info pane: the channel under the focus.
    var focused by mutableStateOf<Item?>(null)
        private set
    var upcoming by mutableStateOf<List<Programme>>(emptyList())
        private set

    var favorites by mutableStateOf<Set<String>>(emptySet())
        private set
    var busy by mutableStateOf<String?>(null)
        private set
    var message by mutableStateOf<String?>(null)
        private set
    /** Bumped whenever settings change, so pages that read [settings] recompose. */
    var settingsVersion by mutableStateOf(0)
        private set
    val nowNext = mutableStateMapOf<String, NowNext>()

    val engineVersion: String get() = repo.version

    private var itemsJob: Job? = null
    private var searchJob: Job? = null
    private var focusJob: Job? = null
    private var loadingMore = false

    init {
        viewModelScope.launch {
            repo.init()
            sources = repo.sources()
            favorites = repo.favoriteIds()
            if (sources.isEmpty()) {
                screen = Screen.Setup
            } else {
                screen = Screen.Main
                val start = settings.lastSourceId?.let { id -> sources.firstOrNull { it.id == id } } ?: sources.first()
                section = startSection()
                selectSource(start)
            }
            refreshStaleGuides()
        }
        // Keep "now playing" fresh while the app is open.
        viewModelScope.launch {
            while (isActive) {
                delay(60_000)
                refreshNowNext()
            }
        }
    }

    private fun startSection(): Section {
        val name = if (settings.startupSection == "Last") settings.lastSection else settings.startupSection
        return Section.entries.firstOrNull { it.name == name && it != Section.Settings && it != Section.Search } ?: Section.Live
    }

    override fun onCleared() {
        playback.release()
    }

    /** Re-download guides older than the configured age in the background; never blocks watching. */
    private suspend fun refreshStaleGuides() {
        for (s in sources) {
            try {
                val age = repo.epgAgeMs(s.id)
                if (age != null && age < settings.epgRefreshHours * 3600_000L) continue
                if (!repo.hasEpg(s.id)) continue
                repo.refreshEpg(s.id)
                refreshNowNext()
            } catch (e: Exception) {
                android.util.Log.w(Repository.TAG, "background guide refresh failed for ${s.name}", e)
            }
        }
    }

    private fun explain(e: Throwable): String = when (e) {
        is IptvException -> when (e.code) {
            "api" -> "The server refused the request: ${e.message?.substringAfter(": ")}"
            "parse" -> "The server's answer could not be read: ${e.message?.substringAfter(": ")}"
            else -> e.message ?: e.code
        }
        is java.net.UnknownHostException -> "Server not found. Check the address and your connection."
        is java.net.SocketTimeoutException -> "The server took too long to answer."
        is java.io.IOException -> "Network error: ${e.message}"
        else -> e.message ?: e.javaClass.simpleName
    }

    /** Run a long task with a status line; errors become a readable message. */
    private fun task(label: String, block: suspend () -> Unit) {
        viewModelScope.launch {
            busy = label
            message = null
            try {
                block()
            } catch (e: Exception) {
                message = explain(e)
            } finally {
                busy = null
            }
        }
    }

    fun dismissMessage() {
        message = null
    }

    fun toast(text: String) {
        message = text
    }

    fun changeSettings(block: AppSettings.() -> Unit) {
        settings.block()
        settings.save()
        settingsVersion++
    }

    // ------------------------------------------------------------- sources

    fun showSetup() {
        screen = Screen.Setup
    }

    fun cancelSetup() {
        if (sources.isNotEmpty()) screen = Screen.Main
    }

    fun addM3u(name: String, url: String) = task("Downloading playlist…") {
        afterAdd(repo.addM3u(name.ifBlank { "Playlist" }, url.trim()))
    }

    fun addXtream(name: String, server: String, user: String, pass: String) = task("Logging in and downloading the catalog…") {
        afterAdd(repo.addXtream(name.ifBlank { "Provider" }, server.trim(), user.trim(), pass))
    }

    fun addFreeTv() = task("Downloading Free TV channels…") {
        val id = repo.addFreeTv()
        changeSettings { freeTvSourceId = id }
        afterAdd(id)
    }

    private suspend fun afterAdd(id: Int) {
        sources = repo.sources()
        screen = Screen.Main
        sources.firstOrNull { it.id == id }?.let {
            section = Section.Live
            selectSource(it)
        }
    }

    fun selectSource(s: Source) {
        source = s
        changeSettings { lastSourceId = s.id }
        liveGroupBy = if (s.isFreeTv) settings.freeTvGroupBy else "category"
        selectSection(if (section == Section.Search || section == Section.Settings) Section.Live else section)
    }

    fun refreshSource(s: Source) = task("Refreshing ${s.name}…") {
        repo.refresh(s.id)
        sources = repo.sources()
        if (s.id == source?.id) reloadSection()
        message = "${s.name} is up to date."
    }

    fun refreshGuide(s: Source) = task("Downloading the guide for ${s.name}…") {
        repo.refreshEpg(s.id)
        refreshNowNext()
        message = "Guide updated."
    }

    fun updateSource(s: Source, name: String, location: String?, server: String?, user: String?, pass: String?) =
        task("Saving ${s.name}…") {
            repo.updateSource(s.id, name, location, server, user, pass)
            sources = repo.sources()
            sources.firstOrNull { it.id == source?.id }?.let { source = it }
            reloadSection()
            message = "Saved."
        }

    fun removeSource(s: Source) = task("Removing ${s.name}…") {
        repo.removeSource(s.id)
        if (settings.freeTvSourceId == s.id) changeSettings { freeTvSourceId = null }
        settings.hiddenCategories.keys.removeAll { it.startsWith("${s.id}:") }
        settings.save()
        sources = repo.sources()
        if (source?.id == s.id) {
            source = null
            playback.stop()
        }
        when {
            sources.isEmpty() -> screen = Screen.Setup
            source == null -> selectSource(sources.first())
        }
    }

    /** Turn the Free TV playlist on or off (Settings). */
    fun setFreeTv(on: Boolean) {
        val existing = sources.firstOrNull { it.isFreeTv }
        if (on && existing == null) addFreeTv()
        if (!on && existing != null) removeSource(existing)
    }

    suspend fun account(s: Source): Account = repo.account(s.id)

    suspend fun groupsOf(s: Source, kind: String, field: String? = null): List<Group> = repo.groups(s.id, kind, field)

    fun setHidden(s: Source, kind: String, name: String, hidden: Boolean) {
        changeSettings {
            val set = hidden(s.id, kind)
            if (hidden) set.add(name) else set.remove(name)
        }
        if (s.id == source?.id) reloadSection()
    }

    fun clearHistory() = task("Clearing history…") {
        repo.clearHistory()
        message = "Watch history cleared."
    }

    // ------------------------------------------------------------ browsing

    private val kindKey: String?
        get() = section.kind?.let { if (it == "live" && liveGroupBy == "country") "live:country" else it }

    private val groupField: String? get() = if (section == Section.Live && liveGroupBy == "country") FreeTv.COUNTRY_FIELD else null

    fun selectSection(s: Section) {
        section = s
        if (s != Section.Settings && s != Section.Search) changeSettings { lastSection = s.name }
        reloadSection()
    }

    private fun reloadSection() {
        openSeries = null
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch {
            val src = source
            val s = section
            when {
                s.kind != null && src != null -> {
                    val hidden = settings.hidden(src.id, kindKey!!)
                    val groups = repo.groups(src.id, s.kind, groupField).filter { it.name !in hidden }
                    // Not the sum of the groups: a channel can be in several countries.
                    val allCount = repo.query(src.id, s.kind, null, hidden, "provider", 0, 1, groupField).total
                    val all = Category(
                        when (s) {
                            Section.Live -> if (groupField != null) "All countries" else "All channels"
                            Section.Movies -> "All movies"
                            else -> "All series"
                        },
                        allCount,
                        isAll = true,
                    )
                    categories = listOf(all) + groups.map { Category(it.name, it.count) }
                    // Keep the selected category if it still exists.
                    val keep = group?.takeIf { g -> groups.any { it.name == g } }
                    loadGroup(keep)
                }
                s == Section.Favorites -> setItems(repo.favorites())
                s == Section.Recent -> setItems(repo.recent())
                s == Section.Search -> setItems(repo.search(query))
                else -> setItems(emptyList())
            }
        }
    }

    fun selectCategory(c: Category) {
        val name = if (c.isAll) null else c.name
        if (name == group && openSeries == null && items.isNotEmpty()) return
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch { loadGroup(name) }
    }

    fun groupLiveBy(by: String) {
        if (by == liveGroupBy) return
        liveGroupBy = by
        group = null
        changeSettings { freeTvGroupBy = by }
        reloadSection()
    }

    private val sort: String
        get() = if (section == Section.Live && settings.liveSortAz) "name_asc" else "provider"

    private suspend fun loadGroup(name: String?) {
        val src = source ?: return
        val kind = section.kind ?: return
        group = name
        openSeries = null
        val hidden = if (name == null) settings.hidden(src.id, kindKey!!) else emptySet()
        val page = repo.query(src.id, kind, name, hidden, sort, 0, PAGE, groupField)
        total = page.total
        setItems(page.items, page.total)
        // The info pane follows the list: drop a channel that is not in it (unless it is playing).
        if (focused != null && focused?.id != playback.current?.id && page.items.none { it.id == focused?.id }) {
            focused = null
            upcoming = emptyList()
        }
    }

    /** Called by lists when the focus nears the end of what is loaded. */
    fun loadMore() {
        val src = source ?: return
        val kind = section.kind ?: return
        if (loadingMore || openSeries != null || items.size >= total) return
        loadingMore = true
        viewModelScope.launch {
            try {
                val hidden = if (group == null) settings.hidden(src.id, kindKey!!) else emptySet()
                val page = repo.query(src.id, kind, group, hidden, sort, items.size, PAGE, groupField)
                items = items + page.items
                fetchNowNext(page.items)
            } finally {
                loadingMore = false
            }
        }
    }

    private suspend fun setItems(list: List<Item>, count: Int = list.size) {
        items = list
        total = count
        fetchNowNext(list)
    }

    private suspend fun fetchNowNext(list: List<Item>) {
        val live = list.filter { it.isLive }.map { it.id }
        live.chunked(300).forEach { nowNext.putAll(repo.nowNext(it)) }
    }

    private suspend fun refreshNowNext() {
        fetchNowNext(items.take(600))
        playback.current?.takeIf { it.isLive }?.let { fetchNowNext(listOf(it)) }
    }

    fun search(q: String) {
        query = q
        searchJob?.cancel()
        searchJob = viewModelScope.launch {
            delay(300) // debounce: search after the user pauses typing
            if (section == Section.Search) setItems(repo.search(q))
        }
    }

    /** The focus moved onto a channel: show its programme in the info pane. */
    fun focus(item: Item) {
        if (focused?.id == item.id) return
        focused = item
        focusJob?.cancel()
        focusJob = viewModelScope.launch {
            upcoming = emptyList()
            delay(150) // scrolling fast through a list should not query every row
            if (item.isLive) upcoming = repo.upcoming(item.id).take(8)
        }
    }

    /**
     * OK on an item. Live TV: the first press plays the channel in the preview window, a second
     * press on the playing channel goes full screen. Everything else plays full screen.
     */
    fun open(item: Item) {
        when {
            item.isSeries -> task("Loading episodes…") {
                val eps = repo.episodes(item)
                openSeries = item
                items = eps
                total = eps.size
            }
            item.isLive && section == Section.Live && settings.previewOnSelect && playback.current?.id != item.id -> {
                playback.play(item, items.filter { it.isLive })
                viewModelScope.launch { repo.recordPlay(item.id) }
            }
            item.isLive && playback.current?.id == item.id -> screen = Screen.Player
            else -> viewModelScope.launch {
                val resume = if (!item.isLive && settings.resumePlayback) repo.resumePositionMs(item.id) else 0L
                playback.play(item, if (item.isLive) items.filter { it.isLive } else listOf(item), resume)
                repo.recordPlay(item.id)
                screen = Screen.Player
            }
        }
    }

    fun closeSeries() {
        openSeries = null
        reloadSection()
    }

    fun toggleFavorite(item: Item) {
        viewModelScope.launch {
            val on = item.id !in favorites
            repo.setFavorite(item.id, on)
            favorites = if (on) favorites + item.id else favorites - item.id
            if (section == Section.Favorites) setItems(repo.favorites())
            message = if (on) "Added ${item.name} to favorites" else "Removed ${item.name} from favorites"
        }
    }

    fun isFavorite(item: Item) = item.id in favorites

    // ------------------------------------------------------------- player

    fun zap(delta: Int) {
        val prev = playback.current
        saveProgress(prev)
        playback.zap(delta)?.let { next ->
            focused = next
            viewModelScope.launch {
                repo.recordPlay(next.id)
                fetchNowNext(listOf(next))
            }
        }
    }

    /** Back from full screen: live keeps playing in the preview; films and episodes stop. */
    fun closePlayer() {
        val cur = playback.current
        screen = Screen.Main
        if (cur != null && !cur.isLive) {
            saveProgress(cur)
            playback.stop()
        }
        viewModelScope.launch { repo.persist() }
    }

    fun saveProgress(item: Item? = playback.current) {
        if (item == null || item.isLive) return
        val pos = playback.positionMs
        val dur = playback.durationMs
        if (dur <= 0) return
        viewModelScope.launch { repo.setProgress(item.id, pos, dur) }
    }

    /** The app went to the background (Home button, screen saver): stop the stream. */
    fun onAppHidden() {
        saveProgress()
        playback.player.pause()
        viewModelScope.launch { repo.persist() }
    }

    fun enterFullScreen() {
        if (playback.current != null) screen = Screen.Player
    }

    private companion object {
        const val PAGE = 200
    }
}
