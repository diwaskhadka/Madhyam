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

/** Top-level "pages" of the app. */
sealed interface Screen {
    data object Loading : Screen
    data object Setup : Screen
    data object Browse : Screen
    /** `playlist` lets the remote zap channel up/down without leaving the player. */
    data class Player(val playlist: List<Item>, val index: Int) : Screen
}

/** What the left-hand menu is showing. */
enum class Section(val label: String, val kind: String?) {
    Live("Live TV", "live"),
    Movies("Movies", "movie"),
    Series("Series", "series"),
    Favorites("Favorites", null),
    Recent("Recent", null),
    Search("Search", null),
}

class MainViewModel(app: Application) : AndroidViewModel(app) {
    private val repo = (app as App).repo

    var screen by mutableStateOf<Screen>(Screen.Loading)
        private set
    var sources by mutableStateOf<List<Source>>(emptyList())
        private set
    var source by mutableStateOf<Source?>(null)
        private set
    var section by mutableStateOf(Section.Live)
        private set
    var groups by mutableStateOf<List<Group>>(emptyList())
        private set
    var group by mutableStateOf<String?>(null)
        private set
    var items by mutableStateOf<List<Item>>(emptyList())
        private set
    /** Non-null while a series' episodes are shown in place of the item list. */
    var openSeries by mutableStateOf<Item?>(null)
        private set
    var query by mutableStateOf("")
        private set
    var busy by mutableStateOf<String?>(null)
        private set
    var message by mutableStateOf<String?>(null)
        private set
    val nowNext = mutableStateMapOf<String, NowNext>()

    private var itemsJob: Job? = null
    private var searchJob: Job? = null

    init {
        viewModelScope.launch {
            repo.init()
            sources = repo.sources()
            if (sources.isEmpty()) {
                screen = Screen.Setup
            } else {
                screen = Screen.Browse
                selectSource(sources.first())
            }
        }
        // Keep "now playing" fresh while the app is open.
        viewModelScope.launch {
            while (isActive) {
                delay(60_000)
                refreshNowNext()
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

    // ------------------------------------------------------------- sources

    fun showSetup() {
        screen = Screen.Setup
    }

    fun cancelSetup() {
        if (sources.isNotEmpty()) screen = Screen.Browse
    }

    fun addM3u(name: String, url: String) = task("Downloading playlist…") {
        val id = repo.addM3u(name.ifBlank { "Playlist" }, url.trim())
        afterAdd(id)
    }

    fun addXtream(name: String, server: String, user: String, pass: String) = task("Logging in and downloading the catalog…") {
        val id = repo.addXtream(name.ifBlank { "Provider" }, server.trim(), user.trim(), pass)
        afterAdd(id)
    }

    private suspend fun afterAdd(id: Int) {
        sources = repo.sources()
        screen = Screen.Browse
        sources.firstOrNull { it.id == id }?.let { selectSource(it) }
    }

    fun selectSource(s: Source) {
        source = s
        selectSection(if (section == Section.Search) Section.Live else section)
    }

    fun refreshSource() {
        val s = source ?: return
        task("Refreshing ${s.name}…") {
            repo.refresh(s.id)
            selectSection(section)
        }
    }

    fun removeSource() {
        val s = source ?: return
        task("Removing ${s.name}…") {
            repo.removeSource(s.id)
            sources = repo.sources()
            source = null
            if (sources.isEmpty()) screen = Screen.Setup else selectSource(sources.first())
        }
    }

    // ------------------------------------------------------------ browsing

    fun selectSection(s: Section) {
        section = s
        openSeries = null
        group = null
        groups = emptyList()
        items = emptyList()
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch {
            val src = source
            when {
                s.kind != null && src != null -> {
                    groups = repo.groups(src.id, s.kind)
                    selectGroupNow(groups.firstOrNull()?.name)
                }
                s == Section.Favorites -> setItems(repo.favorites())
                s == Section.Recent -> setItems(repo.recent())
                s == Section.Search -> setItems(repo.search(query))
            }
        }
    }

    fun selectGroup(name: String) {
        if (name == group && openSeries == null) return
        itemsJob?.cancel()
        itemsJob = viewModelScope.launch { selectGroupNow(name) }
    }

    private suspend fun selectGroupNow(name: String?) {
        val src = source ?: return
        val kind = section.kind ?: return
        group = name
        openSeries = null
        setItems(if (name == null) emptyList() else repo.items(src.id, kind, name))
    }

    private suspend fun setItems(list: List<Item>) {
        items = list
        refreshNowNext()
    }

    private suspend fun refreshNowNext() {
        val live = items.filter { it.isLive }.take(500).map { it.id }
        if (live.isNotEmpty()) nowNext.putAll(repo.nowNext(live))
    }

    fun search(q: String) {
        query = q
        searchJob?.cancel()
        searchJob = viewModelScope.launch {
            delay(250) // debounce: search after the user pauses typing
            if (section == Section.Search) setItems(repo.search(q))
        }
    }

    fun open(item: Item) {
        if (item.isSeries) {
            task("Loading episodes…") {
                val eps = repo.episodes(item)
                openSeries = item
                items = eps
            }
        } else {
            val list = items.filter { !it.isSeries }
            val index = list.indexOfFirst { it.id == item.id }.coerceAtLeast(0)
            screen = Screen.Player(list.ifEmpty { listOf(item) }, index)
        }
    }

    fun closeSeries() {
        val g = group
        openSeries = null
        if (g != null) selectGroup(g) else selectSection(section)
    }

    fun toggleFavorite(item: Item) {
        viewModelScope.launch {
            repo.setFavorite(item.id, !repo.isFavorite(item.id))
            if (section == Section.Favorites) setItems(repo.favorites())
            message = if (repo.isFavorite(item.id)) "Added ${item.name} to favorites" else "Removed ${item.name} from favorites"
        }
    }

    // ------------------------------------------------------------- player

    fun zap(delta: Int) {
        val p = screen as? Screen.Player ?: return
        val n = p.playlist.size
        if (n > 1) screen = p.copy(index = ((p.index + delta) % n + n) % n)
    }

    fun closePlayer() {
        screen = Screen.Browse
        viewModelScope.launch { repo.persist() }
    }

    fun onPlaybackStarted(item: Item) {
        viewModelScope.launch { repo.recordPlay(item.id) }
    }

    suspend fun resumePositionMs(item: Item): Long = if (item.isLive) 0L else repo.resumePositionMs(item.id)

    fun saveProgress(item: Item, positionMs: Long, durationMs: Long) {
        if (item.isLive || durationMs <= 0) return
        viewModelScope.launch { repo.setProgress(item.id, positionMs, durationMs) }
    }

    suspend fun nowNextFor(item: Item): NowNext? = if (item.isLive) repo.nowNext(listOf(item.id))[item.id] else null
}
