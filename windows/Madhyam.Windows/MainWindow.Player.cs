using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using LibVLCSharp.Shared;
using VlcMediaPlayer = LibVLCSharp.Shared.MediaPlayer;
using Microsoft.Win32;

namespace Madhyam.Windows;

/// <summary>
/// Player chrome drawn over the video: click to pause,
/// double-click for full screen, controls that hide after a few seconds and
/// come back on mouse movement, styled pop-ups for audio / subtitles / speed,
/// audio and subtitle delay, subtitle files and online subtitle search.
/// </summary>
public partial class MainWindow
{
    private readonly DispatcherTimer _barTimer = new() { Interval = TimeSpan.FromSeconds(2.8) };
    private readonly DispatcherTimer _osdTimer = new() { Interval = TimeSpan.FromSeconds(1.6) };
    private readonly HashSet<Window> _hookedOverlays = new();
    private Popup? _openPopup;
    private bool _muted;
    private int _audioDelayMs;
    private int _subDelayMs;
    private bool _barTimersReady;

    private void EnsureBarTimers()
    {
        if (_barTimersReady) return;
        _barTimersReady = true;
        _barTimer.Tick += (_, _) =>
        {
            _barTimer.Stop();
            if (PlayerBar.IsMouseOver || _openPopup is { IsOpen: true } || Playing == null || !_player.IsPlaying) return;
            HideBar();
        };
        _osdTimer.Tick += (_, _) =>
        {
            _osdTimer.Stop();
            Osd.Visibility = Visibility.Collapsed;
            BigPlay.Visibility = Visibility.Collapsed;
        };
    }

    // ================================================================ sizing (16:9)

    private void Players_SizeChanged(object sender, SizeChangedEventArgs e) => SizePlayers();

    /// <summary>Keep the video 16:9 inside a raised card instead of stretching it over the page.</summary>
    private void SizePlayers()
    {
        static bool Ok(double v) => double.IsFinite(v) && v > 0;
        var fs = _fullscreen && (Playing != null || _trailerOpen);

        // Live: card on top (16:9 video + header), guide below it.
        if (fs)
        {
            VideoRow.Height = new GridLength(1, GridUnitType.Star);
            GuideRow.Height = new GridLength(0);
            LiveCardBox.Width = double.NaN;
            LiveCardBox.HorizontalAlignment = HorizontalAlignment.Stretch;
            LiveCardBox.VerticalAlignment = VerticalAlignment.Stretch;
            LiveVideoBox.Height = double.NaN;
        }
        else if (Ok(PlayerPanel.ActualWidth) && Ok(PlayerPanel.ActualHeight))
        {
            const double chrome = 30, header = 62;
            var guide = TimelinePanel.Visibility == Visibility.Visible ? 180 : GuidePanel.Visibility == Visibility.Visible ? 230 : 0;
            var vw = Math.Min(PlayerPanel.ActualWidth - chrome, (PlayerPanel.ActualHeight - guide - header - chrome) * 16 / 9);
            vw = Math.Max(300, vw);
            VideoRow.Height = GridLength.Auto;
            GuideRow.Height = new GridLength(1, GridUnitType.Star);
            LiveCardBox.Width = vw + chrome;
            LiveCardBox.HorizontalAlignment = HorizontalAlignment.Center;
            LiveCardBox.VerticalAlignment = VerticalAlignment.Top;
            LiveVideoBox.Height = vw * 9 / 16;
        }

        // Movie / episode: centred card, leaving room to peek at "About" below.
        if (fs)
        {
            VodPlayerBox.Width = double.NaN;
            VodPlayerBox.HorizontalAlignment = HorizontalAlignment.Stretch;
            VodPlayerBox.VerticalAlignment = VerticalAlignment.Stretch;
            VodVideoFrame.Height = double.NaN;
        }
        else if (Ok(VodPage.ActualWidth) && Ok(VodPage.ActualHeight))
        {
            const double chrome = 34, header = 70;
            // A trailer gets a bigger card (less room kept for the page below).
            var peek = _trailerOpen ? 10 : 90;
            var vw = Math.Min(Math.Min(VodPage.ActualWidth - 36 - chrome, _trailerOpen ? 1500 : 1280), (VodPage.ActualHeight - 60 - header - chrome - peek) * 16 / 9);
            vw = Math.Max(320, vw);
            VodPlayerBox.Width = vw + chrome;
            VodPlayerBox.HorizontalAlignment = HorizontalAlignment.Center;
            VodPlayerBox.VerticalAlignment = VerticalAlignment.Top;
            VodVideoFrame.Height = vw * 9 / 16;
        }
        UpdatePlayerScroll();
    }

    // ================================================================ black around the picture

    private IntPtr _videoHostHwnd;
    private bool _videoSizeHooked;
    private DispatcherTimer? _repaintTimer;

    /// <summary>
    /// The video library hosts VLC in a native "Static" window, which paints the grey of an
    /// empty Windows control. VLC sizes its picture window to the picture and leaves the rest to
    /// its outer window, which it does not repaint after a resize. So a wide movie in full
    /// screen showed grey bars above and below instead of black. Paint the host black and
    /// repaint VLC's windows whenever the video area changes size.
    /// </summary>
    private void BlackVideoBackground()
    {
        if (Video.Template?.FindName("PART_PlayerHost", Video) is not System.Windows.Interop.HwndHost host) return;
        var hwnd = host.Handle;
        if (hwnd == IntPtr.Zero || hwnd == _videoHostHwnd) return;
        _videoHostHwnd = hwnd;
        var parent = NativeMethods.GetParent(hwnd);
        if (System.Windows.Interop.HwndSource.FromHwnd(parent) is { } source)
            source.AddHook((IntPtr _, int msg, IntPtr _, IntPtr lParam, ref bool handled) =>
            {
                if (msg != NativeMethods.WmCtlColorStatic || lParam != _videoHostHwnd) return IntPtr.Zero;
                handled = true;
                return NativeMethods.GetStockObject(NativeMethods.BlackBrush);
            });
        if (!_videoSizeHooked)
        {
            _videoSizeHooked = true;
            Video.SizeChanged += (_, _) => RepaintVideoBackground();
        }
        RepaintVideoBackground();
    }

    /// <summary>Repaint now, and again once VLC has moved its picture window (it does so a moment later).</summary>
    private void RepaintVideoBackground()
    {
        if (_videoHostHwnd == IntPtr.Zero) return;
        NativeMethods.RedrawWindow(_videoHostHwnd, IntPtr.Zero, IntPtr.Zero, NativeMethods.RedrawAll);
        if (_repaintTimer == null)
        {
            _repaintTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
            _repaintTimer.Tick += (_, _) =>
            {
                _repaintTimer.Stop();
                NativeMethods.RedrawWindow(_videoHostHwnd, IntPtr.Zero, IntPtr.Zero, NativeMethods.RedrawAll);
            };
        }
        _repaintTimer.Stop();
        _repaintTimer.Start();
    }

    // ================================================================ player scrolls with the page

    private void VodScroll_ScrollChanged(object sender, ScrollChangedEventArgs e) => UpdatePlayerScroll();

    /// <summary>
    /// Outside full screen the movie / episode player moves up with the page. WPF parts are
    /// clipped by the layout; the native video (VLC, WebView2) and VLC's control overlay are
    /// separate windows, so their part above the page is cut away with a window region.
    /// </summary>
    private void UpdatePlayerScroll()
    {
        var active = (_vodPlaying || _trailerOpen) && !_fullscreen;
        var off = active ? VodScroll.VerticalOffset : 0;
        if (!_fullscreen) VodPlayerBox.Margin = new Thickness(18, 6 - off, 18, 14);
        // Cut the card at the top of the page area so it never paints over the bar above it.
        var cut = active ? off - 6 : 0;
        VodPlayerBox.Clip = cut > 0 && VodPlayerBox.ActualHeight > 0
            ? new RectangleGeometry(new Rect(-30, cut, VodPlayerBox.ActualWidth + 60, Math.Max(0, VodPlayerBox.ActualHeight - cut + 40)))
            : null;
        var spacer = active && VodPlayerBox.ActualHeight > 0 ? VodPlayerBox.ActualHeight + 26 : 14;
        if (Math.Abs(VodContent.Margin.Top - spacer) > 0.5) VodContent.Margin = new Thickness(34, spacer, 34, 30);
        Dispatcher.BeginInvoke(ClipNativeVideo, DispatcherPriority.Render);
    }

    private void VodPlayerBox_SizeChanged(object sender, SizeChangedEventArgs e) => UpdatePlayerScroll();

    private bool _videoTucked;

    /// <summary>
    /// The picture is a native window and cannot be cut at the page edge (tested: window regions
    /// did not take effect on the VLC surface), so it would draw over the top bar. Once any part of
    /// it scrolls past the top, hide the picture and VLC's control overlay and show a placeholder in
    /// the card; sound keeps playing and the picture returns as soon as it is fully in view again.
    /// </summary>
    private void ClipNativeVideo()
    {
        var active = (_vodPlaying || _trailerOpen) && !_fullscreen && VodPlayerBox.IsVisible;
        double hidden = 0;
        if (active)
        {
            try { hidden = -VodVideoFrame.TranslatePoint(new Point(0, 0), VodScroll).Y; }
            catch (InvalidOperationException) { hidden = 0; }
        }
        var tuck = active && hidden > 0.5;
        if (tuck == _videoTucked) return;
        _videoTucked = tuck;
        var vlcHere = Video.Parent == VodVideoHost;
        if (vlcHere) Video.Visibility = tuck ? Visibility.Hidden : Visibility.Visible;
        if (vlcHere) VideoOverlay.Visibility = tuck ? Visibility.Collapsed : Visibility.Visible;
        if (_trailerView is { } tv && tv.Parent == VodVideoHost) tv.Visibility = tuck ? Visibility.Hidden : Visibility.Visible;
        VodTuckedHint.Visibility = tuck ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The wheel over the video goes to VLC's overlay window: scroll the page from there.</summary>
    private void Overlay_MouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (_fullscreen || !_vodPlaying) return;
        e.Handled = true;
        var pe = new MouseWheelEventArgs(e.MouseDevice, e.Timestamp, e.Delta) { RoutedEvent = UIElement.PreviewMouseWheelEvent };
        VodScroll.RaiseEvent(pe);
        if (!pe.Handled) VodScroll.ScrollToVerticalOffset(VodScroll.VerticalOffset - e.Delta);
    }

    // ================================================================ overlay: click, hover, keys

    private void Overlay_Loaded(object sender, RoutedEventArgs e)
    {
        EnsureBarTimers();
        // The overlay lives in VLC's own transparent window: forward its keys to ours.
        if (Window.GetWindow(VideoOverlay) is { } w && w != this && _hookedOverlays.Add(w))
        {
            w.PreviewKeyDown += Window_PreviewKeyDown;
            GuardOverlayScale(w);
        }
    }

    /// <summary>
    /// The video library scales its overlay window's content with a LayoutTransform equal to
    /// window size ÷ video size. While the window switches to full screen the video area is
    /// briefly 0×0, the scale becomes infinite / NaN and WPF throws on every frame
    /// ("Border should not return NaN values as its DesiredSize"). Replace such a scale at once.
    /// </summary>
    private static readonly HashSet<ScaleTransform> _watchedScales = new();

    private static void GuardOverlayScale(Window w)
    {
        void Hook()
        {
            if (VisualTreeHelper.GetChildrenCount(w) == 0 || VisualTreeHelper.GetChild(w, 0) is not FrameworkElement root) return;
            void Check()
            {
                try
                {
                    if (root.LayoutTransform is not ScaleTransform st) return;
                    if (!st.IsFrozen && _watchedScales.Add(st)) st.Changed += (_, _) => Check(); // in-place edits too
                    if (!Sane(st.ScaleX) || !Sane(st.ScaleY)) root.LayoutTransform = Transform.Identity;
                }
                catch (Exception ex)
                {
                    Repository.Log($"overlay scale guard: {ex.Message}"); // never let the guard itself crash
                }
            }
            System.ComponentModel.DependencyPropertyDescriptor
                .FromProperty(FrameworkElement.LayoutTransformProperty, typeof(FrameworkElement))
                .AddValueChanged(root, (_, _) => Check());
            Check();
        }
        if (w.IsLoaded) Hook();
        else w.Loaded += (_, _) => Hook();
    }

    private static bool Sane(double v) => double.IsFinite(v) && v > 0.05 && v < 20;

    private void Overlay_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.OriginalSource != VideoOverlay) return; // clicks on the control bar
        e.Handled = true;
        if (e.ClickCount == 2)
        {
            TogglePause(silent: true); // undo the first click's pause
            ToggleFullscreen();
            return;
        }
        TogglePause();
    }

    private void Overlay_MouseMove(object sender, MouseEventArgs e) => ShowBar();

    private void ShowBar()
    {
        EnsureBarTimers();
        PlayerBar.Opacity = 1;
        PlayerBar.IsHitTestVisible = true;
        VideoOverlay.Cursor = null;
        _barTimer.Stop();
        _barTimer.Start();
    }

    private void HideBar()
    {
        PlayerBar.Opacity = 0;
        PlayerBar.IsHitTestVisible = false;
        VideoOverlay.Cursor = Cursors.None;
        SeekTip.Visibility = Visibility.Collapsed;
    }

    /// <summary>Short message over the video ("Subtitle delay +100 ms").</summary>
    private void ShowOsd(string text)
    {
        EnsureBarTimers();
        OsdText.Text = text;
        Osd.Visibility = Visibility.Visible;
        _osdTimer.Stop();
        _osdTimer.Start();
    }

    /// <summary>Called by PlayAt when a new stream starts.</summary>
    private void OnNewMedia(ItemVm it)
    {
        var vod = !it.IsLive;
        SeekRow.Visibility = vod ? Visibility.Visible : Visibility.Collapsed;
        TimeRow.Visibility = vod ? Visibility.Visible : Visibility.Collapsed;
        Back10Btn.Visibility = vod ? Visibility.Visible : Visibility.Collapsed;
        Fwd10Btn.Visibility = vod ? Visibility.Visible : Visibility.Collapsed;
        SpeedBtn.Visibility = vod ? Visibility.Visible : Visibility.Collapsed;
        BarTitle.Text = it.IsLive ? it.DisplayName : "";
        BarLive.Visibility = it.IsLive ? Visibility.Visible : Visibility.Collapsed;
        SubsBtn.Visibility = it.IsLive ? Visibility.Collapsed : Visibility.Visible; // subtitles are a film / episode thing
        if (it.IsLive)
        {
            LiveCardTitle.Text = it.DisplayName;
            LiveCardSub.Text = it.HasNow ? $"Now: {it.NowTitle}" : it.Group;
        }
        else
        {
            SetVodCardTitles(it);
        }
        _audioDelayMs = 0;
        _subDelayMs = 0;
        _player.Mute = _muted;
        ShowBar();
    }

    // ================================================================ transport

    private void PlayPause_Click(object sender, RoutedEventArgs e) => TogglePause();

    private void TogglePause(bool silent = false)
    {
        if (Playing == null) return;
        var willPause = _player.IsPlaying;
        _player.SetPause(willPause);
        if (silent) return;
        BigPlayGlyph.Text = willPause ? "" : "";
        BigPlay.Visibility = Visibility.Visible;
        _osdTimer.Stop();
        _osdTimer.Start();
        ShowBar();
    }

    private void Back10_Click(object sender, RoutedEventArgs e) => SeekBy(-10_000);

    private void Fwd10_Click(object sender, RoutedEventArgs e) => SeekBy(10_000);

    private void SeekBy(long ms)
    {
        if (Playing is { IsLive: false } && _player.Length > 0)
        {
            _player.Time = Math.Clamp(_player.Time + ms, 0, _player.Length);
            ShowOsd(ms > 0 ? $"+{ms / 1000} s   {Fmt(_player.Time)}" : $"−{-ms / 1000} s   {Fmt(_player.Time)}");
            ShowBar();
        }
    }

    private async void NextEpisode_Click(object sender, RoutedEventArgs e)
    {
        if (_playIndex < _playlist.Count - 1) await PlayAt(_playIndex + 1);
    }

    private void SeekBar_PreviewMouseUp(object sender, MouseButtonEventArgs e)
    {
        if (Playing is { IsLive: false } && _player.Length > 0)
            _player.Time = (long)(SeekBar.Value * _player.Length);
    }

    /// <summary>Time preview above the seek bar where the mouse is.</summary>
    private void SeekBar_MouseMove(object sender, MouseEventArgs e)
    {
        if (_player.Length <= 0 || SeekBar.ActualWidth <= 0) return;
        var x = e.GetPosition(SeekBar).X;
        // While the window switches to full screen the position can be NaN; a NaN margin
        // breaks layout on every frame ("should not return NaN values as its DesiredSize").
        if (!double.IsFinite(x)) return;
        var ratio = Math.Clamp(x / SeekBar.ActualWidth, 0, 1);
        SeekTipText.Text = Fmt((long)(ratio * _player.Length));
        SeekTip.Visibility = Visibility.Visible;
        SeekTip.Margin = new Thickness(Math.Max(0, x - 24), -26, 0, 0);
        SeekBar.MouseLeave -= HideSeekTip;
        SeekBar.MouseLeave += HideSeekTip;
    }

    private void HideSeekTip(object sender, MouseEventArgs e) => SeekTip.Visibility = Visibility.Collapsed;

    private void VodClose_Click(object sender, RoutedEventArgs e)
    {
        if (_trailerOpen) CloseTrailer();
        else StopPlayback();
    }

    private void LiveClose_Click(object sender, RoutedEventArgs e) => StopPlayback();

    /// <summary>"S03E01 · Title" without the series name providers repeat in every episode title.</summary>
    private static string EpisodeLabel(string seriesName, ItemVm ep)
    {
        var name = ep.Name;
        foreach (var cut in new[] { seriesName, CleanTitle(seriesName), Ratings.CleanTitle(seriesName) })
            if (cut.Length > 2) name = name.Replace(cut, "", StringComparison.OrdinalIgnoreCase);
        name = System.Text.RegularExpressions.Regex.Replace(name, @"\bS\d{1,2}\s*E\d{1,3}\b", "", System.Text.RegularExpressions.RegexOptions.IgnoreCase);
        name = System.Text.RegularExpressions.Regex.Replace(name, @"^\s*[A-Z]{2,4}\s*[-|:]\s*", "");
        name = name.Trim(' ', '-', '·', '|', ':', '.');
        return name.Length > 1 ? $"{SE(ep)} · {name}" : SE(ep);
    }

    /// <summary>
    /// Card title: the film or series name, and for episodes "S03E01 · title"
    /// without repeating the series name the provider often puts in every episode title.
    /// </summary>
    private void SetVodCardTitles(ItemVm it)
    {
        if (it.Kind == "episode" && _details is { IsSeries: true } series)
        {
            VodCardLabel.Text = "NOW PLAYING";
            VodCardTitle.Text = series.Name;
            VodCardSub.Text = EpisodeLabel(series.Name, it);
        }
        else
        {
            VodCardLabel.Text = "NOW PLAYING";
            VodCardTitle.Text = it.Name;
            VodCardSub.Text = string.Join("  ·  ", new[] { it.Year, it.Genre }.Where(x => !string.IsNullOrEmpty(x)));
        }
    }

    // ================================================================ volume and mute

    private void Mute_Click(object sender, RoutedEventArgs e) => ToggleMute();

    /// <summary>Our own flag is the truth: libVLC applies mute asynchronously, so reading it back right away is stale.</summary>
    private void ToggleMute()
    {
        _muted = !_muted;
        _player.Mute = _muted;
        UpdateMuteGlyphs();
        ShowOsd(_muted ? "Muted" : $"Volume {(int)VolumeBar.Value}%");
    }

    private void UpdateMuteGlyphs()
    {
        var glyph = _muted || VolumeBar.Value == 0 ? "" : "";
        LiveMute.Content = glyph;
        VodMute.Content = glyph;
    }

    private void VolumeBar_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        // Raised once during InitializeComponent, before the player exists.
        if (_player == null || LiveMute == null) return;
        _player.Volume = (int)e.NewValue;
        if (_muted && e.NewValue > e.OldValue)
        {
            _muted = false;
            _player.Mute = false;
        }
        UpdateMuteGlyphs();
    }

    private void ChangeVolume(int delta)
    {
        VolumeBar.Value = Math.Clamp(VolumeBar.Value + delta, 0, 100);
        ShowOsd($"Volume {(int)VolumeBar.Value}%");
        ShowBar();
    }

    // ================================================================ delays

    private void ChangeAudioDelay(int ms)
    {
        _audioDelayMs = ms == 0 ? 0 : _audioDelayMs + ms;
        _player.SetAudioDelay(_audioDelayMs * 1000L);
        ShowOsd($"Audio delay {Delay(_audioDelayMs)}");
    }

    private void ChangeSubDelay(int ms)
    {
        _subDelayMs = ms == 0 ? 0 : _subDelayMs + ms;
        _player.SetSpuDelay(_subDelayMs * 1000L);
        ShowOsd($"Subtitle delay {Delay(_subDelayMs)}");
    }

    private static string Delay(int ms) => ms == 0 ? "0 ms" : $"{ms:+0;−0} ms";

    // ================================================================ pop-ups

    private void TrackPopup(Popup p)
    {
        _openPopup = p;
        p.Closed += (_, _) => ShowBar();
    }

    private void AudioTracks_Click(object sender, RoutedEventArgs e)
    {
        var tracks = _player.AudioTrackDescription;
        var cur = _player.AudioTrack;
        var rows = tracks.Where(t => t.Id >= 0)
            .Select(t => new UiKit.MenuRow("", string.IsNullOrEmpty(t.Name) ? $"Track {t.Id}" : t.Name, t.Id == cur, () => _player.SetAudioTrack(t.Id)))
            .ToList();
        if (rows.Count == 0) rows.Add(new UiKit.MenuRow("", "Default audio", true, () => { }));
        var extra = new UIElement[]
        {
            new Border { Height = 1, Background = UiKit.B("Line"), Margin = new Thickness(6, 5, 6, 5) },
            UiKit.Stepper("", "Audio delay  (J / K)", () => Delay(_audioDelayMs), () => ChangeAudioDelay(-50), () => ChangeAudioDelay(50), () => ChangeAudioDelay(0)),
        };
        TrackPopup(UiKit.PopMenu(AudioBtn, "Audio", rows, extra, above: true, width: 300, rowsMaxHeight: 230));
    }

    private void Subtitles_Click(object sender, RoutedEventArgs e)
    {
        var tracks = _player.SpuDescription;
        var cur = _player.Spu;
        // Only the language list scrolls; file / online / delay always stay in view.
        var rows = new List<UiKit.MenuRow> { new("\uE711", "Off", cur < 0 && _appSub == null, () => SelectVlcSubtitle(-1)) };
        rows.AddRange(tracks.Where(t => t.Id >= 0)
            .Select(t => new UiKit.MenuRow("\uE774", string.IsNullOrEmpty(t.Name) ? $"Track {t.Id}" : t.Name, t.Id == cur && _appSub == null, () => SelectVlcSubtitle(t.Id))));
        rows.AddRange(_appSubs.Select(f => new UiKit.MenuRow("\uE8A5", f.Name, f == _appSub, () => SelectAppSubtitle(f))));
        var fixedRows = new[]
        {
            new UiKit.MenuRow("", "-", false, () => { }),
            new UiKit.MenuRow("\uE8E5", "Load subtitle file…", false, LoadSubtitleFile),
            new UiKit.MenuRow("\uE721", "Search subtitles online…", false, SearchSubtitlesOnline, "OpenSubtitles.com"),
            new UiKit.MenuRow("\uE8D2", "Subtitle style…", false, OpenSubtitleStyle, $"{SubtitleStyle.PtLabel(S.SubFontSizePt)} · {S.SubFont}"),
        };
        var extra = new UIElement[]
        {
            new Border { Height = 1, Background = UiKit.B("Line"), Margin = new Thickness(6, 5, 6, 5) },
            UiKit.Stepper("\uE916", "Subtitle delay  (G / H)", () => Delay(_subDelayMs), () => ChangeSubDelay(-50), () => ChangeSubDelay(50), () => ChangeSubDelay(0)),
        };
        TrackPopup(UiKit.PopMenu(SubsBtn, "Subtitles", rows, extra, above: true, width: 330, fixedRows: fixedRows, rowsMaxHeight: 230));
    }

    private void Speed_Click(object sender, RoutedEventArgs e)
    {
        var cur = _player.Rate;
        var speeds = new[] { 0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f };
        TrackPopup(UiKit.PopMenu(SpeedBtn, "Playback speed", speeds.Select(s => new UiKit.MenuRow(
            s < 1 ? "" : s == 1 ? "" : "", s == 1f ? "Normal (1×)" : $"{s:0.##}×", Math.Abs(s - cur) < 0.01,
            () => { _player.SetRate(s); ShowOsd($"Speed {s:0.##}×"); })), above: true, width: 220));
    }

    // ================================================================ subtitle files

    private void LoadSubtitleFile()
    {
        var dlg = new OpenFileDialog { Filter = "Subtitles (*.srt;*.ass;*.ssa;*.vtt;*.sub;*.idx)|*.srt;*.ass;*.ssa;*.vtt;*.sub;*.idx|All files|*.*" };
        if (dlg.ShowDialog(this) != true) return;
        UseSubtitle(dlg.FileName);
    }

    /// <summary>
    /// A subtitle file for the playing film. Text formats are drawn by the app (so style changes
    /// show at once); others, such as VobSub pictures, are handed to VLC.
    /// </summary>
    private void UseSubtitle(string path)
    {
        if (Playing == null) return;
        if (SubtitleFile.TryLoad(path) is { } file)
        {
            _appSubs.Add(file);
            SelectAppSubtitle(file);
            ShowOsd($"Subtitles: {file.Name}");
            return;
        }
        var ok = _player.AddSlave(MediaSlaveType.Subtitle, new Uri(path).AbsoluteUri, true);
        if (ok)
        {
            _subFiles.Add(path);
            _appSub = null;
            DrawSubtitles(force: true);
        }
        ShowOsd(ok ? $"Subtitles: {System.IO.Path.GetFileName(path)}" : "These subtitles could not be loaded.");
    }

    // ================================================================ app-drawn subtitles

    /// <summary>Subtitle files the app draws itself for the current film, and the one showing (null: none).</summary>
    private readonly List<SubtitleFile> _appSubs = new();
    private SubtitleFile? _appSub;
    private DispatcherTimer? _subClock;
    private string _subDrawn = "";
    // VLC's time moves in steps; between them the clock runs on from the last step.
    private long _subLastVlcMs = -1;
    private readonly System.Diagnostics.Stopwatch _subSince = new();

    private void SelectAppSubtitle(SubtitleFile file)
    {
        _appSub = file;
        _player.SetSpu(-1);
        if (_subClock == null)
        {
            _subClock = new DispatcherTimer(DispatcherPriority.Render) { Interval = TimeSpan.FromMilliseconds(40) };
            _subClock.Tick += (_, _) => DrawSubtitles();
        }
        _subClock.Start();
        DrawSubtitles(force: true);
    }

    /// <summary>A track inside the film (VLC draws it), or -1 for none.</summary>
    private void SelectVlcSubtitle(int id)
    {
        _appSub = null;
        _subClock?.Stop();
        DrawSubtitles(force: true);
        _player.SetSpu(id);
        // The style may have changed while the app drew a file: bring VLC's up to date.
        if (id >= 0 && VlcStyleKey(_vlcHeight) != _vlcStyle)
        {
            ShowOsd("Applying subtitle style…");
            Restyle();
        }
    }

    /// <summary>New film: no subtitle files yet.</summary>
    private void ClearAppSubtitles()
    {
        _appSubs.Clear();
        _appSub = null;
        _subClock?.Stop();
        DrawSubtitles(force: true);
    }

    /// <summary>Playback time for the subtitles, smoothed between VLC's time updates, less the subtitle delay.</summary>
    private long SubtitleClock()
    {
        var vlc = _player.Time;
        if (vlc != _subLastVlcMs || !_player.IsPlaying)
        {
            _subLastVlcMs = vlc;
            _subSince.Restart();
            return vlc - _subDelayMs;
        }
        var ahead = (long)(Math.Min(_subSince.ElapsedMilliseconds, 1000) * _player.Rate);
        return vlc + ahead - _subDelayMs;
    }

    /// <summary>The part of the video area the picture fills (it is letterboxed to keep its shape).</summary>
    private Rect PictureRect()
    {
        double w = SubLayer.ActualWidth, h = SubLayer.ActualHeight;
        uint vw = 0, vh = 0;
        try
        {
            _player.Size(0, ref vw, ref vh);
        }
        catch { /* no video yet */ }
        if (vw == 0 || vh == 0 || w <= 0 || h <= 0) return new Rect(0, 0, w, h);
        var aspect = (double)vw / vh;
        return w / h > aspect
            ? new Rect((w - h * aspect) / 2, 0, h * aspect, h)
            : new Rect(0, (h - w / aspect) / 2, w, w / aspect);
    }

    /// <summary>Shows the captions due now; only rebuilt when what is shown, the style or the size changes.</summary>
    private void DrawSubtitles(bool force = false)
    {
        if (_appSub == null || Playing == null)
        {
            if (SubLayer.Children.Count > 0) SubLayer.Children.Clear();
            _subDrawn = "";
            return;
        }
        var cues = _appSub.At(SubtitleClock());
        var pic = PictureRect();
        // Above the controls while they show, like the other apps.
        var lift = PlayerBar.Opacity > 0 && PlayerBar.IsVisible ? Math.Max(0, PlayerBar.ActualHeight - 30 - (SubLayer.ActualHeight - pic.Bottom)) : 0;
        var key = $"{string.Join(",", cues.Select(c => c.StartMs))}|{pic}|{lift}|{SubtitleStyle.Signature()}";
        if (!force && key == _subDrawn) return;
        _subDrawn = key;
        SubLayer.Children.Clear();
        if (cues.Count == 0) return;
        var caption = SubtitleStyle.Caption(cues.SelectMany(c => c.Lines), pic.Height);
        caption.VerticalAlignment = VerticalAlignment.Bottom;
        caption.MaxWidth = pic.Width * 0.9;
        caption.Margin = new Thickness(0, 0, 0, SubLayer.ActualHeight - pic.Bottom + Math.Max(SubtitleStyle.BottomMargin(pic.Height), lift));
        SubLayer.Children.Add(caption);
    }

    // ================================================================ subtitle style

    /// <summary>Subtitle files VLC draws for the current film (formats the app can't read), added again when the media restarts.</summary>
    private readonly List<string> _subFiles = new();
    private DispatcherTimer? _styleTimer;
    private int? _videoHeight; // last picture height seen, for the subtitle margin

    /// <summary>The media for an item, with the user's network, decoding, language and subtitle style options.</summary>
    private Media BuildMedia(ItemVm it, Uri uri, double startSec)
    {
        var media = new Media(_vlc, uri);
        var ua = it.UserAgent ?? (S.SourceUserAgent.TryGetValue(it.SourceId, out var sua) && sua.Length > 0 ? sua : Repository.UserAgent);
        media.AddOption($":http-user-agent={ua}");
        if (it.Referrer != null) media.AddOption($":http-referrer={it.Referrer}");
        media.AddOption($":network-caching={S.NetworkCachingMs}");
        media.AddOption($":live-caching={S.NetworkCachingMs}");
        media.AddOption(S.HardwareDecoding ? ":avcodec-hw=any" : ":avcodec-hw=none");
        if (S.AudioLanguage.Length > 0) media.AddOption($":audio-language={S.AudioLanguage}");
        if (S.SubtitleLanguage.Length > 0) media.AddOption($":sub-language={S.SubtitleLanguage}");
        if (startSec > 0) media.AddOption($":start-time={startSec.ToString("0", System.Globalization.CultureInfo.InvariantCulture)}");
        return media;
    }

    /// <summary>Height of the picture now playing, for the subtitle margin (null before the first frame).</summary>
    private int? VideoHeight()
    {
        uint w = 0, h = 0;
        try
        {
            if (_player.Size(0, ref w, ref h) && h > 0) _videoHeight = (int)h;
        }
        catch { /* no video yet */ }
        return _videoHeight;
    }

    /// <summary>Player menu → "Subtitle style…": the dialog, with each change shown on the video.</summary>
    private void OpenSubtitleStyle() => SubtitleStyle.ShowDialog(this, OnSubtitleStyleChanged);

    /// <summary>
    /// A style change during playback. Subtitle files the app draws are simply redrawn. For a
    /// track inside the film, which VLC draws, colours change live (from the next line); other
    /// options are read by VLC only when a media starts, so those restart the film where it is.
    /// With no VLC track showing nothing restarts: VLC catches up when one is chosen.
    /// </summary>
    private void OnSubtitleStyleChanged()
    {
        DrawSubtitles(force: true);
        if (_appSub != null || _player.Spu < 0) return;
        if (VlcStyleKey(_vlcHeight) == _vlcStyle && VlcLive.Apply(_player))
        {
            _styleTimer?.Stop();
            return;
        }
        RestyleSoon();
    }

    /// <summary>
    /// VLC reads most of the subtitle style when a media starts, so a change restarts the film where it
    /// is (same subtitle and audio track, same loaded subtitle files). Quick changes are batched.
    /// </summary>
    private void RestyleSoon()
    {
        if (_styleTimer == null)
        {
            _styleTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(450) };
            _styleTimer.Tick += (_, _) =>
            {
                _styleTimer.Stop();
                Restyle();
            };
        }
        _styleTimer.Stop();
        _styleTimer.Start();
    }

    /// <summary>
    /// A LibVLC instance and player with the current subtitle style. VLC's subtitle renderer
    /// takes its options from the instance only (options on a media never reach it), so a
    /// style change needs a new instance; the video view simply gets the new player.
    /// </summary>
    private void CreatePlayer(int? videoHeight = null)
    {
        var args = VlcArgs(videoHeight);
        var key = string.Join(" ", args);
        if (key == _vlcStyle && _player != null)
        {
            VlcLive.Apply(_player);
            return;
        }
        var (oldPlayer, oldVlc) = (_player, _vlc);
        _vlc = new LibVLC(args.ToArray());
        _player = new VlcMediaPlayer(_vlc);
        _vlcStyle = key;
        _vlcHeight = videoHeight;
        VlcLive.Apply(_player);
        HookPlayer();
        Video.MediaPlayer = _player;
        if (VolumeBar != null) _player.Volume = (int)VolumeBar.Value;
        _player.Mute = _muted;
        if (oldPlayer == null) return;
        // Stopping and freeing the old one can take a moment: off the UI thread.
        _ = Task.Run(() =>
        {
            try
            {
                oldPlayer.Stop();
                oldPlayer.Dispose();
                oldVlc.Dispose();
            }
            catch (Exception ex)
            {
                Repository.Log($"old player not freed: {ex.Message}");
            }
        });
    }

    /// <summary>LibVLC instance arguments; the live colours are left out when they can be set on the player instead.</summary>
    private static List<string> VlcArgs(int? videoHeight)
    {
        var args = new List<string> { "--no-video-title-show", "--network-caching=1500" };
        args.AddRange(SubtitleStyle.VlcArgs(videoHeight, withLive: !VlcLive.Supported));
        return args;
    }

    /// <summary>What <see cref="_vlcStyle"/> would be for the current style.</summary>
    private static string VlcStyleKey(int? videoHeight) => string.Join(" ", VlcArgs(videoHeight));

    /// <summary>Before a new stream: make sure the player has the current subtitle style.</summary>
    private void EnsurePlayerStyle() => CreatePlayer(VideoHeight());

    private void Restyle()
    {
        if (Playing is not { } it || !Uri.TryCreate(_player.Media?.Mrl ?? "", UriKind.Absolute, out var uri)) return;
        var time = it.IsLive ? 0 : Math.Max(0, _player.Time) / 1000.0;
        var spu = _player.Spu;
        var audio = _player.AudioTrack;
        var paused = _player.State == VLCState.Paused;
        var files = _subFiles.ToList();
        CreatePlayer(VideoHeight());
        using var media = BuildMedia(it, uri, time);
        foreach (var f in files) media.AddSlave(MediaSlaveType.Subtitle, 4, new Uri(f).AbsoluteUri);
        void Restore(object? sender, EventArgs e)
        {
            _player.Playing -= Restore;
            Dispatcher.BeginInvoke(async () =>
            {
                // Tracks appear a moment after playback starts.
                for (var n = 0; n < 20 && _player.SpuCount == 0 && spu >= 0; n++) await Task.Delay(100);
                if (audio >= 0) _player.SetAudioTrack(audio);
                _player.SetSpu(spu);
                if (paused) _player.SetPause(true);
            });
        }
        _player.Playing += Restore;
        _player.Play(media);
    }

    private void SearchSubtitlesOnline()
    {
        if (Playing is not { } it) return;
        var query = it.Kind == "episode" && _details != null ? $"{_details.Name}" : it.Name;
        int? season = null, episode = null;
        if (it.Kind == "episode" && int.TryParse(it.Season, out var sn) && int.TryParse(it.Episode, out var en))
        {
            season = sn;
            episode = en;
        }
        if (SubtitleSearchWindow.Show(this, CleanTitle(query), season, episode) is { } file) UseSubtitle(file);
    }

    /// <summary>"EN - The Movie (2024) [4K]" → "The Movie".</summary>
    private static string CleanTitle(string name)
    {
        var s = System.Text.RegularExpressions.Regex.Replace(name, @"^\s*[A-Z]{2,4}\s*[-|:]\s+", "");
        s = System.Text.RegularExpressions.Regex.Replace(s, @"[\(\[].*?[\)\]]", "");
        return s.Trim();
    }
}
