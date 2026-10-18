using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// Settings › Visual theme › Custom: pick one colour (presets, hue/saturation/lightness sliders or
/// a hex code) and a dark or light base. The preview shows the palette the app builds from it.
/// </summary>
public static class ColorPickerWindow
{
    /// <summary>The chosen colour and base, or null when cancelled.</summary>
    public static (string Color, bool Light)? Show(Window owner, string color, bool light)
    {
        var w = DialogKit.Make(owner, "Custom theme colour", 460);
        var root = new StackPanel { Margin = new Thickness(22) };
        (string Color, bool Light)? result = null;

        var current = ThemeColors.Parse(color) ?? ThemeColors.Parse(ThemeColors.Crimson)!.Value;
        var isLight = light;
        var updating = false;

        // Preview: a mini window in the palette this colour produces.
        var preview = new Border { CornerRadius = new CornerRadius(12), Height = 92, Padding = new Thickness(12), Margin = new Thickness(0, 0, 0, 6) };
        var previewRail = new Border { Width = 34, CornerRadius = new CornerRadius(8) };
        var previewCard = new Border { CornerRadius = new CornerRadius(8), Padding = new Thickness(12, 8, 12, 8), Margin = new Thickness(10, 0, 0, 0) };
        var previewTitle = new TextBlock { Text = "Live TV", FontSize = 15, FontWeight = FontWeights.SemiBold };
        var previewMuted = new TextBlock { Text = "Now: Evening News", FontSize = 12 };
        var previewChip = new Border { CornerRadius = new CornerRadius(10), Padding = new Thickness(10, 3, 10, 3), Margin = new Thickness(0, 8, 0, 0), HorizontalAlignment = HorizontalAlignment.Left,
            Child = new TextBlock { Text = "Watch", FontSize = 12, FontWeight = FontWeights.SemiBold } };
        previewCard.Child = new StackPanel { Children = { previewTitle, previewMuted, previewChip } };
        var previewRow = new DockPanel();
        DockPanel.SetDock(previewRail, Dock.Left);
        previewRow.Children.Add(previewRail);
        previewRow.Children.Add(previewCard);
        preview.Child = previewRow;
        root.Children.Add(preview);
        var colorName = new TextBlock { FontSize = 14, FontWeight = FontWeights.SemiBold, Margin = new Thickness(2, 2, 0, 0) };
        root.Children.Add(colorName);

        // Presets.
        root.Children.Add(DialogKit.Label("Presets"));
        var swatches = new WrapPanel();
        root.Children.Add(swatches);

        // Sliders.
        Slider MakeSlider(string label, double max)
        {
            root.Children.Add(DialogKit.Label(label));
            var s = new Slider { Minimum = 0, Maximum = max, Style = St("SeekSlider"), IsMoveToPointEnabled = true };
            root.Children.Add(s);
            return s;
        }
        var hue = MakeSlider("Hue", 359);
        var sat = MakeSlider("Saturation", 100);
        var lig = MakeSlider("Lightness", 100);

        root.Children.Add(DialogKit.Label("Hex code"));
        var hex = new TextBox { Width = 140, HorizontalAlignment = HorizontalAlignment.Left };
        root.Children.Add(hex);

        root.Children.Add(DialogKit.Label("Base"));
        var basePanel = new WrapPanel();
        foreach (var (value, label) in new[] { (false, "☾  Dark"), (true, "☀  Light") })
        {
            var r = new RadioButton { Content = label, GroupName = "themeBase", IsChecked = value == isLight, Style = St("Segment") };
            r.Checked += (_, _) => { isLight = value; Refresh(fromSliders: false, fromHex: false); };
            basePanel.Children.Add(r);
        }
        root.Children.Add(basePanel);

        void Refresh(bool fromSliders, bool fromHex)
        {
            if (updating) return;
            updating = true;
            try
            {
                if (fromSliders) current = ThemeColors.FromHsl(hue.Value, sat.Value / 100, lig.Value / 100);
                var (h, s, l) = ThemeColors.ToHsl(current);
                if (!fromSliders)
                {
                    hue.Value = h;
                    sat.Value = s * 100;
                    lig.Value = l * 100;
                }
                if (!fromHex) hex.Text = ThemeColors.Hex(current);
                var name = ThemeColors.NameOf(ThemeColors.Hex(current));
                colorName.Text = name.StartsWith('#') ? name : $"{name} · {ThemeColors.Hex(current)}";

                var p = ThemeColors.Custom(ThemeColors.Hex(current), isLight);
                Brush P(string key) => new SolidColorBrush(ThemeColors.Parse(p[key])!.Value);
                preview.Background = P("Bg");
                previewRail.Background = P("Rail");
                previewCard.Background = P("Card");
                previewCard.BorderBrush = P("Accent");
                previewCard.BorderThickness = new Thickness(1.5);
                previewTitle.Foreground = P("Fg");
                previewMuted.Foreground = P("Muted");
                previewChip.Background = P("Accent");
                ((TextBlock)previewChip.Child).Foreground = P(isLight ? "PrimaryFg" : "Bg");
                hue.Background = new LinearGradientBrush(
                    new GradientStopCollection(Enumerable.Range(0, 7).Select(i => new GradientStop(ThemeColors.FromHsl(i * 60, 0.8, 0.55), i / 6.0))), 0);
            }
            finally
            {
                updating = false;
            }
        }

        foreach (var (presetName, preset) in ThemeColors.Presets)
        {
            var c = ThemeColors.Parse(preset)!.Value;
            var b = new Button
            {
                Width = 34, Height = 34, Margin = new Thickness(0, 0, 8, 8), ToolTip = presetName,
                Template = BareTemplate(), Cursor = System.Windows.Input.Cursors.Hand,
                Content = new Ellipse { Width = 30, Height = 30, Fill = new SolidColorBrush(c), Stroke = B("Line"), StrokeThickness = 1 },
            };
            b.Click += (_, _) => { current = c; Refresh(fromSliders: false, fromHex: false); };
            swatches.Children.Add(b);
        }
        hue.ValueChanged += (_, _) => Refresh(fromSliders: true, fromHex: false);
        sat.ValueChanged += (_, _) => Refresh(fromSliders: true, fromHex: false);
        lig.ValueChanged += (_, _) => Refresh(fromSliders: true, fromHex: false);
        hex.TextChanged += (_, _) =>
        {
            if (updating || ThemeColors.Parse(hex.Text) is not { } c || hex.Text.Trim().TrimStart('#').Length != 6) return;
            current = c;
            Refresh(fromSliders: false, fromHex: true);
        };

        root.Children.Add(DialogKit.Footer(
            DialogKit.Button("Cancel", false, w.Close),
            DialogKit.Button("Use this theme", true, () => { result = (ThemeColors.Hex(current), isLight); w.Close(); })));
        w.Content = root;
        Refresh(fromSliders: false, fromHex: false);
        w.ShowDialog();
        return result;
    }
}
