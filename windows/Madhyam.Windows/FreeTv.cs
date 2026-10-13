namespace Madhyam.Windows;

/// <summary>
/// Free channels from iptv-org (github.com/iptv-org/iptv): public streams collected by the
/// community. Added as an ordinary M3U playlist named "Free TV" when turned on in Settings.
/// </summary>
public static class FreeTv
{
    public const string Name = "Free TV";

    /// <summary>Every channel, grouped by category (News, Sports, Kids…).</summary>
    public const string AllUrl = "https://iptv-org.github.io/iptv/index.category.m3u";

    /// <summary>Playlist location for the chosen countries (understood by <see cref="Repository"/>).</summary>
    public static string CountriesLocation(IEnumerable<string> codes) => "iptvorg:countries=" + string.Join(",", codes.Select(c => c.Trim().ToLowerInvariant()));

    public static bool IsCountries(string location) => location.StartsWith("iptvorg:countries=", StringComparison.OrdinalIgnoreCase);

    public static IEnumerable<string> CountryUrls(string location) =>
        location["iptvorg:countries=".Length..].Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(c => c.Length == 2 && c.All(char.IsLetter))
            .Select(c => $"https://iptv-org.github.io/iptv/countries/{c.ToLowerInvariant()}.m3u");

    /// <summary>Common countries offered as chips (any other two-letter code can be typed).</summary>
    public static readonly (string Code, string Name)[] Countries =
    {
        ("us", "United States"), ("ca", "Canada"), ("gb", "United Kingdom"), ("ie", "Ireland"), ("au", "Australia"),
        ("nz", "New Zealand"), ("in", "India"), ("np", "Nepal"), ("pk", "Pakistan"), ("bd", "Bangladesh"),
        ("lk", "Sri Lanka"), ("de", "Germany"), ("fr", "France"), ("es", "Spain"), ("it", "Italy"),
        ("pt", "Portugal"), ("nl", "Netherlands"), ("be", "Belgium"), ("ch", "Switzerland"), ("at", "Austria"),
        ("se", "Sweden"), ("no", "Norway"), ("dk", "Denmark"), ("fi", "Finland"), ("pl", "Poland"),
        ("gr", "Greece"), ("tr", "Turkey"), ("ru", "Russia"), ("ua", "Ukraine"), ("ro", "Romania"),
        ("mx", "Mexico"), ("br", "Brazil"), ("ar", "Argentina"), ("co", "Colombia"), ("cl", "Chile"),
        ("jp", "Japan"), ("kr", "South Korea"), ("cn", "China"), ("ph", "Philippines"), ("id", "Indonesia"),
        ("sa", "Saudi Arabia"), ("ae", "UAE"), ("eg", "Egypt"), ("ng", "Nigeria"), ("za", "South Africa"),
    };
}
