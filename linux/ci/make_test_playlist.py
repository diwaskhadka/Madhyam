"""Write a catalog whose only playlist plays mpv's built-in test patterns (no network), for
the Linux playback test in CI.

    python make_test_playlist.py <path/to/libmadhyam_core_ffi.so> <data-dir>

The app started with --mode Live --play-first then plays one of these channels; its app.log
says whether frames were drawn ("first video frame", "time-pos=").
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "bindings", "python"))
from madhyam_core import MadhyamCore  # noqa: E402

lib, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
core = MadhyamCore(lib)
m3u = os.path.join(out, "test.m3u")
with open(m3u, "w") as f:
    f.write("#EXTM3U\n")
    for name, src in [("Test Pattern", "testsrc2"), ("Mandelbrot", "mandelbrot"), ("Colour Bars", "smptehdbars"), ("Life", "life")]:
        f.write(f'#EXTINF:-1 group-title="Test",{name}\nav://lavfi:{src}=size=1280x720:rate=30\n')
sid = core.call("add_m3u_source", name="Test patterns", location=m3u)["id"]
print(core.call("import_m3u_file", source=sid, path=m3u))
core.call("save_snapshot", path=os.path.join(out, "catalog.jsonl"))
with open(os.path.join(out, "settings.json"), "w") as f:
    f.write('{ "FreeTvPreinstalled": true }\n')
