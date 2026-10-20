using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using Microsoft.Win32;

namespace Madhyam.Windows;

/// <summary>
/// Settings page: section list on the left, rows of
/// "title + description … control" on the right. Built in code so each row is
/// one line here and adding a setting stays cheap.
/// </summary>
public sealed partial class SettingsView : UserControl
{
    private readonly MainWindow _host;
    private readonly Repository _repo;
    private readonly ListBox _nav = new();
    private readonly StackPanel _content = new() { Margin = new Thickness(40, 8, 40, 40), MaxWidth = 860 };
    private readonly ScrollViewer _scroll;
    private static AppSettings S => AppSettings.Current;

    private static readonly (string Key, string Icon, string Label)[] Sections =
    {
        ("General", "\uE713", "General"),
        ("Sources", "\uE8FD", "Sources"),
        ("Dashboard", "\uE80F", "Dashboard"),
        ("Playback", "\uE768", "Playback"),
        ("Guide", "\uE787", "Programme guide"),
        ("Parental", "\uE72E", "Parental controls"),
        ("Data", "\uE8F1", "Backup & data"),
        ("About", "\uE946", "About"),
    };

    public SettingsView(MainWindow host, Repository repo)
    {
        _host = host;
        _repo = repo;

        var grid = new Grid();
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(230) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var left = new DockPanel { Margin = new Thickness(0, 0, 14, 0) };
        var title = new TextBlock { Text = "Settings", FontSize = 17, FontWeight = FontWeights.SemiBold, Margin = new Thickness(12, 4, 0, 12) };
        DockPanel.SetDock(title, Dock.Top);
        left.Children.Add(title);
        _nav.Style = (Style)Application.Current.FindResource("PlainList");
        _nav.ItemContainerStyle = (Style)Application.Current.FindResource("CategoryItem");
        foreach (var (_, icon, label) in Sections)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal };
            row.Children.Add(new TextBlock { Text = icon, FontFamily = (FontFamily)Application.Current.FindResource("Icons"), FontSize = 14, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 10, 0) });
            row.Children.Add(new TextBlock { Text = label, FontSize = 14, VerticalAlignment = VerticalAlignment.Center });
            _nav.Items.Add(new ListBoxItem { Content = row });
        }
        _nav.SelectionChanged += (_, _) =>
        {
            if (_navSilent) return;
            var key = Sections[Math.Max(0, _nav.SelectedIndex)].Key;
            // Parental controls itself always needs the PIN once it is on.
            if (key == "Parental" && key != _current && !Parental.Ensure("Parental controls are locked."))
            {
                _navSilent = true;
                _nav.SelectedIndex = Array.FindIndex(Sections, x => x.Key == _current);
                _navSilent = false;
                return;
            }
            Show(key);
        };
        left.Children.Add(_nav);
        grid.Children.Add(left);

        var panel = new Border { CornerRadius = new CornerRadius(14), Background = Brush("Panel") };
        _scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = _content };
        ElasticScroll.Attach(_scroll);
        panel.Child = _scroll;
        Grid.SetColumn(panel, 1);
        grid.Children.Add(panel);
        Content = grid;

        _nav.SelectedIndex = 0;
    }

    private string _current = "General";
    private bool _navSilent;

    /// <summary>Rebuild the visible section (after sources changed).</summary>
    public void Refresh()
    {
        // Coming back to Settings on the Parental controls page: it needs the PIN again once locked.
        if (_current == "Parental" && Parental.On && !Parental.Unlocked)
        {
            _nav.SelectedIndex = 0;
            return;
        }
        if (_current is "Sources" or "About") Show(_current);
    }

    /// <summary>Open a section by key (General, Sources, Dashboard, Playback, Guide, Data, About).</summary>
    public void Open(string key)
    {
        var i = Array.FindIndex(Sections, x => string.Equals(x.Key, key, StringComparison.OrdinalIgnoreCase));
        if (i >= 0) _nav.SelectedIndex = i;
    }

    // ================================================================ building blocks

    private static Brush Brush(string key) => (Brush)Application.Current.FindResource(key);

    private void Header(string title, string subtitle)
    {
        _content.Children.Add(new TextBlock { Text = title, FontSize = 24, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 22, 0, 2) });
        _content.Children.Add(new TextBlock { Text = subtitle, Foreground = Brush("Muted"), Margin = new Thickness(0, 0, 0, 10) });
    }

    private void SubHeader(string title) =>
        _content.Children.Add(new TextBlock { Text = title, FontSize = 17, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 26, 0, 4) });

    /// <summary>One setting: title and description on the left, the control on the right (or below if wide).</summary>
    private void Row(string title, string description, UIElement control, bool controlBelow = false)
    {
        var g = new Grid { Margin = new Thickness(0, 0, 0, 0) };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { Margin = new Thickness(0, 0, 24, 0), VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, FontSize = 14.5, FontWeight = FontWeights.SemiBold });
        if (description.Length > 0)
            text.Children.Add(new TextBlock { Text = description, Foreground = Brush("Muted"), FontSize = 12.5, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 3, 0, 0) });
        g.Children.Add(text);
        if (controlBelow)
        {
            var stack = new StackPanel();
            stack.Children.Add(g);
            if (control is FrameworkElement fe) fe.Margin = new Thickness(0, 10, 0, 0);
            stack.Children.Add(control);
            AddRow(stack);
        }
        else
        {
            if (control is FrameworkElement fe) fe.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(control, 1);
            g.Children.Add(control);
            AddRow(g);
        }
    }

    private void AddRow(UIElement e)
    {
        _content.Children.Add(new Border { Height = 1, Background = Brush("Line"), Margin = new Thickness(0, 14, 0, 14) });
        _content.Children.Add(e);
    }

    private static CheckBox Toggle(bool value, Action<bool> changed)
    {
        var c = new CheckBox { IsChecked = value, Style = (Style)Application.Current.FindResource("Switch") };
        c.Click += (_, _) => changed(c.IsChecked == true);
        return c;
    }

    private static int _group;
    private RadioButton? _customChoice; // the theme choice's "Custom" option, checked by Pick colour

    private static Panel Choice(IEnumerable<(string Value, string Label)> options, string current, Action<string> changed)
    {
        var panel = new WrapPanel();
        var name = $"g{_group++}";
        foreach (var (value, label) in options)
        {
            var r = new RadioButton
            {
                Content = label,
                GroupName = name,
                IsChecked = value == current,
                Style = (Style)Application.Current.FindResource("Segment"),
            };
            r.Checked += (_, _) => changed(value);
            panel.Children.Add(r);
        }
        return panel;
    }

    private static Button Btn(string label, Func<Task> onClick, bool primary = false)
    {
        var b = new Button
        {
            Content = label,
            Style = (Style)Application.Current.FindResource(primary ? "PrimaryButton" : "SecondaryButton"),
            Padding = new Thickness(16, 7, 16, 7),
            FontSize = 13.5,
            Margin = new Thickness(0, 0, 8, 0),
        };
        b.Click += async (_, _) =>
        {
            b.IsEnabled = false;
            try { await onClick(); }
            finally { b.IsEnabled = true; }
        };
        return b;
    }

    private static StackPanel Buttons(params Button[] buttons)
    {
        var p = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in buttons) p.Children.Add(b);
        return p;
    }

    private TextBlock Note(string text)
    {
        var t = new TextBlock { Text = text, Foreground = Brush("Accent"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 8, 0, 0) };
        return t;
    }

    private static void Save() => AppSettings.Save();

    // ================================================================ pages

    private void Show(string key)
    {
        _current = key;
        _content.Children.Clear();
        _scroll.ScrollToTop();
        switch (key)
        {
            case "General": General(); break;
            case "Sources": SourcesPage(); break;
            case "Dashboard": DashboardPage(); break;
            case "Playback": Playback(); break;
            case "Guide": Guide(); break;
            case "Parental": ParentalPage(); break;
            case "Data": Data(); break;
            default: About(); break;
        }
    }

    private void General()
    {
        Header("General", $"How {Brand.Name} looks and starts.");

        var themeNote = Note("");
        themeNote.Visibility = Visibility.Collapsed;
        var restart = Btn("Restart now", () => { _host.RestartApp(); return Task.CompletedTask; }, primary: true);
        restart.Visibility = Visibility.Collapsed;
        restart.Margin = new Thickness(0, 10, 0, 0);
        restart.HorizontalAlignment = HorizontalAlignment.Left;
        var themeBox = new StackPanel();
        void ThemeChanged()
        {
            Save();
            themeNote.Text = $"The new theme is used the next time {Brand.Name} starts.";
            themeNote.Visibility = Visibility.Visible;
            restart.Visibility = Visibility.Visible;
        }
        var customSwatch = new Ellipse { Width = 14, Height = 14, Margin = new Thickness(0, 0, 8, 0), VerticalAlignment = VerticalAlignment.Center };
        var customName = new TextBlock { Foreground = Brush("Muted"), Margin = new Thickness(0, 0, 12, 0), VerticalAlignment = VerticalAlignment.Center };
        void ShowCustom()
        {
            customSwatch.Fill = new SolidColorBrush(ThemeColors.Parse(S.CustomColor) ?? Colors.Crimson);
            customName.Text = $"{ThemeColors.NameOf(S.CustomColor)} · {(S.CustomLight ? "light" : "dark")}";
        }
        ShowCustom();
        var pick = Btn("Pick colour…", () =>
        {
            if (ColorPickerWindow.Show(Window.GetWindow(this)!, S.CustomColor, S.CustomLight) is { } chosen)
            {
                (S.CustomColor, S.CustomLight) = chosen;
                ShowCustom();
                if (S.Theme != "Custom") _customChoice!.IsChecked = true; // → Checked → ThemeChanged
                else ThemeChanged();
            }
            return Task.CompletedTask;
        });
        var themeChoice = Choice(new[] { ("Dark", "☾  Navy & Crimson"), ("Light", "☀  Light"), ("System", "🖥  System"), ("Custom", "🎨  Custom") }, S.Theme, v =>
        {
            S.Theme = v;
            ThemeChanged();
        });
        _customChoice = themeChoice.Children.OfType<RadioButton>().Last();
        themeBox.Children.Add(themeChoice);
        themeBox.Children.Add(new StackPanel
        {
            Orientation = Orientation.Horizontal, Margin = new Thickness(0, 10, 0, 0),
            Children = { customSwatch, customName, pick },
        });
        themeBox.Children.Add(themeNote);
        themeBox.Children.Add(restart);
        Row("Visual theme", "Navy & Crimson is the dark theme: deep navy with crimson highlights. Custom builds a theme from any colour you pick.", themeBox, controlBelow: true);

        Row("Cover size", "Size of posters in the movie and series grids. Larger is easier to see from a distance; smaller shows more per row.",
            Choice(new[] { ("Small", "Small"), ("Medium", "Medium"), ("Large", "Large") }, S.CoverSize, v =>
            {
                S.CoverSize = v;
                Save();
                AppSettings.ApplyLayout(Application.Current.Resources);
            }), controlBelow: true);

        Row("Show titles under covers", "Turn off for a posters-only wall; the title still shows when you point at a poster.",
            Toggle(S.TitlesUnderCovers, v =>
            {
                S.TitlesUnderCovers = v;
                Save();
                AppSettings.ApplyLayout(Application.Current.Resources);
            }));

        Row("Startup view", $"The section {Brand.Name} opens on.",
            Choice(new[] { ("Dashboard", "Dashboard"), ("Live", "Live TV"), ("Movies", "Movies"), ("Series", "Series"), ("Favorites", "Favorites"), ("Last", "Where I left off") },
                S.StartupView, v => { S.StartupView = v; Save(); }), controlBelow: true);

        Row("Window on startup", "Open at the last size and position, maximized, or full screen. F11 toggles full screen at any time.",
            Choice(new[] { ("Last", "Last size"), ("Maximized", "Maximized"), ("Fullscreen", "Full screen") },
                S.WindowStartup, v => { S.WindowStartup = v; Save(); }), controlBelow: true);

        Row("Strip country prefixes from channel names", "Shows “CBC Calgary HD” instead of “CA EN: CBC Calgary HD”. Search still finds both.",
            Toggle(S.StripPrefixes, v => { S.StripPrefixes = v; Save(); _host.RefreshListAppearance(); }));

        Row("Show channel numbers", "Number channels in the order the provider lists them.",
            Toggle(S.ShowNumbers, v => { S.ShowNumbers = v; Save(); _host.RefreshListAppearance(); }));
    }

    // ---------------------------------------------------------------- sources

    private string _srcFilter = "all";
    private string _srcSort = "added";

    private void SourcesPage()
    {
        Header("Sources", "Your playlists and Xtream accounts. Switch, test, refresh, edit or remove them here.");

        var tools = new DockPanel { Margin = new Thickness(0, 6, 0, 4) };
        var add = Btn("Add source", () => { _host.OpenAddSource(); return Task.CompletedTask; }, primary: true);
        add.Margin = new Thickness(0);
        DockPanel.SetDock(add, Dock.Right);
        tools.Children.Add(add);
        var filters = new WrapPanel();
        filters.Children.Add(Choice(new[] { ("all", "All"), ("m3u", "M3U"), ("xtream", "Xtream") }, _srcFilter, v => { _srcFilter = v; Show("Sources"); }));
        var sort = Choice(new[] { ("added", "Newest first"), ("name", "Name A–Z") }, _srcSort, v => { _srcSort = v; Show("Sources"); });
        sort.Margin = new Thickness(18, 0, 0, 0);
        filters.Children.Add(sort);
        tools.Children.Add(filters);
        _content.Children.Add(tools);

        AddRow(FreeTvCard());

        var list = _host.SourcesList.Where(x => _srcFilter == "all" || x.Kind == _srcFilter);
        list = _srcSort == "name"
            ? list.OrderBy(x => x.Name, StringComparer.CurrentCultureIgnoreCase)
            : list.OrderByDescending(x => S.SourceAdded.TryGetValue(x.Id, out var t) ? t : x.Id);
        var shown = list.ToList();
        if (shown.Count == 0)
        {
            AddRow(new TextBlock
            {
                Text = _host.SourcesList.Count == 0 ? "No sources yet. Use “Add source” to add an Xtream account or an M3U playlist." : "No sources of this type.",
                Foreground = Brush("Muted"),
                TextWrapping = TextWrapping.Wrap,
            });
            return;
        }
        foreach (var src in shown) AddRow(SourceRow(src));
    }

    /// <summary>Free TV (iptv-org): on/off and re-download. Categories / countries are chosen in Live TV.</summary>
    private FrameworkElement FreeTvCard()
    {
        var on = S.FreeTvSourceId != null && _host.SourcesList.Any(x => x.Id == S.FreeTvSourceId);
        var box = new StackPanel();
        var head = new Grid();
        var text = new StackPanel { Margin = new Thickness(0, 0, 24, 0) };
        text.Children.Add(new TextBlock { Text = "Free TV channels (iptv-org)", FontSize = 14.5, FontWeight = FontWeights.SemiBold });
        text.Children.Add(new TextBlock
        {
            Text = "Thousands of public channels collected by the iptv-org project, added as a playlist called “Free TV”. " +
                   "Many streams are offline or only work in their own country; turn it off to remove them again.",
            Foreground = Brush("Muted"), FontSize = 12.5, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 3, 0, 0),
        });
        head.Children.Add(text);
        var status = Note("");
        status.Visibility = Visibility.Collapsed;
        var options = new StackPanel { Margin = new Thickness(0, 12, 0, 0), Visibility = on ? Visibility.Visible : Visibility.Collapsed };
        var toggle = Toggle(on, async v =>
        {
            options.Visibility = v ? Visibility.Visible : Visibility.Collapsed;
            status.Visibility = Visibility.Visible;
            status.Text = v ? "Downloading the channel list…" : "Removing Free TV…";
            await _host.ApplyFreeTv(v);
            status.Text = v ? "Free TV is on. Pick it from the playlist menu at the top." : "Free TV removed.";
        });
        toggle.HorizontalAlignment = HorizontalAlignment.Right;
        toggle.VerticalAlignment = VerticalAlignment.Top;
        Grid.SetColumn(toggle, 1);
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.Children.Add(toggle);
        box.Children.Add(head);

        options.Children.Add(Note("One playlist with every channel. In Live TV, switch the left panel between Categories and Countries."));
        var apply = Btn("Update channel list", async () =>
        {
            status.Visibility = Visibility.Visible;
            status.Text = "Downloading the channel list…";
            await _host.ApplyFreeTv(true);
            status.Text = "Free TV updated.";
        }, primary: true);
        apply.Margin = new Thickness(0, 12, 0, 0);
        apply.HorizontalAlignment = HorizontalAlignment.Left;
        options.Children.Add(apply);
        box.Children.Add(options);
        box.Children.Add(status);
        return box;
    }

    private static string When(long unix) => DateTimeOffset.FromUnixTimeSeconds(unix).LocalDateTime.ToString("d MMM yyyy, HH:mm");

    private FrameworkElement SourceRow(Source src)
    {
        var active = _host.CurrentSource?.Id == src.Id;
        var outer = new StackPanel();
        var g = new Grid();
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.Children.Add(new Border
        {
            Width = 42,
            Height = 42,
            CornerRadius = new CornerRadius(11),
            Background = Brush(active ? "RailActive" : "Chip"),
            VerticalAlignment = VerticalAlignment.Top,
            Child = new TextBlock
            {
                Text = src.Kind == "xtream" ? "\uE774" : "\uE8FD",
                FontFamily = (FontFamily)Application.Current.FindResource("Icons"),
                FontSize = 16,
                Foreground = Brush(active ? "Accent" : "Fg"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        });
        var text = new StackPanel { Margin = new Thickness(14, 0, 12, 0), VerticalAlignment = VerticalAlignment.Center };
        var nameRow = new StackPanel { Orientation = Orientation.Horizontal };
        nameRow.Children.Add(new TextBlock { Text = src.Name, FontSize = 15, FontWeight = FontWeights.SemiBold });
        if (active)
            nameRow.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(6),
                Background = Brush("RailActive"),
                Padding = new Thickness(7, 1, 7, 1),
                Margin = new Thickness(10, 0, 0, 0),
                VerticalAlignment = VerticalAlignment.Center,
                Child = new TextBlock { Text = "IN USE", FontSize = 10, FontWeight = FontWeights.Bold, Foreground = Brush("Accent") },
            });
        text.Children.Add(nameRow);
        var meta = new List<string> { src.KindLabel };
        if (S.SourceAdded.TryGetValue(src.Id, out var added)) meta.Add($"Added {When(added)}");
        if (src.UpdatedAt > 0) meta.Add($"Updated {When(src.UpdatedAt)}");
        text.Children.Add(new TextBlock { Text = string.Join("  |  ", meta), Foreground = Brush("Muted"), FontSize = 12.5, Margin = new Thickness(0, 3, 0, 0), TextWrapping = TextWrapping.Wrap });
        text.Children.Add(new TextBlock
        {
            Text = src.Kind == "xtream" ? $"{src.Location}  ·  user {src.Username}" : src.Location,
            Foreground = Brush("Muted"),
            FontSize = 12,
            Margin = new Thickness(0, 2, 0, 0),
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        Grid.SetColumn(text, 1);
        g.Children.Add(text);

        var status = new TextBlock { Foreground = Brush("Accent"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(56, 8, 0, 0), Visibility = Visibility.Collapsed };
        void Say(string msg, bool bad = false)
        {
            status.Text = msg;
            status.Foreground = Brush(bad ? "LiveRed" : "Accent");
            status.Visibility = Visibility.Visible;
        }

        Button Icon(string glyph, string tip, Func<Task> run)
        {
            var b = new Button { Content = glyph, Style = (Style)Application.Current.FindResource("IconButton"), ToolTip = tip, Width = 36, Height = 36, FontSize = 14 };
            b.Click += async (_, _) =>
            {
                b.IsEnabled = false;
                try { await run(); }
                catch (Exception e) { Say(MainWindow.Explain(e), bad: true); }
                finally { b.IsEnabled = true; }
            };
            return b;
        }
        var actions = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        if (!active)
            actions.Children.Add(Icon("\uE8AB", "Use this playlist", async () => { await _host.SwitchSource(src); Show("Sources"); }));
        actions.Children.Add(Icon("\uE703", "Test connection", async () =>
        {
            Say("Testing…");
            Say(await _repo.TestConnection(src.Id));
        }));
        actions.Children.Add(Icon("\uE72C", "Refresh (download again)", async () =>
        {
            Say("Refreshing…");
            await _host.RefreshSource(src);
            Show("Sources");
        }));
        actions.Children.Add(Icon("\uE70F", "Edit", async () =>
        {
            if (!Parental.EnsureSettings("Editing a playlist is locked.")) return;
            var ua = S.SourceUserAgent.TryGetValue(src.Id, out var u) ? u : "";
            var pw = src.Kind == "xtream" ? await _repo.SourcePassword(src.Id) : "";
            var r = EditSourceWindow.Show(Window.GetWindow(this)!, src, ua, pw);
            if (r == null) return;
            if (r.UserAgent.Length > 0) S.SourceUserAgent[src.Id] = r.UserAgent;
            else S.SourceUserAgent.Remove(src.Id);
            Save();
            Say("Saving and downloading again…");
            await _repo.UpdateSource(src.Id, r.Name, r.Location, r.Server, r.Username, r.Password);
            await _host.ReloadSources(_host.CurrentSource?.Id);
            Show("Sources");
        }));
        actions.Children.Add(Icon("\uE946", "Playlist info", () => _host.ShowPlaylistInfo(src)));
        if (src.Kind == "xtream")
            actions.Children.Add(Icon("\uE77B", "Account info", () => _host.ShowAccountInfo(src)));
        actions.Children.Add(Icon("\uE74D", "Remove", async () =>
        {
            await _host.RemoveSource(src);
            Show("Sources");
        }));
        Grid.SetColumn(actions, 2);
        g.Children.Add(actions);
        outer.Children.Add(g);
        outer.Children.Add(status);
        return outer;
    }

    // ---------------------------------------------------------------- dashboard

    private void DashboardPage()
    {
        Header("Dashboard", "Choose what the start page shows.");
        void Toggle(string title, string desc, bool value, Action<bool> set) =>
            Row(title, desc, SettingsView.Toggle(value, v => { set(v); Save(); }));
        Toggle("Banner", "A large banner with the film or episode you were watching, or shortcuts when there is none.", S.DashHero, v => S.DashHero = v);
        Toggle("Continue watching", "Films and episodes you started but did not finish.", S.DashContinue, v => S.DashContinue = v);
        Toggle("Favorite channels", "Your starred live channels with what is on now.", S.DashLiveFavorites, v => S.DashLiveFavorites = v);
        Toggle("Recently watched channels", "The live channels you opened last.", S.DashRecentLive, v => S.DashRecentLive = v);
        Toggle("Favorite movies and series", "", S.DashFavoriteVod, v => S.DashFavoriteVod = v);
        Toggle("Recently added", "The newest movies and series on your Xtream server.", S.DashRecentlyAdded, v => S.DashRecentlyAdded = v);
    }

    private void Playback()
    {
        Header("Playback", "How streams are opened and played (VLC engine).");

        Row("Live stream format", "Auto uses what the provider gives. If live channels stutter or fail, try the other format. Applies to Xtream channels.",
            Choice(new[] { ("Auto", "Auto"), ("TS", "MPEG-TS"), ("HLS", "HLS (m3u8)") }, S.LiveFormat, v => { S.LiveFormat = v; Save(); }),
            controlBelow: true);

        Row("Network buffer", "More buffer means fewer freezes on a slow connection but a longer wait when switching channels.",
            Choice(new[] { ("800", "0.8 s"), ("1500", "1.5 s"), ("3000", "3 s"), ("6000", "6 s") }, S.NetworkCachingMs.ToString(),
                v => { S.NetworkCachingMs = int.Parse(v); Save(); }), controlBelow: true);

        Row("Hardware decoding", "Uses the graphics card to decode video (lower CPU use, smoother 4K). Turn off if you see green or black frames.",
            Toggle(S.HardwareDecoding, v => { S.HardwareDecoding = v; Save(); }));

        Row("Resume movies and episodes", "Continue where you stopped last time.",
            Toggle(S.ResumePlayback, v => { S.ResumePlayback = v; Save(); }));

        Row("Show subtitles by default", "When a film has subtitles, turn them on automatically.",
            Toggle(S.ShowSubtitles, v => { S.ShowSubtitles = v; Save(); }));

        Row("Preferred audio language", "Chosen automatically when a stream has several audio tracks.",
            LanguageChoice(S.AudioLanguage, v => { S.AudioLanguage = v; Save(); }), controlBelow: true);

        Row("Preferred subtitle language", "",
            LanguageChoice(S.SubtitleLanguage, v => { S.SubtitleLanguage = v; Save(); }), controlBelow: true);

        // Online subtitles (VLsub-like search)
        var os = new StackPanel();
        Grid Pair(string label, Control box)
        {
            var g = new Grid { Margin = new Thickness(0, 0, 0, 8) };
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(150) });
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            g.Children.Add(new TextBlock { Text = label, Foreground = Brush("Muted"), VerticalAlignment = VerticalAlignment.Center });
            Grid.SetColumn(box, 1);
            g.Children.Add(box);
            return g;
        }
        var key = new TextBox { Text = S.OsApiKey, MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Left, MinWidth = 320 };
        key.LostFocus += (_, _) => { S.OsApiKey = key.Text.Trim(); Save(); };
        var user = new TextBox { Text = S.OsUsername, MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Left, MinWidth = 320 };
        user.LostFocus += (_, _) => { S.OsUsername = user.Text.Trim(); Save(); };
        var pass = new PasswordBox { MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Left, MinWidth = 320, ToolTip = S.OsPassword.Length > 0 ? "Saved (encrypted). Type to replace." : "" };
        pass.LostFocus += (_, _) => { if (pass.Password.Length > 0) { S.OsPassword = Secret.Protect(pass.Password); Save(); } };
        var langs = new TextBox { Text = S.OsLanguages, MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Left, MinWidth = 320 };
        langs.LostFocus += (_, _) => { S.OsLanguages = langs.Text.Trim(); Save(); };
        os.Children.Add(Pair("API key", key));
        os.Children.Add(Pair("Username (optional)", user));
        os.Children.Add(Pair("Password (optional)", pass));
        os.Children.Add(Pair("Languages", langs));
        os.Children.Add(Buttons(
            Btn("Get a free API key", () =>
            {
                Process.Start(new ProcessStartInfo("https://www.opensubtitles.com/en/consumers") { UseShellExecute = true });
                return Task.CompletedTask;
            }),
            Btn("Forget login", () =>
            {
                S.OsUsername = "";
                S.OsPassword = "";
                user.Text = "";
                pass.Password = "";
                Save();
                return Task.CompletedTask;
            })));
        Row("Online subtitles (OpenSubtitles.com)",
            "Search and download subtitles from the player's CC menu, like VLC's VLsub. Needs a free API key; logging in gives more downloads per day. Languages: comma-separated codes such as en, fr, es.",
            os, controlBelow: true);

        var omdb = new TextBox { Text = S.OmdbApiKey, MinWidth = 320, MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Left };
        omdb.LostFocus += (_, _) => { S.OmdbApiKey = omdb.Text.Trim(); Save(); };
        var omdbBox = new StackPanel();
        omdbBox.Children.Add(omdb);
        omdbBox.Children.Add(Buttons(Btn("Get a free OMDb key", () =>
        {
            Process.Start(new ProcessStartInfo("https://www.omdbapi.com/apikey.aspx") { UseShellExecute = true });
            return Task.CompletedTask;
        })));
        ((FrameworkElement)omdbBox.Children[1]).Margin = new Thickness(0, 8, 0, 0);
        Row("IMDb and Rotten Tomatoes ratings",
            "Shown on movie and series pages when found. Uses OMDb (free key, 1,000 look-ups a day). Leave empty to hide them.",
            omdbBox, controlBelow: true);

        SubHeader("Subtitle style");
        _content.Children.Add(new TextBlock
        {
            Text = "Like YouTube's caption options. Also in the player's subtitle menu, where changes show on the film itself. " +
                   "Styled subtitles (ASS/SSA) keep their own look.",
            Foreground = Brush("Muted"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 12),
        });
        var preview = SubtitleStyle.Preview();
        _content.Children.Add(preview);
        var editor = SubtitleStyle.Editor(() => SubtitleStyle.UpdatePreview(preview));
        editor.Margin = new Thickness(0, 18, 0, 0);
        _content.Children.Add(editor);
        _content.Children.Add(new Border { Height = 1, Background = Brush("Line"), Margin = new Thickness(0, 4, 0, 0) });

        Row("Keyboard", "Space or click the picture: pause · ←/→ 10 s · ↑/↓ volume · M mute · J/K audio delay · G/H subtitle delay · double-click or F11 full screen",
            new TextBlock());

        _content.Children.Add(Note("Playback changes apply to the next channel or film you open."));
    }

    private static Panel LanguageChoice(string current, Action<string> changed) =>
        Choice(new[] { ("", "Stream default"), ("eng", "English"), ("fre", "French"), ("spa", "Spanish"), ("ger", "German"),
                       ("ara", "Arabic"), ("hin", "Hindi"), ("por", "Portuguese"), ("tur", "Turkish") }, current, changed);

    private void Guide()
    {
        Header("Programme guide", "The EPG shown next to live channels.");

        Row("Guide view", "Timeline is a horizontal ribbon under the video (like a TV guide); List is a vertical list of today's programmes.",
            Choice(new[] { ("Timeline", "Timeline"), ("List", "List") }, S.GuideView, v => { S.GuideView = v; Save(); _host.RefreshGuideView(); }),
            controlBelow: true);

        var offsetLabel = new TextBlock { Width = 70, TextAlignment = TextAlignment.Center, VerticalAlignment = VerticalAlignment.Center, FontSize = 14 };
        void ShowOffset() => offsetLabel.Text = S.EpgOffsetMinutes == 0 ? "0 min" : $"{S.EpgOffsetMinutes:+0;-0} min";
        ShowOffset();
        Button Step(string label, int delta)
        {
            var b = new Button { Content = label, Style = (Style)Application.Current.FindResource("ChipButton"), MinWidth = 44 };
            b.Click += (_, _) =>
            {
                S.EpgOffsetMinutes = Math.Clamp(S.EpgOffsetMinutes + delta, -720, 720);
                ShowOffset();
                Save();
                _host.RefreshListAppearance();
            };
            return b;
        }
        var offset = new StackPanel { Orientation = Orientation.Horizontal };
        offset.Children.Add(Step("−1 h", -60));
        offset.Children.Add(Step("−15", -15));
        offset.Children.Add(offsetLabel);
        offset.Children.Add(Step("+15", 15));
        offset.Children.Add(Step("+1 h", 60));
        Row("Time offset", "Shift programme times if your provider's guide is consistently early or late.", offset, controlBelow: true);

        Row("Refresh guides every", "Guides are downloaded in the background when they are older than this.",
            Choice(new[] { ("6", "6 hours"), ("12", "12 hours"), ("24", "24 hours"), ("72", "3 days") }, S.EpgRefreshHours.ToString(),
                v => { S.EpgRefreshHours = int.Parse(v); Save(); }), controlBelow: true);

        // Extra XMLTV sources
        var list = new StackPanel();
        var status = Note("");
        status.Visibility = Visibility.Collapsed;
        void Render()
        {
            list.Children.Clear();
            if (S.ExtraEpgUrls.Count == 0)
                list.Children.Add(new TextBlock { Text = "No extra guide sources. Your playlists' own guides are always used.", Foreground = Brush("Muted"), Margin = new Thickness(0, 0, 0, 8) });
            foreach (var url in S.ExtraEpgUrls.ToList())
            {
                var row = new DockPanel { Margin = new Thickness(0, 0, 0, 6) };
                var remove = new Button { Content = "\uE74D", Style = (Style)Application.Current.FindResource("IconButton"), ToolTip = "Remove", Width = 32, Height = 32, FontSize = 13 };
                remove.Click += (_, _) => { S.ExtraEpgUrls.Remove(url); Save(); Render(); };
                DockPanel.SetDock(remove, Dock.Right);
                row.Children.Add(remove);
                row.Children.Add(new Border
                {
                    CornerRadius = new CornerRadius(8), Background = Brush("Card"), Padding = new Thickness(10, 7, 10, 7),
                    Child = new TextBlock { Text = url, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 13 },
                });
                list.Children.Add(row);
            }
        }
        Render();
        var box = new TextBox { MinWidth = 420, ToolTip = "https://example.org/guide.xml.gz  or  C:\\epg\\guide.xml" };
        var add = Btn("Add", () =>
        {
            var u = box.Text.Trim();
            if (u.Length > 0 && !S.ExtraEpgUrls.Contains(u))
            {
                S.ExtraEpgUrls.Add(u);
                Save();
                box.Text = "";
                Render();
            }
            return Task.CompletedTask;
        });
        add.Margin = new Thickness(8, 0, 0, 0);
        var addRow = new DockPanel { Margin = new Thickness(0, 4, 0, 0) };
        DockPanel.SetDock(add, Dock.Right);
        addRow.Children.Add(add);
        addRow.Children.Add(box);
        var sources = new StackPanel();
        sources.Children.Add(list);
        sources.Children.Add(addRow);
        Row("Extra guide sources (XMLTV)", "Add XMLTV links (http/https, .xml or .xml.gz) or files on this computer. Useful when your provider's guide is missing channels.",
            sources, controlBelow: true);

        var actions = Buttons(
            Btn("Refresh all guides now", async () =>
            {
                status.Visibility = Visibility.Visible;
                status.Text = "Downloading guides…";
                await _host.RefreshAllGuides();
                status.Text = "Guides updated.";
            }, primary: true),
            Btn("Clear guide data", async () =>
            {
                await _repo.ClearEpg();
                _host.RefreshListAppearance();
                status.Visibility = Visibility.Visible;
                status.Text = "Guide data cleared. Use “Refresh all guides now” to download it again.";
            }));
        var actionBox = new StackPanel();
        actionBox.Children.Add(actions);
        actionBox.Children.Add(status);
        AddRow(actionBox);
    }

    private void Data()
    {
        Header("Backup & data", "Your favorites, watch progress and history.");

        var status = Note("");
        status.Visibility = Visibility.Collapsed;

        Row("Backup", "Save favorites, resume positions and recently watched to a file, or restore them on another PC. Playlists and passwords are not included.",
            Buttons(
                Btn("Export…", async () =>
                {
                    var dlg = new SaveFileDialog { FileName = $"{Brand.Name}-backup-{DateTime.Now:yyyy-MM-dd}.json", Filter = $"{Brand.Name} backup (*.json)|*.json" };
                    if (dlg.ShowDialog(Window.GetWindow(this)) != true) return;
                    await File.WriteAllTextAsync(dlg.FileName, await _repo.ExportUserState());
                    status.Visibility = Visibility.Visible;
                    status.Text = $"Saved to {dlg.FileName}";
                }),
                Btn("Import…", async () =>
                {
                    var dlg = new OpenFileDialog { Filter = $"{Brand.Name} backup (*.json)|*.json|All files|*.*" };
                    if (dlg.ShowDialog(Window.GetWindow(this)) != true) return;
                    try
                    {
                        await _repo.ImportUserState(await File.ReadAllBytesAsync(dlg.FileName));
                        await _host.ReloadFavorites();
                        status.Text = "Backup restored (merged with what you already had).";
                    }
                    catch (Exception e)
                    {
                        status.Text = $"Could not import: {e.Message}";
                    }
                    status.Visibility = Visibility.Visible;
                })), controlBelow: true);

        Row("Data folder", $"Where {Brand.Name} keeps its catalog, guides, settings and logs.",
            Buttons(Btn("Open folder", () =>
            {
                Process.Start(new ProcessStartInfo("explorer.exe", $"\"{App.DataDir}\"") { UseShellExecute = true });
                return Task.CompletedTask;
            })), controlBelow: true);

        Row($"Reset {Brand.Name}", "Remove all playlists, favorites, progress, guides and settings, and start fresh.",
            Buttons(Btn("Reset everything…", () =>
            {
                if (MessageBox.Show(Window.GetWindow(this),
                        "This removes all playlists, favorites, watch progress, guides and settings from this PC.\n\nContinue?",
                        $"Reset {Brand.Name}", MessageBoxButton.YesNo, MessageBoxImage.Warning) != MessageBoxResult.Yes)
                    return Task.CompletedTask;
                _host.ResetOnExit = true;
                _host.RestartApp();
                return Task.CompletedTask;
            })), controlBelow: true);

        _content.Children.Add(status);
    }

    private void About()
    {
        Header("About", $"{Brand.Name} ({Brand.NativeName}) — a player for your own IPTV subscription.");
        var v = typeof(App).Assembly.GetName().Version?.ToString(3) ?? "?";
        Row("Version", $"{Brand.Name} {v} · engine {_repo.CoreVersion} (Rust) · video by VLC", new TextBlock());
        Row("Source code and releases", "github.com/diwaskhadka/Madhyam",
            Buttons(Btn("Open on GitHub", () =>
            {
                Process.Start(new ProcessStartInfo("https://github.com/diwaskhadka/Madhyam/releases") { UseShellExecute = true });
                return Task.CompletedTask;
            })));
        Row("Buy me a coffee", $"{Brand.Name} is free. If you enjoy it, you can buy me a coffee.", CoffeeButton());
        Row("Content", $"{Brand.Name} ships with no channels. It plays the Xtream Codes or M3U playlists you add from a provider you are entitled to use.", new TextBlock());
        Row("Keyboard", "Ctrl+K search · F11 full screen · Space pause · PgUp/PgDn change channel · ←/→ seek 10 s · ↑/↓ volume · J/K audio delay · G/H subtitle delay · Esc back", new TextBlock());
    }
}
