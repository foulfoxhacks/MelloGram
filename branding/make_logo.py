"""Builds the flat-colour MelloGram logo from the original artwork.

The original has gradients, glow and a fading outer ring. The flat version
keeps the silhouette (fox ears, paw pads, paper plane, tail, sparkles), drops
glow and gradients, and replaces the faded ring fragments with an optional
crisp full ring. Run: python3 branding/make_logo.py <original-image>
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

OUT = Path(__file__).parent
VIOLET = (124, 77, 255)      # #7C4DFF  primary flat colour
NIGHT = (14, 10, 36)         # #0E0A24  dark tile
WHITE = (255, 255, 255)

src = Image.open(sys.argv[1]).convert("RGB")
W = src.size[0]
S = 4
mx = Image.fromarray(np.asarray(src).max(axis=2).astype(np.uint8))
big = mx.resize((W * S, W * S), Image.BICUBIC).filter(ImageFilter.GaussianBlur(S * 1.2)).point(lambda v: 255 if v >= 140 else 0)
big = big.filter(ImageFilter.GaussianBlur(S * 0.6)).point(lambda v: 255 if v >= 128 else 0)
# Morphological closing removes the ragged teeth along the plane's fold while
# leaving real gaps (ear interiors, pad spacing) open.
big = big.filter(ImageFilter.MaxFilter(15)).filter(ImageFilter.MinFilter(15))
big = big.filter(ImageFilter.GaussianBlur(S * 0.8)).point(lambda v: 255 if v >= 128 else 0)

# Label connected components (4-neighbour) and drop the faded ring arcs:
# long, thin, elongated pieces. Compact shapes (pads, sparkles) stay.
small = np.asarray(big.resize((W, W), Image.NEAREST)) > 0
lab = np.zeros(small.shape, np.int32)
remove, n = [], 0
for y in range(W):
    for x in np.nonzero(small[y] & (lab[y] == 0))[0]:
        if lab[y, x]:
            continue
        n += 1
        stack = [(y, x)]
        lab[y, x] = n
        pts, y0, y1, x0, x1 = 0, y, y, x, x
        while stack:
            cy, cx = stack.pop()
            pts += 1
            y0, y1, x0, x1 = min(y0, cy), max(y1, cy), min(x0, cx), max(x1, cx)
            for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < W and 0 <= nx < W and small[ny, nx] and not lab[ny, nx]:
                    lab[ny, nx] = n
                    stack.append((ny, nx))
        bw, bh = x1 - x0 + 1, y1 - y0 + 1
        elongated = max(bw, bh) / max(1, min(bw, bh)) > 4
        if pts < 40 or (elongated and max(bw, bh) > 80) or (max(bw, bh) > 120 and pts / (bw * bh) < 0.12):
            remove.append(n)

keep = (~np.isin(lab, remove)) & small
keep_big = Image.fromarray((keep * 255).astype(np.uint8)).resize((W * S, W * S), Image.NEAREST)
mask = ImageChops.multiply(big, keep_big).resize((W, W), Image.LANCZOS)

# Tight, centred square crop of the mark with padding.
bx0, by0, bx1, by1 = mask.getbbox()
side = max(bx1 - bx0, by1 - by0)
cx, cy = (bx0 + bx1) // 2, (by0 + by1) // 2
pad = int(side * 0.06)
half = side // 2 + pad
mark = mask.crop((cx - half, cy - half, cx + half, cy + half))
M = mark.size[0]

# Redraw the paper-plane fold as a crisp triangle (the original's lower edge
# is scalloped). Coordinates are measured on the 1024 px mark.
def fold_fix(m):
    size = m.size[0]
    k = size / 1024
    big = m.resize((size * 4, size * 4), Image.BICUBIC)
    d = ImageDraw.Draw(big)
    tip, left, bottom = (713, 460), (364, 629), (460, 671)
    s4 = lambda pt, dx=0, dy=0: ((pt[0] + dx) * k * 4, (pt[1] + dy) * k * 4)
    d.polygon([s4(tip, 20, -16), s4(left, -14, -10), s4(bottom, 10, 14), s4((590, 575), 18, 10), s4((690, 492), 16, 10)], fill=255)   # cover the old wedge
    d.polygon([s4(tip), s4(left), s4(bottom)], fill=0)                        # cut the clean wedge
    return big.resize((size, size), Image.LANCZOS)

mark = fold_fix(mark)




def fill(mask_img, color, size):
    m = mask_img.resize((size, size), Image.LANCZOS)
    img = Image.new("RGBA", (size, size), color + (0,))
    img.putalpha(m)
    return img


# 1) Flat marks on transparent background.
for name, color in (("mellogram-mark-violet", VIOLET), ("mellogram-mark-white", WHITE), ("mellogram-mark-night", NIGHT)):
    fill(mark, color, 1024).save(OUT / f"{name}.png", optimize=True)

# 2) Badge: mark inside a crisp full ring (replaces the faded original ring).
def badge(color, size=1024):
    img = Image.new("RGBA", (size * 2, size * 2), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    stroke = int(size * 2 * 0.026)
    d.ellipse([stroke, stroke, size * 2 - stroke, size * 2 - stroke], outline=color + (255,), width=stroke)
    inner = int(size * 2 * 0.74)
    m = fill(mark, color, inner)
    off = (size * 2 - inner) // 2
    img.alpha_composite(m, (off, off))
    return img.resize((size, size), Image.LANCZOS)

badge(VIOLET).save(OUT / "mellogram-badge-violet.png", optimize=True)
badge(WHITE).save(OUT / "mellogram-badge-white.png", optimize=True)

# 3) App icon: flat violet tile with a white mark (reads at 16 px and on any wallpaper).
def app_icon(size=1024):
    img = Image.new("RGBA", (size * 2, size * 2), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    r = int(size * 2 * 0.225)
    d.rounded_rectangle([0, 0, size * 2 - 1, size * 2 - 1], radius=r, fill=VIOLET + (255,))
    inner = int(size * 2 * 0.80)
    m = fill(mark, WHITE, inner)
    off = (size * 2 - inner) // 2
    img.alpha_composite(m, (off, off))
    return img.resize((size, size), Image.LANCZOS)

icon = app_icon()
icon.save(OUT / "mellogram-app-icon.png", optimize=True)
print("removed components:", len(remove), "mark size:", M)
