#!/usr/bin/env python3
"""Makes FROSTHALL's pictures, its ink tables and its map, and writes them into
games/niece/frosthall.s between the two marker lines. The game's source stays one file;
this is only where its data comes from.

    python3 scripts/frosthall-art.py            # rewrite the data in frosthall.s
    python3 scripts/frosthall-art.py --png DIR  # also save each picture as a PNG (needs PIL)

Everything here is whole numbers and its own random numbers, so it writes the same bytes on
every computer.
"""
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
SOURCE = os.path.join(ROOT, 'games', 'niece', 'frosthall.s')
BEGIN = '; ---- BEGIN DATA: made by scripts/frosthall-art.py, not by hand ----\n'
END = '; ---- END DATA ----\n'

BLACK, WHITE, RED, CYAN, PURPLE, GREEN, BLUE, YELLOW = range(8)
ORANGE, BROWN, LRED, DGREY, GREY, LGREEN, LBLUE, LGREY = range(8, 16)
KEY = PURPLE  # the see-through colour of the things in the hall
N = 64


class Rand:
    def __init__(self, seed):
        self.s = seed

    def next(self, n):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return (self.s >> 16) % n


def blank(c):
    return [[c] * N for _ in range(N)]


def rect(t, x0, y0, x1, y1, c):
    for y in range(max(0, y0), min(N, y1)):
        for x in range(max(0, x0), min(N, x1)):
            t[y][x] = c


def timber():
    """Upright planks, sixteen pixels each, with a rail across."""
    r = Rand(7)
    t = blank(BROWN)
    for p in range(4):
        x0 = p * 16
        for y in range(N):
            t[y][x0] = BLACK
            t[y][x0 + 1] = ORANGE
            t[y][x0 + 15] = DGREY
        for _ in range(5):  # the grain
            x, y, n = x0 + 3 + r.next(11), r.next(N), 6 + r.next(14)
            for k in range(n):
                t[(y + k) % N][x] = ORANGE if r.next(4) else DGREY
        kx, ky = x0 + 4 + r.next(8), 8 + r.next(44)  # a knot
        rect(t, kx, ky, kx + 3, ky + 4, DGREY)
        t[ky + 1][kx + 1] = BLACK
    for y0 in (10, 46):  # the rails, and their nails
        rect(t, 0, y0, N, y0 + 7, ORANGE)
        rect(t, 0, y0, N, y0 + 1, YELLOW)
        rect(t, 0, y0 + 6, N, y0 + 7, BLACK)
        for x in range(7, N, 16):
            t[y0 + 3][x] = LGREY
            t[y0 + 3][x + 1] = DGREY
    return t


def stone(seed=3):
    """Blocks in courses, lit from the top left."""
    r = Rand(seed)
    t = blank(BLACK)
    for row in range(4):
        y0 = row * 16
        x = -(row % 2) * 16 - 8
        while x < N:
            w = 24 + 8 * r.next(2)
            body = GREY if r.next(3) else LGREY
            rect(t, x + 1, y0 + 1, x + w, y0 + 16, body)
            rect(t, x + 1, y0 + 1, x + w, y0 + 2, LGREY if body == GREY else WHITE)
            rect(t, x + 1, y0 + 1, x + 2, y0 + 16, LGREY if body == GREY else WHITE)
            rect(t, x + 1, y0 + 15, x + w, y0 + 16, DGREY)
            rect(t, x + w - 1, y0 + 1, x + w, y0 + 16, DGREY)
            for _ in range(w // 3):
                px, py = x + 3 + r.next(w - 5), y0 + 3 + r.next(11)
                if 0 <= px < N:
                    t[py][px] = DGREY if r.next(3) else (LGREY if body == GREY else GREY)
            x += w
    return t


def ice():
    """Blocks of ice: light blue, with white where the light catches and cracks of blue."""
    r = Rand(11)
    t = blank(BLUE)
    for row in range(2):
        for col in range(2):
            x0, y0 = col * 32 + (row * 16) % 32, row * 32
            for y in range(y0 + 1, y0 + 32):
                for x in range(x0 + 1, x0 + 32):
                    d = (x - x0) + (y - y0)
                    c = LBLUE
                    if d < 14 or 30 <= d < 33:
                        c = CYAN
                    if d in (8, 9, 31):
                        c = WHITE
                    if d > 52:
                        c = BLUE
                    t[y % N][x % N] = c
            x, y = x0 + 6 + r.next(18), y0 + 4  # a crack, wandering down
            for _ in range(22):
                t[y % N][x % N] = BLUE
                t[y % N][(x + 1) % N] = WHITE
                y += 1
                x += r.next(3) - 1
    return t


def banner():
    """The same stone with a banner hung on it: red, a yellow sun."""
    t = stone(5)
    rect(t, 12, 4, 52, 6, BROWN)  # the pole
    rect(t, 10, 3, 12, 7, YELLOW)
    rect(t, 52, 3, 54, 7, YELLOW)
    for y in range(6, 56):
        inset = max(0, y - 46)  # the swallow tail
        for x in range(16, 48):
            mid = abs(x - 31.5)
            if y >= 46 and mid < inset:
                continue
            c = RED
            if x in (16, 47) or y == 6:
                c = LRED
            if x in (17, 46):
                c = YELLOW
            t[y][x] = c
    cx, cy = 31.5, 24.5
    for y in range(8, 42):
        for x in range(18, 46):
            dx, dy = x - cx, y - cy
            d2 = dx * dx + dy * dy
            if d2 < 36:
                t[y][x] = YELLOW
            elif d2 < 110 and (abs(dx) < 1 or abs(dy) < 1 or abs(abs(dx) - abs(dy)) < 1.2):
                t[y][x] = YELLOW
    rect(t, 29, 22, 31, 24, ORANGE)
    rect(t, 33, 22, 35, 24, ORANGE)
    rect(t, 29, 27, 35, 28, ORANGE)
    return t


def barrel():
    t = blank(KEY)
    for y in range(26, 62):
        k = y - 26
        bulge = 4 - abs(k - 17) * abs(k - 17) // 80  # widest in the middle
        x0, x1 = 20 - bulge, 44 + bulge
        for x in range(x0, x1):
            f = (x - x0) * 8 // (x1 - x0)
            c = (BLACK, BROWN, ORANGE, YELLOW, ORANGE, BROWN, BROWN, BLACK)[f]
            if (x - x0) % 5 == 0 and c not in (BLACK,):
                c = DGREY  # the gaps between the staves
            t[y][x] = c
        if k in (0, 35):
            for x in range(x0, x1):
                t[y][x] = BLACK
        if k in (5, 6, 28, 29):  # the hoops
            for x in range(x0, x1):
                t[y][x] = LGREY if (x - x0) * 3 // (x1 - x0) == 1 else GREY
    for x in range(22, 42):  # the lid, seen a little from above
        t[26][x] = ORANGE
        t[27][x] = BROWN
    rect(t, 18, 62, 46, 63, BLACK)  # its shadow
    return t


def brazier():
    r = Rand(23)
    t = blank(KEY)
    rect(t, 30, 34, 34, 60, DGREY)  # the stand
    rect(t, 31, 34, 32, 60, GREY)
    rect(t, 22, 60, 42, 62, DGREY)
    rect(t, 20, 62, 44, 63, BLACK)
    for y in range(26, 35):  # the bowl
        w = 13 - (y - 26)
        for x in range(32 - w, 32 + w):
            t[y][x] = GREY if x < 30 else DGREY
        t[y][32 - w] = LGREY
    rect(t, 18, 25, 46, 27, LGREY)
    for y in range(4, 26):  # the flame
        h = 25 - y
        w = max(1, min(10, h // 2 + 2) - (h * h) // 60)
        sway = (h // 4) % 3 - 1
        for x in range(32 - w + sway, 32 + w + sway):
            d = abs(x - 32 - sway)
            c = RED if d > w - 2 else ORANGE
            if d < w - 3:
                c = YELLOW
            if d < w - 6 and h < 9:
                c = WHITE
            t[y][x] = c
    for _ in range(7):  # sparks
        t[2 + r.next(10)][22 + r.next(20)] = YELLOW
    return t


def crystal():
    """A spire of ice from the floor, with two smaller beside it."""
    t = blank(KEY)

    def spire(cx, top, base, w):
        for y in range(top, base):
            half = max(1, (y - top) * w // (base - top))
            for x in range(cx - half, cx + half + 1):
                f = x - (cx - half)
                c = LBLUE
                if f <= half // 2:
                    c = CYAN
                if f == max(0, half // 3):
                    c = WHITE
                if f >= half * 2 - half // 3:
                    c = BLUE
                t[y][x] = c
            t[y][cx - half] = WHITE if y % 7 < 3 else CYAN
            t[y][cx + half] = BLACK

    spire(32, 4, 62, 9)
    spire(18, 30, 62, 6)
    spire(46, 38, 62, 5)
    rect(t, 10, 62, 54, 63, BLACK)
    return t


# What each colour turns into a step further from the light.
DARKER = {WHITE: LGREY, LGREY: GREY, GREY: DGREY, DGREY: BLACK, BLACK: BLACK, YELLOW: ORANGE, ORANGE: BROWN, BROWN: DGREY,
          LRED: RED, RED: BROWN, LGREEN: GREEN, GREEN: DGREY, CYAN: LBLUE, LBLUE: BLUE, BLUE: DGREY, PURPLE: PURPLE}

# The hall. Digits are walls (the picture they wear), letters the things in it, P where you start.
MAP = """
11111111111122222222222233333333
1..........1....b.....2........3
1..P.......1..........2...c....3
1..........4....22....2........3
1....b.....1....22.........33..3
1..........1..........3....33..3
1111..111111..........3........3
2.........f.......2...3..c.....3
2..22.............2...33333..333
2..22.....2222..22........3....3
2.........2..2..22..b.....3....3
2.....b......2............4..f.3
2..2222222...2....11111...3....3
2........2...2....1...1...3....3
2.....f..2........1.c.....33..33
2........2...2....1...1........3
22222..2222222....11111...333333
3.........3......2........4....1
3..c......3......2........1..b.1
3.........3..3333333......1....1
3....33...3..3.....3...........1
3....33......3..c..3......1....1
3............3.....3......1....1
33333333333333333333111111111111
"""


def hexrow(row):
    return '        .byte ' + ','.join('$%X%X' % (row[i], row[i + 1]) for i in range(0, N, 2))


def data():
    out = [BEGIN]
    pictures = [('timber', timber()), ('stone', stone()), ('ice', ice()), ('banner', banner()),
                ('barrel', barrel()), ('brazier', brazier()), ('crystal', crystal())]
    out.append('; The pictures: 64 x 64 each, a digit a pixel, 32 bytes a line. Walls first, in the order\n')
    out.append('; of the map\'s numbers, then the things. Colour %d is the things\' see-through colour.\n' % KEY)
    out.append('PICTURES = %d\n' % len(pictures))
    out.append('pictures:\n')
    for name, t in pictures:
        out.append('; %s\n' % name)
        out.extend(hexrow(r) + '\n' for r in t)
    out.append('; The ink tables 1-3: every colour one, two and three steps darker.\n')
    out.append('inks:\n')
    table = list(range(16))
    for _ in range(3):
        table = [DARKER[c] for c in table]
        out.append('        .byte ' + ','.join(str(c) for c in table) + '\n')
    rows = [r for r in MAP.strip().split('\n')]
    assert all(len(r) == 32 for r in rows), [len(r) for r in rows]
    things, start = [], None
    out.append('; The hall: %d rows of 32. 0 is floor; 1-4 a wall, and which picture it wears.\n' % len(rows))
    out.append('MAPROWS = %d\n' % len(rows))
    out.append('map:\n')
    for y, r in enumerate(rows):
        cells = []
        for x, ch in enumerate(r):
            cells.append(int(ch) if ch.isdigit() else 0)
            if ch == 'P':
                start = (x, y)
            elif ch in 'bfc':
                things.append((x, y, 4 + 'bfc'.index(ch)))
        out.append('        .byte ' + ','.join(str(c) for c in cells) + '    ; ' + r + '\n')
    out.append('; Where you start, and the things: the cell each stands in the middle of, and its picture.\n')
    out.append('STARTX  = %d\nSTARTY  = %d\n' % start)
    out.append('THINGS  = %d\n' % len(things))
    out.append('thingx: .byte ' + ','.join(str(t[0]) for t in things) + '\n')
    out.append('thingy: .byte ' + ','.join(str(t[1]) for t in things) + '\n')
    out.append('thingp: .byte ' + ','.join(str(t[2]) for t in things) + '\n')
    out.append(END)
    return ''.join(out), pictures


def main():
    text, pictures = data()
    src = open(SOURCE).read()
    a, b = src.index(BEGIN), src.index(END) + len(END)
    open(SOURCE, 'w').write(src[:a] + text + src[b:])
    print('games/niece/frosthall.s: %d pictures, %d bytes of them' % (len(pictures), len(pictures) * N * N // 2))
    if '--png' in sys.argv:
        from PIL import Image
        pal = [0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b,
               0x9fe08a, 0x6f8fd8, 0xa9a9a9]
        d = sys.argv[sys.argv.index('--png') + 1]
        sheet = Image.new('RGB', (N * len(pictures), N))
        for i, (name, t) in enumerate(pictures):
            for y in range(N):
                for x in range(N):
                    c = pal[t[y][x]]
                    sheet.putpixel((i * N + x, y), (c >> 16, c >> 8 & 255, c & 255))
        sheet.resize((N * len(pictures) * 3, N * 3), Image.NEAREST).save(os.path.join(d, 'frosthall-pictures.png'))


if __name__ == '__main__':
    main()
