"""A fake Xtream Codes panel with fixed content, for the Android TV focus tests.

    py android/qa/fixture_panel.py            # serves on port 8099

From the emulator it is http://10.0.2.2:8099, user "qa", password "qa".
Every name is predictable, so a test case can name the item that should have the focus:

    Live TV   News 01..25, Sports 01..25, Kids 01..25 (News has a guide)
    Movies    Action 01..30, Drama 01..30
    Series    Crime 01..10, Comedy 01..10; each has seasons 1 and 2 of 4 episodes
"""

import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8099
NOW = int(time.time())
HOUR = 3600
USER, PASSWORD = "qa", "qa"

LIVE_CATS = [("1", "News"), ("2", "Sports"), ("3", "Kids")]
VOD_CATS = [("10", "Action"), ("11", "Drama")]
SERIES_CATS = [("20", "Crime"), ("21", "Comedy")]


def live_streams():
    out, n = [], 0
    for cid, name in LIVE_CATS:
        for i in range(1, 26):
            n += 1
            out.append({
                "num": n, "name": f"{name} {i:02d}", "stream_id": 1000 + n, "category_id": cid,
                "epg_channel_id": f"news{i}.qa" if name == "News" else "",
                "tv_archive": 0,
            })
    return out


def vod_streams():
    out, n = [], 0
    for cid, name in VOD_CATS:
        for i in range(1, 31):
            n += 1
            out.append({
                "name": f"{name} {i:02d}", "stream_id": str(5000 + n), "container_extension": "mp4",
                "category_id": cid, "rating": str(5 + (i % 5)), "added": str(NOW - n * 86400),
            })
    return out


def series():
    out, n = [], 0
    for cid, name in SERIES_CATS:
        for i in range(1, 11):
            n += 1
            out.append({"series_id": 9000 + n, "name": f"{name} {i:02d}", "category_id": cid,
                        "rating": str(6 + (i % 4)), "last_modified": str(NOW - n * 86400)})
    return out


def series_info(sid):
    eps = {}
    for season in (1, 2):
        eps[str(season)] = [
            {"id": f"{sid}{season}{e}", "episode_num": e, "season": season, "title": f"Episode {e}",
             "container_extension": "mp4"}
            for e in range(1, 5)
        ]
    return {"info": {"name": f"Series {sid}"}, "seasons": [{"season_number": 1}, {"season_number": 2}], "episodes": eps}


def guide():
    def t(x):
        return time.strftime("%Y%m%d%H%M%S +0000", time.gmtime(x))
    now = int(time.time())  # around the time of the request, so a panel left running still has a "now"
    rows = ['<?xml version="1.0" encoding="UTF-8"?>', "<tv>"]
    for i in range(1, 26):
        rows.append(f' <channel id="news{i}.qa"><display-name>News {i:02d}</display-name></channel>')
        start = now - now % HOUR - HOUR
        for k in range(6):
            rows.append(f' <programme start="{t(start + k * HOUR)}" stop="{t(start + (k + 1) * HOUR)}" '
                        f'channel="news{i}.qa"><title>Show {i}.{k}</title></programme>')
    rows.append("</tv>")
    return "\n".join(rows)


PANEL = {
    None: {"user_info": {"auth": 1, "status": "Active", "exp_date": str(NOW + 86400 * 365), "max_connections": "1"},
           "server_info": {"timezone": "UTC", "timestamp_now": NOW}},
    "get_live_categories": [{"category_id": c, "category_name": n} for c, n in LIVE_CATS],
    "get_live_streams": live_streams(),
    "get_vod_categories": [{"category_id": c, "category_name": n} for c, n in VOD_CATS],
    "get_vod_streams": vod_streams(),
    "get_series_categories": [{"category_id": c, "category_name": n} for c, n in SERIES_CATS],
    "get_series": series(),
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, body, ctype="application/json", code=200):
        data = body.encode() if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        if u.path == "/player_api.php":
            if q.get("username") != [USER] or q.get("password") != [PASSWORD]:
                return self._send(json.dumps({"user_info": {"auth": 0}}))
            action = q.get("action", [None])[0]
            if action == "get_series_info":
                return self._send(json.dumps(series_info(q.get("series_id", ["0"])[0])))
            if action == "get_vod_info":
                return self._send(json.dumps({"info": {"plot": "A film for the focus tests."}, "movie_data": {}}))
            if action in ("get_short_epg", "get_simple_data_table"):
                return self._send(json.dumps({"epg_listings": []}))
            return self._send(json.dumps(PANEL.get(action, [])))
        if u.path == "/xmltv.php":
            return self._send(guide(), "application/xml")
        self._send("", "text/plain", 404)


if __name__ == "__main__":
    print(f"fixture panel on http://0.0.0.0:{PORT}  (emulator: http://10.0.2.2:{PORT}, user {USER}, password {PASSWORD})")
    ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
