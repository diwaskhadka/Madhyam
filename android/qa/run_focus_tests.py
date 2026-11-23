"""Runs the D-pad focus test cases in focus_cases.md on an emulator or TV.

    py android/qa/fixture_panel.py                    # once, in another terminal
    py android/qa/run_focus_tests.py                  # every case
    py android/qa/run_focus_tests.py LV               # only cases whose ID starts with LV
    DEV=192.168.1.143:5555 py android/qa/run_focus_tests.py --record   # a TV with its own playlist

Expects the "QA" playlist from the fixture panel (see focus_cases.md); --record runs anywhere and
only records. Each group opens its page from the menu, like a person would. For each case it sends
the keys, reads which element has the focus (uiautomator), compares its text with "Expect", and
saves a screenshot. The report, screenshots and one contact sheet per group go to android/qa/out/
(not committed).
"""

import os
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
ADB = os.path.join(os.environ.get("LOCALAPPDATA", ""), "Android", "Sdk", "platform-tools", "adb.exe")
if not os.path.exists(ADB):
    ADB = "adb"
DEV = os.environ.get("DEV", "emulator-5554")
PKG = "dev.madhyam"
OUT = os.path.join(HERE, "out")
SETTLE = float(os.environ.get("SETTLE", "0.9"))  # seconds after the last key of a case

KEYS = {
    "UP": "DPAD_UP", "DOWN": "DPAD_DOWN", "LEFT": "DPAD_LEFT", "RIGHT": "DPAD_RIGHT",
    "OK": "DPAD_CENTER", "BACK": "BACK", "MENU": "MENU",
}


def adb(*args, binary=False):
    r = subprocess.run([ADB, "-s", DEV, *args], capture_output=True)
    return r.stdout if binary else r.stdout.decode("utf-8", "replace")


def press(token):
    """UP, DOWN, LEFT, RIGHT, OK, BACK, MENU, HOLD (long OK), WAIT, or UPx3 for three presses."""
    m = re.fullmatch(r"([A-Z]+)(?:x(\d+))?", token)
    if not m:
        raise SystemExit(f"unknown key {token!r}")
    name, times = m.group(1), int(m.group(2) or 1)
    for _ in range(times):
        if name == "WAIT":
            time.sleep(1.5)
        elif name == "HOLD":
            adb("shell", "input keyevent --longpress DPAD_CENTER")
            time.sleep(0.6)
        elif name in KEYS:
            adb("shell", f"input keyevent {KEYS[name]}")
            time.sleep(0.45)
        else:
            raise SystemExit(f"unknown key {token!r}")


def box(s):
    return [int(v) for v in re.findall(r"-?\d+", s or "")]


def focused():
    """The focused element's text: its own and its children's, or (a poster) the title just below it."""
    for _ in range(4):
        adb("shell", "uiautomator dump /sdcard/qa-ui.xml")
        x = adb("shell", "cat /sdcard/qa-ui.xml")
        if "<hierarchy" in x:
            break
        time.sleep(0.5)
    else:
        return "?"
    root = ET.fromstring(x[x.index("<"):])
    for n in root.iter("node"):
        if n.get("focused") != "true":
            continue
        texts = [m.get("text") or m.get("content-desc") for m in n.iter("node")]
        texts = [t for t in texts if t]
        b = box(n.get("bounds"))
        # A poster or card: its title sits under it (inside there is at most a rating badge).
        if len(b) == 4 and b[2] - b[0] < 600 and not any(re.search(r"[A-Za-z]", t) and not t.startswith("★") for t in texts):
            for m in root.iter("node"):
                t = m.get("text") or m.get("content-desc")
                c = box(m.get("bounds"))
                if t and len(c) == 4 and b[0] - 4 <= c[0] <= b[2] and b[1] <= c[1] <= b[3] + 90:
                    texts.append(t)
        return " | ".join(t.replace("\n", " ") for t in texts) or "(no text)"
    return "(nothing focused)"


def parse(path):
    """Groups: '## Title' then 'start: <page> [keys…]', then table rows | ID | Keys | Expect | Why |."""
    groups, g = [], None
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("## "):
            g = {"title": line[3:].strip(), "start": None, "cases": []}
            groups.append(g)
        elif g is not None and line.startswith("start:"):
            words = line[6:].strip().split()
            page = next(m for m in sorted(MENU, key=len, reverse=True) if " ".join(words).startswith(m))
            g["start"] = [page] + words[len(page.split()):]
        elif g is not None and line.startswith("|") and not re.match(r"\|\s*-", line):
            # A backslash-escaped pipe is a pipe inside a cell (the "a || b" of Expect).
            cells = [c.strip().replace("\\|", "|") for c in re.split(r"(?<!\\)\|", line.strip().strip("|"))]
            if len(cells) < 3 or cells[0] in ("ID", ""):
                continue
            g["cases"].append({"id": cells[0], "keys": cells[1].replace("`", "").split(), "expect": cells[2].replace("`", ""),
                               "why": cells[3] if len(cells) > 3 else ""})
    return groups


MENU = ["Home", "Live TV", "Movies", "Series", "Favorites", "Recent", "Search", "Settings"]


def menu_label(text):
    """The menu entry the focus is on (its text repeats the label), or None."""
    first = text.split(" | ")[0]
    if first in MENU:
        return first
    if "Switch playlist" in text or text.endswith("| Playlists"):
        return "playlist"
    return None


def start(page, keys, tries=2):
    """Restart the app, go Left into the menu, walk to [page] and press OK, then [keys]."""
    for attempt in range(tries):
        if open_page(page):
            break
        print(f"  (menu entry {page!r} not reached, restarting the app)", flush=True)
    else:
        raise SystemExit(f"menu entry {page!r} not found")
    for k in keys:
        press(k)


def open_page(page):
    adb("shell", f"am force-stop {PKG}")
    adb("shell", f"am start -n {PKG}/.MainActivity")
    t0 = time.time()
    while time.time() - t0 < 40:
        time.sleep(1.5)
        if focused() not in ("?", "(nothing focused)"):
            break
    time.sleep(1.0)
    for _ in range(6):
        if menu_label(focused()):
            break
        press("LEFT")
    for _ in range(len(MENU) + 2):
        if menu_label(focused()) == page:
            break
        press("DOWN")
    else:
        return False
    press("OK")
    time.sleep(2.5)
    return True


def matches(got, expect):
    """'News 01' = the focused text contains it; 'a || b' = either; '!x' = must not contain x; '*' = anything."""
    if expect in ("", "*"):
        return True
    if expect.startswith("!"):
        return expect[1:].lower() not in got.lower()
    return any(e.strip().lower() in got.lower() for e in expect.split("||"))


def sheet(shots, path):
    """One contact sheet per group: each case's screenshot with its ID, keys and result."""
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        return
    if not shots:
        return
    w, h, cols = 480, 270, 3
    rows = (len(shots) + cols - 1) // cols
    out = Image.new("RGB", (cols * (w + 4), rows * (h + 4)), "white")
    d = ImageDraw.Draw(out)
    for i, (png, label, ok) in enumerate(shots):
        x, y = (i % cols) * (w + 4), (i // cols) * (h + 4)
        try:
            out.paste(Image.open(png).convert("RGB").resize((w, h)), (x, y))
        except Exception:
            pass
        d.rectangle((x, y + h - 20, x + w, y + h), fill="black")
        d.text((x + 4, y + h - 17), label, fill="lime" if ok else "red")
    out.save(path)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    record = "--record" in sys.argv
    only = args[0] if args else ""
    os.makedirs(OUT, exist_ok=True)
    groups = parse(os.path.join(HERE, "focus_cases.md"))
    rows, failed, total = [], 0, 0
    for g in groups:
        cases = [c for c in g["cases"] if c["id"].startswith(only)]
        if not cases:
            continue
        page, *setup = g["start"] or ["Live"]
        print(f"\n## {g['title']}  (start: {page} {' '.join(setup)})", flush=True)
        start(page, setup)
        shots = []
        for c in cases:
            for k in c["keys"]:
                press(k)
            time.sleep(SETTLE)
            got = focused()
            ok = record or matches(got, c["expect"])
            total += 1
            failed += 0 if ok else 1
            png = adb("exec-out", "screencap -p", binary=True)
            shot = os.path.join(OUT, f"{c['id']}.png")
            open(shot, "wb").write(png)
            mark = "SEEN" if record else "PASS" if ok else "FAIL"
            shots.append((shot, f"{c['id']} {' '.join(c['keys'])} -> {got[:48]}", ok))
            print(f"{mark}  {c['id']:6} {' '.join(c['keys']):18} expect {c['expect']!r:28} got {got[:70]!r}", flush=True)
            rows.append(f"| {mark} | {c['id']} | `{' '.join(c['keys'])}` | {c['expect']} | {got[:90]} |")
        sheet(shots, os.path.join(OUT, f"sheet_{cases[0]['id'].rstrip('0123456789')}.png"))
    report = os.path.join(OUT, "report.md")
    with open(report, "w", encoding="utf-8") as f:
        f.write(f"# Focus test run on {DEV}\n\n{total - failed} of {total} passed.\n\n")
        f.write("| Result | ID | Keys | Expect | Focused |\n|---|---|---|---|---|\n" + "\n".join(rows) + "\n")
    print(f"\n{total - failed} of {total} passed. Report: {report}")
    sys.exit(1 if failed and not record else 0)


if __name__ == "__main__":
    main()
