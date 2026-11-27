"""Derives every branded asset from the flat logo (run make_logo.py first).

- Inno Setup wizard images (side panel + header) at 100/150/200 % scaling
- In-app logo, website logo, favicons, Open Graph card
Run: python3 branding/make_assets.py
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
B = ROOT / "branding"
VIOLET = (124, 77, 255)
NIGHT = (14, 10, 36)
LILAC = (196, 181, 255)

mark_white = Image.open(B / "mellogram-mark-white.png").convert("RGBA")
mark_violet = Image.open(B / "mellogram-mark-violet.png").convert("RGBA")
app_icon = Image.open(B / "mellogram-app-icon.png").convert("RGBA")


def font(size, bold=True):
    for p in ("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf" if bold else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",):
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            pass
    return ImageFont.load_default()


def centered_text(d, cx, y, text, f, fill):
    w = d.textlength(text, font=f)
    d.text((cx - w / 2, y), text, font=f, fill=fill)


# ---- Inno Setup ------------------------------------------------------------
inno = ROOT / "installer" / "windows" / "art"
inno.mkdir(parents=True, exist_ok=True)
for scale, tag in ((1, "100"), (1.5, "150"), (2, "200")):
    w, h = int(164 * scale), int(314 * scale)
    side = Image.new("RGB", (w, h), NIGHT)
    d = ImageDraw.Draw(side)
    d.rectangle([0, h - int(10 * scale), w, h], fill=VIOLET)          # flat brand stripe
    m = int(118 * scale)
    side.paste(mark_white.resize((m, m), Image.LANCZOS), ((w - m) // 2, int(64 * scale)), mark_white.resize((m, m), Image.LANCZOS))
    centered_text(d, w / 2, int(196 * scale), "MelloGram", font(int(21 * scale)), (255, 255, 255))
    centered_text(d, w / 2, int(226 * scale), "customizable", font(int(11 * scale), False), LILAC)
    centered_text(d, w / 2, int(242 * scale), "Telegram client", font(int(11 * scale), False), LILAC)
    side.save(inno / f"wizard-side-{tag}.bmp")

    s = int(55 * scale)
    small = Image.new("RGB", (s, s), (255, 255, 255))
    ic = app_icon.resize((s, s), Image.LANCZOS)
    small.paste(ic, (0, 0), ic)
    small.save(inno / f"wizard-small-{tag}.bmp")

# ---- app UI ------------------------------------------------------------------
app_icon.resize((256, 256), Image.LANCZOS).save(ROOT / "app" / "ui" / "logo.png", optimize=True)

# ---- website -----------------------------------------------------------------
site = ROOT / "site" / "src" / "assets"
app_icon.resize((256, 256), Image.LANCZOS).save(site / "logo.png", optimize=True)
for size, name in ((32, "favicon-32.png"), (180, "apple-touch-icon.png"), (192, "icon-192.png"), (512, "icon-512.png")):
    app_icon.resize((size, size), Image.LANCZOS).save(site / name, optimize=True)
app_icon.save(site / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)])
mark_violet.resize((640, 640), Image.LANCZOS).save(site / "mark.png", optimize=True)

# Open Graph card: flat night background, white mark, wordmark, real screenshot.
og = Image.new("RGB", (1200, 630), NIGHT)
d = ImageDraw.Draw(og)
d.rectangle([0, 600, 1200, 630], fill=VIOLET)
shot_src = ROOT / "site" / "src" / "assets" / "screenshot-chat.webp"
if shot_src.exists():
    shot = Image.open(shot_src).convert("RGB").resize((620, 388), Image.LANCZOS)
    mask = Image.new("L", shot.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, 620, 388], radius=16, fill=255)
    og.paste(shot, (540, 120), mask)
m = mark_white.resize((150, 150), Image.LANCZOS)
og.paste(m, (70, 80), m)
d.text((70, 260), "MelloGram", font=font(64), fill=(255, 255, 255))
d.text((70, 345), "The customizable", font=font(32, False), fill=LILAC)
d.text((70, 388), "Telegram client", font=font(32, False), fill=LILAC)
d.text((70, 470), "Plugins · Themes · Wallpapers", font=font(24, False), fill=(167, 139, 255))
d.text((70, 505), "Windows · macOS · Linux · Android", font=font(24, False), fill=(167, 139, 255))
og.save(site / "og-image.png", optimize=True)
print("assets written")
