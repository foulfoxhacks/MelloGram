#!/usr/bin/env python3
"""Generate Android launcher icons from the MelloGram branding.

Writes app/src-tauri/icons/android/, which scripts/configure-android.py copies
into the generated Gradle project (gen/android/app/src/main/res).

* ic_launcher.png / ic_launcher_round.png: legacy (pre-Android 8) icons.
* ic_launcher_foreground.png + @color/ic_launcher_background: adaptive icon
  (Android 8+), with the mark kept inside the 66dp safe zone so launcher
  masks (circle, squircle, teardrop) never clip it.
* ic_launcher_monochrome.png: Android 13+ themed icon.
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
BRANDING = ROOT / "branding"
OUT = ROOT / "app/src-tauri/icons/android"
VIOLET = (124, 77, 255, 255)  # #7C4DFF, the app icon background
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}

app_icon = Image.open(BRANDING / "mellogram-app-icon.png").convert("RGBA")
mark = Image.open(BRANDING / "mellogram-mark-white.png").convert("RGBA")
mark = mark.crop(mark.getbbox())


def centered_mark(size: int, fraction: float) -> Image.Image:
    """Transparent square canvas with the white mark centred at `fraction`."""
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    scale = fraction * size / max(mark.size)
    m = mark.resize((max(1, round(mark.width * scale)), max(1, round(mark.height * scale))), Image.LANCZOS)
    canvas.alpha_composite(m, ((size - m.width) // 2, (size - m.height) // 2))
    return canvas


for name, factor in DENSITIES.items():
    d = OUT / f"mipmap-{name}"
    d.mkdir(parents=True, exist_ok=True)
    legacy = round(48 * factor)
    adaptive = round(108 * factor)

    app_icon.resize((legacy, legacy), Image.LANCZOS).save(d / "ic_launcher.png", optimize=True)

    big = legacy * 4
    round_icon = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(round_icon).ellipse((0, 0, big - 1, big - 1), fill=VIOLET)
    round_icon.alpha_composite(centered_mark(big, 0.62))
    round_icon.resize((legacy, legacy), Image.LANCZOS).save(d / "ic_launcher_round.png", optimize=True)

    # 66dp safe zone of a 108dp canvas; the mark's corners stay inside it.
    fg = centered_mark(adaptive, 0.50)
    fg.save(d / "ic_launcher_foreground.png", optimize=True)
    fg.save(d / "ic_launcher_monochrome.png", optimize=True)

adaptive_xml = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
  <background android:drawable="@color/ic_launcher_background"/>
  <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
  <monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>
</adaptive-icon>
"""
anydpi = OUT / "mipmap-anydpi-v26"
anydpi.mkdir(parents=True, exist_ok=True)
(anydpi / "ic_launcher.xml").write_text(adaptive_xml, encoding="utf-8")
(anydpi / "ic_launcher_round.xml").write_text(adaptive_xml, encoding="utf-8")

values = OUT / "values"
values.mkdir(parents=True, exist_ok=True)
(values / "ic_launcher_background.xml").write_text(
    '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n  <color name="ic_launcher_background">#7C4DFF</color>\n</resources>\n',
    encoding="utf-8",
)
print(f"Android launcher icons written to {OUT.relative_to(ROOT)}")
