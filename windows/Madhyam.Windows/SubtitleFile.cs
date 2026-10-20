using System.Globalization;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace Madhyam.Windows;

/// <summary>One piece of a subtitle line: its text and whether it is italic / bold / underlined.</summary>
public readonly record struct SubSpan(string Text, bool Italic, bool Bold, bool Underline);

/// <summary>One caption: shown from <see cref="StartMs"/> to <see cref="EndMs"/>, one or more lines.</summary>
public sealed record SubCue(long StartMs, long EndMs, IReadOnlyList<IReadOnlyList<SubSpan>> Lines);

/// <summary>
/// Text subtitle files (SRT, WebVTT, ASS / SSA) read by the app itself, so it can draw them
/// over the video in the user's style and change that style without touching playback.
/// Picture-based formats (VobSub .sub/.idx) and MicroDVD are left to VLC.
/// </summary>
public sealed class SubtitleFile
{
    public string Name { get; }
    public IReadOnlyList<SubCue> Cues { get; }
    private readonly long[] _starts;

    private SubtitleFile(string name, List<SubCue> cues)
    {
        Name = name;
        cues.Sort((a, b) => a.StartMs.CompareTo(b.StartMs));
        Cues = cues;
        _starts = cues.Select(c => c.StartMs).ToArray();
    }

    /// <summary>The file's captions, or null when it is not a text format the app can read.</summary>
    public static SubtitleFile? TryLoad(string path)
    {
        try
        {
            var text = ReadText(path);
            var ext = Path.GetExtension(path).ToLowerInvariant();
            var trimmed = text.TrimStart();
            List<SubCue>? cues = ext switch
            {
                ".ass" or ".ssa" => ParseAss(text),
                ".vtt" => ParseSrt(text),
                ".srt" => ParseSrt(text),
                // Some downloads have no useful extension: go by the content.
                _ when trimmed.StartsWith("[Script Info]", StringComparison.OrdinalIgnoreCase) => ParseAss(text),
                _ when TimeLine.IsMatch(text) => ParseSrt(text),
                _ => null,
            };
            return cues is { Count: > 0 } ? new SubtitleFile(Path.GetFileName(path), cues) : null;
        }
        catch (Exception ex)
        {
            Repository.Log($"subtitle file not read ({Path.GetFileName(path)}): {ex.Message}");
            return null;
        }
    }

    /// <summary>The captions showing at <paramref name="ms"/> (several may overlap).</summary>
    public List<SubCue> At(long ms)
    {
        var found = new List<SubCue>();
        // Last cue starting at or before ms; overlapping cues are close before it.
        var i = Array.BinarySearch(_starts, ms);
        if (i < 0) i = ~i - 1;
        for (var j = i; j >= 0 && j > i - 12; j--)
        {
            var c = Cues[j];
            if (c.StartMs <= ms && ms < c.EndMs) found.Insert(0, c);
        }
        return found;
    }

    // ================================================================ reading

    static SubtitleFile() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

    /// <summary>UTF-8 (with or without BOM) or UTF-16 by its BOM, else the system's ANSI code page.</summary>
    private static string ReadText(string path)
    {
        var bytes = File.ReadAllBytes(path);
        if (bytes.Length >= 2 && (bytes[0] == 0xFF && bytes[1] == 0xFE || bytes[0] == 0xFE && bytes[1] == 0xFF))
            return new StreamReader(new MemoryStream(bytes), true).ReadToEnd();
        try
        {
            return new UTF8Encoding(false, true).GetString(bytes).TrimStart('﻿');
        }
        catch (DecoderFallbackException)
        {
            return Encoding.GetEncoding(CultureInfo.CurrentCulture.TextInfo.ANSICodePage).GetString(bytes);
        }
    }

    // 00:01:02,345 --> 00:01:04,000   (SRT)   or   01:02.345 --> 01:04.000 line:90%  (WebVTT)
    private static readonly Regex TimeLine = new(
        @"^\s*((?:\d+:)?\d{1,2}:\d{1,2}[,.]\d{1,3})\s*-->\s*((?:\d+:)?\d{1,2}:\d{1,2}[,.]\d{1,3})", RegexOptions.Multiline);

    private static long Ms(string t)
    {
        var parts = t.Replace(',', '.').Split(':');
        double seconds = 0;
        foreach (var p in parts) seconds = seconds * 60 + double.Parse(p, CultureInfo.InvariantCulture);
        return (long)Math.Round(seconds * 1000);
    }

    /// <summary>SRT and WebVTT: a time line, then the text up to the next blank line.</summary>
    private static List<SubCue> ParseSrt(string text)
    {
        var cues = new List<SubCue>();
        var lines = text.Replace("\r\n", "\n").Replace('\r', '\n').Split('\n');
        for (var i = 0; i < lines.Length; i++)
        {
            var m = TimeLine.Match(lines[i]);
            if (!m.Success) continue;
            var body = new List<string>();
            for (i++; i < lines.Length && lines[i].Trim().Length > 0 && !TimeLine.IsMatch(lines[i]); i++) body.Add(lines[i]);
            // A lone number before the next time line is that cue's index, not text.
            if (body.Count > 0 && i < lines.Length && TimeLine.IsMatch(lines[i]) && int.TryParse(body[^1].Trim(), out _)) body.RemoveAt(body.Count - 1);
            i--;
            var parsed = body.Select(Markup).Where(l => l.Count > 0).ToList();
            if (parsed.Count > 0) cues.Add(new SubCue(Ms(m.Groups[1].Value), Ms(m.Groups[2].Value), parsed));
        }
        return cues;
    }

    private static readonly Regex Tag = new(@"<\s*(/?)\s*([a-zA-Z]+)[^>]*>|<[\d:.]+>|\{\\([^}]*)\}|\{[^}]*\}");

    /// <summary>One line of text with &lt;i&gt; &lt;b&gt; &lt;u&gt; (and ASS {\i1}-style) tags, other tags dropped.</summary>
    private static List<SubSpan> Markup(string line)
    {
        var spans = new List<SubSpan>();
        bool italic = false, bold = false, underline = false;
        var at = 0;
        void Add(string s)
        {
            s = System.Net.WebUtility.HtmlDecode(s);
            if (s.Length > 0) spans.Add(new SubSpan(s, italic, bold, underline));
        }
        foreach (Match m in Tag.Matches(line))
        {
            Add(line[at..m.Index]);
            at = m.Index + m.Length;
            if (m.Groups[2].Success)
            {
                var on = m.Groups[1].Value.Length == 0;
                switch (m.Groups[2].Value.ToLowerInvariant())
                {
                    case "i": italic = on; break;
                    case "b": bold = on; break;
                    case "u": underline = on; break;
                }
            }
            else if (m.Groups[3].Success)
            {
                foreach (var cmd in m.Groups[3].Value.Split('\\'))
                {
                    if (cmd is "i1") italic = true;
                    else if (cmd is "i0" or "i") italic = false;
                    else if (cmd is "b1") bold = true;
                    else if (cmd is "b0" or "b") bold = false;
                    else if (cmd is "u1") underline = true;
                    else if (cmd is "u0" or "u") underline = false;
                    else if (cmd is "r") italic = bold = underline = false;
                }
            }
        }
        Add(line[at..]);
        // Drop a line that is only spaces.
        return spans.Any(s => s.Text.Trim().Length > 0) ? spans : new List<SubSpan>();
    }

    /// <summary>ASS / SSA: the [Events] section's Dialogue lines, by the order its Format line gives.</summary>
    private static List<SubCue> ParseAss(string text)
    {
        var cues = new List<SubCue>();
        var inEvents = false;
        string[] format = { "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text" };
        foreach (var raw in text.Replace("\r\n", "\n").Split('\n'))
        {
            var line = raw.Trim();
            if (line.StartsWith('['))
            {
                inEvents = line.Equals("[Events]", StringComparison.OrdinalIgnoreCase);
                continue;
            }
            if (!inEvents) continue;
            if (line.StartsWith("Format:", StringComparison.OrdinalIgnoreCase))
            {
                format = line[7..].Split(',').Select(f => f.Trim().ToLowerInvariant()).ToArray();
                continue;
            }
            if (!line.StartsWith("Dialogue:", StringComparison.OrdinalIgnoreCase)) continue;
            var fields = line[9..].Split(',', format.Length);
            if (fields.Length < format.Length) continue;
            var start = Array.IndexOf(format, "start");
            var end = Array.IndexOf(format, "end");
            var body = Array.IndexOf(format, "text");
            if (start < 0 || end < 0 || body < 0) continue;
            // Drawing commands ({\p1}…) are shapes, not text.
            if (fields[body].Contains(@"\p1")) continue;
            var lines = fields[body].Replace(@"\N", "\n").Replace(@"\n", "\n").Replace(@"\h", " ")
                .Split('\n').Select(Markup).Where(l => l.Count > 0).ToList();
            if (lines.Count > 0) cues.Add(new SubCue(Ms(fields[start].Trim()), Ms(fields[end].Trim()), lines));
        }
        return cues;
    }
}
