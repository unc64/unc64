# UNC 64: the full reference, for AI agents

> The UNC 64 is a fantasy 8-bit computer by United Northern Computers (a company that never existed), and the machine in the game UNC 64 (https://unc64.com). It is fully emulated. This file is its complete specification: the UNC 64 Programmer's Reference Guide, followed by the cassette file format. The emulator implements the guide exactly, and every program listing in it is assembled and run by the project's test suite.

Before you write code for it:

- **It is not a Commodore 64, and the UNC-1 is not a 6502.** It's in the 6502 family, but with its own opcode numbering, fixed cycle costs (no page-crossing penalties), a "fast page" where the 6502 has zero page, no decimal mode and no `(zp,X)` mode. Appendix F lists every difference. Don't use C64 addresses, KERNAL calls or 6502 cycle counts.
- **Two machines.** The UNC 16 has 16K of memory, a 40 × 25 text screen in the UNSCII character set, a 320 × 200 bitmap, a beeper and a tape deck. The UNC 64 is the UNC 16 fully expanded: 64K, and the video board (chapter 15: sprites, scrolling, four colours).
- **Where things are:** the assembler's syntax in chapter 6, the screen in chapter 8, timing in chapter 9, the keyboard in chapter 11, sound in chapter 12, the ROM routines in chapter 13 and Appendix D, tape in chapter 14, UNC BASIC in chapter 2, the memory map and I/O registers in Appendix B, every instruction with its cycles in Appendix C, and the assembler's messages in Appendix E.
- **Programs** are UNC-1 assembly (`.s`, for the assembler in chapter 6) or UNC BASIC listings (`.bas`, chapter 2). A finished program travels as a tape image (`.tap`, chapter 14) or a cassette (`.unc.png`, the last section of this file).
- **Example games** with full source run in the browser at https://unc64.com/showcase/.

---

