using System.ComponentModel;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using MadhyamCoreNet;

namespace Madhyam.Windows;

public sealed record Source(int Id, string Name, string Kind);

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

    public bool IsLive => Kind == "live";
    public bool IsSeries => Kind == "series";
    public bool HasRating => !string.IsNullOrEmpty(Rating) && Rating != "0";
    public string Initials => Name.Length >= 2 ? Name[..2].ToUpperInvariant() : Name.ToUpperInvariant();

    private int _number;
    /// <summary>Position in the current list, shown as "12. Channel".</summary>
    public int Number { get => _number; set { Set(ref _number, value); Raise(nameof(Title)); } }
    public string Title => Number > 0 && IsLive ? $"{Number}. {Name}" : Name;

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

    private static ItemVm ParseItem(JsonElement o)
    {
        var extra = o.TryGetProperty("extra", out var x) ? x : default;
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
            Rating = Str(extra, "rating"),
            Year = year,
            Genre = Str(extra, "genre"),
            Season = Str(extra, "season"),
            Episode = Str(extra, "episode"),
        };
    }

    private static List<ItemVm> Items(JsonElement arr) =>
        arr.ValueKind == JsonValueKind.Array ? arr.EnumerateArray().Select(ParseItem).ToList() : new List<ItemVm>();

    private void Save()
    {
        try { _core.Call("save_snapshot", new { path = SnapshotPath }); }
        catch (IptvException e) { Log($"snapshot save failed: {e.Message}"); }
    }

    public static void Log(string msg)
    {
        try { File.AppendAllText(Path.Combine(App.DataDir, "app.log"), $"{DateTime.Now:u} {msg}\n"); } catch { /* logging must never crash */ }
    }

    // ---------------------------------------------------------------- sources

    public Task Init() => Bg(() =>
    {
        if (File.Exists(SnapshotPath))
        {
            try { _core.Call("load_snapshot", new { path = SnapshotPath }); }
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
            .Select(s => new Source(s.GetProperty("id").GetInt32(), Str(s, "name") ?? "", Str(s, "kind") ?? "m3u"))
            .ToList());

    public async Task<int> AddM3u(string name, string url)
    {
        var id = await Bg(() => _core.Call("add_m3u_source", new { name, location = url }).GetProperty("id").GetInt32());
        return await RefreshOrRemove(id);
    }

    public async Task<int> AddXtream(string name, string server, string username, string password)
    {
        var id = await Bg(() => _core.Call("add_xtream_source", new { name, server, username, password }).GetProperty("id").GetInt32());
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
            var file = await Download(location, "playlist");
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
            try { file = await Download(url, "guide"); }
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

    /// <summary>Stream a URL (or local file) to a temp file, gunzipping .gz content.</summary>
    private async Task<string> Download(string location, string name)
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
                resp = await _http.GetAsync(location, HttpCompletionOption.ResponseHeadersRead);
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

    public Task<List<ItemVm>> List(int source, string kind, string group) =>
        Bg(() => Items(_core.Call("list", new { source, kind, group, limit = 5000 })));

    public Task<List<ItemVm>> Search(string query, string? kind = null) =>
        string.IsNullOrWhiteSpace(query)
            ? Task.FromResult(new List<ItemVm>())
            : Bg(() => Items(_core.Call("search", new { query, kind, limit = 300 })));

    /// <summary>Total items of one kind in a source (for "677 channels" under the playlist name).</summary>
    public Task<int> CountKind(int source, string kind) => Bg(() =>
        _core.Call("groups", new { source, kind }).EnumerateArray().Sum(g => g.GetProperty("count").GetInt32()));

    public Task<List<ItemVm>> Favorites() => Bg(() => Items(_core.Call("favorites")));
    public Task<List<ItemVm>> Recent() => Bg(() => Items(_core.Call("recent", new { limit = 30 })));

    public Task<bool> ToggleFavorite(string id) => Bg(() =>
    {
        var on = !_core.Call("item", new { id }).GetProperty("favorite").GetBoolean();
        _core.Call("set_favorite", new { id, on });
        Save();
        return on;
    });

    /// <summary>Episodes of a series, fetched from the panel the first time.</summary>
    public async Task<List<ItemVm>> Episodes(ItemVm series)
    {
        var eps = await Bg(() => Items(_core.Call("episodes", new { id = series.Id })));
        if (eps.Count > 0) return eps;
        var url = await Bg(() => _core.Call("series_info_url", new { id = series.Id }).GetString() ?? "");
        var body = await _http.GetByteArrayAsync(url);
        return await Bg(() =>
        {
            _core.Import("series_info", body, new { id = series.Id });
            Save();
            return Items(_core.Call("episodes", new { id = series.Id }));
        });
    }

    public Task<Dictionary<string, (Programme? Now, Programme? Next)>> NowNext(IReadOnlyList<string> ids) => Bg(() =>
    {
        var result = new Dictionary<string, (Programme? Now, Programme? Next)>();
        if (ids.Count == 0) return result;
        var res = _core.Call("now_next_many", new { ids });
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

    private static Programme ParseProgramme(JsonElement p) =>
        new(Str(p, "title") ?? "", p.GetProperty("start").GetInt64(), p.GetProperty("stop").GetInt64(), Str(p, "desc") ?? "");

    /// <summary>Guide for one channel between two Unix times (for the EPG list under the video).</summary>
    public Task<List<Programme>> Guide(string id, long from, long to) => Bg(() =>
    {
        var res = _core.Call("guide", new { ids = new[] { id }, from, to });
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
