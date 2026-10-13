using System.IO;
using System.Windows;
using System.Windows.Controls;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// YouTube trailer inside Madhyam: a normal resizable window (minimize, maximize,
/// close) with the YouTube player in WebView2. The page is served from a local
/// https origin because YouTube refuses embeds that send no referrer.
/// </summary>
public sealed class TrailerWindow : Window
{
    private static TrailerWindow? _current;
    private readonly WebView2 _web = new();
    private readonly string _id;
    private readonly TextBlock _status;
    private WindowState _beforeFullscreen;

    public static void Open(string youtubeId, string title)
    {
        if (_current != null)
        {
            _current.Close();
            _current = null;
        }
        _current = new TrailerWindow(youtubeId, title);
        _current.Closed += (_, _) => _current = null;
        _current.Show();
    }

    private TrailerWindow(string youtubeId, string title)
    {
        _id = youtubeId;
        Title = $"{title} · Trailer";
        Width = 1040;
        Height = 640;
        MinWidth = 420;
        MinHeight = 280;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = System.Windows.Media.Brushes.Black;
        FontFamily = (System.Windows.Media.FontFamily)Application.Current.FindResource("UiFont");
        if (Application.Current.MainWindow?.Icon is { } icon) Icon = icon;

        var grid = new Grid();
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        _status = new TextBlock { Text = "Loading trailer…", Foreground = B("Muted"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        grid.Children.Add(_status);
        grid.Children.Add(_web);

        var bar = new DockPanel { Background = B("Panel"), LastChildFill = false };
        Grid.SetRow(bar, 1);
        var label = Text(title, 13, "Fg", FontWeights.SemiBold);
        label.Margin = new Thickness(14, 0, 0, 0);
        label.VerticalAlignment = VerticalAlignment.Center;
        bar.Children.Add(label);
        var close = IconButton("", "Close", Close, 36);
        var max = IconButton("", "Maximize / restore", () => WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized, 36);
        var min = IconButton("", "Minimize", () => WindowState = WindowState.Minimized, 36);
        var yt = IconButton("", "Open on YouTube", () => OpenInBrowser(_id), 36);
        var top = IconButton("", "Keep on top", () => Topmost = !Topmost, 36);
        foreach (var b in new[] { close, max, min, top, yt })
        {
            DockPanel.SetDock(b, Dock.Right);
            bar.Children.Add(b);
        }
        grid.Children.Add(bar);
        Content = grid;

        Loaded += async (_, _) => await Start();
        Closed += (_, _) => _web.Dispose();
    }

    private static void OpenInBrowser(string id)
    {
        try { System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo($"https://www.youtube.com/watch?v={id}") { UseShellExecute = true }); }
        catch (Exception ex) { Repository.Log($"could not open trailer: {ex.Message}"); }
    }

    private async Task Start()
    {
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, Path.Combine(App.DataDir, "webview2"));
            await _web.EnsureCoreWebView2Async(env);
            var folder = Path.Combine(App.DataDir, "trailer");
            Directory.CreateDirectory(folder);
            await File.WriteAllTextAsync(Path.Combine(folder, "trailer.html"), Page);
            var core = _web.CoreWebView2;
            core.SetVirtualHostNameToFolderMapping("madhyam.local", folder, CoreWebView2HostResourceAccessKind.Allow);
            core.Settings.AreDevToolsEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.ContainsFullScreenElementChanged += (_, _) => ToggleFullscreen(core.ContainsFullScreenElement);
            // Links inside the player (YouTube logo, "Watch on YouTube") open in the browser.
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                try { System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(e.Uri) { UseShellExecute = true }); } catch { }
            };
            core.Navigate($"https://madhyam.local/trailer.html?v={Uri.EscapeDataString(_id)}");
        }
        catch (Exception ex) when (ex is WebView2RuntimeNotFoundException or System.Runtime.InteropServices.COMException)
        {
            Repository.Log($"WebView2 unavailable: {ex.Message}");
            _status.Text = "The Microsoft Edge WebView2 runtime is missing, so the trailer opens in your browser.";
            OpenInBrowser(_id);
        }
    }

    private void ToggleFullscreen(bool on)
    {
        if (on)
        {
            _beforeFullscreen = WindowState;
            WindowStyle = WindowStyle.None;
            WindowState = WindowState.Normal;
            WindowState = WindowState.Maximized;
        }
        else
        {
            WindowStyle = WindowStyle.SingleBorderWindow;
            WindowState = _beforeFullscreen;
        }
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
