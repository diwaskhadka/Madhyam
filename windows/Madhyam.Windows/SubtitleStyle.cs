using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Media;
using System.Windows.Media.Effects;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// Subtitle style like YouTube's caption options: size in points, font, text / background /
/// window colour and opacity, edge style and position. Subtitle files are drawn by the app
/// itself (<see cref="Caption"/>), so any change shows at once. Tracks inside the film are drawn
/// by VLC's freetype renderer from LibVLC options: colours change live (<see cref="VlcLive"/>),
/// the rest makes the player restart the media, keeping the position.
/// Same options, names and defaults as the Android TV and Linux apps.
/// </summary>
public static class SubtitleStyle
{
    private static AppSettings S => AppSettings.Current;

    public const int DefaultPt = 24;
    public static readonly int[] Sizes = { 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72 };
    public static readonly string[] Fonts = { "Sans-serif", "Serif", "Monospace", "Casual", "Small caps" };
    public static readonly string[] Edges = { "None", "Drop shadow", "Raised", "Depressed", "Outline" };
    public static readonly string[] Positions = { "Bottom", "Lower third", "Middle", "Top" };
    public static readonly int[] TextOpacities = { 25, 50, 75, 100 };
    public static readonly int[] BoxOpacities = { 0, 25, 50, 75, 100 };

    private static readonly (string Name, Color Color)[] Palette =
    {
        ("White", Colors.White), ("Yellow", Color.FromRgb(255, 255, 0)), ("Green", Color.FromRgb(0, 255, 0)),
        ("Cyan", Color.FromRgb(0, 255, 255)), ("Blue", Color.FromRgb(0, 0, 255)), ("Magenta", Color.FromRgb(255, 0, 255)),
        ("Red", Color.FromRgb(255, 0, 0)), ("Black", Colors.Black),
    };

    public static readonly string[] TextColors = Palette.Select(p => p.Name).ToArray();
    /// <summary>Background and window colours: the same eight, black first.</summary>
    public static readonly string[] BoxColors = new[] { "Black" }.Concat(TextColors.Where(c => c != "Black")).ToArray();

    public static Color ColorOf(string name) => Palette.FirstOrDefault(p => p.Name == name).Name != null
        ? Palette.First(p => p.Name == name).Color
        : Colors.White;

    private static int Rgb(Color c) => (c.R << 16) | (c.G << 8) | c.B;

    private static int Byte(int percent) => (int)Math.Round(Math.Clamp(percent, 0, 100) * 2.55);

    /// <summary>Installed font for a family choice, falling back to the closest one present.</summary>
    public static string FontName(string choice)
    {
        string[] wanted = choice switch
        {
            "Serif" => new[] { "Georgia", "Times New Roman", "Cambria" },
            "Monospace" => new[] { "Consolas", "Cascadia Mono", "Courier New", "Lucida Console" },
            "Casual" => new[] { "Comic Sans MS", "Segoe Print", "Segoe Script" },
            "Small caps" => new[] { "Copperplate Gothic Light", "Copperplate Gothic Bold", "Felix Titling", "Arial" },
            _ => new[] { "Arial", "Segoe UI", "Tahoma" },
        };
        var installed = Fonts_Installed.Value;
        return wanted.FirstOrDefault(installed.Contains) ?? "Arial";
    }

    private static readonly Lazy<HashSet<string>> Fonts_Installed = new(() =>
        System.Windows.Media.Fonts.SystemFontFamilies.Select(f => f.Source).ToHashSet(StringComparer.OrdinalIgnoreCase));

    public static string PtLabel(int pt) => $"{pt} pt";

    /// <summary>Text height in pixels for a picture shown this many pixels high (1 pt = 1/72 inch at 1080p, 96 dpi).</summary>
    public static double Pixels(int pt, double pictureHeight) => pt * 96.0 / 72.0 * pictureHeight / 1080.0;

    /// <summary>Distance of the caption's bottom from the picture's bottom, as a fraction of its height.</summary>
    public static double BottomFraction()
    {
        var basePos = S.SubPosition switch
        {
            "Lower third" => 0.24,
            "Middle" => 0.46,
            "Top" => 0.86,
            _ => 0.0,
        };
        return Math.Clamp(basePos + S.SubPosOffset / 100.0, 0, 0.9);
    }

    /// <summary>
    /// VLC options for the current style (LibVLC instance arguments: the subtitle renderer
    /// belongs to the video output, which does not see options set on a media). Sizes are relative to the picture height, so
    /// subtitles keep their look in a window and in full screen; the margin needs the
    /// picture's own height in pixels (1080 when not known yet).
    /// </summary>
    public static IEnumerable<string> VlcArgs(int? videoHeight, bool withLive)
    {
        var inv = CultureInfo.InvariantCulture;
        // freetype-rel-fontsize: the picture height divided by the text height.
        var rel = (int)Math.Round(1080.0 / Pixels(Math.Clamp(S.SubFontSizePt, 6, 120), 1080));
        yield return $"--freetype-rel-fontsize={Math.Max(1, rel)}";
        yield return $"--freetype-font={FontName(S.SubFont)}";
        yield return $"--freetype-opacity={Byte(S.SubTextOpacity)}";
        if (withLive)
        {
            foreach (var (name, value) in LiveVlcValues()) yield return $"--{name}={value}";
        }
        var edge = S.SubEdge;
        yield return $"--freetype-outline-thickness={(edge == "Outline" ? 4 : 0)}";
        yield return "--freetype-outline-color=0";
        yield return $"--freetype-outline-opacity={(edge == "Outline" ? 255 : 0)}";
        var (shadowOpacity, angle, distance) = edge switch
        {
            "Drop shadow" => (220, -45.0, 0.07),
            "Raised" => (255, -45.0, 0.03),
            "Depressed" => (255, 135.0, 0.03),
            _ => (0, -45.0, 0.06),
        };
        yield return "--freetype-shadow-color=0";
        yield return $"--freetype-shadow-opacity={shadowOpacity}";
        yield return $"--freetype-shadow-angle={angle.ToString(inv)}";
        yield return $"--freetype-shadow-distance={distance.ToString(inv)}";
        var h = videoHeight is > 0 ? videoHeight.Value : 1080;
        yield return $"--sub-margin={(int)Math.Round(BottomFraction() * h)}";
    }

    /// <summary>
    /// The options VLC's subtitle renderer reads again for every new line, so they can change
    /// while a film plays (see <see cref="VlcLive"/>): text colour and the box behind the text.
    /// VLC draws one box; the window option (a box round the whole caption) is shown with it,
    /// whichever of the two is more opaque.
    /// </summary>
    public static IEnumerable<(string Name, int Value)> LiveVlcValues()
    {
        yield return ("freetype-color", Rgb(ColorOf(S.SubTextColor)));
        var (box, boxOpacity) = S.SubWindowOpacity > S.SubBackOpacity ? (S.SubWindowColor, S.SubWindowOpacity) : (S.SubBackColor, S.SubBackOpacity);
        yield return ("freetype-background-color", Rgb(ColorOf(box)));
        yield return ("freetype-background-opacity", Byte(boxOpacity));
    }

    /// <summary>All the style options in one string, to tell when a drawn caption is out of date.</summary>
    public static string Signature() =>
        $"{S.SubFontSizePt}/{S.SubFont}/{S.SubTextColor}/{S.SubTextOpacity}/{S.SubBackColor}/{S.SubBackOpacity}/" +
        $"{S.SubWindowColor}/{S.SubWindowOpacity}/{S.SubEdge}/{S.SubPosition}/{S.SubPosOffset}";

    public static void Reset()
    {
        S.SubFontSizePt = DefaultPt;
        S.SubFont = "Sans-serif";
        S.SubTextColor = "White";
        S.SubTextOpacity = 100;
        S.SubBackColor = "Black";
        S.SubBackOpacity = 50;
        S.SubWindowColor = "Black";
        S.SubWindowOpacity = 0;
        S.SubEdge = "Drop shadow";
        S.SubPosition = "Bottom";
        S.SubPosOffset = 0;
        AppSettings.Save();
    }

    // ================================================================ editor

    /// <summary>
    /// The options as rows (Settings › Playback and the player's "Subtitle style…" dialog).
    /// Every change is saved at once and reported through <paramref name="changed"/>.
    /// </summary>
    public static StackPanel Editor(Action changed)
    {
        var root = new StackPanel();
        void Build()
        {
            root.Children.Clear();
            void Changed()
            {
                AppSettings.Save();
                changed();
            }

            Panel Pick<T>(IEnumerable<T> values, T current, Func<T, string> label, Action<T> set) =>
                Segments(values.Select(v => (label(v), label(v))), label(current), l =>
                {
                    set(values.First(v => label(v) == l));
                    Changed();
                });

            // Font size: − 24 pt +
            var size = new TextBlock { Text = PtLabel(S.SubFontSizePt), MinWidth = 64, TextAlignment = TextAlignment.Center, VerticalAlignment = VerticalAlignment.Center, FontSize = 14 };
            void StepSize(int dir)
            {
                var i = Array.IndexOf(Sizes, S.SubFontSizePt);
                if (i < 0) i = Array.FindIndex(Sizes, x => x >= S.SubFontSizePt) is var j and >= 0 ? j : Sizes.Length - 1;
                S.SubFontSizePt = Sizes[Math.Clamp(i + dir, 0, Sizes.Length - 1)];
                size.Text = PtLabel(S.SubFontSizePt);
                Changed();
            }
            root.Children.Add(Line("Font size", "Typographic points at 1080p, scaled with the picture.", Stepper(size, () => StepSize(-1), () => StepSize(1), "Smaller", "Larger")));
            root.Children.Add(Line("Font family", "", Pick(Fonts, S.SubFont, x => x, v => S.SubFont = v)));
            root.Children.Add(Line("Text colour", "", Swatches(TextColors, S.SubTextColor, v => { S.SubTextColor = v; Changed(); })));
            root.Children.Add(Line("Text opacity", "", Pick(TextOpacities, S.SubTextOpacity, x => $"{x}%", v => S.SubTextOpacity = v)));
            root.Children.Add(Line("Background colour", "Behind the text.", Swatches(BoxColors, S.SubBackColor, v => { S.SubBackColor = v; Changed(); })));
            root.Children.Add(Line("Background opacity", "", Pick(BoxOpacities, S.SubBackOpacity, x => $"{x}%", v => S.SubBackOpacity = v)));
            root.Children.Add(Line("Window colour", "A box around the whole caption area.", Swatches(BoxColors, S.SubWindowColor, v => { S.SubWindowColor = v; Changed(); })));
            root.Children.Add(Line("Window opacity", "", Pick(BoxOpacities, S.SubWindowOpacity, x => $"{x}%", v => S.SubWindowOpacity = v)));
            root.Children.Add(Line("Edge style", "", Pick(Edges, S.SubEdge, x => x, v => S.SubEdge = v)));

            var offset = new TextBlock { MinWidth = 64, TextAlignment = TextAlignment.Center, VerticalAlignment = VerticalAlignment.Center, FontSize = 14 };
            void ShowOffset() => offset.Text = S.SubPosOffset == 0 ? "0%" : $"{S.SubPosOffset:+0;−0}%";
            ShowOffset();
            void Move(int d)
            {
                S.SubPosOffset = Math.Clamp(S.SubPosOffset + d, -20, 40);
                ShowOffset();
                Changed();
            }
            var pos = new StackPanel();
            pos.Children.Add(Pick(Positions, S.SubPosition, x => x, v => S.SubPosition = v));
            var fine = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 4, 0, 0) };
            fine.Children.Add(Text("Move up / down", 13, "Muted"));
            ((TextBlock)fine.Children[0]).VerticalAlignment = VerticalAlignment.Center;
            ((TextBlock)fine.Children[0]).Margin = new Thickness(0, 0, 10, 0);
            fine.Children.Add(Stepper(offset, () => Move(-2), () => Move(2), "Move down 2%", "Move up 2%", "", ""));
            pos.Children.Add(fine);
            root.Children.Add(Line("Position", "", pos));

            var reset = new Button { Content = "Reset to defaults", Style = St("SecondaryButton"), Padding = new Thickness(16, 7, 16, 7), HorizontalAlignment = HorizontalAlignment.Left };
            reset.Click += (_, _) =>
            {
                Reset();
                changed();
                Build();
            };
            root.Children.Add(Line("Reset to defaults", "24 pt sans-serif, white on 50% black, drop shadow, at the bottom.", reset));
        }
        Build();
        return root;
    }

    private static FrameworkElement Line(string title, string description, UIElement control)
    {
        var s = new StackPanel { Margin = new Thickness(0, 0, 0, 16) };
        s.Children.Add(Text(title, 14, "Fg", FontWeights.SemiBold));
        if (description.Length > 0)
        {
            var d = Text(description, 12.5, "Muted", wrap: true);
            d.Margin = new Thickness(0, 2, 0, 0);
            s.Children.Add(d);
        }
        if (control is FrameworkElement fe) fe.Margin = new Thickness(0, 8, 0, 0);
        s.Children.Add(control);
        return s;
    }

    private static FrameworkElement Stepper(TextBlock value, Action minus, Action plus, string minusTip, string plusTip, string minusGlyph = "", string plusGlyph = "")
    {
        var p = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Left };
        var m = new Button { Content = minusGlyph, Style = St("ChipButton"), MinWidth = 40, FontFamily = Icons, ToolTip = minusTip };
        m.Click += (_, _) => minus();
        var pl = new Button { Content = plusGlyph, Style = St("ChipButton"), MinWidth = 40, FontFamily = Icons, ToolTip = plusTip };
        pl.Click += (_, _) => plus();
        value.Foreground = B("Fg");
        p.Children.Add(m);
        p.Children.Add(value);
        p.Children.Add(pl);
        return p;
    }

    /// <summary>Colour choice: a row of round swatches, the chosen one ringed.</summary>
    private static Panel Swatches(IEnumerable<string> names, string current, Action<string> changed)
    {
        var p = new WrapPanel();
        var rings = new List<(string Name, Border Ring)>();
        foreach (var name in names)
        {
            var ring = new Border
            {
                Width = 34, Height = 34, CornerRadius = new CornerRadius(17), Margin = new Thickness(0, 0, 8, 6),
                BorderThickness = new Thickness(2.5), Padding = new Thickness(3), Cursor = System.Windows.Input.Cursors.Hand,
                ToolTip = name,
                Child = new Border
                {
                    CornerRadius = new CornerRadius(12), Background = new SolidColorBrush(ColorOf(name)),
                    BorderThickness = new Thickness(1), BorderBrush = B("Line"),
                },
            };
            rings.Add((name, ring));
            ring.MouseLeftButtonUp += (_, _) =>
            {
                foreach (var (n, r) in rings) r.BorderBrush = n == name ? B("Accent") : Brushes.Transparent;
                changed(name);
            };
            p.Children.Add(ring);
        }
        foreach (var (n, r) in rings) r.BorderBrush = n == current ? B("Accent") : Brushes.Transparent;
        return p;
    }

    // ================================================================ preview

    /// <summary>Sample line over a dark frame (Settings), redrawn by <see cref="UpdatePreview"/>.</summary>
    public static Border Preview()
    {
        var frame = new Border
        {
            Width = 640, Height = 360, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(12), ClipToBounds = true,
            Background = new LinearGradientBrush(Color.FromRgb(0x2A, 0x33, 0x48), Color.FromRgb(0x0B, 0x0D, 0x14), 70),
        };
        // A 16:9 picture, so the sample is true to scale for a video shown this size.
        UpdatePreview(frame);
        return frame;
    }

    private static readonly IReadOnlyList<SubSpan>[] SampleLines =
    {
        new[] { new SubSpan("This is how subtitles", false, false, false) },
        new[] { new SubSpan("will look while you watch.", false, false, false) },
    };

    public static void UpdatePreview(Border frame)
    {
        var h = frame.ActualHeight > 0 ? frame.ActualHeight : frame.Height;
        var window = Caption(SampleLines, h);
        window.VerticalAlignment = VerticalAlignment.Bottom;
        window.Margin = new Thickness(12, 0, 12, BottomMargin(h));
        var g = new Grid();
        g.Children.Add(new TextBlock
        {
            Text = "PREVIEW", FontSize = 10.5, FontWeight = FontWeights.Bold, Foreground = new SolidColorBrush(Color.FromArgb(0x99, 0xFF, 0xFF, 0xFF)),
            Margin = new Thickness(14, 10, 0, 0),
        });
        g.Children.Add(window);
        frame.Child = g;
    }

    /// <summary>Space below the caption for a picture this high: the position, at least a little off the edge.</summary>
    public static double BottomMargin(double pictureHeight) => Math.Max(pictureHeight * 0.04, BottomFraction() * pictureHeight);

    /// <summary>
    /// A caption in the current style for a picture shown <paramref name="pictureHeight"/> high:
    /// the window box with the lines inside, each line on its own background. Used by the
    /// preview and by the player for subtitle files, which the app draws itself.
    /// </summary>
    public static Border Caption(IEnumerable<IReadOnlyList<SubSpan>> lines, double pictureHeight)
    {
        var h = pictureHeight;
        var px = Pixels(S.SubFontSizePt, h);
        var font = new FontFamily(FontName(S.SubFont));
        var text = new TextBlock
        {
            FontFamily = font, FontSize = px, TextAlignment = TextAlignment.Center, TextWrapping = TextWrapping.Wrap,
            LineHeight = px * 1.25, LineStackingStrategy = LineStackingStrategy.BlockLineHeight,
        };
        if (S.SubFont == "Small caps") Typography.SetCapitals(text, FontCapitals.SmallCaps);
        var fg = ColorOf(S.SubTextColor);
        fg.A = (byte)Byte(S.SubTextOpacity);
        var bg = ColorOf(S.SubBackColor);
        bg.A = (byte)Byte(S.SubBackOpacity);
        var (fgBrush, bgBrush) = (new SolidColorBrush(fg), new SolidColorBrush(bg));
        var first = true;
        foreach (var line in lines)
        {
            if (!first) text.Inlines.Add(new LineBreak());
            first = false;
            text.Inlines.Add(new Run(" ") { Background = bgBrush });
            foreach (var span in line)
            {
                var run = new Run(span.Text) { Foreground = fgBrush, Background = bgBrush };
                if (span.Italic) run.FontStyle = FontStyles.Italic;
                if (span.Bold) run.FontWeight = FontWeights.Bold;
                if (span.Underline) run.TextDecorations = TextDecorations.Underline;
                text.Inlines.Add(run);
            }
            text.Inlines.Add(new Run(" ") { Background = bgBrush });
        }
        text.Effect = S.SubEdge switch
        {
            "Drop shadow" => new DropShadowEffect { Color = Colors.Black, Direction = 315, ShadowDepth = Math.Max(1, px * 0.07), BlurRadius = px * 0.12, Opacity = 0.9 },
            "Raised" => new DropShadowEffect { Color = Colors.Black, Direction = 315, ShadowDepth = Math.Max(1, px * 0.04), BlurRadius = 0, Opacity = 1 },
            "Depressed" => new DropShadowEffect { Color = Colors.Black, Direction = 135, ShadowDepth = Math.Max(1, px * 0.04), BlurRadius = 0, Opacity = 1 },
            "Outline" => new DropShadowEffect { Color = Colors.Black, ShadowDepth = 0, BlurRadius = Math.Max(3, px * 0.15), Opacity = 1 },
            _ => null,
        };
        var win = ColorOf(S.SubWindowColor);
        win.A = (byte)Byte(S.SubWindowOpacity);
        return new Border
        {
            Background = new SolidColorBrush(win), Padding = new Thickness(px * 0.3, px * 0.15, px * 0.3, px * 0.15),
            HorizontalAlignment = HorizontalAlignment.Center, IsHitTestVisible = false,
            Child = text,
        };
    }

    /// <summary>The player's "Subtitle style…" dialog: changes apply to the playing video as they are made.</summary>
    public static void ShowDialog(Window owner, Action changed)
    {
        var w = DialogKit.Make(owner, "Subtitle style", 560);
        w.SizeToContent = SizeToContent.Manual;
        w.Height = Math.Min(760, SystemParameters.WorkArea.Height - 60);
        w.ResizeMode = ResizeMode.CanResize;
        // At the left, so the subtitles at the bottom middle of the picture stay in view.
        w.WindowStartupLocation = WindowStartupLocation.Manual;
        var maximized = owner.WindowState == WindowState.Maximized;
        w.Left = (maximized ? SystemParameters.WorkArea.Left : owner.Left) + 24;
        w.Top = (maximized ? SystemParameters.WorkArea.Top : owner.Top) + 48;
        var root = new DockPanel { Margin = new Thickness(24, 20, 12, 16) };
        var head = new StackPanel { Margin = new Thickness(0, 0, 12, 12) };
        head.Children.Add(Text("Subtitle style", 21, "Fg", FontWeights.SemiBold));
        var sub = Text("Changes show on the video as you make them.", 13, "Muted", wrap: true);
        sub.Margin = new Thickness(0, 4, 0, 0);
        head.Children.Add(sub);
        DockPanel.SetDock(head, Dock.Top);
        root.Children.Add(head);
        var footer = DialogKit.Footer(DialogKit.Button("Done", true, w.Close));
        footer.Margin = new Thickness(0, 12, 12, 0);
        DockPanel.SetDock(footer, Dock.Bottom);
        root.Children.Add(footer);
        var scroll = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = new Border { Padding = new Thickness(0, 0, 12, 0), Child = Editor(changed) } };
        ElasticScroll.Attach(scroll);
        root.Children.Add(scroll);
        w.Content = root;
        w.ShowDialog();
    }
}
