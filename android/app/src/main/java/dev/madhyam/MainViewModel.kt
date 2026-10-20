package dev.madhyam

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.madhyam.ui.dayLabel
import dev.madhyam.ui.hhmm
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
    Home("Home", null),
    Live("Live TV", "live"),
    Movies("Movies", "movie"),
    Series("Series", "series"),
    Favorites("Favorites", null),
    Recent("Recent", null),
    Search("Search", null),
    Settings("Settings", null),
}

/** What the Home page shows (each row can be switched off in Settings › Home). */
data class HomeData(
    val continueWatching: List<Item> = emptyList(),
    val favoriteLive: List<Item> = emptyList(),
    val recentLive: List<Item> = emptyList(),
    val favoriteVod: List<Item> = emptyList(),
    val addedMovies: List<Item> = emptyList(),
    val addedSeries: List<Item> = emptyList(),
    val loaded: Boolean = false,
)

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

    // Details page of a film or series (Movies, Series, Favorites…); null = the grid or list.
    var details by mutableStateOf<Item?>(null)
        private set
    var detailsInfo by mutableStateOf<DetailsInfo?>(null)
        private set
    /** IMDb / Rotten Tomatoes ratings of the open details page (needs an OMDb key). */
    var ratings by mutableStateOf<ExternalRatings?>(null)
        private set
    var detailsLoading by mutableStateOf(false)
        private set
    var episodes by mutableStateOf<List<Item>>(emptyList())
        private set
    /** Season tab shown on a series page. */
    var season by mutableStateOf<String?>(null)
        private set
    /** The poster last opened, so the grid puts the focus back on it. */
    var lastOpenedId by mutableStateOf<String?>(null)
        private set
    /** "Continue watching" row above the Movies and Series grids. */
    var continueItems by mutableStateOf<List<Item>>(emptyList())
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
    /** Bumped whenever settings change, for screens that show the settings' collections (hidden categories, extra guides…); the simple settings are snapshot state themselves. */
    var settingsVersion by mutableStateOf(0)
        private set
    val nowNext = mutableStateMapOf<String, NowNext>()

    val engineVersion: String get() = repo.version

    /** Settings › Appearance (snapshot state, so only a theme change recomposes the theme, not every setting). */
    val themeName: String get() = settings.theme
    val customColor: String get() = settings.customColor
    val customLight: Boolean get() = settings.customLight

    private var itemsJob: Job? = null
    private var searchJob: Job? = null
    private var focusJob: Job? = null
    private var detailsJob: Job? = null
    private var loadingMore = false

    init {
        viewModelScope.launch {
            val started = System.currentTimeMillis()
            repo.init()
            // Remembered for next time's progress bar (only when there was a catalog to load).
            if (repo.hasSnapshot) changeSettings { lastLoadMs = System.currentTimeMillis() - started }
            sources = repo.sources()
            favorites = repo.favoriteIds()
            if (sources.isEmpty()) {
                screen = Screen.Setup
                if (!settings.freeTvPreinstalled) preinstallFreeTv()
            } else {
                // Upgrading with playlists already set up: their choice stands, nothing is added.
                if (!settings.freeTvPreinstalled) changeSettings { freeTvPreinstalled = true }
                screen = Screen.Main
                val start = settings.lastSourceId?.let { id -> sources.firstOrNull { it.id == id } } ?: sources.first()
                section = startSection()
                selectSource(start)
            }
            // Guides last: the pages show first, now/next fills in when they are in.
            repo.loadCachedGuides()
            refreshNowNext()
            refreshStaleGuides()
        }
        // The shared player only runs while it is on screen (#25): leaving the player from Search,
        // Favorites, Recent or Home, or Live TV for another section, stops the stream. Otherwise it
        // decodes into no surface and holds the provider's only connection.
        viewModelScope.launch {
            snapshotFlow { playerShown }.collect { shown -> if (!shown) stopHiddenPlayer() }
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
        // Settings › Guide › Extra guide URLs, on the same schedule (Windows does the same).
        val urls = settings.extraEpgUrls.toList()
        val age = repo.extraEpgAgeMs()
        if (urls.isNotEmpty() && (age == null || age > settings.epgRefreshHours * 3600_000L)) {
            try {
                repo.refreshExtraEpg(urls)
                refreshNowNext()
            } catch (e: Exception) {
                android.util.Log.w(Repository.TAG, "extra guide refresh failed", e)
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
        is java.io.IOException -> if (e.message?.startsWith("OpenSubtitles") == true) e.message!! else "Network error: ${e.message}"
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

    /** Change settings and save them (settings.json is written in the background). */
    fun changeSettings(block: AppSettings.() -> Unit) {
        settings.block()
        settings.save()
        settingsVersion++
    }

    // ------------------------------------------------------------- sources

    fun showSetup() = withPin("Enter the PIN to add a playlist") {
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

    /** First start: Free TV comes preinstalled, so there is something to watch at once. Tried again on the next start if it fails (offline). */
    private fun preinstallFreeTv() = task("Setting up Free TV channels…") {
        val id = repo.addFreeTv()
        changeSettings {
            freeTvSourceId = id
            freeTvPreinstalled = true
        }
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
        if (s == Section.Settings && section != Section.Settings && settings.parentalLockSettings && parentalActive && !unlocked) {
            withPin("Enter the PIN to open Settings") { selectSection(s) }
            return
        }
        section = s
        if (s != Section.Live) guideOpen = false
        if (s != Section.Settings && s != Section.Search) changeSettings { lastSection = s.name }
        reloadSection()
    }

    private fun reloadSection() {
        closeDetailsSilently()
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch {
            val src = source
            val s = section
            when {
                s.kind != null && src != null -> {
                    val hidden = settings.hidden(src.id, kindKey!!)
                    val groups = repo.groups(src.id, s.kind, groupField).filter { it.name !in hidden }
                        .let { sortCategories(it, s.kind) }
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
                    loadContinue()
                    // Keep the selected category if it still exists.
                    val keep = group?.takeIf { g -> groups.any { it.name == g } }
                    loadGroup(keep)
                }
                s == Section.Home -> loadHome()
                s == Section.Favorites -> setItems(byPageKind(repo.favorites()))
                s == Section.Recent -> setItems(byPageKind(repo.recent()))
                s == Section.Search -> setItems(searchHits(query))
                else -> setItems(emptyList())
            }
        }
    }

    /**
     * Show a category. Moving the focus over a locked one only says it is locked; OK on it
     * ([fromClick]) asks for the PIN.
     */
    fun selectCategory(c: Category, fromClick: Boolean = false) {
        val name = if (c.isAll) null else c.name
        if (name != null && isCategoryLocked(name)) {
            if (fromClick) {
                withPin("Enter the PIN to open “$name”") { selectCategory(c, fromClick = true) }
            } else {
                itemsJob?.cancel()
                group = name
                lockedGroup = true
                items = emptyList()
                total = 0
                loadedRows = 0
            }
            return
        }
        if (name == group && details == null && items.isNotEmpty() && !lockedGroup) return
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch { loadGroup(name) }
    }

    /** The selected category is locked and shown empty until the PIN is entered. */
    var lockedGroup by mutableStateOf(false)
        private set

    fun groupLiveBy(by: String) {
        if (by == liveGroupBy) return
        liveGroupBy = by
        group = null
        changeSettings { freeTvGroupBy = by }
        reloadSection()
    }

    private val isVod get() = section == Section.Movies || section == Section.Series

    private val sort: String
        get() = when {
            isVod -> settings.vodSort
            section == Section.Live && settings.liveSortAz -> "name_asc"
            else -> "provider"
        }

    private val minRating: Double get() = if (isVod) settings.vodMinRating.toDouble() else 0.0

    fun setVodSort(sort: String) {
        if (sort == settings.vodSort) return
        changeSettings { vodSort = sort }
        reloadGrid()
    }

    fun setVodMinRating(rating: Int) {
        if (rating == settings.vodMinRating) return
        changeSettings { vodMinRating = rating }
        reloadGrid()
    }

    private fun reloadGrid() {
        lastOpenedId = null
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch { loadGroup(group) }
    }

    /** Films (Movies) or episodes (Series) started but not finished. */
    private suspend fun loadContinue() {
        val want = when (section) {
            Section.Movies -> "movie"
            Section.Series -> "episode"
            else -> null
        }
        continueItems = if (want == null) emptyList() else repo.continueWatching(40).filter { it.kind == want && it.durationSec > 0 }
    }

    private suspend fun loadGroup(name: String?) {
        val src = source ?: return
        val kind = section.kind ?: return
        group = name
        closeDetailsSilently()
        // A locked category stays empty with its note until the PIN is entered.
        lockedGroup = name != null && isCategoryLocked(name)
        if (lockedGroup) {
            setItems(emptyList())
            return
        }
        val hidden = if (name == null) settings.hidden(src.id, kindKey!!) + lockedCategoryNames() else emptySet()
        val page = repo.query(src.id, kind, name, hidden, sort, 0, PAGE, groupField, minRating)
        total = page.total
        setItems(page.items.filterNot { itemLocked(it) }, page.total, rows = page.items.size)
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
        if (loadingMore || loadedRows >= total) return
        loadingMore = true
        viewModelScope.launch {
            try {
                val hidden = if (group == null) settings.hidden(src.id, kindKey!!) + lockedCategoryNames() else emptySet()
                val page = repo.query(src.id, kind, group, hidden, sort, loadedRows, PAGE, groupField, minRating)
                loadedRows += page.items.size
                items = items + page.items.filterNot { itemLocked(it) }
                fetchNowNext(page.items)
                if (guideOpen) guideData = guideData + repo.guide(page.items.filter { it.isLive }.map { it.id }, guideFrom, guideTo)
            } finally {
                loadingMore = false
            }
        }
    }

    /** [rows]: how many rows of the engine's list [list] came from (locked items are left out). */
    private suspend fun setItems(list: List<Item>, count: Int = list.size, rows: Int = list.size) {
        items = list
        loadedRows = rows
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
            if (section == Section.Search) setItems(searchHits(q))
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
     * press on the playing channel goes full screen. Films and series open their details page;
     * episodes play full screen.
     */
    fun open(item: Item) {
        if (itemLocked(item)) {
            withPin("Enter the PIN to open “${item.name}”") { open(item) }
            return
        }
        when {
            item.isSeries || item.kind == "movie" -> showDetails(item)
            item.isEpisode -> resume(item)
            item.isLive && section == Section.Live && settings.previewOnSelect && playback.current?.id != item.id -> {
                playback.play(item, items.filter { it.isLive })
                viewModelScope.launch { repo.recordPlay(item.id) }
            }
            item.isLive && playback.current?.id == item.id -> screen = Screen.Player
            else -> viewModelScope.launch {
                playback.play(item, items.filter { it.isLive })
                repo.recordPlay(item.id)
                screen = Screen.Player
            }
        }
    }

    // ------------------------------------------------------------ details

    /** Open the details page of a film or series and load its details (and episodes). */
    fun showDetails(item: Item, then: (suspend () -> Unit)? = null) {
        if (itemLocked(item)) {
            withPin("Enter the PIN to open “${item.name}”") { showDetails(item, then) }
            return
        }
        lastOpenedId = item.id
        detailsJob?.cancel()
        details = item
        detailsInfo = null
        ratings = null
        episodes = emptyList()
        season = null
        detailsLoading = true
        detailsJob = viewModelScope.launch {
            try {
                val fresh = repo.item(item.id) ?: item
                details = fresh
                val info = repo.details(fresh)
                // Above the age limit: the page closes and asks for the PIN.
                val age = Parental.age(info?.ageRating)
                if (age != null && settings.parentalMaxAge > 0 && age > settings.parentalMaxAge && parentalActive && !unlocked) {
                    closeDetailsSilently()
                    withPin("“${fresh.name}” is rated $age+. Enter the PIN to open it") { showDetails(item, then) }
                    return@launch
                }
                detailsInfo = info
                if (settings.omdbApiKey.isNotBlank()) {
                    val info = detailsInfo
                    viewModelScope.launch { ratings = repo.ratings(fresh, info).takeIf { details?.id == fresh.id } }
                }
                if (fresh.isSeries) {
                    episodes = try {
                        repo.episodes(fresh)
                    } catch (e: Exception) {
                        message = "Episodes could not be loaded: ${explain(e)}"
                        emptyList()
                    }
                    season = resumeEpisode()?.season ?: episodes.firstOrNull()?.season
                }
            } finally {
                detailsLoading = false
            }
            then?.invoke()
        }
    }

    /** Back on the details page. */
    fun closeDetails() {
        closeDetailsSilently()
        viewModelScope.launch { loadContinue() }
    }

    private fun closeDetailsSilently() {
        detailsJob?.cancel()
        details = null
        detailsInfo = null
        episodes = emptyList()
        season = null
        detailsLoading = false
    }

    fun selectSeason(s: String) {
        season = s
    }

    /** Episode to offer as "Resume": started but unfinished, else the first unwatched one after a watched one. */
    fun resumeEpisode(): Item? =
        episodes.firstOrNull { it.isStarted }
            ?: if (episodes.any { it.isWatched }) episodes.firstOrNull { !it.isWatched } else null

    fun playMovie(startOver: Boolean = false) {
        val it = details?.takeIf { !it.isSeries } ?: return
        viewModelScope.launch {
            val resume = if (!startOver && settings.resumePlayback && it.isStarted) (it.positionSec * 1000).toLong() else 0L
            playback.play(it, listOf(it), resume)
            repo.recordPlay(it.id)
            screen = Screen.Player
        }
    }

    fun playEpisode(ep: Item, startOver: Boolean = false) {
        season = ep.season
        viewModelScope.launch {
            val resume = if (!startOver && settings.resumePlayback && ep.isStarted) (ep.positionSec * 1000).toLong() else 0L
            playback.play(ep, episodes.ifEmpty { listOf(ep) }, resume)
            repo.recordPlay(ep.id)
            screen = Screen.Player
        }
    }

    /** Continue watching / Recent: resume a film or an episode, with its details page behind the player. */
    fun resume(item: Item) {
        if (itemLocked(item)) {
            withPin("Enter the PIN to watch “${item.name}”") { resume(item) }
            return
        }
        when {
            item.isEpisode -> viewModelScope.launch {
                val series = item.seriesId?.let { repo.item(it) }
                if (series == null) {
                    // The series is gone from the catalog: just play the episode.
                    val resume = if (settings.resumePlayback) repo.resumePositionMs(item.id) else 0L
                    playback.play(item, listOf(item), resume)
                    repo.recordPlay(item.id)
                    screen = Screen.Player
                } else {
                    showDetails(series) { playEpisode(episodes.firstOrNull { it.id == item.id } ?: item) }
                }
            }
            else -> showDetails(item) { playMovie() }
        }
    }

    /** Watched mark on one film or episode. */
    fun toggleWatched(item: Item) = setWatched(listOf(item), !item.isWatched)

    fun setWatched(list: List<Item>, watched: Boolean) {
        viewModelScope.launch {
            repo.setWatched(list, watched)
            reloadDetailsProgress()
        }
    }

    /** Re-read progress after playing or marking (the page shows resume points and watched marks). */
    private suspend fun reloadDetailsProgress() {
        val d = details ?: return
        val fresh = repo.item(d.id)
        val eps = if (d.isSeries) repo.episodes(d) else episodes
        if (details?.id != d.id) return
        if (fresh != null) details = fresh
        episodes = eps
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
        playback.zap(delta) { itemLocked(it) }?.let { next ->
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
        val pos = playback.positionMs
        val dur = playback.durationMs
        if (cur != null && !cur.isLive) playback.stop()
        viewModelScope.launch {
            if (cur != null && !cur.isLive && !cur.isCatchup && dur > 0) repo.setProgress(cur.id, pos, dur)
            repo.persist()
            reloadDetailsProgress()
            if (isVod) loadContinue()
            if (section == Section.Home) loadHome()
        }
    }

    fun saveProgress(item: Item? = playback.current) {
        if (item == null || item.isLive || item.isCatchup) return
        val pos = playback.positionMs
        val dur = playback.durationMs
        if (dur <= 0) return
        viewModelScope.launch { repo.setProgress(item.id, pos, dur) }
    }

    /** False between onStop and onStart (the stream is suspended, not stopped, meanwhile). */
    private var appVisible = true

    /** Whether a page that shows the shared player is open: full screen, or Live TV's preview. */
    private val playerShown: Boolean
        get() = screen == Screen.Player || (screen == Screen.Main && section == Section.Live && !guideOpen)

    /** Stop the stream nobody sees; films and episodes keep their resume position. */
    private fun stopHiddenPlayer() {
        if (playback.current == null) return
        saveProgress()
        playback.stop()
    }

    /**
     * The app went to the background (Home button, another app, screen saver): stop the stream
     * and let go of the connection, but keep what was playing so [onAppShown] can start it again.
     */
    fun onAppHidden() {
        appVisible = false
        saveProgress()
        playback.suspend()
        viewModelScope.launch { repo.persist() }
    }

    /** Back in front: restart what was on screen (live at the live edge, films where they were). */
    fun onAppShown() {
        if (appVisible) return
        appVisible = true
        if (playerShown) playback.resume() else playback.stop()
    }

    // -------------------------------------------- 0.9.0: Windows parity settings

    /** Settings › Categories › Order (per kind); countries default to A–Z, "Undefined" last. */
    private fun sortCategories(groups: List<Group>, kind: String): List<Group> {
        val byCountry = groupField != null
        val order = settings.categorySort[kind] ?: if (byCountry) "name_asc" else "provider"
        val sorted = when (order) {
            "name_asc" -> groups.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
            "name_desc" -> groups.sortedWith(compareByDescending(String.CASE_INSENSITIVE_ORDER) { it.name })
            else -> groups
        }
        return if (byCountry) sorted.sortedBy { if (it.name == "Undefined") 1 else 0 } else sorted
    }

    fun setCategoryOrder(kind: String, order: String) {
        changeSettings { categorySort[kind.substringBefore(':')] = order }
        reloadSection()
    }

    suspend fun testConnection(s: Source): String = try {
        repo.testConnection(s)
    } catch (e: Exception) {
        explain(e)
    }

    fun setSourceUserAgent(s: Source, ua: String) = changeSettings {
        if (ua.isBlank()) sourceUserAgent.remove(s.id) else sourceUserAgent[s.id] = ua.trim()
    }

    fun addExtraEpg(url: String) {
        val u = url.trim()
        if (u.isEmpty() || u in settings.extraEpgUrls) return
        changeSettings { extraEpgUrls += u }
        refreshExtraGuides()
    }

    fun removeExtraEpg(url: String) {
        changeSettings { extraEpgUrls -= url }
        refreshExtraGuides()
    }

    fun refreshExtraGuides() = task("Downloading extra guides…") {
        val n = repo.refreshExtraEpg(settings.extraEpgUrls.toList())
        refreshNowNext()
        message = if (settings.extraEpgUrls.isEmpty()) "Extra guides removed." else "$n of ${settings.extraEpgUrls.size} extra guides loaded."
    }

    /** Settings › Guide › Time offset: guide times move at once. */
    fun setEpgOffset(minutes: Int) {
        changeSettings { epgOffsetMinutes = minutes }
        nowNext.clear()
        viewModelScope.launch {
            refreshNowNext()
            if (guideOpen) moveGuideTo(guideFrom)
        }
    }

    // ------------------------------------------------------- home, filters

    /** Rows of the Home page (Windows dashboard). */
    var home by mutableStateOf(HomeData())
        private set

    private suspend fun loadHome() {
        val src = source
        val cont = repo.continueWatching(20).filter { !it.isLive && it.durationSec > 0 && !it.isWatched }
        val favs = repo.favorites()
        val recentLive = repo.recent("live", 30)
        val addedMovies = if (src != null && settings.dashRecentlyAdded) {
            repo.query(src.id, "movie", null, settings.hidden(src.id, "movie"), "added_desc", 0, 20).items
        } else {
            emptyList()
        }
        val addedSeries = if (src != null && settings.dashRecentlyAdded) {
            repo.query(src.id, "series", null, settings.hidden(src.id, "series"), "added_desc", 0, 20).items
        } else {
            emptyList()
        }
        val ok = { it: Item -> !itemLocked(it) }
        home = HomeData(
            continueWatching = cont.filter(ok),
            favoriteLive = favs.filter { it.isLive && ok(it) },
            recentLive = recentLive.filter(ok),
            favoriteVod = favs.filter { (it.kind == "movie" || it.isSeries) && ok(it) },
            addedMovies = addedMovies.filter(ok),
            addedSeries = addedSeries.filter(ok),
            loaded = true,
        )
        fetchNowNext(home.favoriteLive + home.recentLive)
    }

    /** Favorites / Recent: "live", "vod" (films, series, episodes) or null for everything. */
    var pageKind by mutableStateOf<String?>(null)
        private set

    fun changePageKind(kind: String?) {
        pageKind = kind
        reloadSection()
    }

    private fun byPageKind(list: List<Item>): List<Item> = when (pageKind) {
        "live" -> list.filter { it.isLive }
        "vod" -> list.filter { !it.isLive }
        else -> list
    }

    /** Search across the kinds and playlists chosen with the filter chips, grouped by kind. */
    private suspend fun searchHits(q: String): List<Item> {
        if (q.isBlank()) return emptyList()
        val kinds = buildList {
            if (settings.searchLive) add("live")
            if (settings.searchMovies) add("movie")
            if (settings.searchSeries) add("series")
        }
        val src = source
        return kinds.flatMap { repo.search(q, it) }
            .filter { settings.searchAllSources || src == null || it.sourceId == src.id }
            .filterNot { itemLocked(it) }
    }

    fun changeSearchFilter(block: AppSettings.() -> Unit) {
        changeSettings(block)
        search(query)
    }

    /** Watch a channel full screen; Up/Down zap through [list] (a Home row, for example). */
    fun watchChannel(channel: Item, list: List<Item>) {
        if (itemLocked(channel)) {
            withPin("Enter the PIN to watch “${channel.name}”") { watchChannel(channel, list) }
            return
        }
        viewModelScope.launch {
            focused = channel
            playback.play(channel, list.filter { it.isLive })
            repo.recordPlay(channel.id)
            screen = Screen.Player
        }
    }

    /** Details page of the series an episode belongs to. */
    fun openSeriesOf(episode: Item) {
        viewModelScope.launch {
            val series = episode.seriesId?.let { repo.item(it) }
            if (series != null) showDetails(series) else message = "The series of this episode is no longer in the playlist."
        }
    }

    /** Recent: hold OK removes an entry from the history. */
    fun removeFromHistory(item: Item) {
        viewModelScope.launch {
            repo.removeFromHistory(item.id)
            if (section == Section.Recent) setItems(byPageKind(repo.recent()))
            message = "Removed ${item.name} from Recent"
        }
    }

    fun clearHistoryHere() = task("Clearing history…") {
        repo.clearHistory()
        if (section == Section.Recent) setItems(emptyList())
        message = "Recent is empty. Favorites and resume positions are kept."
    }

    // ----------------------------------------------------- online subtitles

    /** OpenSubtitles search needs an API key (Settings › Online services). */
    val subtitleSearchAvailable: Boolean get() = settings.osApiKey.isNotBlank()

    /** Results of the subtitle search; null while the search dialog is closed. */
    var subtitleResults by mutableStateOf<List<SubtitleHit>?>(null)
        private set
    var subtitleSearching by mutableStateOf(false)
        private set
    var subtitleError by mutableStateOf<String?>(null)
        private set
    var subtitleQuery by mutableStateOf("")
        private set

    /** Open the search for the playing film or episode, with its title as the query. */
    fun openSubtitleSearch() {
        val item = playback.current ?: return
        val title = if (item.isEpisode) details?.name ?: item.name else item.name
        searchSubtitles(Ratings.cleanTitle(title))
    }

    fun searchSubtitles(query: String) {
        val item = playback.current ?: return
        subtitleQuery = query
        subtitleResults = emptyList()
        subtitleError = null
        subtitleSearching = true
        viewModelScope.launch {
            try {
                val season = if (item.isEpisode) item.season?.toIntOrNull() else null
                val episode = if (item.isEpisode) item.episode?.toIntOrNull() else null
                subtitleResults = repo.searchSubtitles(query, season, episode)
                    .sortedByDescending { it.downloads }
                    .take(60)
            } catch (e: Exception) {
                subtitleError = explain(e)
            } finally {
                subtitleSearching = false
            }
        }
    }

    fun closeSubtitleSearch() {
        subtitleResults = null
        subtitleError = null
    }

    fun useSubtitle(hit: SubtitleHit) {
        closeSubtitleSearch()
        task("Downloading the subtitle…") {
            val (sub, remaining) = repo.downloadSubtitle(hit)
            playback.addSubtitle(sub)
            message = "Subtitle added · $remaining downloads left today"
        }
    }

    /** The OpenSubtitles login password (kept encrypted, never in settings.json). */
    var openSubtitlesPassword: String
        get() = repo.openSubtitlesPassword
        set(value) {
            repo.openSubtitlesPassword = value
            settingsVersion++
        }

    // ------------------------------------------------------------ TV guide

    /** The channels × time grid is showing in Live TV. */
    var guideOpen by mutableStateOf(false)
        private set
    /** Start of the visible window (Unix seconds, on a half hour). */
    var guideFrom by mutableLongStateOf(0L)
        private set
    val guideTo: Long get() = guideFrom + GUIDE_SPAN_MIN * 60L
    var guideData by mutableStateOf<Map<String, List<Programme>>>(emptyMap())
        private set
    private var guideJob: Job? = null

    /**
     * The channel the guide opens at: the one playing when it was opened, else the focused one.
     * (The guide has no preview, so the stream stops as it opens.)
     */
    var guideStartId by mutableStateOf<String?>(null)
        private set

    fun openGuide() {
        guideStartId = playback.current?.takeIf { it.isLive }?.id ?: focused?.id
        if (section != Section.Live) selectSection(Section.Live)
        guideOpen = true
        guideNow()
    }

    fun closeGuide() {
        guideOpen = false
        guideJob?.cancel()
    }

    /** Back to "now": the window starts half an hour before the current half hour. */
    fun guideNow() {
        val now = System.currentTimeMillis() / 1000
        moveGuideTo(now - now % 1800 - 1800)
    }

    /** Move the window by [minutes]; never before the archive (or a day back) nor past two days ahead. */
    fun shiftGuide(minutes: Int) {
        val now = System.currentTimeMillis() / 1000
        val archive = items.maxOfOrNull { it.archiveDays }?.coerceAtLeast(1) ?: 1
        val earliest = now - now % 1800 - archive * 86_400L
        val latest = now + 2 * 86_400L
        moveGuideTo((guideFrom + minutes * 60L).coerceIn(earliest, latest))
    }

    private fun moveGuideTo(from: Long) {
        guideFrom = from
        guideJob?.cancel()
        guideJob = viewModelScope.launch {
            guideData = repo.guide(items.filter { it.isLive }.map { it.id }, guideFrom, guideTo)
        }
    }

    /** Whether a past programme can be played from the provider's archive (Xtream catch-up). */
    fun canCatchup(channel: Item, p: Programme): Boolean = source?.isXtream == true && repo.canCatchup(channel, p)

    /** OK on a guide cell: live now plays the channel, a recorded past programme plays as catch-up. */
    fun openFromGuide(channel: Item, p: Programme?) {
        if (itemLocked(channel)) {
            withPin("Enter the PIN to watch “${channel.name}”") { openFromGuide(channel, p) }
            return
        }
        val now = System.currentTimeMillis() / 1000
        when {
            p == null || (p.start <= now && now < p.stop) -> viewModelScope.launch {
                focused = channel
                playback.play(channel, items.filter { it.isLive })
                repo.recordPlay(channel.id)
                screen = Screen.Player
            }
            p.stop <= now && canCatchup(channel, p) -> playCatchup(channel, p)
            p.stop <= now -> message = "${channel.name} keeps no recording of “${p.title}”."
            else -> message = "“${p.title}” starts ${dayLabel(p.start).lowercase()} at ${hhmm(p.start)}."
        }
    }

    fun playCatchup(channel: Item, p: Programme) = task("Opening the recording…") {
        val rec = repo.catchup(channel, p) ?: throw IllegalStateException("No recording of “${p.title}” is available.")
        playback.play(rec, listOf(rec))
        screen = Screen.Player
    }

    fun enterFullScreen() {
        if (playback.current != null) screen = Screen.Player
    }

    // ------------------------------------------------- 1.1.1: parental controls

    /** The PIN dialog the screen shows; [onOk] runs once the right PIN is entered. */
    class PinRequest(val title: String, val onOk: () -> Unit)

    var pinRequest by mutableStateOf<PinRequest?>(null)
        private set

    /** The PIN was entered in this session (until the app closes or 10 idle minutes pass). */
    var unlocked by mutableStateOf(false)
        private set
    private var relockJob: Job? = null
    private var wrongPins = 0
    private var pinWaitUntil = 0L

    /** Rows of the current list already read from the engine (locked items are left out of [items]). */
    private var loadedRows = 0

    val parentalActive: Boolean get() = settings.parentalOn && settings.parentalPinHash.isNotEmpty()

    /** Run [action] now, or once the PIN is entered when parental controls are on and locked. */
    fun withPin(title: String, action: () -> Unit) {
        if (!parentalActive || unlocked) {
            if (unlocked) touchUnlock()
            action()
        } else {
            pinRequest = PinRequest(title, action)
        }
    }

    /** Ask for the PIN even while unlocked (turning parental controls off, changing the PIN). */
    fun requirePin(title: String, action: () -> Unit) {
        pinRequest = PinRequest(title, action)
    }

    fun cancelPin() {
        pinRequest = null
    }

    sealed interface PinCheck {
        data object Ok : PinCheck
        data class Wrong(val triesLeft: Int) : PinCheck
        data class Wait(val seconds: Int) : PinCheck
    }

    /** Check a PIN typed in the dialog; the right one unlocks the session and runs the request. */
    fun checkPin(pin: String): PinCheck {
        val now = System.currentTimeMillis()
        if (now < pinWaitUntil) return PinCheck.Wait(((pinWaitUntil - now + 999) / 1000).toInt())
        if (Parental.hash(settings.parentalPinSalt, pin) != settings.parentalPinHash) {
            wrongPins++
            if (wrongPins >= Parental.MAX_TRIES) {
                wrongPins = 0
                pinWaitUntil = now + Parental.WAIT_MS
                return PinCheck.Wait((Parental.WAIT_MS / 1000).toInt())
            }
            return PinCheck.Wrong(Parental.MAX_TRIES - wrongPins)
        }
        wrongPins = 0
        unlocked = true
        touchUnlock()
        val req = pinRequest
        pinRequest = null
        req?.onOk?.invoke()
        return PinCheck.Ok
    }

    /** Seconds to wait before the next try (after five wrong PINs), or 0. */
    fun pinWaitSeconds(): Int = ((pinWaitUntil - System.currentTimeMillis() + 999) / 1000).toInt().coerceAtLeast(0)

    private fun touchUnlock() {
        relockJob?.cancel()
        relockJob = viewModelScope.launch {
            delay(Parental.UNLOCK_MS)
            lockNow()
        }
    }

    /** Lock again (the open lock in the rail, or 10 idle minutes); lists drop locked items again. */
    fun lockNow() {
        relockJob?.cancel()
        if (!unlocked) return
        unlocked = false
        if (playback.current?.let { itemLocked(it) } == true) {
            if (screen == Screen.Player) closePlayer()
            playback.stop()
        }
        if (section == Section.Settings && settings.parentalLockSettings && parentalActive) selectSection(Section.Home)
        else reloadSection()
    }

    /** Why a category is locked ("Locked" or "Adult"), whether or not the session is unlocked. */
    fun categoryLockReason(sourceId: Int, kind: String, name: String): String? =
        if (parentalActive) categoryLockSetting(sourceId, kind, name) else null

    /** The lock set up for a category ("Locked", "Adult" or null), even while parental controls are off. */
    fun categoryLockSetting(sourceId: Int, kind: String, name: String): String? {
        val key = AppSettings.hiddenKey(sourceId, Parental.kindKey(kind))
        if (settings.lockedCategories[key]?.contains(name) == true) return "Locked"
        if (settings.parentalAutoAdult && Parental.looksAdult(name) && settings.unlockedCategories[key]?.contains(name) != true) return "Adult"
        return null
    }

    /** A category of the current list that needs the PIN now. */
    fun isCategoryLocked(name: String): Boolean {
        val src = source ?: return false
        val kind = section.kind ?: return false
        return !unlocked && categoryLockReason(src.id, kind, name) != null
    }

    private fun lockedCategoryNames(): Set<String> =
        if (unlocked || !parentalActive) emptySet() else categories.filter { !it.isAll && isCategoryLocked(it.name) }.map { it.name }.toSet()

    /** An item that needs the PIN now: its category is locked, or its name looks adult. */
    fun itemLocked(item: Item): Boolean {
        if (!parentalActive || unlocked) return false
        if (categoryLockReason(item.sourceId, item.kind, item.group) != null) return true
        if (!settings.parentalAutoAdult || !Parental.looksAdult(item.name)) return false
        // An adult-looking channel in a category the user unlocked follows its category.
        val key = AppSettings.hiddenKey(item.sourceId, Parental.kindKey(item.kind))
        return settings.unlockedCategories[key]?.contains(item.group) != true
    }

    /** Lock or unlock a category in Settings › Parental controls › Locked categories. */
    fun setCategoryLock(sourceId: Int, kind: String, name: String, locked: Boolean) {
        changeSettings {
            val key = AppSettings.hiddenKey(sourceId, kind)
            val lockedSet = lockedCategories.getOrPut(key) { mutableSetOf() }
            val unlockedSet = unlockedCategories.getOrPut(key) { mutableSetOf() }
            if (locked) {
                lockedSet += name
                unlockedSet -= name
            } else {
                lockedSet -= name
                // An adult-looking category stays open only with an explicit unlock.
                if (Parental.looksAdult(name)) unlockedSet += name
            }
        }
        if (sourceId == source?.id) reloadSection()
    }

    fun setPin(pin: String) {
        changeSettings {
            parentalPinSalt = Parental.newSalt()
            parentalPinHash = Parental.hash(parentalPinSalt, pin)
        }
    }

    fun setParentalOn(on: Boolean) {
        changeSettings { parentalOn = on }
        if (on) {
            unlocked = true // the PIN was just entered: this session stays open
            touchUnlock()
        } else {
            relockJob?.cancel()
            unlocked = false
        }
        reloadSection()
    }

    /** Forgotten PIN: remove the PIN and every lock (after the user typed RESET). */
    fun resetParental() {
        android.util.Log.i(Repository.TAG, "parental controls reset: PIN and locks removed")
        changeSettings {
            parentalOn = false
            parentalPinHash = ""
            parentalPinSalt = ""
            parentalAutoAdult = true
            parentalLockSettings = false
            parentalMaxAge = 0
            lockedCategories.clear()
            unlockedCategories.clear()
        }
        relockJob?.cancel()
        unlocked = false
        reloadSection()
        message = "Parental controls were reset."
    }


    private companion object {
        const val PAGE = 200
        const val GUIDE_SPAN_MIN = 150
    }
}
