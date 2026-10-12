"""End-to-end test of the C ABI, playing the role of a host app.

Starts a local HTTP server that impersonates an Xtream panel and serves an
M3U playlist plus an XMLTV guide, then drives the library exactly the way an
Android TV or Windows app would: ask the core which URLs to fetch, fetch them
with the host's own HTTP stack, hand the bytes back.

    python3 e2e_test.py path/to/libmadhyam_core_ffi.so
"""

import json
import os
import sys
import tempfile
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from madhyam_core import MadhyamCore, IptvError  # noqa: E402

NOW = int(time.time())
HOUR = 3600


def xmltv_time(t):
    return time.strftime("%Y%m%d%H%M%S +0000", time.gmtime(t))


GUIDE = f"""<?xml version="1.0" encoding="UTF-8"?>
<tv>
 <channel id="cnn.us"><display-name>CNN</display-name></channel>
 <channel id="bbc1.uk"><display-name>BBC One</display-name></channel>
 <programme start="{xmltv_time(NOW - HOUR)}" stop="{xmltv_time(NOW + HOUR)}" channel="cnn.us"><title>Newsroom</title></programme>
 <programme start="{xmltv_time(NOW + HOUR)}" stop="{xmltv_time(NOW + 2 * HOUR)}" channel="cnn.us"><title>Late Edition</title></programme>
 <programme start="{xmltv_time(NOW - 600)}" stop="{xmltv_time(NOW + 600)}" channel="bbc1.uk"><title>Weather &amp; Travel</title></programme>
</tv>"""

PLAYLIST = """#EXTM3U url-tvg="http://127.0.0.1:{port}/guide.xml"
#EXTINF:-1 tvg-id="bbc1.uk" tvg-logo="http://l/bbc.png" group-title="UK",BBC One HD
http://127.0.0.1:{port}/stream/bbc1.m3u8
#EXTINF:-1 group-title="UK",Channel 4
http://127.0.0.1:{port}/stream/c4.m3u8
"""

PANEL = {
    None: {"user_info": {"auth": 1, "status": "Active", "exp_date": str(NOW + 86400 * 30), "max_connections": "1"},
           "server_info": {"timezone": "UTC", "timestamp_now": NOW}},
    "get_live_categories": [{"category_id": "1", "category_name": "News"}],
    "get_live_streams": [{"num": 1, "name": "CNN", "stream_id": 101, "epg_channel_id": "cnn.us", "category_id": "1",
                          "tv_archive": 1, "tv_archive_duration": 2}],
    "get_vod_categories": [{"category_id": 5, "category_name": "Drama"}],
    "get_vod_streams": [{"name": "Heat", "stream_id": "7", "container_extension": "mkv", "category_id": "5"}],
    "get_series_categories": [],
    "get_series": [{"series_id": 900, "name": "Detective Show", "cover": "c.jpg"}],
    "get_series_info": {"episodes": {"1": [{"id": "9001", "episode_num": 1, "title": "Pilot", "container_extension": "mp4"},
                                           {"id": "9002", "episode_num": 2, "title": "Second", "container_extension": "mp4"}]}},
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, body, ctype="application/json"):
        data = body.encode() if isinstance(body, str) else body
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        if u.path == "/player_api.php":
            if q.get("username") != ["user"] or q.get("password") != ["p@ss word"]:
                return self._send(json.dumps({"user_info": {"auth": 0}}))
            return self._send(json.dumps(PANEL[q.get("action", [None])[0]]))
        if u.path == "/playlist.m3u":
            return self._send(PLAYLIST.format(port=self.server.server_port), "audio/x-mpegurl")
        if u.path in ("/guide.xml", "/xmltv.php"):
            return self._send(GUIDE, "application/xml")
        self.send_response(404)
        self.end_headers()


def http_get(url):
    """The host's HTTP stack (on Android this would be OkHttp)."""
    with urllib.request.urlopen(url, timeout=5) as r:
        return r.read()


def check(cond, msg):
    if not cond:
        raise AssertionError(msg)
    print("  ok -", msg)


def main(lib):
    srv = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_port}"

    core = MadhyamCore(lib)
    print(f"madhyam_core {core.version}")

    print("M3U source")
    m3u = core.call("add_m3u_source", name="Home", location=f"{base}/playlist.m3u")["id"]
    st = core.import_bytes("m3u", http_get(f"{base}/playlist.m3u"), source=m3u)
    check(st["items"] == 2, "playlist imported")
    for url in core.call("epg_fetch_urls", source=m3u):
        core.import_bytes("xmltv", http_get(url))
    bbc = core.call("search", query="bbc")[0]
    nn = core.call("now_next", id=bbc["id"])
    check(nn["now"]["title"] == "Weather & Travel", "guide matched by tvg-id, entities decoded")

    print("Xtream source (staged, host does HTTP)")
    xt = core.call("add_xtream_source", name="Panel", server=f"{base}/", username="user", password="p@ss word")["id"]
    for req in core.call("xtream_requests", source=xt):
        res = core.import_bytes("xtream_part", http_get(req["url"]), source=xt, part=req["part"])
        if req["part"] == "account":
            check(res["status"] == "Active", "login accepted, account info returned early")
    res = core.call("xtream_commit", source=xt)
    check(res["items"] == 3 and not res["warnings"], "live + vod + series imported")
    check("p@ss" not in json.dumps(core.call("sources")), "password never exposed in UI JSON")

    cnn = core.call("search", query="cnn", kind="live")[0]
    check(cnn["archive_days"] == 2 and cnn["url"].endswith("/live/user/p%40ss%20word/101.ts"), "stream URL built and encoded")
    for url in core.call("epg_fetch_urls", source=xt):
        core.import_bytes("xmltv", http_get(url))
    check(core.call("now_next", id=cnn["id"])["next"]["title"] == "Late Edition", "Xtream guide via xmltv.php")
    ts = core.call("timeshift_url", id=cnn["id"], start=NOW - HOUR, duration_min=30)
    check("/timeshift/user/" in ts, "catch-up URL")

    show = core.call("search", query="detect")[0]
    core.import_bytes("series_info", http_get(core.call("series_info_url", id=show["id"])), id=show["id"])
    eps = core.call("episodes", id=show["id"])
    check([e["name"] for e in eps] == ["Pilot", "Second"], "series episodes fetched on demand")

    print("User state + persistence")
    heat = core.call("search", query="heat")[0]
    core.call("set_progress", id=heat["id"], position=1200, duration=6000)
    core.call("set_favorite", id=cnn["id"])
    core.call("record_play", id=cnn["id"])
    check(core.call("continue_watching")[0]["id"] == heat["id"], "continue watching")
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "catalog.jsonl")
        core.call("save_snapshot", path=path)
        fresh = MadhyamCore(lib)
        n = fresh.call("load_snapshot", path=path)["items"]
        check(n == core.call("stats")["items"], f"snapshot restored ({n} items) without network")
        check(fresh.call("favorites")[0]["name"] == "CNN", "favorites survive restart")
        fresh.close()

    print("Errors and threads")
    try:
        core.call("item", id="12345")
        raise AssertionError("expected not_found")
    except IptvError as e:
        check(e.code == "not_found", "typed error codes")
    bad = core.call("add_xtream_source", name="Bad", server=base, username="user", password="wrong")["id"]
    try:
        core.import_bytes("xtream_part", http_get(core.call("xtream_requests", source=bad)[0]["url"]), source=bad, part="account")
        raise AssertionError("expected api error")
    except IptvError as e:
        check(e.code == "api", "wrong password reported at the first request")

    errors = []

    def hammer():
        try:
            for _ in range(300):
                core.call("search", query="c")
                core.call("now_next_many", ids=[cnn["id"], bbc["id"]])
        except Exception as e:  # noqa: BLE001
            errors.append(e)

    threads = [threading.Thread(target=hammer) for _ in range(8)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    check(not errors, "2400 concurrent calls from 8 threads")

    core.close()
    srv.shutdown()
    print("ALL PASSED")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else None)
