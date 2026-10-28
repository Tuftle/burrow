#!/usr/bin/env python3
"""Copies burrow's web app into Wren's built-in extension (pages/burrow), so the browser runs the very same code.

    python tools/sync-to-wren.py [wren source tree]

Wren's own glue (background/vault.js, content/vault.js, the manifest lines) lives in the Wren tree and its overlay
repo, not here. After syncing, refresh the overlay (make_overlay.py) and rebuild the browser UI
(MOZCONFIG=mozconfig.full ./mach build faster).
"""
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(HERE, "..", "web")
TREE = sys.argv[1] if len(sys.argv) > 1 else r"C:\mozilla-source\firefox"
DEST = os.path.join(TREE, "browser", "extensions", "wren", "pages", "burrow")


def main():
    if os.path.isdir(DEST):
        shutil.rmtree(DEST)
    shutil.copytree(WEB, DEST)
    count = sum(len(files) for _, _, files in os.walk(DEST))
    print("copied", count, "files to", os.path.normpath(DEST))


if __name__ == "__main__":
    main()
