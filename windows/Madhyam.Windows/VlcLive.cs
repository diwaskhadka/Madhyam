using System.Runtime.InteropServices;
using LibVLCSharp.Shared;

namespace Madhyam.Windows;

/// <summary>
/// Subtitle colours that change while a film plays, for the tracks VLC draws (those inside the
/// film). LibVLC 3 has no call for this, but its text renderer looks the colour options up again
/// for every new line, through its parents: renderer → subpicture unit → video output → media
/// player. A variable of the same name set on the media player object (in LibVLC 3 the player
/// is a VLC object) therefore wins over the instance option. Done through libvlccore's own
/// exported variable functions; if anything about that is missing it quietly does nothing and
/// the caller falls back to restarting the media.
/// </summary>
public static class VlcLive
{
    private const int VarInteger = 0x0030; // VLC_VAR_INTEGER

    [DllImport("libvlccore", EntryPoint = "var_Create", CallingConvention = CallingConvention.Cdecl)]
    private static extern int VarCreate(IntPtr obj, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int type);

    // vlc_value_t is an 8-byte union, passed by value: an int64 on x64.
    [DllImport("libvlccore", EntryPoint = "var_SetChecked", CallingConvention = CallingConvention.Cdecl)]
    private static extern int VarSetChecked(IntPtr obj, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int type, long value);

    private static bool? _supported;

    /// <summary>Whether live colours work here (the functions exist; checked once).</summary>
    public static bool Supported
    {
        get
        {
            if (_supported is { } s) return s;
            try
            {
                var lib = NativeLibrary.Load("libvlccore", typeof(VlcLive).Assembly, null);
                _supported = NativeLibrary.TryGetExport(lib, "var_Create", out _) && NativeLibrary.TryGetExport(lib, "var_SetChecked", out _);
            }
            catch (Exception ex)
            {
                Repository.Log($"live subtitle colours unavailable: {ex.Message}");
                _supported = false;
            }
            return _supported.Value;
        }
    }

    private static readonly System.Runtime.CompilerServices.ConditionalWeakTable<MediaPlayer, HashSet<string>> Created = new();

    /// <summary>Sets the live subtitle options on <paramref name="player"/>; false when that is not possible.</summary>
    public static bool Apply(MediaPlayer player)
    {
        if (!Supported) return false;
        try
        {
            var obj = player.NativeReference;
            var made = Created.GetOrCreateValue(player);
            foreach (var (name, value) in SubtitleStyle.LiveVlcValues())
            {
                if (made.Add(name) && VarCreate(obj, name, VarInteger) != 0) return false;
                if (VarSetChecked(obj, name, VarInteger, value) != 0) return false;
            }
            return true;
        }
        catch (Exception ex)
        {
            Repository.Log($"live subtitle colours not set: {ex.Message}");
            _supported = false;
            return false;
        }
    }
}
