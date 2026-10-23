"""Builds the inventory icons (src/public/icons) from an installed copy of Dark Souls II.

Where the game keeps them (archive and regulation reading live in ds2data.py):
- Each item icon is a one-texture TPF named "<KIND>_IC_<icon id>" in GameDataEbl (HQ_ variants
  are 128x256, the others 64x128), DXT5.
- The regulation's ItemParam: row id = item id, s32 at +0 = icon id, u16 at +0x4A = how many
  the player can hold (field names from soulsmods/Paramdex).

Writes src/public/icons/<icon id>.webp (each distinct icon once, cropped to its 2:3 card) and
src-tauri/data/item_params.json ({item id: [icon id, max held]}).

usage: python scripts/build-item-icons.py "<game folder>"
needs: pip install cryptography pillow
"""

import io
import json
import os
import re
import struct
import sys

from PIL import Image

from ds2data import Archive, param_rows, regulation

ICON_SIZE = (96, 144)  # 2:3, the shape of the art inside the game's 1:2 textures
CROP = (0, 32, 128, 224)  # in 128x256 texture space


def item_params(game):
    data = regulation(game)["ItemParam.param"]
    return {
        rid: (struct.unpack_from("<i", data, off)[0], struct.unpack_from("<H", data, off + 0x4A)[0])
        for rid, off in param_rows(data).items()
    }


def icon_textures(archive):
    """icon id -> (is HQ, entry hash) for every item icon in the archive, HQ preferred."""
    found = {}
    for h, (size, _, _) in archive.entries.items():
        if size > 200_000:
            continue
        head = archive.read(h)[:0x200]
        if head[:4] != b"TPF\0":
            continue
        m = re.search(rb"(HQ_)?[A-Z]{2}_IC_(\d{10})", head)
        if not m:
            continue
        icon, hq = int(m.group(2)), m.group(1) is not None
        if icon not in found or (hq and not found[icon][0]):
            found[icon] = (hq, h)
    return found


def card(tpf):
    image = Image.open(io.BytesIO(tpf[tpf.find(b"DDS ") :]))
    image.load()
    image = image.convert("RGBA")
    if image.size != (128, 256):
        image = image.resize((128, 256), Image.LANCZOS)
    return image.crop(CROP).resize(ICON_SIZE, Image.LANCZOS)


def main(game):
    items = json.load(open("src-tauri/data/items.json", encoding="utf-8"))
    params = item_params(game)
    archive = Archive(game)
    textures = icon_textures(archive)

    os.makedirs("src/public/icons", exist_ok=True)
    mapping, written, missing = {}, set(), []
    for item in items:
        iid = item["id"]
        icon, held = params.get(iid, (iid, 0))
        if icon not in textures and item["cat"] == "ring":
            icon = params.get(iid - iid % 10, (iid - iid % 10, 0))[0]
        if icon not in textures:
            missing.append(f"{iid} {item['name']}")
            mapping[str(iid)] = [None, held]
            continue
        mapping[str(iid)] = [icon, held]
        if icon not in written:
            card(archive.read(textures[icon][1])).save(f"src/public/icons/{icon}.webp", "WEBP", quality=82, method=6)
            written.add(icon)

    with open("src-tauri/data/item_params.json", "w", encoding="utf-8") as f:
        json.dump(mapping, f, separators=(",", ":"), sort_keys=True)
    total = sum(os.path.getsize(f"src/public/icons/{i}.webp") for i in written)
    print(f"{len(written)} icons ({total / 1e6:.1f} MB) for {len(items) - len(missing)} of {len(items)} items")
    print("without icon:", "; ".join(missing))


if __name__ == "__main__":
    main(sys.argv[1])
