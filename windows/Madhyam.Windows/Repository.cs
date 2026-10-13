using System.ComponentModel;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using MadhyamCoreNet;

namespace Madhyam.Windows;

public sealed record Source(int Id, string Name, string Kind)
{
    public string Location { get; init; } = "";
    public string Username { get; init; } = "";
    public long UpdatedAt { get; init; }
    public string KindLabel => Kind == "xtream" ? "Xtream Code" : "M3U playlist";
}

/// <summary>Xtream account endpoint answer.</summary>
public sealed record AccountInfo(string Status, DateTime? Expires, int MaxConnections, int ActiveConnections, bool IsTrial,
                                 DateTime? Created, string ServerUrl, string Timezone, string Formats);

/// <summary>Extra information for a film (get_vod_info) or series (get_series_info).</summary>
public sealed class DetailsInfo
{
    public string? Plot { get; set; }
    public string? Cast { get; set; }
    public string? Director { get; set; }
    public string? Genre { get; set; }
    public string? Released { get; set; }
    public string? Duration { get; set; }
    public string? Rating { get; set; }
    public string? Country { get; set; }
    public string? Language { get; set; }
    public string? Poster { get; set; }
    public string? Backdrop { get; set; }
    /// <summary>YouTube video id (not a URL).</summary>
    public string? Trailer { get; set; }
    /// <summary>IMDb id such as "tt1234567" when the panel provides one.</summary>
    public string? ImdbId { get; set; }
}

public sealed record Group(string Name, int Count)
{
    public string Label => $"{Name}  ({Count})";
}

public sealed record Programme(string Title, long Start, long Stop, string Desc = "")
{
    public double Progress(long now) => Stop <= Start ? 0 : Math.Clamp((double)(now - Start) / (Stop - Start), 0, 1);
}

/// <summary>Base for small view models: raise PropertyChanged without boilerplate.</summary>
public abstract class Observable : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    protected void Set<T>(ref T field, T value, [System.Runtime.CompilerServices.CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return;
        field = value;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
    }

    protected void Raise(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

/// <summary>A channel, film, series or episode as shown in lists and grids.</summary>
public sealed class ItemVm : Observable
{
    public required string Id { get; init; }
    public required string Kind { get; init; }
    public required string Name { get; init; }
    public string Group { get; init; } = "";
    public string? Logo { get; init; }
    public string Url { get; init; } = "";
    public string? UserAgent { get; init; }
    public string? Referrer { get; init; }
    public string? Plot { get; init; }
    public string? Rating { get; init; }
    public string? Year { get; init; }
    public string? Genre { get; init; }
    public string? Season { get; init; }
    public string? Episode { get; init; }
    public int SourceId { get; init; }
    /// <summary>For episodes: id of the series item (opens the series page).</summary>
    public string? SeriesId { get; init; }
    public long AddedAt { get; init; }
    public long PlayedAt { get; init; }
    public double PositionSec { get; init; }
    public double DurationSec { get; init; }
    public double DurationHint { get; init; } // from the provider (episodes)

    public double WatchedRatio => DurationSec > 0 ? Math.Clamp(PositionSec / DurationSec, 0, 1) : 0;
    public bool IsWatched => WatchedRatio >= 0.95;
    public string TypeBadge => Kind switch { "live" => "LIVE", "series" => "SERIES", "episode" => "EPISODE", _ => "MOVIE" };
    public string TimeLeftText =>
        DurationSec > 0 ? $"{FormatDuration(DurationSec - PositionSec)} left" : "";
    public string AddedText => AddedAt > 0 ? DateTimeOffset.FromUnixTimeSeconds(AddedAt).LocalDateTime.ToString("d MMM, HH:mm") : "";
    public string PlayedText => PlayedAt > 0 ? DateTimeOffset.FromUnixTimeSeconds(PlayedAt).LocalDateTime.ToString("d MMM, HH:mm") : "";
    public string DurationText => DurationHint > 0 ? FormatClock(DurationHint) : "";

    public static string FormatDuration(double seconds)
    {
        var t = TimeSpan.FromSeconds(Math.Max(0, seconds));
        return t.TotalHours >= 1 ? $"{(int)t.TotalHours}h {t.Minutes}m" : $"{Math.Max(1, t.Minutes)}m";
    }

    public static string FormatClock(double seconds)
    {
        var t = TimeSpan.FromSeconds(Math.Max(0, seconds));
        return t.TotalHours >= 1 ? t.ToString(@"hh\:mm\:ss") : t.ToString(@"mm\:ss");
    }

    public bool IsLive => Kind == "live";
    public bool IsSeries => Kind == "series";
    public bool HasRating => !string.IsNullOrEmpty(Rating) && Rating != "0";
    public string Initials => DisplayName.Length >= 2 ? DisplayName[..2].ToUpperInvariant() : DisplayName.ToUpperInvariant();

    private int _number;
    /// <summary>Position in the current list, shown as "12. Channel".</summary>
    public int Number { get => _number; set { Set(ref _number, value); Raise(nameof(Title)); } }
    public string Title => Number > 0 && IsLive ? $"{Number}. {DisplayName}" : DisplayName;

    /// <summary>Same item with another picture (episodes shown with their series poster).</summary>
    public ItemVm WithLogo(string? logo) => new()
    {
        Id = Id, Kind = Kind, Name = Name, Group = Group, Logo = logo, Url = Url, UserAgent = UserAgent, Referrer = Referrer,
        Plot = Plot, Rating = Rating, Year = Year, Genre = Genre, Season = Season, Episode = Episode, SourceId = SourceId,
        SeriesId = SeriesId, AddedAt = AddedAt, PlayedAt = PlayedAt, PositionSec = PositionSec, DurationSec = DurationSec,
        DurationHint = DurationHint,
    };

    /// <summary>Re-evaluate display text after a naming setting changed.</summary>
    public void Refresh()
    {
        Raise(nameof(DisplayName));
        Raise(nameof(Title));
        Raise(nameof(Initials));
    }

    /// <summary>"CA EN: CBC Calgary" → "CBC Calgary" when Settings → Strip country prefixes is on.</summary>
    public string DisplayName => AppSettings.Current.StripPrefixes && IsLive ? StripPrefix(Name) : Name;

    private static readonly System.Text.RegularExpressions.Regex Prefix = new(
        @"^\s*(?:[A-Z0-9]{2,4})(?:[ \-]?[A-Z]{2,3})?\s*[|:\-–]\s+",
        System.Text.RegularExpressions.RegexOptions.Compiled);

    public static string StripPrefix(string name)
    {
        var m = Prefix.Match(name);
        return m.Success && m.Length < name.Length - 1 ? name[m.Length..] : name;
    }

    private bool _favorite;
    public bool IsFavorite { get => _favorite; set { Set(ref _favorite, value); Raise(nameof(StarGlyph)); } }
    /// <summary>Filled or outline star (Segoe Fluent Icons).</summary>
    public string StarGlyph => IsFavorite ? "\uE735" : "\uE734";

    // ---- what's on now (live channels) ----
    private string _nowTitle = "";
    public string NowTitle { get => _nowTitle; set => Set(ref _nowTitle, value); }
    private string _startText = "";
    public string StartText { get => _startText; set => Set(ref _startText, value); }
    private string _endText = "";
    public string EndText { get => _endText; set => Set(ref _endText, value); }
    private double _progress;
    public double Progress { get => _progress; set => Set(ref _progress, value); }
    private bool _hasNow;
    public bool HasNow { get => _hasNow; set => Set(ref _hasNow, value); }

    /// <summary>Second line under the name for films/episodes.</summary>
    public string Subtitle => Kind switch
    {
        "episode" => Plot ?? "",
        _ => string.Join("  ·  ", new[] { Year, Genre }.Where(x => !string.IsNullOrEmpty(x))),
    };

    public void SetNow(Programme? now, long unixNow)
    {
        if (now == null)
        {
            HasNow = false;
            NowTitle = "No programme information";
            return;
        }
        HasNow = true;
        NowTitle = now.Title;
        StartText = DateTimeOffset.FromUnixTimeSeconds(now.Start).LocalDateTime.ToString("HH:mm");
        EndText = DateTimeOffset.FromUnixTimeSeconds(now.Stop).LocalDateTime.ToString("HH:mm");
        Progress = now.Progress(unixNow);
    }
}

/// <summary>One guide entry under the video ("12:15 – 13:00  LIVE").</summary>
public sealed class ProgrammeVm
{
    public required Programme P { get; init; }
    public required long Now { get; init; }
    public bool IsLive => P.Start <= Now && Now < P.Stop;
    public bool IsPast => P.Stop <= Now;
    public double Progress => P.Progress(Now);
    public string TimeRange =>
        $"{DateTimeOffset.FromUnixTimeSeconds(P.Start).LocalDateTime:HH:mm} – {DateTimeOffset.FromUnixTimeSeconds(P.Stop).LocalDateTime:HH:mm}";
    public string Title => P.Title;
    public string Desc => P.Desc;
    public bool HasDesc => !string.IsNullOrWhiteSpace(P.Desc);
}

/// <summary>
/// All I/O for the UI. The Rust core parses, indexes and searches; this class
/// downloads bytes with HttpClient and converts JSON into view models.
/// Every method is safe to call from the UI thread (work runs on the thread pool).
/// </summary>
public sealed class Repository : IDisposable
{
    public const string UserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Madhyam/0.1";

    private readonly MadhyamCore _core = new();
    private readonly HttpClient _http;
    private string SnapshotPath => Path.Combine(App.DataDir, "catalog.jsonl");
    /// <summary>
    /// Downloaded guides are kept here and re-imported at start-up: the catalog
    /// snapshot deliberately excludes EPG (it is large and goes stale).
    /// </summary>
    private static string EpgDir => Path.Combine(App.DataDir, "epg");
    private static string EpgFile(int source, int index) => Path.Combine(EpgDir, $"source-{source}-{index}.xml");
    private static IEnumerable<string> EpgFiles(int source) =>
        Directory.Exists(EpgDir) ? Directory.EnumerateFiles(EpgDir, $"source-{source}-*.xml") : Enumerable.Empty<string>();

    public Repository()
    {
        var handler = new HttpClientHandler
        {
            AllowAutoRedirect = true,
            AutomaticDecompression = DecompressionMethods.GZip | DecompressionMethods.Deflate,
        };
        _http = new HttpClient(handler) { Timeout = TimeSpan.FromMinutes(3) };
        _http.DefaultRequestHeaders.UserAgent.ParseAdd(UserAgent);
    }

    public string CoreVersion => MadhyamCore.Version;

    private static Task<T> Bg<T>(Func<T> f) => Task.Run(f);
    private static Task Bg(Action f) => Task.Run(f);

    private static string? Str(JsonElement e, string key) =>
        e.ValueKind == JsonValueKind.Object && e.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String && v.GetString() is { Length: > 0 } s
            ? s
            : null;

    private static long Long(JsonElement e, string key) =>
        e.ValueKind == JsonValueKind.Object && e.TryGetProperty(key, out var v)
            ? v.ValueKind switch
            {
                JsonValueKind.Number => v.TryGetInt64(out var n) ? n : (long)v.GetDouble(),
                JsonValueKind.String => long.TryParse(v.GetString(), out var n) ? n : 0,
                _ => 0,
            }
            : 0;

    private static double Dbl(JsonElement e, string key) =>
        e.ValueKind == JsonValueKind.Object && e.TryGetProperty(key, out var v)
            ? v.ValueKind switch
            {
                JsonValueKind.Number => v.GetDouble(),
                JsonValueKind.String => double.TryParse(v.GetString(), System.Globalization.NumberStyles.Float,
                    System.Globalization.CultureInfo.InvariantCulture, out var d) ? d : 0,
                _ => 0,
            }
            : 0;

    private static ItemVm ParseItem(JsonElement o)
    {
        var extra = o.TryGetProperty("extra", out var x) ? x : default;
        var progress = o.TryGetProperty("progress", out var pr) ? pr : default;
        var kind = Str(o, "kind") ?? "live";
        var plot = Str(extra, "plot");
        var year = Str(extra, "year");
        if (year is { Length: > 4 }) year = year[..4]; // "2024-05-01" -> "2024"
        return new ItemVm
        {
            Id = Str(o, "id") ?? "",
            Kind = kind,
            Name = Str(o, "name") ?? "",
            Group = Str(o, "group") ?? "",
            Logo = Str(o, "logo"),
            Url = Str(o, "url") ?? "",
            UserAgent = Str(extra, "user_agent"),
            Referrer = Str(extra, "referrer"),
            Plot = plot,
            Rating = CleanRating(Str(extra, "rating")),
            Year = year,
            Genre = Str(extra, "genre"),
            Season = Str(extra, "season"),
            Episode = Str(extra, "episode"),
            SourceId = (int)Long(o, "source"),
            SeriesId = Str(o, "series"),
            AddedAt = Long(extra, "added") is var a and > 0 ? a : Long(extra, "last_modified"),
            PlayedAt = Long(o, "played_at"),
            PositionSec = Dbl(progress, "position"),
            DurationSec = Dbl(progress, "duration"),
            DurationHint = Dbl(extra, "duration_secs") is var d and > 0 ? d : Dbl(extra, "duration"),
        };
    }

    /// <summary>"7.5" stays, "72" becomes "7.2", junk like "7125" is dropped (same rule as the core's sort).</summary>
    private static string? CleanRating(string? raw)
    {
        if (raw == null || !double.TryParse(raw.Replace(',', '.'), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var r)) return null;
        if (r > 10 && r <= 100) r /= 10;
        if (r <= 0 || r > 10) return null;
        return Math.Round(r, 1).ToString("0.#", System.Globalization.CultureInfo.InvariantCulture);
    }

    private static List<ItemVm> Items(JsonElement arr) =>
        arr.ValueKind == JsonValueKind.Array ? arr.EnumerateArray().Select(ParseItem).ToList() : new List<ItemVm>();

    private System.Threading.Timer? _saveTimer;
    private readonly object _saveLock = new();

    /// <summary>
    /// Save a little later, once: favorites, watched marks and history change in bursts
    /// (a whole season at once), and every save writes the full catalog snapshot.
    /// </summary>
    private void SaveSoon()
    {
        lock (_saveLock)
        {
            _saveTimer ??= new System.Threading.Timer(_ => Save(), null, System.Threading.Timeout.Infinite, System.Threading.Timeout.Infinite);
            _saveTimer.Change(1500, System.Threading.Timeout.Infinite);
        }
    }

    private void Save()
    {
        lock (_saveLock)
        {
            try
            {
                // Passwords go to secrets.json (encrypted for this Windows user), never into the catalog file.
                bool secretsOk;
                try { SyncSecrets(); secretsOk = true; }
                catch (Exception e) { Log($"secrets not saved: {e.Message}"); secretsOk = false; }
                // Never drop a password from the catalog unless it is safely in secrets.json.
                _core.Call("save_snapshot", new { path = SnapshotPath, redact_passwords = secretsOk });
            }
            catch (IptvException e) { Log($"snapshot save failed: {e.Message}"); }
        }
    }

    // ---------------------------------------------------------------- secrets

    private static string SecretsPath => Path.Combine(App.DataDir, "secrets.json");
    private Dictionary<int, string> _secrets = new(); // source id → DPAPI-protected password (base64)

    private void LoadSecrets()
    {
        try
        {
            if (File.Exists(SecretsPath))
                _secrets = JsonSerializer.Deserialize<Dictionary<int, string>>(File.ReadAllText(SecretsPath)) ?? new();
        }
        catch (Exception e)
        {
            Log($"secrets unreadable: {e.Message}");
            _secrets = new();
        }
    }

    /// <summary>Copy every Xtream password from the engine into secrets.json (encrypted), dropping removed sources.</summary>
    private void SyncSecrets()
    {
        var changed = false;
        var ids = new HashSet<int>();
        foreach (var s in _core.Call("sources").EnumerateArray())
        {
            if (Str(s, "kind") != "xtream") continue;
            var id = s.GetProperty("id").GetInt32();
            ids.Add(id);
            var pw = _core.Call("source_password", new { source = id });
            var plain = pw.ValueKind == JsonValueKind.String ? pw.GetString() ?? "" : "";
            if (plain.Length == 0) continue; // keep what we have
            if (_secrets.TryGetValue(id, out var stored) && Secret.Unprotect(stored) == plain) continue;
            _secrets[id] = Secret.Protect(plain);
            changed = true;
        }
        foreach (var gone in _secrets.Keys.Where(k => !ids.Contains(k)).ToList())
        {
            _secrets.Remove(gone);
            changed = true;
        }
        if (!changed) return;
        var tmp = SecretsPath + ".tmp";
        File.WriteAllText(tmp, JsonSerializer.Serialize(_secrets));
        File.Move(tmp, SecretsPath, overwrite: true);
    }

    /// <summary>After loading a (redacted) catalog, give the engine its passwords back.</summary>
    private void RestoreSecrets()
    {
        foreach (var s in _core.Call("sources").EnumerateArray())
        {
            if (Str(s, "kind") != "xtream") continue;
            var id = s.GetProperty("id").GetInt32();
            var pw = _core.Call("source_password", new { source = id });
            var current = pw.ValueKind == JsonValueKind.String ? pw.GetString() ?? "" : "";
            if (current.Length > 0) continue; // an older, unredacted file: migrated on the next save
            if (_secrets.TryGetValue(id, out var stored) && Secret.Unprotect(stored) is { Length: > 0 } plain)
                _core.Call("update_source", new { source = id, password = plain });
        }
    }

    public static void Log(string msg)
    {
        try { File.AppendAllText(Path.Combine(App.DataDir, "app.log"), $"{DateTime.Now:u} {msg}\n"); } catch { /* logging must never crash */ }
    }

    // ---------------------------------------------------------------- sources

    public Task Init() => Bg(() =>
    {
        LoadSecrets();
        if (File.Exists(SnapshotPath))
        {
            try
            {
                _core.Call("load_snapshot", new { path = SnapshotPath });
                RestoreSecrets();
                // Older versions wrote the password in plain text: rewrite the file without it now.
                if (File.ReadLines(SnapshotPath).Take(50).Any(l => l.Contains("\"t\":\"source\"") && l.Contains("\"kind\":\"xtream\"") && !l.Contains("\"password\":\"\"")))
                    Save();
            }
            catch (IptvException e)
            {
                Log($"snapshot unreadable, starting empty: {e.Message}");
                File.Delete(SnapshotPath);
            }
        }
        if (!Directory.Exists(EpgDir)) return;
        foreach (var f in Directory.EnumerateFiles(EpgDir, "*.xml"))
        {
            try { _core.Call("import_xmltv_file", new { path = f }); }
            catch (IptvException e) { Log($"cached guide {Path.GetFileName(f)} unreadable: {e.Message}"); }
        }
    });

    /// <summary>Age of the cached guide for a source, or null if none is cached.</summary>
    public TimeSpan? EpgAge(int source)
    {
        var files = EpgFiles(source).ToList();
        return files.Count == 0 ? null : DateTime.Now - files.Min(File.GetLastWriteTime);
    }

    /// <summary>Does this source announce any guide URL (playlist header or Xtream)?</summary>
    public Task<bool> HasEpg(int source) => Bg(() => _core.Call("epg_fetch_urls", new { source }).GetArrayLength() > 0);

    public Task<List<Source>> Sources() => Bg(() =>
        _core.Call("sources").EnumerateArray()
            .Select(s => new Source(s.GetProperty("id").GetInt32(), Str(s, "name") ?? "", Str(s, "kind") ?? "m3u")
            {
                Location = Str(s, "location") ?? "",
                Username = Str(s, "username") ?? "",
                UpdatedAt = Long(s, "updated_at"),
            })
            .ToList());

    /// <summary>Change a source's name and connection details, then re-download it.</summary>
    public async Task UpdateSource(int id, string name, string? location, string? server, string? username, string? password)
    {
        await Bg(() => _core.Call("update_source", new
        {
            source = id,
            name,
            location = string.IsNullOrWhiteSpace(location) ? null : location.Trim(),
            server = string.IsNullOrWhiteSpace(server) ? null : server.Trim(),
            username = string.IsNullOrWhiteSpace(username) ? null : username.Trim(),
            password = string.IsNullOrEmpty(password) ? null : password,
        }));
        await Refresh(id);
    }

    /// <summary>The saved Xtream password (for the edit form), or "" for M3U.</summary>
    public Task<string> SourcePassword(int source) => Bg(() =>
    {
        var v = _core.Call("source_password", new { source });
        return v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
    });

    /// <summary>Xtream account status (login, expiry, connections).</summary>
    public async Task<AccountInfo> Account(int source)
    {
        var url = await Bg(() => _core.Call("account_url", new { source }).GetString() ?? "");
        using var doc = JsonDocument.Parse(await GetBytes(url, source));
        var root = doc.RootElement;
        if (!root.TryGetProperty("user_info", out var u)) throw new IptvException("api", "The server did not return account information.");
        root.TryGetProperty("server_info", out var si);
        static DateTime? Date(long t) => t > 0 ? DateTimeOffset.FromUnixTimeSeconds(t).LocalDateTime : null;
        var auth = u.TryGetProperty("auth", out var a) && (a.ValueKind == JsonValueKind.Number ? a.GetInt32() == 1 : a.ToString() is "1" or "true");
        if (!auth) throw new IptvException("api", "Login rejected: check the username and password.");
        var formats = u.TryGetProperty("allowed_output_formats", out var f) && f.ValueKind == JsonValueKind.Array
            ? string.Join(", ", f.EnumerateArray().Select(x => x.ToString()))
            : "";
        var server = Str(si, "url") is { } host ? $"{Str(si, "server_protocol") ?? "http"}://{host}:{Str(si, "port") ?? ""}".TrimEnd(':') : "";
        return new AccountInfo(Str(u, "status") ?? "", Date(Long(u, "exp_date")), (int)Long(u, "max_connections"),
            (int)Long(u, "active_cons"), Str(u, "is_trial") is "1", Date(Long(u, "created_at")), server, Str(si, "timezone") ?? "", formats);
    }

    /// <summary>Quick reachability check: Xtream login, or the first bytes of an M3U.</summary>
    public async Task<string> TestConnection(int source)
    {
        var src = (await Sources()).First(s => s.Id == source);
        var sw = System.Diagnostics.Stopwatch.StartNew();
        if (src.Kind == "xtream")
        {
            var acc = await Account(source);
            return $"Connected in {sw.ElapsedMilliseconds} ms · account {acc.Status}" +
                   (acc.Expires is { } e ? $", expires {e:d MMM yyyy}" : "");
        }
        if (System.IO.File.Exists(src.Location)) return "Local playlist file found.";
        var probe = FreeTv.IsCountries(src.Location) ? FreeTv.CountryUrls(src.Location).FirstOrDefault() ?? FreeTv.AllUrl : src.Location;
        using var req = new HttpRequestMessage(HttpMethod.Get, probe);
        ApplyUserAgent(req, source);
        using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead);
        return resp.IsSuccessStatusCode
            ? $"Reachable in {sw.ElapsedMilliseconds} ms (HTTP {(int)resp.StatusCode})."
            : $"Server answered HTTP {(int)resp.StatusCode}.";
    }

    private static void ApplyUserAgent(HttpRequestMessage req, int source)
    {
        if (AppSettings.Current.SourceUserAgent.TryGetValue(source, out var ua) && !string.IsNullOrWhiteSpace(ua))
        {
            req.Headers.UserAgent.Clear();
            req.Headers.TryAddWithoutValidation("User-Agent", ua);
        }
    }

    private async Task<byte[]> GetBytes(string url, int source)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        ApplyUserAgent(req, source);
        using var resp = await _http.SendAsync(req);
        if (!resp.IsSuccessStatusCode) throw new HttpRequestException($"HTTP {(int)resp.StatusCode}");
        return await resp.Content.ReadAsByteArrayAsync();
    }

    public async Task<int> AddM3u(string name, string url)
    {
        var id = await Bg(() => _core.Call("add_m3u_source", new { name, location = url }).GetProperty("id").GetInt32());
        AppSettings.Current.SourceAdded[id] = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        return await RefreshOrRemove(id);
    }

    public async Task<int> AddXtream(string name, string server, string username, string password)
    {
        var id = await Bg(() => _core.Call("add_xtream_source", new { name, server, username, password }).GetProperty("id").GetInt32());
        AppSettings.Current.SourceAdded[id] = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        return await RefreshOrRemove(id);
    }

    private async Task<int> RefreshOrRemove(int id)
    {
        try
        {
            await Refresh(id);
            return id;
        }
        catch
        {
            await Bg(() => _core.Call("remove_source", new { source = id }));
            throw;
        }
    }

    public Task RemoveSource(int id) => Bg(() =>
    {
        _core.Call("remove_source", new { source = id });
        foreach (var f in EpgFiles(id).ToList()) File.Delete(f);
        Save();
    });

    /// <summary>Re-download a source's catalog, then (best effort) its guide.</summary>
    public async Task Refresh(int id)
    {
        var src = (await Sources()).First(s => s.Id == id);
        if (src.Kind == "xtream")
        {
            await _core.SyncXtreamAsync(id, _http);
        }
        else
        {
            var location = await Bg(() => _core.Call("sources").EnumerateArray()
                .First(s => s.GetProperty("id").GetInt32() == id).GetProperty("location").GetString() ?? "");
            var file = FreeTv.IsCountries(location) ? await DownloadJoined(FreeTv.CountryUrls(location), id) : await Download(location, "playlist", id);
            try { await Bg(() => _core.Call("import_m3u_file", new { source = id, path = file })); }
            finally { File.Delete(file); }
        }
        await Bg(Save);
        try { await RefreshEpg(id); }
        catch (Exception e) { Log($"EPG refresh failed for source {id}: {e.Message}"); } // never block watching
    }

    public async Task RefreshEpg(int id)
    {
        var urls = await Bg(() => _core.Call("epg_fetch_urls", new { source = id }).EnumerateArray().Select(u => u.GetString() ?? "").ToList());
        Directory.CreateDirectory(EpgDir);
        var index = 0;
        foreach (var url in urls.Where(u => u.Length > 0))
        {
            string file;
            try { file = await Download(url, "guide", id); }
            catch (Exception e) { Log($"guide download failed: {e.Message}"); continue; }
            try
            {
                await Bg(() => _core.Call("import_xmltv_file", new { path = file }));
                // Keep it for the next start (plain XML, already decompressed).
                File.Move(file, EpgFile(id, index++), overwrite: true);
            }
            finally { if (File.Exists(file)) File.Delete(file); }
        }
        // Drop cached guides of URLs the source no longer lists.
        foreach (var stale in EpgFiles(id).Where(f => !Enumerable.Range(0, index).Select(i => EpgFile(id, i)).Contains(f)).ToList())
            File.Delete(stale);
        await Bg(Save);
    }

    /// <summary>Download several M3U playlists and join them into one file (Free TV by country).</summary>
    private async Task<string> DownloadJoined(IEnumerable<string> urls, int source)
    {
        var joined = Path.Combine(Path.GetTempPath(), $"madhyam-joined-{Guid.NewGuid():N}.m3u");
        await using var outFile = new StreamWriter(joined);
        await outFile.WriteLineAsync("#EXTM3U");
        var any = false;
        foreach (var url in urls)
        {
            string part;
            try { part = await Download(url, "playlist", source); }
            catch (Exception e) { Log($"free tv list {url} failed: {e.Message}"); continue; }
            try
            {
                foreach (var line in File.ReadLines(part))
                    if (!line.StartsWith("#EXTM3U", StringComparison.OrdinalIgnoreCase)) await outFile.WriteLineAsync(line);
                any = true;
            }
            finally { File.Delete(part); }
        }
        if (!any) throw new HttpRequestException("None of the chosen country lists could be downloaded.");
        return joined;
    }

    /// <summary>Stream a URL (or local file) to a temp file, gunzipping .gz content.</summary>
    private async Task<string> Download(string location, string name, int? source = null)
    {
        var tmp = Path.Combine(Path.GetTempPath(), $"madhyam-{name}-{Guid.NewGuid():N}.tmp");
        try
        {
            Stream input;
            HttpResponseMessage? resp = null;
            if (File.Exists(location))
            {
                input = File.OpenRead(location);
            }
            else
            {
                using var req = new HttpRequestMessage(HttpMethod.Get, location);
                if (source is { } sid) ApplyUserAgent(req, sid);
                resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead);
                if (!resp.IsSuccessStatusCode) throw new HttpRequestException($"HTTP {(int)resp.StatusCode} downloading {name}");
                input = await resp.Content.ReadAsStreamAsync();
            }
            using (resp)
            await using (input)
            {
                var buffered = new BufferedStream(input, 64 * 1024);
                var magic = new byte[2];
                var n = await buffered.ReadAsync(magic.AsMemory(0, 2));
                await using var file = File.Create(tmp);
                if (n == 2 && magic[0] == 0x1f && magic[1] == 0x8b)
                {
                    // Re-assemble the stream including the two bytes we peeked.
                    var whole = new ConcatStream(magic, buffered);
                    await using var gz = new GZipStream(whole, CompressionMode.Decompress);
                    await gz.CopyToAsync(file);
                }
                else
                {
                    await file.WriteAsync(magic.AsMemory(0, n));
                    await buffered.CopyToAsync(file);
                }
            }
            return tmp;
        }
        catch
        {
            File.Delete(tmp);
            throw;
        }
    }

    // --------------------------------------------------------------- browsing

    public Task<List<Group>> Groups(int source, string kind) => Bg(() =>
        _core.Call("groups", new { source, kind }).EnumerateArray()
            .Select(g => new Group(Str(g, "name") ?? "", g.GetProperty("count").GetInt32())).ToList());

    /// <summary>
    /// Sorted, filtered, paged listing (core "query"). group = null means all
    /// categories of the kind; source = null means every playlist.
    /// </summary>
    public Task<(int Total, List<ItemVm> Items)> Query(string kind, int? source, string? group, IEnumerable<string>? hidden,
                                                      string sort = "provider", double minRating = 0, int offset = 0, int limit = 100) =>
        Bg(() =>
        {
            var r = _core.Call("query", new
            {
                kind,
                source,
                group,
                hidden_groups = hidden?.ToArray() ?? Array.Empty<string>(),
                sort,
                min_rating = minRating,
                offset,
                limit,
            });
            return (r.GetProperty("total").GetInt32(), Items(r.GetProperty("items")));
        });

    public Task<List<ItemVm>> ContinueWatching(int limit = 20) => Bg(() => Items(_core.Call("continue_watching", new { limit })));

    public Task<List<ItemVm>> Favorites(string? kind) => Bg(() => Items(_core.Call("favorites", new { kind })));

    public Task<List<ItemVm>> Recent(string? kind, int limit = 60) => Bg(() => Items(_core.Call("recent", new { kind, limit })));

    public Task RemoveFromHistory(string id) => Bg(() => { _core.Call("remove_history", new { id }); SaveSoon(); });

    public Task ClearHistory() => Bg(() => { _core.Call("clear_history"); SaveSoon(); });

    public Task<ItemVm?> Item(string id) => Bg(() =>
    {
        try { return ParseItem(_core.Call("item", new { id })); }
        catch (IptvException) { return null; }
    });

    /// <summary>Mark many episodes at once (one background call, one save).</summary>
    public Task SetWatchedMany(IReadOnlyList<ItemVm> items, bool watched) => Bg(() =>
    {
        foreach (var it in items)
        {
            var dur = it.DurationSec > 0 ? it.DurationSec : it.DurationHint > 0 ? it.DurationHint : 1;
            _core.Call("set_progress", new { id = it.Id, position = watched ? dur : 0, duration = dur });
        }
        SaveSoon();
    });

    /// <summary>Mark a film or episode as watched / not watched.</summary>
    public Task SetWatched(ItemVm it, bool watched) => Bg(() =>
    {
        var dur = it.DurationSec > 0 ? it.DurationSec : it.DurationHint > 0 ? it.DurationHint : 1;
        _core.Call("set_progress", new { id = it.Id, position = watched ? dur : 0, duration = dur });
        SaveSoon();
    });

    public Task<(double Position, double Duration)> Progress(string id) => Bg(() =>
    {
        var it = _core.Call("item", new { id });
        return it.TryGetProperty("progress", out var p) ? (Dbl(p, "position"), Dbl(p, "duration")) : (0.0, 0.0);
    });

    // ---------------------------------------------------------------- details

    private readonly Dictionary<string, DetailsInfo> _details = new();
    private readonly Dictionary<string, byte[]> _seriesBodies = new();

    /// <summary>Extra film details from the Xtream panel (cached for the session).</summary>
    public async Task<DetailsInfo?> MovieDetails(ItemVm movie)
    {
        if (_details.TryGetValue(movie.Id, out var cached)) return cached;
        string url;
        try { url = await Bg(() => _core.Call("vod_info_url", new { id = movie.Id }).GetString() ?? ""); }
        catch (IptvException) { return null; } // M3U films have no details endpoint
        using var doc = JsonDocument.Parse(await GetBytes(url, movie.SourceId));
        var info = doc.RootElement.TryGetProperty("info", out var i) ? i : default;
        var d = ParseDetails(info);
        _details[movie.Id] = d;
        return d;
    }

    /// <summary>Series details; the same answer also carries the episodes.</summary>
    public async Task<DetailsInfo?> SeriesDetails(ItemVm series)
    {
        if (_details.TryGetValue(series.Id, out var cached)) return cached;
        var body = await SeriesBody(series);
        using var doc = JsonDocument.Parse(body);
        var info = doc.RootElement.TryGetProperty("info", out var i) ? i : default;
        var d = ParseDetails(info);
        _details[series.Id] = d;
        return d;
    }

    private async Task<byte[]> SeriesBody(ItemVm series)
    {
        if (_seriesBodies.TryGetValue(series.Id, out var b)) return b;
        var url = await Bg(() => _core.Call("series_info_url", new { id = series.Id }).GetString() ?? "");
        b = await GetBytes(url, series.SourceId);
        _seriesBodies[series.Id] = b;
        return b;
    }

    private static DetailsInfo ParseDetails(JsonElement info)
    {
        string? Any(params string[] keys) => keys.Select(k => Str(info, k)).FirstOrDefault(v => v != null);
        string? backdrop = null;
        if (info.ValueKind == JsonValueKind.Object && info.TryGetProperty("backdrop_path", out var bp))
            backdrop = bp.ValueKind == JsonValueKind.Array ? bp.EnumerateArray().Select(x => x.GetString()).FirstOrDefault(x => !string.IsNullOrEmpty(x))
                     : bp.ValueKind == JsonValueKind.String ? bp.GetString() : null;
        var trailer = Any("youtube_trailer", "trailer");
        if (trailer != null)
        {
            // Accept bare ids and full YouTube URLs.
            var m = System.Text.RegularExpressions.Regex.Match(trailer, @"(?:v=|youtu\.be/|embed/)([\w-]{11})");
            trailer = m.Success ? m.Groups[1].Value : trailer.Length == 11 ? trailer : null;
        }
        var secs = Dbl(info, "duration_secs");
        var duration = secs > 0 ? ItemVm.FormatClock(secs) : Any("duration", "episode_run_time");
        if (duration != null && int.TryParse(duration, out var mins) && mins < 600) duration = $"{mins} min";
        var rating = Any("rating", "rating_5based");
        if (rating is "0" or "0.0") rating = null;
        return new DetailsInfo
        {
            Plot = Any("plot", "description"),
            Cast = Any("cast", "actors"),
            Director = Any("director"),
            Genre = Any("genre"),
            Released = Any("releasedate", "releaseDate", "release_date"),
            Duration = duration,
            Rating = rating,
            Country = Any("country"),
            Language = Str(info.ValueKind == JsonValueKind.Object && info.TryGetProperty("audio", out var au) ? au : default, "language") ?? Any("language"),
            Poster = Any("movie_image", "cover_big", "cover"),
            Backdrop = backdrop,
            Trailer = trailer,
            ImdbId = Any("imdb_id", "imdb") is { } imdb && System.Text.RegularExpressions.Regex.Match(imdb, @"tt\d{5,10}") is { Success: true } m2 ? m2.Value : null,
        };
    }

    public Task<List<ItemVm>> Search(string query, string? kind = null) =>
        string.IsNullOrWhiteSpace(query)
            ? Task.FromResult(new List<ItemVm>())
            : Bg(() => Items(_core.Call("search", new { query, kind, limit = 300 })));

    /// <summary>Total items of one kind in a source (for "677 channels" under the playlist name).</summary>
    public Task<int> CountKind(int source, string kind) => Bg(() =>
        _core.Call("groups", new { source, kind }).EnumerateArray().Sum(g => g.GetProperty("count").GetInt32()));

    // ----------------------------------------------------------- guide data

    private static string CustomEpgFile(int index) => Path.Combine(EpgDir, $"custom-{index}.xml");

    /// <summary>Download the extra XMLTV URLs from Settings → Guide (in addition to each playlist's own).</summary>
    public async Task RefreshCustomEpg(IReadOnlyList<string> urls)
    {
        Directory.CreateDirectory(EpgDir);
        var index = 0;
        foreach (var url in urls.Where(u => !string.IsNullOrWhiteSpace(u)))
        {
            string file;
            try { file = await Download(url.Trim(), "guide"); }
            catch (Exception e) { Log($"extra guide {url} failed: {e.Message}"); continue; }
            try
            {
                await Bg(() => _core.Call("import_xmltv_file", new { path = file }));
                File.Move(file, CustomEpgFile(index++), overwrite: true);
            }
            finally { if (File.Exists(file)) File.Delete(file); }
        }
        foreach (var stale in Directory.EnumerateFiles(EpgDir, "custom-*.xml")
                     .Where(f => !Enumerable.Range(0, index).Select(CustomEpgFile).Contains(f)).ToList())
            File.Delete(stale);
    }

    public static TimeSpan? CustomEpgAge() =>
        Directory.Exists(EpgDir) && Directory.EnumerateFiles(EpgDir, "custom-*.xml").ToList() is { Count: > 0 } files
            ? DateTime.Now - files.Min(File.GetLastWriteTime)
            : null;

    /// <summary>Forget every downloaded guide (memory and disk).</summary>
    public Task ClearEpg() => Bg(() =>
    {
        _core.Call("clear_epg");
        if (Directory.Exists(EpgDir)) Directory.Delete(EpgDir, recursive: true);
    });

    // ---------------------------------------------------------------- backup

    /// <summary>Favorites, watch progress and history as JSON (no passwords).</summary>
    public Task<string> ExportUserState() => Bg(() => _core.Call("export_user_state").GetRawText());

    public Task ImportUserState(byte[] json) => Bg(() =>
    {
        _core.Import("user_state", json);
        Save();
    });


    public Task<bool> ToggleFavorite(string id) => Bg(() =>
    {
        var on = !_core.Call("item", new { id }).GetProperty("favorite").GetBoolean();
        _core.Call("set_favorite", new { id, on });
        SaveSoon();
        return on;
    });

    /// <summary>Episodes of a series, fetched from the panel the first time.</summary>
    public async Task<List<ItemVm>> Episodes(ItemVm series)
    {
        var eps = await Bg(() => Items(_core.Call("episodes", new { id = series.Id })));
        if (eps.Count > 0) return eps;
        var body = await SeriesBody(series);
        return await Bg(() =>
        {
            _core.Import("series_info", body, new { id = series.Id });
            SaveSoon();
            return Items(_core.Call("episodes", new { id = series.Id }));
        });
    }

    public Task<Dictionary<string, (Programme? Now, Programme? Next)>> NowNext(IReadOnlyList<string> ids) => Bg(() =>
    {
        var result = new Dictionary<string, (Programme? Now, Programme? Next)>();
        if (ids.Count == 0) return result;
        var res = _core.Call("now_next_many", new { ids, now = DateTimeOffset.UtcNow.ToUnixTimeSeconds() - Offset });
        foreach (var id in ids)
        {
            if (!res.TryGetProperty(id, out var o)) continue;
            result[id] = (Prog(o, "now"), Prog(o, "next"));
        }
        return result;
    });

    private static Programme? Prog(JsonElement o, string key) =>
        o.TryGetProperty(key, out var p) && p.ValueKind == JsonValueKind.Object
            ? ParseProgramme(p)
            : null;

    /// <summary>Settings → Guide → time offset, in seconds (positive = programmes shown later).</summary>
    private static long Offset => AppSettings.Current.EpgOffsetMinutes * 60L;

    private static Programme ParseProgramme(JsonElement p) =>
        new(Str(p, "title") ?? "", p.GetProperty("start").GetInt64() + Offset, p.GetProperty("stop").GetInt64() + Offset, Str(p, "desc") ?? "");

    /// <summary>Guide for one channel between two Unix times (for the EPG list under the video).</summary>
    public Task<List<Programme>> Guide(string id, long from, long to) => Bg(() =>
    {
        var res = _core.Call("guide", new { ids = new[] { id }, from = from - Offset, to = to - Offset });
        return res.TryGetProperty(id, out var list) && list.ValueKind == JsonValueKind.Array
            ? list.EnumerateArray().Select(ParseProgramme).ToList()
            : new List<Programme>();
    });

    /// <summary>Ids of all favorites, to draw filled stars in lists.</summary>
    public Task<HashSet<string>> FavoriteIds() => Bg(() =>
        _core.Call("favorites").EnumerateArray().Select(f => Str(f, "id") ?? "").ToHashSet());

    // -------------------------------------------------------------- playback

    public Task RecordPlay(string id) => Bg(() => { _core.Call("record_play", new { id }); });

    /// <summary>Resume position in seconds (0 = start over).</summary>
    public Task<double> ResumeSeconds(string id) => Bg(() =>
    {
        var it = _core.Call("item", new { id });
        if (!it.TryGetProperty("progress", out var p)) return 0.0;
        var pos = p.GetProperty("position").GetDouble();
        var dur = p.GetProperty("duration").GetDouble();
        return dur > 0 && pos / dur > 0.95 ? 0.0 : pos;
    });

    public Task SetProgress(string id, double positionSec, double durationSec) =>
        Bg(() => { _core.Call("set_progress", new { id, position = positionSec, duration = durationSec }); });

    public void Persist() => Save();

    public void Dispose()
    {
        _core.Dispose();
        _http.Dispose();
    }
}

/// <summary>Read-only stream that yields a prefix buffer, then another stream.</summary>
internal sealed class ConcatStream(byte[] prefix, Stream rest) : Stream
{
    private int _pos;

    public override int Read(byte[] buffer, int offset, int count)
    {
        if (_pos < prefix.Length)
        {
            var n = Math.Min(count, prefix.Length - _pos);
            Array.Copy(prefix, _pos, buffer, offset, n);
            _pos += n;
            return n;
        }
        return rest.Read(buffer, offset, count);
    }

    public override bool CanRead => true;
    public override bool CanSeek => false;
    public override bool CanWrite => false;
    public override long Length => throw new NotSupportedException();
    public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
    public override void Flush() { }
    public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
    public override void SetLength(long value) => throw new NotSupportedException();
    public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
}
