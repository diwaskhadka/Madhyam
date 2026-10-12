using System.ComponentModel;
using System.Net.Http;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using MadhyamCoreNet;
using LibVLCSharp.Shared;

namespace Madhyam.Windows;

public partial class MainWindow : Window
{
    private enum Section { Live, Movies, Series, Favorites, Recent, Search }

    private static readonly (Section Section, string Label, string? Kind)[] Sections =
    {
        (Section.Live, "Live TV", "live"),
        (Section.Movies, "Movies", "movie"),
        (Section.Series, "Series", "series"),
        (Section.Favorites, "Favorites", null),
        (Section.Recent, "Recent", null),
        (Section.Search, "Search", null),
    };

    private readonly Repository _repo = new();
    private readonly LibVLC _vlc = new("--no-video-title-show", "--network-caching=1500");
    private readonly MediaPlayer _player;
    private readonly DispatcherTimer _searchDebounce = new() { Interval = TimeSpan.FromMilliseconds(300) };
    private readonly DispatcherTimer _tick = new() { Interval = TimeSpan.FromSeconds(1) };

    private List<Source> _sources = new();
    private Source? _source;
    private Section _section = Section.Live;
    private ItemVm? _openSeries;
    private string? _group;
    private int _loadVersion; // drops results of superseded loads

    private List<ItemVm> _playlist = new();
    private int _playIndex = -1;
    private ItemVm? Playing => _playIndex >= 0 && _playIndex < _playlist.Count ? _playlist[_playIndex] : null;
    private int _ticks;
    private bool _fullscreen;
    private WindowState _savedState;

    public MainWindow()
    {
        InitializeComponent();
        _player = new MediaPlayer(_vlc);
        Video.MediaPlayer = _player;
        _player.EncounteredError += (_, _) => Dispatcher.Invoke(() =>
            NowInfo.Text = "Playback failed: the stream could not be opened. Try another channel or refresh the source.");

        SectionList.ItemsSource = Sections.Select(s => s.Label).ToList();
        _searchDebounce.Tick += async (_, _) =>
        {
            _searchDebounce.Stop();
            if (_section == Section.Search) await LoadSection();
        };
        _tick.Tick += (_, _) => OnTick();
        VersionText.Text = $"engine {_repo.CoreVersion}";
    }

    // ------------------------------------------------------------- lifecycle

    private async void Window_Loaded(object sender, RoutedEventArgs e)
    {
        await Run("Loading…", async () =>
        {
            await _repo.Init();
            await ReloadSources();
        });
        if (_sources.Count == 0) OpenAddSource();
        _tick.Start();
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

    /// <summary>Run a task with a status line; errors are shown, not thrown.</summary>
    private async Task Run(string label, Func<Task> work)
    {
        Status.Text = label;
        try
        {
            await work();
            Status.Text = "";
        }
        catch (Exception ex)
        {
            Repository.Log(ex.ToString());
            Status.Text = Explain(ex);
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

    // ---------------------------------------------------------------- sources

    private async Task ReloadSources(int? select = null)
    {
        _sources = await _repo.Sources();
        SourceBox.ItemsSource = _sources;
        var pick = _sources.FirstOrDefault(s => s.Id == select) ?? _sources.FirstOrDefault();
        SourceBox.SelectedItem = pick; // triggers SourceBox_SelectionChanged
        if (pick == null)
        {
            _source = null;
            GroupList.ItemsSource = null;
            ItemList.ItemsSource = null;
        }
        if (SectionList.SelectedIndex < 0) SectionList.SelectedIndex = 0;
    }

    private async void SourceBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _source = SourceBox.SelectedItem as Source;
        await LoadSection();
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
        });
    }

    private async void Refresh_Click(object sender, RoutedEventArgs e)
    {
        if (_source is not { } s) return;
        RefreshButton.IsEnabled = false;
        await Run($"Refreshing {s.Name}…", async () =>
        {
            await _repo.Refresh(s.Id);
            await LoadSection();
        });
        RefreshButton.IsEnabled = true;
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (_source is not { } s) return;
        if (MessageBox.Show(this, $"Remove \"{s.Name}\" and its channels?", "Madhyam", MessageBoxButton.YesNo) != MessageBoxResult.Yes) return;
        await Run($"Removing {s.Name}…", async () =>
        {
            await _repo.RemoveSource(s.Id);
            await ReloadSources();
        });
    }

    // --------------------------------------------------------------- browsing

    private async void SectionList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SectionList.SelectedIndex < 0) return;
        _section = Sections[SectionList.SelectedIndex].Section;
        if (_section == Section.Search) SearchBox.Focus();
        await LoadSection();
    }

    private async Task LoadSection()
    {
        var version = ++_loadVersion;
        _openSeries = null;
        BackToSeries.Visibility = Visibility.Collapsed;
        var (_, label, kind) = Sections[(int)_section];
        ListTitle.Text = label;

        if (kind != null && _source != null)
        {
            var groups = await _repo.Groups(_source.Id, kind);
            if (version != _loadVersion) return;
            GroupList.ItemsSource = groups;
            GroupList.Visibility = Visibility.Visible;
            GroupList.SelectedIndex = groups.Count > 0 ? 0 : -1; // triggers GroupList_SelectionChanged
            if (groups.Count == 0) ShowItems(new List<ItemVm>());
            return;
        }

        GroupList.ItemsSource = null;
        var items = _section switch
        {
            Section.Favorites => await _repo.Favorites(),
            Section.Recent => await _repo.Recent(),
            Section.Search => await _repo.Search(SearchBox.Text),
            _ => new List<ItemVm>(),
        };
        if (version != _loadVersion) return;
        ShowItems(items);
    }

    private async void GroupList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (GroupList.SelectedItem is not Group g || _source == null) return;
        var kind = Sections[(int)_section].Kind;
        if (kind == null) return;
        var version = ++_loadVersion;
        _group = g.Name;
        _openSeries = null;
        BackToSeries.Visibility = Visibility.Collapsed;
        ListTitle.Text = g.Name;
        var items = await _repo.List(_source.Id, kind, g.Name);
        if (version != _loadVersion) return;
        ShowItems(items);
    }

    private void ShowItems(List<ItemVm> items)
    {
        ItemList.ItemsSource = items;
        _ = UpdateNowNext(items);
    }

    private async Task UpdateNowNext(IReadOnlyList<ItemVm> items)
    {
        var live = items.Where(i => i.IsLive).Take(500).ToList();
        if (live.Count == 0) return;
        try
        {
            var nn = await _repo.NowNext(live.Select(i => i.Id).ToList());
            foreach (var it in live)
            {
                if (nn.TryGetValue(it.Id, out var v) && v.Now is { } now)
                {
                    it.Detail = v.Next is { } next ? $"{now.Title}  ·  next: {next.Title}" : now.Title;
                }
            }
        }
        catch (Exception ex)
        {
            Repository.Log($"now/next failed: {ex.Message}");
        }
    }

    private void SearchBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        if (_section != Section.Search && SearchBox.IsKeyboardFocused && SearchBox.Text.Length > 0)
        {
            SectionList.SelectedIndex = (int)Section.Search; // start searching from anywhere
            return;
        }
        _searchDebounce.Stop();
        _searchDebounce.Start();
    }

    private async void BackToSeries_Click(object sender, RoutedEventArgs e)
    {
        if (GroupList.SelectedItem is Group && _source != null)
        {
            GroupList_SelectionChanged(GroupList, null!);
        }
        else
        {
            await LoadSection();
        }
    }

    private void ItemList_MouseDoubleClick(object sender, MouseButtonEventArgs e) => OpenSelected();

    private void ItemList_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            OpenSelected();
            e.Handled = true;
        }
    }

    private void Play_Click(object sender, RoutedEventArgs e) => OpenSelected();

    private async void Favorite_Click(object sender, RoutedEventArgs e)
    {
        if (ItemList.SelectedItem is not ItemVm it) return;
        var on = await _repo.ToggleFavorite(it.Id);
        Status.Text = on ? $"Added {it.Name} to favorites" : $"Removed {it.Name} from favorites";
        if (_section == Section.Favorites) await LoadSection();
    }

    private async void OpenSelected()
    {
        if (ItemList.SelectedItem is not ItemVm it) return;
        if (it.IsSeries)
        {
            await Run("Loading episodes…", async () =>
            {
                var eps = await _repo.Episodes(it);
                _openSeries = it;
                ListTitle.Text = it.Name;
                BackToSeries.Visibility = Visibility.Visible;
                ItemList.ItemsSource = eps;
            });
            return;
        }
        var list = (ItemList.ItemsSource as IEnumerable<ItemVm>)?.Where(i => !i.IsSeries).ToList() ?? new List<ItemVm> { it };
        _playlist = list;
        await PlayAt(Math.Max(0, list.FindIndex(i => i.Id == it.Id)));
    }

    // ----------------------------------------------------------------- player

    private async Task PlayAt(int index)
    {
        SaveProgress();
        if (_playlist.Count == 0) return;
        _playIndex = (index % _playlist.Count + _playlist.Count) % _playlist.Count;
        var it = _playlist[_playIndex];

        using var media = new Media(_vlc, new Uri(it.Url));
        media.AddOption($":http-user-agent={it.UserAgent ?? Repository.UserAgent}");
        if (it.Referrer != null) media.AddOption($":http-referrer={it.Referrer}");
        if (!it.IsLive)
        {
            var resume = await _repo.ResumeSeconds(it.Id);
            if (resume > 0) media.AddOption($":start-time={resume:0}");
        }
        _player.Play(media);
        _ = _repo.RecordPlay(it.Id);

        NowTitle.Text = it.Name;
        NowInfo.Text = it.IsLive ? "" : it.Plot ?? "";
        SeekBar.Visibility = it.IsLive ? Visibility.Collapsed : Visibility.Visible;
        if (it.IsLive) _ = ShowNowNext(it);
        if (ItemList.ItemsSource is IEnumerable<ItemVm> shown && shown.Contains(it)) ItemList.SelectedItem = it;
    }

    private async Task ShowNowNext(ItemVm it)
    {
        var nn = await _repo.NowNext(new[] { it.Id });
        if (Playing != it || !nn.TryGetValue(it.Id, out var v)) return;
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var parts = new List<string>();
        if (v.Now is { } n) parts.Add($"Now: {n.Title} ({n.Progress(now):P0})");
        if (v.Next is { } x) parts.Add($"Next: {x.Title} at {DateTimeOffset.FromUnixTimeSeconds(x.Start).LocalDateTime:t}");
        if (_playlist.Count > 1) parts.Add($"Channel {_playIndex + 1} of {_playlist.Count}");
        NowInfo.Text = string.Join("   ·   ", parts);
    }

    private void SaveProgress()
    {
        if (Playing is not { IsLive: false } it) return;
        var len = _player.Length;
        if (len > 0) _ = _repo.SetProgress(it.Id, _player.Time / 1000.0, len / 1000.0);
    }

    private void OnTick()
    {
        _ticks++;
        if (Playing is { } it && !it.IsLive && _player.Length > 0 && !SeekBar.IsMouseCaptureWithin)
        {
            SeekBar.Value = Math.Clamp((double)_player.Time / _player.Length, 0, 1);
            NowInfo.Text = $"{Fmt(_player.Time)} / {Fmt(_player.Length)}{(_player.IsPlaying ? "" : "   (paused)")}";
        }
        if (_ticks % 15 == 0) SaveProgress();
        if (_ticks % 60 == 0 && Playing is { IsLive: true } live) _ = ShowNowNext(live);
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

    // ---------------------------------------------------------------- keyboard

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
            case Key.F when ctrl:
                SectionList.SelectedIndex = (int)Section.Search;
                SearchBox.Focus();
                SearchBox.SelectAll();
                e.Handled = true;
                break;
            case Key.Space when !typing && Playing != null:
                _player.Pause();
                e.Handled = true;
                break;
            case Key.PageUp when Playing != null:
            case Key.Up when _fullscreen && Playing != null:
                await PlayAt(_playIndex - 1);
                e.Handled = true;
                break;
            case Key.PageDown when Playing != null:
            case Key.Down when _fullscreen && Playing != null:
                await PlayAt(_playIndex + 1);
                e.Handled = true;
                break;
            case Key.Left when (ctrl || _fullscreen) && !typing:
                SeekBy(-10_000);
                e.Handled = true;
                break;
            case Key.Right when (ctrl || _fullscreen) && !typing:
                SeekBy(10_000);
                e.Handled = true;
                break;
        }
    }

    private void ToggleFullscreen()
    {
        _fullscreen = !_fullscreen;
        var w = _fullscreen ? new GridLength(0) : (GridLength?)null;
        ColSide.Width = w ?? new GridLength(210);
        ColGroups.Width = w ?? new GridLength(250);
        ColItems.Width = w ?? new GridLength(380);
        InfoPanel.Visibility = _fullscreen ? Visibility.Collapsed : Visibility.Visible;
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
    }
}
