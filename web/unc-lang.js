// UNC assembly for the web tools: the language reference, a line tokenizer, cycle costs,
// hover docs and a static highlighter. Shared by the Machine Room's editor (web/editor.js)
// and the showcase's "Read the source" on unc64.com. A plain script: it sets
// window.UNCLang, so it works the same from a file, a module or an inlined preview.
//
// The reference comes from the emulator when it can: UNCLang.fromWasm(exports) reads
// language_json() (unc_asm::lang, generated from the CPU's own tables) if the build exports
// it. Otherwise, and for anything that's missing there, it uses the copy below, which
// follows the manual (Appendix B, C and D) and crates/unc-core/src/isa.rs.
//
// The website's showcase keeps a copy of this file: edit this one, then copy it over.
(function () {
  'use strict';

  // ---------- addressing modes ----------
  const MODES = {
    imp: { name: 'implied', syntax: '' },
    acc: { name: 'accumulator', syntax: 'A' },
    imm: { name: 'immediate', syntax: '#$nn' },
    fp: { name: 'fast page', syntax: '$nn' },
    fpx: { name: 'fast page,X', syntax: '$nn,X' },
    fpy: { name: 'fast page,Y', syntax: '$nn,Y' },
    abs: { name: 'absolute', syntax: '$nnnn' },
    absx: { name: 'absolute,X', syntax: '$nnnn,X' },
    absy: { name: 'absolute,Y', syntax: '$nnnn,Y' },
    ind: { name: 'indirect', syntax: '($nnnn)' },
    indy: { name: '(fast page),Y', syntax: '($nn),Y' },
    rel: { name: 'relative', syntax: 'label' },
  };
  const BYTES = { imp: 1, acc: 1, imm: 2, fp: 2, fpx: 2, fpy: 2, indy: 2, rel: 2, abs: 3, absx: 3, absy: 3, ind: 3 };

  // ---------- the built-in reference ----------
  // [mnemonic, operation, flags]: Appendix C, from crates/unc-asm/src/doc.rs.
  const OPS = [
    ['BRK', 'Break: push PC and status (B set), jump through $FFFE', 'I'],
    ['NOP', 'No operation', '-'],
    ['HLT', 'Halt the processor until reset', '-'],
    ['RTS', 'Return from subroutine', '-'],
    ['RTI', 'Return from interrupt: pull status, then PC', 'all'],
    ['JSR', 'Jump to subroutine: push the return address', '-'],
    ['JMP', 'Jump', '-'],
    ['BEQ', 'Branch if equal (Z set)', '-'],
    ['BNE', 'Branch if not equal (Z clear)', '-'],
    ['BCC', 'Branch if carry clear', '-'],
    ['BCS', 'Branch if carry set', '-'],
    ['BMI', 'Branch if minus (N set)', '-'],
    ['BPL', 'Branch if plus (N clear)', '-'],
    ['BVC', 'Branch if overflow clear', '-'],
    ['BVS', 'Branch if overflow set', '-'],
    ['TAX', 'A to X', 'N Z'], ['TAY', 'A to Y', 'N Z'], ['TXA', 'X to A', 'N Z'], ['TYA', 'Y to A', 'N Z'],
    ['TXS', 'X to stack pointer', '-'], ['TSX', 'Stack pointer to X', 'N Z'],
    ['INX', 'X + 1', 'N Z'], ['INY', 'Y + 1', 'N Z'], ['DEX', 'X - 1', 'N Z'], ['DEY', 'Y - 1', 'N Z'],
    ['PHA', 'Push A', '-'], ['PLA', 'Pull A', 'N Z'], ['PHP', 'Push status (with B set)', '-'], ['PLP', 'Pull status', 'all'],
    ['CLC', 'Clear carry', 'C'], ['SEC', 'Set carry', 'C'], ['CLI', 'Clear interrupt disable', 'I'],
    ['SEI', 'Set interrupt disable', 'I'], ['CLV', 'Clear overflow', 'V'],
    ['LDA', 'Load A', 'N Z'], ['STA', 'Store A', '-'],
    ['ADC', 'A + M + C', 'N V Z C'], ['SBC', 'A - M - (1 - C)', 'N V Z C'],
    ['AND', 'A AND M', 'N Z'], ['ORA', 'A OR M', 'N Z'], ['EOR', 'A XOR M', 'N Z'], ['CMP', 'Compare A with M', 'N Z C'],
    ['LDX', 'Load X', 'N Z'], ['LDY', 'Load Y', 'N Z'], ['STX', 'Store X', '-'], ['STY', 'Store Y', '-'],
    ['CPX', 'Compare X with M', 'N Z C'], ['CPY', 'Compare Y with M', 'N Z C'],
    ['INC', 'M + 1', 'N Z'], ['DEC', 'M - 1', 'N Z'],
    ['ASL', 'Shift left; bit 7 to C', 'N Z C'], ['LSR', 'Shift right; bit 0 to C', 'N Z C'],
    ['ROL', 'Rotate left through C', 'N Z C'], ['ROR', 'Rotate right through C', 'N Z C'],
    ['BIT', 'Test bits: Z = A AND M is zero, N = M bit 7, V = M bit 6', 'N V Z'],
  ];
  // The opcode table, built the way crates/unc-core/src/isa.rs builds it.
  const FIXED = [
    [0x00, 'BRK', 'imp', 6], [0x01, 'NOP', 'imp', 2], [0x02, 'HLT', 'imp', 2], [0x03, 'RTS', 'imp', 6],
    [0x04, 'RTI', 'imp', 6], [0x05, 'JSR', 'abs', 6], [0x06, 'JMP', 'abs', 3], [0x07, 'JMP', 'ind', 5],
    [0x08, 'BEQ', 'rel', 2], [0x09, 'BNE', 'rel', 2], [0x0A, 'BCC', 'rel', 2], [0x0B, 'BCS', 'rel', 2],
    [0x0C, 'BMI', 'rel', 2], [0x0D, 'BPL', 'rel', 2], [0x0E, 'BVC', 'rel', 2], [0x0F, 'BVS', 'rel', 2],
    [0x10, 'TAX', 'imp', 2], [0x11, 'TAY', 'imp', 2], [0x12, 'TXA', 'imp', 2], [0x13, 'TYA', 'imp', 2],
    [0x14, 'TXS', 'imp', 2], [0x15, 'TSX', 'imp', 2], [0x16, 'INX', 'imp', 2], [0x17, 'INY', 'imp', 2],
    [0x18, 'DEX', 'imp', 2], [0x19, 'DEY', 'imp', 2], [0x1A, 'PHA', 'imp', 3], [0x1B, 'PLA', 'imp', 4],
    [0x1C, 'PHP', 'imp', 3], [0x1D, 'PLP', 'imp', 4], [0x1E, 'CLC', 'imp', 2], [0x1F, 'SEC', 'imp', 2],
    [0x20, 'CLI', 'imp', 2], [0x21, 'SEI', 'imp', 2], [0x22, 'CLV', 'imp', 2],
  ];
  // [row base, mnemonic, populated slots, slot-7 mode, read-modify-write]
  const ROWS = [
    [0x28, 'LDA', 0x7F, 'imp', 0], [0x30, 'STA', 0x7E, 'imp', 0], [0x38, 'ADC', 0x7F, 'imp', 0],
    [0x40, 'SBC', 0x7F, 'imp', 0], [0x48, 'AND', 0x7F, 'imp', 0], [0x50, 'ORA', 0x7F, 'imp', 0],
    [0x58, 'EOR', 0x7F, 'imp', 0], [0x60, 'CMP', 0x7F, 'imp', 0], [0x68, 'LDX', 0xAB, 'fpy', 0],
    [0x70, 'LDY', 0x1F, 'imp', 0], [0x78, 'STX', 0x8A, 'fpy', 0], [0x80, 'STY', 0x0E, 'imp', 0],
    [0x88, 'CPX', 0x0B, 'imp', 0], [0x90, 'CPY', 0x0B, 'imp', 0], [0x98, 'INC', 0x0E, 'imp', 1],
    [0xA0, 'DEC', 0x0E, 'imp', 1], [0xA8, 'ASL', 0x8E, 'acc', 1], [0xB0, 'LSR', 0x8E, 'acc', 1],
    [0xB8, 'ROL', 0x8E, 'acc', 1], [0xC0, 'ROR', 0x8E, 'acc', 1], [0xC8, 'BIT', 0x0A, 'imp', 0],
  ];
  const SLOTS = ['imm', 'fp', 'fpx', 'abs', 'absx', 'absy', 'indy'];
  const READ = { imm: 2, acc: 2, fp: 3, fpx: 4, fpy: 4, abs: 4, absx: 5, absy: 5, indy: 6 };

  const DIRECTIVES = [
    ['org', '.org address', 'Assemble what follows at this address. The default is $0800.'],
    ['byte', '.byte 1, 2, $FF', 'Store bytes. A string inside .byte stores its characters. .db is the same.'],
    ['db', '.db 1, 2, $FF', 'Store bytes: the same as .byte.'],
    ['word', '.word $C020, loop', 'Store words, low byte first. .dw is the same.'],
    ['dw', '.dw $C020, loop', 'Store words, low byte first: the same as .word.'],
    ['text', '.text "HELLO"', 'Store the characters of a string as their ASCII codes. Write in capitals. .ascii is the same.'],
    ['ascii', '.ascii "HELLO"', 'Store the characters of a string: the same as .text.'],
    ['asciiz', '.asciiz "HELLO"', 'Store a string followed by a zero byte.'],
    ['fill', '.fill count[, value]', 'Store count copies of value. Without a value, zeros.'],
    ['align', '.align n[, value]', 'Move on to the next multiple of n. Between stored bytes the gap is stored as zeros, otherwise reserved; with a value it always stores.'],
    ['res', '.res n', 'Reserve n bytes: the address moves on, but nothing is stored. .dsb is the same.'],
    ['dsb', '.dsb n', 'Reserve n bytes: the same as .res.'],
    ['rept', '.rept n[, counter] ... .endr', 'Assemble the lines up to .endr n times. The counter stands for 0, 1, 2 ... in each copy.'],
    ['irp', '.irp name, values... ... .endr', 'Assemble the lines up to .endr once for each value, with name standing for it.'],
    ['endr', '.endr', 'Ends a .rept or .irp block.'],
    ['include', '.include "file.s"', 'Assemble another file\'s lines here. Not available in the browser: there are no files.'],
    ['assert', '.assert expr[, "message"]', 'Stop with the message if expr is 0, once every symbol is known.'],
  ];
  const FUNCTIONS = [
    ['sin', 'sin(angle, size)', 'size × the sine of angle, rounded. The angle is in 256ths of a circle: 64 is a right angle.'],
    ['cos', 'cos(angle, size)', 'size × the cosine: cos(a, n) is sin(a + 64, n).'],
    ['sqrt', 'sqrt(x)', 'The square root of x, rounded down. x can\'t be negative.'],
    ['abs', 'abs(x)', 'x without its sign.'],
    ['min', 'min(a, b)', 'The smaller of a and b.'],
    ['max', 'max(a, b)', 'The larger of a and b.'],
  ];
  // [name, address, what, kind]: I/O registers (Appendix B), ROM routines and system variables (Appendix D).
  const NAMES = [
    ['VMODE', 0xC000, 'Video mode. Bit 0: 256-glyph character set. Bit 1: bitmap. Bit 2: mixed (bitmap above, the last TEXTROWS text rows below). Bit 3: steady cursor.', 'io'],
    ['RASTER', 0xC001, 'Read: the current raster line, bits 0-7; latches bit 8 for RASTER_HI.', 'io'],
    ['RASTER_HI', 0xC002, 'Read: bit 0 is raster line bit 8, as latched by the last read of RASTER.', 'io'],
    ['RASTER_CMP', 0xC003, 'The raster compare line (0-255) for the raster interrupt.', 'io'],
    ['IRQ_ENABLE', 0xC004, 'Interrupt sources enabled. Bit 0 raster, bit 1 vertical blank.', 'io'],
    ['IRQ_STATUS', 0xC005, 'Read: the sources that have fired. Write 1 bits to acknowledge them.', 'io'],
    ['FRAME', 0xC006, 'Read: the frame counter.', 'io'],
    ['KBD', 0xC010, 'Read: the last key; bit 7 set while it hasn\'t been taken.', 'io'],
    ['KBD_STROBE', 0xC011, 'Read: as KBD (bit 7 included), then clears bit 7. Write: clears bit 7.', 'io'],
    ['HELD', 0xC012, 'Read: the key down now (the latest pressed), or 0.', 'io'],
    ['BORDER', 0xC020, 'Border colour, 0-15.', 'io'],
    ['BACKGROUND', 0xC021, 'Background colour, 0-15.', 'io'],
    ['CHARSET', 0xC022, 'Character set page; $D0 at reset.', 'io'],
    ['BITMAP', 0xC023, 'Bitmap page; $20 at reset.', 'io'],
    ['TEXTROWS', 0xC024, 'Text rows under the bitmap in mixed mode, 0-25; 4 at reset.', 'io'],
    ['BEEPER', 0xC030, 'Write: any value flips SPEAKER bit 1.', 'io'],
    ['SPEAKER', 0xC031, 'Bit 0: timer gate. Bit 1: speaker data (BEEPER flips it). Read bit 5: the timer\'s output.', 'io'],
    ['TIMER_LO', 0xC032, 'Write: the tone timer\'s divisor, low byte.', 'io'],
    ['TIMER_HI', 0xC033, 'Write: the divisor\'s high byte, and loads the divisor (0 is 65,536).', 'io'],
    ['NOISE', 0xC034, 'Read: a pseudo-random byte from the cycle count.', 'io'],
    ['TAPE', 0xC040, 'Bit 0: PLAY down. Bit 1: tape in. Bit 2: motor on (write). Bit 6: overrun. Bit 7: byte ready.', 'io'],
    ['TAPEDATA', 0xC041, 'Read: the byte off the tape; reading takes it (clears TAPE bits 6 and 7).', 'io'],
    ['SHIFT_IN', 0xC050, 'Write: pushes a byte into the shifter\'s window.', 'io'],
    ['SHIFT_AMOUNT', 0xC051, 'The shift, 0-7 (bits 0-2).', 'io'],
    ['SHIFT_OUT', 0xC052, 'Read: the window shifted right by SHIFT_AMOUNT, low byte.', 'io'],
    ['SHIFT_FLIP', 0xC053, 'Write: pushes a byte with its bits reversed.', 'io'],
    ['KEYS', 0xC060, 'The keyboard matrix, rows 0-8 at $C060-$C068: 1 while a key is down.', 'io'],
    ['CLS', 0xFF00, 'Clear the screen; cursor to the top left. Changes A, X.', 'rom'],
    ['CHROUT', 0xFF03, 'Print the character in A; 13 new line, 12 clear, 8 back. Changes nothing.', 'rom'],
    ['GETKEY', 0xFF06, 'Wait for a key: A = its code, bit 7 clear. Changes A.', 'rom'],
    ['PRSTR', 0xFF09, 'Print the zero-terminated string at X/Y (low byte in X). Changes A, Y.', 'rom'],
    ['PRHEX', 0xFF0C, 'Print A as two hex digits. Changes A.', 'rom'],
    ['PRDEC', 0xFF0F, 'Print the 16-bit number in X/Y (low byte in X) in unsigned decimal. Changes A, X; $E4-$E5, $F0-$F1.', 'rom'],
    ['RND', 0xFF12, 'A = a pseudo-random byte; SEED advanced. Changes A.', 'rom'],
    ['LOAD', 0xFF15, 'Load the program named by the zero-terminated string at X/Y (empty: the first) and start it. Doesn\'t return.', 'rom'],
    ['NUMSTR', 0xFF18, 'The 24-bit number in X/Y/A (low byte in X) as eight ASCII digits in NUMBUF; A = index of the first that isn\'t a leading zero. Changes A, X, Y; $E4-$E6.', 'rom'],
    ['PRNUM', 0xFF1B, 'Print NUMBUF: A = width, 0 for no leading zeros, 1-8 to pad with zeros. Changes A, X; $F0.', 'rom'],
    ['RNDSEED', 0xFF1E, 'SEED = the 16-bit number in X/Y (low byte in X); 0 becomes $ACE1. Changes A.', 'rom'],
    ['READY', 0xFF21, 'Back to BASIC\'s READY. with SP = $FF and I clear; memory untouched. Doesn\'t return.', 'rom'],
    ['SRC', 0xF7, 'ROM pointer ($F7-$F8).', 'var'],
    ['DST', 0xF9, 'ROM pointer ($F9-$FA).', 'var'],
    ['PTR', 0xFB, 'ROM pointer: the cursor\'s row ($FB-$FC).', 'var'],
    ['STRP', 0xFD, 'ROM pointer: PRSTR\'s string ($FD-$FE).', 'var'],
    ['CURX', 0x0200, 'Cursor column.', 'var'],
    ['CURY', 0x0201, 'Cursor row.', 'var'],
    ['TMPA', 0x0202, 'ROM scratch.', 'var'],
    ['RAMTOP', 0x0203, '$40 (16K) or $C0 (64K).', 'var'],
    ['RUNFLG', 0x0204, '0 in direct mode; not 0 while a BASIC program runs.', 'var'],
    ['CURLIN', 0x0205, 'Number of the BASIC line being run ($0205-$0206).', 'var'],
    ['FORSP', 0x0207, 'Bytes in use on the FOR stack.', 'var'],
    ['GOSP', 0x0208, 'Bytes in use on the GOSUB stack.', 'var'],
    ['PRGEND', 0x0209, 'Address of the BASIC program\'s end marker ($0209-$020A).', 'var'],
    ['SEED', 0x020B, 'RND state ($020B-$020C), shared by BASIC and the ROM\'s RND.', 'var'],
    ['INKM', 0x020D, 'The current INK.', 'var'],
    ['GFXS', 0x020E, 'Graphics scratch.', 'var'],
    ['CURSAV', 0x020F, 'The character under the screen editor\'s cursor.', 'var'],
    ['GOSTK', 0x0210, 'GOSUB stack: 16 frames of 5 bytes ($0210-$025F).', 'var'],
    ['VARS', 0x0260, 'BASIC variables A-Z, 2 bytes each, low byte first ($0260-$0293).', 'var'],
    ['FORSTK', 0x0294, 'FOR stack: 8 frames of 10 bytes ($0294-$02E3).', 'var'],
    ['TNAME', 0x02E4, 'LOAD: the name asked for, padded with spaces ($02E4-$02F3).', 'var'],
    ['TNAMED', 0x02F4, 'LOAD: not 0 if a name was given.', 'var'],
    ['THDR', 0x02F5, 'LOAD: the header as read ($02F5-$030C).', 'var'],
    ['TCHK', 0x030D, 'LOAD: the bytes so far, EORed.', 'var'],
    ['TOVR', 0x030E, 'LOAD: $40 if a byte was missed.', 'var'],
    ['TBORD', 0x030F, 'LOAD: the border colour before LOAD.', 'var'],
    ['IRQVEC', 0x0314, 'Interrupt handler address ($0314-$0315).', 'var'],
    ['NMIVEC', 0x0316, 'NMI handler address: the RESTORE key ($0316-$0317).', 'var'],
    ['IBUF', 0x0320, 'The line being typed: 120 characters and a 0 ($0320-$0398).', 'var'],
    ['INBUF', 0x03A0, 'INPUT\'s line: 40 characters and a 0 ($03A0-$03C8).', 'var'],
    ['NUMBUF', 0x03D0, 'NUMSTR\'s eight ASCII digits, highest first ($03D0-$03D7).', 'var'],
    ['RLCOL', 0x03D8, 'The screen editor: the column the cursor was in when it began to wait.', 'var'],
    ['RLROW', 0x03D9, 'The screen editor: the row the cursor was on when it began to wait.', 'var'],
    ['LINK', 0x03DA, 'The screen editor: a byte a row, 1 if the row carries on the line of the row above ($03DA-$03F2).', 'var'],
    ['RLMOV', 0x03F3, 'The screen editor: not 0 once the cursor has been moved.', 'var'],
  ];

  function builtIn() {
    const mnemonics = {};
    for (const [m, what, flags] of OPS) mnemonics[m] = { name: m, description: what, flags, modes: [], branch: /^B(EQ|NE|CC|CS|MI|PL|VC|VS)$/.test(m) };
    const add = (code, m, mode, cycles) => mnemonics[m].modes.push({ mode, opcode: code, bytes: BYTES[mode], cycles });
    for (const [code, m, mode, cyc] of FIXED) add(code, m, mode, cyc);
    for (const [base, m, mask, slot7, rmw] of ROWS) {
      for (let s = 0; s < 8; s++) {
        if (!(mask & (1 << s))) continue;
        const mode = s < 7 ? SLOTS[s] : slot7;
        const read = READ[mode] ?? 2;
        add(base + s, m, mode, rmw ? (mode === 'acc' ? 2 : read + 2) : read);
      }
    }
    const directives = {};
    for (const [n, syntax, description] of DIRECTIVES) directives[n] = { name: n, syntax, description };
    const functions = {};
    for (const [n, syntax, description] of FUNCTIONS) functions[n] = { name: n, syntax, description };
    const names = {};
    for (const [n, address, description, kind] of NAMES) names[n] = { name: n, address, description, kind };
    return { source: 'built-in', mnemonics, directives, functions, names };
  }

  // ---------- language_json() from the emulator ----------
  // Folds whatever language_json() returns over the built-in reference, accepting the
  // likely shapes (arrays of records or objects keyed by name), so a field the emulator
  // doesn't give keeps the built-in value.
  const MODE_ALIASES = {
    implied: 'imp', imp: 'imp', accumulator: 'acc', acc: 'acc', immediate: 'imm', imm: 'imm',
    'fast page': 'fp', fp: 'fp', zp: 'fp', 'fast page,x': 'fpx', fpx: 'fpx', 'fast page,y': 'fpy', fpy: 'fpy',
    absolute: 'abs', abs: 'abs', 'absolute,x': 'absx', absx: 'absx', 'absolute,y': 'absy', absy: 'absy',
    indirect: 'ind', ind: 'ind', '(fast page),y': 'indy', indy: 'indy', relative: 'rel', rel: 'rel',
  };
  const modeKey = (m) => {
    const k = String(m || '').toLowerCase().trim();
    return MODE_ALIASES[k] ?? MODE_ALIASES[k.replace(/[\s_]+/g, ' ')] ?? MODE_ALIASES[k.replace(/[\s_]+/g, '')] ?? null;
  };
  const num = (v) => (typeof v === 'number' ? v : typeof v === 'string' && /^\$[0-9a-f]+$/i.test(v) ? parseInt(v.slice(1), 16)
    : typeof v === 'string' && /^(0x)?[0-9a-f]+$/i.test(v) ? parseInt(v, v.startsWith('0x') ? 16 : 10) : undefined);
  const records = (x) => (Array.isArray(x) ? x : x && typeof x === 'object' ? Object.entries(x).map(([k, v]) => ({ name: k, ...(typeof v === 'object' ? v : { description: v }) })) : []);
  const text = (r, ...keys) => { for (const k of keys) if (typeof r[k] === 'string' && r[k]) return r[k]; return undefined; };

  function normalize(json) {
    const lang = builtIn();
    if (!json || typeof json !== 'object') return lang;
    lang.source = 'unc.wasm';
    for (const r of records(json.mnemonics || json.instructions || json.ops)) {
      const name = String(r.name || r.mnemonic || '').toUpperCase();
      if (!name) continue;
      const m = lang.mnemonics[name] || (lang.mnemonics[name] = { name, description: '', flags: '-', modes: [], branch: false });
      m.description = text(r, 'description', 'summary', 'operation', 'doc') ?? m.description;
      if (r.flags !== undefined) m.flags = Array.isArray(r.flags) ? (r.flags.join(' ') || '-') : String(r.flags);
      if (typeof r.branch === 'boolean') m.branch = r.branch;
      const modes = records(r.modes).map((x) => ({
        mode: modeKey(x.mode ?? x.name), opcode: num(x.opcode), bytes: num(x.bytes), cycles: num(x.cycles),
      })).filter((x) => x.mode && x.cycles !== undefined);
      if (modes.length) m.modes = modes.map((x) => ({ ...x, bytes: x.bytes ?? BYTES[x.mode] }));
    }
    for (const r of records(json.directives)) {
      const name = String(r.name || '').replace(/^\./, '').toLowerCase();
      if (!name) continue;
      const d = lang.directives[name] || (lang.directives[name] = { name, syntax: '.' + name, description: '' });
      d.syntax = text(r, 'syntax', 'signature', 'usage') ?? d.syntax;
      d.description = text(r, 'description', 'summary', 'doc') ?? d.description;
      for (const a of Array.isArray(r.aliases) ? r.aliases : []) {
        const k = String(a).replace(/^\./, '').toLowerCase();
        if (k && !lang.directives[k]) lang.directives[k] = { ...d, name: k };
      }
    }
    for (const r of records(json.functions)) {
      const name = String(r.name || '').toLowerCase();
      if (!name) continue;
      const f = lang.functions[name] || (lang.functions[name] = { name, syntax: name + '()', description: '' });
      f.syntax = text(r, 'syntax', 'signature', 'usage') ?? f.syntax;
      f.description = text(r, 'description', 'summary', 'doc') ?? f.description;
    }
    const group = (list, kind) => {
      for (const r of records(list)) {
        const name = String(r.name || '');
        const address = num(r.address ?? r.addr ?? r.value);
        if (!name || address === undefined) continue;
        const old = lang.names[name] || {};
        const rw = [r.read && `Read: ${r.read}.`, r.write && `Write: ${r.write}.`, r.in && `In: ${r.in}.`,
          r.out && `Out: ${r.out}.`, r.changes && `Changes: ${r.changes}.`].filter(Boolean).join(' ');
        lang.names[name] = { name, address, kind: old.kind || kind, description: text(r, 'description', 'summary', 'doc', 'what') || rw || old.description || '' };
      }
    };
    group(json.registers || json.io || json.io_registers, 'io');
    group(json.rom || json.routines || json.rom_routines, 'rom');
    group(json.variables || json.system_variables, 'var');
    return lang;
  }

  // Reads language_json() from an instance of unc.wasm, using its input/output buffer
  // convention (it returns the length of the JSON in the output buffer).
  function fromWasm(ex) {
    if (!ex || typeof ex.language_json !== 'function') return builtIn();
    try {
      const n = ex.language_json();
      const bytes = new Uint8Array(ex.memory.buffer, ex.output_ptr(), n);
      return normalize(JSON.parse(new TextDecoder().decode(bytes)));
    } catch (e) {
      console.warn('language_json() unreadable; using the built-in reference', e);
      return builtIn();
    }
  }

  // ---------- tokens ----------
  // One line at a time; nothing in UNC assembly spans lines. Token types:
  //   comment, labeldef, localdef, constdef, mnemonic, directive, number, string, char,
  //   function, register, name, local, hw (a register, ROM routine or system variable name),
  //   pc (* as an address), operator, bracket, punct, paste (\name), invalid.
  const IDENT = /^[A-Za-z_@][\w@]*(?:\\\w+)*/;
  const VALUE_END = new Set(['number', 'char', 'string', 'name', 'local', 'hw', 'pc', 'register']);

  function tokenizeLine(line, lang) {
    const out = [];
    const push = (from, to, type) => { if (to > from) out.push({ from, to, type, text: line.slice(from, to) }); };
    let i = 0;
    const ws = () => { while (i < line.length && (line[i] === ' ' || line[i] === '\t')) i++; };
    ws();
    // NAME = value
    let m = /^([A-Za-z_][\w]*)(\s*)=/.exec(line.slice(i));
    let stmt = true;
    if (m) {
      push(i, i + m[1].length, 'constdef');
      i += m[1].length + m[2].length;
      push(i, i + 1, 'operator');
      i += 1;
      stmt = false;
    } else if ((m = /^([A-Za-z_@][\w@]*(?:\\\w+)*)(\s*):/.exec(line.slice(i)))) {
      push(i, i + m[1].length, m[1][0] === '@' ? 'localdef' : 'labeldef');
      i += m[1].length + m[2].length;
      push(i, i + 1, 'punct');
      i += 1;
    }
    let mnemonic = null;
    if (stmt) {
      ws();
      const w = /^[^\s;]+/.exec(line.slice(i));
      if (w) {
        const word = w[0];
        if (word[0] === '.') push(i, i + word.length, 'directive');
        else if (lang.mnemonics[word.toUpperCase()]) { push(i, i + word.length, 'mnemonic'); mnemonic = word.toUpperCase(); }
        else push(i, i + word.length, 'invalid');
        i += word.length;
      }
    }
    // The operand or expression.
    const operandStart = i;
    const commentAt = findComment(line, i);
    const opText = line.slice(operandStart, commentAt).trim();
    const accOnly = mnemonic && /^a$/i.test(opText);
    let wantValue = true;
    let prevType = null;
    while (i < commentAt) {
      ws();
      if (i >= commentAt) break;
      const c = line[i];
      const rest = line.slice(i, commentAt);
      let t = null;
      let len = 1;
      if (c === '"') {
        const s = /^"(?:\\.|[^"\\])*"?/.exec(rest);
        len = s[0].length; t = 'string';
      } else if (c === "'") {
        const s = /^'(?:\\.|[^'\\])'/.exec(rest);
        if (s) { len = s[0].length; t = 'char'; } else t = 'invalid';
      } else if (c === '$') {
        len = /^\$[0-9A-Fa-f]*/.exec(rest)[0].length; t = 'number';
      } else if (c === '%' && wantValue && /^%[01]/.test(rest)) {
        len = /^%[01]+/.exec(rest)[0].length; t = 'number';
      } else if (/[0-9]/.test(c)) {
        len = /^[0-9]\w*/.exec(rest)[0].length; t = 'number';
      } else if (/[A-Za-z_@]/.test(c)) {
        const name = IDENT.exec(rest)[0];
        len = name.length;
        const after = rest.slice(len).trimStart();
        if (after[0] === '(' && lang.functions[name.toLowerCase()]) t = 'function';
        else if (accOnly && /^a$/i.test(name)) t = 'register';
        else if (/^[xy]$/i.test(name) && prevType === 'punct') t = 'register';
        else if (name[0] === '@') t = 'local';
        else if (lang.names[name]) t = 'hw';
        else t = 'name';
      } else if (c === '\\') {
        const s = /^\\\w*/.exec(rest);
        len = s[0].length; t = 'paste';
      } else if (c === '*' && wantValue) {
        t = 'pc';
      } else if (c === '#') {
        t = 'operator';
      } else if ('[('.includes(c)) {
        t = 'bracket';
      } else if ('])'.includes(c)) {
        t = 'bracket';
      } else if (c === ',') {
        t = 'punct';
      } else {
        const two = rest.slice(0, 2);
        if (['<<', '>>', '<=', '>=', '==', '!=', '&&', '||'].includes(two)) { len = 2; t = 'operator'; }
        else if ('+-*/%&|^~<>!='.includes(c)) t = 'operator';
        else t = 'invalid';
      }
      push(i, i + len, t);
      i += len;
      prevType = t;
      wantValue = !(VALUE_END.has(t) || (t === 'bracket' && '])'.includes(c)));
    }
    if (commentAt < line.length) push(commentAt, line.length, 'comment');
    return out;
  }

  // Where the comment starts: the first ';' outside a string or character literal.
  function findComment(line, from = 0) {
    let q = null;
    for (let i = from; i < line.length; i++) {
      const c = line[i];
      if (q) {
        if (c === '\\') i++;
        else if (c === q) q = null;
      } else if (c === '"') q = '"';
      else if (c === "'" && /^'(?:\\.|[^'\\])'/.test(line.slice(i))) { i += line[i + 1] === '\\' ? 3 : 2; }
      else if (c === ';') return i;
    }
    return line.length;
  }

  // ---------- one line's parts ----------
  function parseLine(line, lang) {
    const toks = tokenizeLine(line, lang);
    const r = { tokens: toks, label: null, constant: null, mnemonic: null, directive: null, operand: '' };
    for (const t of toks) {
      if (t.type === 'labeldef' || t.type === 'localdef') r.label = t.text;
      else if (t.type === 'constdef') r.constant = t.text;
      else if (t.type === 'mnemonic') r.mnemonic = t.text.toUpperCase();
      else if (t.type === 'directive') r.directive = t.text.slice(1).toLowerCase();
    }
    const at = toks.find((t) => t.type === 'mnemonic' || t.type === 'directive');
    if (at) r.operand = line.slice(at.to, findComment(line, at.to)).trim();
    return r;
  }

  // ---------- cycles ----------
  const forcesAbs = (e) => /^\$[0-9A-Fa-f]{3,}$/.test(e.trim());

  // The addressing mode of an instruction line, as the assembler picks it (crates/unc-asm
  // parse_operand and choose_mode). `len` is the line's length in bytes from the last
  // assembly, which settles fast page against absolute exactly; without it, `value(expr)`
  // guesses from known symbols and the mode is marked a guess.
  function modeOf(op, operand, len, value) {
    const has = (m) => op.modes.some((x) => x.mode === m);
    const s = operand.trim();
    const up = s.toUpperCase().replace(/ /g, '');
    let pair = null;
    let expr = s;
    if (!s) return has('imp') ? 'imp' : has('acc') ? 'acc' : null;
    if (up === 'A') return has('acc') ? 'acc' : null;
    if (s[0] === '#') return has('imm') ? 'imm' : null;
    if (op.branch) return 'rel';
    if (s[0] === '(' && up.endsWith('),Y')) return has('indy') ? 'indy' : null;
    if (s[0] === '(' && s.endsWith(')')) return has('ind') ? 'ind' : null;
    if (up.endsWith(',X')) { pair = ['fpx', 'absx']; expr = s.slice(0, s.lastIndexOf(',')); }
    else if (up.endsWith(',Y')) { pair = ['fpy', 'absy']; expr = s.slice(0, s.lastIndexOf(',')); }
    else pair = ['fp', 'abs'];
    const [fp, abs] = pair;
    if (len === 2 && has(fp)) return fp;
    if (len === 3 && has(abs)) return abs;
    const v = value ? value(expr.trim()) : undefined;
    const small = !forcesAbs(expr) && v !== undefined && v >= 0 && v <= 255;
    if ((small || !has(abs)) && has(fp)) return fp;
    if (has(abs)) return abs;
    return null;
  }

  // A simple value for an operand: a number, a character or a known name. Anything more is
  // left to the assembler (undefined).
  function simpleValue(expr, symbols) {
    const e = expr.trim();
    let m;
    if ((m = /^\$([0-9A-Fa-f]+)$/.exec(e))) return parseInt(m[1], 16);
    if ((m = /^%([01]+)$/.exec(e))) return parseInt(m[1], 2);
    if (/^\d+$/.test(e)) return Number(e);
    if ((m = /^'(\\.|[^'\\])'$/.exec(e))) return m[1].charCodeAt(m[1].length - 1);
    if (symbols && IDENT.test(e) && IDENT.exec(e)[0] === e) return symbols.get(e);
    return undefined;
  }

  // The cost of one line: { cycles, taken, mode, guess } or null if it isn't an instruction.
  // `taken` is the cost of a taken branch.
  function lineCycles(lang, line, len, symbols) {
    const p = typeof line === 'string' ? parseLine(line, lang) : line;
    if (!p.mnemonic) return null;
    const op = lang.mnemonics[p.mnemonic];
    const mode = modeOf(op, p.operand, len, (e) => simpleValue(e, symbols));
    const info = op.modes.find((x) => x.mode === mode);
    if (!info) return null;
    return { cycles: info.cycles, taken: op.branch ? info.cycles + 1 : null, mode, guess: !len && /fp|abs/.test(mode) };
  }
  const cycleText = (c) => (c.taken ? `${c.cycles}/${c.taken}` : String(c.cycles));

  // ---------- docs ----------
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const hex = (v, n) => '$' + (v >>> 0).toString(16).toUpperCase().padStart(n, '0');
  const addr = (v) => (v < 256 ? hex(v, 2) : hex(v, 4));
  const KIND = { io: 'I/O register', rom: 'ROM routine', var: 'System variable' };

  function mnemonicDoc(lang, name, here) {
    const op = lang.mnemonics[name];
    if (!op) return null;
    const rows = op.modes.map((x) => {
      const cyc = op.branch ? `${x.cycles} (+1 if taken)` : x.cycles;
      const syn = (name + ' ' + MODES[x.mode].syntax).trim();
      return `<tr${here && here.mode === x.mode ? ' class="ul-cur"' : ''}><td><code>${esc(syn)}</code></td><td>${esc(MODES[x.mode].name)}</td>`
        + `<td class="ul-n">${x.bytes}</td><td class="ul-n">${cyc}</td></tr>`;
    }).join('');
    const line = here ? `<p class="ul-here">This line: ${esc(MODES[here.mode].name)}, ${cycleText(here)} cycle${here.cycles === 1 && !here.taken ? '' : 's'}${here.guess ? ' (if the operand fits the fast page as written)' : ''}</p>` : '';
    return `<div class="ul-doc"><p class="ul-h"><b>${name}</b> ${esc(op.description)}</p>`
      + `<p class="ul-flags">Flags: ${esc(op.flags)}</p>${line}`
      + `<table class="ul-modes"><thead><tr><th>Syntax</th><th>Mode</th><th>Bytes</th><th>Cycles</th></tr></thead><tbody>${rows}</tbody></table></div>`;
  }

  // Docs for one token. ctx: { symbols: Map(name -> value), defined: Map(name -> line),
  // scope: the global label a local label belongs to, here: this line's cycles }.
  function docHTML(lang, tok, ctx = {}) {
    const t = tok.type;
    if (t === 'mnemonic') return mnemonicDoc(lang, tok.text.toUpperCase(), ctx.here);
    if (t === 'directive') {
      const d = lang.directives[tok.text.slice(1).toLowerCase()];
      if (!d) return `<div class="ul-doc"><p class="ul-h"><b>${esc(tok.text)}</b> isn't a directive.</p></div>`;
      return `<div class="ul-doc"><p class="ul-h"><code>${esc(d.syntax)}</code></p><p>${esc(d.description)}</p></div>`;
    }
    if (t === 'function') {
      const f = lang.functions[tok.text.toLowerCase()];
      return f && `<div class="ul-doc"><p class="ul-h"><code>${esc(f.syntax)}</code></p><p>${esc(f.description)}</p></div>`;
    }
    if (t === 'number' || t === 'char') {
      const v = simpleValue(tok.text);
      if (v === undefined || !Number.isFinite(v)) return null;
      const bin = v < 65536 ? ` · %${v.toString(2).padStart(v < 256 ? 8 : 16, '0')}` : '';
      return `<div class="ul-doc"><p class="ul-h"><code>${v < 65536 ? addr(v) : '$' + v.toString(16).toUpperCase()}</code> · ${v}${bin}</p></div>`;
    }
    if (t === 'register') return `<div class="ul-doc"><p class="ul-h"><b>${tok.text.toUpperCase()}</b> ${/^a$/i.test(tok.text) ? 'The accumulator' : `Index register ${tok.text.toUpperCase()}`}</p></div>`;
    if (['labeldef', 'localdef', 'constdef', 'name', 'local', 'hw'].includes(t)) {
      const full = tok.text[0] === '@' && ctx.scope ? ctx.scope + tok.text : tok.text;
      const v = ctx.symbols && ctx.symbols.get(full);
      const where = ctx.defined && ctx.defined.get(full);
      const hw = lang.names[tok.text];
      const parts = [];
      if (v !== undefined) {
        parts.push(`<p class="ul-h"><b>${esc(tok.text)}</b> = <code>${addr(v & 0xFFFF)}</code> · ${v}${where ? ` <span class="ul-dim">line ${where}</span>` : ''}</p>`);
      } else if (where) {
        parts.push(`<p class="ul-h"><b>${esc(tok.text)}</b> <span class="ul-dim">defined on line ${where}</span></p>`);
      }
      if (hw) {
        if (!parts.length) parts.push(`<p class="ul-h"><b>${esc(hw.name)}</b> <code>${addr(hw.address)}</code></p>`);
        parts.push(`<p><span class="ul-kind">${KIND[hw.kind] || 'Name'} ${addr(hw.address)}</span> ${esc(hw.description)}</p>`);
        if (v === undefined && !where) parts.push(`<p class="ul-dim">Not defined in this program. Add <code>${esc(defineLine(hw))}</code></p>`);
        else if (v !== undefined && (v & 0xFFFF) !== hw.address) parts.push(`<p class="ul-dim">The manual's ${esc(hw.name)} is ${addr(hw.address)}; this program gives the name another value.</p>`);
      }
      if (!parts.length) return t === 'name' || t === 'local' ? `<div class="ul-doc"><p class="ul-h"><b>${esc(tok.text)}</b> <span class="ul-dim">not defined</span></p></div>` : null;
      return `<div class="ul-doc">${parts.join('')}</div>`;
    }
    return null;
  }

  // The constant line that defines a known name, in the samples' layout: `BORDER  = $C020`.
  const defineLine = (hw) => `${hw.name.padEnd(7)} = ${addr(hw.address)}`;

  // ---------- program-wide facts from the source ----------
  // Definitions (name -> line) and, per line, the global label a local label belongs to.
  function scan(src, lang) {
    const lines = src.split('\n');
    const defined = new Map();
    const scopes = new Array(lines.length);
    let scope = '';
    lines.forEach((l, i) => {
      const toks = tokenizeLine(l, lang);
      for (const t of toks) {
        if (t.type === 'labeldef') { scope = t.text; defined.set(t.text, i + 1); }
        else if (t.type === 'localdef') defined.set(scope + t.text, i + 1);
        else if (t.type === 'constdef') defined.set(t.text, i + 1);
      }
      scopes[i] = scope;
    });
    return { lines, defined, scopes };
  }

  // Line -> length in bytes, from assemble_input()'s `lines` ([line, addr, len]); the first
  // copy wins for .rept lines.
  function lengths(asmLines) {
    const m = new Map();
    for (const [line, , len] of asmLines || []) if (!m.has(line)) m.set(line, len);
    return m;
  }

  // ---------- static highlighting (the showcase) ----------
  // The source as HTML: one <span class="ul-line"> per line, a cycles column, and a
  // span per token with its type in data-t. `asm` is assemble_input()'s result, if any.
  function renderHTML(src, lang, asm) {
    const lens = asm && asm.ok ? lengths(asm.lines) : new Map();
    const symbols = new Map(asm && asm.ok ? asm.symbols : []);
    const lines = src.replace(/\n$/, '').split('\n');
    let html = '';
    lines.forEach((l, i) => {
      const p = parseLine(l, lang);
      const c = lineCycles(lang, p, lens.get(i + 1), symbols);
      let row = '';
      let at = 0;
      for (const t of p.tokens) {
        row += esc(l.slice(at, t.from)) + `<span class="ul-${t.type}" data-t="${t.type}">${esc(t.text)}</span>`;
        at = t.to;
      }
      row += esc(l.slice(at));
      const cy = c ? `<span class="ul-cy" data-cy="${c.cycles}">${cycleText(c)}</span>` : '<span class="ul-cy"></span>';
      html += `<span class="ul-line" data-line="${i + 1}">${cy}${row}</span>\n`;
    });
    return html;
  }

  window.UNCLang = {
    MODES, builtIn, normalize, fromWasm, tokenizeLine, findComment, parseLine, modeOf, lineCycles, cycleText,
    simpleValue, docHTML, mnemonicDoc, defineLine, scan, lengths, renderHTML, addr,
  };
})();
