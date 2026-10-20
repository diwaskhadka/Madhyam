using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// Movies and series: the paged poster grid (sort, minimum rating, per page),
/// the details page (backdrop, poster, chips, plot, cast, trailer, seasons and
/// episodes) and the in-page player with "About" and episodes below it.
/// </summary>
public partial class MainWindow
{
    private ItemVm? _details;               // the film or series whose page is open
    private DetailsInfo? _detailsInfo;
    private ExternalRatings? _ratings;
    private bool _detailsLoading;
    private List<ItemVm> _episodes = new();
    private string? _season;
    private bool _progressStale;            // something was played: re-read progress when the player closes
    private string? _backdropKey;

    private int _perPage = 50;
    private int _page;
    private int _gridTotal;
    private string? _vodSearch;
    private List<ItemVm>? _searchHits;

    // ================================================================ grid

    private static readonly (string Value, string Label)[] SortOptions =
    {
        ("provider", "Provider order"),
        ("added_desc", "Date added (latest)"),
        ("added_asc", "Date added (oldest)"),
        ("name_asc", "Name A–Z"),
        ("name_desc", "Name Z–A"),
        ("rating_desc", "Top rated"),
        ("rating_asc", "Lowest rated"),
    };

    private void HighlightPerPage()
    {
        foreach (var b in new[] { Per25, Per50, Per100 })
        {
            var on = b.Tag is string t && t == _perPage.ToString();
            b.Foreground = (Brush)FindResource(on ? "Accent" : "Muted");
            b.FontWeight = on ? FontWeights.Bold : FontWeights.Normal;
        }
    }

    private async Task LoadGrid(bool resetPage)
    {
        if (resetPage) _page = 0;
        var kind = KindNow;
        SortLabel.Text = SortOptions.FirstOrDefault(o => o.Value == S.VodSort).Label is { } l ? l : "Provider order";
        RefineLabel.Text = S.VodMinRating > 0 ? $"Rating {S.VodMinRating:0}+" : "Any rating";
        if (_source == null)
        {
            PosterGrid.ItemsSource = null;
            GridTitle.Text = "No playlist";
            GridCount.Text = "Add a playlist with the + button to start.";
            UpdatePager();
            return;
        }
        var version = _loadVersion;
        List<ItemVm> items;
        if (_vodSearch != null)
        {
            // Search results are paged on this side (the core returns the best 300 hits).
            if (_searchHits == null)
            {
                var hits = await _repo.Search(_vodSearch, kind);
                if (version != _loadVersion) return;
                var hidden = HiddenNow;
                _searchHits = hits.Where(h => h.SourceId == _source.Id && !hidden.Contains(h.Group) && !Parental.HideNow(h)
                                              && (S.VodMinRating <= 0 || (double.TryParse(h.Rating, System.Globalization.NumberStyles.Float,
                                                  System.Globalization.CultureInfo.InvariantCulture, out var r) && r >= S.VodMinRating))).ToList();
            }
            _gridTotal = _searchHits.Count;
            items = _searchHits.Skip(_page * _perPage).Take(_perPage).ToList();
            GridTitle.Text = $"Results for “{_vodSearch}”";
        }
        else
        {
            var (total, page) = await _repo.Query(kind, _source.Id, _group, _group == null ? HiddenOrLockedNow : null,
                S.VodSort, S.VodMinRating, _page * _perPage, _perPage);
            if (version != _loadVersion) return;
            _gridTotal = total;
            items = page;
            GridTitle.Text = _group ?? "All items";
        }
        foreach (var it in items) it.IsFavorite = _favorites.Contains(it.Id);
        PosterGrid.ItemsSource = items;
        GridCount.Text = _gridTotal == 0
            ? (_vodSearch != null ? "Nothing matches." : "No items here.")
            : $"{_gridTotal:N0} {(kind == "series" ? "series" : "movies")}";
        GridScroll.ScrollToTop();
        UpdatePager();
    }

    private int PageCount => Math.Max(1, (int)Math.Ceiling(_gridTotal / (double)_perPage));

    private void UpdatePager()
    {
        PageText.Text = $"{_page + 1} / {PageCount}";
        PrevPage.IsEnabled = _page > 0;
        NextPage.IsEnabled = _page + 1 < PageCount;
    }

    private async Task SearchVod(string q)
    {
        CloseDetailsSilently();
        _vodSearch = q.Length == 0 ? null : q;
        _searchHits = null;
        ++_loadVersion;
        await LoadGrid(resetPage: true);
    }

    private async void Poster_Click(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is ItemVm it) await ShowDetails(it);
    }

    private static string SortGlyph(string v) => v switch
    {
        "provider" => "\uE8FD",
        "added_desc" => "\uE787",
        "added_asc" => "\uE787",
        "name_asc" => "\uE74A",
        "name_desc" => "\uE74B",
        "rating_desc" => "\uE735",
        _ => "\uE734",
    };

    private void Sort_Click(object sender, RoutedEventArgs e)
    {
        UiKit.Menu(SortBtn, SortOptions.Select(o => (SortGlyph(o.Value), o.Label, o.Value == S.VodSort, (Action)(() =>
        {
            S.VodSort = o.Value;
            AppSettings.Save();
            _searchHits = null;
            _ = LoadGrid(resetPage: true);
        }))), "Sort by");
    }

    private void Refine_Click(object sender, RoutedEventArgs e)
    {
        var options = new (double Value, string Label)[] { (0, "Any rating"), (9, "9 and up"), (8, "8 and up"), (7, "7 and up"), (6, "6 and up"), (5, "5 and up") };
        UiKit.Menu(RefineBtn, options.Select(o => (o.Value == 0 ? "\uE71C" : "\uE735", o.Label, Math.Abs(o.Value - S.VodMinRating) < 0.01, (Action)(() =>
        {
            S.VodMinRating = o.Value;
            AppSettings.Save();
            _searchHits = null;
            _ = LoadGrid(resetPage: true);
        }))), "Minimum rating");
    }

    private async void PerPage_Click(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is not string t || !int.TryParse(t, out var n)) return;
        _perPage = n;
        S.PerPage = n;
        AppSettings.Save();
        HighlightPerPage();
        await LoadGrid(resetPage: true);
    }

    private async void PrevPage_Click(object sender, RoutedEventArgs e)
    {
        if (_page == 0) return;
        _page--;
        await LoadGrid(resetPage: false);
    }

    private async void NextPage_Click(object sender, RoutedEventArgs e)
    {
        if (_page + 1 >= PageCount) return;
        _page++;
        await LoadGrid(resetPage: false);
    }

    // ================================================================ details page

    /// <summary>Open a film or series page: shows at once, then fills in details and episodes.</summary>
    private async Task ShowDetails(ItemVm it)
    {
        CloseTrailer();
        if (_vodPlaying) StopPlayback();
        var fresh = await _repo.Item(it.Id) ?? it;
        if (!await ParentalAllows(fresh)) return;
        fresh.IsFavorite = _favorites.Contains(fresh.Id);
        _details = fresh;
        _detailsInfo = null;
        _ratings = null;
        _episodes = new List<ItemVm>();
        _season = null;
        _detailsLoading = true;
        Layout();
        VodHeaderTitle.Text = fresh.DisplayName;
        RenderVodPage();
        VodScroll.ScrollToTop();

        DetailsInfo? info = null;
        var eps = new List<ItemVm>();
        try
        {
            info = fresh.IsSeries ? await _repo.SeriesDetails(fresh) : await _repo.MovieDetails(fresh);
        }
        catch (Exception ex)
        {
            Repository.Log($"details failed for {fresh.Name}: {ex.Message}");
        }
        if (fresh.IsSeries)
        {
            try { eps = await _repo.Episodes(fresh); }
            catch (Exception ex) { SetStatus($"Episodes could not be loaded: {Explain(ex)}"); }
        }
        if (_details != fresh) return;
        _detailsInfo = info;
        _episodes = eps;
        _season = ResumeEpisode()?.Season ?? eps.FirstOrDefault()?.Season;
        _detailsLoading = false;
        RenderVodPage();

        // IMDb / Rotten Tomatoes (only shown when found).
        var year = fresh.Year ?? (System.Text.RegularExpressions.Regex.Match(fresh.Name, @"\b(19|20)\d{2}\b") is { Success: true } ym ? ym.Value : null);
        var ext = await Ratings.Fetch(Ratings.CleanTitle(fresh.Name), year, info?.ImdbId, fresh.IsSeries);
        if (_details != fresh || ext == null) return;
        _ratings = ext;
        RenderVodPage();
    }

    /// <summary>
    /// Parental controls before a film or series page opens: its category or name may be
    /// locked, and with a rating limit its age rating (from the details) may be above it.
    /// Titles without an age rating are allowed.
    /// </summary>
    private async Task<bool> ParentalAllows(ItemVm it)
    {
        if (!Parental.On) return true;
        if (Parental.IsItemLocked(it) && !Parental.Ensure($"“{it.Name}” is locked.")) return false;
        if (S.ParentalMaxAge <= 0 || Parental.Unlocked) return true;
        DetailsInfo? info = null;
        try { info = it.IsSeries ? await _repo.SeriesDetails(it) : await _repo.MovieDetails(it); }
        catch (Exception ex) { Repository.Log($"age rating lookup failed for {it.Name}: {ex.Message}"); }
        var age = Parental.AgeOf(info?.AgeRating);
        return !Parental.AboveLimit(age) || Parental.Ensure($"“{it.Name}” is rated {age}+, above the limit of {S.ParentalMaxAge}.");
    }

    private void CloseDetails()
    {
        CloseDetailsSilently();
        UpdateSearchPill();
    }

    /// <summary>Leave the details page (and its player) without touching the grid.</summary>
    private void CloseDetailsSilently()
    {
        CloseTrailer();
        if (_details == null) return;
        _details = null;
        if (_vodPlaying) StopPlayback();
        _detailsInfo = null;
        _episodes = new List<ItemVm>();
        VodContent.Children.Clear();
        VodBackdrop.Content = null;
        _backdropKey = null;
        Layout();
    }

    private void VodBack_Click(object sender, RoutedEventArgs e)
    {
        // From the player or trailer go back to the details; from the details go back to the grid.
        if (_trailerOpen) CloseTrailer();
        else if (_vodPlaying) StopPlayback();
        else CloseDetails();
    }

    /// <summary>Episode to offer as "Resume": started but unfinished, else the first unwatched one.</summary>
    private ItemVm? ResumeEpisode() =>
        _episodes.Where(e => e.PositionSec > 0 && !e.IsWatched).OrderByDescending(e => e.PlayedAt).FirstOrDefault()
        ?? (_episodes.Any(e => e.IsWatched) ? _episodes.FirstOrDefault(e => !e.IsWatched) : null);

    private async Task ReloadDetailsProgress(int delayMs = 300)
    {
        if (_details is not { } d) return;
        if (delayMs > 0) await Task.Delay(delayMs); // let the player's last progress write land
        var fresh = await _repo.Item(d.Id);
        var eps = d.IsSeries ? await _repo.Episodes(d) : _episodes;
        if (_details != d || fresh == null) return;
        fresh.IsFavorite = _favorites.Contains(fresh.Id);
        _details = fresh;
        _episodes = eps;
        RenderVodPage();
    }

    private async Task PlayMovie(bool startOver = false)
    {
        CloseTrailer();
        if (_details is not { IsSeries: false } it) return;
        if (startOver) await _repo.SetProgress(it.Id, 0, Math.Max(1, it.DurationSec));
        _playlist = new List<ItemVm> { it };
        _progressStale = true;
        await PlayAt(0);
        VodScroll.ScrollToTop();
    }

    private async Task PlayEpisode(ItemVm ep)
    {
        CloseTrailer();
        _playlist = _episodes.ToList();
        var i = _playlist.FindIndex(e => e.Id == ep.Id);
        if (i < 0)
        {
            _playlist = new List<ItemVm> { ep };
            i = 0;
        }
        _season = ep.Season;
        _progressStale = true;
        await PlayAt(i);
        VodScroll.ScrollToTop();
    }

    /// <summary>Open a film, series or episode from anywhere (dashboard, search, favorites, recent).</summary>
    public async Task GoToVod(ItemVm it, bool play = false)
    {
        if (Parental.IsItemLocked(it) && !Parental.Ensure($"“{it.Name}” is locked.")) return;
        ItemVm? episode = null;
        var target = it;
        if (it.Kind == "episode")
        {
            episode = it;
            if (it.SeriesId != null && await _repo.Item(it.SeriesId) is { } series) target = series;
        }
        StopPlayback();
        SearchBox.Text = "";
        _searchDebounce.Stop();
        _vodSearch = null;
        _searchHits = null;
        SetNav(target.IsSeries || target.Kind == "episode" ? Mode.Series : Mode.Movies);
        _details = null;
        _group = null;
        if (_source?.Id != target.SourceId && _sources.FirstOrDefault(s => s.Id == target.SourceId) is { } src)
        {
            _source = src;
            await UpdatePlaylistHeader();
        }
        Layout();
        await LoadCategories(++_loadVersion);
        if (target.Kind == "episode")
        {
            // Series item not found: play the episode on its own page.
            _details = target;
            _detailsInfo = null;
            _episodes = new List<ItemVm> { target };
            Layout();
            await PlayEpisode(target);
            return;
        }
        await ShowDetails(target);
        if (_details?.Id != target.Id) return;
        if (episode != null) await PlayEpisode(_episodes.FirstOrDefault(e => e.Id == episode.Id) ?? episode);
        else if (play && !target.IsSeries) await PlayMovie();
        else if (play && ResumeEpisode() is { } next) await PlayEpisode(next);
    }

    // ================================================================ page rendering

    /// <summary>(Re)build the content under the header: hero (details) or About (while playing), plus episodes.</summary>
    private void RenderVodPage()
    {
        VodContent.Children.Clear();
        if (_details is not { } it) return;
        if (!_vodPlaying && _progressStale)
        {
            _progressStale = false;
            _ = ReloadDetailsProgress();
        }
        var info = _detailsInfo;
        // The list's own image is the reliable one; the details endpoint's poster is a fallback.
        var poster = it.Logo ?? info?.Poster;

        var key = info?.Backdrop ?? poster ?? "";
        if (key != _backdropKey)
        {
            _backdropKey = key;
            VodBackdrop.Content = Backdrop(UiKit.Image(info?.Backdrop ?? poster, 800), 0.4);
        }

        if (!_vodPlaying)
        {
            VodHeaderTitle.Text = "";
            VodContent.Children.Add(Hero(it, info, poster));
        }
        else
        {
            if (Playing is { Kind: "episode" } ep && it.IsSeries) VodContent.Children.Add(NowPlayingEpisode(ep));
            VodContent.Children.Add(SectionHeader("About"));
            VodContent.Children.Add(About(it, info, poster));
        }

        // Seasons and episodes first, the trailer after them.
        if (it.IsSeries) VodContent.Children.Add(Seasons(it));
        if (info?.Trailer is { } yt) VodContent.Children.Add(Trailer(it, yt));
        if (_detailsLoading)
        {
            var t = Text(it.IsSeries ? "Loading details and episodes…" : "Loading details…", 13, "Muted");
            t.Margin = new Thickness(0, 18, 0, 0);
            VodContent.Children.Add(t);
        }
    }

    /// <summary>IMDb and Rotten Tomatoes chips, only when OMDb had a score.</summary>
    private void AddExternalRatings(Panel chips)
    {
        if (_ratings is not { } r) return;
        if (r.Imdb != null)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal };
            row.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(4),
                Background = new SolidColorBrush(Color.FromRgb(0xF5, 0xC5, 0x18)),
                Padding = new Thickness(5, 0, 5, 1),
                Margin = new Thickness(0, 0, 7, 0),
                VerticalAlignment = VerticalAlignment.Center,
                Child = new TextBlock { Text = "IMDb", FontSize = 11, FontWeight = FontWeights.Black, Foreground = Brushes.Black },
            });
            row.Children.Add(new TextBlock { Text = $"{r.Imdb}/10", FontSize = 13, FontWeight = FontWeights.SemiBold, Foreground = B("Fg"), VerticalAlignment = VerticalAlignment.Center });
            chips.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(1), BorderBrush = B("Line"), Background = B("Card"),
                Padding = new Thickness(6, 3, 10, 3), Margin = new Thickness(0, 0, 8, 8), Child = row,
                ToolTip = r.ImdbVotes != null ? $"IMDb · {r.ImdbVotes} votes" : "IMDb",
            });
        }
        if (r.Rotten != null)
        {
            var fresh = int.TryParse(r.Rotten.TrimEnd('%'), out var pct) && pct >= 60;
            var row = new StackPanel { Orientation = Orientation.Horizontal };
            row.Children.Add(new Ellipse { Width = 12, Height = 12, Fill = new SolidColorBrush(fresh ? Color.FromRgb(0xFA, 0x32, 0x0A) : Color.FromRgb(0x2E, 0x9E, 0x4F)), Margin = new Thickness(0, 0, 7, 0), VerticalAlignment = VerticalAlignment.Center });
            row.Children.Add(new TextBlock { Text = r.Rotten, FontSize = 13, FontWeight = FontWeights.SemiBold, Foreground = B("Fg"), VerticalAlignment = VerticalAlignment.Center });
            chips.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(1), BorderBrush = B("Line"), Background = B("Card"),
                Padding = new Thickness(8, 3, 10, 3), Margin = new Thickness(0, 0, 8, 8), Child = row,
                ToolTip = fresh ? "Rotten Tomatoes · Fresh" : "Rotten Tomatoes · Rotten",
            });
        }
    }

    private static IEnumerable<string> ChipTexts(ItemVm it, DetailsInfo? info)
    {
        var year = info?.Released is { Length: >= 4 } r ? r[..4] : it.Year;
        if (!string.IsNullOrEmpty(year)) yield return year!;
        var genre = info?.Genre ?? it.Genre;
        if (!string.IsNullOrEmpty(genre))
            foreach (var g in genre!.Split(new[] { ',', '/', '|' }, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries).Take(3))
                yield return g;
        if (!string.IsNullOrEmpty(info?.Duration)) yield return info!.Duration!;
        if (!string.IsNullOrEmpty(info?.Language)) yield return info!.Language!.ToUpperInvariant().Length <= 3 ? info.Language!.ToUpperInvariant() : info.Language!;
        if (!string.IsNullOrEmpty(info?.Country)) yield return info!.Country!;
    }

    private static FrameworkElement PosterImage(string? url, double w, double h)
    {
        var g = new Grid { Width = w, Height = h, VerticalAlignment = VerticalAlignment.Top };
        g.Children.Add(new Border
        {
            CornerRadius = new CornerRadius(14),
            Background = B("Card"),
            Child = new TextBlock { Text = "", FontFamily = Icons, FontSize = 36, Foreground = B("Muted"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center },
        });
        if (UiKit.Image(url, (int)(w * 1.5)) is { } src)
            g.Children.Add(new Border { CornerRadius = new CornerRadius(14), Background = new ImageBrush(src) { Stretch = Stretch.UniformToFill } });
        return g;
    }

    private FrameworkElement Hero(ItemVm it, DetailsInfo? info, string? poster)
    {
        var g = new Grid { Margin = new Thickness(0, 6, 0, 0) };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.Children.Add(PosterImage(poster, 230, 340));

        var right = new StackPanel { Margin = new Thickness(34, 4, 0, 0), MaxWidth = 900, HorizontalAlignment = HorizontalAlignment.Left };
        Grid.SetColumn(right, 1);
        var title = Text(it.Name, 32, "Fg", FontWeights.SemiBold, wrap: true);
        right.Children.Add(title);

        var chips = new WrapPanel { Margin = new Thickness(0, 14, 0, 4) };
        var rating = info?.Rating ?? (it.HasRating ? it.Rating : null);
        if (rating != null) chips.Children.Add(Chip($"★ {rating}", "Rating"));
        AddExternalRatings(chips);
        foreach (var c in ChipTexts(it, info)) chips.Children.Add(Chip(c));
        right.Children.Add(chips);

        var plot = info?.Plot ?? it.Plot;
        if (!string.IsNullOrWhiteSpace(plot))
        {
            var p = Text(plot!, 14.5, "Fg", wrap: true);
            p.TextTrimming = TextTrimming.None;
            p.LineHeight = 22;
            p.Margin = new Thickness(0, 8, 0, 0);
            right.Children.Add(p);
        }
        AddCredit(right, "Actors", info?.Cast);
        AddCredit(right, "Director", info?.Director);

        // Progress line for a started film
        if (!it.IsSeries && it.DurationSec > 0 && it.PositionSec > 0 && !it.IsWatched)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 18, 0, 0) };
            row.Children.Add(new ProgressBar { Style = St("ThinProgress"), Value = it.WatchedRatio, Width = 220, VerticalAlignment = VerticalAlignment.Center });
            var left = Text($"{it.TimeLeftText} · {it.WatchedRatio:P0} watched", 12.5, "Muted");
            left.Margin = new Thickness(10, 0, 0, 0);
            row.Children.Add(left);
            right.Children.Add(row);
        }

        var buttons = new WrapPanel { Margin = new Thickness(0, 22, 0, 0) };
        if (it.IsSeries)
        {
            var resume = ResumeEpisode();
            var first = _episodes.FirstOrDefault();
            if (resume != null)
                buttons.Children.Add(Primary($"Resume {SE(resume)}", "", () => _ = PlayEpisode(resume)));
            else if (first != null)
                buttons.Children.Add(Primary($"Play first episode {SE(first)}", "", () => _ = PlayEpisode(first)));
        }
        else
        {
            var started = it.PositionSec > 0 && !it.IsWatched;
            buttons.Children.Add(Primary(started ? "Resume" : "Play", "", () => _ = PlayMovie()));
            if (started) buttons.Children.Add(Primary("Start over", "", () => _ = PlayMovie(startOver: true), secondary: true));
        }
        buttons.Children.Add(Primary(it.IsFavorite ? "Remove from favorites" : "Add to favorites", it.IsFavorite ? "" : "",
            () => _ = ToggleFavorite(it), secondary: true));
        if (!it.IsSeries)
        {
            buttons.Children.Add(Primary(it.IsWatched ? "Watched" : "Mark as watched", it.IsWatched ? "" : "", async () =>
            {
                await _repo.SetWatched(it, !it.IsWatched);
                _progressStale = false;
                await ReloadDetailsProgress(delayMs: 0);
            }, secondary: true));
        }
        if (info?.Trailer is { } yt)
            buttons.Children.Add(Primary("Trailer", "", () => OpenTrailer(yt), secondary: true));
        right.Children.Add(buttons);
        g.Children.Add(right);
        return g;
    }

    /// <summary>"S01E03".</summary>
    private static string SE(ItemVm ep) =>
        $"S{(int.TryParse(ep.Season, out var s) ? s : 1):00}E{(int.TryParse(ep.Episode, out var e) ? e : 0):00}";

    private static void AddCredit(Panel parent, string label, string? value)
    {
        if (string.IsNullOrWhiteSpace(value)) return;
        var t = new TextBlock { TextWrapping = TextWrapping.Wrap, FontSize = 13.5, Margin = new Thickness(0, 10, 0, 0), Foreground = B("Muted") };
        t.Inlines.Add(new System.Windows.Documents.Run($"{label}:  ") { FontWeight = FontWeights.SemiBold, Foreground = B("Fg") });
        t.Inlines.Add(new System.Windows.Documents.Run(value));
        parent.Children.Add(t);
    }

    /// <summary>Compact details shown under the player.</summary>
    private FrameworkElement About(ItemVm it, DetailsInfo? info, string? poster)
    {
        var g = new Grid();
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.Children.Add(PosterImage(poster, 140, 208));
        var right = new StackPanel { Margin = new Thickness(24, 0, 0, 0), MaxWidth = 900, HorizontalAlignment = HorizontalAlignment.Left };
        Grid.SetColumn(right, 1);
        right.Children.Add(Text(it.Name, 22, "Fg", FontWeights.SemiBold, wrap: true));
        var chips = new WrapPanel { Margin = new Thickness(0, 10, 0, 0) };
        var rating = info?.Rating ?? (it.HasRating ? it.Rating : null);
        if (rating != null) chips.Children.Add(Chip($"★ {rating}", "Rating"));
        AddExternalRatings(chips);
        foreach (var c in ChipTexts(it, info)) chips.Children.Add(Chip(c));
        right.Children.Add(chips);
        var plot = info?.Plot ?? it.Plot;
        if (!string.IsNullOrWhiteSpace(plot))
        {
            var p = Text(plot!, 13.5, "Fg", wrap: true);
            p.TextTrimming = TextTrimming.None;
            p.LineHeight = 21;
            p.Margin = new Thickness(0, 4, 0, 0);
            right.Children.Add(p);
        }
        AddCredit(right, "Actors", info?.Cast);
        AddCredit(right, "Director", info?.Director);
        var buttons = new WrapPanel { Margin = new Thickness(0, 16, 0, 0) };
        buttons.Children.Add(Primary(it.IsFavorite ? "Remove from favorites" : "Add to favorites", it.IsFavorite ? "" : "",
            () => _ = ToggleFavorite(it), secondary: true));
        right.Children.Add(buttons);
        g.Children.Add(right);
        return g;
    }

    private FrameworkElement NowPlayingEpisode(ItemVm ep)
    {
        var s = new StackPanel { Margin = new Thickness(0, 4, 0, 0) };
        s.Children.Add(Text($"{SE(ep)} · {ep.Name}", 19, "Fg", FontWeights.SemiBold, wrap: true));
        if (!string.IsNullOrWhiteSpace(ep.Plot))
        {
            var p = Text(ep.Plot!, 13.5, "Muted", wrap: true);
            p.TextTrimming = TextTrimming.None;
            p.Margin = new Thickness(0, 6, 0, 0);
            p.MaxWidth = 900;
            p.HorizontalAlignment = HorizontalAlignment.Left;
            s.Children.Add(p);
        }
        return s;
    }

    private Microsoft.Web.WebView2.Wpf.WebView2? _trailerView;
    private bool _trailerOpen;

    /// <summary>Play the trailer in the page's own player card (stops the film if it is playing).</summary>
    private async void OpenTrailer(string youtubeId)
    {
        if (_vodPlaying) StopPlayback();
        _trailerView ??= TrailerPlayer.Create();
        // Show the card and put the web view in the window first: WebView2 only initialises once it is loaded.
        _trailerOpen = true;
        VodCardLabel.Text = "TRAILER";
        VodCardTitle.Text = _details?.Name ?? "Trailer";
        VodCardSub.Text = "YouTube";
        if (_trailerView.Parent is Panel old && old != VodVideoHost) old.Children.Remove(_trailerView);
        if (_trailerView.Parent == null) VodVideoHost.Children.Add(_trailerView);
        _trailerView.Visibility = Visibility.Visible;
        Layout();
        VodScroll.ScrollToTop();
        if (!await TrailerPlayer.Init(_trailerView, on => { if (on != _fullscreen) ToggleFullscreen(force: true); }))
        {
            CloseTrailer();
            SetStatus("The Microsoft Edge WebView2 runtime is missing, so the trailer opens in your browser.");
            TrailerPlayer.OpenInBrowser($"https://www.youtube.com/watch?v={youtubeId}");
            return;
        }
        if (_trailerOpen) _trailerView.CoreWebView2.Navigate(TrailerPlayer.Url(youtubeId).AbsoluteUri);
    }

    private void CloseTrailer()
    {
        if (!_trailerOpen) return;
        _trailerOpen = false;
        try { _trailerView?.CoreWebView2?.Navigate("about:blank"); } catch { /* already gone */ }
        _videoTucked = false;
        VodTuckedHint.Visibility = Visibility.Collapsed;
        if (_trailerView != null)
        {
            _trailerView.Visibility = Visibility.Collapsed;
            (_trailerView.Parent as Panel)?.Children.Remove(_trailerView);
        }
        if (_fullscreen) ToggleFullscreen();
        Layout();
    }

    private FrameworkElement Trailer(ItemVm it, string youtubeId)
    {
        var s = new StackPanel();
        s.Children.Add(SectionHeader("Trailer"));
        var thumb = new Grid { Width = 360, Height = 202, HorizontalAlignment = HorizontalAlignment.Left };
        thumb.Children.Add(new Border { CornerRadius = new CornerRadius(12), Background = B("Card") });
        if (UiKit.Image($"https://img.youtube.com/vi/{youtubeId}/hqdefault.jpg", 480) is { } img)
            thumb.Children.Add(new Border { CornerRadius = new CornerRadius(12), Background = new ImageBrush(img) { Stretch = Stretch.UniformToFill } });
        thumb.Children.Add(new Border
        {
            Width = 64,
            Height = 44,
            CornerRadius = new CornerRadius(12),
            Background = new SolidColorBrush(Color.FromArgb(0xE6, 0xE5, 0x2D, 0x27)),
            Child = new TextBlock { Text = "", FontFamily = Icons, FontSize = 18, Foreground = Brushes.White, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center },
        });
        var label = new Border
        {
            CornerRadius = new CornerRadius(6),
            Background = B("Badge"),
            Padding = new Thickness(8, 3, 8, 3),
            Margin = new Thickness(10),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Bottom,
            Child = Text($"{it.Name} · YouTube", 12, "Fg"),
        };
        thumb.Children.Add(label);
        var hover = new Border { CornerRadius = new CornerRadius(12), BorderThickness = new Thickness(2), BorderBrush = Brushes.Transparent };
        thumb.Children.Add(hover);
        var btn = new Button { Content = thumb, Template = BareTemplate(), Cursor = Cursors.Hand, Focusable = false, ToolTip = "Play the trailer", HorizontalAlignment = HorizontalAlignment.Left };
        btn.MouseEnter += (_, _) => hover.BorderBrush = B("Accent");
        btn.MouseLeave += (_, _) => hover.BorderBrush = Brushes.Transparent;
        btn.Click += (_, _) => OpenTrailer(youtubeId);
        s.Children.Add(btn);
        return s;
    }

    // ---------------------------------------------------------------- seasons and episodes

    private FrameworkElement Seasons(ItemVm series)
    {
        var s = new StackPanel();
        if (_episodes.Count == 0)
        {
            if (!_detailsLoading)
            {
                s.Children.Add(SectionHeader("Seasons and episodes"));
                s.Children.Add(Text("This series has no episodes on the server.", 13, "Muted"));
            }
            return s;
        }
        var seasons = _episodes.Select(e => e.Season ?? "1").Distinct().ToList();
        var current = _season != null && seasons.Contains(_season) ? _season : seasons[0];
        var eps = _episodes.Where(e => (e.Season ?? "1") == current).ToList();

        s.Children.Add(SectionHeader("Seasons and episodes", _episodes.Count));
        var bar = new DockPanel { Margin = new Thickness(0, 0, 0, 14) };
        var allWatched = eps.All(e => e.IsWatched);
        var mark = Primary(allWatched ? "Mark season as unwatched" : "Mark season as watched", allWatched ? "" : "", async () =>
        {
            await _repo.SetWatchedMany(eps, !allWatched);
            _progressStale = false;
            await ReloadDetailsProgress(delayMs: 0);
        }, secondary: true);
        DockPanel.SetDock(mark, Dock.Right);
        bar.Children.Add(mark);
        bar.Children.Add(Segments(seasons.Select(x => (x, $"Season {x}")), current, v =>
        {
            _season = v;
            RenderVodPage();
        }));
        s.Children.Add(bar);
        s.Children.Add(Wrap(eps.Select(e => EpisodeCard(series, e))));
        return s;
    }

    private FrameworkElement EpisodeCard(ItemVm series, ItemVm ep)
    {
        const double w = 260, h = 146;
        var playing = Playing?.Id == ep.Id;
        var img = new Grid { Width = w, Height = h };
        img.Children.Add(new Border
        {
            CornerRadius = new CornerRadius(10),
            Background = B("Card"),
            Child = new TextBlock { Text = "", FontFamily = Icons, FontSize = 26, Foreground = B("Muted"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center },
        });
        if (UiKit.Image(ep.Logo ?? series.Logo, 390) is { } src)
            img.Children.Add(new Border { CornerRadius = new CornerRadius(10), Background = new ImageBrush(src) { Stretch = Stretch.UniformToFill } });
        var num = Badge($"E{ep.Episode}", "Fg");
        num.HorizontalAlignment = HorizontalAlignment.Left;
        num.VerticalAlignment = VerticalAlignment.Top;
        num.Margin = new Thickness(7);
        img.Children.Add(num);
        if (ep.DurationText.Length > 0)
        {
            var d = Badge(ep.DurationText, "Fg");
            d.HorizontalAlignment = HorizontalAlignment.Right;
            d.VerticalAlignment = VerticalAlignment.Bottom;
            d.Margin = new Thickness(7, 7, 7, 12);
            img.Children.Add(d);
        }
        if (!ep.IsWatched && ep.WatchedRatio > 0)
            img.Children.Add(new ProgressBar { Style = St("ThinProgress"), Value = ep.WatchedRatio, VerticalAlignment = VerticalAlignment.Bottom, Margin = new Thickness(8, 0, 8, 6) });

        // Watched toggle: filled when watched, an outline that appears on hover otherwise.
        var tick = new TextBlock { Text = "\uE73E", FontFamily = Icons, FontSize = 12, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        var circle = new Border { Width = 28, Height = 28, CornerRadius = new CornerRadius(14), BorderThickness = new Thickness(1.6), Child = tick };
        void Paint(bool hot)
        {
            circle.Background = ep.IsWatched ? B("Accent") : new SolidColorBrush(Color.FromArgb(0xB3, 0, 0, 0));
            circle.BorderBrush = ep.IsWatched ? B("Accent") : Brushes.White;
            tick.Foreground = Brushes.White;
            circle.Opacity = ep.IsWatched || hot ? 1 : 0;
        }
        Paint(false);
        var watchBtn = new Button
        {
            Content = circle,
            Template = BareTemplate(),
            Cursor = Cursors.Hand,
            Focusable = false,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(7),
            ToolTip = ep.IsWatched ? "Watched — click to mark as unwatched" : "Mark as watched",
        };
        watchBtn.Click += async (_, e) =>
        {
            e.Handled = true;
            await _repo.SetWatched(ep, !ep.IsWatched);
            await ReloadDetailsProgress(delayMs: 0);
        };
        img.Children.Add(watchBtn);

        // Play button in the middle on hover.
        var playHint = new Border
        {
            Width = 44,
            Height = 44,
            CornerRadius = new CornerRadius(22),
            Background = B("Accent"),
            Opacity = 0,
            IsHitTestVisible = false,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock { Text = "\uE768", FontFamily = Icons, FontSize = 16, Foreground = Brushes.White, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(3, 0, 0, 0) },
        };
        if (!playing) img.Children.Add(playHint);
        if (playing)
        {
            var p = Badge("▶ PLAYING", "PrimaryFg");
            p.Background = B("Accent");
            p.HorizontalAlignment = HorizontalAlignment.Center;
            p.VerticalAlignment = VerticalAlignment.Center;
            img.Children.Add(p);
        }
        var hover = new Border { CornerRadius = new CornerRadius(10), BorderThickness = new Thickness(2), BorderBrush = playing ? B("Accent") : Brushes.Transparent };
        img.Children.Add(hover);

        var stack = new StackPanel { Width = w };
        stack.Children.Add(img);
        var title = Text($"{ep.Episode}. {ep.Name}", 13.5, playing ? "Accent" : "Fg", FontWeights.SemiBold);
        title.Margin = new Thickness(2, 8, 2, 0);
        stack.Children.Add(title);
        if (!string.IsNullOrWhiteSpace(ep.Plot))
        {
            var plot = Text(ep.Plot!, 12, "Muted", wrap: true);
            plot.MaxHeight = 34;
            plot.Margin = new Thickness(2, 3, 2, 0);
            stack.Children.Add(plot);
        }
        var card = new Button { Content = stack, Template = BareTemplate(), Cursor = Cursors.Hand, Focusable = false, Margin = new Thickness(0, 0, 18, 20), ToolTip = ep.Name };
        card.MouseEnter += (_, _) =>
        {
            hover.BorderBrush = B("Accent");
            Paint(true);
            playHint.Opacity = 1;
        };
        card.MouseLeave += (_, _) =>
        {
            hover.BorderBrush = playing ? B("Accent") : Brushes.Transparent;
            Paint(false);
            playHint.Opacity = 0;
        };
        card.Click += (_, _) => _ = PlayEpisode(ep);
        card.MouseRightButtonUp += (_, e) =>
        {
            e.Handled = true;
            UiKit.PopMenu(card, $"{SE(ep)}", new[]
            {
                new UiKit.MenuRow("\uE768", "Play", false, () => _ = PlayEpisode(ep)),
                new UiKit.MenuRow(ep.IsWatched ? "\uE7BA" : "\uE73E", ep.IsWatched ? "Mark as unwatched" : "Mark as watched", false, async () =>
                {
                    await _repo.SetWatched(ep, !ep.IsWatched);
                    await ReloadDetailsProgress();
                }),
            });
        };
        return card;
    }
}
