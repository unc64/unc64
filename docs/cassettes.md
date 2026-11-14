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
  "title": "FROSTHALL",
  "author": "",
  "about": "",
  "machine": { "ram": 64, "niece": true },
  "tape": "VU5DVAEB...",
  "files": { "frosthall.s": "; FROSTHALL ...", "art.s": "..." },
  "main": "frosthall.s",
  "created": "2026-11-03T12:00:00Z"
}
```

| Field | | |
|---|---|---|
| `format`, `version` | required | `"unc64-cassette"`, `1`. A reader refuses a higher version it doesn't know. |
| `title` | required | What the label says. The tape's own name (what `LOAD "NAME"` finds) is in the tape header; they're usually the same. |
| `author`, `about` | optional | Free text: who made it, a line or two about it. |
| `machine` | optional | What it needs: `ram` 16 or 64 (K), `niece` true if it needs the NIECE, the graphics chip (manual chapter 15), and `aunt` false if it was made for a machine with the AUNT, the sound chip, taken out (each left out otherwise). A host may warn; it never refuses. Older cassettes may have `board` true, for a video board the machine no longer has. |
| `tape` | required | The tape image, base64: exactly the bytes of a `.tap` file (manual chapter 14: `UNCT`, version 1, type, 16-character name, load, length, entry, data, checksum; programs can follow one another). This is what loads. Its programs can be machine code (type 1) or BASIC programs (type 2: the program as BASIC keeps it, which `LOAD` puts back and `SAVE` records), so a BASIC program travels as a cassette too. |
| `files` | optional | The project it was built from: a map of path → UTF-8 text (assembly sources, `.include`d files, UNC BASIC listings as `.bas`, notes such as `README.md`). Paths are relative, `/`-separated, no `..`. Never needed to load; it's what opens in the Machine Room's editor. A cassette with `tape` and no `files` is a binary-only release; one with `files` is source-available. |
| `main` | with `files` | The file the project runs from: an assembly source, which the assembler starts from, or a `.bas` listing, which the Machine Room types into the machine (lines starting with `#` are notes, not typed). For a BASIC project the tape is a type-2 program, the listing as BASIC keeps it once typed in (`unc-asm --basic prog.bas --tape prog.tap` makes the same). |
| `controls` | optional | The keys, for a host that shows them beside the screen (the Portal does): plain text, a line each, `KEYS: what they do`, the keys with spaces between (`A D: move`, `UP DOWN LEFT RIGHT: steer`, `SPACE: fire`). A line with no colon is a note. 400 characters at most. |
| `instructions` | optional | How to play, if it needs saying: plain text, 800 characters at most. |
| `source_url` | optional | Where the source lives, if its author keeps it in a repository: a link on github.com (a repository, or a folder or a file in one). A host may link to it (the Portal's Read the source does). A cassette can carry `files`, or a link, or both. |
| `screenshots` | optional | Up to four pictures of the program running, each a PNG or a WebP of 200 KB or less, as base64. The first is the main one. A host may show them (the Portal does, on the shelf and on the cassette's page). |
| `cover` | optional | Cover art, the card from the cassette's box: one picture of the same kind, upright (about 416 x 640). |
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
- **The Portal** (unc64.com/portal): the community's library, the same file, posted as is.
  Each cassette there has a page that plays it in the browser, and it's one press away in
  the Machine Room (Browse, Online) and in the game (the pause menu: Settings, The Portal).
  Its Submit page packs a `.tap`, a `.s` or a `.bas` into a cassette, so a program saved in
  the game, or written in any editor, becomes one without the Machine Room.

## The Portal's index

What the Machine Room and the game read to list the Portal: `https://unc64.com/portal/index.json`.

```json
{
  "format": "unc64-portal",
  "version": 1,
  "updated": "2026-11-14T12:00:00Z",
  "cassettes": [
    { "id": "snake", "file": "cassettes/snake.unc.png", "title": "SNAKE", "author": "...",
      "about": "...", "category": "game", "tags": [], "controls": "A D: move\nSPACE: fire",
      "instructions": "...",
      "machine": { "ram": 16, "niece": false },
      "programs": [{ "name": "SNAKE", "type": 1, "length": 1715 }],
      "bytes": 157747, "width": 1004, "height": 638, "source": true, "lang": "asm",
      "added": "2026-11-14", "featured": true }
  ]
}
```

`id` is the cassette's name on the Portal (lower-case letters, digits and `-`): its page is
`/portal/<id>/`. `file` is relative to the index, and always `cassettes/<id>.unc.png`.
`category` is `game`, `demo`, `music`, `tool` or `other`; `lang` is `asm` or `basic`;
`source` says whether the project is on the cassette. `shots` and `cover` are pictures, relative to the index: the cassette's own `screenshots` and `cover`, copied out as files. Everything else repeats what's in the
cassette, so a list can be shown without fetching one. A reader refuses a higher `version`,
and treats the index as untrusted: it fetches only a `file` of that shape, and checks that
what arrives is a cassette.
