"""Builds src/public/fonts/ds2-menu.woff2 from Dark Souls II's own menu font.

The game ships its English menu font as a bitmap atlas: font/English/FeFont_Big.fontbnd.dcx is a
DCX-compressed BND4 holding FeFont_Big.ccm (the glyph table) and DXT5 textures. This script cuts
every Latin glyph out of the atlas, traces it into outlines with potrace and writes a web font
whose metrics follow the game's own advances. Turkish letters the game never needed (ğ ş ı İ ...)
are composed from the game's own base letters and marks.

usage: python scripts/build-game-font.py "<game folder>" src/public/fonts/ds2-menu.woff2
needs: pip install fonttools brotli potracer pillow numpy
"""

import io
import struct
import sys
import zlib

import numpy as np
import potrace
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen
from PIL import Image

UPSCALE = 8
UNITS = 1000
CAP_UNITS = 650  # cap height in font units; close to Garamond's proportions


def dcx_payload(path):
    data = open(path, "rb").read()
    start = data.find(b"\x78\xda")
    if start < 0:
        start = data.find(b"\x78\x9c")
    return zlib.decompress(data[start:])


def bnd4_files(data):
    count = struct.unpack_from("<i", data, 12)[0]
    header = struct.unpack_from("<q", data, 0x20)[0]
    files = {}
    for i in range(count):
        o = 0x40 + header * i
        size = struct.unpack_from("<q", data, o + 8)[0]
        offset = struct.unpack_from("<I", data, o + 24)[0]
        name_at = struct.unpack_from("<I", data, o + 32)[0]
        name = data[name_at : data.index(b"\0", name_at)].decode().replace("\\", "/").split("/")[-1]
        files[name] = data[offset : offset + size]
    return files


def texture(tpf):
    image = Image.open(io.BytesIO(tpf[tpf.find(b"DDS ") :]))
    image.load()
    return np.asarray(image.convert("RGBA"), dtype=np.float32) / 255.0


def parse_ccm(ccm):
    region_count = struct.unpack_from("<h", ccm, 0x0E)[0]
    glyph_count = struct.unpack_from("<I", ccm, 0x10)[0]
    regions_at = struct.unpack_from("<I", ccm, 0x14)[0]
    glyphs_at = struct.unpack_from("<I", ccm, 0x18)[0]
    regions = [struct.unpack_from("<hhhh", ccm, regions_at + 8 * i) for i in range(region_count)]
    glyphs = {}
    for i in range(glyph_count):
        code, region_at, tex, pre, width, advance = struct.unpack_from("<IIhhhh", ccm, glyphs_at + 24 * i)
        glyphs[code] = {
            "region": regions[(region_at - regions_at) // 8],
            "tex": tex,
            "pre": pre,
            "advance": advance,
        }
    return glyphs


def coverage(glyph, textures):
    x1, y1, x2, y2 = glyph["region"]
    rgba = textures[glyph["tex"]][y1:y2, x1:x2]
    # White glyphs on transparency: alpha is the coverage.
    return rgba[..., 3] * rgba[..., :3].mean(axis=2)


def ink_rows(cov):
    rows = np.where(cov.max(axis=1) > 0.35)[0]
    return (rows.min(), rows.max()) if len(rows) else (None, None)


def trace(cov):
    h, w = cov.shape
    big = Image.fromarray((cov * 255).astype(np.uint8)).resize((w * UPSCALE, h * UPSCALE), Image.LANCZOS)
    bits = np.asarray(big) > 150
    if not bits.any():
        return []
    # potracer fills the False pixels (dark-on-light convention), so hand it the inverse.
    return potrace.Bitmap(~bits).trace(turdsize=6, alphamax=1.0, opticurve=True, opttolerance=0.25)


def compose(base, mark, mark_dx=0, mark_dy=0, clear_top=None):
    """Draws `mark` onto a copy of `base` (same cell height); used for missing Turkish letters."""
    out = base.copy()
    if clear_top is not None:
        out[:clear_top, :] = 0
    if mark is not None:
        h, w = mark.shape
        ox = max(0, (out.shape[1] - w) // 2 + mark_dx)
        for y in range(h):
            ty = y + mark_dy
            if 0 <= ty < out.shape[0]:
                seg = mark[y, : max(0, min(w, out.shape[1] - ox))]
                out[ty, ox : ox + len(seg)] = np.maximum(out[ty, ox : ox + len(seg)], seg)
    return out


def isolate(cov, top, bottom):
    """The rows [top, bottom) of a glyph cell, cropped to their inked columns."""
    part = cov[top:bottom]
    cols = np.where(part.max(axis=0) > 0.2)[0]
    return part[:, cols.min() : cols.max() + 1] if len(cols) else None


# Each language ships its own atlas of the same typeface with just the letters it needs;
# Portuguese carries all of Latin-1 and Polish the Latin Extended-A letters it uses.
LANGUAGES = ["English", "Portuguese", "Polish", "Germany", "French", "Spanish", "Italian"]


def load_language(game, lang):
    files = bnd4_files(dcx_payload(f"{game}/font/{lang}/FeFont_Big.fontbnd.dcx"))
    glyphs = parse_ccm(files[next(n for n in files if n.endswith(".ccm"))])
    textures = {}
    for name, blob in files.items():
        if name.endswith(".tpf"):
            textures[int(name.split("_")[2].split(".")[0])] = texture(blob)
    cov = {code: coverage(g, textures) for code, g in glyphs.items() if g["tex"] in textures}
    return glyphs, cov


def main(game, out_path):
    glyphs, cov = {}, {}
    for lang in LANGUAGES:
        g, c = load_language(game, lang)
        for code in c:
            if code not in glyphs and code < 0x2200:
                glyphs[code], cov[code] = g[code], c[code]
    top, baseline = ink_rows(cov[ord("H")])
    baseline += 1
    scale = CAP_UNITS / (baseline - top)
    cell = glyphs[ord("H")]["region"][3] - glyphs[ord("H")]["region"][1]

    # Turkish letters missing from the game's set, composed from its own parts.
    def add(code, base, bitmap):
        if code not in glyphs and base in glyphs and bitmap is not None:
            glyphs[code] = dict(glyphs[base])
            cov[code] = bitmap

    i_top, _ = ink_rows(cov[ord("i")])
    x_top, _ = ink_rows(cov[ord("x")])
    dot = isolate(cov[ord("i")], i_top, x_top - 1)
    add(0x131, ord("i"), compose(cov[ord("i")], None, clear_top=x_top - 1))  # ı
    if dot is not None:
        cap_top, _ = ink_rows(cov[ord("I")])
        add(0x130, ord("I"), compose(cov[ord("I")], dot, mark_dy=max(0, cap_top - dot.shape[0] - 2)))  # İ
    for plain, cedilla_src, target in ((ord("s"), 0xE7, 0x15F), (ord("S"), 0xC7, 0x15E)):
        if cedilla_src in cov:
            src = cov[cedilla_src]
            _, c_bottom = ink_rows(cov[ord("c") if plain == ord("s") else ord("C")])
            ced = isolate(src, c_bottom + 1, src.shape[0])
            if ced is not None:
                add(target, plain, compose(cov[plain], ced, mark_dy=c_bottom + 1))
    # No atlas has a breve; cut one from the lower half of the game's "o" bowl.
    o_top, o_bottom = ink_rows(cov[ord("o")])
    bowl = isolate(cov[ord("o")], (o_top + o_bottom) // 2, o_bottom + 1)
    if bowl is not None:
        h, w = bowl.shape
        breve = np.asarray(
            Image.fromarray((bowl * 255).astype(np.uint8)).resize((max(4, int(w * 0.62)), max(3, int(h * 0.55))), Image.LANCZOS),
            dtype=np.float32,
        ) / 255.0
        for plain, target in ((ord("g"), 0x11F), (ord("G"), 0x11E)):
            g_top, _ = ink_rows(cov[plain])
            add(target, plain, compose(cov[plain], breve, mark_dy=max(0, g_top - breve.shape[0] - 2)))

    wanted = [c for c in sorted(glyphs) if (0x20 <= c <= 0x7E or 0xA0 <= c <= 0x17F or c in (0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026, 0x2022, 0x203A, 0x2039, 0x2122)) and c in cov]
    order = [".notdef"] + [f"uni{c:04X}" for c in wanted]
    charstrings, metrics, cmap = {}, {}, {}

    empty = T2CharStringPen(500, None)
    charstrings[".notdef"] = empty.getCharString()
    metrics[".notdef"] = (500, 0)

    for code in wanted:
        g, name = glyphs[code], f"uni{code:04X}"
        advance = round(g["advance"] * scale)
        pen = T2CharStringPen(advance, None)
        xs = []

        def pt(p):
            x = (p.x / UPSCALE + g["pre"]) * scale
            y = (baseline - p.y / UPSCALE) * scale
            xs.append(x)
            return (round(x, 1), round(y, 1))

        for curve in trace(cov[code]) if code != 0x20 else []:
            pen.moveTo(pt(curve.start_point))
            for seg in curve.segments:
                if seg.is_corner:
                    pen.lineTo(pt(seg.c))
                    pen.lineTo(pt(seg.end_point))
                else:
                    pen.curveTo(pt(seg.c1), pt(seg.c2), pt(seg.end_point))
            pen.closePath()
        charstrings[name] = pen.getCharString()
        metrics[name] = (advance, round(min(xs)) if xs else 0)
        cmap[code] = name

    ascent = round(baseline * scale)
    descent = round((cell - baseline) * scale)
    fb = FontBuilder(UNITS, isTTF=False)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap(cmap)
    fb.setupCFF("DS2Menu-Regular", {"FullName": "DS2 Menu"}, charstrings, {})
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=ascent, descent=-descent)
    fb.setupNameTable({"familyName": "DS2 Menu", "styleName": "Regular"})
    fb.setupOS2(
        sTypoAscender=ascent,
        sTypoDescender=-descent,
        sTypoLineGap=0,
        usWinAscent=ascent,
        usWinDescent=descent,
        sCapHeight=CAP_UNITS,
        sxHeight=round((baseline - x_top) * scale),
    )
    fb.setupPost()
    fb.font.flavor = "woff2"
    fb.save(out_path)
    print(f"{len(wanted)} glyphs, scale {scale:.2f} units/px, ascent {ascent}, descent {descent} -> {out_path}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
