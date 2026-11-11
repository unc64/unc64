"""The UNC 64's character ROM as a font, for the game's UI and the site: rom/unscii.txt,
pixel for pixel, as UNC-Regular.ttf. Everything the machine says or you type is set in it,
outside the screen as on it, so the two can never drift apart.

    /usr/bin/python3 scripts/unc-font.py            # -> fonts/UNC-Regular.ttf

Each pixel is a 128-unit square on a 1024-unit em, so the font is crisp at multiples of
8 px (16, 24, 32). The cell is 8x8; text glyphs use rows 0-6 above the baseline and row 7
is the gap (the tails of g j p q y hang into it). ASCII is ASCII, small letters and all, as
on the machine. All 256 glyphs are in the Private Use Area at U+E000 + code, and the
graphics, signs and letters Unicode has are at their Unicode points as well.

    /usr/bin/python3 scripts/unc-font.py --woff2    # also web/fonts/UNC-Regular.woff2

Needs fontTools (and brotli for --woff2).
"""
import re
import sys
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "rom" / "unscii.txt"
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 and not sys.argv[1].startswith("--") else ROOT / "fonts" / "UNC-Regular.ttf"
PX = 128
EM = 8 * PX
BASE_ROW = 7  # rows 0-6 sit above the baseline

# graphics with a Unicode home, by ROM code
UNICODE = {
    0x01: 0x2598, 0x02: 0x259D, 0x03: 0x2580, 0x04: 0x2596, 0x05: 0x258C, 0x06: 0x259E,
    0x07: 0x259B, 0x08: 0x2581, 0x09: 0x2582, 0x0A: 0x2583, 0x0B: 0x2584, 0x0C: 0x2585,
    0x0D: 0x2586, 0x0E: 0x2587, 0x0F: 0x2588, 0x10: 0x2500, 0x11: 0x2502, 0x12: 0x250C,
    0x13: 0x2510, 0x14: 0x2514, 0x15: 0x2518, 0x16: 0x251C, 0x17: 0x2524, 0x18: 0x252C,
    0x19: 0x2534, 0x1A: 0x253C, 0x1B: 0x2571, 0x1C: 0x2572, 0x1D: 0x2573, 0x1E: 0x2592,
    0x1F: 0x2591,
    0x7F: 0x25C6,
    # the second half: the rest of the quarters and eighths, suits and the rest of the pictures
    0x80: 0x2597, 0x81: 0x259A, 0x82: 0x2590, 0x83: 0x259C, 0x85: 0x2599,
    0x86: 0x259F, 0x87: 0x258F, 0x88: 0x258E, 0x89: 0x258D, 0x8B: 0x258B,
    0x8C: 0x258A, 0x8D: 0x2589, 0x8E: 0x2594, 0x8F: 0x2595,
    0x90: 0x2660, 0x91: 0x2665, 0x92: 0x2666, 0x93: 0x2663, 0x94: 0x25CF, 0x95: 0x25CB,
    0x96: 0x2190, 0x97: 0x2191, 0x98: 0x2192, 0x99: 0x2193, 0x9A: 0x25E2, 0x9B: 0x25E3,
    0x9C: 0x25E4, 0x9D: 0x25E5, 0x9E: 0x263A, 0x9F: 0x266A,
    # double lines, round corners
    0xC0: 0x2550, 0xC1: 0x2551, 0xC2: 0x2554, 0xC3: 0x2557, 0xC4: 0x255A, 0xC5: 0x255D,
    0xC6: 0x2560, 0xC7: 0x2563, 0xC8: 0x2566, 0xC9: 0x2569, 0xCA: 0x256C,
    0xCB: 0x256D, 0xCC: 0x256E, 0xCD: 0x2570, 0xCE: 0x256F,
    # letters with accents, and signs
    0xD7: 0xC5, 0xD8: 0xC4, 0xD9: 0xD6, 0xDA: 0xE5, 0xDB: 0xE4, 0xDC: 0xF6, 0xDD: 0xD8,
    0xDE: 0xF8, 0xDF: 0xC6, 0xE0: 0xE6, 0xE1: 0xC9, 0xE2: 0xE9, 0xE3: 0xDC, 0xE4: 0xFC,
    0xE5: 0xDF, 0xE6: 0xF1, 0xE7: 0xA3, 0xE8: 0xA5, 0xE9: 0xA7, 0xEA: 0xB0, 0xEB: 0xB1,
    0xEC: 0xD7, 0xED: 0xF7, 0xEE: 0x3C0, 0xEF: 0xB5, 0xF0: 0xBD, 0xF1: 0xBC, 0xF2: 0xBE,
    0xF3: 0xBF, 0xF4: 0xA1, 0xF5: 0xAB, 0xF6: 0xBB,
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
        if 0x20 <= code <= 0x7E:
            cmap[code] = name
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
    if "--woff2" in sys.argv:
        from fontTools.ttLib import TTFont
        web = ROOT / "web" / "fonts" / "UNC-Regular.woff2"
        f = TTFont(str(OUT))
        f.flavor = "woff2"
        f.save(str(web))
        print(f"{web}: {web.stat().st_size} bytes")


if __name__ == "__main__":
    main()
