using System.Windows;

namespace Madhyam.Windows;

public partial class AddSourceWindow : Window
{
    public AddSourceWindow()
    {
        InitializeComponent();
        Loaded += (_, _) => ServerBox.Focus();
    }

    public bool IsXtream => XtreamMode.IsChecked == true;
    public string SourceName => string.IsNullOrWhiteSpace(NameBox.Text) ? (IsXtream ? "Provider" : "Playlist") : NameBox.Text.Trim();
    public string Server => ServerBox.Text.Trim();
    public string Username => UserBox.Text.Trim();
    public string Password => PassBox.Password;
    public string PlaylistUrl => UrlBox.Text.Trim();

    private void Mode_Changed(object sender, RoutedEventArgs e)
    {
        // Fires during InitializeComponent before all named elements exist.
        if (XtreamFields == null || M3uFields == null) return;
        XtreamFields.Visibility = IsXtream ? Visibility.Visible : Visibility.Collapsed;
        M3uFields.Visibility = IsXtream ? Visibility.Collapsed : Visibility.Visible;
    }

    private void Browse_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new Microsoft.Win32.OpenFileDialog { Filter = "Playlists (*.m3u;*.m3u8)|*.m3u;*.m3u8|All files|*.*" };
        if (dlg.ShowDialog(this) == true) UrlBox.Text = dlg.FileName;
    }

    private void Ok_Click(object sender, RoutedEventArgs e)
    {
        Error.Text = IsXtream switch
        {
            true when Server.Length == 0 => "Enter the server address.",
            true when Username.Length == 0 => "Enter your username.",
            false when PlaylistUrl.Length == 0 => "Enter a playlist URL or choose a file.",
            _ => "",
        };
        if (Error.Text.Length == 0) DialogResult = true;
    }
}
