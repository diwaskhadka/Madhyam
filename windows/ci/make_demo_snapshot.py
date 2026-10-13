"""Build a demo catalog snapshot for UI screenshots in CI (no network needed).

    python make_demo_snapshot.py <path/to/madhyam_core_ffi.dll> <out-dir>

Writes <out-dir>/catalog.jsonl (what Madhyam loads at start) plus generated
logo/poster PNGs. All names are fictional.
"""

import json
import os
import struct
import sys
import time
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "bindings", "python"))
from madhyam_core import MadhyamCore  # noqa: E402


def png(path, w, h, top, bottom):
    """Tiny PNG writer: vertical gradient between two RGB colours."""
    rows = []
    for y in range(h):
        t = y / max(1, h - 1)
        px = bytes(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
        rows.append(b"\x00" + px * w)
    raw = zlib.compress(b"".join(rows), 9)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", raw) + chunk(b"IEND", b""))


PALETTE = [((233, 87, 63), (120, 30, 60)), ((74, 144, 226), (20, 40, 110)), ((80, 200, 160), (20, 80, 90)),
           ((245, 180, 60), (150, 60, 20)), ((160, 110, 240), (50, 30, 110)), ((240, 110, 170), (100, 30, 80))]

CHANNELS = {
    "News": ["World News 24", "Metro Today", "Global Business", "Morning Desk", "City Report", "Night Edition"],
    "Sports": ["Arena One", "Goal TV", "Racing Live", "Court Side", "Extreme Plus"],
    "Kids": ["Cartoon Valley", "Little Planet", "Story Time"],
    "Documentary": ["Wild Earth", "History Lane", "Deep Ocean", "Sky & Space"],
    "Music": ["Hit Radio TV", "Jazz Lounge"],
}
SHOWS = ["Headlines", "The Big Interview", "Market Watch", "Weather Update", "Evening Magazine", "Late Report",
         "Match Highlights", "Studio Talk", "Nature's Secrets", "Top 20 Countdown", "Animated Adventures"]
MOVIES = {
    "Action & Mystery": [("Crimson Skylark", 8.5, 2026), ("The Voltage Guard", 8.1, 2025), ("Midnight Mantle", 8.0, 2024)],
    "Cosmic & Future Worlds": [("Quantum Comet", 8.7, 2026), ("Orbit of Atlas", 7.9, 2025), ("Nova Relay", 8.2, 2023)],
    "Documentary & Drama": [("Beacon Boy", 8.3, 2024), ("The Invisible Saturday", 7.8, 2022), ("River of Glass", 7.4, 2021),
                            ("Paper Lanterns", 7.1, 2020), ("North Light", 6.9, 2019)],
}
SERIES = [("Harbor Lights", "Drama"), ("Signal Lost", "Sci-Fi"), ("The Long Table", "Comedy"), ("Glass Coast", "Thriller")]


def main(lib, out):
    os.makedirs(out, exist_ok=True)
    art = os.path.join(out, "art")
    os.makedirs(art, exist_ok=True)
    core = MadhyamCore(lib)

    # ---- live channels (become Xtream live streams below)
    live_cats, live_streams, n = [], [], 0
    for gi, (group, names) in enumerate(CHANNELS.items()):
        live_cats.append({"category_id": str(100 + gi), "category_name": group})
        for name in names:
            top, bottom = PALETTE[n % len(PALETTE)]
            logo = os.path.join(art, f"logo{n}.png")
            png(logo, 96, 96, top, bottom)
            live_streams.append({"num": n + 1, "name": name, "stream_id": n + 1, "stream_icon": logo,
                                 "epg_channel_id": f"ch{n}", "category_id": str(100 + gi)})
            n += 1

    now = int(time.time())
    base = now - now % 1800 - 6 * 3600
    progs = []
    for c in range(n):
        t = base + (c % 3) * 600
        k = 0
        while t < now + 30 * 3600:
            length = 1800 * (1 + (c + k) % 3)
            title = SHOWS[(c + k) % len(SHOWS)]
            s = time.strftime("%Y%m%d%H%M%S +0000", time.gmtime(t))
            e = time.strftime("%Y%m%d%H%M%S +0000", time.gmtime(t + length))
            progs.append(f'<programme start="{s}" stop="{e}" channel="ch{c}"><title>{title}</title>'
                         f"<desc>Episode {k + 1}: a fictional programme description used for screenshots, long enough to wrap onto a second line.</desc></programme>")
            t += length
            k += 1
    guide = ("<tv>" + "".join(progs) + "</tv>").encode()

    # ---- one Xtream source with live, movies and series (fed directly, no server)
    xt = core.call("add_xtream_source", name="Demo provider", server="http://127.0.0.1:9", username="demo", password="demo")["id"]
    vod_cats, vods, i = [], [], 0
    for ci, (cat, films) in enumerate(MOVIES.items()):
        vod_cats.append({"category_id": str(ci + 1), "category_name": cat})
        for title, rating, year in films:
            top, bottom = PALETTE[i % len(PALETTE)]
            poster = os.path.join(art, f"poster{i}.png")
            png(poster, 200, 300, top, bottom)
            vods.append({"stream_id": 1000 + i, "name": title, "stream_icon": poster, "category_id": str(ci + 1),
                         "rating": str(rating), "year": str(year), "container_extension": "mp4",
                         "added": str(int(time.time()) - i * 86400 - 3600),
                         "plot": f"{title} is a fictional film used to preview Madhyam's movie page. "
                                 "Every sunrise needs a signal; this one arrives a little late."})
            i += 1
    series = []
    for si, (title, genre) in enumerate(SERIES):
        top, bottom = PALETTE[(si + 3) % len(PALETTE)]
        poster = os.path.join(art, f"series{si}.png")
        png(poster, 200, 300, top, bottom)
        series.append({"series_id": 500 + si, "name": title, "cover": poster, "category_id": "1", "genre": genre,
                       "last_modified": str(int(time.time()) - si * 43200 - 7200),
                       "rating": str(7 + si * 0.4), "releaseDate": f"{2020 + si}-01-01",
                       "plot": f"{title}: a fictional {genre.lower()} series for screenshots."})
    parts = {
        "account": {"user_info": {"auth": 1, "status": "Active"}, "server_info": {}},
        "live_categories": live_cats, "live_streams": live_streams,
        "vod_categories": vod_cats, "vod_streams": vods,
        "series_categories": [{"category_id": "1", "category_name": "All series"}], "series": series,
    }
    for part, body in parts.items():
        core.import_bytes("xtream_part", json.dumps(body).encode(), source=xt, part=part)
    core.call("xtream_commit", source=xt)
    # The app re-imports cached guides at start (the snapshot excludes EPG).
    os.makedirs(os.path.join(out, "epg"), exist_ok=True)
    with open(os.path.join(out, "epg", f"source-{xt}-0.xml"), "wb") as f:
        f.write(guide)

    first_series = core.call("search", query="harbor", kind="series")[0]
    episodes = {"1": [{"id": str(9000 + e), "episode_num": e + 1, "title": f"Chapter {e + 1}", "container_extension": "mp4",
                       "season": 1, "info": {"plot": "A fictional episode description."}} for e in range(6)]}
    core.import_bytes("series_info", json.dumps({"episodes": episodes}).encode(), id=first_series["id"])

    some = core.call("search", query="world")[0]
    core.call("set_favorite", id=some["id"], on=True)
    for q in ("arena", "jazz"):
        core.call("set_favorite", id=core.call("search", query=q, kind="live")[0]["id"], on=True)
    film = core.call("search", query="quantum", kind="movie")[0]
    core.call("set_favorite", id=film["id"], on=True)
    core.call("set_favorite", id=first_series["id"], on=True)
    # Watch history and progress for Continue watching / Recently viewed.
    eps = core.call("episodes", id=first_series["id"])
    started = [(film["id"], 2400, 6600),
               (core.call("search", query="beacon", kind="movie")[0]["id"], 1200, 5400),
               (eps[0]["id"], 2700, 2700), (eps[1]["id"], 900, 2700)]
    for item_id, pos, dur in started:
        core.call("record_play", id=item_id)
        core.call("set_progress", id=item_id, position=pos, duration=dur)
    for q in ("metro", "goal", "wild"):
        core.call("record_play", id=core.call("search", query=q, kind="live")[0]["id"])
    core.call("save_snapshot", path=os.path.join(out, "catalog.jsonl"))
    print("demo snapshot:", core.call("stats"))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
