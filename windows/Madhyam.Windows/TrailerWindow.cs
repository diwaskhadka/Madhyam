using System.IO;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;

namespace Madhyam.Windows;

/// <summary>
/// YouTube trailers inside Madhyam's own player card (WebView2). The page is served from a
/// local https origin because YouTube refuses embeds that send no referrer.
/// </summary>
public static class TrailerPlayer
{
    private const string Host = "madhyam.local";

    /// <summary>Prepare a WebView2 once. Returns false when the Edge WebView2 runtime is missing.</summary>
    public static async Task<bool> Init(WebView2 web, Action<bool> fullscreenChanged)
    {
        if (web.CoreWebView2 != null) return true;
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, Path.Combine(App.DataDir, "webview2"));
            await web.EnsureCoreWebView2Async(env);
            var folder = Path.Combine(App.DataDir, "trailer");
            Directory.CreateDirectory(folder);
            await File.WriteAllTextAsync(Path.Combine(folder, "trailer.html"), Page);
            var core = web.CoreWebView2!;
            core.SetVirtualHostNameToFolderMapping(Host, folder, CoreWebView2HostResourceAccessKind.Allow);
            core.Settings.AreDevToolsEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.ContainsFullScreenElementChanged += (_, _) => fullscreenChanged(core.ContainsFullScreenElement);
            // Links inside the player (YouTube logo, "Watch on YouTube") open in the browser.
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                OpenInBrowser(e.Uri);
            };
            return true;
        }
        catch (Exception ex) when (ex is WebView2RuntimeNotFoundException or System.Runtime.InteropServices.COMException or InvalidOperationException)
        {
            Repository.Log($"WebView2 unavailable: {ex.Message}");
            return false;
        }
    }

    public static Uri Url(string youtubeId) => new($"https://{Host}/trailer.html?v={Uri.EscapeDataString(youtubeId)}");

    public static void OpenInBrowser(string url)
    {
        try { System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(url) { UseShellExecute = true }); }
        catch (Exception ex) { Repository.Log($"could not open {url}: {ex.Message}"); }
    }

    private const string Page = """
        <!doctype html>
        <html><head><meta charset="utf-8"><meta name="referrer" content="strict-origin-when-cross-origin">
        <style>html,body{margin:0;height:100%;background:#000;overflow:hidden}iframe{border:0;width:100%;height:100%}</style>
        </head><body>
        <iframe id="p" allow="autoplay; encrypted-media; picture-in-picture; fullscreen" allowfullscreen
                referrerpolicy="strict-origin-when-cross-origin"></iframe>
        <script>
          const id = new URLSearchParams(location.search).get('v') || '';
          document.getElementById('p').src = 'https://www.youtube.com/embed/' + encodeURIComponent(id) +
            '?autoplay=1&rel=0&playsinline=1&origin=' + encodeURIComponent(location.origin);
        </script>
        </body></html>
        """;
}
