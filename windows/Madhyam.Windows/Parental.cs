using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using static Madhyam.Windows.UiKit;

namespace Madhyam.Windows;

/// <summary>
/// Parental controls: a 4-digit PIN (only a salted SHA-256 is stored), locked categories,
/// auto-locked adult names, a rating limit and an optional lock on Settings. Once the PIN is
/// entered everything stays unlocked until the app closes or 10 minutes pass without a
/// PIN-gated action. Same rules as the Android TV and Linux apps.
/// </summary>
public static class Parental
{
    private static AppSettings S => AppSettings.Current;

    public static readonly TimeSpan IdleTimeout = TimeSpan.FromMinutes(10);

    /// <summary>Names that lock a category or channel by themselves (whole word, any case).</summary>
    public static readonly string[] AdultWords =
        { "xxx", "adult", "adults", "18+", "+18", "porn", "erotic", "erotica", "playboy", "hustler", "brazzers", "redlight", "private" };

    // A word counts when nothing but separators (space, punctuation, _ …) is on either side.
    private static readonly Regex AdultRx = new(
        @"(?<![\p{L}\p{N}])(?:" + string.Join("|", AdultWords.Select(Regex.Escape)) + @")(?![\p{L}\p{N}])",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant | RegexOptions.Compiled);

    public static bool IsAdultName(string? name) => !string.IsNullOrEmpty(name) && AdultRx.IsMatch(name);

    public static bool HasPin => S.ParentalPinHash.Length > 0 && S.ParentalPinSalt.Length > 0;

    /// <summary>Parental controls are on (a PIN is always set when they are).</summary>
    public static bool On => S.ParentalEnabled && HasPin;

    // ---------------------------------------------------------------- PIN

    private static string Hash(string salt, string pin) =>
        Convert.ToHexString(SHA256.HashData(Convert.FromHexString(salt).Concat(Encoding.UTF8.GetBytes(pin)).ToArray())).ToLowerInvariant();

    public static bool Verify(string pin) =>
        HasPin && CryptographicOperations.FixedTimeEquals(
            Encoding.ASCII.GetBytes(Hash(S.ParentalPinSalt, pin)), Encoding.ASCII.GetBytes(S.ParentalPinHash));

    public static void SetPin(string pin)
    {
        S.ParentalPinSalt = Convert.ToHexString(RandomNumberGenerator.GetBytes(16)).ToLowerInvariant();
        S.ParentalPinHash = Hash(S.ParentalPinSalt, pin);
        AppSettings.Save();
    }

    /// <summary>Forgotten PIN: remove the PIN and every lock (after the typed confirmation).</summary>
    public static void Reset()
    {
        S.ParentalEnabled = false;
        S.ParentalPinHash = "";
        S.ParentalPinSalt = "";
        S.ParentalAutoAdult = true;
        S.ParentalLockSettings = false;
        S.ParentalMaxAge = 0;
        S.LockedCategories.Clear();
        S.UnlockedCategories.Clear();
        AppSettings.Save();
        Repository.Log("parental controls reset: PIN and all locks removed");
        Lock();
    }

    // ---------------------------------------------------------------- unlocked for this session

    private static DateTime? _lastUse;

    /// <summary>Raised when the session is unlocked or locked again (the open-lock icon in the top bar).</summary>
    public static event Action? Changed;

    public static bool Unlocked => On && _lastUse is { } t && DateTime.UtcNow - t < IdleTimeout;

    public static void Lock()
    {
        var was = _lastUse != null;
        _lastUse = null;
        if (was) Changed?.Invoke();
    }

    /// <summary>Just proved the PIN another way (chose a new one): count as unlocked.</summary>
    public static void MarkUnlocked()
    {
        if (!On) return;
        _lastUse = DateTime.UtcNow;
        Changed?.Invoke();
    }

    /// <summary>Called every second: lock again after 10 minutes without a PIN-gated action.</summary>
    public static void CheckExpiry()
    {
        if (_lastUse != null && !Unlocked) Lock();
    }

    /// <summary>
    /// The gate in front of every protected action: true when parental controls are off, the
    /// session is already unlocked (which also keeps it unlocked for another 10 minutes), or
    /// the right PIN is entered now.
    /// </summary>
    public static bool Ensure(string reason, Window? owner = null)
    {
        if (!On) return true;
        if (Unlocked)
        {
            _lastUse = DateTime.UtcNow;
            return true;
        }
        if (PinDialog.Ask(owner ?? Application.Current.MainWindow, "Enter your PIN", reason, Verify, offerReset: true) == null) return false;
        if (!On) return true; // reset from the dialog: nothing is locked any more
        _lastUse = DateTime.UtcNow;
        Changed?.Invoke();
        return true;
    }

    /// <summary>With "Lock Settings": Settings and adding, editing or removing a playlist need the PIN.</summary>
    public static bool EnsureSettings(string reason) => !S.ParentalLockSettings || Ensure(reason);

    // ---------------------------------------------------------------- categories and items

    private static string Key(int source, string kind) => AppSettings.HiddenKey(source, kind == "episode" ? "series" : kind);

    private static bool InList(Dictionary<string, List<string>> map, int source, string kind, string group) =>
        map.TryGetValue(Key(source, kind), out var l) && l.Contains(group);

    /// <summary>
    /// Lock state of a category, whether or not the session is unlocked (the 🔒 stays visible).
    /// An explicit unlock wins over the adult-name rule.
    /// </summary>
    public static (bool Locked, bool Adult) CategoryState(int source, string kind, string group)
    {
        if (!On) return (false, false);
        if (InList(S.LockedCategories, source, kind, group)) return (true, false);
        if (S.ParentalAutoAdult && IsAdultName(group) && !InList(S.UnlockedCategories, source, kind, group)) return (true, true);
        return (false, false);
    }

    public static void SetCategoryLocked(int source, string kind, string group, bool locked)
    {
        var key = Key(source, kind);
        List<string> Get(Dictionary<string, List<string>> map) => map.TryGetValue(key, out var l) ? l : map[key] = new List<string>();
        var adult = S.ParentalAutoAdult && IsAdultName(group);
        Get(S.LockedCategories).Remove(group);
        Get(S.UnlockedCategories).Remove(group);
        if (locked && !adult) Get(S.LockedCategories).Add(group);
        if (!locked && adult) Get(S.UnlockedCategories).Add(group);
        if (S.LockedCategories[key].Count == 0) S.LockedCategories.Remove(key);
        if (S.UnlockedCategories[key].Count == 0) S.UnlockedCategories.Remove(key);
        AppSettings.Save();
    }

    /// <summary>A category that needs the PIN right now (locked and the session not unlocked).</summary>
    public static bool CategoryNeedsPin(int source, string kind, string group) =>
        !Unlocked && CategoryState(source, kind, group).Locked;

    /// <summary>The item's own lock: its category, or an adult name on the channel / title itself.</summary>
    public static bool IsItemLocked(ItemVm it) =>
        On && (CategoryState(it.SourceId, it.Kind, it.Group).Locked || (S.ParentalAutoAdult && IsAdultName(it.Name)));

    /// <summary>Left out of search results, Home rows and "All" lists while locked.</summary>
    public static bool HideNow(ItemVm it) => !Unlocked && IsItemLocked(it);

    /// <summary>Locked category names to leave out of an "All" list (empty when unlocked or off).</summary>
    public static HashSet<string> LockedGroups(int source, string kind, IEnumerable<string> groups) =>
        Unlocked || !On ? new HashSet<string>() : groups.Where(g => CategoryState(source, kind, g).Locked).ToHashSet();

    // ---------------------------------------------------------------- age ratings

    private static readonly Regex Letters = new(@"^(?:RATED\s+)?(NC-?17|PG-?13|PG|G|R|TV-?MA|TV-?Y7?|TV-?G|TV-?PG)\b", RegexOptions.IgnoreCase);

    /// <summary>
    /// Age from a provider rating: G/PG → 7, PG-13 → 12, R → 16, NC-17 → 18 (TV-Y/G/PG → 7, TV-MA → 18);
    /// numbers such as "12+" or "FSK 16" as they are. Null when there is no usable rating.
    /// </summary>
    public static int? AgeOf(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return null;
        var s = raw.Trim();
        if (Letters.Match(s) is { Success: true } m)
            return m.Groups[1].Value.ToUpperInvariant().Replace("-", "") switch
            {
                "NC17" or "TVMA" => 18,
                "PG13" => 12,
                "R" => 16,
                _ => 7,
            };
        if (Regex.Match(s, @"(?<!\d)(\d{1,2})(?!\d)") is { Success: true } n && int.Parse(n.Groups[1].Value) is var age and <= 21)
            return age;
        return null;
    }

    /// <summary>Above "Allow up to"? Items with no age rating are allowed.</summary>
    public static bool AboveLimit(int? age) => On && S.ParentalMaxAge > 0 && age is { } a && a > S.ParentalMaxAge;

    // ---------------------------------------------------------------- PIN set / change / turn off

    /// <summary>"Set PIN": asks for a new PIN twice. Returns false when cancelled.</summary>
    public static bool AskNewPin(Window owner)
    {
        while (true)
        {
            var first = PinDialog.Ask(owner, "Choose a PIN", "Four digits. You need it to open locked content and to change these settings.");
            if (first == null) return false;
            var again = PinDialog.Ask(owner, "Enter the PIN again", "To make sure it is the one you meant.");
            if (again == null) return false;
            if (again == first)
            {
                SetPin(first);
                return true;
            }
            DialogKit.Alert(owner, "The PINs didn't match", "The two PINs were different. Please try again.");
        }
    }

    /// <summary>"Change PIN": the current one first, then the new one twice.</summary>
    public static bool ChangePin(Window owner)
    {
        if (PinDialog.Ask(owner, "Enter your current PIN", "Before choosing a new one.", Verify, offerReset: true) == null || !HasPin) return false;
        return AskNewPin(owner);
    }

    /// <summary>"Reset parental controls": typed confirmation, then everything is removed.</summary>
    public static bool ConfirmReset(Window owner)
    {
        var w = DialogKit.Make(owner, "Reset parental controls", 460);
        var root = new StackPanel { Margin = new Thickness(24) };
        root.Children.Add(Text("Reset parental controls", 21, "Fg", FontWeights.SemiBold));
        var t = Text("Forgot the PIN? This removes the PIN and every lock (locked categories, adult lock, rating limit and the Settings lock). " +
                     "Type RESET to confirm.", 13, "Muted", wrap: true);
        t.Margin = new Thickness(0, 6, 0, 12);
        root.Children.Add(t);
        var box = new TextBox();
        root.Children.Add(box);
        var ok = false;
        var reset = DialogKit.Button("Reset", true, () =>
        {
            ok = true;
            w.Close();
        });
        reset.IsEnabled = false;
        box.TextChanged += (_, _) => reset.IsEnabled = box.Text.Trim() == "RESET";
        root.Children.Add(DialogKit.Footer(DialogKit.Button("Cancel", false, w.Close), reset));
        w.Content = root;
        w.Loaded += (_, _) => box.Focus();
        w.ShowDialog();
        if (ok) Reset();
        return ok;
    }
}

/// <summary>
/// PIN entry: four digit boxes, typed digits (also the number pad). A wrong PIN shakes and
/// clears; after 5 wrong tries the dialog waits 30 seconds.
/// </summary>
public static class PinDialog
{
    private static int _fails;
    private static DateTime _waitUntil;

    /// <param name="verify">Checks the PIN; null accepts any 4 digits (choosing a new PIN).</param>
    /// <returns>The PIN entered, or null when cancelled.</returns>
    public static string? Ask(Window owner, string title, string subtitle, Func<string, bool>? verify = null, bool offerReset = false)
    {
        var w = DialogKit.Make(owner, "Parental controls", 420);
        var root = new StackPanel { Margin = new Thickness(26, 24, 26, 24) };
        var icon = new TextBlock { Text = "", FontFamily = Icons, FontSize = 28, Foreground = B("Accent"), HorizontalAlignment = HorizontalAlignment.Center };
        root.Children.Add(icon);
        var head = Text(title, 20, "Fg", FontWeights.SemiBold);
        head.HorizontalAlignment = HorizontalAlignment.Center;
        head.Margin = new Thickness(0, 10, 0, 0);
        root.Children.Add(head);
        var sub = Text(subtitle, 13, "Muted", wrap: true);
        sub.TextAlignment = TextAlignment.Center;
        sub.HorizontalAlignment = HorizontalAlignment.Center;
        sub.Margin = new Thickness(0, 6, 0, 18);
        root.Children.Add(sub);

        var digits = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Center };
        var shake = new TranslateTransform();
        digits.RenderTransform = shake;
        var boxes = new List<(Border Box, TextBlock Dot)>();
        for (var i = 0; i < 4; i++)
        {
            var dot = new TextBlock { FontSize = 22, Foreground = B("Fg"), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
            var b = new Border
            {
                Width = 54, Height = 62, CornerRadius = new CornerRadius(12), Margin = new Thickness(6, 0, 6, 0),
                Background = B("Card"), BorderThickness = new Thickness(2), BorderBrush = B("Line"), Child = dot,
            };
            boxes.Add((b, dot));
            digits.Children.Add(b);
        }
        root.Children.Add(digits);
        var message = Text("", 12.5, "LiveRed", wrap: true);
        message.TextAlignment = TextAlignment.Center;
        message.HorizontalAlignment = HorizontalAlignment.Center;
        message.Margin = new Thickness(0, 14, 0, 0);
        root.Children.Add(message);

        var footer = new DockPanel { Margin = new Thickness(0, 18, 0, 0) };
        if (offerReset)
        {
            var forgot = new Button { Content = "Forgot PIN?", Style = St("LinkButton"), VerticalAlignment = VerticalAlignment.Center };
            forgot.Click += (_, _) =>
            {
                if (!Parental.ConfirmReset(w)) return;
                w.Tag = "reset";
                w.Close();
            };
            footer.Children.Add(forgot);
        }
        var cancel = DialogKit.Button("Cancel", false, w.Close);
        cancel.HorizontalAlignment = HorizontalAlignment.Right;
        footer.Children.Add(cancel);
        root.Children.Add(footer);
        w.Content = root;

        var entered = "";
        string? result = null;
        var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(0.5) };
        bool Waiting() => DateTime.UtcNow < _waitUntil;
        void Paint()
        {
            for (var i = 0; i < 4; i++)
            {
                boxes[i].Dot.Text = i < entered.Length ? "●" : "";
                boxes[i].Box.BorderBrush = B(i == entered.Length && !Waiting() ? "Accent" : "Line");
            }
            if (Waiting())
                message.Text = $"Too many wrong tries. Try again in {(int)Math.Ceiling((_waitUntil - DateTime.UtcNow).TotalSeconds)} s.";
        }
        timer.Tick += (_, _) =>
        {
            if (!Waiting() && message.Text.StartsWith("Too many")) message.Text = "";
            Paint();
        };
        timer.Start();
        w.Closed += (_, _) => timer.Stop();

        void Wrong()
        {
            entered = "";
            _fails++;
            if (_fails >= 5)
            {
                _fails = 0;
                _waitUntil = DateTime.UtcNow.AddSeconds(30);
            }
            else message.Text = $"Wrong PIN. {5 - _fails} {(5 - _fails == 1 ? "try" : "tries")} left before a short wait.";
            var a = new DoubleAnimationUsingKeyFrames { Duration = TimeSpan.FromMilliseconds(420) };
            foreach (var (x, ms) in new[] { (-14.0, 60), (12.0, 120), (-10.0, 180), (8.0, 240), (-5.0, 300), (3.0, 360), (0.0, 420) })
                a.KeyFrames.Add(new LinearDoubleKeyFrame(x, KeyTime.FromTimeSpan(TimeSpan.FromMilliseconds(ms))));
            shake.BeginAnimation(TranslateTransform.XProperty, a);
            Paint();
        }

        void Type(char c)
        {
            if (Waiting() || entered.Length >= 4) return;
            entered += c;
            message.Text = "";
            Paint();
            if (entered.Length < 4) return;
            if (verify == null || verify(entered))
            {
                _fails = verify == null ? _fails : 0;
                result = entered;
                // Let the fourth dot show for a moment before closing.
                w.Dispatcher.BeginInvoke(() => w.Close(), DispatcherPriority.Background);
                return;
            }
            Wrong();
        }

        w.PreviewTextInput += (_, e) =>
        {
            foreach (var c in e.Text.Where(char.IsAsciiDigit)) Type(c);
            e.Handled = true;
        };
        w.PreviewKeyDown += (_, e) =>
        {
            if (e.Key == Key.Back && entered.Length > 0 && !Waiting())
            {
                entered = entered[..^1];
                Paint();
                e.Handled = true;
            }
        };
        w.Loaded += (_, _) =>
        {
            Paint();
            w.Activate();
            Keyboard.Focus(w);
        };
        w.ShowDialog();
        if (w.Tag is "reset") return "";
        return result;
    }
}
