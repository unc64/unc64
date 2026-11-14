# Cassettes: the UNC 64's shareable program file

A **cassette** is the UNC 64's distribution format for programs, binaries and sources together. It's how a program travels between the Machine Room (the developer's
browser tool), the game, and, later, the community library. Inspired by PICO-8's
cartridge PNGs: the file is a picture of the tape, and the program rides inside it. Pass the
file on as it is (a download, an attachment, a repository) and it still loads. A site that
re-encodes pictures, as most social sites and chat apps do, keeps the face and drops the tape.

## The file

- A valid PNG, named `<something>.unc.png` by convention.
- The **image** is the cassette's face, label side up: any size with the cassette's
  proportions (100.4 × 63.8, e.g. 1004 × 638 or 502 × 319). The Machine Room draws it: a
  shell, a label with the title, and a picture of the screen. The game shows it as the
  cassette's face.
- The **data** is in one ancillary, private chunk of type `unCt` (lower-case first letter:
  ancillary, so every PNG reader skips it; lower-case second: private; upper-case third, as
  the PNG spec requires; lower-case fourth: safe to copy), anywhere before `IEND`. Its CRC is
  the PNG CRC-32 of type + data, as for every chunk.
- The chunk's data is UTF-8 JSON:

```json
{
  "format": "unc64-cassette",
  "version": 1,
  "title": "AURORA",
  "author": "",
  "about": "",
  "machine": { "ram": 16, "board": true },
  "tape": "VU5DVAEB...",
  "files": { "aurora.s": "; AURORA ...", "sprites.s": "..." },
  "main": "aurora.s",
  "created": "2026-11-03T12:00:00Z"
}
```

| Field | | |
|---|---|---|
| `format`, `version` | required | `"unc64-cassette"`, `1`. A reader refuses a higher version it doesn't know. |
| `title` | required | What the label says. The tape's own name (what `LOAD "NAME"` finds) is in the tape header; they're usually the same. |
| `author`, `about` | optional | Free text: who made it, a line or two about it. |
| `machine` | optional | What it needs: `ram` 16 or 64 (K), `board` true if it needs the video board, `niece` true if it needs the NIECE, the prototype graphics chip (manual chapter 17; left out when it doesn't). A host may warn; it never refuses. |
| `tape` | required | The tape image, base64: exactly the bytes of a `.tap` file (manual chapter 14: `UNCT`, version 1, type, 16-character name, load, length, entry, data, checksum; programs can follow one another). This is what loads. Its programs can be machine code (type 1) or BASIC programs (type 2: the program as BASIC keeps it, which `LOAD` puts back and `SAVE` records), so a BASIC program travels as a cassette too. |
| `files` | optional | The project it was built from: a map of path → UTF-8 text (assembly sources, `.include`d files, UNC BASIC listings as `.bas`, notes such as `README.md`). Paths are relative, `/`-separated, no `..`. Never needed to load; it's what opens in the Machine Room's editor. A cassette with `tape` and no `files` is a binary-only release; one with `files` is source-available. |
| `main` | with `files` | The file the project runs from: an assembly source, which the assembler starts from, or a `.bas` listing, which the Machine Room types into the machine (lines starting with `#` are notes, not typed). For a BASIC project the tape is a type-2 program, the listing as BASIC keeps it once typed in (`unc-asm --basic prog.bas --tape prog.tap` makes the same). |
| `created` | optional | ISO 8601, UTC. |

Unknown fields are kept when a cassette is re-saved, and ignored otherwise.

## Bare tapes

A `.tap` file (from `unc-asm --tape`) is also accepted everywhere a cassette is: its title is
the first program's name (header bytes 6-21, trimmed), and it gets a plain blank-tape face.

## Where they go

- **Machine Room:** the library (Browse, Yours) keeps your cassettes in the browser
  (IndexedDB). **Save** puts the project there as a cassette; **Export** downloads the
  `.unc.png` (or the bare `.tap`), as does Download in Browse. **Open** opens a `.unc.png` or
  `.tap` (its project in the editor, its tape in the deck); dropping one on the page puts it in
  the library.
- **The game:** one folder, the same on every computer:

  | | |
  |---|---|
  | macOS | `~/Documents/UNC 64/Cassettes` |
  | Windows | `C:\Users\<you>\Documents\UNC 64\Cassettes` |
  | Linux | `~/Documents/UNC 64/Cassettes` (the system's Documents folder) |

  It's opt-in: **My tapes** in the pause menu (Settings, Assembler) is off at first, and
  until it's on the game never touches the folder; turning it on makes it, and **Open the
  folder** beside it opens it (turning it on too). Put `.unc.png` and `.tap` files there, or
  drop them on the game's window: they're copied there (with My tapes off, the game asks
  first). Your cassettes appear in the shoebox on the desk ("my tapes"); pick one and it goes into
  the deck like any other tape: `LOAD`, PLAY. A tape you `SAVE` in the game lands there too.
- **Later, the community library:** the same file, posted as is.
