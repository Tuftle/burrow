#!/usr/bin/env python3
"""Builds the browser extension for Firefox (Manifest V2) and Chrome (Manifest V3) from web/ and extension/.

    python tools/build-extension.py [output folder]

Each browser gets a folder (extension-firefox/, extension-chrome/) and a zip. The app itself is web/ copied unchanged
into app/, so the extension runs the very same code as the website and the Windows app. match.js is also written out as
match.classic.js, because the background script is a classic script, not a module.
"""
import os
import re
import shutil
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, ".."))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "extension", "dist")
sys.path.insert(0, HERE)
import make_logo  # noqa: E402


def version():
    toml = open(os.path.join(ROOT, "desktop", "Cargo.toml"), encoding="utf-8").read()
    return re.search(r'^version = "([^"]+)"', toml, re.M).group(1)


def classic_match():
    """match.js without `export`, hung on `self.burrowMatch`, for scripts that cannot import modules."""
    src = open(os.path.join(ROOT, "web", "match.js"), encoding="utf-8").read()
    src = re.sub(r"^export ", "", src, flags=re.M)
    return src + "\nself.burrowMatch = { entryHost, matches };\n"


def build(browser, ver):
    folder = os.path.join(OUT, f"extension-{browser}")
    if os.path.isdir(folder):
        shutil.rmtree(folder)
    os.makedirs(folder)
    shutil.copytree(os.path.join(ROOT, "web"), os.path.join(folder, "app"))
    for name in ("background.js", "content.js"):
        shutil.copy(os.path.join(ROOT, "extension", "src", name), folder)
    open(os.path.join(folder, "match.classic.js"), "w", encoding="utf-8").write(classic_match())
    manifest = open(os.path.join(ROOT, "extension", f"manifest.{browser}.json"), encoding="utf-8").read().replace("__VERSION__", ver)
    open(os.path.join(folder, "manifest.json"), "w", encoding="utf-8").write(manifest)
    os.makedirs(os.path.join(folder, "icons"))
    for size in (16, 32, 48, 128):
        make_logo.draw(size, detail=size >= 32).save(os.path.join(folder, "icons", f"icon-{size}.png"))
    zpath = os.path.join(OUT, f"Burrow-{browser.capitalize()}.zip")
    if os.path.exists(zpath):
        os.remove(zpath)
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
        for base, _, files in os.walk(folder):
            for f in files:
                full = os.path.join(base, f)
                z.write(full, os.path.relpath(full, folder).replace(os.sep, "/"))
    print(browser, "->", zpath, round(os.path.getsize(zpath) / 1024), "KB")


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    v = version()
    for b in ("firefox", "chrome"):
        build(b, v)
