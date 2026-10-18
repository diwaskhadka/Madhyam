using System.Windows.Media;

namespace Madhyam.Windows;

/// <summary>
/// The colour sets behind Settings › Visual theme: the built-in dark theme, Navy &amp; Crimson
/// (the brand colours of branding/README.md), and the Custom theme, which builds a whole
/// palette from one colour the user picks. Keys match the brushes in App.xaml.
/// </summary>
public static class ThemeColors
{
    public const string Crimson = "#DC143C";
    public const string RoyalBlue = "#003893";

    /// <summary>Picker presets with the names shown for them (same list on Android TV).</summary>
    public static readonly (string Name, string Hex)[] Presets =
    {
        ("Crimson", Crimson), ("Royal blue", RoyalBlue), ("Vermilion", "#E4572E"), ("Saffron", "#F2A541"),
        ("Green", "#2BA84A"), ("Teal", "#12A4A4"), ("Sky blue", "#2F6FEB"), ("Violet", "#6C4AD8"),
        ("Purple", "#B43FB4"), ("Rose", "#E0457B"), ("Slate", "#5E6B7D"), ("Brown", "#8B5E3C"),
    };

    /// <summary>"Crimson" for a preset colour, otherwise its hex code.</summary>
    public static string NameOf(string hex) =>
        Presets.FirstOrDefault(p => p.Hex.Equals(hex, StringComparison.OrdinalIgnoreCase)).Name ?? hex.ToUpperInvariant();

    /// <summary>Dark theme, Navy &amp; Crimson: deep navy surfaces, crimson highlights.</summary>
    public static readonly Dictionary<string, string> NavyCrimson = new()
    {
        ["Bg"] = "#0B1024",
        ["Rail"] = "#080C1C",
        ["Panel"] = "#10183A",
        ["Card"] = "#162048",
        ["CardHover"] = "#1C2856",
        ["CardSelected"] = "#2A2352",
        ["Line"] = "#26315E",
        ["Accent"] = "#F2506B",
        ["Fg"] = "#ECEEF7",
        ["Muted"] = "#97A0C3",
        ["LiveRed"] = "#FF6B81",
        ["Rating"] = "#7EE2A8",
        ["Chip"] = "#1E2958",
        ["RailActive"] = "#33224C",
        ["CatSelected"] = "#252452",
        ["Secondary"] = "#222C5C",
        ["Thumb"] = "#3A4680",
        ["PrimaryBg"] = "#F2F4FA",
        ["PrimaryFg"] = "#0B1024",
        ["Badge"] = "#D90B1024",
        ["Star"] = "#FFD166",
        ["Shade"] = "#330B1024",
        ["ShadeEnd"] = "#F20B1024",
    };

    /// <summary>
    /// A full palette from one colour: surfaces take its hue (dark or light), highlights use the
    /// colour itself, brightened or darkened just enough to stay readable on those surfaces.
    /// </summary>
    public static Dictionary<string, string> Custom(string accentHex, bool light)
    {
        var accent = Parse(accentHex) ?? (Color)ColorConverter.ConvertFromString(Crimson);
        var (h, s, l) = ToHsl(accent);
        var tint = Math.Min(s, 0.55); // strong colours give tinted greys, not neon surfaces
        string Hsl(double sat, double lig, byte alpha = 255) => Hex(FromHsl(h, sat, lig), alpha);
        string Mix(string over, double amount) => Hex(Blend(Parse(over)!.Value, accent, amount));

        if (!light)
        {
            var bg = Hsl(tint * 0.8, 0.07);
            var rail = Hsl(tint * 0.8, 0.05);
            var card = Hsl(tint * 0.7, 0.14);
            return new()
            {
                ["Bg"] = bg,
                ["Rail"] = rail,
                ["Panel"] = Hsl(tint * 0.75, 0.10),
                ["Card"] = card,
                ["CardHover"] = Hsl(tint * 0.7, 0.18),
                ["CardSelected"] = Mix(card, 0.22),
                ["Line"] = Hsl(tint * 0.55, 0.24),
                ["Accent"] = Hex(FromHsl(h, Math.Max(s, 0.55), Math.Max(l, 0.64))),
                ["Fg"] = Hsl(tint * 0.25, 0.94),
                ["Muted"] = Hsl(tint * 0.3, 0.68),
                ["LiveRed"] = "#FF6B81",
                ["Rating"] = "#7EE2A8",
                ["Chip"] = Hsl(tint * 0.65, 0.18),
                ["RailActive"] = Mix(rail, 0.30),
                ["CatSelected"] = Mix(card, 0.16),
                ["Secondary"] = Hsl(tint * 0.6, 0.20),
                ["Thumb"] = Hsl(tint * 0.5, 0.36),
                ["PrimaryBg"] = "#F2F4FA",
                ["PrimaryFg"] = bg,
                ["Badge"] = Hsl(tint * 0.8, 0.07, 0xD9),
                ["Star"] = "#FFD166",
                ["Shade"] = Hsl(tint * 0.8, 0.07, 0x33),
                ["ShadeEnd"] = Hsl(tint * 0.8, 0.07, 0xF2),
            };
        }

        var lbg = Hsl(tint * 0.5, 0.965);
        return new()
        {
            ["Bg"] = lbg,
            ["Rail"] = Hsl(tint * 0.5, 0.93),
            ["Panel"] = "#FFFFFF",
            ["Card"] = "#FFFFFF",
            ["CardHover"] = Hsl(tint * 0.5, 0.94),
            ["CardSelected"] = Mix("#FFFFFF", 0.16),
            ["Line"] = Hsl(tint * 0.35, 0.86),
            ["Accent"] = Hex(FromHsl(h, Math.Max(s, 0.6), Math.Min(l, 0.42))),
            ["Fg"] = Hsl(tint * 0.4, 0.13),
            ["Muted"] = Hsl(tint * 0.25, 0.42),
            ["LiveRed"] = "#D93838",
            ["Rating"] = "#13874B",
            ["Chip"] = Hsl(tint * 0.45, 0.92),
            ["RailActive"] = Mix(Hsl(tint * 0.5, 0.93), 0.18),
            ["CatSelected"] = Mix("#FFFFFF", 0.12),
            ["Secondary"] = Hsl(tint * 0.4, 0.91),
            ["Thumb"] = Hsl(tint * 0.3, 0.80),
            ["PrimaryBg"] = Hsl(tint * 0.4, 0.13),
            ["PrimaryFg"] = "#FFFFFF",
            ["Badge"] = "#E6FFFFFF",
            ["Star"] = "#E0A100",
            ["Shade"] = Hsl(tint * 0.5, 0.965, 0x00),
            ["ShadeEnd"] = Hsl(tint * 0.5, 0.965, 0xF2),
        };
    }

    // ---------------------------------------------------------------- colour maths

    public static Color? Parse(string? hex)
    {
        try
        {
            return string.IsNullOrWhiteSpace(hex) ? null : (Color)ColorConverter.ConvertFromString(hex.Trim().StartsWith('#') ? hex.Trim() : "#" + hex.Trim());
        }
        catch (FormatException)
        {
            return null;
        }
    }

    public static string Hex(Color c, byte alpha = 255) =>
        alpha == 255 ? $"#{c.R:X2}{c.G:X2}{c.B:X2}" : $"#{alpha:X2}{c.R:X2}{c.G:X2}{c.B:X2}";

    private static Color Blend(Color baseColor, Color over, double amount) => Color.FromRgb(
        (byte)Math.Round(baseColor.R + (over.R - baseColor.R) * amount),
        (byte)Math.Round(baseColor.G + (over.G - baseColor.G) * amount),
        (byte)Math.Round(baseColor.B + (over.B - baseColor.B) * amount));

    public static (double H, double S, double L) ToHsl(Color c)
    {
        double r = c.R / 255.0, g = c.G / 255.0, b = c.B / 255.0;
        double max = Math.Max(r, Math.Max(g, b)), min = Math.Min(r, Math.Min(g, b));
        var l = (max + min) / 2;
        if (max == min) return (0, 0, l);
        var d = max - min;
        var s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        var h = max == r ? (g - b) / d + (g < b ? 6 : 0) : max == g ? (b - r) / d + 2 : (r - g) / d + 4;
        return (h * 60, s, l);
    }

    public static Color FromHsl(double h, double s, double l)
    {
        h = ((h % 360) + 360) % 360 / 360;
        double Channel(double t)
        {
            var q = l < 0.5 ? l * (1 + s) : l + s - l * s;
            var p = 2 * l - q;
            t = (t + 1) % 1;
            return t < 1 / 6.0 ? p + (q - p) * 6 * t : t < 0.5 ? q : t < 2 / 3.0 ? p + (q - p) * (2 / 3.0 - t) * 6 : p;
        }
        return s == 0
            ? Color.FromRgb((byte)Math.Round(l * 255), (byte)Math.Round(l * 255), (byte)Math.Round(l * 255))
            : Color.FromRgb((byte)Math.Round(Channel(h + 1 / 3.0) * 255), (byte)Math.Round(Channel(h) * 255), (byte)Math.Round(Channel(h - 1 / 3.0) * 255));
    }
}
