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

    /// <summary>The same channels grouped by country; used only to tag channels with their countries.</summary>
    public const string CountryIndexUrl = "https://iptv-org.github.io/iptv/index.country.m3u";

    /// <summary>Item field holding a channel's countries (';'-separated).</summary>
    public const string CountryField = "country";

    /// <summary>Whether a playlist location is iptv-org's (the Free TV playlist).</summary>
    public static bool IsFreeTv(string location) =>
        location.Equals(AllUrl, StringComparison.OrdinalIgnoreCase) || IsCountries(location);

    /// <summary>0.4.8's "only these countries" playlist location (still read so it can be merged away).</summary>
    public static bool IsCountries(string location) => location.StartsWith("iptvorg:countries=", StringComparison.OrdinalIgnoreCase);

    public static IEnumerable<string> CountryUrls(string location) =>
        location["iptvorg:countries=".Length..].Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(c => c.Length == 2 && c.All(char.IsLetter))
            .Select(c => $"https://iptv-org.github.io/iptv/countries/{c.ToLowerInvariant()}.m3u");
}
