using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace Madhyam.Windows;

/// <summary>Settings › Parental controls, and the Buy me a coffee button under About.</summary>
public sealed partial class SettingsView
{
    private int? _lockSource;          // playlist shown in "Locked categories"
    private string _lockKind = "live";
    private string _lockFilter = "";

    private void ParentalPage()
    {
        Header("Parental controls", "Keep channels, films and settings behind a 4-digit PIN.");
        var owner = Window.GetWindow(this)!;

        CheckBox? master = null;
        master = Toggle(S.ParentalEnabled && Parental.HasPin, on =>
        {
            if (on)
            {
                if (!Parental.HasPin && !Parental.AskNewPin(owner))
                {
                    master!.IsChecked = false;
                    return;
                }
                S.ParentalEnabled = true;
                S.ParentalAutoAdult = S.ParentalAutoAdult || S.LockedCategories.Count == 0; // on by default
                Save();
                Parental.MarkUnlocked(); // the PIN was just entered
            }
            else
            {
                // Turning off always asks for the PIN, even while unlocked.
                if (PinDialog.Ask(owner, "Enter your PIN", "To turn parental controls off.", Parental.Verify, offerReset: true) == null)
                {
                    master!.IsChecked = true;
                    return;
                }
                Parental.Lock();
                S.ParentalEnabled = false;
                Save();
            }
            _host.ParentalSettingsChanged();
            Show("Parental");
        });
        Row("Parental controls", "When on, locked categories, adult content and films above the age limit need the PIN. " +
            "After you enter it, things stay unlocked until you close the app or for 10 minutes without a PIN-protected action.", master);

        if (!Parental.On)
        {
            _content.Children.Add(Note(Parental.HasPin
                ? "Parental controls are off. Your PIN is kept for when you turn them on again."
                : "Turning parental controls on asks you to choose a PIN."));
            return;
        }

        Row("PIN", "Change the 4-digit PIN. You are asked for the current one first.",
            Buttons(Btn("Change PIN…", () =>
            {
                if (Parental.ChangePin(owner)) Parental.MarkUnlocked();
                Show("Parental");
                return Task.CompletedTask;
            })));

        Row("Auto-lock adult content", "Categories and channels named for example XXX, Adult, 18+, Porn, Erotic, Playboy, Hustler, Brazzers, Redlight or Private " +
            "are locked by themselves. You can still unlock one such category in the list below.",
            Toggle(S.ParentalAutoAdult, v => { S.ParentalAutoAdult = v; Save(); _host.ParentalSettingsChanged(); Show("Parental"); }));

        Row("Lock Settings", "Settings, adding, editing or removing a playlist, and this page need the PIN.",
            Toggle(S.ParentalLockSettings, v => { S.ParentalLockSettings = v; Save(); }));

        Row("Rating limit", "Allow films and series up to this age. Uses the provider's age rating (G and PG count as 7, PG-13 as 12, R as 16, NC-17 as 18). " +
            "Titles without an age rating are allowed.",
            Choice(new[] { ("0", "Any"), ("7", "7"), ("12", "12"), ("16", "16"), ("18", "18") }, S.ParentalMaxAge.ToString(),
                v => { S.ParentalMaxAge = int.Parse(v); Save(); }), controlBelow: true);

        LockedCategories();

        Row("Reset parental controls", "Forgot the PIN? This removes the PIN and every lock after you type RESET.",
            Buttons(Btn("Reset…", () =>
            {
                if (Parental.ConfirmReset(owner))
                {
                    _host.ParentalSettingsChanged();
                    Show("Parental");
                }
                return Task.CompletedTask;
            })), controlBelow: true);
    }

    /// <summary>Every category of a playlist and section, each with a lock switch.</summary>
    private void LockedCategories()
    {
        var sources = _host.SourcesList;
        if (sources.Count == 0) return;
        if (_lockSource == null || sources.All(s => s.Id != _lockSource)) _lockSource = _host.CurrentSource?.Id ?? sources[0].Id;

        var box = new StackPanel();
        if (sources.Count > 1)
        {
            var srcChoice = Choice(sources.Select(s => (s.Id.ToString(), s.Name)), _lockSource.ToString()!, v =>
            {
                _lockSource = int.Parse(v);
                Show("Parental");
            });
            srcChoice.Margin = new Thickness(0, 0, 0, 4);
            box.Children.Add(srcChoice);
        }
        box.Children.Add(Choice(new[] { ("live", "Live TV"), ("movie", "Movies"), ("series", "Series") }, _lockKind, v =>
        {
            _lockKind = v;
            Show("Parental");
        }));
        var filter = new TextBox { Text = _lockFilter, MaxWidth = 420, MinWidth = 320, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 8, 0, 8), ToolTip = "Search categories" };
        box.Children.Add(filter);
        var list = new StackPanel();
        var count = new TextBlock { Foreground = Brush("Muted"), FontSize = 12.5, Margin = new Thickness(0, 0, 0, 6) };
        box.Children.Add(count);
        box.Children.Add(new Border
        {
            CornerRadius = new CornerRadius(10), Background = Brush("Card"), Padding = new Thickness(12, 4, 12, 4), MaxHeight = 420,
            Child = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = list },
        });
        Row("Locked categories", "A locked category keeps its name, with a 🔒. Opening it, or playing anything in it from Search, Home, Favorites " +
            "or Recent, needs the PIN, and its items are left out of search results, Home rows and “All items” while locked.", box, controlBelow: true);

        var source = _lockSource.Value;
        var kind = _lockKind;
        List<Group> groups = new();
        void Render()
        {
            list.Children.Clear();
            var f = filter.Text.Trim();
            var shown = groups.Where(g => f.Length == 0 || g.Name.Contains(f, StringComparison.CurrentCultureIgnoreCase)).ToList();
            var locked = groups.Count(g => Parental.CategoryState(source, kind, g.Name).Locked);
            count.Text = $"{locked:N0} of {groups.Count:N0} locked";
            foreach (var g in shown.Take(1500))
            {
                var (isLocked, adult) = Parental.CategoryState(source, kind, g.Name);
                var row = new Grid { Margin = new Thickness(0, 6, 0, 6) };
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(24) });
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                if (isLocked)
                    row.Children.Add(new TextBlock { Text = "", FontFamily = (FontFamily)Application.Current.FindResource("Icons"), FontSize = 12, Foreground = Brush("Muted"), VerticalAlignment = VerticalAlignment.Center, ToolTip = adult ? "Adult" : "Locked" });
                var name = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
                name.Children.Add(new TextBlock { Text = $"{g.Name}   ({g.Count:N0})", FontSize = 13.5, TextTrimming = TextTrimming.CharacterEllipsis });
                if (adult) name.Children.Add(new TextBlock { Text = "Adult", FontSize = 11.5, Foreground = Brush("Muted") });
                else if (!isLocked && S.ParentalAutoAdult && Parental.IsAdultName(g.Name))
                    name.Children.Add(new TextBlock { Text = "Adult · unlocked by you", FontSize = 11.5, Foreground = Brush("Muted") });
                Grid.SetColumn(name, 1);
                row.Children.Add(name);
                var name0 = g.Name;
                var sw = Toggle(isLocked, on =>
                {
                    Parental.SetCategoryLocked(source, kind, name0, on);
                    _host.ParentalSettingsChanged();
                    Render();
                });
                sw.ToolTip = "Lock";
                Grid.SetColumn(sw, 2);
                row.Children.Add(sw);
                list.Children.Add(row);
            }
            if (shown.Count == 0) list.Children.Add(new TextBlock { Text = groups.Count == 0 ? "No categories here." : "No categories match.", Foreground = Brush("Muted"), Margin = new Thickness(0, 6, 0, 6) });
        }
        filter.TextChanged += (_, _) =>
        {
            _lockFilter = filter.Text;
            Render();
        };
        count.Text = "Loading…";
        _ = Load();
        async Task Load()
        {
            groups = await _repo.Groups(source, kind);
            if (_current == "Parental") Render();
        }
    }

    /// <summary>About › Buy me a coffee: opens the page in the browser.</summary>
    private static Button CoffeeButton()
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal };
        row.Children.Add(new TextBlock { Text = "☕", FontFamily = new FontFamily("Segoe UI Emoji"), FontSize = 15, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 0) });
        row.Children.Add(new TextBlock { Text = "Buy me a coffee", VerticalAlignment = VerticalAlignment.Center });
        var b = new Button
        {
            Content = row,
            Style = (Style)Application.Current.FindResource("SecondaryButton"),
            Padding = new Thickness(14, 7, 16, 7),
            FontSize = 13.5,
            ToolTip = Brand.CoffeeUrl,
        };
        b.Click += (_, _) => Process.Start(new ProcessStartInfo(Brand.CoffeeUrl) { UseShellExecute = true });
        return b;
    }
}
