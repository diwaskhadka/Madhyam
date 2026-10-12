using System.ComponentModel;
using System.Net.Http;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using MadhyamCoreNet;
using LibVLCSharp.Shared;
using VlcMediaPlayer = LibVLCSharp.Shared.MediaPlayer;

namespace Madhyam.Windows;

/// <summary>
/// layout:
///   rail | top bar (playlist switcher, search pill, actions)
///        | Live:   categories | channel cards | video + guide cards
///        | VOD:    categories | poster grid (paged) → details page → player + transport bar
/// </summary>
public partial class MainWindow : Window
{
    private enum Mode { Live, Movies, Series, Favorites, Recent, Search }

    private readonly Repository _repo = new();
    private readonly LibVLC _vlc = new("--no-video-title-show", "--network-caching=1500");
    private readonly VlcMediaPlayer _player;
    private readonly DispatcherTimer _searchDebounce = new() { Interval = TimeSpan.FromMilliseconds(300) };
    private readonly DispatcherTimer _tick = new() { Interval = TimeSpan.FromSeconds(1) };

    private List<Source> _sources = new();
    private Source? _source;
    private Mode _mode = Mode.Live;
    private HashSet<string> _favorites = new();
    private int _loadVersion; // results of superseded loads are dropped

    // Poster grid paging
    private List<ItemVm> _gridAll = new();
    private int _page;
    private int _perPage = 50;

    // Details page
    private ItemVm? _details;
    private List<ItemVm> _episodes = new();

    // Playback
    private List<ItemVm> _playlist = new();
    private int _playIndex = -1;
    private ItemVm? Playing => _playIndex >= 0 && _playIndex < _playlist.Count ? _playlist[_playIndex] : null;
    private bool _vodPlayerOpen; // films/episodes fill the body with a transport bar
    private DateTime _guideDay = DateTime.Today;
    private ItemVm? _guideItem;
    private int _ticks;
    private bool _fullscreen;
    private WindowState _savedState;

    public MainWindow()
    {
        InitializeComponent();
        _player = new VlcMediaPlayer(_vlc);
        Video.MediaPlayer = _player;
        _player.EncounteredError += (_, _) => Dispatcher.Invoke(() =>
            SetStatus("Playback failed: the stream could not be opened. Try another channel or refresh the playlist."));
        _player.Playing += (_, _) => Dispatcher.Invoke(() => PlayPause.Content = "");
        _player.Paused += (_, _) => Dispatcher.Invoke(() => PlayPause.Content = "");

        _searchDebounce.Tick += async (_, _) =>
        {
            _searchDebounce.Stop();
            await ApplySearch();
        };
        _tick.Tick += (_, _) => OnTick();
        HighlightPerPage();
    }

    // ================================================================ lifecycle

    private async void Window_Loaded(object sender, RoutedEventArgs e)
    {
        await Run("Loading…", async () =>
        {
            await _repo.Init();
            _favorites = await _repo.FavoriteIds();
            await ReloadSources();
        });
        var args = Environment.GetCommandLineArgs();
        string? Arg(string name) => Array.IndexOf(args, name) is var i and >= 0 && i + 1 < args.Length ? args[i + 1] : null;
        var start = Arg("--mode") is { } m && Enum.TryParse<Mode>(m, true, out var parsed) ? parsed : Mode.Live;
        (start switch
        {
            Mode.Movies => NavMovies,
            Mode.Series => NavSeries,
            Mode.Favorites => NavFavorites,
            Mode.Recent => NavRecent,
            Mode.Search => NavSearch,
            _ => NavLive,
        }).IsChecked = true; // triggers Nav_Checked → LoadMode
        _tick.Start();

        if (Arg("--screenshot") is { } shot)
        {
            await CaptureForCi(shot, args.Contains("--open-first"), Arg("--search"));
            return;
        }
        if (_sources.Count == 0) OpenAddSource();
        await RefreshStaleGuides();
    }

    /// <summary>Re-download guides older than 12 h in the background; never blocks watching.</summary>
    private async Task RefreshStaleGuides()
    {
        foreach (var s in _sources.ToList())
        {
            try
            {
                if (_repo.EpgAge(s.Id) is { } age && age < TimeSpan.FromHours(12)) continue;
                if (!await _repo.HasEpg(s.Id)) continue;
                await _repo.RefreshEpg(s.Id);
                if (ItemList.ItemsSource is IReadOnlyList<ItemVm> shown) await UpdateNowNext(shown);
                if (_guideItem != null) await LoadGuide();
            }
            catch (Exception ex)
            {
                Repository.Log($"background guide refresh failed for {s.Name}: {ex.Message}");
            }
        }
    }

    /// <summary>
    /// CI helper: render the window to a PNG without needing a real screen, then exit.
    /// `--open-first` opens the first poster's details page, `--search q` types a query.
    /// </summary>
    private async Task CaptureForCi(string path, bool openFirst, string? search)
    {
        await Task.Delay(1500);
        if (search != null)
        {
            SearchBox.Text = search;
            await Task.Delay(1200);
        }
        if (openFirst && PosterGrid.Items.Count > 0 && PosterGrid.Items[0] is ItemVm first)
        {
            await ShowDetails(first);
            await Task.Delay(1200);
        }
        if (ItemList.Items.Count > 0 && ItemList.Visibility == Visibility.Visible && ListPanel.Visibility == Visibility.Visible)
        {
            ItemList.SelectedIndex = Math.Min(2, ItemList.Items.Count - 1);
            await Task.Delay(1200);
        }
        var root = (FrameworkElement)Content;
        root.UpdateLayout();
        var dpi = VisualTreeHelper.GetDpi(this);
        var bmp = new RenderTargetBitmap(
            (int)(root.ActualWidth * dpi.DpiScaleX), (int)(root.ActualHeight * dpi.DpiScaleY),
            96 * dpi.DpiScaleX, 96 * dpi.DpiScaleY, PixelFormats.Pbgra32);
        // Paint the window background first; Content itself is transparent.
        var dv = new DrawingVisual();
        using (var dc = dv.RenderOpen())
        {
            dc.DrawRectangle(Background, null, new Rect(0, 0, root.ActualWidth, root.ActualHeight));
            dc.DrawRectangle(new VisualBrush(root), null, new Rect(0, 0, root.ActualWidth, root.ActualHeight));
        }
        bmp.Render(dv);
        var enc = new PngBitmapEncoder();
        enc.Frames.Add(BitmapFrame.Create(bmp));
        await using (var f = System.IO.File.Create(path)) enc.Save(f);
        Close();
    }

    private void Window_Closing(object? sender, CancelEventArgs e)
    {
        SaveProgress();
        _player.Stop();
        _repo.Persist();
        _player.Dispose();
        _vlc.Dispose();
        _repo.Dispose();
    }

    private void SetStatus(string text)
    {
        Status.Text = text;
        GridStatus.Text = text;
        GridStatus.Visibility = text.Length > 0 && GridPanel.Visibility == Visibility.Visible ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>Run a task with a status line; errors are shown, not thrown.</summary>
    private async Task Run(string label, Func<Task> work)
    {
        SetStatus(label);
        try
        {
            await work();
            SetStatus("");
        }
        catch (Exception ex)
        {
            Repository.Log(ex.ToString());
            SetStatus(Explain(ex));
        }
    }

    private static string Explain(Exception ex) => ex switch
    {
        IptvException { Code: "api" } e => $"The server refused the request: {e.Message}",
        IptvException { Code: "parse" } e => $"The server's answer could not be read: {e.Message}",
        HttpRequestException e => $"Network error: {e.Message}",
        TaskCanceledException => "The server took too long to answer.",
        _ => ex.Message,
    };

    private static string ModeLabel(Mode m) => m switch
    {
        Mode.Live => "Live TV",
        Mode.Movies => "Movies",
        Mode.Series => "Series",
        Mode.Favorites => "Favorites",
        Mode.Recent => "Recently watched",
        _ => "Search",
    };

    private static string? ModeKind(Mode m) => m switch
    {
        Mode.Live => "live",
        Mode.Movies => "movie",
        Mode.Series => "series",
        _ => null,
    };

    // ================================================================ playlists

    private async Task ReloadSources(int? select = null)
    {
        _sources = await _repo.Sources();
        _source = _sources.FirstOrDefault(s => s.Id == select) ?? _sources.FirstOrDefault(s => s.Id == _source?.Id) ?? _sources.FirstOrDefault();
        await UpdatePlaylistHeader();
    }

    private async Task UpdatePlaylistHeader()
    {
        if (_source == null)
        {
            PlaylistName.Text = "No playlist";
            PlaylistCount.Text = "Add a source to begin";
            return;
        }
        PlaylistName.Text = _source.Name;
        var live = await _repo.CountKind(_source.Id, "live");
        var movies = await _repo.CountKind(_source.Id, "movie");
        var series = await _repo.CountKind(_source.Id, "series");
        var parts = new List<string>();
        if (live > 0) parts.Add($"{live:N0} channels");
        if (movies > 0) parts.Add($"{movies:N0} movies");
        if (series > 0) parts.Add($"{series:N0} series");
        PlaylistCount.Text = parts.Count > 0 ? string.Join(" · ", parts) : (_source.Kind == "xtream" ? "Xtream Code" : "M3U playlist");
    }

    private void PlaylistButton_Click(object sender, MouseButtonEventArgs e)
    {
        var menu = new ContextMenu { PlacementTarget = PlaylistButton, Placement = PlacementMode.Bottom, MinWidth = 300 };
        foreach (var s in _sources)
        {
            var item = new MenuItem
            {
                Header = $"{s.Name}   ({(s.Kind == "xtream" ? "Xtream" : "M3U")})",
                IsCheckable = true,
                IsChecked = s.Id == _source?.Id,
            };
            item.Click += async (_, _) =>
            {
                _source = s;
                await UpdatePlaylistHeader();
                await LoadMode();
            };
            menu.Items.Add(item);
        }
        if (_sources.Count > 0) menu.Items.Add(new Separator());
        var add = new MenuItem { Header = "Add playlist…" };
        add.Click += (_, _) => OpenAddSource();
        menu.Items.Add(add);
        if (_source != null)
        {
            var remove = new MenuItem { Header = $"Remove \"{_source.Name}\"" };
            remove.Click += Remove_Click;
            menu.Items.Add(remove);
        }
        menu.IsOpen = true;
    }

    private void Add_Click(object sender, RoutedEventArgs e) => OpenAddSource();

    private async void OpenAddSource()
    {
        var dlg = new AddSourceWindow { Owner = this };
        if (dlg.ShowDialog() != true) return;
        await Run(dlg.IsXtream ? "Logging in and downloading the catalog…" : "Downloading playlist…", async () =>
        {
            var id = dlg.IsXtream
                ? await _repo.AddXtream(dlg.SourceName, dlg.Server, dlg.Username, dlg.Password)
                : await _repo.AddM3u(dlg.SourceName, dlg.PlaylistUrl);
            await ReloadSources(id);
            await LoadMode();
        });
    }

    private async void Refresh_Click(object sender, RoutedEventArgs e)
    {
        if (_source is not { } s) return;
        RefreshButton.IsEnabled = false;
        await Run($"Refreshing {s.Name}…", async () =>
        {
            await _repo.Refresh(s.Id);
            await UpdatePlaylistHeader();
            await LoadMode();
        });
        RefreshButton.IsEnabled = true;
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (_source is not { } s) return;
        if (MessageBox.Show(this, $"Remove \"{s.Name}\" and its channels?", "Madhyam", MessageBoxButton.YesNo) != MessageBoxResult.Yes) return;
        await Run($"Removing {s.Name}…", async () =>
        {
            StopPlayback();
            await _repo.RemoveSource(s.Id);
            _source = null;
            await ReloadSources();
            await LoadMode();
        });
    }

    private void About_Click(object sender, RoutedEventArgs e) =>
        MessageBox.Show(this,
            $"Madhyam {typeof(App).Assembly.GetName().Version?.ToString(3)}\nEngine (Rust) {_repo.CoreVersion}\n\n" +
            "Madhyam plays your own Xtream Codes or M3U playlists. It ships with no channels.",
            "About Madhyam");

    // ================================================================ layout

    /// <summary>Show the panels that belong to the current mode / view.</summary>
    private void Layout()
    {
        var vod = _mode is Mode.Movies or Mode.Series;
        var playingVod = _vodPlayerOpen;
        var details = _details != null && !playingVod;
        var showCats = !_fullscreen && !playingVod && !details && _mode is Mode.Live or Mode.Movies or Mode.Series;
        var showList = !_fullscreen && !playingVod && !details && !vod;
        var showGrid = !_fullscreen && !playingVod && !details && vod;
        var showPlayer = playingVod || (!vod && !details) || _fullscreen;

        CatsPanel.Visibility = showCats ? Visibility.Visible : Visibility.Collapsed;
        ColCats.Width = new GridLength(showCats ? 250 : 0);
        ListPanel.Visibility = showList ? Visibility.Visible : Visibility.Collapsed;
        ColList.Width = showList ? new GridLength(430) : new GridLength(0);
        GridPanel.Visibility = showGrid ? Visibility.Visible : Visibility.Collapsed;
        DetailsPanel.Visibility = details && !_fullscreen ? Visibility.Visible : Visibility.Collapsed;
        PlayerPanel.Visibility = showPlayer ? Visibility.Visible : Visibility.Collapsed;

        var liveGuide = Playing?.IsLive ?? !playingVod;
        GuidePanel.Visibility = !_fullscreen && liveGuide && !playingVod ? Visibility.Visible : Visibility.Collapsed;
        ControlBar.Visibility = !_fullscreen && (playingVod || Playing is { IsLive: false }) ? Visibility.Visible : Visibility.Collapsed;
        GuideRow.Height = _fullscreen ? new GridLength(0)
            : GuidePanel.Visibility == Visibility.Visible ? new GridLength(1.15, GridUnitType.Star)
            : GridLength.Auto;

        RailCol.Width = new GridLength(_fullscreen ? 0 : 64);
        TopRow.Height = new GridLength(_fullscreen ? 0 : 68);

        SectionLabel.Text = _details != null && !playingVod ? $"{ModeLabel(_mode)} / {_details.Name}" : ModeLabel(_mode);
        SearchHint.Text = _mode == Mode.Search ? "Search channels, movies and series…" : "Filter this section…";
    }

    // ================================================================ navigation

    private async void Nav_Checked(object sender, RoutedEventArgs e)
    {
        if (sender is not RadioButton { Tag: string tag }) return;
        var mode = Enum.Parse<Mode>(tag);
        // Leaving a video context stops playback (one VideoView).
        if (_vodPlayerOpen || (mode is Mode.Movies or Mode.Series && Playing != null)) StopPlayback();
        _mode = mode;
        _details = null;
        SearchBox.Text = "";
        if (mode == Mode.Search) SearchBox.Focus();
        await LoadMode();
    }

    private async Task LoadMode()
    {
        var version = ++_loadVersion;
        _details = null;
        Layout();
        var kind = ModeKind(_mode);

        if (kind != null)
        {
            CatsTitle.Text = _mode switch { Mode.Live => "Live categories", Mode.Movies => "Movie categories", _ => "Series categories" };
            var groups = _source == null ? new List<Group>() : await _repo.Groups(_source.Id, kind);
            if (version != _loadVersion) return;
            GroupList.ItemsSource = groups;
            if (groups.Count > 0)
            {
                GroupList.SelectedIndex = 0; // → GroupList_SelectionChanged loads the items
            }
            else
            {
                ShowList(new List<ItemVm>(), _source == null ? "Add a playlist with the + button to start." : "Nothing in this playlist yet.");
                ShowGrid(new List<ItemVm>(), "No items");
            }
            return;
        }

        GroupList.ItemsSource = null;
        var items = _mode switch
        {
            Mode.Favorites => await _repo.Favorites(),
            Mode.Recent => await _repo.Recent(),
            _ => await _repo.Search(SearchBox.Text),
        };
        if (version != _loadVersion) return;
        ListTitle.Text = ModeLabel(_mode);
        ShowList(items, _mode switch
        {
            Mode.Favorites => "No favorites yet. Click ☆ on a channel to add it.",
            Mode.Recent => "Nothing watched yet.",
            _ => SearchBox.Text.Length == 0 ? "Type in the search bar to search everything." : "No results.",
        });
    }

    private async void GroupList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (GroupList.SelectedItem is not Group g || _source == null) return;
        var kind = ModeKind(_mode);
        if (kind == null) return;
        var version = ++_loadVersion;
        SearchBox.Text = "";
        var items = await _repo.List(_source.Id, kind, g.Name);
        if (version != _loadVersion) return;
        if (_mode == Mode.Live)
        {
            ListTitle.Text = g.Name;
            ShowList(items, "No channels in this category.");
        }
        else
        {
            ShowGrid(items, g.Name);
        }
    }

    private void ShowList(List<ItemVm> items, string emptyText)
    {
        for (var i = 0; i < items.Count; i++)
        {
            items[i].Number = i + 1;
            items[i].IsFavorite = _favorites.Contains(items[i].Id);
        }
        ItemList.ItemsSource = items;
        ListCount.Text = items.Count > 0 ? $"{items.Count:N0}" : "";
        ListEmpty.Text = emptyText;
        ListEmpty.Visibility = items.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        if (Playing is { } p && items.FirstOrDefault(i => i.Id == p.Id) is { } same) ItemList.SelectedItem = same;
        _ = UpdateNowNext(items);
    }

    private async Task UpdateNowNext(IReadOnlyList<ItemVm> items)
    {
        var live = items.Where(i => i.IsLive).Take(600).ToList();
        if (live.Count == 0) return;
        try
        {
            var nn = await _repo.NowNext(live.Select(i => i.Id).ToList());
            var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
            foreach (var it in live) it.SetNow(nn.TryGetValue(it.Id, out var v) ? v.Now : null, now);
        }
        catch (Exception ex)
        {
            Repository.Log($"now/next failed: {ex.Message}");
        }
    }

    // ---------------------------------------------------------------- search

    private void SearchBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        SearchHint.Visibility = SearchBox.Text.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        _searchDebounce.Stop();
        _searchDebounce.Start();
    }

    /// <summary>
    /// Search pill semantics (as in a reference player): it filters the current section.
    /// In Live/Movies/Series it searches that kind across all categories;
    /// in Favorites/Recent it filters the list; Search searches everything.
    /// </summary>
    private async Task ApplySearch()
    {
        var q = SearchBox.Text.Trim();
        var version = ++_loadVersion;
        switch (_mode)
        {
            case Mode.Search:
                var all = await _repo.Search(q);
                if (version != _loadVersion) return;
                ListTitle.Text = q.Length == 0 ? "Search" : $"Results for “{q}”";
                ShowList(all, q.Length == 0 ? "Type in the search bar to search everything." : "No results.");
                break;
            case Mode.Favorites or Mode.Recent:
                var baseList = _mode == Mode.Favorites ? await _repo.Favorites() : await _repo.Recent();
                if (version != _loadVersion) return;
                ShowList(baseList.Where(i => i.Name.Contains(q, StringComparison.OrdinalIgnoreCase)).ToList(), "No matches.");
                break;
            default:
                if (q.Length == 0)
                {
                    GroupList_SelectionChanged(GroupList, null!); // back to the selected category
                    return;
                }
                if (_details != null) { _details = null; Layout(); }
                var hits = await _repo.Search(q, ModeKind(_mode));
                if (version != _loadVersion) return;
                if (_mode == Mode.Live)
                {
                    ListTitle.Text = $"Results for “{q}”";
                    ShowList(hits, "No channels match.");
                }
                else
                {
                    ShowGrid(hits, $"Results for “{q}”");
                }
                break;
        }
    }

    // ================================================================ channel list

    private ItemVm? ItemFromEvent(RoutedEventArgs e) =>
        ItemsControl.ContainerFromElement(ItemList, e.OriginalSource as DependencyObject) is ListBoxItem c ? c.DataContext as ItemVm : null;

    private async void ItemList_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        if (ItemFromEvent(e) is { } it) await Open(it);
    }

    private async void ItemList_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter && ItemList.SelectedItem is ItemVm it)
        {
            e.Handled = true;
            await Open(it);
        }
    }

    private async void ItemList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        // The guide follows the selected channel.
        if (ItemList.SelectedItem is ItemVm { IsLive: true } it && it != _guideItem)
        {
            _guideItem = it;
            _guideDay = DateTime.Today;
            await LoadGuide();
        }
    }

    private async void Play_Click(object sender, RoutedEventArgs e)
    {
        if (ItemList.SelectedItem is ItemVm it) await Open(it);
    }

    private async void Favorite_Click(object sender, RoutedEventArgs e)
    {
        if (ItemList.SelectedItem is ItemVm it) await ToggleFavorite(it);
    }

    private async void Star_Click(object sender, RoutedEventArgs e)
    {
        e.Handled = true;
        if ((sender as FrameworkElement)?.Tag is ItemVm it) await ToggleFavorite(it);
    }

    private async Task ToggleFavorite(ItemVm it)
    {
        var on = await _repo.ToggleFavorite(it.Id);
        it.IsFavorite = on;
        if (on) _favorites.Add(it.Id); else _favorites.Remove(it.Id);
        if (_details?.Id == it.Id) UpdateDetailsFavorite();
        if (_mode == Mode.Favorites && !on) await LoadMode();
    }

    /// <summary>Play a channel/film, or open a series.</summary>
    private async Task Open(ItemVm it)
    {
        if (it.IsSeries)
        {
            await ShowDetails(it);
            return;
        }
        if (it.IsLive)
        {
            _vodPlayerOpen = false;
            _playlist = (ItemList.ItemsSource as IEnumerable<ItemVm>)?.Where(i => i.IsLive).ToList() ?? new List<ItemVm> { it };
            await PlayAt(Math.Max(0, _playlist.FindIndex(i => i.Id == it.Id)));
            return;
        }
        // A film or episode opened from a list (favorites/recent/search): play in the big player.
        _playlist = new List<ItemVm> { it };
        _vodPlayerOpen = true;
        await PlayAt(0);
    }

    // ================================================================ poster grid

    private void ShowGrid(List<ItemVm> items, string title)
    {
        _gridAll = items;
        _page = 0;
        GridTitle.Text = title;
        GridCount.Text = $"{items.Count:N0} items";
        RenderPage();
    }

    private void RenderPage()
    {
        var pages = Math.Max(1, (int)Math.Ceiling(_gridAll.Count / (double)_perPage));
        _page = Math.Clamp(_page, 0, pages - 1);
        var slice = _gridAll.Skip(_page * _perPage).Take(_perPage).ToList();
        PosterGrid.ItemsSource = slice;
        var from = _gridAll.Count == 0 ? 0 : _page * _perPage + 1;
        PageText.Text = $"{from:N0} – {_page * _perPage + slice.Count:N0} of {_gridAll.Count:N0}";
        PrevPage.IsEnabled = _page > 0;
        NextPage.IsEnabled = _page < pages - 1;
        GridScroll.ScrollToTop();
    }

    private void PerPage_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: string t } && int.TryParse(t, out var n))
        {
            _perPage = n;
            _page = 0;
            HighlightPerPage();
            RenderPage();
        }
    }

    private void HighlightPerPage()
    {
        foreach (var b in new[] { Per25, Per50, Per100 })
        {
            var on = (string)b.Tag == _perPage.ToString();
            b.Background = on ? (Brush)FindResource("CardSelected") : Brushes.Transparent;
            b.BorderBrush = on ? (Brush)FindResource("Accent") : (Brush)FindResource("Line");
        }
    }

    private void PrevPage_Click(object sender, RoutedEventArgs e) { _page--; RenderPage(); }
    private void NextPage_Click(object sender, RoutedEventArgs e) { _page++; RenderPage(); }

    private async void Poster_Click(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is ItemVm it) await ShowDetails(it);
    }

    // ================================================================ details page

    private static ImageSource? TryImage(string? url)
    {
        if (string.IsNullOrWhiteSpace(url) || !Uri.TryCreate(url, UriKind.Absolute, out var uri)) return null;
        try
        {
            return new BitmapImage(uri);
        }
        catch
        {
            return null;
        }
    }

    private async Task ShowDetails(ItemVm it)
    {
        _details = it;
        _episodes = new List<ItemVm>();
        DetTitle.Text = it.Name;
        DetInitials.Text = it.Initials;
        DetPlot.Text = it.Plot ?? "";
        var img = TryImage(it.Logo);
        DetPoster.ImageSource = img;
        DetBackdrop.ImageSource = img;
        DetChips.Children.Clear();
        foreach (var chip in new[] { it.Year, it.Genre, it.HasRating ? $"★ {it.Rating}" : null, it.Group })
        {
            if (string.IsNullOrWhiteSpace(chip)) continue;
            DetChips.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(6),
                BorderThickness = new Thickness(1),
                BorderBrush = (Brush)FindResource("Line"),
                Background = (Brush)FindResource("Card"),
                Padding = new Thickness(10, 4, 10, 4),
                Margin = new Thickness(0, 0, 8, 8),
                Child = new TextBlock { Text = chip, FontSize = 13, Foreground = chip.StartsWith('★') ? (Brush)FindResource("Rating") : (Brush)FindResource("Fg") },
            });
        }
        UpdateDetailsFavorite();
        EpisodesTitle.Visibility = it.IsSeries ? Visibility.Visible : Visibility.Collapsed;
        EpisodeList.ItemsSource = null;
        DetPlayText.Text = "Play";
        Layout();

        if (it.IsSeries)
        {
            await Run("Loading episodes…", async () =>
            {
                _episodes = await _repo.Episodes(it);
                if (_details != it) return;
                EpisodeList.ItemsSource = _episodes;
                EpisodesTitle.Text = $"Episodes ({_episodes.Count})";
                DetPlayText.Text = _episodes.Count > 0 ? "Play S" + (_episodes[0].Season ?? "1") + " E" + (_episodes[0].Episode ?? "1") : "No episodes";
                DetPlay.IsEnabled = _episodes.Count > 0;
            });
        }
        else
        {
            DetPlay.IsEnabled = true;
            var resume = await _repo.ResumeSeconds(it.Id);
            if (_details == it && resume > 60) DetPlayText.Text = $"Resume from {Fmt((long)resume * 1000)}";
        }
    }

    private void UpdateDetailsFavorite()
    {
        if (_details == null) return;
        var on = _favorites.Contains(_details.Id);
        DetFavIcon.Text = on ? "" : "";
        DetFavText.Text = on ? "In favorites" : "Add to favorites";
    }

    private void DetailsBack_Click(object sender, RoutedEventArgs e) => CloseDetails();

    private void CloseDetails()
    {
        _details = null;
        Layout();
    }

    private async void DetFav_Click(object sender, RoutedEventArgs e)
    {
        if (_details is { } it) await ToggleFavorite(it);
    }

    private async void DetPlay_Click(object sender, RoutedEventArgs e)
    {
        if (_details is not { } it) return;
        if (it.IsSeries)
        {
            if (_episodes.Count > 0) await PlayEpisode(_episodes[0]);
            return;
        }
        _playlist = new List<ItemVm> { it };
        _vodPlayerOpen = true;
        await PlayAt(0);
    }

    private async void Episode_Click(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is ItemVm ep) await PlayEpisode(ep);
    }

    private async Task PlayEpisode(ItemVm ep)
    {
        _playlist = _episodes;
        _vodPlayerOpen = true;
        await PlayAt(Math.Max(0, _episodes.IndexOf(ep)));
    }

    // ================================================================ player

    private async Task PlayAt(int index)
    {
        SaveProgress();
        if (_playlist.Count == 0) return;
        _playIndex = (index % _playlist.Count + _playlist.Count) % _playlist.Count;
        var it = _playlist[_playIndex];

        if (!Uri.TryCreate(it.Url, UriKind.Absolute, out var uri))
        {
            SetStatus($"“{it.Name}” has no valid stream address.");
            return;
        }
        using var media = new Media(_vlc, uri);
        media.AddOption($":http-user-agent={it.UserAgent ?? Repository.UserAgent}");
        if (it.Referrer != null) media.AddOption($":http-referrer={it.Referrer}");
        if (!it.IsLive)
        {
            var resume = await _repo.ResumeSeconds(it.Id);
            if (resume > 0) media.AddOption($":start-time={resume:0}");
        }
        Video.Visibility = Visibility.Visible;
        VideoHint.Visibility = Visibility.Collapsed;
        _player.Play(media);
        _ = _repo.RecordPlay(it.Id);
        SetStatus("");

        PlayingTitle.Text = it.Kind == "episode" && _details != null ? $"{_details.Name} — {it.Name}" : it.Name;
        SeekBar.Value = 0;
        Layout();
        if (it.IsLive)
        {
            if (ItemList.ItemsSource is IEnumerable<ItemVm> shown && shown.Contains(it))
            {
                ItemList.SelectedItem = it;
                ItemList.ScrollIntoView(it);
            }
            if (_guideItem != it)
            {
                _guideItem = it;
                _guideDay = DateTime.Today;
                await LoadGuide();
            }
        }
    }

    private void StopPlayback()
    {
        SaveProgress();
        _player.Stop();
        _playIndex = -1;
        _vodPlayerOpen = false;
        Video.Visibility = Visibility.Collapsed;
        VideoHint.Visibility = Visibility.Visible;
        Layout();
    }

    private void ClosePlayer_Click(object sender, RoutedEventArgs e) => CloseVodPlayer();

    private void CloseVodPlayer()
    {
        if (_fullscreen) ToggleFullscreen();
        StopPlayback(); // back to the details page (if any) or the list
    }

    private void PlayPause_Click(object sender, RoutedEventArgs e) => _player.Pause();

    private void SaveProgress()
    {
        if (Playing is not { IsLive: false } it) return;
        var len = _player.Length;
        if (len > 0) _ = _repo.SetProgress(it.Id, _player.Time / 1000.0, len / 1000.0);
    }

    private void OnTick()
    {
        _ticks++;
        if (Playing is { IsLive: false } && _player.Length > 0 && !SeekBar.IsMouseCaptureWithin)
        {
            SeekBar.Value = Math.Clamp((double)_player.Time / _player.Length, 0, 1);
            TimeText.Text = $"{Fmt(_player.Time)} / {Fmt(_player.Length)}";
        }
        if (_ticks % 15 == 0) SaveProgress();
        if (_ticks % 60 == 0)
        {
            if (ItemList.ItemsSource is IReadOnlyList<ItemVm> shown) _ = UpdateNowNext(shown);
            if (_guideItem != null && GuidePanel.Visibility == Visibility.Visible) _ = LoadGuide();
        }
    }

    private static string Fmt(long ms)
    {
        var t = TimeSpan.FromMilliseconds(Math.Max(0, ms));
        return t.TotalHours >= 1 ? t.ToString(@"h\:mm\:ss") : t.ToString(@"m\:ss");
    }

    private void SeekBar_PreviewMouseUp(object sender, MouseButtonEventArgs e)
    {
        if (_player.Length > 0) _player.Time = (long)(SeekBar.Value * _player.Length);
    }

    private void SeekBy(long ms)
    {
        if (Playing is { IsLive: false } && _player.Length > 0)
            _player.Time = Math.Clamp(_player.Time + ms, 0, _player.Length);
    }

    // ---------------------------------------------------------------- guide

    private async Task LoadGuide()
    {
        if (_guideItem is not { } it) return;
        var day = _guideDay;
        GuideDay.Text = day == DateTime.Today ? $"Today, {day:dddd d MMMM}" : day.ToString("dddd, d MMMM");
        var from = new DateTimeOffset(day).ToUnixTimeSeconds();
        var to = new DateTimeOffset(day.AddDays(1)).ToUnixTimeSeconds();
        var progs = await _repo.Guide(it.Id, from, to);
        if (_guideItem != it || _guideDay != day) return;
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var vms = progs.Select(p => new ProgrammeVm { P = p, Now = now }).ToList();
        GuideList.ItemsSource = vms;
        GuideEmpty.Visibility = vms.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        if (vms.FirstOrDefault(v => v.IsLive) is { } live)
        {
            GuideList.SelectedItem = live;
            GuideList.ScrollIntoView(live);
        }
    }

    private async void GuidePrev_Click(object sender, RoutedEventArgs e)
    {
        _guideDay = _guideDay.AddDays(-1);
        await LoadGuide();
    }

    private async void GuideNext_Click(object sender, RoutedEventArgs e)
    {
        _guideDay = _guideDay.AddDays(1);
        await LoadGuide();
    }

    // ================================================================ keyboard

    private void Fullscreen_Click(object sender, RoutedEventArgs e) => ToggleFullscreen();

    private async void Window_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        var typing = Keyboard.FocusedElement is TextBox;
        var ctrl = Keyboard.Modifiers.HasFlag(ModifierKeys.Control);
        switch (e.Key)
        {
            case Key.F11:
                ToggleFullscreen();
                e.Handled = true;
                break;
            case Key.Escape when _fullscreen:
                ToggleFullscreen();
                e.Handled = true;
                break;
            case Key.Escape when _vodPlayerOpen:
                CloseVodPlayer();
                e.Handled = true;
                break;
            case Key.Escape when _details != null:
                CloseDetails();
                e.Handled = true;
                break;
            case Key.Escape when typing && SearchBox.Text.Length > 0:
                SearchBox.Text = "";
                e.Handled = true;
                break;
            case Key.K when ctrl:
            case Key.F when ctrl:
                SearchBox.Focus();
                SearchBox.SelectAll();
                e.Handled = true;
                break;
            case Key.Space when !typing && Playing != null:
                _player.Pause();
                e.Handled = true;
                break;
            case Key.PageUp when Playing is { IsLive: true }:
            case Key.Up when _fullscreen && Playing is { IsLive: true }:
                await PlayAt(_playIndex - 1);
                e.Handled = true;
                break;
            case Key.PageDown when Playing is { IsLive: true }:
            case Key.Down when _fullscreen && Playing is { IsLive: true }:
                await PlayAt(_playIndex + 1);
                e.Handled = true;
                break;
            case Key.Left when !typing && (ctrl || _fullscreen || _vodPlayerOpen):
                SeekBy(-10_000);
                e.Handled = true;
                break;
            case Key.Right when !typing && (ctrl || _fullscreen || _vodPlayerOpen):
                SeekBy(10_000);
                e.Handled = true;
                break;
        }
    }

    private void ToggleFullscreen()
    {
        if (Playing == null && !_fullscreen) return; // nothing to show full screen
        _fullscreen = !_fullscreen;
        if (_fullscreen)
        {
            _savedState = WindowState;
            WindowStyle = WindowStyle.None;
            WindowState = WindowState.Normal; // required for Maximized to cover the taskbar
            WindowState = WindowState.Maximized;
        }
        else
        {
            WindowStyle = WindowStyle.SingleBorderWindow;
            WindowState = _savedState;
        }
        Layout();
    }
}
