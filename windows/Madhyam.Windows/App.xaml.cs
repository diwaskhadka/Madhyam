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
        // Loads libvlc.dll from the libvlc\win-x64 folder next to the exe.
        LibVLCSharp.Shared.Core.Initialize();

        DispatcherUnhandledException += (_, args) =>
        {
            File.AppendAllText(Path.Combine(DataDir, "error.log"), $"{DateTime.Now:u} {args.Exception}\n\n");
            MessageBox.Show(args.Exception.Message, "Madhyam error", MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true; // keep the app alive; errors are logged
        };
        base.OnStartup(e);
    }
}
