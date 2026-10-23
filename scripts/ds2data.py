"""Reading Dark Souls II's own data files, shared by the build scripts.

- GameDataEbl.bhd/.bdt is a BHD5 archive. Its header is RSA-encrypted with the public key the
  game ships next to it (GameDataKeyCode.pem); some entries are AES-ECB encrypted in ranges whose
  keys the header lists. Entries are found by a hash of their lower-case path. Layout and keys
  follow Atvaark/BinderTool.
- enc_regulation.bnd.dcx (AES-CTR over a zlib DCX) is a BND4 of .param files. Their row layout
  comes from soulsmods/Paramdex (DS2S/Defs/*.xml), passed in as a folder of those definitions.
- Menu text is in FMG files inside the archive (menu/text/english/*.fmg), read as BinderTool's
  FmgFile does.

needs: pip install cryptography
"""

import base64
import os
import re
import struct
import xml.etree.ElementTree as ET
import zlib

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

REGULATION_KEY = bytes.fromhex("40178130DF0A94543309E171ECBF254C")


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
            raise SystemExit(f"{name}.bhd did not decrypt; is this a Dark Souls II folder?")
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

    def read_path(self, path):
        """An entry by its path inside the archive, e.g. /menu/text/english/itemname.fmg."""
        h = path_hash(path)
        return self.read(h) if h in self.entries else None


def regulation(game):
    """The regulation's files by name, e.g. {"ItemParam.param": bytes}."""
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


def param_rows(data):
    """{row id: offset of the row's data} of a DS2 .param file."""
    rows = {}
    for i in range(struct.unpack_from("<h", data, 0x0A)[0]):
        rid, off, _ = struct.unpack_from("<qqq", data, 0x40 + 24 * i)
        rows[rid] = off
    return rows


_FORMATS = {"s8": "b", "u8": "B", "s16": "h", "u16": "H", "s32": "i", "u32": "I", "f32": "f"}


class ParamDef:
    """A Paramdex definition: each field's name, struct format and offset. Repeated names get
    a #2, #3... suffix; one-bit fields that share a byte are read as that byte's bits."""

    def __init__(self, xml_path):
        self.fields = []
        offset, bit, seen = 0, None, {}
        for f in ET.parse(xml_path).getroot().iter("Field"):
            # "type name", then ":bits" or "[count]", then "= default". Names may hold colons.
            m = re.match(r"(\w+)\s+(.+?)(?::(\d+))?(?:\[(\d+)\])?\s*(?:=[^=]*)?$", f.get("Def"))
            if not m:
                raise ValueError(f"unreadable field {f.get('Def')!r} in {xml_path}")
            kind, name, bits, count = m.group(1), m.group(2).strip(), m.group(3), m.group(4)
            seen[name] = seen.get(name, 0) + 1
            if seen[name] > 1:
                name = f"{name} #{seen[name]}"
            if kind.startswith("dummy8"):
                bit = None
                offset += int(count or 1)
                continue
            fmt = _FORMATS[kind]
            size = struct.calcsize(fmt)
            if bits:
                if bit is None or bit[1] + int(bits) > size * 8 or bit[2] != fmt:
                    bit = [offset, 0, fmt]
                    offset += size
                self.fields.append((name, fmt, bit[0], (bit[1], int(bits))))
                bit[1] += int(bits)
                continue
            bit = None
            for k in range(int(count or 1)):
                self.fields.append((f"{name}[{k}]" if count else name, fmt, offset, None))
                offset += size
        self.size = offset
        self.by_name = {f[0]: f for f in self.fields}

    def read(self, data, off):
        row = {}
        for name, fmt, at, bits in self.fields:
            v = struct.unpack_from("<" + fmt, data, off + at)[0]
            if bits:
                v = (v >> bits[0]) & ((1 << bits[1]) - 1)
            row[name] = v
        return row


class Param:
    """Rows of one .param file read through its definition: param[row id] -> {field: value}."""

    def __init__(self, data, definition):
        self.data, self.d = data, definition
        self.rows = param_rows(data)

    def __contains__(self, rid):
        return rid in self.rows

    def __getitem__(self, rid):
        return self.d.read(self.data, self.rows[rid])

    def get(self, rid):
        return self[rid] if rid in self.rows else None

    def ids(self):
        return sorted(self.rows)


def fmg(data):
    """{text id: text} of an FMG file. Dark Souls II writes version 1: 32-bit offsets and
    12-byte id ranges; version 2 (BinderTool's FmgFile) widens both."""
    wide = data[2] >= 2
    ranges, strings = struct.unpack_from("<ii", data, 0x0C)
    if wide:
        offsets_at, range_at, range_size, ofmt = struct.unpack_from("<q", data, 0x18)[0], 0x28, 16, "q"
    else:
        offsets_at, range_at, range_size, ofmt = struct.unpack_from("<i", data, 0x14)[0], 0x1C, 12, "i"
    offsets = struct.unpack_from(f"<{strings}{ofmt}", data, offsets_at)
    out = {}
    for r in range(ranges):
        index, first, last = struct.unpack_from("<iii", data, range_at + range_size * r)
        for k in range(last - first + 1):
            at = offsets[index + k]
            if at > 0:
                end = at
                while data[end : end + 2] != b"\0\0":
                    end += 2
                out[first + k] = data[at:end].decode("utf-16-le")
    return out
