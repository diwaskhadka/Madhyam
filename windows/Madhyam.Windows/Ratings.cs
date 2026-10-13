using System.Net.Http;
using System.Text.Json;

namespace Madhyam.Windows;

/// <summary>IMDb rating and Rotten Tomatoes score of a film or series.</summary>
public sealed record ExternalRatings(string? Imdb, string? ImdbVotes, string? Rotten, string? ImdbId);

/// <summary>
/// Looks ratings up on OMDb (omdbapi.com, free key). Nothing is shown when there is no key,
/// no match, or the service has no score for that title.
/// </summary>
public static class Ratings
{
    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(12) };
    private static readonly Dictionary<string, ExternalRatings?> Cache = new();

    public static async Task<ExternalRatings?> Fetch(string title, string? year, string? imdbId, bool series)
    {
        var key = AppSettings.Current.OmdbApiKey.Trim();
        if (key.Length == 0) return null;
        var cacheKey = imdbId ?? $"{title}|{year}|{series}";
        if (Cache.TryGetValue(cacheKey, out var hit)) return hit;
        ExternalRatings? result = null;
        try
        {
            var q = imdbId != null
                ? $"i={Uri.EscapeDataString(imdbId)}"
                : $"t={Uri.EscapeDataString(title)}{(year is { Length: 4 } ? $"&y={year}" : "")}&type={(series ? "series" : "movie")}";
            var json = await Http.GetStringAsync($"https://www.omdbapi.com/?apikey={Uri.EscapeDataString(key)}&{q}");
            using var doc = JsonDocument.Parse(json);
            var r = doc.RootElement;
            if (r.TryGetProperty("Response", out var ok) && ok.GetString() == "True")
            {
                static string? Val(JsonElement e, string k) => e.TryGetProperty(k, out var v) && v.GetString() is { } s && s != "N/A" && s.Length > 0 ? s : null;
                string? rotten = null;
                if (r.TryGetProperty("Ratings", out var list) && list.ValueKind == JsonValueKind.Array)
                    foreach (var x in list.EnumerateArray())
                        if (Val(x, "Source") == "Rotten Tomatoes") rotten = Val(x, "Value");
                var imdb = Val(r, "imdbRating");
                if (imdb != null || rotten != null) result = new ExternalRatings(imdb, Val(r, "imdbVotes"), rotten, Val(r, "imdbID"));
            }
        }
        catch (Exception ex)
        {
            Repository.Log($"ratings lookup failed for {title}: {ex.Message}");
        }
        Cache[cacheKey] = result;
        return result;
    }

    /// <summary>"EN - The Movie (2024) [4K]" → "The Movie".</summary>
    public static string CleanTitle(string name)
    {
        var s = System.Text.RegularExpressions.Regex.Replace(name, @"^\s*(?:4K-)?[A-Z0-9]{2,4}\s*[-|:]\s+", "");
        s = System.Text.RegularExpressions.Regex.Replace(s, @"[\(\[].*?[\)\]]", "");
        return s.Trim(' ', '-', '|');
    }
}
