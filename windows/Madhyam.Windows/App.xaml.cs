using System.IO;
using System.Windows;

namespace Madhyam.Windows;

public partial class App : Application
{
    public static string DataDir { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Madhyam");

    protected override void OnStartup(StartupEventArgs e)
    {
        Directory.CreateDirectory(DataDir);
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
            MessageBox.Show($"The video engine (LibVLC) could not be loaded from:\n{vlcDir}\n\n{ex.Message}", "Madhyam",
                MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
            return;
        }

        DispatcherUnhandledException += (_, args) =>
        {
            File.AppendAllText(Path.Combine(DataDir, "error.log"), $"{DateTime.Now:u} {args.Exception}\n\n");
            MessageBox.Show(args.Exception.Message, "Madhyam error", MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true; // keep the app alive; errors are logged
        };
        base.OnStartup(e);
    }
}
