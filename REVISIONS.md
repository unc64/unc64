# Revisions

## Rev A

A program that runs on a **System Rev A** machine runs the same, cycle for cycle, on every later one. Rev A fixes:

| What | Defined in |
|---|---|
| The instruction set: every opcode the processor runs, its addressing mode, its flags and its cycles | Manual Appendix C, `crates/unc-core/src/isa.rs` |
| The memory map and I/O registers, including what unmapped addresses read | Appendix B |
| Timing: 64 cycles a line, 312 lines a frame; the picture drawn line by line as the beam finishes each, from line 51; FRAME and VBLANK at line 251; no cycle ever taken from the processor | Chapter 9 |
| The screen: text, the character set, the bitmap, the modes | Chapter 8 |
| The video board Rev A: its registers, the sprite table, eight sprites a line, and its id (BOARD reads `$15`) | Chapter 15 |
| The beeper's timer, NOISE, the shifter and the keyboard matrix | Chapters 8, 11, 12 |
| The ROM's jump table, `$FF00-$FF21`: the addresses and what each call does | Chapter 13, Appendix D |
| UNC BASIC: keywords, tokens `$80-$A6`, the program's format in memory, the error messages | Chapter 2 |
| The tape format, `UNCT` version 1, and the cassette, version 1 | Chapter 14, `docs/cassettes.md` |
| The character ROM (UNSCII) | `rom/unscii.txt` |

Not fixed by a revision: the Rust APIs (semver, as usual), the Machine Room, the assembler's messages and directives beyond what the manual lists, save states (versioned on their own), how long a ROM call takes, and the manual's prose.

`cargo test` holds Rev A to what it was when it was frozen (`crates/unc-asm/tests/rev_a.rs` against `rev_a.txt`): a change to any of these tables fails with "this is a Rev B change".

## Later revisions

They only add. New hardware comes the way the video board did: a card or chip with an id register, absent on a Rev A machine. A later revision can add opcodes in free slots, registers in free I/O space, ROM calls at new addresses (`$FF24` and up) and BASIC keywords after `$A6`. It can't change anything Rev A fixes.

When the emulator and the manual disagree, the manual wins, unless the behaviour has shipped and programs rely on it. Then it becomes documented behaviour.
