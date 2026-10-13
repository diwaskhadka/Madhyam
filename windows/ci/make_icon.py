"""Draw the Madhyam app icon (Windows .ico + PNGs). Run: python make_icon.py <out-dir>

Design: a near-black squircle (ArtCraft-style dark, crafted look) holding a screen
outlined with a warm-to-violet gradient, a solid play mark, and an antenna spark.
"""
import math
import os
import sys

from PIL import Image, ImageDraw, ImageFilter

S = 1024


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def gradient(size, stops, angle_deg=35):
    w, h = size
    img = Image.new("RGBA", size)
    px = img.load()
    a = math.radians(angle_deg)
    dx, dy = math.cos(a), math.sin(a)
    proj = [x * dx + y * dy for x, y in ((0, 0), (w, 0), (0, h), (w, h))]
    lo, hi = min(proj), max(proj)
    for y in range(h):
        for x in range(w):
            t = ((x * dx + y * dy) - lo) / (hi - lo)
            for i in range(len(stops) - 1):
                t0, c0 = stops[i]
                t1, c1 = stops[i + 1]
                if t <= t1:
                    px[x, y] = lerp(c0, c1, (t - t0) / max(1e-6, t1 - t0))
                    break
            else:
                px[x, y] = stops[-1][1]
    return img


def squircle_mask(size, radius):
    m = Image.new("L", (size, size), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return m


def build():
    small = 256  # gradients are computed small and scaled up (fast, smooth)
    bg = gradient((small, small), [(0, (24, 24, 30, 255)), (1, (8, 8, 10, 255))], 60).resize((S, S), Image.BICUBIC)
    icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    icon.paste(bg, (0, 0), squircle_mask(S, 230))

    warm = gradient((small, small), [(0, (255, 176, 59, 255)), (0.5, (255, 92, 92, 255)), (1, (155, 92, 255, 255))], 35).resize((S, S), Image.BICUBIC)

    # soft glow behind the screen
    glow = Image.new("L", (S, S), 0)
    ImageDraw.Draw(glow).rounded_rectangle((200, 300, 824, 770), radius=110, fill=150)
    glow = glow.filter(ImageFilter.GaussianBlur(70))
    icon.paste(warm, (0, 0), Image.composite(glow, Image.new("L", (S, S), 0), squircle_mask(S, 230)))
    shade = Image.new("RGBA", (S, S), (10, 10, 12, 170))
    icon.alpha_composite(Image.composite(shade, Image.new("RGBA", (S, S)), squircle_mask(S, 230)))

    # screen outline (gradient stroke)
    ring = Image.new("L", (S, S), 0)
    d = ImageDraw.Draw(ring)
    d.rounded_rectangle((190, 290, 834, 780), radius=120, fill=255)
    d.rounded_rectangle((250, 350, 774, 720), radius=72, fill=0)
    icon.paste(warm, (0, 0), ring)

    # antenna: two strokes meeting at a spark
    ant = Image.new("L", (S, S), 0)
    d = ImageDraw.Draw(ant)
    d.line((512, 290, 390, 160), fill=255, width=46)
    d.line((512, 290, 640, 150), fill=255, width=46)
    for cx, cy, r in ((390, 160, 34), (640, 150, 40)):
        d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=255)
    icon.paste(warm, (0, 0), ant)

    # play mark (white, slightly rounded)
    play = Image.new("L", (S * 2, S * 2), 0)
    ImageDraw.Draw(play).polygon([(880, 820), (880, 1320), (1300, 1070)], fill=255)
    play = play.filter(ImageFilter.GaussianBlur(10)).point(lambda v: 255 if v > 120 else 0).resize((S, S), Image.LANCZOS)
    icon.paste(Image.new("RGBA", (S, S), (255, 255, 255, 255)), (0, 0), play)
    return icon


def main(out):
    os.makedirs(out, exist_ok=True)
    icon = build()
    icon.resize((256, 256), Image.LANCZOS).save(os.path.join(out, "Madhyam-256.png"))
    icon.resize((64, 64), Image.LANCZOS).save(os.path.join(out, "Madhyam-64.png"))
    sizes = [(16, 16), (20, 20), (24, 24), (32, 32), (40, 40), (48, 48), (64, 64), (128, 128), (256, 256)]
    icon.resize((256, 256), Image.LANCZOS).save(os.path.join(out, "Madhyam.ico"), sizes=sizes)
    icon.resize((512, 512), Image.LANCZOS).save(os.path.join(out, "Madhyam-512.png"))


if __name__ == "__main__":
    main(sys.argv[1])
