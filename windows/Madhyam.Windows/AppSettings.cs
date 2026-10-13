using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Media;
using Microsoft.Win32;

namespace Madhyam.Windows;

/// <summary>
/// User preferences, stored as settings.json in %LOCALAPPDATA%\Madhyam.
/// Every property has a default, so an old or missing file always loads.
/// </summary>
public sealed class AppSettings
{
    // ---- General
    public string Theme { get; set; } = "Dark";            // Dark | Light | System
    public string CoverSize { get; set; } = "Medium";      // Small | Medium | Large
    public bool TitlesUnderCovers { get; set; } = true;
    public string StartupView { get; set; } = "Live";      // Live | Movies | Series | Favorites | Recent | Last
    public string LastView { get; set; } = "Live";
    public string WindowStartup { get; set; } = "Last";    // Last | Maximized | Fullscreen
    public double? WinLeft { get; set; }
    public double? WinTop { get; set; }
    public double? WinWidth { get; set; }
    public double? WinHeight { get; set; }
    public bool WinMaximized { get; set; }
    public bool StripPrefixes { get; set; }
    public bool ShowNumbers { get; set; }

    // ---- Playback
    public string LiveFormat { get; set; } = "Auto";       // Auto | TS | HLS
    public int NetworkCachingMs { get; set; } = 1500;
    public bool HardwareDecoding { get; set; } = true;
    public bool ResumePlayback { get; set; } = true;
    public bool ShowSubtitles { get; set; } = true;
    public string AudioLanguage { get; set; } = "";
    public string SubtitleLanguage { get; set; } = "";

    // ---- Guide
    public string GuideView { get; set; } = "Timeline";    // Timeline | List
    public int EpgOffsetMinutes { get; set; }
    public int EpgRefreshHours { get; set; } = 12;
    public List<string> ExtraEpgUrls { get; set; } = new();

    // ------------------------------------------------------------------ io

    public static AppSettings Current { get; private set; } = new();

    private static string FilePath => Path.Combine(App.DataDir, "settings.json");
    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    public static void Load()
    {
        try
        {
            if (File.Exists(FilePath))
                Current = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(FilePath)) ?? new AppSettings();
        }
        catch (Exception e)
        {
            Repository.Log($"settings unreadable, using defaults: {e.Message}");
            Current = new AppSettings();
        }
    }

    public static void Save()
    {
        try
        {
            var tmp = FilePath + ".tmp";
            File.WriteAllText(tmp, JsonSerializer.Serialize(Current, Json));
            File.Move(tmp, FilePath, overwrite: true);
        }
        catch (Exception e)
        {
            Repository.Log($"settings not saved: {e.Message}");
        }
    }

    // --------------------------------------------------------------- theme

    /// <summary>Dark/Light after resolving "System" from the Windows app theme.</summary>
    public bool IsLight =>
        Theme == "Light" || (Theme == "System" && WindowsUsesLightTheme());

    private static bool WindowsUsesLightTheme()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return key?.GetValue("AppsUseLightTheme") is int v && v == 1;
        }
        catch
        {
            return false;
        }
    }

    private static readonly Dictionary<string, string> LightPalette = new()
    {
        ["Bg"] = "#F4F5F9",
        ["Rail"] = "#E9EBF2",
        ["Panel"] = "#FFFFFF",
        ["Card"] = "#FFFFFF",
        ["CardHover"] = "#ECEFF6",
        ["CardSelected"] = "#DFE8FF",
        ["Line"] = "#D6DAE5",
        ["Accent"] = "#2F6FEB",
        ["Fg"] = "#1A1D29",
        ["Muted"] = "#5E6479",
        ["LiveRed"] = "#D93838",
        ["Rating"] = "#13874B",
        ["Chip"] = "#E7EAF2",
        ["RailActive"] = "#DCE5FF",
        ["CatSelected"] = "#E3EAFF",
        ["Secondary"] = "#E3E6EF",
        ["Thumb"] = "#C3C8D6",
        ["PrimaryBg"] = "#1A1D29",
        ["PrimaryFg"] = "#FFFFFF",
        ["Badge"] = "#E6FFFFFF",
        ["Star"] = "#E0A100",
        ["Shade"] = "#00F4F5F9",
        ["ShadeEnd"] = "#F2F4F5F9",
    };

    /// <summary>
    /// Recolour the brushes in App.xaml before any window loads. Styles hold
    /// references to these brush objects, so changing their Color (instead of
    /// replacing them) restyles everything, including StaticResource uses.
    /// </summary>
    public static void ApplyTheme(ResourceDictionary res)
    {
        if (!Current.IsLight) return;
        foreach (var (key, hex) in LightPalette)
        {
            var color = (Color)ColorConverter.ConvertFromString(hex);
            // Replace (not mutate): brushes referenced by templates are frozen at load.
            // App.xaml styles use DynamicResource for these keys, so they follow the swap;
            // window XAML loads later and picks the new brushes up via StaticResource.
            if (res.Contains(key)) res[key] = new SolidColorBrush(color);
            if (res.Contains(key + "Color")) res[key + "Color"] = color;
        }
    }

    /// <summary>Poster sizes and title visibility, applied live through DynamicResource.</summary>
    public static void ApplyLayout(ResourceDictionary res)
    {
        var (w, h) = Current.CoverSize switch
        {
            "Small" => (136.0, 202.0),
            "Large" => (220.0, 328.0),
            _ => (176.0, 262.0),
        };
        res["PosterW"] = w;
        res["PosterH"] = h;
        res["PosterTitleVis"] = Current.TitlesUnderCovers ? Visibility.Visible : Visibility.Collapsed;
    }
}
