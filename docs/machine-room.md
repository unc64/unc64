# The Machine Room

The guide to the Machine Room, the browser IDE for the UNC 64: free in your browser at unc64.github.io/unc64, and bundled with the game (`web/`, built into `machine-room/index.html`). It was Appendix G of the Programmer's Reference Guide; it lives here because it's about a tool on your computer, not the UNC 64. The machine itself is in the manual (`docs/prg/programmers-reference-guide.md`).

This document describes the Machine Room, the developer's tool. It's free: use it at unc64.github.io/unc64, or keep it as one file, `machine-room.html`, and open it by double-clicking. The game comes with a copy too, as `machine-room/index.html` beside the game. From the repository, `scripts/build-machine-room.sh` builds that file into `build/machine-room/`; for working on the Machine Room itself, `python3 -m http.server -d web 8064` serves `web/` at `http://localhost:8064` (after `scripts/build-web.sh` if the emulator changed).

The Machine Room is the UNC 64 in a browser, with an editor, an assembler, a debugger and a tape deck around it: a small IDE for writing programs for the machine. It runs the real emulator core compiled to WebAssembly, so it runs exactly the machine this book describes. It is one web page that works from your disk, in a current Chrome, Edge, Firefox or Safari. It needs no installing and no internet, and nothing you write leaves your computer.

## The room

Along the top: the project's title (click it to rename the project), **New**, **Open**, **Browse**, **Save** and **Export**, then **Run** (Ctrl+Enter, Cmd+Enter on a Mac), the machine's settings, and **Help**. Below it, from the left:

- **Files**: the project's files. The one marked MAIN is the one that runs. Click a file to open it in a tab; the open file has **main**, **rename** and **×** (delete) beside it. **+ File** adds one.
- **The editor**, a tab for each open file (below).
- **The machine**: the monitor, the machine's status and controls, and the tape deck.
- Under the editor, three tabs: **Problems** (the assembler's errors: click one to go to its line), **Debugger** and **Output** (what the last assembly made: its size, the address ranges it fills, where it starts, and its symbols).

On a narrow screen the same parts are stacked, the machine first.

## Projects

A project is one or more files: assembly sources (`.s`), UNC BASIC listings (`.bas`) and notes (`.md`, `.txt`). **Run** starts from the main file:

- **Assembly**: the machine is switched off and on, main is assembled, the program is loaded, and it runs from `start` in the start state of chapter 6. `.include "file.s"` reads another of the project's files (chapter 5). If there are errors, nothing runs; they're under Problems, and the editor goes to the first.
- **BASIC**: the machine is switched off and on, the listing is typed in for you, as fast as BASIC takes the keys, and then `RUN`. Lines starting with `#` are notes, not typed. BASIC checks the lines itself, as you type them in: an error shows on the screen, `?SYNTAX ERROR IN 20`. Saved as a cassette, a BASIC project's tape is the program as BASIC keeps it, which `LOAD` puts back (see Cassettes).

**New** starts an assembly or a BASIC project. **Open** opens a file from your computer: a `.s` or `.bas` becomes a new project; a cassette opens its project, and its tape goes in the deck. Drop `.s` and `.bas` files on the page to add them to the project.

The project you're working on is kept in the browser, so it's there when you come back. Save it as a cassette to keep it properly.

## The editor

- **Highlighting** for UNC assembly: mnemonics, directives, labels, `@local` labels, constants, numbers (`$`, `%`, decimal, `'c'`), strings, comments, operators, functions, `A`/`X`/`Y`, `*` and `\name` pastes. Names the book gives to I/O registers, ROM routines and system variables (`BORDER`, `CHROUT`, `CURX`...) have their own colour. BASIC gets line numbers, keywords (chapter 2), strings, numbers and `REM`.
- **Errors** from the real assembler as you type (after a pause): underlined, with the message on hover. A file that main includes is checked as part of the project. An `undefined symbol` that's a name from the book gets a fix: **Define BORDER = $C020**.
- **Completion** (as you type, or Ctrl+Space): mnemonics and directives where an instruction goes; labels, constants, local labels in scope, functions, and the book's register, ROM and variable names where a value goes. Picking a name the program doesn't define adds its line, `BORDER  = $C020`, after the constants at the top. In BASIC, the keywords.
- **Hover** on an instruction, a label, a register, a directive or a number for what it is.
- **Cycle counts** in a gutter beside each instruction, `2/3` for a branch (taken: 3), exact after each assembly. Select some lines to see their total under the editor.
- **Keys: Standard / Vim**, remembered. `:w` runs.

Tab types a tab, Shift+Tab unindents, Ctrl+F finds (`/` in Vim), Ctrl+/ comments lines out.

## The machine

The monitor shows the machine at 50 frames a second. Each refresh of your screen runs the frames the machine is owed, at most two, so the picture keeps time without bursts; on a 60 Hz screen, one refresh in six shows no new frame. Click it, and your keys go to the machine: the cursor keys as codes 28-31, Ctrl+C as STOP, and every key's release to HELD (chapter 11). In the top bar:

- **RAM**: 16K (UNC 16) or 64K (with the expansion).
- **Video board**: the board of chapter 15 in, or out. It goes in or out as the machine is switched on, so changing it (or RAM) switches the machine off and on.
- **Colour**: a colour monitor, or the monochrome one, in the phosphor you pick (green, amber, paper white, or the blue screen).
- **Mute**: silence the speaker. Browsers allow sound only after you click, tap or press a key, so it starts at your first one. On an iPhone or an iPad it plays with the silent switch on (Safari 17 and later; before that, switch it off or use headphones).

| Control | Key | Does |
|---|---|---|
| Run / Pause | F8 | Run the machine, or stop it where it is. |
| Step | F7 | Run one instruction. |
| Step over | F6 | Run one instruction, but run a `JSR`'s whole subroutine as one step. |
| Reset | | The RESET button: the processor and the I/O start over, memory is kept, and BASIC says `READY.` with your program still there. It gets out of anything, a jammed processor too (chapter 13). |
| Power cycle | | Switch the machine off and on. The program is gone; the tape stays in the deck. |

On a phone or a tablet, tap the screen and the keyboard comes up. The keys it doesn't have are in a row under the monitor: the four cursor keys and STOP, which are down for as long as you hold them, and RESTORE.

The status under the monitor shows RUNNING, PAUSED, HALTED, `JAMMED ON $nn` (the opcode it jammed on) or `BREAK AT $nnnn`.

## The debugger

The **Debugger** tab shows the registers, the flags (lit when set), the cycle count, the raster line, the RAM, and the source file and line the PC is on.

**Breakpoints in your source.** Click in the editor's gutter, left of the cycle counts, to put a red dot beside a line; click it again to take it away. Running stops *before* that line's instruction, and the Debugger tab comes up. The dots move with your text as you edit. A dot on a line with no code (a comment, a label on its own, `.org`) moves to the next line in that file that has some, when you Run. A dot on the very first instruction stops the program before it starts.

When the machine stops, at a breakpoint, a Step or a Pause, the line it stopped on is lit in the editor with an arrow in the gutter, in whichever of your files it's in: step into a subroutine in another file and that file opens. Run (F8) carries on to the next dot.

**Disassembly** shows the instructions around PC, with labels. Click a line to set or clear a breakpoint there; on a line of your source, that's the same breakpoint as its dot. Running stops *before* that instruction.

**Memory** shows 256 bytes from any address. Type an address, or jump to the fast page, the screen, your program, I/O or the ROM. Bytes that changed since the display last refreshed are highlighted. **Stack** shows the bytes on the stack, from SP up.

## The tape deck

The deck under the monitor is the deck of chapter 14. The cassette in it shows by its face, with its name, the programs on it, the counter, and lights for the motor and PLAY. To load the way the machine does, put a cassette in, type `LOAD` (or `LOAD "NAME"`) at `READY.` and press **Play**. **Stop** lets PLAY up, **◀◀** rewinds, **Eject** takes the tape out. A tape stays in through a power cycle: the deck isn't part of the computer.

- **Fast load**: the next byte arrives as soon as `LOAD` has taken the last, so a game loads in a moment. `LOAD` runs exactly as usual, stripes and all; without it, the tape gives about 600 bytes a second, as on the real deck.
- **Tape it** puts your project on a tape, as `unc-asm --tape` would, named after its title, and puts it in the deck.
- **Run** in Browse puts a cassette in the deck, switches the machine on with what the cassette asks for (RAM, the video board), types `LOAD` and presses Play for you.

- **Blank tape** puts a blank C60 in the deck, and **Rec** is the RECORD key (it takes PLAY down with it). Type `SAVE "NAME"` at `READY.`; `SAVE` says `PRESS RECORD AND PLAY ON TAPE`; press Rec. When you eject a tape with something new recorded on it, the Machine Room offers to **keep it as a cassette** in your library, named after the program, with a blank tape's face.

A BASIC program comes off a tape and waits at `READY.` (chapter 14): **Run** in Browse types `RUN` for you once it has loaded.

## Cassettes

A **cassette** is how a program travels: a `.unc.png` file, a picture of the tape with the program inside it. Pass the file on as it is, as a download, an attachment or in a repository, and it still loads; a site that re-encodes pictures, as most social sites and chat apps do, keeps the face and drops the tape. The picture is the cassette's face: a label with the title, a picture of the screen, what it needs (16K or 64K, the video board) and who made it. Inside is the tape, exactly the bytes of a `.tap` file (chapter 14), and, if you include it, the project: every file, and which is main. A cassette with the source is a project anyone can open and learn from; one without is a binary release. The file format is in `docs/cassettes.md` in the repository. A bare `.tap` works everywhere a cassette does, with a plain blank-tape face.

**Save** makes a cassette of your project and keeps it in your library. Give it a title (the tape's name is the title in capitals, up to 16 characters: that's what `LOAD "NAME"` finds), your name, a line about it, and what it needs. The label's picture is the screen as it is when you press Save, so stop on the frame you want. **Include the source** puts the project's files on it; without, it's the tape alone. **Export** is the same, but downloads the `.unc.png` (or just the `.tap`).

For a BASIC project, the tape is a BASIC program (type 2, chapter 14): the Machine Room types the listing into a machine of its own and puts what BASIC made of it on the tape, exactly what `SAVE` would record. On the command line, `unc-asm --basic prog.bas --tape prog.tap` makes the same tape.

## Browse

**Browse** shows cassettes as pictures on a shelf, with a search box, in three sections:

- **Bundled**: the games (SNAKE, STACK, ORBIT, DEBUG and AURORA, the video board's demonstration cassette), the Machine Room's samples, and BASIC to type in, among them the Game Design Corner starter from *8-Bit Monthly*. They come as source, so their pictures are taken by running them, the first time you look.
- **Yours**: the cassettes you've saved or brought in. They're kept in this browser (in its own storage, not your files), so Download the ones you want to keep for good. A browser that won't keep anything for a page opened from your disk (some private windows do this) keeps them until you close the page, and says so.
- **Online**: coming later, a library of what other people have made, in the same file format.

Each cassette has **Run** (into the deck, and loaded), **Open** (its project into the editor, if it has the source) and **Download**; yours also have **Duplicate** and **Delete**. Drop `.unc.png` and `.tap` files anywhere on the page to put them in Yours.

## Cassettes in the game

To play a cassette in the game: turn on **My tapes** in the game's pause menu (Settings, Assembler), then put the cassette in the game's cassettes folder, or drop it on the game's window (it's copied there). The folder is the same on every computer, and the game makes it when you turn My tapes on:

| | |
|---|---|
| macOS | `~/Documents/UNC 64/Cassettes` |
| Windows | `C:\Users\<you>\Documents\UNC 64\Cassettes` |
| Linux | `~/Documents/UNC 64/Cassettes` (the system's Documents folder) |

**Open the folder**, beside My tapes, opens it. Your cassettes turn up in the shoebox on the desk: put one in the deck, `LOAD`, PLAY.

## The command-line assembler

The same assembler runs from a terminal:

```text
cargo run -p unc-asm -- prog.s -o prog.bin [--range 0800-0FFF] [--fill FF]
                        [--sym prog.sym] [--lst prog.lst]
cargo run -p unc-asm -- prog.s --tape prog.tap [--name NAME]
cargo run -p unc-asm -- --basic prog.bas --tape prog.tap [--name NAME]
```

It writes a flat binary of the range: by default from the lowest to the highest byte the program emits, with gaps filled with `$FF`. Reserved bytes (`.res`) are gaps too. It prints errors as `prog.s:12: message`, or `file.s:3: message` for a line in an included file. `.include "file.s"` finds the file in the same folder as `prog.s`.

- `--sym FILE` writes the symbol table, one `NAME = $XXXX` line per label and constant, in address order. Local labels appear as `global@local`.
- `--lst FILE` writes a listing: each source line with its address, the bytes it made (up to three on the line, the rest on lines below) and its line number. `.rept` copies are listed one by one. Where an operand that fits the fast page was assembled as absolute, the line ends with a note: `; ** LDA has no fast page,Y mode: absolute,Y`, or `; ** defined later: absolute`.

```text
0800  2D 10 00      2   LDA PTR,Y  ; ** LDA has no fast page,Y mode: absolute,Y
0803  6F 10         3   LDX PTR,Y
```

- `--tape FILE` writes a tape image (chapter 14): one machine-language program from the lowest to the highest byte the program emits (gaps are 0), entry at `start`, named NAME or after the file. The image is the bytes on the tape, so `cat a.tap b.tap > both.tap` makes a tape with both programs.

- `--basic` reads the file as a BASIC listing instead, and `--tape FILE` puts it on a tape as a BASIC program (type 2, chapter 14). It types the listing into a UNC 64, line by line as you would (lower case as capitals), and saves what BASIC made of it, tokens and all, so the tape is exactly what `SAVE` would record. Every line needs a line number; a line BASIC couldn't take (a character the UNC can't type, more than 120 characters) or a program too big for memory is an error. It says if the program is too big for a UNC 16.

`-o` can be left out when you only want the symbols, the listing or the tape.

## Editors

The UNC 64 has its own editors. On a modern computer, you can also write UNC programs in Vim,
Neovim, VS Code or Emacs, with help from the assembler itself. These are tools outside the
machine. They read your program the way the assembler does and tell you what it would do,
but they change nothing about the processor, its timing or the emulator, and no program on
the UNC 64 can tell they're there.

**The language server**, `unc-lsp`, works with any editor that speaks the Language Server
Protocol. Install it with `cargo install --path crates/unc-lsp`. It assembles your program
again every time you type, and gives you:

- **Errors and warnings** on the line and words they're about, with the explanation from
  Appendix E. An error in an included file shows on the `.include` line.
- **Cycle counts** at the end of each instruction's line: `4c` for a fixed cost (Appendix C),
  `2/3c` for a branch (not taken/taken), and `5c ×8` for a line that `.rept` assembles eight
  times. A `.rept` line shows its block's total, such as `8 × 6 = 48 cycles`. You can also
  turn on a running total since the last label.
- **Hover:** on an instruction, the mode it assembled to, its bytes, cycles and flags. On a
  label or constant, its value and where it's defined. On an I/O register or ROM routine,
  named or as a number like `$C020`, what Appendix B or D says about it.
- **Completion** of mnemonics with their addressing modes, directives, labels (with the local
  labels in scope), constants and functions. It also completes the names in Appendices B and D.
  The UNC has no built-in names, so picking `BORDER` also adds `BORDER  = $C020` to your
  program, and a quick fix does the same for an `undefined symbol 'BORDER'`.
- **Go to definition, find references, an outline and rename** for labels and constants.
  These also work across `.include`d files, which are found next to the file that includes
  them.

**Vim** works without the server too. `editors/vim` has a syntax file, filetype detection and
settings for UNC sources. A `.s` file counts as a UNC source when a comment near the top names
the UNC, as every program in this book does, or when a `.unc` file marks its project. `.uncs`
and `.unc.s` files always count. **VS Code** has a small extension in `editors/vscode`.
`editors/README.md` explains how to set up each editor.

The editors take what they know about the language from the assembler and from this book's
own tables, so they always agree with both. Other programs can use the same information:
`cargo run -p unc-asm -- --lang-json` prints every instruction (with its modes, opcodes, bytes,
cycles and flags), directive, function, I/O register, ROM routine and system variable, and
`--analysis-json prog.s` prints each line's address, bytes and cycles.
