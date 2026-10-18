package dev.madhyam.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.LiveTv
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.VideoLibrary
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Button
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.Brand
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import dev.madhyam.Section
import kotlinx.coroutines.delay

/**
 * Home (the Windows dashboard): a "continue watching" hero, then rows of favorite and recent
 * channels, favorite films and series, and what was added recently. Rows are switched in
 * Settings › Home.
 */
@Composable
fun HomeScreen(vm: MainViewModel) {
    val h = vm.home
    val st = vm.settings
    @Suppress("UNUSED_EXPRESSION") vm.settingsVersion
    if (!h.loaded) {
        EmptyNote("Loading…")
        return
    }
    val hero = h.continueWatching.firstOrNull()
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 28.dp, end = 28.dp, top = 20.dp, bottom = 40.dp),
        verticalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        if (st.dashHero) item { Hero(vm, hero) }
        if (st.dashContinue && h.continueWatching.isNotEmpty()) {
            item { PosterRow(vm, "Continue watching", h.continueWatching, resume = true) }
        }
        if (st.dashLiveFavorites && h.favoriteLive.isNotEmpty()) item { ChannelRow(vm, "Favorite channels", h.favoriteLive) }
        if (st.dashRecentLive && h.recentLive.isNotEmpty()) item { ChannelRow(vm, "Recently watched channels", h.recentLive) }
        if (st.dashFavoriteVod && h.favoriteVod.isNotEmpty()) item { PosterRow(vm, "Favorite movies and series", h.favoriteVod) }
        if (st.dashRecentlyAdded && h.addedMovies.isNotEmpty()) item { PosterRow(vm, "Recently added movies", h.addedMovies) }
        if (st.dashRecentlyAdded && h.addedSeries.isNotEmpty()) item { PosterRow(vm, "Recently added series", h.addedSeries) }
        val nothing = h.continueWatching.isEmpty() && h.favoriteLive.isEmpty() && h.recentLive.isEmpty() &&
            h.favoriteVod.isEmpty() && h.addedMovies.isEmpty() && h.addedSeries.isEmpty()
        if (nothing) {
            item {
                Text(
                    "Watch something, add favorites (hold OK) and this page fills up.",
                    color = Muted, fontSize = 14.sp, modifier = Modifier.padding(top = 12.dp),
                )
            }
        }
    }
}

@Composable
private fun Hero(vm: MainViewModel, item: Item?) {
    val first = remember { FocusRequester() }
    val rail = LocalRailFocus.current // Left from the first button goes to the rail, not down to a row
    LaunchedEffect(item?.id) {
        delay(120)
        runCatching { first.requestFocus() }
    }
    Box(Modifier.fillMaxWidth().height(250.dp).clip(RoundedCornerShape(18.dp)).background(Panel)) {
        if (item != null) Poster(item.logo, "", Modifier.fillMaxSize().alpha(0.28f))
        Row(Modifier.fillMaxSize().padding(26.dp), verticalAlignment = Alignment.CenterVertically) {
            if (item != null) {
                Poster(item.logo, item.name, Modifier.size(132.dp, 198.dp).clip(RoundedCornerShape(12.dp)))
            } else {
                Box(Modifier.size(132.dp).clip(RoundedCornerShape(28.dp)).background(Accent.copy(alpha = 0.2f)), contentAlignment = Alignment.Center) {
                    Icon(Icons.Filled.LiveTv, null, tint = Accent, modifier = Modifier.size(56.dp))
                }
            }
            Column(Modifier.padding(start = 28.dp)) {
                Text(if (item != null) "CONTINUE WATCHING" else "WELCOME BACK", color = Accent, fontSize = 12.sp, fontWeight = FontWeight.Bold)
                // Episodes: the series on a small line, then "S01E01 · title" (not the provider's
                // "Series (2006) - S01E01 - title" twice over).
                item?.takeIf { it.isEpisode }?.episodeSeries?.let {
                    Text(it, color = Muted, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
                }
                Text(
                    when {
                        item == null -> vm.source?.name ?: Brand.NAME
                        item.isEpisode -> item.episodeLabel
                        else -> item.name
                    },
                    fontSize = 28.sp, lineHeight = 34.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis,
                )
                if (item != null) {
                    Row(Modifier.padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        ProgressBar(item.watchedRatio, width = 240.dp)
                        Text("  ${(item.watchedRatio * 100).toInt()}% watched", color = Muted, fontSize = 12.sp)
                    }
                } else {
                    Text("${vm.sources.size} playlist${if (vm.sources.size == 1) "" else "s"}", color = Muted, fontSize = 14.sp)
                }
                Row(Modifier.padding(top = 18.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (item != null) {
                        HeroButton("Continue watching", Icons.Filled.PlayArrow, Modifier.focusRequester(first).focusProperties { left = rail }, primary = true) { vm.resume(item) }
                        HeroButton("Details", Icons.Filled.Info) {
                            val series = item.seriesId
                            if (item.isEpisode && series != null) vm.openSeriesOf(item) else vm.showDetails(item)
                        }
                    } else {
                        HeroButton("Live TV", Icons.Filled.LiveTv, Modifier.focusRequester(first).focusProperties { left = rail }, primary = true) { vm.selectSection(Section.Live) }
                        HeroButton("Movies", Icons.Filled.Movie) { vm.selectSection(Section.Movies) }
                        HeroButton("Series", Icons.Filled.VideoLibrary) { vm.selectSection(Section.Series) }
                    }
                }
            }
        }
    }
}

@Composable
private fun HeroButton(label: String, icon: ImageVector, modifier: Modifier = Modifier, primary: Boolean = false, onClick: () -> Unit) {
    val content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        Icon(icon, null, modifier = Modifier.size(18.dp))
        Text("  $label", fontSize = 14.sp)
    }
    if (primary) Button(onClick = onClick, modifier = modifier, content = content)
    else OutlinedButton(onClick = onClick, modifier = modifier, content = content)
}

@Composable
private fun RowTitle(title: String, count: Int) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 10.dp)) {
        Text(title, fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Text("  $count", color = Muted, fontSize = 13.sp)
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun PosterRow(vm: MainViewModel, title: String, list: List<Item>, resume: Boolean = false) {
    Column {
        RowTitle(title, list.size)
        LazyRow(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(list, key = { it.id }) { item ->
                PosterCard(vm, item, width = 132.dp, onClick = { if (resume) vm.resume(item) else vm.open(item) })
            }
        }
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun ChannelRow(vm: MainViewModel, title: String, list: List<Item>) {
    val strip = vm.settings.stripPrefixes
    val now = System.currentTimeMillis() / 1000
    Column {
        RowTitle(title, list.size)
        LazyRow(Modifier.focusRestorer(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(list, key = { it.id }) { ch ->
                Column(Modifier.width(196.dp)) {
                    Surface(
                        onClick = { vm.watchChannel(ch, list) },
                        onLongClick = { vm.toggleFavorite(ch) },
                        modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f),
                        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
                        // A tinted highlight: many logos are white and would vanish on a white card.
                        colors = ClickableSurfaceDefaults.colors(containerColor = Panel, focusedContainerColor = Color(0xFF3A4785)),
                        border = ClickableSurfaceDefaults.border(focusedBorder = androidx.tv.material3.Border(androidx.compose.foundation.BorderStroke(2.dp, TextColor))),
                    ) {
                        Box(Modifier.fillMaxSize().padding(14.dp), contentAlignment = Alignment.Center) {
                            Logo(ch, 120.dp, 64.dp, strip)
                        }
                    }
                    Spacer(Modifier.height(6.dp))
                    Text(ch.displayName(strip), fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    vm.nowNext[ch.id]?.now?.let { p ->
                        Text(p.title, color = Muted, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        ProgressBar(p.progress(now))
                    }
                }
            }
        }
    }
}
