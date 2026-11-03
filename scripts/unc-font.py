"""The UNC 64's character ROM as a font, for the game's UI and the site: rom/unscii.txt,
pixel for pixel, as UNC-Regular.ttf. Everything the machine says or you type is set in it,
outside the screen as on it, so the two can never drift apart.

    /usr/bin/python3 scripts/unc-font.py            # -> fonts/UNC-Regular.ttf

Each pixel is a 128-unit square on a 1024-unit em, so the font is crisp at multiples of
8 px (16, 24, 32). The cell is 8x8; text glyphs use rows 0-6 above the baseline and row 7
is the gap. The ROM has capitals only, like the machine: a-z map to A-Z. The graphics
($00-$1F, $60-$7F) are in the Private Use Area at U+E000 + code, and the ones Unicode has
(quarter blocks, box lines, shades, eighths) at their Unicode points as well.
"""
import re
import sys
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "rom" / "unscii.txt"
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "fonts" / "UNC-Regular.ttf"
PX = 128
EM = 8 * PX
BASE_ROW = 7  # rows 0-6 sit above the baseline

# graphics with a Unicode home, by ROM code
UNICODE = {
    0x01: 0x2598, 0x02: 0x259D, 0x03: 0x2580, 0x04: 0x2596, 0x05: 0x258C, 0x06: 0x259E,
    0x07: 0x259B, 0x08: 0x2597, 0x09: 0x259A, 0x0A: 0x2590, 0x0B: 0x259C, 0x0C: 0x2584,
    0x0D: 0x2599, 0x0E: 0x259F, 0x0F: 0x2588, 0x10: 0x2500, 0x11: 0x2502, 0x12: 0x250C,
    0x13: 0x2510, 0x14: 0x2514, 0x15: 0x2518, 0x16: 0x251C, 0x17: 0x2524, 0x18: 0x252C,
    0x19: 0x2534, 0x1A: 0x253C, 0x1B: 0x2571, 0x1C: 0x2572, 0x1D: 0x2573, 0x1E: 0x2592,
    0x1F: 0x2591,
    0x60: 0x2581, 0x61: 0x2582, 0x62: 0x2583, 0x63: 0x2584, 0x64: 0x2585, 0x65: 0x2586,
    0x66: 0x2587, 0x67: 0x258F, 0x68: 0x258E, 0x69: 0x258D, 0x6A: 0x258C, 0x6B: 0x258B,
    0x6C: 0x258A, 0x6D: 0x2589,
}


def glyphs():
    rows, code = [], None
    for line in SRC.read_text().splitlines():
        m = re.match(r"^\$([0-9A-Fa-f]{2}) ", line)
        if m:
            code, rows = int(m.group(1), 16), []
        elif code is not None and re.fullmatch(r"[#.]{8}", line):
            rows.append(line)
            if len(rows) == 8:
                yield code, rows
                code = None


def outline(rows):
    pen = TTGlyphPen(None)
    for r, row in enumerate(rows):
        top = (BASE_ROW - r) * PX
        c = 0
        while c < 8:  # one rectangle per run of ink in a row
            if row[c] != "#":
                c += 1
                continue
            s = c
            while c < 8 and row[c] == "#":
                c += 1
            x0, x1, y0, y1 = s * PX, c * PX, top - PX, top
            pen.moveTo((x0, y0)); pen.lineTo((x0, y1)); pen.lineTo((x1, y1)); pen.lineTo((x1, y0))
            pen.closePath()
    return pen.glyph()


def main():
    names, outlines, cmap, metrics = [".notdef"], {}, {}, {}
    outlines[".notdef"] = outline(["########"] + ["#......#"] * 6 + ["########"])
    metrics[".notdef"] = (EM, 0)
    for code, rows in glyphs():
        name = "uni%04X" % (0xE000 + code)
        names.append(name)
        outlines[name] = outline(rows)
        metrics[name] = (EM, 0)
        cmap[0xE000 + code] = name
        if 0x20 <= code <= 0x5F:
            cmap[code] = name
            if 0x41 <= code <= 0x5A:
                cmap[code + 0x20] = name  # lower case prints as the machine's capitals
        if code in UNICODE:
            cmap[UNICODE[code]] = name
    fb = FontBuilder(EM, isTTF=True)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(outlines)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=BASE_ROW * PX, descent=-PX)
    fb.setupNameTable({"familyName": "UNC", "styleName": "Regular",
                       "copyright": "UNSCII glyphs, public domain; UNC 64 character ROM"})
    fb.setupOS2(sTypoAscender=BASE_ROW * PX, sTypoDescender=-PX, sTypoLineGap=0,
                usWinAscent=BASE_ROW * PX, usWinDescent=PX, fsType=0)
    fb.setupPost(isFixedPitch=1)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    fb.save(str(OUT))
    print(f"{OUT}: {len(names) - 1} glyphs")


if __name__ == "__main__":
    main()
