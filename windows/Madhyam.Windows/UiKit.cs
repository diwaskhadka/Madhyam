using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;

namespace Madhyam.Windows;

/// <summary>
/// Small builders for the code-built pages (dashboard, search, favorites,
/// recent, recently added, details). Everything reads its colours from the
/// theme resources so light/dark both work.
/// </summary>
public static class UiKit
{
    public static Brush B(string key) => (Brush)Application.Current.FindResource(key);
    public static Style St(string key) => (Style)Application.Current.FindResource(key);
    public static FontFamily Icons => (FontFamily)Application.Current.FindResource("Icons");

    public static ImageSource? Image(string? url, int decodeWidth = 0)
    {
        if (string.IsNullOrWhiteSpace(url)) return null;
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri)) return null;
        try
        {
            var bmp = new BitmapImage();
            bmp.BeginInit();
            bmp.UriSource = uri;
            bmp.CacheOption = BitmapCacheOption.OnLoad;
            bmp.CreateOptions = BitmapCreateOptions.IgnoreColorProfile;
            if (decodeWidth > 0) bmp.DecodePixelWidth = decodeWidth; // keeps memory low for big grids
            bmp.EndInit();
            return bmp;
        }
        catch
        {
            return null;
        }
    }

    public static TextBlock Text(string text, double size = 14, string brush = "Fg", FontWeight? weight = null, bool wrap = false) => new()
    {
        Text = text,
        FontSize = size,
        Foreground = B(brush),
        FontWeight = weight ?? FontWeights.Normal,
        TextWrapping = wrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
        TextTrimming = TextTrimming.CharacterEllipsis,
    };

    public static TextBlock Icon(string glyph, double size = 14, string brush = "Fg") => new()
    {
        Text = glyph,
        FontFamily = Icons,
        FontSize = size,
        Foreground = B(brush),
        VerticalAlignment = VerticalAlignment.Center,
    };

    public static Button IconButton(string glyph, string tooltip, Action onClick, double size = 38)
    {
        var b = new Button { Content = glyph, Style = St("IconButton"), ToolTip = tooltip, Width = size, Height = size };
        b.Click += (_, e) => { e.Handled = true; onClick(); };
        return b;
    }

    public static Button Primary(string label, string? glyph, Action onClick, bool secondary = false)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal };
        var fg = secondary ? "Fg" : "PrimaryFg";
        if (glyph != null) row.Children.Add(new TextBlock { Text = glyph, FontFamily = Icons, Foreground = B(fg), VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 10, 0) });
        row.Children.Add(new TextBlock { Text = label, Foreground = B(fg), VerticalAlignment = VerticalAlignment.Center });
        var b = new Button { Content = row, Style = St(secondary ? "SecondaryButton" : "PrimaryButton"), Margin = new Thickness(0, 0, 10, 10) };
        b.Click += (_, _) => onClick();
        return b;
    }

    public static Border Chip(string text, string brush = "Fg") => new()
    {
        CornerRadius = new CornerRadius(6),
        BorderThickness = new Thickness(1),
        BorderBrush = B("Line"),
        Background = B("Card"),
        Padding = new Thickness(10, 4, 10, 4),
        Margin = new Thickness(0, 0, 8, 8),
        Child = new TextBlock { Text = text, FontSize = 13, Foreground = B(brush) },
    };

    public static Border Badge(string text, string brush = "Rating") => new()
    {
        CornerRadius = new CornerRadius(5),
        Background = B("Badge"),
        Padding = new Thickness(6, 1, 6, 1),
        Child = new TextBlock { Text = text, FontSize = 10, FontWeight = FontWeights.Bold, Foreground = B(brush) },
    };

    // ----------------------------------------------------------- cards

    public const double PosterW = 150, PosterH = 222;

    /// <summary>Poster card: image, optional type badge, rating, progress, title, subtitle, optional ✕.</summary>
    public static FrameworkElement Poster(ItemVm it, Action<ItemVm> open, string? subtitle = null, bool typeBadge = false,
                                          Action<ItemVm>? remove = null, double w = PosterW, double h = PosterH)
    {
        var img = new Grid { Width = w, Height = h };
        img.Children.Add(new Border
        {
            CornerRadius = new CornerRadius(10),
            Background = B("Card"),
            Child = new TextBlock
            {
                Text = it.IsLive ? "" : "",
                FontFamily = Icons,
                FontSize = 28,
                Foreground = B("Muted"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        });
        if (Image(it.Logo, (int)(w * 1.5)) is { } src)
        {
            img.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(10),
                Background = new ImageBrush(src) { Stretch = it.IsLive ? Stretch.Uniform : Stretch.UniformToFill },
                Margin = it.IsLive ? new Thickness(18) : new Thickness(0),
            });
        }
        var hover = new Border { CornerRadius = new CornerRadius(10), BorderThickness = new Thickness(2), BorderBrush = Brushes.Transparent };
        img.Children.Add(hover);
        if (typeBadge)
        {
            var tb = Badge(it.TypeBadge, it.IsLive ? "LiveRed" : it.IsSeries ? "Rating" : "Accent");
            tb.HorizontalAlignment = HorizontalAlignment.Left;
            tb.VerticalAlignment = VerticalAlignment.Top;
            tb.Margin = new Thickness(7);
            img.Children.Add(tb);
        }
        if (it.HasRating && !it.IsLive)
        {
            var rb = Badge($"★ {it.Rating}");
            rb.HorizontalAlignment = HorizontalAlignment.Right;
            rb.VerticalAlignment = VerticalAlignment.Top;
            rb.Margin = remove != null ? new Thickness(7, 7, 36, 7) : new Thickness(7); // leave room for ✕
            img.Children.Add(rb);
        }
        if (it.DurationSec > 0 && it.WatchedRatio > 0)
        {
            var bar = new ProgressBar { Style = St("ThinProgress"), Value = it.WatchedRatio, VerticalAlignment = VerticalAlignment.Bottom, Margin = new Thickness(8, 0, 8, 8) };
            img.Children.Add(bar);
        }
        if (remove != null)
        {
            var x = IconButton("", "Remove", () => remove(it), 26);
            x.FontSize = 10;
            x.Background = B("Badge");
            x.HorizontalAlignment = HorizontalAlignment.Right;
            x.VerticalAlignment = VerticalAlignment.Top;
            x.Margin = new Thickness(5);
            img.Children.Add(x);
        }

        var stack = new StackPanel { Width = w };
        stack.Children.Add(img);
        var title = Text(it.DisplayName, 13, "Fg", FontWeights.SemiBold);
        title.Margin = new Thickness(2, 8, 2, 0);
        stack.Children.Add(title);
        if (!string.IsNullOrEmpty(subtitle))
        {
            var sub = Text(subtitle!, 11.5, "Muted");
            sub.Margin = new Thickness(2, 2, 2, 0);
            stack.Children.Add(sub);
        }
        var card = new Button
        {
            Tag = "press",
            Content = stack,
            Cursor = Cursors.Hand,
            Focusable = false,
            Margin = new Thickness(0, 0, 16, 18),
            ToolTip = it.Name,
            Template = BareTemplate(),
        };
        card.MouseEnter += (_, _) => hover.BorderBrush = B("Accent");
        card.MouseLeave += (_, _) => hover.BorderBrush = Brushes.Transparent;
        card.Click += (_, _) => open(it);
        return card;
    }

    /// <summary>Wide live-channel card: logo, name, now playing with progress.</summary>
    public static FrameworkElement LiveCard(ItemVm it, Action<ItemVm> open, Action<ItemVm>? remove = null)
    {
        var g = new Grid { Width = 300 };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(56) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var logo = new Border { Width = 48, Height = 48, CornerRadius = new CornerRadius(10), Background = B("Chip") };
        if (Image(it.Logo, 96) is { } src) logo.Child = new Image { Source = src, Margin = new Thickness(4), Stretch = Stretch.Uniform };
        else logo.Child = new TextBlock { Text = it.Initials, Foreground = B("Muted"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        g.Children.Add(logo);
        var info = new StackPanel { Margin = new Thickness(10, 0, 4, 0), VerticalAlignment = VerticalAlignment.Center };
        info.Children.Add(Text(it.DisplayName, 13.5, "Fg", FontWeights.SemiBold));
        var now = Text(it.HasNow ? it.NowTitle : "No programme information", 12, "Muted");
        info.Children.Add(now);
        if (it.HasNow)
        {
            var row = new Grid { Margin = new Thickness(0, 4, 0, 0) };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.Children.Add(Text(it.StartText, 10.5, "Muted"));
            var bar = new ProgressBar { Style = St("ThinProgress"), Value = it.Progress, Margin = new Thickness(6, 0, 6, 0), VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(bar, 1);
            row.Children.Add(bar);
            var end = Text(it.EndText, 10.5, "Muted");
            Grid.SetColumn(end, 2);
            row.Children.Add(end);
            info.Children.Add(row);
        }
        Grid.SetColumn(info, 1);
        g.Children.Add(info);
        if (remove != null)
        {
            var x = IconButton("", "Remove", () => remove(it), 28);
            x.FontSize = 10;
            Grid.SetColumn(x, 2);
            g.Children.Add(x);
        }
        var border = new Border { CornerRadius = new CornerRadius(12), Background = B("Card"), Padding = new Thickness(10), Child = g, BorderThickness = new Thickness(1.5), BorderBrush = Brushes.Transparent };
        var card = new Button { Tag = "press", Content = border, Cursor = Cursors.Hand, Focusable = false, Margin = new Thickness(0, 0, 12, 12), Template = BareTemplate(), ToolTip = it.Name };
        card.MouseEnter += (_, _) => border.BorderBrush = B("Accent");
        card.MouseLeave += (_, _) => border.BorderBrush = Brushes.Transparent;
        card.Click += (_, _) => open(it);
        return card;
    }

    /// <summary>Button template that draws only its content (cards handle their own hover).</summary>
    public static ControlTemplate BareTemplate()
    {
        var t = new ControlTemplate(typeof(Button));
        t.VisualTree = new FrameworkElementFactory(typeof(ContentPresenter));
        return t;
    }

    // ----------------------------------------------------------- layout

    /// <summary>Section heading with a count pill and an optional "See all ›" link.</summary>
    public static FrameworkElement SectionHeader(string title, int? count = null, Action? seeAll = null, string seeAllLabel = "See all")
    {
        var g = new Grid { Margin = new Thickness(0, 26, 0, 12) };
        var left = new StackPanel { Orientation = Orientation.Horizontal };
        left.Children.Add(Text(title, 19, "Fg", FontWeights.SemiBold));
        if (count is { } c)
        {
            left.Children.Add(new Border
            {
                CornerRadius = new CornerRadius(9),
                Background = B("Chip"),
                Padding = new Thickness(7, 1, 7, 1),
                Margin = new Thickness(10, 3, 0, 0),
                VerticalAlignment = VerticalAlignment.Center,
                Child = Text(c.ToString("N0"), 11, "Muted"),
            });
        }
        g.Children.Add(left);
        if (seeAll != null)
        {
            var link = new Button
            {
                Content = Text($"{seeAllLabel}  ›", 12.5, "Muted"),
                Template = BareTemplate(),
                Cursor = Cursors.Hand,
                HorizontalAlignment = HorizontalAlignment.Right,
                VerticalAlignment = VerticalAlignment.Center,
                Focusable = false,
            };
            link.Click += (_, _) => seeAll();
            g.Children.Add(link);
        }
        return g;
    }

    /// <summary>Horizontal rail of cards with mouse-wheel scrolling.</summary>
    public static FrameworkElement Rail(IEnumerable<FrameworkElement> cards)
    {
        var panel = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var c in cards) panel.Children.Add(c);
        var sv = new ScrollViewer
        {
            Content = panel,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Padding = new Thickness(0, 0, 0, 6),
        };
        sv.PreviewMouseWheel += (_, e) =>
        {
            // Wheel scrolls the page; Shift+wheel scrolls the rail sideways.
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Shift))
            {
                sv.ScrollToHorizontalOffset(sv.HorizontalOffset - e.Delta);
                e.Handled = true;
            }
            else
            {
                e.Handled = true;
                var parent = FindParent<ScrollViewer>(VisualTreeHelper.GetParent(sv));
                // Preview first so the page's elastic scrolling gets it, then the plain event as a fallback.
                var pe = new MouseWheelEventArgs(e.MouseDevice, e.Timestamp, e.Delta) { RoutedEvent = UIElement.PreviewMouseWheelEvent };
                parent?.RaiseEvent(pe);
                if (!pe.Handled) parent?.RaiseEvent(new MouseWheelEventArgs(e.MouseDevice, e.Timestamp, e.Delta) { RoutedEvent = UIElement.MouseWheelEvent });
            }
        };
        return sv;
    }

    /// <summary>First descendant of a type (breadth-first through the visual tree).</summary>
    public static T? FindChild<T>(DependencyObject root) where T : DependencyObject
    {
        var queue = new Queue<DependencyObject>();
        queue.Enqueue(root);
        while (queue.Count > 0)
        {
            var d = queue.Dequeue();
            for (var i = 0; i < VisualTreeHelper.GetChildrenCount(d); i++)
            {
                var c = VisualTreeHelper.GetChild(d, i);
                if (c is T t) return t;
                queue.Enqueue(c);
            }
        }
        return null;
    }

    public static T? FindParent<T>(DependencyObject? d) where T : DependencyObject
    {
        while (d != null)
        {
            if (d is T t) return t;
            d = VisualTreeHelper.GetParent(d);
        }
        return null;
    }

    public static WrapPanel Wrap(IEnumerable<FrameworkElement> cards)
    {
        var w = new WrapPanel();
        foreach (var c in cards) w.Children.Add(c);
        return w;
    }

    public static FrameworkElement Empty(string glyph, string title, string text)
    {
        var s = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 90, 0, 0) };
        s.Children.Add(new TextBlock { Text = glyph, FontFamily = Icons, FontSize = 44, Foreground = B("Accent"), HorizontalAlignment = HorizontalAlignment.Center });
        var t = Text(title, 18, "Accent", FontWeights.SemiBold);
        t.HorizontalAlignment = HorizontalAlignment.Center;
        t.Margin = new Thickness(0, 14, 0, 4);
        s.Children.Add(t);
        var d = Text(text, 13, "Muted", wrap: true);
        d.HorizontalAlignment = HorizontalAlignment.Center;
        d.TextAlignment = TextAlignment.Center;
        d.MaxWidth = 420;
        s.Children.Add(d);
        return s;
    }

    /// <summary>Rounded on/off filter chip with a check mark (advanced search filters).</summary>
    public static FrameworkElement Filter(string label, bool on, Action<bool> changed)
    {
        var tick = new TextBlock { Text = "\uE73E", FontFamily = Icons, FontSize = 11, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 0) };
        var text = new TextBlock { Text = label, FontSize = 13, VerticalAlignment = VerticalAlignment.Center };
        var row = new StackPanel { Orientation = Orientation.Horizontal };
        row.Children.Add(tick);
        row.Children.Add(text);
        var chip = new Border { CornerRadius = new CornerRadius(17), BorderThickness = new Thickness(1), Padding = new Thickness(14, 7, 16, 7), Child = row, Cursor = Cursors.Hand, Margin = new Thickness(0, 0, 10, 8) };
        var state = on;
        void Paint()
        {
            chip.Background = state ? B("CardSelected") : B("Card");
            chip.BorderBrush = state ? B("Accent") : B("Line");
            text.Foreground = state ? B("Accent") : B("Fg");
            tick.Foreground = B("Accent");
            tick.Visibility = state ? Visibility.Visible : Visibility.Collapsed;
        }
        Paint();
        chip.MouseLeftButtonUp += (_, _) =>
        {
            state = !state;
            Paint();
            changed(state);
        };
        return chip;
    }

    /// <summary>Chip-like segmented choice (All | Live | Movies | Series).</summary>
    public static Panel Segments(IEnumerable<(string Value, string Label)> options, string current, Action<string> changed)
    {
        var p = new StackPanel { Orientation = Orientation.Horizontal };
        var group = Guid.NewGuid().ToString("N");
        foreach (var (v, l) in options)
        {
            var r = new RadioButton { Content = l, GroupName = group, IsChecked = v == current, Style = St("Segment") };
            r.Checked += (_, _) => changed(v);
            p.Children.Add(r);
        }
        return p;
    }

    /// <summary>Blurred backdrop + gradient used behind hero and details pages.</summary>
    public static FrameworkElement Backdrop(ImageSource? img, double opacity = 0.35)
    {
        var g = new Grid { ClipToBounds = true };
        if (img != null)
        {
            g.Children.Add(new Border
            {
                Background = new ImageBrush(img) { Stretch = Stretch.UniformToFill },
                Opacity = opacity,
                Effect = new BlurEffect { Radius = 30, RenderingBias = RenderingBias.Performance },
                Margin = new Thickness(-40),
                // Blur once and reuse the bitmap: re-blurring a page-sized image on every frame made clicks feel slow.
                CacheMode = new BitmapCache(),
            });
        }
        g.Children.Add(new Border
        {
            Background = new LinearGradientBrush
            {
                StartPoint = new Point(0, 0),
                EndPoint = new Point(0, 1),
                GradientStops =
                {
                    new GradientStop((Color)Application.Current.FindResource("ShadeColor"), 0),
                    new GradientStop((Color)Application.Current.FindResource("ShadeEndColor"), 1),
                },
            },
        });
        return g;
    }

    // ----------------------------------------------------------- pop-up menus

    /// <summary>One row of a pop-up menu. Label "-" draws a separator; Glyph may be empty.</summary>
    public sealed record MenuRow(string Glyph, string Label, bool Checked, Action Run, string? Sub = null, string? GlyphBrush = null, string? LabelBrush = null);

    /// <summary>
    /// pop-up: optional caps header, rows with a leading icon and a
    /// trailing check mark, plus optional extra controls (delay steppers, chips).
    /// It is a real window, so it also draws above the video surface.
    /// </summary>
    /// <summary>The pop-up currently shown (Esc closes it before anything else).</summary>
    public static Popup? OpenPopup { get; private set; }

    /// <param name="rows">Choices; they scroll when there are many (e.g. 30 subtitle languages).</param>
    /// <param name="fixedRows">Actions that always stay visible below the choices.</param>
    /// <param name="extra">Other always-visible controls (delay steppers).</param>
    public static Popup PopMenu(FrameworkElement anchor, string? title, IEnumerable<MenuRow> rows, IEnumerable<UIElement>? extra = null,
                                bool above = false, double width = 250, IEnumerable<MenuRow>? fixedRows = null, double rowsMaxHeight = 420,
                                IEnumerable<UIElement>? prefix = null, PlacementMode? placement = null, bool keepOthers = false)
    {
        var popup = new Popup
        {
            PlacementTarget = anchor,
            Placement = placement ?? (above ? PlacementMode.Top : PlacementMode.Bottom),
            StaysOpen = false,
            AllowsTransparency = true,
            PopupAnimation = PopupAnimation.None, // our own "unfold" below
            VerticalOffset = placement == null ? (above ? -6 : 6) : -8,
            HorizontalOffset = placement == PlacementMode.Right ? 4 : 0,
        };
        void Close() => popup.IsOpen = false;
        UIElement Build(MenuRow r) => r.Label == "-"
            ? new Border { Height = 1, Background = B("Line"), Margin = new Thickness(6, 5, 6, 5) }
            : MenuRowElement(r, Close);

        var outer = new StackPanel();
        if (title != null) outer.Children.Add(MenuHeader(title));
        if (prefix != null)
            foreach (var p in prefix) outer.Children.Add(p);
        var list = new StackPanel();
        foreach (var r in rows) list.Children.Add(Build(r));
        var scroll = new ScrollViewer { Content = list, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, MaxHeight = rowsMaxHeight };
        ElasticScroll.Attach(scroll);
        outer.Children.Add(scroll);
        if (fixedRows != null)
            foreach (var r in fixedRows) outer.Children.Add(Build(r));
        if (extra != null)
            foreach (var e in extra) outer.Children.Add(e);
        var root = new Border
        {
            Width = width,
            CornerRadius = new CornerRadius(14),
            Background = B("Panel"),
            BorderBrush = B("Line"),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(6),
            Margin = new Thickness(12),
            Effect = new DropShadowEffect { BlurRadius = 22, ShadowDepth = 4, Opacity = 0.4 },
            Child = outer,
        };
        // A pop-up inherits fonts from its anchor; player and icon buttons use the icon font,
        // which turned every label into boxes. Reset text properties explicitly.
        TextElement.SetFontFamily(root, (FontFamily)Application.Current.FindResource("UiFont"));
        TextElement.SetFontSize(root, 13.5);
        TextElement.SetForeground(root, B("Fg"));
        popup.Child = root;
        popup.Closed += (_, _) => { if (OpenPopup == popup) OpenPopup = null; };
        if (!keepOthers) OpenPopup?.SetCurrentValue(Popup.IsOpenProperty, false);
        if (!keepOthers) OpenPopup = popup;
        popup.IsOpen = true;
        return popup;
    }

    public static TextBlock MenuHeader(string title) => new()
    {
        Text = title.ToUpperInvariant(),
        FontSize = 10.5,
        FontWeight = FontWeights.Bold,
        Foreground = B("Muted"),
        Margin = new Thickness(10, 6, 10, 6),
    };

    public static FrameworkElement MenuRowElement(MenuRow r, Action close)
    {
        var g = new Grid { Margin = new Thickness(10, 7, 10, 7) };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(28) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        if (r.Glyph.Length > 0)
            g.Children.Add(new TextBlock { Text = r.Glyph, FontFamily = Icons, FontSize = 14, Foreground = B(r.GlyphBrush ?? (r.Checked ? "Accent" : "Fg")), VerticalAlignment = VerticalAlignment.Center });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Text(r.Label, 13.5, r.LabelBrush ?? (r.Checked ? "Accent" : "Fg"), r.Checked ? FontWeights.SemiBold : FontWeights.Normal));
        if (r.Sub != null) text.Children.Add(Text(r.Sub, 11.5, "Muted"));
        Grid.SetColumn(text, 1);
        g.Children.Add(text);
        if (r.Checked)
        {
            var check = new TextBlock { Text = "\uE73E", FontFamily = Icons, FontSize = 12, Foreground = B("Accent"), VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(10, 0, 0, 0) };
            Grid.SetColumn(check, 2);
            g.Children.Add(check);
        }
        var row = new Border { CornerRadius = new CornerRadius(8), Background = r.Checked ? B("CardSelected") : Brushes.Transparent, Child = g, Cursor = Cursors.Hand };
        var normal = row.Background;
        row.MouseEnter += (_, _) => row.Background = B("CardHover");
        row.MouseLeave += (_, _) => row.Background = normal;
        row.MouseLeftButtonUp += (_, e) =>
        {
            e.Handled = true;
            close();
            r.Run();
        };
        return row;
    }

    /// <summary>"Label   [−]  value  [+]" stepper row for pop-ups (audio / subtitle delay).</summary>
    public static FrameworkElement Stepper(string glyph, string label, Func<string> value, Action minus, Action plus, Action? reset = null)
    {
        var g = new Grid { Margin = new Thickness(10, 6, 6, 6) };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(28) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.Children.Add(new TextBlock { Text = glyph, FontFamily = Icons, FontSize = 14, Foreground = B("Fg"), VerticalAlignment = VerticalAlignment.Center });
        var t = Text(label, 13.5);
        t.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(t, 1);
        g.Children.Add(t);
        var val = new TextBlock { Text = value(), MinWidth = 64, TextAlignment = TextAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Foreground = B("Fg"), FontSize = 12.5 };
        if (reset != null)
        {
            val.Cursor = Cursors.Hand;
            val.ToolTip = "Click to reset";
            val.MouseLeftButtonUp += (_, _) => { reset(); val.Text = value(); };
        }
        var p = new StackPanel { Orientation = Orientation.Horizontal };
        var m = IconButton("\uE738", "Earlier", () => { minus(); val.Text = value(); }, 28);
        m.FontSize = 11;
        var pl = IconButton("\uE710", "Later", () => { plus(); val.Text = value(); }, 28);
        pl.FontSize = 11;
        p.Children.Add(m);
        p.Children.Add(val);
        p.Children.Add(pl);
        Grid.SetColumn(p, 2);
        g.Children.Add(p);
        return g;
    }

    /// <summary>Back-compat helper: simple menu with icons below a button.</summary>
    public static void Menu(FrameworkElement anchor, IEnumerable<(string Glyph, string Label, bool Checked, Action Run)> items, string? title = null)
    {
        PopMenu(anchor, title, items.Select(i => new MenuRow(i.Glyph, i.Label, i.Checked, i.Run)));
    }
}
