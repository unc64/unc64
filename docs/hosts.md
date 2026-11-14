# For hosts: watching the machine, and save states

For the programs that run the emulator (the game, the Machine Room, the tests), not for programs that run on the UNC 64. It was Appendix H of the Programmer's Reference Guide. The machine itself is in the manual (`docs/prg/programmers-reference-guide.md`).

This document is for the programs that run the emulator (the game, the Machine Room, the tests), not for programs that run on the UNC 64. Nothing here can be seen from inside the machine: there's no instruction, register or address for any of it.

## Watching the machine

`Machine::observe()` in Rust, `observe()` on the game's machine node, says what the processor has done since the last call, then starts the counts again from 0:

| Field | What it counts |
|---|---|
| `reads`, `writes` | The processor's reads and writes of each I/O address `$C000-$C0FF`: 256 counts each. `INC $C020` is one of each. |
| `reserved_read` | Whether anything read `$C3F0-$C3FF` (Appendix B's reserved range). |
| `frames` | Frames that ended. |
| `screen_writes` | Writes to screen memory, `$0400-$07E7`. |
| `bitmap_writes` | Writes to the 8,000 bytes from BITMAP × 256, wherever BITMAP points at the time of the write, and whether or not the bitmap is showing. |

Only the processor's own reads and writes count, whoever's program it's running: BASIC's `PEEK` and `POKE` count, because the ROM does them with `LDA` and `STA`. The host's own `peek`, `poke` and `load` don't, so a debugger or a check that looks at memory doesn't change what it measures.

The counts are the host's bookkeeping. No program can read them, and nothing the machine does depends on them, so a program runs the same, cycle for cycle, whether anything watches it or not. Keeping them costs one comparison on each memory access, too little to measure. A reset doesn't clear them; `Machine::new` starts them at 0. A few calls a second is plenty: a count only wraps after 2³² events.

In the game, `observe()` returns a dictionary: `reads` and `writes` (PackedInt32Array, 256 each), `reserved_read` (bool), and `frames`, `screen_writes` and `bitmap_writes` (int).

## The picture

`Machine::frame()` is the 320 × 200 picture as the beam drew it (chapter 9): a byte a pixel, each line drawn as the raster finished it. After `run_frame`, the whole frame. Without the video board each byte is 0 (paper) or 1 (ink). With it, each is a colour number, 0-3 (chapter 15). With the NIECE in and its page shown (chapter 17; `Machine::set_niece(true)` puts the prototype chip in, and the bindings' `set_niece` does at the next power-on), a pixel where the picture is colour 0 shows the page instead: its value is 16 + the page's colour, 0-15 from Appendix A, so a host tests bit 4 first and only looks 0-3 up in `line_colours`. `Machine::line_colours(y)` gives the four colours, 0-15 from Appendix A, that values 0-3 showed on line *y* when it was drawn: BACKGROUND, then white (1) without the board, COLOUR1-COLOUR3 with it. `Machine::blink_on` is the cursor's blink, from the host's clock, for the lines the beam draws next. `Machine::render(blink_on, out)` draws every line from the machine as it stands now, for tests and tools, and `Machine::colour_of(n)` gives what value *n* shows now. The Machine Room and the site call `render(blink)`, which sets the blink and returns the beam's picture, and `line_colours_ptr()` (`unc.wasm`); the game's `frame_rgba` does the same mapping itself, line by line, and on the monochrome monitor shows each colour as a shade of the phosphor, as bright as the colour is light, with white as the phosphor at full.

## Save states

`Machine::save_state()` returns the whole computer as bytes, and `Machine::load_state(bytes)` puts it back exactly: the same bytes run the same future, cycle for cycle. In the game they're `save_state()`, which returns a PackedByteArray, and `load_state(bytes)`, which returns false for bytes it can't use.

The format, numbers low byte first:

| Bytes | Holds |
|---|---|
| 4 | `UNCS` |
| 1 | Format version: 6 |
| 1 | RAM: 16 or 64 (K) |
| 18 | The processor: A, X, Y, SP, PC, P, the cycle count, whether it's running, halted or jammed (and on what), and the IRQ line |
| 66 + keys held | The I/O: VMODE, the raster compare, the interrupt registers, FRAME, KBD and the keys held, the keyboard matrix, the colours, CHARSET, BITMAP, TEXTROWS, the timer, the speaker, the shifter |
| 1, or 16 | The video board: 0 if it isn't in; 1 and its 15 bytes of registers (SPRTAB, SPRBANK, the collision and line registers, the scrolls, the layer, COLOURMODE and the colours) if it is. Its sprite table and images are in RAM |
| 1, or 65,900 and 43 a queued command | The NIECE (manual chapter 17): 0 if it isn't in; 1 and its registers, ink tables, depth line, queue and 64K of sheet if it is |
| 4 | The computer's side of the tape interface: the motor, TAPEDATA, byte ready, overrun |
| 4 | Cycles into the current frame |
| 312 | The border colour on each line of the last frame (for the picture) |
| 16K or 48K | RAM, from `$0000` to the top of memory |

A UNC 16 with no key down saves to 16,918 bytes; with the 64K expansion, 49,686. The video board adds 15. The AUNT, the sound chip (manual ch. 12), is in every machine and in every save (121 bytes of it); `Machine::set_sound(false)` takes it out, and the bindings' `set_sound_chip` does that at the next power-on. `observe()` also reports the last write the program made to the reserved range (`reserved_write`), which changes nothing in the machine: the game makes its secret of it. A save state takes the board with it: loading one puts the board in or takes it out, as it does the 64K expansion.

**Not saved:** the tape deck (the cassette in it, how far it has wound and the PLAY and RECORD keys), the host's `observe` counts, breakpoints, and sound already played. The deck isn't part of the computer, just as a reset doesn't touch it. `load_state` leaves the deck as the host has it and takes only the computer's side of the interface from the save, so a host that wants the tape back puts it in again. That's safe at any moment: a `LOAD` that was reading carries on with whatever tape it finds, and stops with `BREAK` or `?LOAD ERROR` if it isn't the same one; a `SAVE` that was recording stops with `BREAK` unless RECORD is down.

`load_state` checks everything before it changes anything. Bytes that aren't a save state, a version this build doesn't know, a RAM size that isn't 16 or 64, or a state that's been cut short or damaged are refused, and the machine carries on as it was. A newer layout gets a new version number, and a build reads the older ones it knows. Version 5 is the same without the NIECE's byte, and loads as a machine without one. Version 1, from before the video board, is the same layout without the board's byte, and loads as a machine without the board. Version 2 had the first Rev A board, eight sprites kept in 37 bytes of registers; that board no longer exists, so a version 2 state loads with the board in, its bank, scroll and layer kept and its sprites dropped: the sprite table and the colours start as at reset. Anything else is never guessed at. After a load, the `observe` counts start from 0 and the speaker's sound starts afresh.
