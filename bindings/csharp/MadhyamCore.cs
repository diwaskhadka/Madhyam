// C# binding for Windows (WinUI 3, WPF, Avalonia, MAUI) via P/Invoke.
//
// NOT compiled in the sandbox this was written in (no .NET SDK) — the C ABI it
// calls is tested end-to-end by bindings/python/e2e_test.py.
//
// Ship madhyam_core_ffi.dll (from `cargo build --release --target x86_64-pc-windows-msvc`)
// next to your .exe, or under runtimes/win-x64/native/ in a NuGet package.

using System;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;

namespace MadhyamCoreNet;

public sealed class IptvException(string code, string message) : Exception($"{code}: {message}")
{
    public string Code { get; } = code;
}

internal static partial class Native
{
    private const string Lib = "madhyam_core_ffi";

    [LibraryImport(Lib)] internal static partial IntPtr iptv_core_new();
    [LibraryImport(Lib)] internal static partial void iptv_core_free(IntPtr core);
    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr iptv_call(IntPtr core, string method, string? args);
    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr iptv_import(IntPtr core, string method, byte[] data, nuint len, string? args);
    [LibraryImport(Lib)] internal static partial void iptv_string_free(IntPtr s);
    [LibraryImport(Lib)] internal static partial IntPtr iptv_version();
}

/// <summary>Owns one catalog. Calls block: wrap them in Task.Run from UI code.</summary>
public sealed class MadhyamCore : IDisposable
{
    private IntPtr _handle = Native.iptv_core_new();

    public static string Version => Marshal.PtrToStringUTF8(Native.iptv_version()) ?? "";

    private static JsonElement Unwrap(IntPtr p)
    {
        string text;
        try { text = Marshal.PtrToStringUTF8(p) ?? "{}"; }
        finally { Native.iptv_string_free(p); }
        using var doc = JsonDocument.Parse(text);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var err))
            throw new IptvException(err.GetProperty("code").GetString()!, err.GetProperty("message").GetString()!);
        return root.GetProperty("ok").Clone();
    }

    private IntPtr Handle => _handle != IntPtr.Zero ? _handle : throw new ObjectDisposedException(nameof(MadhyamCore));

    public JsonElement Call(string method, object? args = null) =>
        Unwrap(Native.iptv_call(Handle, method, args is null ? null : JsonSerializer.Serialize(args)));

    public JsonElement Import(string method, byte[] data, object? args = null) =>
        Unwrap(Native.iptv_import(Handle, method, data, (nuint)data.Length, args is null ? null : JsonSerializer.Serialize(args)));

    /// <summary>Full Xtream refresh: the core lists URLs, HttpClient fetches them.</summary>
    public async Task<JsonElement> SyncXtreamAsync(int source, HttpClient http)
    {
        var requests = await Task.Run(() => Call("xtream_requests", new { source }));
        foreach (var r in requests.EnumerateArray())
        {
            var body = await http.GetByteArrayAsync(r.GetProperty("url").GetString());
            var part = r.GetProperty("part").GetString();
            await Task.Run(() => Import("xtream_part", body, new { source, part }));
        }
        return await Task.Run(() => Call("xtream_commit", new { source }));
    }

    public void Dispose()
    {
        if (_handle != IntPtr.Zero) { Native.iptv_core_free(_handle); _handle = IntPtr.Zero; }
    }
}
