#!/usr/bin/env python3
"""Draws burrow's logo once and writes every size it is used at.

A brown tile, a cream hill rising from the bottom edge, a round door cut out of the hill, and (when there is room) a
keyhole and a green sprout. Small sizes drop the fine parts so the shape still reads at 16 px.

    python tools/make_logo.py [preview.png]

Writes web/assets/logo.svg, desktop/icons/icon.ico and icon.png, installer/burrow.ico.
"""
import io
import os
import struct
import sys

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")

BROWN = (138, 75, 42, 255)      # --link
CREAM = (244, 239, 230, 255)    # --bg
DOOR = (94, 58, 34, 255)        # a darker brown, the inside of the door
SPROUT = (168, 196, 128, 255)   # a leaf green that still shows on brown
AMBER = (255, 198, 110, 255)


def draw(size, detail=True):
    """One logo at `size` px, drawn at 8x and shrunk so the edges are smooth."""
    S = size * 8
    k = S / 64
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([0, 0, S - 1, S - 1], radius=14 * k, fill=BROWN)

    layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ld = ImageDraw.Draw(layer)
    # the hill: a wide dome that runs off the bottom edge
    ld.ellipse([6 * k, 26 * k, 58 * k, 98 * k], fill=CREAM)
    # the door: straight sides, round top, running off the bottom
    ld.rounded_rectangle([24 * k, 36 * k, 40 * k, 72 * k], radius=8 * k, fill=DOOR)
    if detail:
        ld.ellipse([30 * k, 46 * k, 34 * k, 50 * k], fill=AMBER)
        ld.polygon([(31 * k, 49 * k), (33 * k, 49 * k), (34 * k, 56 * k), (30 * k, 56 * k)], fill=AMBER)
    # the sprout on top of the hill: a short stem and two slim leaves (a little bolder when small)
    g = 1.0 if detail else 1.35
    ld.line([(32 * k, 27 * k), (32 * k, 17 * k)], fill=SPROUT, width=int((3.2 if detail else 4.2) * k))
    ld.ellipse([(27.5 - 4.6 * g) * k, (16 - 3 * g) * k, (27.5 + 4.6 * g) * k, (16 + 3 * g) * k], fill=SPROUT)
    ld.ellipse([(36.5 - 4.6 * g) * k, (13 - 3 * g) * k, (36.5 + 4.6 * g) * k, (13 + 3 * g) * k], fill=SPROUT)
    # keep everything inside the tile's rounded corners
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=14 * k, fill=255)
    im.alpha_composite(Image.composite(layer, Image.new("RGBA", (S, S), (0, 0, 0, 0)), mask))
    return im.resize((size, size), Image.LANCZOS)


def png_bytes(im):
    b = io.BytesIO()
    im.save(b, "PNG")
    return b.getvalue()


def write_ico(path, sizes=(256, 128, 64, 48, 32, 24, 16)):
    """An .ico with a separately drawn image per size (Pillow would only shrink the biggest one)."""
    blobs = [(s, png_bytes(draw(s, detail=s >= 32))) for s in sizes]
    head = struct.pack("<HHH", 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    table = b""
    data = b""
    for s, blob in blobs:
        table += struct.pack("<BBBBHHII", 0 if s >= 256 else s, 0 if s >= 256 else s, 0, 0, 1, 32, len(blob), offset + len(data))
        data += blob
    with open(path, "wb") as f:
        f.write(head + table + data)


SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><clipPath id="t"><rect width="64" height="64" rx="14"/></clipPath></defs>
  <rect width="64" height="64" rx="14" fill="#8a4b2a"/>
  <g clip-path="url(#t)">
    <ellipse cx="32" cy="62" rx="26" ry="36" fill="#f4efe6"/>
    <rect x="24" y="36" width="16" height="36" rx="8" fill="#5e3a22"/>
    <circle cx="32" cy="48" r="2.2" fill="#ffc66e"/>
    <path d="M31 49h2l1 7h-4z" fill="#ffc66e"/>
    <path d="M32 27V17" stroke="#a8c480" stroke-width="3.2" stroke-linecap="round" fill="none"/>
    <ellipse cx="27.5" cy="16" rx="4.6" ry="3" fill="#a8c480"/>
    <ellipse cx="36.5" cy="13" rx="4.6" ry="3" fill="#a8c480"/>
  </g>
</svg>
"""


def preview(path):
    """Old versus new, at the sizes that matter, on light and dark."""
    sizes = [256, 64, 48, 32, 24, 16]
    W = sum(s + 24 for s in sizes) + 24
    sheet = Image.new("RGB", (W, 2 * 300 + 20), (244, 239, 230))
    ImageDraw.Draw(sheet).rectangle([0, 310, W, 620], fill=(33, 28, 23))
    for row, bg in enumerate([(244, 239, 230), (33, 28, 23)]):
        x = 24
        for s in sizes:
            im = draw(s, detail=s >= 32)
            sheet.paste(im, (x, row * 310 + 20 + (256 - s) // 2), im)
            x += s + 24
    sheet.save(path)


if __name__ == "__main__":
    write_ico(os.path.join(ROOT, "desktop", "icons", "icon.ico"))
    write_ico(os.path.join(ROOT, "installer", "burrow.ico"))
    draw(256).save(os.path.join(ROOT, "desktop", "icons", "icon.png"))
    with open(os.path.join(ROOT, "web", "assets", "logo.svg"), "w", encoding="utf-8") as f:
        f.write(SVG)
    preview(sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "logo-preview.png"))
    print("logo written")
