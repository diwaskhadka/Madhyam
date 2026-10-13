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
    public string StartupView { get; set; } = "Dashboard"; // Dashboard | Live | Movies | Series | Favorites | Recent | Last
    public string LastView { get; set; } = "Dashboard";
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
    /// <summary>OpenSubtitles.com: API key (required), optional login (password DPAPI-encrypted), languages.</summary>
    public string OsApiKey { get; set; } = "";
    public string OsUsername { get; set; } = "";
    public string OsPassword { get; set; } = "";
    public string OsLanguages { get; set; } = "en";

    // ---- Guide
    public string GuideView { get; set; } = "Timeline";    // Timeline | List
    public int EpgOffsetMinutes { get; set; }
    public int EpgRefreshHours { get; set; } = 12;
    public List<string> ExtraEpgUrls { get; set; } = new();

    // ---- Dashboard (which rails are shown)
    public bool DashHero { get; set; } = true;
    public bool DashContinue { get; set; } = true;
    public bool DashLiveFavorites { get; set; } = true;
    public bool DashRecentLive { get; set; } = true;
    public bool DashFavoriteVod { get; set; } = true;
    public bool DashRecentlyAdded { get; set; } = true;

    // ---- Browsing state
    /// <summary>Hidden categories per "sourceId:kind" (Manage categories).</summary>
    public Dictionary<string, List<string>> HiddenCategories { get; set; } = new();
    /// <summary>Category list order per kind: provider | name_asc | name_desc.</summary>
    public Dictionary<string, string> CategorySort { get; set; } = new();
    public bool LiveSortAz { get; set; }
    /// <summary>Poster grid sort (core sort names) and minimum rating.</summary>
    public string VodSort { get; set; } = "provider";
    public double VodMinRating { get; set; }
    public int PerPage { get; set; } = 50;

    // ---- Sources
    /// <summary>When each source was added (Unix seconds), keyed by source id.</summary>
    public Dictionary<int, long> SourceAdded { get; set; } = new();
    /// <summary>Optional user agent per source, sent for playlist downloads and playback.</summary>
    public Dictionary<int, string> SourceUserAgent { get; set; } = new();

    public static string HiddenKey(int source, string kind) => $"{source}:{kind}";

    public HashSet<string> Hidden(int source, string kind) =>
        HiddenCategories.TryGetValue(HiddenKey(source, kind), out var l) ? l.ToHashSet() : new HashSet<string>();

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
