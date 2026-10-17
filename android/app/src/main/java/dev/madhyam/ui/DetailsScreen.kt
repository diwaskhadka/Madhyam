package dev.madhyam.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.OndemandVideo
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Replay
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import dev.madhyam.DetailsInfo
import dev.madhyam.Item
import dev.madhyam.MainViewModel
import kotlinx.coroutines.delay

/**
 * Details page of a film or series, as on Windows: backdrop, poster, chips, plot, cast and
 * director, Play / Resume; for a series the season tabs and episodes with watched marks.
 */
@OptIn(ExperimentalComposeUiApi::class, ExperimentalFoundationApi::class)
@Composable
fun DetailsScreen(vm: MainViewModel) {
    val item = vm.details ?: return
    val info = vm.detailsInfo
    BackHandler { vm.closeDetails() }
    // The list's own picture is the reliable one; the details endpoint's poster is a fallback.
    val poster = item.logo.ifBlank { null } ?: info?.poster
    Box(Modifier.fillMaxSize()) {
        Poster(info?.backdrop ?: poster, "", Modifier.fillMaxSize().alpha(0.35f))
        Box(
            Modifier.fillMaxSize().background(
                Brush.horizontalGradient(0f to Background, 0.45f to Background.copy(alpha = 0.85f), 1f to Background.copy(alpha = 0.35f)),
            ),
        )
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Background)))
        // Scroll only as far as needed: TV's default keeps the focus a third down the page,
        // which would push the title off screen when the buttons get the focus.
        CompositionLocalProvider(LocalBringIntoViewSpec provides MinimalScroll) {
            LazyColumn(
                state = rememberLazyListState(),
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 40.dp, end = 40.dp, top = 32.dp, bottom = 40.dp),
                verticalArrangement = Arrangement.spacedBy(22.dp),
            ) {
                item(key = "hero") { Hero(vm, item, info, poster) }
                if (item.isSeries) item(key = "seasons") { Seasons(vm, item) }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
private val MinimalScroll = object : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float {
        val end = offset + size
        return when {
            offset >= 0 && end <= containerSize -> 0f
            offset < 0 || size > containerSize -> offset
            else -> end - containerSize
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Hero(vm: MainViewModel, item: Item, info: DetailsInfo?, poster: String?) {
    Row {
        Poster(poster, item.name, Modifier.size(200.dp, 300.dp).clip(RoundedCornerShape(14.dp)))
        Column(Modifier.padding(start = 32.dp).widthIn(max = 720.dp)) {
            Text(item.name, fontSize = 30.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            FlowRow(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                (info?.rating ?: item.rating)?.let { Chip("★ $it", Accent) }
                vm.ratings?.imdb?.let { Chip("IMDb $it", Color(0xFFF5C518)) }
                vm.ratings?.rotten?.let { Chip("Rotten Tomatoes $it", Color(0xFFFA6240)) }
                for (c in chips(item, info)) Chip(c)
            }
            val plot = info?.plot ?: item.plot
            if (!plot.isNullOrBlank()) {
                Text(plot, fontSize = 14.sp, lineHeight = 21.sp, maxLines = 6, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 14.dp))
            }
            Credit("Actors", info?.cast)
            Credit("Director", info?.director)

            // Progress line for a started film.
            if (!item.isSeries && item.isStarted) {
                Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    ProgressBar(item.watchedRatio, width = 220.dp)
                    Text(
                        "  ${timeLeft(item)} · ${(item.watchedRatio * 100).toInt()}% watched",
                        color = Muted,
                        fontSize = 12.sp,
                    )
                }
            }
            if (vm.detailsLoading) {
                Text(if (item.isSeries) "Loading details and episodes…" else "Loading details…", color = Muted, fontSize = 13.sp, modifier = Modifier.padding(top = 14.dp))
            }
            Buttons(vm, item)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Buttons(vm: MainViewModel, item: Item) {
    val first = remember { FocusRequester() }
    // Opening the page puts the focus on Play / Resume (again once the episodes are known).
    LaunchedEffect(item.id, vm.detailsLoading) {
        delay(80)
        runCatching { first.requestFocus() }
    }
    FlowRow(Modifier.padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (item.isSeries) {
            val resume = vm.resumeEpisode()
            val ep = resume ?: vm.episodes.firstOrNull()
            if (ep != null) {
                ActionButton(
                    if (resume != null) "Resume ${ep.seasonEpisode}" else "Play ${ep.seasonEpisode}",
                    Icons.Filled.PlayArrow,
                    Modifier.focusRequester(first),
                    primary = true,
                ) { vm.playEpisode(ep) }
            }
        } else {
            ActionButton(if (item.isStarted) "Resume" else "Play", Icons.Filled.PlayArrow, Modifier.focusRequester(first), primary = true) {
                vm.playMovie()
            }
            if (item.isStarted) ActionButton("Start over", Icons.Filled.Replay) { vm.playMovie(startOver = true) }
        }
        val fav = vm.isFavorite(item)
        ActionButton(
            if (fav) "Favorite" else "Add to favorites",
            if (fav) Icons.Filled.Star else Icons.Filled.StarBorder,
            if (item.isSeries && vm.episodes.isEmpty()) Modifier.focusRequester(first) else Modifier,
        ) { vm.toggleFavorite(item) }
        if (!item.isSeries) {
            ActionButton(if (item.isWatched) "Watched" else "Mark as watched", Icons.Filled.Check) { vm.toggleWatched(item) }
        }
        vm.detailsInfo?.trailer?.let { id ->
            val context = LocalContext.current
            ActionButton("Trailer", Icons.Filled.OndemandVideo) { openTrailer(context, id) }
        }
    }
}

@Composable
private fun ActionButton(label: String, icon: ImageVector, modifier: Modifier = Modifier, primary: Boolean = false, onClick: () -> Unit) {
    val content: @Composable RowScope.() -> Unit = {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
        Text("  $label", fontSize = 14.sp, maxLines = 1, softWrap = false)
    }
    val pad = PaddingValues(horizontal = 16.dp, vertical = 8.dp)
    if (primary) Button(onClick = onClick, modifier = modifier, contentPadding = pad, content = content)
    else OutlinedButton(onClick = onClick, modifier = modifier, contentPadding = pad, content = content)
}

/** Season tabs, "mark season watched" and the season's episodes. */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
private fun Seasons(vm: MainViewModel, series: Item) {
    Column {
        if (vm.episodes.isEmpty()) {
            if (!vm.detailsLoading) {
                Text("Seasons and episodes", fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
                Text("This series has no episodes on the server.", color = Muted, fontSize = 13.sp, modifier = Modifier.padding(top = 6.dp))
            }
            return@Column
        }
        val seasons = vm.episodes.map { it.season ?: "1" }.distinct()
        val current = vm.season?.takeIf { it in seasons } ?: seasons.first()
        val eps = vm.episodes.filter { (it.season ?: "1") == current }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Seasons and episodes", fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
            Text("  ${vm.episodes.size} episodes · hold OK on one to mark it watched", color = Muted, fontSize = 12.sp)
        }
        Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            LazyRow(Modifier.weight(1f, fill = false).focusRestorer(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(seasons, key = { it }) { s -> ToggleChip("Season $s", s == current) { vm.selectSeason(s) } }
            }
            val allWatched = eps.all { it.isWatched }
            OutlinedButton(
                onClick = { vm.setWatched(eps, !allWatched) },
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                modifier = Modifier.padding(start = 16.dp),
            ) {
                Text(if (allWatched) "Mark season as unwatched" else "Mark season as watched", fontSize = 12.sp)
            }
        }
        LazyRow(
            Modifier.fillMaxWidth().padding(top = 6.dp).focusRestorer(),
            horizontalArrangement = Arrangement.spacedBy(18.dp),
            contentPadding = PaddingValues(vertical = 12.dp, horizontal = 12.dp),
        ) {
            items(eps, key = { it.id }) { ep -> EpisodeCard(vm, series, ep) }
        }
    }
}

@Composable
private fun EpisodeCard(vm: MainViewModel, series: Item, ep: Item) {
    var focused by remember { mutableStateOf(false) }
    Column(Modifier.width(240.dp)) {
        Surface(
            onClick = { vm.playEpisode(ep) },
            onLongClick = { vm.toggleWatched(ep) },
            modifier = Modifier.fillMaxWidth().size(240.dp, 135.dp).onFocusChanged { focused = it.isFocused },
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(10.dp)),
            colors = ClickableSurfaceDefaults.colors(containerColor = Panel, focusedContainerColor = Panel),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1.08f),
            border = ClickableSurfaceDefaults.border(
                focusedBorder = Border(androidx.compose.foundation.BorderStroke(3.dp, Color.White), shape = RoundedCornerShape(10.dp)),
            ),
        ) {
            Box(Modifier.fillMaxSize()) {
                Poster(ep.logo.ifBlank { series.logo }, ep.name, Modifier.fillMaxSize())
                Badge("E${ep.episode ?: "?"}", Modifier.align(Alignment.TopStart).padding(7.dp))
                if (ep.durationHint > 0) Badge(DetailsInfo.clock(ep.durationHint), Modifier.align(Alignment.BottomEnd).padding(start = 7.dp, end = 7.dp, bottom = 12.dp))
                if (ep.isWatched) WatchedMark(Modifier.align(Alignment.TopEnd).padding(7.dp))
                if (ep.isStarted) ProgressBar(ep.watchedRatio, Modifier.align(Alignment.BottomCenter).padding(horizontal = 8.dp, vertical = 3.dp))
            }
        }
        Text(
            "${ep.episode ?: ""}. ${ep.name}".trimStart('.', ' '),
            fontSize = 14.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = if (focused) Color.White else Color(0xFFCCD0E0),
            modifier = Modifier.padding(top = 8.dp),
        )
        val sub = when {
            ep.isWatched -> "Watched"
            ep.isStarted -> timeLeft(ep)
            else -> ep.plot
        }
        if (!sub.isNullOrBlank()) Text(sub, color = Muted, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun Chip(text: String, color: Color = Color(0xFFCCD0E0)) {
    Text(
        text,
        fontSize = 13.sp,
        color = color,
        modifier = Modifier.background(Panel.copy(alpha = 0.85f), RoundedCornerShape(6.dp)).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

@Composable
private fun Credit(label: String, value: String?) {
    if (value.isNullOrBlank()) return
    Text(
        buildAnnotatedString {
            withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = Color.White)) { append("$label:  ") }
            append(value)
        },
        color = Muted,
        fontSize = 13.sp,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.fillMaxWidth().padding(top = 10.dp),
    )
}

/** Year, up to three genres, length, language and country (Windows ChipTexts). */
private fun chips(item: Item, info: DetailsInfo?): List<String> = buildList {
    (info?.released?.takeIf { it.length >= 4 }?.take(4) ?: item.year)?.let { add(it) }
    (info?.genre ?: item.genre)?.split(',', '/', '|')?.map { it.trim() }?.filter { it.isNotEmpty() }?.take(3)?.let { addAll(it) }
    info?.duration?.let { add(it) }
    info?.language?.let { add(if (it.length <= 3) it.uppercase() else it) }
    info?.country?.let { add(it) }
}

/** "1h 12m left". */
private fun timeLeft(item: Item): String {
    val s = (item.durationSec - item.positionSec).toLong().coerceAtLeast(0)
    return if (s >= 3600) "${s / 3600}h ${s % 3600 / 60}m left" else "${(s / 60).coerceAtLeast(1)}m left"
}

/** Play a YouTube trailer in the TV's YouTube app (or the browser when there is none). */
private fun openTrailer(context: android.content.Context, id: String) {
    val app = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("vnd.youtube:$id"))
    val web = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("https://www.youtube.com/watch?v=$id"))
    try {
        context.startActivity(app)
    } catch (_: android.content.ActivityNotFoundException) {
        try {
            context.startActivity(web)
        } catch (_: android.content.ActivityNotFoundException) {
            android.widget.Toast.makeText(context, "No app can play YouTube trailers on this device.", android.widget.Toast.LENGTH_LONG).show()
        }
    }
}
