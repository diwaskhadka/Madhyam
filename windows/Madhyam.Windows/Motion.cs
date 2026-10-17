using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;

namespace Madhyam.Windows;

/// <summary>
/// The app's own motion language (deliberately not a plain cross-fade):
/// pages "rise" a few pixels while fading in, menus "unfold" from their button,
/// and buttons give a short springy press.
/// </summary>
public static class Motion
{
    private static readonly IEasingFunction Out = new CubicEase { EasingMode = EasingMode.EaseOut };
    private static readonly IEasingFunction Spring = new BackEase { EasingMode = EasingMode.EaseOut, Amplitude = 0.45 };

    private static TranslateTransform Translate(UIElement e)
    {
        if (e.RenderTransform is TranslateTransform t && !t.IsFrozen) return t;
        t = new TranslateTransform();
        e.RenderTransform = t;
        return t;
    }

    /// <summary>Fade in while sliding up 16 px (page and content changes).</summary>
    public static void Rise(UIElement? e, double distance = 16, int ms = 260)
    {
        if (e == null) return;
        var t = Translate(e);
        var d = TimeSpan.FromMilliseconds(ms);
        e.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(0, 1, d) { EasingFunction = Out });
        t.BeginAnimation(TranslateTransform.YProperty, new DoubleAnimation(distance, 0, d) { EasingFunction = Out });
    }

    /// <summary>Menus grow from the edge next to their button with a slight overshoot.</summary>
    public static void Unfold(FrameworkElement e, bool fromBottom, bool fromLeft = false)
    {
        var s = new ScaleTransform(0.9, 0.82);
        e.RenderTransformOrigin = fromLeft ? new Point(0, 0.2) : new Point(0.5, fromBottom ? 1 : 0);
        e.RenderTransform = s;
        var d = TimeSpan.FromMilliseconds(210);
        s.BeginAnimation(ScaleTransform.ScaleXProperty, new DoubleAnimation(0.9, 1, d) { EasingFunction = Spring });
        s.BeginAnimation(ScaleTransform.ScaleYProperty, new DoubleAnimation(0.82, 1, d) { EasingFunction = Spring });
        e.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromMilliseconds(140)));
    }

    /// <summary>Press feedback for every button in the app (registered once at start-up).</summary>
    public static void EnablePressFeedback()
    {
        EventManager.RegisterClassHandler(typeof(ButtonBase), UIElement.PreviewMouseLeftButtonDownEvent,
            new MouseButtonEventHandler((s, _) => Press(s as ButtonBase, true)), handledEventsToo: true);
        EventManager.RegisterClassHandler(typeof(ButtonBase), UIElement.PreviewMouseLeftButtonUpEvent,
            new MouseButtonEventHandler((s, _) => Press(s as ButtonBase, false)), handledEventsToo: true);
        EventManager.RegisterClassHandler(typeof(ButtonBase), UIElement.MouseLeaveEvent,
            new MouseEventHandler((s, _) => Press(s as ButtonBase, false)), handledEventsToo: true);
    }

    private static Style? TryStyle(string key) => Application.Current.TryFindResource(key) as Style;

    private static void Press(ButtonBase? b, bool down)
    {
        // Slider tracks use repeat buttons and must not wobble; checkboxes and radios keep their own look.
        if (b == null || b is RepeatButton || b is ToggleButton) return;
        // Only icon buttons and picture cards: text never moves or scales.
        var style = b.Style;
        var iconish = style != null && (style == TryStyle("IconButton") || style == TryStyle("PlayerButton"));
        if (!iconish && !Equals(b.Tag, "press")) return;
        if (b.RenderTransform is not ScaleTransform s || s.IsFrozen)
        {
            if (b.RenderTransform != Transform.Identity && b.RenderTransform is not ScaleTransform) return; // owned by something else
            s = new ScaleTransform(1, 1);
            b.RenderTransform = s;
            b.RenderTransformOrigin = new Point(0.5, 0.5);
        }
        var to = down ? 0.94 : 1.0;
        var a = new DoubleAnimation(to, TimeSpan.FromMilliseconds(down ? 90 : 260)) { EasingFunction = down ? Out : Spring };
        s.BeginAnimation(ScaleTransform.ScaleXProperty, a);
        s.BeginAnimation(ScaleTransform.ScaleYProperty, a);
    }
}

/// <summary>
/// Smooth wheel scrolling with a rubber-band bounce at the top and bottom
/// (WPF has none built in). Works for ScrollViewers and ListBoxes.
/// </summary>
public static class ElasticScroll
{
    private sealed class State
    {
        public double Target;
        public double Stretch; // overscroll in px (positive = past the top, negative = past the bottom)
        public bool Running;
        public TranslateTransform? Shift;
    }

    private static readonly DependencyProperty StateProperty =
        DependencyProperty.RegisterAttached("ElasticState", typeof(State), typeof(ElasticScroll));

    /// <summary>Attach to a ListBox (its inner ScrollViewer is found once the template is applied).</summary>
    public static void Attach(ItemsControl list)
    {
        // Offsets must be pixels for smooth scrolling (item scrolling jumps whole rows).
        VirtualizingPanel.SetScrollUnit(list, ScrollUnit.Pixel);
        void Hook()
        {
            if (UiKit.FindChild<ScrollViewer>(list) is { } sv) Attach(sv);
        }
        if (list.IsLoaded) Hook();
        else list.Loaded += (_, _) => Hook();
    }

    public static void Attach(ScrollViewer sv)
    {
        if (sv.GetValue(StateProperty) != null) return;
        var st = new State();
        sv.SetValue(StateProperty, st);
        sv.PreviewMouseWheel += (_, e) =>
        {
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Shift)) return; // sideways scrolling elsewhere
            if (sv.VerticalScrollBarVisibility == ScrollBarVisibility.Disabled) return;
            e.Handled = true;
            if (!st.Running) st.Target = sv.VerticalOffset;
            st.Target -= e.Delta * 0.9;
            if (st.Target < 0)
            {
                st.Stretch = Math.Min(70, st.Stretch + (-st.Target) * 0.35);
                st.Target = 0;
            }
            else if (st.Target > sv.ScrollableHeight)
            {
                st.Stretch = Math.Max(-70, st.Stretch - (st.Target - sv.ScrollableHeight) * 0.35);
                st.Target = sv.ScrollableHeight;
            }
            Start(sv, st);
        };
    }

    private static void Start(ScrollViewer sv, State st)
    {
        if (st.Running) return;
        st.Running = true;
        if (st.Shift == null && sv.Content is UIElement content)
        {
            st.Shift = new TranslateTransform();
            content.RenderTransform = st.Shift;
        }
        EventHandler? tick = null;
        tick = (_, _) =>
        {
            var cur = sv.VerticalOffset;
            var diff = st.Target - cur;
            if (Math.Abs(diff) > 0.5) sv.ScrollToVerticalOffset(cur + diff * 0.22);
            st.Stretch *= 0.78; // spring back
            if (Math.Abs(st.Stretch) < 0.4) st.Stretch = 0;
            if (st.Shift != null) st.Shift.Y = st.Stretch;
            if (Math.Abs(diff) <= 0.5 && st.Stretch == 0)
            {
                st.Running = false;
                CompositionTarget.Rendering -= tick;
            }
        };
        CompositionTarget.Rendering += tick;
    }
}


/// <summary>Short black cover over the monitor while the window switches to / from full screen.</summary>
public sealed class FullscreenCurtain
{
    private readonly Window? _w;

    private FullscreenCurtain(Window? w) => _w = w;

    public static FullscreenCurtain Show(Window owner)
    {
        try
        {
            var src = PresentationSource.FromVisual(owner);
            var hwnd = new System.Windows.Interop.WindowInteropHelper(owner).Handle;
            var mon = NativeMethods.MonitorFromWindow(hwnd, 2);
            var info = new NativeMethods.MonitorInfo { Size = System.Runtime.InteropServices.Marshal.SizeOf<NativeMethods.MonitorInfo>() };
            if (mon == IntPtr.Zero || !NativeMethods.GetMonitorInfo(mon, ref info) || src == null) return new FullscreenCurtain(null);
            var dpi = VisualTreeHelper.GetDpi(owner);
            var r = info.Monitor;
            var w = new Window
            {
                WindowStyle = WindowStyle.None,
                ResizeMode = ResizeMode.NoResize,
                AllowsTransparency = true,
                Background = Brushes.Black,
                ShowInTaskbar = false,
                ShowActivated = false,
                Topmost = true,
                Left = r.Left / dpi.DpiScaleX,
                Top = r.Top / dpi.DpiScaleY,
                Width = (r.Right - r.Left) / dpi.DpiScaleX,
                Height = (r.Bottom - r.Top) / dpi.DpiScaleY,
                Opacity = 1,
            };
            w.Show();
            return new FullscreenCurtain(w);
        }
        catch
        {
            return new FullscreenCurtain(null);
        }
    }

    /// <summary>Fade the curtain away once the new layout has been drawn.</summary>
    public void Lift()
    {
        if (_w == null) return;
        _w.Dispatcher.BeginInvoke(new Action(() =>
        {
            var a = new DoubleAnimation(1, 0, TimeSpan.FromMilliseconds(260)) { BeginTime = TimeSpan.FromMilliseconds(120) };
            a.Completed += (_, _) => _w.Close();
            _w.BeginAnimation(UIElement.OpacityProperty, a);
        }), System.Windows.Threading.DispatcherPriority.ContextIdle);
    }
}

internal static class NativeMethods
{
    [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
    public struct RectI
    {
        public int Left, Top, Right, Bottom;
    }

    [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
    public struct MonitorInfo
    {
        public int Size;
        public RectI Monitor;
        public RectI Work;
        public uint Flags;
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint flags);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern int SetWindowRgn(IntPtr hwnd, IntPtr region, bool redraw);

    [System.Runtime.InteropServices.DllImport("gdi32.dll")]
    public static extern IntPtr CreateRectRgn(int left, int top, int right, int bottom);
}
