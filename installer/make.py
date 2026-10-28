#!/usr/bin/env python3
"""Builds the release files:

  Burrow-Setup.exe     the installer window with burrow.exe stuck on its end, followed by
                       [burrow.exe length: u64]["BURROWSP"]
  Burrow-Portable.exe  burrow.exe on its own. A file name with "portable" in it keeps the vault and the web view's
                       cache in a burrow-data folder next to the exe.
  SHA256SUMS.txt

    (cd ../desktop && cargo build --release --features custom-protocol)
    cargo build --release
    python make.py [output folder]
"""
import hashlib
import os
import shutil
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
STUB = os.path.join(HERE, "target", "release", "burrow-setup.exe")
APP = os.path.join(HERE, "..", "desktop", "target", "release", "burrow.exe")
OUT = sys.argv[1] if len(sys.argv) > 1 else r"I:\Documents\code\burrow-release"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    os.makedirs(OUT, exist_ok=True)
    setup = os.path.join(OUT, "Burrow-Setup.exe")
    portable = os.path.join(OUT, "Burrow-Portable.exe")
    with open(setup, "wb") as out:
        with open(STUB, "rb") as f:
            out.write(f.read())
        with open(APP, "rb") as f:
            data = f.read()
        out.write(data)
        out.write(struct.pack("<Q", len(data)))
        out.write(b"BURROWSP")
    shutil.copyfile(APP, portable)
    lines = []
    for path in (setup, portable):
        lines.append(f"{sha256(path)}  {os.path.basename(path)}")
        print(os.path.basename(path), round(os.path.getsize(path) / 1e6, 1), "MB")
    with open(os.path.join(OUT, "SHA256SUMS.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
