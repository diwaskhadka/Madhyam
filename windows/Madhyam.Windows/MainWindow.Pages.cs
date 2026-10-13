using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// Full-width pages: Dashboard, Advanced search, Favorites, Recently viewed and
/// Recently added. They never host the player: opening an item jumps to its
/// section (Live TV or the movie/series page), so the video is never left
/// covering these pages.
/// </summary>
public partial class MainWindow
{
    // Advanced search filters
    private bool _searchLive = true, _searchMovies = true, _searchSeries = true;
    private bool _searchVisibleOnly = true, _searchAllSources;

    // Favorites / Recent filters
    private string _pageKind = "all";
    private bool _pageAllSources = true;

    private async Task ShowPage(int version)
    {
        var body = new StackPanel { Margin = new Thickness(26, 4, 26, 34) };
        var scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = body };
        try
        {
            switch (_mode)
            {
                case Mode.Dashboard: await BuildDashboard(body, version); break;
                case Mode.Search: await BuildSearch(body, version); break;
                case Mode.Favorites: await BuildFavorites(body, version); break;
                case Mode.Recent: await BuildRecent(body, version); break;
                case Mode.Added: await BuildAdded(body, version); break;
            }
        }
        catch (Exception ex)
        {
            Repository.Log(ex.ToString());
            SetStatus(Explain(ex));
        }
        if (version != _loadVersion) return;
        ElasticScroll.Attach(scroll);
        PageHost.Content = scroll;
    }

    private void Open(ItemVm it)
    {
        if (it.IsLive) _ = GoToLive(it);
        else _ = GoToVod(it);
    }

    private bool Matches(ItemVm it)
    {
        var q = SearchBox.Text.Trim();
        return q.Length == 0 || it.Name.Contains(q, StringComparison.CurrentCultureIgnoreCase) || it.Group.Contains(q, StringComparison.CurrentCultureIgnoreCase);
    }

    private static FrameworkElement PageTitle(string title, string? subtitle, FrameworkElement? right = null)
    {
        var g = new Grid { Margin = new Thickness(0, 10, 0, 6) };
        var left = new StackPanel();
        left.Children.Add(Text(title, 26, "Fg", FontWeights.SemiBold));
        if (subtitle != null)
        {
            var s = Text(subtitle, 13, "Muted");
            s.Margin = new Thickness(0, 3, 0, 0);
            left.Children.Add(s);
        }
        g.Children.Add(left);
        if (right != null)
        {
            right.HorizontalAlignment = HorizontalAlignment.Right;
            right.VerticalAlignment = VerticalAlignment.Center;
            g.Children.Add(right);
        }
        return g;
    }

    private FrameworkElement NoSource() =>
        WithButton(Empty("", "No playlist yet", "Add an Xtream Codes account or an M3U playlist from your provider to start watching."),
            Primary("Add playlist", "", OpenAddSource));

    private static FrameworkElement WithButton(FrameworkElement empty, Button b)
    {
        var s = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center };
        s.Children.Add(empty);
        b.HorizontalAlignment = HorizontalAlignment.Center;
        b.Margin = new Thickness(0, 20, 0, 0);
        s.Children.Add(b);
        return s;
    }

    /// <summary>The series' own poster; falls back to the details endpoint's cover when the list had none.</summary>
    private async Task<string?> SeriesPoster(ItemVm series)
    {
        if (!string.IsNullOrWhiteSpace(series.Logo)) return series.Logo;
        try { return (await _repo.SeriesDetails(series))?.Poster; }
        catch { return null; }
    }

    private static string ProgressSubtitle(ItemVm it)
    {
        var left = it.TimeLeftText;
        if (it.Kind == "episode") return left.Length > 0 ? $"{SE(it)} · {left}" : SE(it);
        return left;
    }

    // ================================================================ dashboard

    private async Task BuildDashboard(StackPanel body, int version)
    {
        if (_source == null)
        {
            body.Children.Add(NoSource());
            return;
        }
        var cont = (await _repo.ContinueWatching(20)).Where(i => !i.IsLive && i.DurationSec > 0 && !i.IsWatched).ToList();
        if (version != _loadVersion) return;

        if (S.DashHero) body.Children.Add(await DashboardHero(cont.FirstOrDefault()));

        if (S.DashContinue && cont.Count > 0)
        {
            body.Children.Add(SectionHeader("Continue watching", cont.Count, () => NavButton(Mode.Recent).IsChecked = true, "History"));
            var shown = new List<ItemVm>();
            foreach (var i in cont)
            {
                if (i.Kind == "episode" && i.SeriesId != null && await _repo.Item(i.SeriesId) is { } sr)
                    shown.Add(i.WithLogo(await SeriesPoster(sr) ?? i.Logo));
                else shown.Add(i);
            }
            body.Children.Add(Rail(shown.Select(i => Poster(i, x => _ = GoToVod(x, play: true), ProgressSubtitle(i)))));
        }

        if (S.DashLiveFavorites)
        {
            var favLive = await _repo.Favorites("live");
            if (favLive.Count > 0)
            {
                await UpdateNowNext(favLive);
                body.Children.Add(SectionHeader("Favorite channels", favLive.Count, () => { _pageKind = "live"; NavButton(Mode.Favorites).IsChecked = true; }));
                body.Children.Add(Rail(favLive.Take(30).Select(i => LiveCard(i, Open))));
            }
        }

        if (S.DashRecentLive)
        {
            var recentLive = await _repo.Recent("live", 20);
            if (recentLive.Count > 0)
            {
                await UpdateNowNext(recentLive);
                body.Children.Add(SectionHeader("Recently watched channels", recentLive.Count, () => { _pageKind = "live"; NavButton(Mode.Recent).IsChecked = true; }));
                body.Children.Add(Rail(recentLive.Select(i => LiveCard(i, Open))));
            }
        }

        if (S.DashFavoriteVod)
        {
            var favVod = (await _repo.Favorites(null)).Where(i => !i.IsLive).ToList();
            if (favVod.Count > 0)
            {
                body.Children.Add(SectionHeader("Favorite movies and series", favVod.Count, () => { _pageKind = "vod"; NavButton(Mode.Favorites).IsChecked = true; }));
                body.Children.Add(Rail(favVod.Take(30).Select(i => Poster(i, Open, i.Year, typeBadge: true))));
            }
        }

        if (S.DashRecentlyAdded)
        {
            foreach (var (kind, label) in new[] { ("movie", "Recently added movies"), ("series", "Recently added series") })
            {
                var (total, items) = await _repo.Query(kind, _source.Id, null, S.Hidden(_source.Id, kind), "added_desc", 0, 0, 20);
                if (version != _loadVersion) return;
                if (items.Count == 0 || items.All(i => i.AddedAt == 0)) continue;
                body.Children.Add(SectionHeader(label, total, () => NavButton(Mode.Added).IsChecked = true, "Browse all"));
                body.Children.Add(Rail(items.Select(i => Poster(i, Open, i.AddedText))));
            }
        }

        if (_sources.Count > 1)
        {
            body.Children.Add(SectionHeader("Your playlists", _sources.Count, () => OpenSettings("Sources"), "Manage"));
            body.Children.Add(Rail(_sources.Select(SourceCard)));
        }
    }

    /// <summary>Big banner: the film or episode to continue, or a welcome card with shortcuts.</summary>
    private async Task<FrameworkElement> DashboardHero(ItemVm? it)
    {
        var grid = new Grid { Height = 300, Margin = new Thickness(0, 8, 0, 4) };
        var clip = new Border { CornerRadius = new CornerRadius(18), Background = B("Panel"), ClipToBounds = true };
        grid.Children.Add(clip);
        var content = new Grid { Margin = new Thickness(30, 26, 30, 26) };
        content.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        content.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.Children.Add(content);
        var right = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(28, 0, 0, 0) };
        Grid.SetColumn(right, 1);
        content.Children.Add(right);

        if (it != null)
        {
            string? poster = it.Logo;
            string? title = it.Name;
            if (it.Kind == "episode" && it.SeriesId != null && await _repo.Item(it.SeriesId) is { } series)
            {
                title = $"{series.Name}";
                poster = await SeriesPoster(series) ?? poster;
            }
            clip.Child = Backdrop(UiKit.Image(poster, 700), 0.45);
            content.Children.Add(PosterImage(poster, 168, 248));
            right.Children.Add(Text("CONTINUE WATCHING", 11.5, "Accent", FontWeights.Bold));
            var t = Text(title ?? "", 30, "Fg", FontWeights.SemiBold, wrap: true);
            t.Margin = new Thickness(0, 6, 0, 0);
            t.MaxHeight = 84;
            right.Children.Add(t);
            if (it.Kind == "episode") right.Children.Add(Text(EpisodeLabel(title ?? "", it), 14, "Muted"));
            var row = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 16, 0, 0) };
            row.Children.Add(new ProgressBar { Style = St("ThinProgress"), Value = it.WatchedRatio, Width = 260, VerticalAlignment = VerticalAlignment.Center });
            var l = Text($"{it.TimeLeftText} · {it.WatchedRatio:P0} watched", 12.5, "Muted");
            l.Margin = new Thickness(12, 0, 0, 0);
            row.Children.Add(l);
            right.Children.Add(row);
            var buttons = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 22, 0, 0) };
            buttons.Children.Add(Primary("Continue watching", "", () => _ = GoToVod(it, play: true)));
            buttons.Children.Add(Primary("Details", "", () => _ = GoToVod(it.Kind == "episode" && it.SeriesId != null
                ? new ItemVm { Id = it.SeriesId, Kind = "series", Name = title ?? "", SourceId = it.SourceId } : it), secondary: true));
            right.Children.Add(buttons);
        }
        else
        {
            clip.Child = Backdrop(null);
            var icon = new Border
            {
                Width = 120,
                Height = 120,
                CornerRadius = new CornerRadius(28),
                Background = B("RailActive"),
                VerticalAlignment = VerticalAlignment.Center,
                Child = new TextBlock { Text = "", FontFamily = Icons, FontSize = 46, Foreground = B("Accent"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center },
            };
            content.Children.Add(icon);
            right.Children.Add(Text("WELCOME BACK", 11.5, "Accent", FontWeights.Bold));
            var t = Text(_source?.Name ?? "Madhyam", 30, "Fg", FontWeights.SemiBold);
            t.Margin = new Thickness(0, 6, 0, 0);
            right.Children.Add(t);
            right.Children.Add(Text(PlaylistCount.Text, 14, "Muted"));
            var buttons = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 22, 0, 0) };
            buttons.Children.Add(Primary("Live TV", "", () => NavLive.IsChecked = true));
            buttons.Children.Add(Primary("Movies", "", () => NavMovies.IsChecked = true, secondary: true));
            buttons.Children.Add(Primary("Series", "", () => NavSeries.IsChecked = true, secondary: true));
            right.Children.Add(buttons);
        }
        return grid;
    }

    private FrameworkElement SourceCard(Source s)
    {
        var active = s.Id == _source?.Id;
        var stack = new StackPanel { Width = 230 };
        var top = new StackPanel { Orientation = Orientation.Horizontal };
        top.Children.Add(new Border
        {
            Width = 36,
            Height = 36,
            CornerRadius = new CornerRadius(10),
            Background = B(active ? "RailActive" : "Chip"),
            Child = new TextBlock { Text = s.Kind == "xtream" ? "" : "", FontFamily = Icons, FontSize = 15, Foreground = B(active ? "Accent" : "Fg"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center },
        });
        var names = new StackPanel { Margin = new Thickness(10, 0, 0, 0), VerticalAlignment = VerticalAlignment.Center };
        names.Children.Add(Text(s.Name, 14, "Fg", FontWeights.SemiBold));
        names.Children.Add(Text(active ? $"{s.KindLabel} · in use" : s.KindLabel, 11.5, active ? "Accent" : "Muted"));
        top.Children.Add(names);
        stack.Children.Add(top);
        if (s.UpdatedAt > 0)
        {
            var u = Text($"Updated {DateTimeOffset.FromUnixTimeSeconds(s.UpdatedAt).LocalDateTime:d MMM, HH:mm}", 11.5, "Muted");
            u.Margin = new Thickness(0, 10, 0, 0);
            stack.Children.Add(u);
        }
        var border = new Border { CornerRadius = new CornerRadius(12), Background = B("Card"), Padding = new Thickness(14), Child = stack, BorderThickness = new Thickness(1.5), BorderBrush = active ? B("Accent") : Brushes.Transparent };
        var card = new Button { Content = border, Template = BareTemplate(), Cursor = System.Windows.Input.Cursors.Hand, Focusable = false, Margin = new Thickness(0, 0, 12, 12) };
        card.Click += (_, _) => _ = SwitchSource(s);
        return card;
    }

    // ================================================================ advanced search

    private async Task BuildSearch(StackPanel body, int version)
    {
        var q = SearchBox.Text.Trim();
        var filters = new WrapPanel { Margin = new Thickness(0, 12, 0, 4) };
        void Rerun() => _ = ShowPage(++_loadVersion);
        filters.Children.Add(Filter("Live TV", _searchLive, v => { _searchLive = v; Rerun(); }));
        filters.Children.Add(Filter("Movies", _searchMovies, v => { _searchMovies = v; Rerun(); }));
        filters.Children.Add(Filter("Series", _searchSeries, v => { _searchSeries = v; Rerun(); }));
        filters.Children.Add(new Border { Width = 1, Height = 18, Background = B("Line"), Margin = new Thickness(4, 0, 22, 0) });
        filters.Children.Add(Filter("Visible categories only", _searchVisibleOnly, v => { _searchVisibleOnly = v; Rerun(); }));
        if (_sources.Count > 1) filters.Children.Add(Filter("All playlists", _searchAllSources, v => { _searchAllSources = v; Rerun(); }));

        if (q.Length == 0)
        {
            body.Children.Add(PageTitle("Advanced search", "Search channels, movies and series at once."));
            body.Children.Add(filters);
            body.Children.Add(Empty("", "Type to search", "Use the search box at the top. Press Enter there from any page to come here."));
            return;
        }

        var kinds = new List<string>();
        if (_searchLive) kinds.Add("live");
        if (_searchMovies) kinds.Add("movie");
        if (_searchSeries) kinds.Add("series");
        var hits = new List<ItemVm>();
        foreach (var k in kinds) hits.AddRange(await _repo.Search(q, k));
        if (version != _loadVersion) return;
        if (!_searchAllSources && _source != null) hits = hits.Where(h => h.SourceId == _source.Id).ToList();
        if (_searchVisibleOnly)
            hits = hits.Where(h => !S.Hidden(h.SourceId, h.Kind).Contains(h.Group)).ToList();
        foreach (var h in hits) h.IsFavorite = _favorites.Contains(h.Id);

        body.Children.Add(PageTitle($"Results for “{q}”", hits.Count == 0 ? "Nothing found." : $"Found {hits.Count:N0} results"));
        body.Children.Add(filters);
        if (hits.Count == 0)
        {
            body.Children.Add(Empty("", "No results", "Try fewer words, or turn on more types above."));
            return;
        }
        var live = hits.Where(h => h.IsLive).ToList();
        if (live.Count > 0)
        {
            await UpdateNowNext(live);
            body.Children.Add(SectionHeader("Live TV", live.Count));
            body.Children.Add(Wrap(live.Take(60).Select(i => LiveCard(i, Open))));
        }
        foreach (var (kind, label) in new[] { ("movie", "Movies"), ("series", "Series") })
        {
            var part = hits.Where(h => h.Kind == kind).ToList();
            if (part.Count == 0) continue;
            body.Children.Add(SectionHeader(label, part.Count));
            body.Children.Add(Wrap(part.Take(200).Select(i => Poster(i, Open, i.Group))));
        }
    }

    // ================================================================ favorites / recent

    private FrameworkElement KindAndSourceBar(bool withLive = true)
    {
        var bar = new WrapPanel { Margin = new Thickness(0, 12, 0, 4) };
        var kinds = new List<(string, string)> { ("all", "All") };
        if (withLive) kinds.Add(("live", "Live TV"));
        kinds.Add(("movie", "Movies"));
        kinds.Add(("series", "Series"));
        var cur = _pageKind == "vod" ? "all" : _pageKind;
        bar.Children.Add(Segments(kinds, cur, v => { _pageKind = v; _ = ShowPage(++_loadVersion); }));
        if (_sources.Count > 1)
        {
            var src = Segments(new[] { ("this", "This playlist"), ("all", "All playlists") }, _pageAllSources ? "all" : "this",
                v => { _pageAllSources = v == "all"; _ = ShowPage(++_loadVersion); });
            src.Margin = new Thickness(24, 0, 0, 0);
            bar.Children.Add(src);
        }
        return bar;
    }

    private List<ItemVm> PageFilter(IEnumerable<ItemVm> items) =>
        items.Where(i => _pageKind switch
            {
                "live" => i.IsLive,
                "movie" => i.Kind == "movie",
                "series" => i.Kind is "series" or "episode",
                "vod" => !i.IsLive,
                _ => true,
            })
            .Where(i => _pageAllSources || _source == null || i.SourceId == _source.Id)
            .Where(Matches)
            .ToList();

    private async Task BuildFavorites(StackPanel body, int version)
    {
        var all = await _repo.Favorites(null);
        if (version != _loadVersion) return;
        var items = PageFilter(all);
        body.Children.Add(PageTitle("Favorites", all.Count == 0 ? null : $"{items.Count:N0} of {all.Count:N0}"));
        body.Children.Add(KindAndSourceBar());
        if (items.Count == 0)
        {
            body.Children.Add(Empty("", all.Count == 0 ? "No favorites yet" : "Nothing here",
                all.Count == 0 ? "Use the star next to a channel, or “Add to favorites” on a movie or series." : "Try another filter."));
            return;
        }
        async void Remove(ItemVm it)
        {
            await ToggleFavorite(it);
        }
        await AddLiveAndVod(body, items, Remove, i => i.Group);
    }

    private async Task BuildRecent(StackPanel body, int version)
    {
        var all = await _repo.Recent(null, 200);
        if (version != _loadVersion) return;
        var items = PageFilter(all);
        Button? clear = null;
        if (all.Count > 0)
        {
            clear = Primary("Clear history", "", async () =>
            {
                if (MessageBox.Show(this, "Remove everything from Recently viewed?\n\nFavorites and resume positions are kept.", "Madhyam", MessageBoxButton.YesNo) != MessageBoxResult.Yes) return;
                await _repo.ClearHistory();
                await ShowPage(++_loadVersion);
            }, secondary: true);
        }
        body.Children.Add(PageTitle("Recently viewed", all.Count == 0 ? null : $"{items.Count:N0} items", clear));
        body.Children.Add(KindAndSourceBar());
        if (items.Count == 0)
        {
            body.Children.Add(Empty("", all.Count == 0 ? "Nothing watched yet" : "Nothing here",
                all.Count == 0 ? "Channels, movies and episodes you play appear here." : "Try another filter."));
            return;
        }
        async void Remove(ItemVm it)
        {
            await _repo.RemoveFromHistory(it.Id);
            await ShowPage(++_loadVersion);
        }
        await AddLiveAndVod(body, items, Remove, i => i.PlayedText);
    }

    private async Task AddLiveAndVod(StackPanel body, List<ItemVm> items, Action<ItemVm> remove, Func<ItemVm, string> subtitle)
    {
        var live = items.Where(i => i.IsLive).ToList();
        var vod = items.Where(i => !i.IsLive).ToList();
        if (live.Count > 0)
        {
            await UpdateNowNext(live);
            body.Children.Add(SectionHeader("Live TV", live.Count));
            body.Children.Add(Wrap(live.Select(i => LiveCard(i, Open, remove))));
        }
        foreach (var (kinds, label) in new[] { (new[] { "movie" }, "Movies"), (new[] { "series", "episode" }, "Series") })
        {
            var part = vod.Where(i => kinds.Contains(i.Kind)).ToList();
            if (part.Count == 0) continue;
            body.Children.Add(SectionHeader(label, part.Count));
            body.Children.Add(Wrap(part.Select(i => Poster(i, Open,
                i.Kind == "episode" ? ProgressSubtitle(i) : subtitle(i), typeBadge: i.Kind == "episode", remove: remove))));
        }
    }

    // ================================================================ recently added

    private async Task BuildAdded(StackPanel body, int version)
    {
        if (_source == null)
        {
            body.Children.Add(NoSource());
            return;
        }
        body.Children.Add(PageTitle("Recently added", $"Newest first in {_source.Name}"));
        var any = false;
        foreach (var (kind, label, mode) in new[] { ("movie", "Movies", Mode.Movies), ("series", "Series", Mode.Series), ("live", "Live TV", Mode.Live) })
        {
            var (total, items) = await _repo.Query(kind, _source.Id, null, S.Hidden(_source.Id, kind), "added_desc", 0, 0, 30);
            if (version != _loadVersion) return;
            items = items.Where(i => i.AddedAt > 0).ToList();
            if (items.Count == 0) continue;
            any = true;
            var m = mode;
            body.Children.Add(SectionHeader(label, total, () =>
            {
                if (m != Mode.Live)
                {
                    S.VodSort = "added_desc";
                    AppSettings.Save();
                }
                NavButton(m).IsChecked = true;
            }, "Browse all"));
            if (kind == "live")
            {
                await UpdateNowNext(items);
                body.Children.Add(Rail(items.Select(i => LiveCard(i, Open))));
            }
            else
            {
                foreach (var i in items) i.IsFavorite = _favorites.Contains(i.Id);
                body.Children.Add(Rail(items.Select(i => Poster(i, Open, i.AddedText))));
            }
        }
        if (!any)
            body.Children.Add(Empty("", "No dates from this provider",
                "Recently added uses the dates an Xtream server sends. M3U playlists do not include them."));
    }
}
