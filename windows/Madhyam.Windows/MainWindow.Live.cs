using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using LibVLCSharp.Shared;

namespace Madhyam.Windows;

/// <summary>Categories (all sections), Live TV list, the player core and the programme guide.</summary>
public partial class MainWindow
{
    /// <summary>Category list row. <see cref="IsAll"/> is the synthetic "All items" entry.</summary>
    public sealed record CategoryRow(string Name, int Count, bool IsAll = false);

    private List<CategoryRow> _allCategories = new();
    private string? _group;           // null = All items
    private bool _groupSilent;        // suppress SelectionChanged while filling the list
    private bool _countryTagging;     // Free TV country tags being fetched

    // Live list (paged: big "All items" lists load 400 at a time while scrolling)
    private readonly ObservableCollection<ItemVm> _liveItems = new();
    private int _liveTotal;
    private bool _liveLoading;
    private string? _liveSearch;
    private const int LivePage = 400;

    // Playback
    private List<ItemVm> _playlist = new();
    private int _playIndex = -1;
    private ItemVm? Playing => _playIndex >= 0 && _playIndex < _playlist.Count ? _playlist[_playIndex] : null;
    private bool _vodPlaying;
    private DateTime _guideDay = DateTime.Today;
    private ItemVm? _guideItem;

    // ================================================================ categories

    private string KindNow => ModeKind(_mode) ?? "live";

    /// <summary>Is the current playlist the Free TV one (iptv-org)?</summary>
    private bool IsFreeTvSource => _source != null && (S.FreeTvSourceId == _source.Id || FreeTv.IsFreeTv(_source.Location));

    /// <summary>Item field the category panel groups by: "country" for Free TV by country, else null (categories).</summary>
    private string? GroupFieldNow => _mode == Mode.Live && IsFreeTvSource && S.FreeTvGroupBy == "country" ? FreeTv.CountryField : null;

    /// <summary>The Categories | Countries switch at the top of the panel (Free TV in Live TV only).</summary>
    private void UpdateCatMode()
    {
        var show = _mode == Mode.Live && IsFreeTvSource;
        CatModeBox.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        CatManageBtn.Visibility = GroupFieldNow == null ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;
        CatModeBox.Content = UiKit.Segments(new[] { ("category", "Categories"), ("country", "Countries") },
            S.FreeTvGroupBy == "country" ? "country" : "category", v =>
            {
                if (S.FreeTvGroupBy == v) return;
                S.FreeTvGroupBy = v;
                AppSettings.Save();
                _group = null;
                CatFilter.Text = "";
                _ = LoadCategories(++_loadVersion);
            });
    }

    private async Task LoadCategories(int version, string? select = null)
    {
        var field = GroupFieldNow;
        CatsTitle.Text = field != null ? "Countries" : _mode switch { Mode.Live => "Live categories", Mode.Movies => "Movie categories", _ => "Series categories" };
        CatFilterHint.Text = field != null ? "Search countries" : "Search categories";
        UpdateCatMode();
        LiveSortBtn.ToolTip = S.LiveSortAz ? "Sorted A–Z (click for provider order)" : "Sort A–Z";
        var kind = KindNow;
        var groups = _source == null ? new List<Group>() : await _repo.Groups(_source.Id, kind, field);
        if (version != _loadVersion) return;
        if (field != null && groups.Count == 0 && _source != null && !_countryTagging)
        {
            // Older Free TV download without country tags: fetch them now, then show the countries.
            _countryTagging = true;
            CatsTitle.Text = "Countries (loading…)";
            try { await _repo.TagCountries(_source.Id); }
            finally { _countryTagging = false; }
            if (version != _loadVersion) return;
            groups = await _repo.Groups(_source.Id, kind, field);
            if (version != _loadVersion) return;
            CatsTitle.Text = "Countries";
        }
        // Hidden categories are category names; they don't apply to the country list.
        var hidden = _source == null || field != null ? new HashSet<string>() : S.Hidden(_source.Id, kind);
        var visible = groups.Where(g => !hidden.Contains(g.Name)).ToList();
        var order = S.CategorySort.TryGetValue(kind, out var o) ? o : "provider";
        if (field != null && order == "provider") order = "name_asc"; // countries have no provider order worth keeping
        visible = order switch
        {
            "name_asc" => visible.OrderBy(g => g.Name, StringComparer.CurrentCultureIgnoreCase).ToList(),
            "name_desc" => visible.OrderByDescending(g => g.Name, StringComparer.CurrentCultureIgnoreCase).ToList(),
            _ => visible,
        };
        if (field != null) visible = visible.OrderBy(g => g.Name == "Undefined" ? 1 : 0).ToList(); // channels with no country last
        // By country a channel counts once per country, so "All items" takes the real channel count.
        var allCount = field != null && _source != null ? await _repo.CountKind(_source.Id, kind) : visible.Sum(g => g.Count);
        if (version != _loadVersion) return;
        _allCategories = new List<CategoryRow> { new("All items", allCount, true) };
        _allCategories.AddRange(visible.Select(g => new CategoryRow(g.Name, g.Count)));
        ApplyCategoryFilter(select);
    }

    private void ApplyCategoryFilter(string? select = null)
    {
        var f = CatFilter.Text.Trim();
        var rows = f.Length == 0 ? _allCategories : _allCategories.Where(r => r.IsAll || r.Name.Contains(f, StringComparison.CurrentCultureIgnoreCase)).ToList();
        _groupSilent = true;
        GroupList.ItemsSource = rows;
        _groupSilent = false;
        var target = select ?? _group;
        var pick = rows.FirstOrDefault(r => target == null ? r.IsAll : !r.IsAll && r.Name == target) ?? rows.FirstOrDefault();
        // Live opens on the first real category (smaller list); grids open on All items.
        if (select == null && _group == null && _mode == Mode.Live && rows.Count > 1) pick = rows[1];
        GroupList.SelectedItem = pick; // → GroupList_SelectionChanged loads items
        if (pick == null) _ = ShowGroup(null);
    }

    private void CatFilter_TextChanged(object sender, TextChangedEventArgs e)
    {
        CatFilterHint.Visibility = CatFilter.Text.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        _groupSilent = true;
        var f = CatFilter.Text.Trim();
        GroupList.ItemsSource = f.Length == 0 ? _allCategories : _allCategories.Where(r => r.IsAll || r.Name.Contains(f, StringComparison.CurrentCultureIgnoreCase)).ToList();
        GroupList.SelectedItem = (GroupList.ItemsSource as IEnumerable<CategoryRow>)?.FirstOrDefault(r => _group == null ? r.IsAll : r.Name == _group);
        _groupSilent = false;
    }

    private void CatSearch_Click(object sender, RoutedEventArgs e)
    {
        var show = CatFilterBox.Visibility != Visibility.Visible;
        CatFilterBox.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (show) CatFilter.Focus();
        else CatFilter.Text = "";
    }

    private void CatSort_Click(object sender, RoutedEventArgs e)
    {
        var kind = KindNow;
        var cur = S.CategorySort.TryGetValue(kind, out var v) ? v : "provider";
        void Set(string order)
        {
            S.CategorySort[kind] = order;
            AppSettings.Save();
            _ = LoadCategories(++_loadVersion, _group);
        }
        UiKit.Menu(CatSortBtn, new (string, string, bool, Action)[]
        {
            ("\uE8FD", "Provider order", cur == "provider", () => Set("provider")),
            ("\uE74A", "Name A–Z", cur == "name_asc", () => Set("name_asc")),
            ("\uE74B", "Name Z–A", cur == "name_desc", () => Set("name_desc")),
        }, "Sort categories");
    }

    private async void CatManage_Click(object sender, RoutedEventArgs e)
    {
        if (_source == null) return;
        var kind = KindNow;
        var all = await _repo.Groups(_source.Id, kind);
        var hidden = S.Hidden(_source.Id, kind);
        var result = ManageCategoriesWindow.Show(this, all, hidden);
        if (result == null) return;
        S.HiddenCategories[AppSettings.HiddenKey(_source.Id, kind)] = result.ToList();
        AppSettings.Save();
        if (_group != null && result.Contains(_group)) _group = null;
        await LoadCategories(++_loadVersion, _group);
    }

    private async void GroupList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_groupSilent || GroupList.SelectedItem is not CategoryRow row) return;
        if (SearchBox.Text.Length > 0 || _vodSearch != null)
        {
            SearchBox.Text = "";
            _searchDebounce.Stop();
            _vodSearch = null;
            _searchHits = null;
            _liveSearch = null;
        }
        CloseDetailsSilently();
        await ShowGroup(row.IsAll ? null : row.Name);
    }

    private async Task ShowGroup(string? group)
    {
        _group = group;
        UpdateSearchPill();
        if (_mode == Mode.Live) await LoadLive(reset: true);
        else if (_mode is Mode.Movies or Mode.Series) await LoadGrid(resetPage: true);
    }

    private HashSet<string> HiddenNow => _source == null ? new HashSet<string>() : S.Hidden(_source.Id, KindNow);

    // ================================================================ live list

    private async Task LoadLive(bool reset)
    {
        if (_source == null)
        {
            ShowLiveEmpty("Add a playlist with the + button to start.");
            return;
        }
        if (_liveLoading) return;
        _liveLoading = true;
        var version = _loadVersion;
        try
        {
            if (reset)
            {
                _liveItems.Clear();
                _liveTotal = 0;
                _liveSearch = null;
                ItemList.ItemsSource = _liveItems;
                ListTitle.Text = _group ?? "All channels";
            }
            var (total, items) = await _repo.Query("live", _source.Id, _group, _group == null ? HiddenNow : null,
                S.LiveSortAz ? "name_asc" : "provider", 0, _liveItems.Count, LivePage, GroupFieldNow);
            if (version != _loadVersion) return;
            _liveTotal = total;
            AppendLive(items);
        }
        finally
        {
            _liveLoading = false;
        }
    }

    private void AppendLive(List<ItemVm> items)
    {
        var start = _liveItems.Count;
        foreach (var it in items)
        {
            it.Number = S.ShowNumbers ? ++start : 0;
            it.IsFavorite = _favorites.Contains(it.Id);
            _liveItems.Add(it);
        }
        ListCount.Text = _liveTotal > 0 ? _liveTotal.ToString("N0") : "";
        ListEmpty.Text = _liveSearch != null ? "No channels match." : "No channels in this category.";
        ListEmpty.Visibility = _liveItems.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        if (Playing is { IsLive: true } p && _liveItems.FirstOrDefault(i => i.Id == p.Id) is { } same) ItemList.SelectedItem = same;
        else if (Playing == null && _liveItems.Count > 0 && _guideItem == null)
        {
            _guideItem = _liveItems[0];
            _guideDay = DateTime.Today;
            _ = LoadGuide();
        }
        _ = UpdateNowNext(items);
    }

    private void ShowLiveEmpty(string text)
    {
        _liveItems.Clear();
        ItemList.ItemsSource = _liveItems;
        ListCount.Text = "";
        ListEmpty.Text = text;
        ListEmpty.Visibility = Visibility.Visible;
    }

    private async void ItemList_ScrollChanged(object sender, ScrollChangedEventArgs e)
    {
        // Load the next page when the user scrolls near the end of a long list.
        if (_liveSearch == null && !_liveLoading && _liveItems.Count < _liveTotal && e.VerticalOffset + e.ViewportHeight > e.ExtentHeight - 300)
            await LoadLive(reset: false);
    }

    private async Task SearchLive(string q)
    {
        if (q.Length == 0)
        {
            await LoadLive(reset: true);
            return;
        }
        var version = ++_loadVersion;
        var hits = await _repo.Search(q, "live");
        if (version != _loadVersion) return;
        if (_source != null) hits = hits.Where(h => h.SourceId == _source.Id).ToList();
        _liveItems.Clear();
        _liveSearch = q;
        _liveTotal = hits.Count;
        ListTitle.Text = $"Results for “{q}”";
        AppendLive(hits);
    }

    private void LiveSort_Click(object sender, RoutedEventArgs e)
    {
        void Set(bool az)
        {
            S.LiveSortAz = az;
            AppSettings.Save();
            LiveSortBtn.ToolTip = S.LiveSortAz ? "Sorted A–Z" : "Provider order";
            _ = LoadLive(reset: true);
        }
        UiKit.Menu(LiveSortBtn, new (string, string, bool, Action)[]
        {
            ("\uE8FD", "Provider order", !S.LiveSortAz, () => Set(false)),
            ("\uE74A", "Name A–Z", S.LiveSortAz, () => Set(true)),
        }, "Sort channels");
    }

    public async Task UpdateNowNext(IReadOnlyList<ItemVm> items)
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

    private ItemVm? ItemFromEvent(RoutedEventArgs e) =>
        ItemsControl.ContainerFromElement(ItemList, e.OriginalSource as DependencyObject) is ListBoxItem c ? c.DataContext as ItemVm : null;

    private async void ItemList_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        if (ItemFromEvent(e) is { } it) await PlayLive(it);
    }

    private async void ItemList_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter && ItemList.SelectedItem is ItemVm it)
        {
            e.Handled = true;
            await PlayLive(it);
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
        if (ItemList.SelectedItem is ItemVm it) await PlayLive(it);
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

    private async Task PlayLive(ItemVm it)
    {
        _playlist = _liveItems.ToList();
        var i = _playlist.FindIndex(x => x.Id == it.Id);
        if (i < 0)
        {
            _playlist = new List<ItemVm> { it };
            i = 0;
        }
        await PlayAt(i);
    }

    /// <summary>Open a live channel from anywhere (dashboard, search, favorites…): go to Live TV, its category, and play it.</summary>
    public async Task GoToLive(ItemVm it)
    {
        StopPlayback();
        SearchBox.Text = "";
        _searchDebounce.Stop();
        _vodSearch = null;
        _searchHits = null;
        SetNav(Mode.Live);
        _details = null;
        if (_source?.Id != it.SourceId && _sources.FirstOrDefault(s => s.Id == it.SourceId) is { } src)
        {
            _source = src;
            await UpdatePlaylistHeader();
        }
        Layout();
        var hidden = _source == null ? new HashSet<string>() : S.Hidden(_source.Id, "live");
        var version = ++_loadVersion;
        await LoadCategories(version, hidden.Contains(it.Group) ? null : it.Group);
        // Wait for the list of that category, then play the channel.
        for (var n = 0; n < 50 && (_liveLoading || _liveItems.Count == 0); n++) await Task.Delay(60);
        var target = _liveItems.FirstOrDefault(x => x.Id == it.Id) ?? it;
        ItemList.SelectedItem = target;
        ItemList.ScrollIntoView(target);
        await PlayLive(target);
    }

    // ================================================================ player core

    /// <summary>The single VLC surface lives either in the live panel or in the movie page.</summary>
    private void MoveVideo(Panel host)
    {
        if (Video.Parent == host) return;
        (Video.Parent as Panel)?.Children.Remove(Video);
        host.Children.Add(Video);
    }

    private async Task PlayAt(int index)
    {
        SaveProgress();
        if (_playlist.Count == 0) return;
        _playIndex = (index % _playlist.Count + _playlist.Count) % _playlist.Count;
        var it = _playlist[_playIndex];

        var url = it.Url;
        if (it.IsLive && S.LiveFormat != "Auto")
        {
            // Xtream live URLs end in .ts or .m3u8; the server offers both.
            var (from, to) = S.LiveFormat == "HLS" ? (".ts", ".m3u8") : (".m3u8", ".ts");
            if (url.EndsWith(from, StringComparison.OrdinalIgnoreCase) && url.Contains("/live/")) url = url[..^from.Length] + to;
        }
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri))
        {
            SetStatus($"“{it.DisplayName}” has no valid stream address.");
            return;
        }

        _player.Stop();
        _vodPlaying = !it.IsLive;
        MoveVideo(it.IsLive ? LiveVideoHost : VodVideoHost);
        Video.Visibility = Visibility.Visible;
        VideoOverlay.Visibility = Visibility.Visible;
        _videoTucked = false;
        VodTuckedHint.Visibility = Visibility.Collapsed;
        VideoHint.Visibility = it.IsLive ? Visibility.Collapsed : Visibility.Visible;

        using var media = new Media(_vlc, uri);
        var ua = it.UserAgent ?? (S.SourceUserAgent.TryGetValue(it.SourceId, out var sua) && sua.Length > 0 ? sua : Repository.UserAgent);
        media.AddOption($":http-user-agent={ua}");
        if (it.Referrer != null) media.AddOption($":http-referrer={it.Referrer}");
        media.AddOption($":network-caching={S.NetworkCachingMs}");
        media.AddOption($":live-caching={S.NetworkCachingMs}");
        media.AddOption(S.HardwareDecoding ? ":avcodec-hw=any" : ":avcodec-hw=none");
        if (S.AudioLanguage.Length > 0) media.AddOption($":audio-language={S.AudioLanguage}");
        if (S.SubtitleLanguage.Length > 0) media.AddOption($":sub-language={S.SubtitleLanguage}");
        if (!it.IsLive && S.ResumePlayback)
        {
            var resume = await _repo.ResumeSeconds(it.Id);
            if (resume > 0) media.AddOption($":start-time={resume:0}");
        }
        Layout();
        _player.Play(media);
        _player.SetRate(1);
        OnNewMedia(it);
        _ = _repo.RecordPlay(it.Id);
        SetStatus("");

        if (it.IsLive)
        {
            if (_liveItems.Contains(it))
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
        else
        {
            SeekBar.Value = 0;
            TimePos.Text = "0:00";
            TimeLen.Text = "";
            NextEpisodeBtn.Visibility = it.Kind == "episode" && _playIndex < _playlist.Count - 1 ? Visibility.Visible : Visibility.Collapsed;
            VodHeaderTitle.Text = it.Kind == "episode" && _details != null && _details.Id != it.Id
                ? $"{_details.DisplayName} · {SE(it)} · {it.DisplayName}"
                : it.DisplayName;
            RenderVodPage();
        }
    }

    public void StopPlayback()
    {
        SaveProgress();
        _player.Stop();
        _playIndex = -1;
        var wasVod = _vodPlaying;
        _vodPlaying = false;
        Video.Visibility = Visibility.Collapsed;
        _videoTucked = false;
        VodTuckedHint.Visibility = Visibility.Collapsed;
        // VLC's overlay window does not always follow the video when it is hidden or moved:
        // empty it so no controls are left floating over other pages.
        VideoOverlay.Visibility = Visibility.Collapsed;
        UiKit.OpenPopup?.SetCurrentValue(System.Windows.Controls.Primitives.Popup.IsOpenProperty, false);
        VideoHint.Visibility = Visibility.Visible;
        MoveVideo(LiveVideoHost);
        if (_fullscreen) ToggleFullscreen();
        Layout();
        if (wasVod && _details != null) RenderVodPage();
    }

    private void SaveProgress()
    {
        if (Playing is not { IsLive: false } it) return;
        var len = _player.Length;
        if (len > 0) _ = _repo.SetProgress(it.Id, _player.Time / 1000.0, len / 1000.0);
    }

    private async void OnEndReached()
    {
        // Films: mark finished. Episodes: continue with the next one.
        if (Playing is not { IsLive: false } it) return;
        await _repo.SetProgress(it.Id, Math.Max(1, _player.Length / 1000.0), Math.Max(1, _player.Length / 1000.0));
        if (it.Kind == "episode" && _playIndex < _playlist.Count - 1) await PlayAt(_playIndex + 1);
    }

    // ================================================================ guide (list + timeline)

    private async Task LoadGuide()
    {
        if (_guideItem is not { } it) return;
        if (Playing is not { IsLive: true })
        {
            LiveCardTitle.Text = it.DisplayName;
            LiveCardSub.Text = "Click the channel to start watching";
        }
        var day = _guideDay;
        GuideDay.Text = day == DateTime.Today ? $"Today, {day:dddd d MMMM}" : day.ToString("dddd, d MMMM");
        TlDay.Text = day == DateTime.Today ? $"Today · {day:ddd d MMM}" : day.ToString("ddd d MMM");
        var from = new DateTimeOffset(day).ToUnixTimeSeconds();
        var to = new DateTimeOffset(day.AddDays(1)).ToUnixTimeSeconds();
        var progs = await _repo.Guide(it.Id, from, to);
        if (_guideItem != it || _guideDay != day) return;
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var vms = progs.Select(p => new ProgrammeVm { P = p, Now = now }).ToList();
        RenderTimeline(it, day, vms);
        GuideList.ItemsSource = vms;
        GuideEmpty.Visibility = vms.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        if (vms.FirstOrDefault(v => v.IsLive) is { } live)
        {
            GuideList.SelectedItem = live;
            GuideList.ScrollIntoView(live);
        }
    }

    private const double PxPerMinute = 3.2;
    private DateTime _timelineDay;

    /// <summary>Horizontal guide ribbon: hour ruler, one block per programme, a red "now" line.</summary>
    private void RenderTimeline(ItemVm channel, DateTime day, List<ProgrammeVm> progs)
    {
        _timelineDay = day;
        TlChannel.Text = channel.DisplayName;
        var live = progs.FirstOrDefault(p => p.IsLive);
        TlNow.Text = live != null ? $"Now: {live.Title}  ·  {live.TimeRange}" : "";
        TlEmpty.Visibility = progs.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        TlCanvas.Children.Clear();
        TlCanvas.Width = 24 * 60 * PxPerMinute;

        var muted = (Brush)FindResource("Muted");
        var line = (Brush)FindResource("Line");
        for (var h = 0; h < 24; h++)
        {
            var x = h * 60 * PxPerMinute;
            var label = new TextBlock { Text = $"{h:00}:00", FontSize = 11, Foreground = muted };
            Canvas.SetLeft(label, x + 4);
            TlCanvas.Children.Add(label);
            var tick = new Border { Width = 1, Height = 8, Background = line };
            Canvas.SetLeft(tick, x);
            Canvas.SetTop(tick, 14);
            TlCanvas.Children.Add(tick);
        }

        var dayStart = new DateTimeOffset(day).ToUnixTimeSeconds();
        var dayEnd = dayStart + 86400;
        foreach (var p in progs)
        {
            var start = Math.Max(p.P.Start, dayStart);
            var stop = Math.Min(p.P.Stop, dayEnd);
            if (stop <= start) continue;
            var x = (start - dayStart) / 60.0 * PxPerMinute;
            var w = Math.Max(6, (stop - start) / 60.0 * PxPerMinute - 4);
            var block = new Border
            {
                Width = w,
                Height = 78,
                CornerRadius = new CornerRadius(8),
                BorderThickness = new Thickness(1.5),
                BorderBrush = p.IsLive ? (Brush)FindResource("Accent") : line,
                Background = (Brush)FindResource(p.IsLive ? "CardSelected" : "Card"),
                Padding = new Thickness(8, 6, 6, 6),
                Opacity = p.IsPast ? 0.55 : 1,
                ToolTip = string.IsNullOrWhiteSpace(p.Desc) ? $"{p.TimeRange}\n{p.Title}" : $"{p.TimeRange}\n{p.Title}\n\n{p.Desc}",
                ClipToBounds = true,
            };
            var inner = new StackPanel();
            inner.Children.Add(new TextBlock { Text = p.TimeRange, FontSize = 10.5, Foreground = muted, TextTrimming = TextTrimming.CharacterEllipsis });
            inner.Children.Add(new TextBlock
            {
                Text = p.Title,
                FontSize = 13,
                FontWeight = FontWeights.SemiBold,
                TextWrapping = TextWrapping.Wrap,
                MaxHeight = 36,
                TextTrimming = TextTrimming.CharacterEllipsis,
                Margin = new Thickness(0, 2, 0, 0),
                Foreground = (Brush)FindResource(p.IsLive ? "Accent" : "Fg"),
            });
            if (p.IsLive)
                inner.Children.Add(new TextBlock { Text = "● ON NOW", FontSize = 10, FontWeight = FontWeights.Bold, Foreground = (Brush)FindResource("LiveRed"), Margin = new Thickness(0, 2, 0, 0) });
            block.Child = inner;
            Canvas.SetLeft(block, x);
            Canvas.SetTop(block, 26);
            TlCanvas.Children.Add(block);
        }

        var nowSec = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        if (nowSec >= dayStart && nowSec < dayEnd)
        {
            var x = (nowSec - dayStart) / 60.0 * PxPerMinute;
            var red = (Brush)FindResource("LiveRed");
            var nowLine = new Border { Width = 2, Height = 90, Background = red };
            Canvas.SetLeft(nowLine, x - 1);
            Canvas.SetTop(nowLine, 18);
            TlCanvas.Children.Add(nowLine);
            var tag = new Border
            {
                CornerRadius = new CornerRadius(4),
                Background = red,
                Padding = new Thickness(4, 0, 4, 0),
                Child = new TextBlock { Text = DateTime.Now.ToString("HH:mm"), FontSize = 10, FontWeight = FontWeights.Bold, Foreground = Brushes.White },
            };
            Canvas.SetLeft(tag, x - 16);
            TlCanvas.Children.Add(tag);
        }
        Dispatcher.InvokeAsync(ScrollTimelineToNow, DispatcherPriority.Loaded);
    }

    private void ScrollTimelineToNow()
    {
        var dayStart = new DateTimeOffset(_timelineDay).ToUnixTimeSeconds();
        var nowSec = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var minutes = nowSec >= dayStart && nowSec < dayStart + 86400 ? (nowSec - dayStart) / 60.0 : 18 * 60;
        TlScroll.ScrollToHorizontalOffset(Math.Max(0, minutes * PxPerMinute - TlScroll.ViewportWidth * 0.3));
    }

    private void TimelineNow_Click(object sender, RoutedEventArgs e)
    {
        if (_guideDay != DateTime.Today)
        {
            _guideDay = DateTime.Today;
            _ = LoadGuide();
        }
        else
        {
            ScrollTimelineToNow();
        }
    }

    private void TlScroll_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        TlScroll.ScrollToHorizontalOffset(TlScroll.HorizontalOffset - e.Delta);
        e.Handled = true;
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
}
