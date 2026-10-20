using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>Where a playlist load is: the step, and a detail such as the size downloaded so far.</summary>
/// <remarks>A <see cref="Final"/> status is a finished step's result (such as how many movies came in), shown on its line after the tick.</remarks>
public sealed record LoadStatus(LoadStatus.Step At, string Detail, bool Final = false)
{
    public enum Step { SignIn, Live, Movies, Series, Download, Read, Save, Guide, Pages }

    public static string Size(long bytes) => bytes switch
    {
        >= 1 << 30 => $"{bytes / (double)(1 << 30):0.0} GB",
        >= 1 << 20 => $"{bytes / (double)(1 << 20):0.0} MB",
        _ => $"{Math.Max(1, bytes / 1024)} KB",
    };

    public static readonly (Step Step, string Label)[] XtreamSteps =
    {
        (Step.SignIn, "Signing in"), (Step.Live, "Live channels"), (Step.Movies, "Movies"), (Step.Series, "Series"),
        (Step.Save, "Saving the catalog"), (Step.Guide, "Programme guide"), (Step.Pages, "Getting the pages ready"),
    };

    public static readonly (Step Step, string Label)[] M3uSteps =
    {
        (Step.Download, "Downloading the playlist"), (Step.Read, "Reading the playlist"),
        (Step.Live, "Live channels"), (Step.Movies, "Movies"), (Step.Series, "Series"), (Step.Save, "Saving the catalog"),
        (Step.Guide, "Programme guide"), (Step.Pages, "Getting the pages ready"),
    };
}

/// <summary>
/// Pop-up shown while a playlist is added: its steps with a tick as each finishes, what is
/// happening now (with the amount downloaded) and an overall bar, so nobody waits at a blank
/// screen. "Hide" lets the load go on in the background (the status bar still shows it).
/// </summary>
public sealed class LoadingWindow : IProgress<LoadStatus>
{
    private readonly Window _w;
    private readonly (LoadStatus.Step Step, string Label)[] _steps;
    private readonly List<(TextBlock Mark, TextBlock Label, TextBlock Detail)> _rows = new();
    private readonly ProgressBar _bar;
    private readonly TextBlock _title;
    private readonly TextBlock _sub;
    private readonly TextBlock _error;
    private readonly Button _button;
    private readonly DispatcherTimer _creep;
    private readonly DateTime _started = DateTime.Now;
    private int _current = -1;
    private DateTime _stepStarted = DateTime.Now;
    private bool _finished;

    private LoadingWindow(Window owner, string name, (LoadStatus.Step, string)[] steps)
    {
        _steps = steps;
        _w = DialogKit.Make(owner, "Adding playlist", 440);
        _w.WindowStyle = WindowStyle.ToolWindow;
        var root = new StackPanel { Margin = new Thickness(26, 22, 26, 18) };
        _title = Text($"Adding “{name}”", 19, "Fg", FontWeights.SemiBold);
        _title.TextTrimming = TextTrimming.CharacterEllipsis;
        root.Children.Add(_title);
        var sub = _sub = Text("This can take a minute for a large playlist.", 13, "Muted", wrap: true);
        sub.Margin = new Thickness(0, 4, 0, 16);
        root.Children.Add(sub);
        foreach (var (_, label) in steps)
        {
            var row = new Grid { Margin = new Thickness(0, 0, 0, 9) };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(26) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var mark = new TextBlock { Text = "○", FontSize = 13, Foreground = B("Muted"), VerticalAlignment = VerticalAlignment.Center };
            var text = Text(label, 14, "Muted");
            var detail = Text("", 12.5, "Muted");
            detail.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(text, 1);
            Grid.SetColumn(detail, 2);
            row.Children.Add(mark);
            row.Children.Add(text);
            row.Children.Add(detail);
            root.Children.Add(row);
            _rows.Add((mark, text, detail));
        }
        _bar = new ProgressBar { Style = St("ThinProgress"), Margin = new Thickness(0, 8, 0, 0) };
        root.Children.Add(_bar);
        _error = Text("", 13, "Fg", wrap: true);
        _error.Foreground = new SolidColorBrush(Color.FromRgb(0xFF, 0x7A, 0x7A));
        _error.Margin = new Thickness(0, 12, 0, 0);
        _error.Visibility = Visibility.Collapsed;
        root.Children.Add(_error);
        _button = DialogKit.Button("Hide", false, () => _w.Hide());
        _button.ToolTip = "Keep loading in the background";
        var footer = DialogKit.Footer(_button);
        footer.Margin = new Thickness(0, 16, 0, 0);
        root.Children.Add(footer);
        _w.Content = root;
        // Esc hides too (DialogKit closes on Esc; closing early just hides it).
        _w.Closing += (_, e) =>
        {
            if (_finished) return;
            e.Cancel = true;
            _w.Hide();
        };
        // Without a known size, a step's share of the bar creeps forward so it never looks stuck.
        _creep = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(120) };
        _creep.Tick += (_, _) => MoveBar();
    }

    public static LoadingWindow Show(Window owner, string name, bool xtream)
    {
        var w = new LoadingWindow(owner, name, xtream ? LoadStatus.XtreamSteps : LoadStatus.M3uSteps);
        w._w.Show();
        w.Report(new LoadStatus(w._steps[0].Step, ""));
        w._creep.Start();
        return w;
    }

    public void Report(LoadStatus value)
    {
        if (!_w.Dispatcher.CheckAccess())
        {
            _w.Dispatcher.BeginInvoke(() => Report(value));
            return;
        }
        if (_finished) return;
        var i = Array.FindIndex(_steps, s => s.Step == value.At);
        if (i < 0) return;
        if (i > _current)
        {
            for (var j = Math.Max(0, _current); j < i; j++) Mark(j, done: true);
            _current = i;
            _stepStarted = DateTime.Now;
            Mark(i, done: false);
        }
        if (value.Final)
        {
            // A result: tick the line (the next step then starts from here) and keep the text.
            if (i == _current) Mark(i, done: true);
            var row = _rows[i];
            row.Detail.Text = value.Detail;
            row.Detail.Foreground = B(value.Detail == "None" ? "Muted" : "Fg");
            if (value.Detail == "None") row.Label.Foreground = B("Muted");
            return;
        }
        if (i == _current) _rows[i].Detail.Text = value.Detail;
    }

    private void Mark(int i, bool done)
    {
        var (mark, label, _) = _rows[i];
        if (done && mark.Text == "✓") return;
        mark.Text = done ? "✓" : "●";
        mark.Foreground = B("Accent");
        label.Foreground = B("Fg");
        label.FontWeight = done ? FontWeights.Normal : FontWeights.SemiBold;
        if (!done)
        {
            var pulse = new DoubleAnimation(1, 0.35, TimeSpan.FromMilliseconds(650)) { AutoReverse = true, RepeatBehavior = RepeatBehavior.Forever };
            mark.BeginAnimation(UIElement.OpacityProperty, pulse);
        }
        else
        {
            mark.BeginAnimation(UIElement.OpacityProperty, null);
            mark.Opacity = 1;
        }
    }

    private void MoveBar()
    {
        if (_current < 0) return;
        var seconds = (DateTime.Now - _stepStarted).TotalSeconds;
        var within = 0.9 * (1 - Math.Exp(-seconds / 6));
        _bar.Value = Math.Max(_bar.Value, (_current + within) / _steps.Length);
    }

    /// <summary>All done: tick every step, then close after a moment.</summary>
    public async Task Done()
    {
        Report(new LoadStatus(_steps[^1].Step, ""));
        _finished = true;
        _creep.Stop();
        for (var j = 0; j < _steps.Length; j++) Mark(j, done: true);
        _bar.Value = 1;
        var took = (DateTime.Now - _started).TotalSeconds;
        _title.Text = took >= 1 ? $"Ready in {took:0} s" : "Ready";
        _sub.Text = "Here's what the playlist has.";
        _button.Content = "Done";
        _button.ToolTip = null;
        _button.Click += (_, _) => _w.Close();
        // Long enough to read the counts; stays while the pointer is on it.
        await Task.Delay(4000);
        while (_w.IsVisible && _w.IsMouseOver) await Task.Delay(500);
        _w.Close();
    }

    /// <summary>It failed: say why on the pop-up (shown again if hidden), with a Close button.</summary>
    public void Failed(string message)
    {
        _finished = true;
        _creep.Stop();
        if (_current >= 0)
        {
            var (mark, _, _) = _rows[_current];
            mark.BeginAnimation(UIElement.OpacityProperty, null);
            mark.Opacity = 1;
            mark.Text = "✕";
            mark.Foreground = _error.Foreground;
        }
        _title.Text = "The playlist could not be added";
        _sub.Visibility = Visibility.Collapsed;
        _error.Text = message;
        _error.Visibility = Visibility.Visible;
        _button.Content = "Close";
        _button.ToolTip = null;
        _button.Click += (_, _) => _w.Close();
        if (!_w.IsVisible) _w.Show();
    }
}
