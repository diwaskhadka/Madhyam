using System.IO;
using System.Windows;

namespace Madhyam.Windows;

public partial class App : Application
{
    // Fixed name from 0.9.0 on: changing it would lose everyone's playlists and settings (not Brand.Name).
    public static string DataDir { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Madhyam");

    // Where Madhyam 0.4.x kept its data; moved to DataDir on the first start of 0.9.0.
    private static string OldDataDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Madhyam");

    protected override void OnStartup(StartupEventArgs e)
    {
        MoveOldDataDir(OldDataDir, DataDir);
        Directory.CreateDirectory(DataDir);
        AppSettings.Load();
        AppSettings.ApplyTheme(Resources);   // must run before any window or style is used
        AppSettings.ApplyLayout(Resources);
        // Point LibVLC at the libvlc\win-x64 folder next to the exe explicitly:
        // in a single-file app, assembly locations are empty, so auto-discovery fails.
        var vlcDir = Path.Combine(AppContext.BaseDirectory, "libvlc", "win-x64");
        try
        {
            LibVLCSharp.Shared.Core.Initialize(vlcDir);
        }
        catch (Exception ex)
        {
            File.AppendAllText(Path.Combine(DataDir, "error.log"),
                $"{DateTime.Now:u} LibVLC failed to load from {vlcDir} (exists: {Directory.Exists(vlcDir)}): {ex}\n\n");
            MessageBox.Show($"The video engine (LibVLC) could not be loaded from:\n{vlcDir}\n\n{ex.Message}", Brand.Name,
                MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
            return;
        }

        // One dialog at a time, and the same error only once a minute: a layout error repeats on
        // every frame and used to stack dozens of dialogs.
        var showing = false;
        var lastText = "";
        var lastAt = DateTime.MinValue;
        var lastLogged = "";
        var lastLoggedAt = DateTime.MinValue;
        DispatcherUnhandledException += (_, args) =>
        {
            args.Handled = true; // keep the app alive; errors are logged
            var text0 = args.Exception.Message;
            try
            {
                // Keep the log small (a repeating layout error once grew it to 100 MB).
                var log = Path.Combine(DataDir, "error.log");
                if (File.Exists(log) && new FileInfo(log).Length > 2_000_000) File.Move(log, log + ".old", overwrite: true);
                if (text0 != lastLogged || DateTime.Now - lastLoggedAt > TimeSpan.FromSeconds(30))
                {
                    File.AppendAllText(log, $"{DateTime.Now:u} {args.Exception}\n\n");
                    lastLogged = text0;
                    lastLoggedAt = DateTime.Now;
                }
            }
            catch { }
            var text = args.Exception.Message;
            if (showing || (text == lastText && DateTime.Now - lastAt < TimeSpan.FromMinutes(1))) return;
            showing = true;
            lastText = text;
            lastAt = DateTime.Now;
            try { MessageBox.Show(text, $"{Brand.Name} error", MessageBoxButton.OK, MessageBoxImage.Error); }
            finally { showing = false; }
        };
        Motion.EnablePressFeedback();
        base.OnStartup(e);
    }

    /// <summary>
    /// The app was called Madhyam before 0.9.0. Bring its playlists, settings, favorites and
    /// progress over once: move the folder (same drive, instant), or copy it when the move fails
    /// (e.g. a file is still open), leaving the old folder in place. Encrypted passwords stay valid,
    /// since they are tied to the Windows user, not the folder.
    /// </summary>
    internal static void MoveOldDataDir(string oldDir, string newDir)
    {
        try
        {
            if (!Directory.Exists(oldDir)) return;
            if (Directory.Exists(newDir))
            {
                if (Directory.EnumerateFileSystemEntries(newDir).Any()) return;   // already in use
                Directory.Delete(newDir);
            }
            try
            {
                Directory.Move(oldDir, newDir);
            }
            catch (IOException)
            {
                // Copy beside it first, so a failed copy never leaves a half-filled data folder.
                var tmp = newDir + ".copying";
                if (Directory.Exists(tmp)) Directory.Delete(tmp, true);
                CopyDir(oldDir, tmp);
                Directory.Move(tmp, newDir);
            }
        }
        catch (Exception ex)
        {
            // Start with a fresh folder rather than not at all; the old data stays where it was.
            try
            {
                Directory.CreateDirectory(newDir);
                File.AppendAllText(Path.Combine(newDir, "error.log"),
                    $"{DateTime.Now:u} could not bring over {oldDir}: {ex}\n\n");
            }
            catch { }
        }
    }

    private static void CopyDir(string from, string to)
    {
        Directory.CreateDirectory(to);
        foreach (var f in Directory.EnumerateFiles(from)) File.Copy(f, Path.Combine(to, Path.GetFileName(f)), true);
        foreach (var d in Directory.EnumerateDirectories(from)) CopyDir(d, Path.Combine(to, Path.GetFileName(d)));
    }

    /// <summary>Start a fresh copy of the app and quit this one (used after theme changes / reset).</summary>
    public static void Restart()
    {
        if (Environment.ProcessPath is { } exe) System.Diagnostics.Process.Start(exe);
        Current.Shutdown();
    }
}
