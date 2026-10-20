using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>One subtitle file offered by OpenSubtitles.</summary>
public sealed record SubtitleHit(long FileId, string Language, string Release, string FileName, int Downloads, bool HearingImpaired, string Title);

/// <summary>
/// Minimal client for the OpenSubtitles.com REST API (the service VLsub uses).
/// Needs a free API key from opensubtitles.com; logging in raises the daily download quota.
/// </summary>
public sealed class OpenSubtitlesClient : IDisposable
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(30) };
    private string _base = "https://api.opensubtitles.com/api/v1";
    private string? _token;
    private static AppSettings S => AppSettings.Current;

    public OpenSubtitlesClient()
    {
        _http.DefaultRequestHeaders.UserAgent.ParseAdd($"{Brand.Name} v{typeof(App).Assembly.GetName().Version?.ToString(3) ?? "0"}");
        _http.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
    }

    private HttpRequestMessage Req(HttpMethod m, string path, object? body = null)
    {
        var r = new HttpRequestMessage(m, _base + path);
        r.Headers.TryAddWithoutValidation("Api-Key", S.OsApiKey.Trim());
        if (_token != null) r.Headers.Authorization = new AuthenticationHeaderValue("Bearer", _token);
        if (body != null) r.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
        return r;
    }

    private async Task<JsonDocument> Send(HttpRequestMessage r)
    {
        using var resp = await _http.SendAsync(r);
        var text = await resp.Content.ReadAsStringAsync();
        if (!resp.IsSuccessStatusCode)
        {
            string msg = $"HTTP {(int)resp.StatusCode}";
            try
            {
                using var d = JsonDocument.Parse(text);
                if (d.RootElement.TryGetProperty("message", out var m)) msg = m.GetString() ?? msg;
            }
            catch (JsonException) { /* not JSON */ }
            throw new HttpRequestException(resp.StatusCode == System.Net.HttpStatusCode.Unauthorized || resp.StatusCode == System.Net.HttpStatusCode.Forbidden
                ? $"OpenSubtitles refused the request ({msg}). Check the API key and login in Settings → Playback."
                : $"OpenSubtitles: {msg}");
        }
        return JsonDocument.Parse(text);
    }

    private async Task EnsureLogin()
    {
        if (_token != null || S.OsUsername.Length == 0) return;
        var pass = Secret.Unprotect(S.OsPassword);
        if (pass.Length == 0) return;
        using var d = await Send(Req(HttpMethod.Post, "/login", new { username = S.OsUsername, password = pass }));
        _token = d.RootElement.TryGetProperty("token", out var t) ? t.GetString() : null;
        if (d.RootElement.TryGetProperty("base_url", out var b) && b.GetString() is { Length: > 0 } host)
            _base = $"https://{host}/api/v1";
    }

    public async Task<List<SubtitleHit>> Search(string query, string languages, int? season, int? episode)
    {
        await EnsureLogin();
        var q = new List<string> { $"query={Uri.EscapeDataString(query.ToLowerInvariant())}" };
        if (languages.Trim().Length > 0) q.Add($"languages={Uri.EscapeDataString(languages.Replace(" ", "").ToLowerInvariant())}");
        if (season is { } s) q.Add($"season_number={s}");
        if (episode is { } e) q.Add($"episode_number={e}");
        q.Sort(StringComparer.Ordinal); // the API redirects unless parameters are sorted
        using var d = await Send(Req(HttpMethod.Get, "/subtitles?" + string.Join("&", q)));
        var hits = new List<SubtitleHit>();
        if (!d.RootElement.TryGetProperty("data", out var data)) return hits;
        foreach (var x in data.EnumerateArray())
        {
            if (!x.TryGetProperty("attributes", out var a)) continue;
            var title = a.TryGetProperty("feature_details", out var f) && f.TryGetProperty("title", out var ft) ? ft.GetString() ?? "" : "";
            if (!a.TryGetProperty("files", out var files)) continue;
            foreach (var file in files.EnumerateArray())
            {
                hits.Add(new SubtitleHit(
                    file.GetProperty("file_id").GetInt64(),
                    a.TryGetProperty("language", out var l) ? l.GetString() ?? "" : "",
                    a.TryGetProperty("release", out var rel) ? rel.GetString() ?? "" : "",
                    file.TryGetProperty("file_name", out var fn) ? fn.GetString() ?? "" : "",
                    a.TryGetProperty("download_count", out var dc) && dc.TryGetInt32(out var n) ? n : 0,
                    a.TryGetProperty("hearing_impaired", out var hi) && hi.ValueKind == JsonValueKind.True,
                    title));
            }
        }
        return hits;
    }

    /// <summary>Download one subtitle file into the data folder; returns its path.</summary>
    public async Task<(string Path, string Remaining)> Download(SubtitleHit hit)
    {
        await EnsureLogin();
        using var d = await Send(Req(HttpMethod.Post, "/download", new { file_id = hit.FileId }));
        var link = d.RootElement.GetProperty("link").GetString() ?? throw new HttpRequestException("OpenSubtitles gave no download link.");
        var name = d.RootElement.TryGetProperty("file_name", out var fn) && fn.GetString() is { Length: > 0 } n ? n : $"{hit.FileId}.srt";
        var remaining = d.RootElement.TryGetProperty("remaining", out var r) ? r.ToString() : "?";
        var dir = Path.Combine(App.DataDir, "subtitles");
        Directory.CreateDirectory(dir);
        var safe = string.Concat(name.Select(c => Path.GetInvalidFileNameChars().Contains(c) ? '_' : c));
        var path = Path.Combine(dir, safe);
        await File.WriteAllBytesAsync(path, await _http.GetByteArrayAsync(link));
        return (path, remaining);
    }

    public void Dispose() => _http.Dispose();
}

/// <summary>Windows DPAPI: the OpenSubtitles password is stored encrypted for this Windows user only.</summary>
public static class Secret
{
    public static string Protect(string plain) =>
        plain.Length == 0 ? "" : Convert.ToBase64String(System.Security.Cryptography.ProtectedData.Protect(
            Encoding.UTF8.GetBytes(plain), null, System.Security.Cryptography.DataProtectionScope.CurrentUser));

    public static string Unprotect(string stored)
    {
        if (stored.Length == 0) return "";
        try
        {
            return Encoding.UTF8.GetString(System.Security.Cryptography.ProtectedData.Unprotect(
                Convert.FromBase64String(stored), null, System.Security.Cryptography.DataProtectionScope.CurrentUser));
        }
        catch
        {
            return "";
        }
    }
}

/// <summary>VLsub-like search dialog: title, season/episode, languages → list → download and use.</summary>
public static class SubtitleSearchWindow
{
    public static string? Show(Window owner, string query, int? season, int? episode)
    {
        var S = AppSettings.Current;
        if (S.OsApiKey.Trim().Length == 0)
        {
            var go = DialogKit.Confirm(owner, "Online subtitles need a free key",
                "Online subtitle search uses OpenSubtitles.com, which needs a free API key.\n\n" +
                "1. Create an account at opensubtitles.com\n2. Open “API consumers” in your profile and create a key\n" +
                "3. Paste it in Settings › Playback › Online subtitles",
                "Open OpenSubtitles", "Not now");
            if (go)
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("https://www.opensubtitles.com/en/consumers") { UseShellExecute = true });
            return null;
        }

        var w = DialogKit.Make(owner, "Search subtitles", 680);
        var root = new StackPanel { Margin = new Thickness(24) };
        root.Children.Add(Text("Search subtitles", 21, "Fg", FontWeights.SemiBold));
        var sub = Text("From OpenSubtitles.com. Pick one to download it and turn it on.", 13, "Muted");
        sub.Margin = new Thickness(0, 4, 0, 10);
        root.Children.Add(sub);

        var form = new Grid();
        form.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        form.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(70) });
        form.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(70) });
        form.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(110) });
        form.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        FrameworkElement Field(string label, Control box, int col)
        {
            var p = new StackPanel { Margin = new Thickness(col == 0 ? 0 : 8, 0, 0, 0) };
            p.Children.Add(DialogKit.Label(label));
            p.Children.Add(box);
            Grid.SetColumn(p, col);
            return p;
        }
        var title = new TextBox { Text = query };
        var seasonBox = new TextBox { Text = season?.ToString() ?? "" };
        var episodeBox = new TextBox { Text = episode?.ToString() ?? "" };
        var langs = new TextBox { Text = S.OsLanguages, ToolTip = "Comma-separated codes: en, fr, es, de, ar, hi, pt…" };
        form.Children.Add(Field("Title", title, 0));
        form.Children.Add(Field("Season", seasonBox, 1));
        form.Children.Add(Field("Episode", episodeBox, 2));
        form.Children.Add(Field("Languages", langs, 3));
        var searchBtn = DialogKit.Button("Search", true, () => { });
        searchBtn.VerticalAlignment = VerticalAlignment.Bottom;
        Grid.SetColumn(searchBtn, 4);
        form.Children.Add(searchBtn);
        root.Children.Add(form);

        var status = Text("", 12.5, "Muted", wrap: true);
        status.Margin = new Thickness(0, 12, 0, 6);
        root.Children.Add(status);

        var list = new ListBox
        {
            Height = 330,
            Style = St("PlainList"),
            ItemContainerStyle = St("RowItem"),
        };
        root.Children.Add(new Border { CornerRadius = new CornerRadius(12), Background = B("Panel"), Padding = new Thickness(6), Child = list });

        string? result = null;
        var client = new OpenSubtitlesClient();
        var use = DialogKit.Button("Download and use", true, () => { });
        use.IsEnabled = false;
        root.Children.Add(DialogKit.Footer(DialogKit.Button("Close", false, w.Close), use));

        async Task DoSearch()
        {
            S.OsLanguages = langs.Text.Trim();
            AppSettings.Save();
            status.Text = "Searching…";
            list.ItemsSource = null;
            use.IsEnabled = false;
            try
            {
                var hits = await client.Search(title.Text.Trim(),
                    langs.Text, int.TryParse(seasonBox.Text, out var s) ? s : null, int.TryParse(episodeBox.Text, out var e) ? e : null);
                status.Text = hits.Count == 0 ? "No subtitles found. Try a shorter title or another language." : $"{hits.Count} results, most downloaded first.";
                list.ItemsSource = hits.OrderByDescending(h => h.Downloads).Select(Row).ToList();
            }
            catch (Exception ex)
            {
                status.Text = MainWindow.Explain(ex);
            }
        }

        FrameworkElement Row(SubtitleHit h)
        {
            var g = new Grid { Tag = h, Margin = new Thickness(4, 2, 4, 2) };
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(36) });
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var lang = Badge(h.Language.ToUpperInvariant(), "Accent");
            lang.VerticalAlignment = VerticalAlignment.Center;
            lang.HorizontalAlignment = HorizontalAlignment.Left;
            g.Children.Add(lang);
            var t = new StackPanel();
            t.Children.Add(Text(h.Release.Length > 0 ? h.Release : h.FileName, 13, "Fg", FontWeights.SemiBold));
            t.Children.Add(Text(h.Title + (h.HearingImpaired ? "  ·  hearing impaired" : ""), 11.5, "Muted"));
            Grid.SetColumn(t, 1);
            g.Children.Add(t);
            var dl = Text($"↓ {h.Downloads:N0}", 12, "Muted");
            dl.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(dl, 2);
            g.Children.Add(dl);
            return g;
        }

        async Task DoUse()
        {
            if ((list.SelectedItem as FrameworkElement)?.Tag is not SubtitleHit h) return;
            status.Text = "Downloading…";
            try
            {
                var (path, remaining) = await client.Download(h);
                result = path;
                status.Text = $"Downloaded. Downloads left today: {remaining}.";
                w.Close();
            }
            catch (Exception ex)
            {
                status.Text = MainWindow.Explain(ex);
            }
        }

        searchBtn.Click += async (_, _) => await DoSearch();
        title.KeyDown += async (_, e) => { if (e.Key == Key.Enter) await DoSearch(); };
        list.SelectionChanged += (_, _) => use.IsEnabled = list.SelectedItem != null;
        list.MouseDoubleClick += async (_, _) => await DoUse();
        use.Click += async (_, _) => await DoUse();
        w.Content = root;
        w.Loaded += async (_, _) => { if (title.Text.Length > 0) await DoSearch(); };
        w.Closed += (_, _) => client.Dispose();
        w.ShowDialog();
        return result;
    }
}
