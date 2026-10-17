"""Render the branding SVGs into each platform's image files. Run: py branding/build.py

Needs Windows with Microsoft Edge (used headless as the SVG renderer, so the Devanagari on the
TV banner is shaped by the system font) and Python 3 only; no packages.

Writes:
  windows/Madhyam.Windows/Assets/app.ico          16-256 px, exe + window + installer icon
  windows/Madhyam.Windows/Assets/app-64.png       sidebar logo
  windows/Madhyam.Windows/Assets/app-256.png
  android/app/src/main/res/drawable-xhdpi/banner.png Android TV home-row banner, 320x180
  branding/png/icon-512.png, banner-1280.png          for store pages, README, new platforms

The Android launcher icon itself is vector (res/drawable*/ic_launcher*.xml), not rendered here.
"""
import os
import struct
import subprocess
import sys
import tempfile
import time
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]

# (output path relative to the repository, source svg, width, height)
OUTPUTS = [(f"ico:{s}", "icon.svg", s, s) for s in ICO_SIZES] + [
    ("windows/Madhyam.Windows/Assets/app-64.png", "icon.svg", 64, 64),
    ("windows/Madhyam.Windows/Assets/app-256.png", "icon.svg", 256, 256),
    ("android/app/src/main/res/drawable-xhdpi/banner.png", "banner.svg", 320, 180),
    ("branding/png/icon-512.png", "icon.svg", 512, 512),
    ("branding/png/banner-1280.png", "banner.svg", 1280, 720),
]
ICO_OUT = "windows/Madhyam.Windows/Assets/app.ico"


def find_edge():
    for base in (os.environ.get("ProgramFiles(x86)"), os.environ.get("ProgramFiles"), os.environ.get("LOCALAPPDATA")):
        if base:
            p = os.path.join(base, "Microsoft", "Edge", "Application", "msedge.exe")
            if os.path.exists(p):
                return p
    sys.exit("Microsoft Edge not found")


def layout():
    """Place every output on one sheet, left to right in rows, 8 px apart."""
    x = y = row_h = 0
    width = 1400
    placed = []
    for name, svg, w, h in OUTPUTS:
        if x + w > width:
            x, y, row_h = 0, y + row_h + 8, 0
        placed.append((name, svg, x, y, w, h))
        x += w + 8
        row_h = max(row_h, h)
    return placed, width, y + row_h


def render(placed, width, height, tmp):
    imgs = "".join(
        f'<img src="file:///{os.path.join(HERE, svg).replace(os.sep, "/")}" '
        f'style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px">'
        for _, svg, x, y, w, h in placed)
    html = os.path.join(tmp, "sheet.html")
    with open(html, "w", encoding="utf-8") as f:
        f.write(f'<!doctype html><meta charset="utf-8"><body style="margin:0;background:transparent">{imgs}</body>')
    png = os.path.join(tmp, "sheet.png")
    subprocess.run([find_edge(), "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=1", "--default-background-color=00000000",
                    f"--user-data-dir={os.path.join(tmp, 'profile')}", f"--window-size={width},{height}",
                    f"--screenshot={png}", "file:///" + html.replace(os.sep, "/")],
                   check=True, capture_output=True)
    for _ in range(150):  # msedge.exe can return before the headless browser has written the file
        if os.path.exists(png) and os.path.getsize(png) > 0:
            time.sleep(0.5)
            return png
        time.sleep(0.2)
    sys.exit("Edge did not write the screenshot")


def read_png(path):
    data = open(path, "rb").read()
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
            assert depth == 8 and ctype in (2, 6), "unexpected PNG format from Edge"
            bpp = 4 if ctype == 6 else 3
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw, stride, rows, prev = zlib.decompress(idat), w * bpp, [], bytearray(w * bpp)
    for r in range(h):
        ft, line = raw[r * (stride + 1)], bytearray(raw[r * (stride + 1) + 1:(r + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], prev[i - bpp] if i >= bpp else 0
            if ft == 1: line[i] = (line[i] + a) & 255
            elif ft == 2: line[i] = (line[i] + b) & 255
            elif ft == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        prev = line
        if bpp == 3:  # opaque screenshot: add alpha
            line = b"".join(bytes(line[i:i + 3]) + b"\xff" for i in range(0, stride, 3))
        rows.append(bytes(line))
    return rows


def write_png(rows, w, h):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    raw = b"".join(b"\0" + r for r in rows)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def write_ico(pngs):
    """ICO with a PNG image per size (supported since Windows Vista)."""
    head = struct.pack("<HHH", 0, 1, len(pngs))
    offset, entries, blobs = 6 + 16 * len(pngs), b"", b""
    for size, data in pngs:
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
        blobs += data
    return head + entries + blobs


def main():
    placed, width, height = layout()
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        sheet = read_png(render(placed, width, height, tmp))
    ico = []
    for name, _, x, y, w, h in placed:
        png = write_png([row[x * 4:(x + w) * 4] for row in sheet[y:y + h]], w, h)
        if name.startswith("ico:"):
            ico.append((w, png))
            continue
        out = os.path.join(ROOT, name)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        open(out, "wb").write(png)
        print("wrote", name)
    open(os.path.join(ROOT, ICO_OUT), "wb").write(write_ico(ico))
    print("wrote", ICO_OUT)


if __name__ == "__main__":
    main()
