using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>Shared look for the small code-built dialogs.</summary>
internal static class DialogKit
{
    public static Window Make(Window owner, string title, double width)
    {
        var w = new Window
        {
            Title = title,
            Owner = owner,
            Width = width,
            SizeToContent = SizeToContent.Height,
            ResizeMode = ResizeMode.NoResize,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = B("Bg"),
            Foreground = B("Fg"),
            FontFamily = (System.Windows.Media.FontFamily)Application.Current.FindResource("UiFont"),
            ShowInTaskbar = false,
        };
        w.PreviewKeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape) w.Close();
        };
        return w;
    }

    public static Button Button(string label, bool primary, Action click)
    {
        var b = new Button
        {
            Content = label,
            Style = St(primary ? "PrimaryButton" : "SecondaryButton"),
            Margin = new Thickness(10, 0, 0, 0),
            MinWidth = 90,
        };
        b.Click += (_, _) => click();
        return b;
    }

    public static StackPanel Footer(params Button[] buttons)
    {
        var p = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 18, 0, 0) };
        foreach (var b in buttons) p.Children.Add(b);
        return p;
    }

    public static TextBlock Label(string text) => new() { Text = text, Foreground = B("Muted"), Margin = new Thickness(0, 10, 0, 3), FontSize = 12.5 };
}

/// <summary>Two-column read-only information (Playlist info, Account info).</summary>
public static class InfoWindow
{
    public static void Show(Window owner, string title, IEnumerable<(string Label, string Value)> rows)
    {
        var w = DialogKit.Make(owner, title, 520);
        var root = new StackPanel { Margin = new Thickness(24) };
        root.Children.Add(Text(title, 21, "Fg", FontWeights.SemiBold));
        var card = new Border { CornerRadius = new CornerRadius(12), Background = B("Panel"), Padding = new Thickness(16, 6, 16, 6), Margin = new Thickness(0, 14, 0, 0) };
        var g = new Grid();
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(160) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var r = 0;
        foreach (var (label, value) in rows)
        {
            g.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var l = Text(label, 13, "Muted");
            l.Margin = new Thickness(0, 8, 0, 8);
            Grid.SetRow(l, r);
            g.Children.Add(l);
            // Selectable, so a server address can be copied.
            var v = new TextBox
            {
                Text = string.IsNullOrEmpty(value) ? "—" : value,
                IsReadOnly = true,
                BorderThickness = new Thickness(0),
                Background = System.Windows.Media.Brushes.Transparent,
                Foreground = B("Fg"),
                FontSize = 13.5,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 6, 0, 6),
                Padding = new Thickness(0),
            };
            Grid.SetRow(v, r);
            Grid.SetColumn(v, 1);
            g.Children.Add(v);
            r++;
        }
        card.Child = g;
        root.Children.Add(card);
        root.Children.Add(DialogKit.Footer(DialogKit.Button("Close", true, w.Close)));
        w.Content = root;
        w.ShowDialog();
    }
}

/// <summary>Show / hide categories ("Manage categories").</summary>
public static class ManageCategoriesWindow
{
    /// <returns>The new set of hidden category names, or null when cancelled.</returns>
    public static HashSet<string>? Show(Window owner, List<Group> groups, HashSet<string> hidden)
    {
        var w = DialogKit.Make(owner, "Manage categories", 560);
        var root = new StackPanel { Margin = new Thickness(24) };
        root.Children.Add(Text("Manage categories", 21, "Fg", FontWeights.SemiBold));
        var sub = Text("Unticked categories are hidden from the list, from “All items” and from search.", 13, "Muted", wrap: true);
        sub.Margin = new Thickness(0, 4, 0, 12);
        root.Children.Add(sub);

        var filter = new TextBox { ToolTip = "Search categories" };
        var checks = groups.Select(gr => new CheckBox
        {
            Content = $"{gr.Name}   ({gr.Count:N0})",
            Tag = gr.Name,
            IsChecked = !hidden.Contains(gr.Name),
            Foreground = B("Fg"),
            Margin = new Thickness(2, 5, 2, 5),
            FontSize = 13.5,
        }).ToList();
        var count = Text("", 12.5, "Muted");
        void Count() => count.Text = $"{checks.Count(c => c.IsChecked == true):N0} of {checks.Count:N0} shown";
        foreach (var c in checks) c.Click += (_, _) => Count();
        Count();

        var list = new StackPanel();
        foreach (var c in checks) list.Children.Add(c);
        filter.TextChanged += (_, _) =>
        {
            var f = filter.Text.Trim();
            foreach (var c in checks)
                c.Visibility = f.Length == 0 || ((string)c.Tag).Contains(f, StringComparison.CurrentCultureIgnoreCase) ? Visibility.Visible : Visibility.Collapsed;
        };

        var tools = new DockPanel { Margin = new Thickness(0, 0, 0, 8) };
        var sel = new StackPanel { Orientation = Orientation.Horizontal };
        void SetVisible(bool on)
        {
            foreach (var c in checks.Where(c => c.Visibility == Visibility.Visible)) c.IsChecked = on;
            Count();
        }
        var all = new Button { Content = "Select all", Style = St("ChipButton") };
        all.Click += (_, _) => SetVisible(true);
        var none = new Button { Content = "Deselect all", Style = St("ChipButton") };
        none.Click += (_, _) => SetVisible(false);
        sel.Children.Add(all);
        sel.Children.Add(none);
        DockPanel.SetDock(sel, Dock.Right);
        tools.Children.Add(sel);
        count.VerticalAlignment = VerticalAlignment.Center;
        tools.Children.Add(count);

        root.Children.Add(filter);
        var panel = new Border
        {
            CornerRadius = new CornerRadius(12),
            Background = B("Panel"),
            Padding = new Thickness(12, 6, 12, 6),
            Margin = new Thickness(0, 10, 0, 0),
            Height = 380,
            Child = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = list },
        };
        var wrap = new StackPanel();
        wrap.Children.Add(tools);
        wrap.Children.Add(panel);
        wrap.Margin = new Thickness(0, 12, 0, 0);
        root.Children.Add(wrap);

        HashSet<string>? result = null;
        root.Children.Add(DialogKit.Footer(
            DialogKit.Button("Cancel", false, w.Close),
            DialogKit.Button("Save", true, () =>
            {
                result = checks.Where(c => c.IsChecked != true).Select(c => (string)c.Tag).ToHashSet();
                w.Close();
            })));
        w.Content = root;
        w.Loaded += (_, _) => filter.Focus();
        w.ShowDialog();
        return result;
    }
}

/// <summary>Edit a playlist: name, connection details, user agent.</summary>
public sealed class EditSourceResult
{
    public string Name { get; init; } = "";
    public string? Location { get; init; }
    public string? Server { get; init; }
    public string? Username { get; init; }
    public string? Password { get; init; }
    public string UserAgent { get; init; } = "";
}

public static class EditSourceWindow
{
    public static EditSourceResult? Show(Window owner, Source s, string userAgent)
    {
        var w = DialogKit.Make(owner, "Edit playlist", 540);
        var root = new StackPanel { Margin = new Thickness(24) };
        root.Children.Add(Text("Edit playlist", 21, "Fg", FontWeights.SemiBold));
        var kind = Text(s.KindLabel, 13, "Muted");
        kind.Margin = new Thickness(0, 4, 0, 4);
        root.Children.Add(kind);

        var name = new TextBox { Text = s.Name };
        root.Children.Add(DialogKit.Label("Title"));
        root.Children.Add(name);

        TextBox? server = null, user = null, location = null;
        PasswordBox? pass = null;
        if (s.Kind == "xtream")
        {
            server = new TextBox { Text = s.Location };
            user = new TextBox { Text = s.Username };
            pass = new PasswordBox();
            root.Children.Add(DialogKit.Label("Server URL"));
            root.Children.Add(server);
            root.Children.Add(DialogKit.Label("Username"));
            root.Children.Add(user);
            root.Children.Add(DialogKit.Label("Password (leave empty to keep the current one)"));
            root.Children.Add(pass);
        }
        else
        {
            location = new TextBox { Text = s.Location };
            root.Children.Add(DialogKit.Label("Playlist URL or file"));
            root.Children.Add(location);
        }

        var ua = new TextBox { Text = userAgent };
        root.Children.Add(DialogKit.Label("User agent (optional; some providers require a specific one)"));
        root.Children.Add(ua);

        var error = Text("", 12.5, "LiveRed", wrap: true);
        error.Margin = new Thickness(0, 10, 0, 0);
        root.Children.Add(error);
        var note = Text("Saving re-downloads the playlist. Favorites and progress are kept.", 12.5, "Muted", wrap: true);
        note.Margin = new Thickness(0, 6, 0, 0);
        root.Children.Add(note);

        EditSourceResult? result = null;
        var save = DialogKit.Button("Save", true, () =>
        {
            if (server != null && server.Text.Trim().Length == 0) { error.Text = "Enter the server address."; return; }
            if (user != null && user.Text.Trim().Length == 0) { error.Text = "Enter the username."; return; }
            if (location != null && location.Text.Trim().Length == 0) { error.Text = "Enter a playlist URL or file."; return; }
            result = new EditSourceResult
            {
                Name = name.Text.Trim().Length > 0 ? name.Text.Trim() : s.Name,
                Location = location?.Text.Trim(),
                Server = server?.Text.Trim(),
                Username = user?.Text.Trim(),
                Password = pass?.Password,
                UserAgent = ua.Text.Trim(),
            };
            w.Close();
        });
        save.IsDefault = true;
        root.Children.Add(DialogKit.Footer(DialogKit.Button("Cancel", false, w.Close), save));
        w.Content = root;
        w.Loaded += (_, _) => name.Focus();
        w.ShowDialog();
        return result;
    }
}
