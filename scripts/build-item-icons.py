"""Builds the inventory icons (src/public/icons) from an installed copy of Dark Souls II.

Where the game keeps them:
- GameDataEbl.bhd/.bdt is a BHD5 archive. Its header is RSA-encrypted with the public key the
  game ships next to it (GameDataKeyCode.pem); some entries are AES-ECB encrypted in ranges whose
  keys the header lists. Layout and keys follow Atvaark/BinderTool.
- Each item icon is a one-texture TPF named "<KIND>_IC_<icon id>" (HQ_ variants are 128x256,
  the others 64x128), DXT5.
- enc_regulation.bnd.dcx (AES-CTR) holds ItemParam: row id = item id, s32 at +0 = icon id,
  u16 at +0x4A = how many the player can hold (field names from soulsmods/Paramdex).

Writes src/public/icons/<icon id>.webp (each distinct icon once, cropped to its 2:3 card) and
src-tauri/data/item_params.json ({item id: [icon id, max held]}).

usage: python scripts/build-item-icons.py "<game folder>"
needs: pip install cryptography pillow
"""

import base64
import io
import json
import os
import re
import struct
import sys
import zlib

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from PIL import Image

REGULATION_KEY = bytes.fromhex("40178130DF0A94543309E171ECBF254C")
ICON_SIZE = (96, 144)  # 2:3, the shape of the art inside the game's 1:2 textures
CROP = (0, 32, 128, 224)  # in 128x256 texture space


def rsa_public_key(pem_path):
    body = "".join(l.strip() for l in open(pem_path) if l.strip() and not l.startswith("-----"))
    der = base64.b64decode(body)

    def length(i):
        n = der[i]
        i += 1
        if n & 0x80:
            k = n & 0x7F
            n = int.from_bytes(der[i : i + k], "big")
            i += k
        return n, i

    _, i = length(1)
    ints = []
    while i < len(der):
        n, i = length(i + 1)
        ints.append(int.from_bytes(der[i : i + n], "big"))
        i += n
    return ints[0], ints[1]


def path_hash(path):
    h = 0
    for c in path.replace("\\", "/").lower():
        h = (h * 37 + ord(c)) & 0xFFFFFFFF
    return h


class Archive:
    def __init__(self, game, name="GameDataEbl"):
        n, e = rsa_public_key(os.path.join(game, name.replace("Ebl", "") + "KeyCode.pem"))
        size = (n.bit_length() + 7) // 8
        raw = open(os.path.join(game, name + ".bhd"), "rb").read()
        hdr = b"".join(pow(int.from_bytes(raw[i : i + size], "big"), e, n).to_bytes(size - 1, "big") for i in range(0, len(raw), size))
        if hdr[:4] != b"BHD5":
            raise SystemExit("GameDataEbl.bhd did not decrypt; is this a Dark Souls II folder?")
        buckets, bucket_at = struct.unpack_from("<ii", hdr, 16)
        self.entries = {}
        for b in range(buckets):
            count, at = struct.unpack_from("<ii", hdr, bucket_at + 8 * b)
            for k in range(count):
                h, fsize, off, _sha, aes_at = struct.unpack_from("<IIqqq", hdr, at + 32 * k)
                aes = None
                if aes_at:
                    rc = struct.unpack_from("<i", hdr, aes_at + 16)[0]
                    aes = (hdr[aes_at : aes_at + 16], [struct.unpack_from("<qq", hdr, aes_at + 20 + 16 * r) for r in range(rc)])
                self.entries[h] = (fsize, off, aes)
        self.bdt = open(os.path.join(game, name + ".bdt"), "rb")

    def read(self, h):
        size, off, aes = self.entries[h]
        self.bdt.seek(off)
        data = bytearray(self.bdt.read(size))
        if aes:
            dec = Cipher(algorithms.AES(aes[0]), modes.ECB()).decryptor()
            for s, e in aes[1]:
                if s >= 0 and e > s:
                    data[s:e] = dec.update(bytes(data[s:e]))
        return bytes(data)


def regulation(game):
    raw = open(os.path.join(game, "enc_regulation.bnd.dcx"), "rb").read()
    iv = b"\x80" + raw[0:11] + b"\x00\x00\x00\x01"
    data = Cipher(algorithms.AES(REGULATION_KEY), modes.CTR(iv)).decryptor().update(raw[32:])
    start = min(i for i in (data.find(b"\x78\xda"), data.find(b"\x78\x9c")) if i >= 0)
    bnd = zlib.decompress(data[start:])
    count = struct.unpack_from("<i", bnd, 12)[0]
    header = struct.unpack_from("<q", bnd, 0x20)[0]
    files = {}
    for i in range(count):
        o = 0x40 + header * i
        size = struct.unpack_from("<q", bnd, o + 8)[0]
        # 36-byte headers carry an uncompressed size and an id before the name offset.
        offset = struct.unpack_from("<I", bnd, o + (24 if header >= 36 else 16))[0]
        name_at = struct.unpack_from("<I", bnd, o + (32 if header >= 36 else 20))[0]
        if bnd[0x30] == 1:
            end = name_at
            while bnd[end : end + 2] != b"\0\0":
                end += 2
            name = bnd[name_at:end].decode("utf-16-le")
        else:
            name = bnd[name_at : bnd.index(b"\0", name_at)].decode("shift_jis", "replace")
        files[name.replace("\\", "/").split("/")[-1]] = bnd[offset : offset + size]
    return files


def item_params(game):
    data = regulation(game)["ItemParam.param"]
    rows = {}
    for i in range(struct.unpack_from("<h", data, 0x0A)[0]):
        rid, off, _ = struct.unpack_from("<qqq", data, 0x40 + 24 * i)
        rows[rid] = (struct.unpack_from("<i", data, off)[0], struct.unpack_from("<H", data, off + 0x4A)[0])
    return rows


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
