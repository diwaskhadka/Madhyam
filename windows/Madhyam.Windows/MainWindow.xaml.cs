using System.ComponentModel;
using System.Net.Http;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;
using System.Windows.Threading;
using MadhyamCoreNet;
using LibVLCSharp.Shared;
using VlcMediaPlayer = LibVLCSharp.Shared.MediaPlayer;

namespace Madhyam.Windows;

/// <summary>
/// Shell:
///   rail | top bar: playlist switcher · compact search pill · actions
///        | Dashboard / Search / Favorites / Recent / Recently added / Settings → full-width pages (MainWindow.Pages.cs)
///        | Live TV → categories | channel list | player + guide          (MainWindow.Live.cs)
///        | Movies / Series → categories | poster grid → details page     (MainWindow.Vod.cs)
///          with the player inside the page and "About" + episodes below it.
/// One VLC video surface is moved between the live and the movie page as needed.
/// </summary>
public partial class MainWindow : Window
{
    public enum Mode { Dashboard, Search, Favorites, Recent, Movies, Live, Series, Added, Settings }

    private readonly Repository _repo = new();
    private readonly LibVLC _vlc = new("--no-video-title-show", "--network-caching=1500");
    private readonly VlcMediaPlayer _player;
    private readonly DispatcherTimer _searchDebounce = new() { Interval = TimeSpan.FromMilliseconds(300) };
    private readonly DispatcherTimer _tick = new() { Interval = TimeSpan.FromSeconds(1) };

    private List<Source> _sources = new();
    private Source? _source;
    private Mode _mode = Mode.Dashboard;
    private HashSet<string> _favorites = new();
    private int _loadVersion; // results of superseded loads are dropped
    private bool _navSilent;  // set while changing the rail selection from code

    private bool _fullscreen;
    private WindowState _savedState;
    private SettingsView? _settingsView;
    private int _ticks;
    private static AppSettings S => AppSettings.Current;

    /// <summary>Set by Settings → Reset: wipe the data folder after this window closes.</summary>
    public bool ResetOnExit { get; set; }
    private bool _restartOnExit;

    public Repository Repo => _repo;
    public IReadOnlyList<Source> SourcesList => _sources;
    public Source? CurrentSource => _source;

    public MainWindow()
    {
        InitializeComponent();
        _player = new VlcMediaPlayer(_vlc);
        Video.MediaPlayer = _player;
        _player.EncounteredError += (_, _) => Dispatcher.Invoke(() =>
            SetStatus("Playback failed: the stream could not be opened. Try another channel or refresh the playlist."));
        _player.Playing += (_, _) => Dispatcher.InvokeAsync(() =>
        {
            PlayPause.Content = "";
            if (!S.ShowSubtitles) _player.SetSpu(-1);
            _player.Mute = _muted; // a new audio output starts unmuted
        });
        _player.Paused += (_, _) => Dispatcher.InvokeAsync(() => PlayPause.Content = "");
        _player.EndReached += (_, _) => Dispatcher.InvokeAsync(OnEndReached);

        _searchDebounce.Tick += async (_, _) =>
        {
            _searchDebounce.Stop();
            await ApplySearch();
        };
        _tick.Tick += (_, _) => OnTick();
        // Smooth, elastic scrolling in the category, channel and guide lists and the grids.
        ElasticScroll.Attach(GroupList);
        ElasticScroll.Attach(ItemList);
        ElasticScroll.Attach(GuideList);
        ElasticScroll.Attach(GridScroll);
        ElasticScroll.Attach(VodScroll);
        _perPage = S.PerPage is 25 or 50 or 100 ? S.PerPage : 50;
        HighlightPerPage();
        RestoreWindowPlacement();
    }

    // ================================================================ window placement

    private void RestoreWindowPlacement()
    {
        if (S.WinWidth is double w and > 400 && S.WinHeight is double h and > 300)
        {
            var vs = new Rect(SystemParameters.VirtualScreenLeft, SystemParameters.VirtualScreenTop,
                              SystemParameters.VirtualScreenWidth, SystemParameters.VirtualScreenHeight);
            if (S.WinLeft is { } l && S.WinTop is { } t && vs.IntersectsWith(new Rect(l, t, w, h)))
            {
                WindowStartupLocation = WindowStartupLocation.Manual;
                Left = l;
                Top = t;
            }
            Width = w;
            Height = h;
        }
        if (S.WindowStartup == "Maximized" || (S.WindowStartup == "Last" && S.WinMaximized)) WindowState = WindowState.Maximized;
    }

    private void SaveWindowPlacement()
    {
        var b = WindowState == WindowState.Normal && !_fullscreen ? new Rect(Left, Top, Width, Height) : RestoreBounds;
        if (b.Width > 0)
        {
            S.WinLeft = b.Left;
            S.WinTop = b.Top;
            S.WinWidth = b.Width;
            S.WinHeight = b.Height;
        }
        S.WinMaximized = WindowState == WindowState.Maximized && !_fullscreen;
    }

    /// <summary>Close and start again (theme change, reset).</summary>
    public void RestartApp()
    {
        _restartOnExit = true;
        Close();
    }

    // ================================================================ lifecycle

    private static string? Arg(string name)
    {
        var args = Environment.GetCommandLineArgs();
        var i = Array.IndexOf(args, name);
        return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
    }

    private async void Window_Loaded(object sender, RoutedEventArgs e)
    {
        await Run("Loading…", async () =>
        {
            await _repo.Init();
            _favorites = await _repo.FavoriteIds();
            await ReloadSources();
        }, quiet: true);
        if (_source != null && Arg("--screenshot") == null) _ = CheckSource(_source, announce: false);
        var wanted = Arg("--mode") ?? (S.StartupView == "Last" ? S.LastView : S.StartupView);
        // Search is only a start view when asked for on the command line (CI screenshots).
        var start = Enum.TryParse<Mode>(wanted, true, out var parsed) && parsed != Mode.Settings && (parsed != Mode.Search || Arg("--mode") != null)
            ? parsed
            : Mode.Dashboard;
        NavButton(start).IsChecked = true; // → Nav_Checked → ShowMode
        _tick.Start();
        if (S.WindowStartup == "Fullscreen" && Arg("--screenshot") == null) ToggleFullscreen(force: true);

        if (Arg("--screenshot") is { } shot)
        {
            await CaptureForCi(shot);
            return;
        }
        if (_sources.Count == 0) OpenAddSource();
        await RefreshStaleGuides();
    }

    private RadioButton NavButton(Mode m) => m switch
    {
        Mode.Search => NavSearch,
        Mode.Favorites => NavFavorites,
        Mode.Recent => NavRecent,
        Mode.Movies => NavMovies,
        Mode.Live => NavLive,
        Mode.Series => NavSeries,
        Mode.Added => NavAdded,
        Mode.Settings => NavSettings,
        _ => NavDashboard,
    };

    /// <summary>Re-download guides older than the configured interval in the background.</summary>
    private async Task RefreshStaleGuides()
    {
        foreach (var s in _sources.ToList())
        {
            try
            {
                if (_repo.EpgAge(s.Id) is { } age && age < TimeSpan.FromHours(S.EpgRefreshHours)) continue;
                if (!await _repo.HasEpg(s.Id)) continue;
                await _repo.RefreshEpg(s.Id);
                RefreshListAppearance();
            }
            catch (Exception ex)
            {
                Repository.Log($"background guide refresh failed for {s.Name}: {ex.Message}");
            }
        }
        if (S.ExtraEpgUrls.Count > 0 && (Repository.CustomEpgAge() is not { } cage || cage > TimeSpan.FromHours(S.EpgRefreshHours)))
        {
            try
            {
                await _repo.RefreshCustomEpg(S.ExtraEpgUrls);
                RefreshListAppearance();
            }
            catch (Exception ex)
            {
                Repository.Log($"extra guide refresh failed: {ex.Message}");
            }
        }
    }

    /// <summary>
    /// CI helper: render the window to a PNG with demo data, then exit.
    /// --mode M · --open-first (details of the first poster) · --search q · --settings Page · --play-first
    /// </summary>
    private async Task CaptureForCi(string path)
    {
        await Task.Delay(1800);
        if (Arg("--search") is { } q)
        {
            SearchBox.Text = q;
            await ApplySearch();
            await Task.Delay(1200);
        }
        if (Arg("--settings") is { } page)
        {
            NavSettings.IsChecked = true;
            await Task.Delay(800);
            _settingsView?.Open(page);
            await Task.Delay(800);
        }
        if (Environment.GetCommandLineArgs().Contains("--open-first") && PosterGrid.Items.Count > 0 && PosterGrid.Items[0] is ItemVm first)
        {
            await ShowDetails(first);
            await Task.Delay(1500);
            if (Environment.GetCommandLineArgs().Contains("--play-first"))
            {
                if (first.IsSeries && _episodes.FirstOrDefault() is { } ep) await PlayEpisode(ep);
                else await PlayMovie();
                await Task.Delay(2500);
            }
        }
        if (_mode == Mode.Live && ItemList.Items.Count > 0)
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
        if (!ResetOnExit && Arg("--screenshot") == null)
        {
            _repo.Persist();
            SaveWindowPlacement();
            AppSettings.Save();
        }
        _player.Dispose();
        _vlc.Dispose();
        _repo.Dispose();
        if (ResetOnExit)
        {
            try
            {
                foreach (var f in System.IO.Directory.EnumerateFiles(App.DataDir)) System.IO.File.Delete(f);
                foreach (var d in System.IO.Directory.EnumerateDirectories(App.DataDir)) System.IO.Directory.Delete(d, true);
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Some files could not be removed: {ex.Message}", "Reset Madhyam");
            }
        }
        if ((_restartOnExit || ResetOnExit) && Environment.ProcessPath is { } exe) System.Diagnostics.Process.Start(exe);
    }

    public void SetStatus(string text) => Status.Text = text;

    /// <summary>Run a task with a status line; errors are shown, not thrown.</summary>
    public async Task Run(string label, Func<Task> work, bool quiet = false)
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

    public static string Explain(Exception ex) => ex switch
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
        Mode.Recent => "Recently viewed",
        Mode.Added => "Recently added",
        Mode.Settings => "Settings",
        _ => "Advanced search",
    };

    private static string? ModeKind(Mode m) => m switch
    {
        Mode.Live => "live",
        Mode.Movies => "movie",
        Mode.Series => "series",
        _ => null,
    };

    private bool IsSection => _mode is Mode.Live or Mode.Movies or Mode.Series;

    // ================================================================ playlists

    public async Task ReloadSources(int? select = null)
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
        if (live > 0) parts.Add($"{Compact(live)} channels");
        if (movies > 0) parts.Add($"{Compact(movies)} movies");
        if (series > 0) parts.Add($"{Compact(series)} series");
        PlaylistCount.Text = parts.Count > 0 ? string.Join(" · ", parts) : _source.KindLabel;
    }

    /// <summary>53,324 → "53K", 950 → "950".</summary>
    public static string Compact(int n) => n >= 10_000 ? $"{n / 1000:N0}K" : n.ToString("N0");

    /// <summary>Result of the last connection check per source: true = online, false = failed, missing = not checked.</summary>
    private readonly Dictionary<int, bool> _sourceOnline = new();

    private async Task CheckSource(Source src, bool announce)
    {
        try
        {
            var msg = await _repo.TestConnection(src.Id);
            _sourceOnline[src.Id] = true;
            if (announce) SetStatus($"{src.Name}: {msg}");
        }
        catch (Exception ex)
        {
            _sourceOnline[src.Id] = false;
            if (announce) SetStatus($"{src.Name}: {Explain(ex)}");
        }
    }

    /// <summary>playlist menu: playlists with status dot and a ⋮ menu, then shortcuts.</summary>
    private void PlaylistButton_Click(object sender, MouseButtonEventArgs e)
    {
        Popup? popup = null;
        var rows = new List<UiKit.MenuRow>();
        if (_source is { } cur)
        {
            rows.Add(new UiKit.MenuRow("\uE946", "Playlist info", false, () => _ = ShowPlaylistInfo(cur)));
            if (cur.Kind == "xtream") rows.Add(new UiKit.MenuRow("\uE77B", "Account info", false, () => _ = ShowAccountInfo(cur)));
        }
        rows.Add(new UiKit.MenuRow("\uE710", "Add playlist", false, OpenAddSource));

        var list = new StackPanel();
        foreach (var src in _sources) list.Children.Add(PlaylistRow(src, () => popup!.IsOpen = false));
        var top = new UIElement[]
        {
            list,
            new Border { Height = 1, Background = UiKit.B("Line"), Margin = new Thickness(6, 6, 6, 4) },
        };
        // Header + playlists first, then the shortcut rows.
        popup = UiKit.PopMenu(PlaylistButton, null, Array.Empty<UiKit.MenuRow>(), null, width: 320, fixedRows: rows,
            prefix: new UIElement[] { PlaylistHeader() }.Concat(top));
    }

    private FrameworkElement PlaylistHeader()
    {
        var g = new Grid { Margin = new Thickness(10, 6, 4, 6) };
        var left = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        left.Children.Add(new TextBlock { Text = "\uE8FD", FontFamily = UiKit.Icons, FontSize = 13, Foreground = UiKit.B("Muted"), VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 10, 0) });
        left.Children.Add(UiKit.Text("MY PLAYLISTS", 11, "Muted", FontWeights.Bold));
        g.Children.Add(left);
        var manage = UiKit.IconButton("\uE713", "Manage sources", () => { UiKit.OpenPopup?.SetCurrentValue(Popup.IsOpenProperty, false); OpenSettings("Sources"); }, 30);
        manage.FontSize = 13;
        manage.HorizontalAlignment = HorizontalAlignment.Right;
        g.Children.Add(manage);
        return g;
    }

    private FrameworkElement PlaylistRow(Source src, Action close)
    {
        var active = src.Id == _source?.Id;
        var g = new Grid { Margin = new Thickness(8, 6, 4, 6) };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var icon = new Grid { Width = 36, Height = 36 };
        icon.Children.Add(new Border { CornerRadius = new CornerRadius(10), Background = UiKit.B(active ? "RailActive" : "Chip") });
        icon.Children.Add(new TextBlock { Text = src.Kind == "xtream" ? "\uE774" : "\uE8FD", FontFamily = UiKit.Icons, FontSize = 15, Foreground = UiKit.B(active ? "Accent" : "Fg"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center });
        var dotColor = _sourceOnline.TryGetValue(src.Id, out var ok) ? (ok ? Color.FromRgb(0x22, 0xC5, 0x5E) : Color.FromRgb(0xEF, 0x44, 0x44)) : Color.FromRgb(0x9C, 0xA3, 0xAF);
        icon.Children.Add(new Ellipse
        {
            Width = 10,
            Height = 10,
            Fill = new SolidColorBrush(dotColor),
            Stroke = UiKit.B("Panel"),
            StrokeThickness = 2,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, -2, -2),
            ToolTip = _sourceOnline.TryGetValue(src.Id, out var o2) ? (o2 ? "Online" : "Not reachable") : "Not checked yet",
        });
        g.Children.Add(icon);

        var text = new StackPanel { Margin = new Thickness(12, 0, 6, 0), VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(UiKit.Text(src.Name, 14, active ? "Accent" : "Fg", FontWeights.SemiBold));
        var when = src.UpdatedAt > 0 ? $" · {DateTimeOffset.FromUnixTimeSeconds(src.UpdatedAt).LocalDateTime:MMM d, HH:mm}" : "";
        text.Children.Add(UiKit.Text($"{src.KindLabel}{when}", 11.5, active ? "Accent" : "Muted"));
        Grid.SetColumn(text, 1);
        g.Children.Add(text);

        var more = UiKit.IconButton("\uE712", "More", () => { }, 32);
        more.FontSize = 14;
        more.Click += (_, ev) =>
        {
            ev.Handled = true;
            close();
            // Show the playlist's actions in the same place once the list has closed.
            Dispatcher.BeginInvoke(new Action(() =>
            {
                var items = new List<UiKit.MenuRow> { new("\uE946", "Playlist info", false, () => _ = ShowPlaylistInfo(src)) };
                if (src.Kind == "xtream") items.Add(new("\uE77B", "Account info", false, () => _ = ShowAccountInfo(src)));
                items.Add(new("\uE701", "Check again", false, () => _ = CheckSource(src, announce: true)));
                items.Add(new("\uE72C", "Refresh playlist", false, () => _ = RefreshSource(src)));
                items.Add(new("", "-", false, () => { }));
                items.Add(new("\uE74D", "Remove playlist", false, () => _ = RemoveSource(src), GlyphBrush: "LiveRed", LabelBrush: "LiveRed"));
                UiKit.PopMenu(PlaylistButton, src.Name, items, width: 300);
            }), System.Windows.Threading.DispatcherPriority.Input);
        };
        Grid.SetColumn(more, 2);
        g.Children.Add(more);

        var row = new Border
        {
            CornerRadius = new CornerRadius(10),
            Background = active ? UiKit.B("CardSelected") : Brushes.Transparent,
            BorderThickness = new Thickness(3, 0, 0, 0),
            BorderBrush = active ? UiKit.B("Accent") : Brushes.Transparent,
            Child = g,
            Cursor = Cursors.Hand,
        };
        var normal = row.Background;
        row.MouseEnter += (_, _) => { if (!active) row.Background = UiKit.B("CardHover"); };
        row.MouseLeave += (_, _) => row.Background = normal;
        row.MouseLeftButtonUp += (_, ev) =>
        {
            ev.Handled = true;
            close();
            if (!active) _ = SwitchSource(src);
        };
        return row;
    }

    public async Task SwitchSource(Source s)
    {
        _source = s;
        await UpdatePlaylistHeader();
        if (IsSection) StopPlayback();
        await ShowMode();
    }

    public async Task ShowPlaylistInfo(Source s)
    {
        var live = await _repo.CountKind(s.Id, "live");
        var movies = await _repo.CountKind(s.Id, "movie");
        var series = await _repo.CountKind(s.Id, "series");
        var rows = new List<(string, string)>
        {
            ("Name", s.Name),
            ("Type", s.KindLabel),
            (s.Kind == "xtream" ? "Server" : "Location", s.Location),
        };
        if (s.Kind == "xtream") rows.Add(("Username", s.Username));
        rows.Add(("Live channels", live.ToString("N0")));
        rows.Add(("Movies", movies.ToString("N0")));
        rows.Add(("Series", series.ToString("N0")));
        if (S.SourceAdded.TryGetValue(s.Id, out var added)) rows.Add(("Added", DateTimeOffset.FromUnixTimeSeconds(added).LocalDateTime.ToString("d MMM yyyy, HH:mm")));
        if (s.UpdatedAt > 0) rows.Add(("Last updated", DateTimeOffset.FromUnixTimeSeconds(s.UpdatedAt).LocalDateTime.ToString("d MMM yyyy, HH:mm")));
        rows.Add(("Guide cached", _repo.EpgAge(s.Id) is { } age ? $"{age.TotalHours:0.#} hours ago" : "no"));
        rows.Add(("Connection", _sourceOnline.TryGetValue(s.Id, out var ok) ? (ok ? "Online" : "Not reachable") : "Not checked yet"));
        InfoWindow.Show(this, "Playlist info", rows);
    }

    public async Task ShowAccountInfo(Source s)
    {
        await Run("Checking account…", async () =>
        {
            var a = await _repo.Account(s.Id);
            InfoWindow.Show(this, "Account info", new List<(string, string)>
            {
                ("Status", a.Status),
                ("Expires", a.Expires?.ToString("d MMM yyyy, HH:mm") ?? "Never / unlimited"),
                ("Connections", $"{a.ActiveConnections} active of {a.MaxConnections} allowed"),
                ("Trial", a.IsTrial ? "Yes" : "No"),
                ("Created", a.Created?.ToString("d MMM yyyy") ?? "—"),
                ("Server time zone", a.Timezone.Length > 0 ? a.Timezone : "—"),
                ("Stream formats", a.Formats.Length > 0 ? a.Formats : "—"),
            });
        });
    }

    private void Add_Click(object sender, RoutedEventArgs e) => OpenAddSource();

    public async void OpenAddSource()
    {
        var dlg = new AddSourceWindow { Owner = this };
        if (dlg.ShowDialog() != true) return;
        await Run(dlg.IsXtream ? "Logging in and downloading the catalog…" : "Downloading playlist…", async () =>
        {
            var id = dlg.IsXtream
                ? await _repo.AddXtream(dlg.SourceName, dlg.Server, dlg.Username, dlg.Password)
                : await _repo.AddM3u(dlg.SourceName, dlg.PlaylistUrl);
            AppSettings.Save();
            await ReloadSources(id);
            await ShowMode();
            _settingsView?.Refresh();
        });
    }

    private async void Refresh_Click(object sender, RoutedEventArgs e)
    {
        if (_source is not { } s) return;
        await RefreshSource(s);
    }

    public async Task RefreshSource(Source s)
    {
        await Run($"Refreshing {s.Name}…", async () =>
        {
            await _repo.Refresh(s.Id);
            await ReloadSources(_source?.Id);
            if (_mode != Mode.Settings) await ShowMode();
        });
    }

    public async Task RemoveSource(Source s)
    {
        if (MessageBox.Show(this, $"Remove \"{s.Name}\" and its channels, movies and series?", "Madhyam", MessageBoxButton.YesNo) != MessageBoxResult.Yes) return;
        await Run($"Removing {s.Name}…", async () =>
        {
            StopPlayback();
            await _repo.RemoveSource(s.Id);
            S.SourceAdded.Remove(s.Id);
            S.SourceUserAgent.Remove(s.Id);
            AppSettings.Save();
            if (_source?.Id == s.Id) _source = null;
            await ReloadSources();
            if (_mode != Mode.Settings) await ShowMode();
        });
    }

    public void OpenSettings(string page)
    {
        NavSettings.IsChecked = true;
        _settingsView?.Open(page);
    }

    // ================================================================ layout

    /// <summary>Show the panels that belong to the current mode / view.</summary>
    private void Layout()
    {
        var page = _mode is Mode.Dashboard or Mode.Search or Mode.Favorites or Mode.Recent or Mode.Added or Mode.Settings;
        var vod = _mode is Mode.Movies or Mode.Series;
        var live = _mode == Mode.Live;
        var details = vod && _details != null;
        var fs = _fullscreen && (Playing != null || _trailerOpen);

        PageHost.Visibility = page && !fs ? Visibility.Visible : Visibility.Collapsed;
        var showCats = !page && !fs;
        CatsPanel.Visibility = showCats ? Visibility.Visible : Visibility.Collapsed;
        ColCats.Width = new GridLength(showCats ? 250 : 0);

        LiveLayout.Visibility = live || (fs && Playing is { IsLive: true }) ? Visibility.Visible : Visibility.Collapsed;
        ListPanel.Visibility = live && !fs ? Visibility.Visible : Visibility.Collapsed;
        ColList.Width = new GridLength(live && !fs ? 340 : 0);
        var timeline = S.GuideView != "List";
        GuidePanel.Visibility = live && !fs && !timeline ? Visibility.Visible : Visibility.Collapsed;
        TimelinePanel.Visibility = live && !fs && timeline ? Visibility.Visible : Visibility.Collapsed;

        GridPanel.Visibility = vod && !details && !fs ? Visibility.Visible : Visibility.Collapsed;
        VodPage.Visibility = details || (fs && (_vodPlaying || _trailerOpen)) ? Visibility.Visible : Visibility.Collapsed;
        VodPlayerBox.Visibility = _vodPlaying || _trailerOpen ? Visibility.Visible : Visibility.Collapsed;
        VodPlayerRow.Height = fs ? new GridLength(1, GridUnitType.Star) : GridLength.Auto;
        VodScroll.Visibility = fs ? Visibility.Collapsed : Visibility.Visible;
        VodHeader.Visibility = fs ? Visibility.Collapsed : Visibility.Visible;
        VodHeaderTitle.Visibility = _vodPlaying || _trailerOpen ? Visibility.Collapsed : Visibility.Visible; // shown inside the card instead
        // Full screen: the card becomes a bare black video surface.
        foreach (var (card, shadow, header, frame) in new[] { (VodCard, VodCardShadow, (FrameworkElement)VodCardHeader, VodVideoFrame), (LiveCard, LiveCardShadow, LiveCardHeader, LiveVideoFrame) })
        {
            card.Padding = fs ? new Thickness(0) : (card == VodCard ? new Thickness(16, 12, 16, 16) : new Thickness(14, 10, 14, 14));
            card.CornerRadius = new CornerRadius(fs ? 0 : 18);
            card.BorderThickness = new Thickness(fs ? 0 : 1);
            card.Background = fs ? Brushes.Black : (Brush)FindResource("Panel");
            shadow.Visibility = fs ? Visibility.Collapsed : Visibility.Visible;
            header.Visibility = fs ? Visibility.Collapsed : Visibility.Visible;
            frame.CornerRadius = new CornerRadius(fs ? 0 : 12);
        }
        VodPlayerBox.Margin = fs ? new Thickness(0) : new Thickness(18, 6, 18, 14);
        LiveCloseBtn.Visibility = Playing is { IsLive: true } ? Visibility.Visible : Visibility.Collapsed;
        LiveBadge.Visibility = Playing is { IsLive: true } ? Visibility.Visible : Visibility.Collapsed;
        Grid.SetRowSpan(VodPlayerBox, fs ? 3 : 1);
        Grid.SetRow(VodPlayerBox, fs ? 0 : 1);

        RailCol.Width = new GridLength(fs ? 0 : 64);
        TopRow.Height = new GridLength(fs ? 0 : 62);
        Body.Margin = fs ? new Thickness(0) : new Thickness(12, 6, 12, 12);
        SizePlayers();

        UpdateSearchPill();
    }

    private void UpdateSearchPill()
    {
        SectionLabel.Text = _mode switch
        {
            Mode.Live or Mode.Movies or Mode.Series when _details != null && _mode != Mode.Live => $"{ModeLabel(_mode)} / {_details.DisplayName}",
            Mode.Live or Mode.Movies or Mode.Series => _group is { } g ? $"{ModeLabel(_mode)} / {g}" : $"{ModeLabel(_mode)} / All items",
            Mode.Favorites or Mode.Recent or Mode.Settings or Mode.Added => ModeLabel(_mode),
            _ => "Advanced search",
        };
        SearchHint.Text = _mode switch
        {
            Mode.Live or Mode.Movies or Mode.Series => "Search in this section…",
            Mode.Favorites or Mode.Recent => "Filter this section…",
            Mode.Settings => "Search everything…",
            _ => "Search in this playlist…",
        };
        SearchHint.Visibility = SearchBox.Text.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        SearchClear.Visibility = SearchBox.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    // ================================================================ navigation

    private async void Nav_Checked(object sender, RoutedEventArgs e)
    {
        if (_navSilent || sender is not RadioButton { Tag: string tag }) return;
        var mode = Enum.Parse<Mode>(tag);
        // One video surface: leaving the section that is playing stops playback.
        if (Playing != null && mode != _mode) StopPlayback();
        CloseTrailer();
        _mode = mode;
        _details = null;
        _group = null;
        // A search belongs to the section it was typed in: leaving the section forgets it.
        _vodSearch = null;
        _searchHits = null;
        _liveSearch = null;
        CatFilter.Text = "";
        CatFilterBox.Visibility = Visibility.Collapsed;
        if (mode != Mode.Search) SearchBox.Text = "";
        _searchDebounce.Stop();
        if (mode is not (Mode.Search or Mode.Settings)) S.LastView = mode.ToString();
        await ShowMode();
        if (mode == Mode.Search) SearchBox.Focus();
    }

    /// <summary>Select a rail item from code without triggering a reload.</summary>
    private void SetNav(Mode m)
    {
        _navSilent = true;
        NavButton(m).IsChecked = true;
        _navSilent = false;
        _mode = m;
    }

    /// <summary>(Re)build whatever the current mode shows.</summary>
    public async Task ShowMode()
    {
        var version = ++_loadVersion;
        Layout();
        switch (_mode)
        {
            case Mode.Live or Mode.Movies or Mode.Series:
                await LoadCategories(version);
                break;
            case Mode.Settings:
                _settingsView ??= new SettingsView(this, _repo);
                PageHost.Content = _settingsView;
                _settingsView.Refresh();
                break;
            default:
                await ShowPage(version);
                break;
        }
    }

    // ================================================================ search pill

    private void SearchBox_Focus(object sender, KeyboardFocusChangedEventArgs e) =>
        SearchPill.BorderBrush = SearchBox.IsKeyboardFocused ? (Brush)FindResource("Accent") : Brushes.Transparent;

    private void SearchBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        UpdateSearchPill();
        _searchDebounce.Stop();
        // Sections and Favorites/Recent filter as you type; elsewhere Enter opens advanced search.
        if (IsSection || _mode is Mode.Favorites or Mode.Recent or Mode.Search) _searchDebounce.Start();
    }

    private async void SearchBox_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key != Key.Enter) return;
        e.Handled = true;
        _searchDebounce.Stop();
        if (_mode is Mode.Dashboard or Mode.Added or Mode.Settings && SearchBox.Text.Trim().Length > 0)
        {
            StopPlayback();
            _details = null;
            SetNav(Mode.Search);
            await ShowMode();
            return;
        }
        await ApplySearch();
    }

    private void SearchClear_Click(object sender, RoutedEventArgs e)
    {
        SearchBox.Text = "";
        SearchBox.Focus();
    }

    private async Task ApplySearch()
    {
        switch (_mode)
        {
            case Mode.Live:
                await SearchLive(SearchBox.Text.Trim());
                break;
            case Mode.Movies or Mode.Series:
                await SearchVod(SearchBox.Text.Trim());
                break;
            case Mode.Search or Mode.Favorites or Mode.Recent:
                await ShowPage(++_loadVersion);
                break;
        }
    }

    // ================================================================ hooks used by Settings / pages

    /// <summary>Re-render names, numbers and guide after a display setting changed.</summary>
    public void RefreshListAppearance()
    {
        if (ItemList.ItemsSource is IEnumerable<ItemVm> items)
        {
            var list = items.ToList();
            for (var i = 0; i < list.Count; i++)
            {
                list[i].Number = S.ShowNumbers ? i + 1 : 0;
                list[i].Refresh();
            }
            _ = UpdateNowNext(list);
        }
        if (_guideItem != null) _ = LoadGuide();
    }

    public void RefreshGuideView()
    {
        Layout();
        if (_guideItem != null) _ = LoadGuide();
    }

    public async Task ReloadFavorites()
    {
        _favorites = await _repo.FavoriteIds();
        RefreshListAppearance();
    }

    /// <summary>Settings → "Refresh all guides now": every playlist's guide plus the extra XMLTV sources.</summary>
    public async Task RefreshAllGuides()
    {
        foreach (var s in _sources.ToList())
        {
            try { await _repo.RefreshEpg(s.Id); }
            catch (Exception ex) { Repository.Log($"guide refresh failed for {s.Name}: {ex.Message}"); }
        }
        await _repo.RefreshCustomEpg(S.ExtraEpgUrls);
        RefreshListAppearance();
    }

    public async Task ToggleFavorite(ItemVm it)
    {
        var on = await _repo.ToggleFavorite(it.Id);
        it.IsFavorite = on;
        if (on) _favorites.Add(it.Id); else _favorites.Remove(it.Id);
        if (_details?.Id == it.Id) RenderVodPage();
        if (_mode == Mode.Favorites) await ShowPage(++_loadVersion);
    }

    public bool IsFavorite(string id) => _favorites.Contains(id);

    // ================================================================ keyboard

    private void Fullscreen_Click(object sender, RoutedEventArgs e) => ToggleFullscreen();

    private async void Window_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        var typing = Keyboard.FocusedElement is TextBox or PasswordBox;
        var ctrl = Keyboard.Modifiers.HasFlag(ModifierKeys.Control);
        if (e.Key == Key.Escape && UiKit.OpenPopup is { IsOpen: true } pop)
        {
            pop.IsOpen = false; // Esc closes the menu, not the player
            e.Handled = true;
            return;
        }
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
            case Key.Escape when _trailerOpen:
                CloseTrailer();
                e.Handled = true;
                break;
            case Key.Escape when _vodPlaying:
                StopPlayback();
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
                TogglePause();
                e.Handled = true;
                break;
            case Key.Up when !typing && _vodPlaying:
                ChangeVolume(5);
                e.Handled = true;
                break;
            case Key.Down when !typing && _vodPlaying:
                ChangeVolume(-5);
                e.Handled = true;
                break;
            case Key.J when !typing && Playing != null:
                ChangeAudioDelay(-50);
                e.Handled = true;
                break;
            case Key.K when !typing && !ctrl && Playing != null:
                ChangeAudioDelay(50);
                e.Handled = true;
                break;
            case Key.G when !typing && Playing != null:
                ChangeSubDelay(-50);
                e.Handled = true;
                break;
            case Key.H when !typing && Playing != null:
                ChangeSubDelay(50);
                e.Handled = true;
                break;
            case Key.M when !typing && Playing != null:
                ToggleMute();
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
            case Key.Left when !typing && (ctrl || _fullscreen || _vodPlaying):
                SeekBy(-10_000);
                e.Handled = true;
                break;
            case Key.Right when !typing && (ctrl || _fullscreen || _vodPlaying):
                SeekBy(10_000);
                e.Handled = true;
                break;
        }
    }

    private Rect _savedBounds;
    private ResizeMode _savedResize;

    /// <summary>
    /// Full screen in one step: cover the current monitor exactly (no restore-then-maximize
    /// double resize, which flashed the desktop). A black curtain hides the switch.
    /// </summary>
    private void ToggleFullscreen(bool force = false)
    {
        if (!force && Playing == null && !_trailerOpen && !_fullscreen) return; // nothing to show full screen
        _fullscreen = !_fullscreen;
        var curtain = FullscreenCurtain.Show(this);
        try
        {
            if (_fullscreen)
            {
                _savedState = WindowState;
                _savedResize = ResizeMode;
                _savedBounds = WindowState == WindowState.Normal ? new Rect(Left, Top, Width, Height) : RestoreBounds;
                var m = MonitorRect();
                WindowStyle = WindowStyle.None;
                ResizeMode = ResizeMode.NoResize;
                if (WindowState == WindowState.Maximized)
                {
                    // Set the target bounds first so leaving "maximized" lands directly on the monitor.
                    Left = m.Left; Top = m.Top; Width = m.Width; Height = m.Height;
                    WindowState = WindowState.Normal;
                }
                Left = m.Left; Top = m.Top; Width = m.Width; Height = m.Height;
                Topmost = true;
            }
            else
            {
                Topmost = false;
                WindowStyle = WindowStyle.SingleBorderWindow;
                ResizeMode = _savedResize;
                if (_savedBounds.Width > 0)
                {
                    Left = _savedBounds.Left; Top = _savedBounds.Top; Width = _savedBounds.Width; Height = _savedBounds.Height;
                }
                WindowState = _savedState;
            }
        }
        finally
        {
            // Always re-layout, even if the video library threw during the resize.
            FullBtn.Content = _fullscreen ? "\uE73F" : "\uE740";
            Layout();
            curtain.Lift();
        }
    }

    /// <summary>Bounds of the monitor the window is on, in WPF units.</summary>
    private Rect MonitorRect()
    {
        var hwnd = new System.Windows.Interop.WindowInteropHelper(this).Handle;
        var mon = NativeMethods.MonitorFromWindow(hwnd, 2 /* nearest */);
        var info = new NativeMethods.MonitorInfo { Size = System.Runtime.InteropServices.Marshal.SizeOf<NativeMethods.MonitorInfo>() };
        if (mon == IntPtr.Zero || !NativeMethods.GetMonitorInfo(mon, ref info))
            return new Rect(0, 0, SystemParameters.PrimaryScreenWidth, SystemParameters.PrimaryScreenHeight);
        var dpi = VisualTreeHelper.GetDpi(this);
        var r = info.Monitor;
        return new Rect(r.Left / dpi.DpiScaleX, r.Top / dpi.DpiScaleY, (r.Right - r.Left) / dpi.DpiScaleX, (r.Bottom - r.Top) / dpi.DpiScaleY);
    }

    private void OnTick()
    {
        _ticks++;
        if (_vodPlaying && _player.Length > 0 && !SeekBar.IsMouseCaptureWithin)
        {
            SeekBar.Value = Math.Clamp((double)_player.Time / _player.Length, 0, 1);
            TimePos.Text = Fmt(_player.Time);
            TimeLen.Text = Fmt(_player.Length);
        }
        if (_ticks % 15 == 0) SaveProgress();
        if (_ticks % 60 == 0)
        {
            if (_mode == Mode.Live && ItemList.ItemsSource is IEnumerable<ItemVm> shown) _ = UpdateNowNext(shown.ToList());
            if (_guideItem != null && _mode == Mode.Live) _ = LoadGuide();
        }
    }

    private static string Fmt(long ms)
    {
        var t = TimeSpan.FromMilliseconds(Math.Max(0, ms));
        return t.TotalHours >= 1 ? t.ToString(@"h\:mm\:ss") : t.ToString(@"m\:ss");
    }
}
