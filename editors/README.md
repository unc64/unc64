# Editor tools for UNC assembly

Modern tools for writing UNC 64 programs on a modern computer: a language server, Vim and
VS Code files, and the language as data for the in-game editor and the website. They live
**outside the machine**. None of them changes the CPU, its timing or the emulation, and
nothing on the UNC 64 can see them.

Everything here comes from one source of truth, `unc_asm::lang` in `crates/unc-asm`:

- The instructions come from the processor's own opcode table (`unc_core::isa`). Their
  descriptions come from `unc-asm/src/doc.rs`, which also generates Appendix C.
- The directives, functions, memory map, I/O registers, ROM routines and their costs, system
  variables and the assembler's messages are read from the manual's own tables when the crate
  builds (`crates/unc-asm/build.rs`). Tests check these tables against the assembler, the
  emulator's I/O map and the ROM's jump table.
- The Vim syntax file and the VS Code grammar are generated from that data. A test fails if
  either file falls behind.

| Path | What |
|---|---|
| `crates/unc-lsp` | The language server, `unc-lsp`, over stdio. |
| `editors/vim/` | `syntax/`, `ftdetect/` and `ftplugin/unc.vim`: for Vim and Neovim, with or without the LSP. |
| `editors/nvim/README.md` | Setting up the LSP in Neovim (0.11+ and lspconfig), Vim (vim-lsp, coc, yegappan/lsp) and Emacs. |
| `editors/vscode/` | A minimal VS Code extension: the grammar plus a small client that starts `unc-lsp`. |

## Install the server

```sh
cargo install --path crates/unc-lsp     # puts unc-lsp in ~/.cargo/bin
unc-lsp --version
```

### Neovim

Add `editors/vim` to the runtimepath. That gives you the filetype, the syntax and the
ftplugin. Then enable the server:

```lua
vim.opt.rtp:append('/path/to/unc-64/editors/vim')
vim.lsp.config('unc_lsp', { cmd = { 'unc-lsp' }, filetypes = { 'unc' }, root_markers = { '.unc', '.git' } })
vim.lsp.enable('unc_lsp')
vim.lsp.inlay_hint.enable(true)
```

[`nvim/README.md`](nvim/README.md) has the full setup, the lspconfig version for older
Neovims, and the settings.

### Vim

The syntax file works with no plugins at all:

```vim
set runtimepath+=/path/to/unc-64/editors/vim
filetype plugin on
syntax on
```

For the LSP, use vim-lsp, coc.nvim or yegappan/lsp. The snippets are in
[`nvim/README.md`](nvim/README.md#vim).

### VS Code

```sh
cd editors/vscode
npm install                                           # vscode-languageclient
ln -s "$PWD" ~/.vscode/extensions/unc-asm             # or copy the folder there
```

Restart VS Code. The extension starts `unc-lsp` from PATH. To use a different binary, set
`unc.serverPath`. `unc.cumulativeCycles` turns on cumulative cycle hints. VS Code shows inlay
hints when `editor.inlayHints.enabled` is on, which is the default.

To package it instead: `npx @vscode/vsce package`, then
`code --install-extension unc-asm-0.1.0.vsix`.

### Emacs

For the Emacs fans, before anyone screams. eglot only, in one line:

```elisp
(with-eval-after-load 'eglot (add-to-list 'eglot-server-programs '(asm-mode "unc-lsp")))
```

Then run `M-x eglot` in a `.s` buffer. Emacs 29 and later show the cycle counts as inlay
hints.

## Which files are UNC sources (ftdetect)

`.s` is the extension the manual, the games and the ROM use. It's also GNU as's extension,
and the extension of every other 6502 assembler, so the Vim files don't claim every `.s`:

- `*.uncs` and `*.unc.s` are always UNC.
- A `*.s` file is UNC when one of these is true:
  - A comment in its first 20 lines names the UNC (`; SNAKE for the UNC 16`). Every source in
    the manual and the repository starts like this.
  - It's under a folder that has a `.unc` file or folder, in its own folder or in any folder
    above it. To mark a project, `touch .unc` at its root.
  - It's inside the unc-64 repository itself, which is detected by `crates/unc-asm` above it.
- Any other `*.s` stays Vim's `asm`. To make every `*.s` UNC, use `let g:unc_all_s = 1`.

The rule is a comment or a marker file, not a guess from the contents, so it's quick and
predictable, and it never takes over an unrelated `.s` file. VS Code follows the same idea:
`.uncs` and `.unc.s` files, and any file whose first line is a comment naming the UNC. For
anything else, add `"files.associations": { "*.s": "unc" }` to the workspace settings.

## The language server

`unc-lsp` reassembles the document with the real assembler on every change, and works from
that. If the program has errors, addresses and symbols are as far as the assembler got, so
hints and hovers stay useful while you type.

| Feature | What it does |
|---|---|
| Diagnostics | Errors and warnings from the assembler, on the line and columns they're about: `undefined symbol 'BORDER'` underlines `BORDER`. The explanation from Appendix E comes as related information. An error in an included file shows on the `.include` line and links to the file's own line. |
| Inlay hints | Each instruction's fixed cost at the end of its line: `4c`. A branch shows `2/3c`: 2 cycles not taken, 3 taken. A line that a `.rept` assembles 8 times shows `5c ×8`. A `.rept`/`.irp` line shows its block's total: `4 × 10 = 40 cycles` (with `+ taken branches` if there are branches inside, and `, ×8 = …` when an outer block repeats it). With `cumulativeCycles`, each line also shows the cycles since the last label: `5c  Σ23` (`+` when a branch on the way could add cycles). |
| Hover | On a mnemonic: the mode it assembled to, the bytes, cycles, opcode, flags, its address and bytes, and the table of all its modes. On a label or constant: its value in hex, decimal and binary, and where it's defined. On a constant or number that's an I/O address, ROM routine or system variable (`$C020`, `BORDER`): the manual's description. A manual name that the program hasn't defined shows its description and the line that would define it. Also hovers for directives, functions and registers. |
| Completion | At the start of a statement: mnemonics (each with a snippet per addressing mode) and directives (`.rept` makes the whole block). In column 0: `BORDER = $C020`-style definitions for the manual's names that aren't defined yet. In an operand: labels, constants, the local labels in scope, functions (`sin(a, n)`), and the manual's names that aren't defined yet. Picking one of those also adds its definition after the program's constants, because the UNC has no built-in names. |
| Code action | A quick fix for `undefined symbol 'BORDER'`: *Define BORDER = $C020 (I/O register $C020)*. |
| Navigation | Go to definition (for labels, locals and constants, also in included files, and on an `.include` file name), find references, the outline (constants, and labels with their locals nested inside), rename (labels, locals and constants, across included files; renaming `clear` also renames `clear@loop`). |
| Also | Semantic tokens, and folding for `.rept`/`.irp` blocks. |

`.include "file.s"` is read relative to the including file, from the editor if the file is
open there, and from disk if it isn't.

Settings: send `{ "cumulativeCycles": true }` as `initializationOptions`, or
`{ "unc": { "cumulativeCycles": true } }` through `workspace/didChangeConfiguration`. When
the client supports it, the server asks it to refresh the hints.

## The data, for other tools

### Where to get it

| From | Language data | One line's tokens | A source's analysis |
|---|---|---|---|
| Rust | `unc_asm::lang::get()` (typed) or `unc_asm::lang::json()` | `unc_asm::lang::tokens(line)` (byte offsets) | `unc_asm::analyze(src)` / `analyze_with(src, resolve)` |
| Command line | `unc-asm --lang-json` | | `unc-asm --analysis-json prog.s` |
| Godot (`UncMachine`, static) | `UncMachine.language() -> String` (JSON) | `UncMachine.tokens(line) -> Array` of `[start, end, kind]` | `UncMachine.analyze(source) -> String` (JSON) |
| WebAssembly | `language_json() -> len` | `tokens_input() -> len`: every line of the input buffer | `analyze_input() -> len` |

The WebAssembly exports follow the crate's existing buffer convention. Put the source in the
buffer from `input_buffer(len)`, call the export, and read `len` bytes of UTF-8 JSON at
`output_ptr()`. `language_json` takes no input.

The language data is a few tens of kilobytes and never changes while the program runs, so
fetch it once.

Columns: the Rust API gives byte offsets. The JSON from Godot, WebAssembly and
`--analysis-json` counts characters (Unicode scalar values), which match JavaScript's UTF-16
indices except for characters outside the Basic Multilingual Plane. The language server
speaks UTF-16, as LSP does. Lines are numbered from 1 everywhere except in LSP messages,
where they're numbered from 0.

### Language data (schema 1)

```jsonc
{
  "schema": 1,                       // goes up when a change would break a reader
  "language": "UNC assembly",
  "cpu": "UNC-1",
  "syntax": {
    "comment": ";", "label_suffix": ":", "local_prefix": "@", "constant": "NAME = value",
    "name": "[A-Za-z_][A-Za-z0-9_]*", "names_case_sensitive": true,
    "case_insensitive": ["mnemonics", "directives", "functions", "registers"],
    "hex_prefix": "$", "binary_prefix": "%", "char_quote": "'", "string_quote": "\"",
    "escapes": ["\\\"", "\\'", "\\\\"], "current_address": "*", "group": ["[", "]"],
    "paste": "\\", "registers": ["A", "X", "Y"],
    "operators": ["||", "&&", "|", "^", "&", "==", "!=", "<", "<=", ">", ">=", "<<", ">>", "+", "-", "*", "/", "%"],
    "unary": ["-", "~", "<", ">"],
    "extensions": [".s", ".uncs", ".unc.s"]
  },
  "token_kinds": ["mnemonic", "directive", "label", "local", "constant", "symbol", "number",
                  "string", "comment", "operator", "register", "function", "invalid"],
  "instructions": [{
    "mnemonic": "LDA",
    "description": "Load A",
    "flags": "N Z",                  // the flags it changes; "-" none, "all" for PLP and RTI
    "branch": false,
    "modes": [{
      "mode": "absx",                // imp acc imm fp fpx fpy abs absx absy ind indy rel
      "name": "absolute,X",          // as the manual and the messages say it
      "syntax": "LDA $nnnn,X",       // as Appendix C writes it
      "snippet": "LDA ${1:addr},X",  // LSP/VS Code snippet syntax
      "opcode": 44, "bytes": 3,
      "cycles": 5,                   // fixed; for a branch, not taken
      "cycles_taken": null           // a branch's cost when taken (cycles + 1)
    }]
  }],
  "directives": [{
    "name": ".byte", "aliases": [".db"],
    "syntax": ".byte value, ...", "snippet": ".byte ${1:value}",
    "example": ".byte 1, 2, $FF",    // the manual's example
    "description": "Store bytes. .db is the same.",
    "block": null                    // "open" for .rept and .irp, "close" for .endr
  }],
  "functions": [{ "name": "sin", "arity": 2, "syntax": "sin(a, n)", "snippet": "sin(${1:a}, ${2:n})", "description": "…" }],
  "io": [{                           // Appendix B
    "name": "BORDER", "address": 49184, "end": 49184, "hex": "$C020",
    "read": "Border colour", "write": "Border colour, 0-15",   // "" where the manual's cell is blank
    "chapter": 7
  }],                                // KEYS is a range: address $C060, end $C068
  "rom": [{                          // Appendix D's jump table, cycles from chapter 13
    "name": "CHROUT", "address": 65283, "hex": "$FF03",
    "in": "A = character; 13 new line, 12 clear, 8 back",
    "out": "Character printed; no cursor drawn", "changes": "nothing",
    "cycles": "about 125, on any row; when it scrolls: about 19,000: nearly a whole frame"   // "" if none given
  }],
  "variables": [{ "name": "CURX", "address": 512, "end": 512, "hex": "$0200", "description": "Cursor column" }],
  "memory": [{ "start": 1024, "end": 2023, "hex": "$0400-$07E7", "description": "Screen memory. 40 × 25 screen codes, row by row." }],
  "messages": [{ "message": "unknown instruction 'LDQ'", "meaning": "Not a UNC-1 mnemonic. …", "severity": "error" }]
}
```

All text is plain, with the manual's markdown removed. Addresses are numbers; `hex` is the
same address written as the manual writes it. The names in `io`, `rom` and `variables` are
not built into the assembler. A program defines the ones it uses (`BORDER = $C020`), and
editors offer to.

### Tokens

`[[start, end, kind], ...]` for one line, with `end` exclusive, and no tokens for whitespace.
`tokens_input` returns one such array per line. The kinds:

| Kind | Is |
|---|---|
| `mnemonic` | `LDA`, in any case |
| `directive` | `.byte`, a known directive |
| `label` | a label being defined: `loop` in `loop:` (`ROW\i` too) |
| `local` | a local label, defined or used: `@loop` |
| `constant` | a constant being defined: `BORDER` in `BORDER = $C020` |
| `symbol` | a name used in an expression: a label, a constant, or a `.rept`/`.irp` name |
| `number` | `$C020`, `%0101`, `42`, `'A'` |
| `string` | `"HELLO"`, with its quotes |
| `comment` | `;` to the end of the line |
| `operator` | `#` `,` `=` `:` `+` `<<` `[` `(` and the rest, `*` as the address too |
| `register` | `A` alone as an operand, `X` or `Y` after the last comma |
| `function` | `sin`, `cos`, `sqrt`, `abs`, `min`, `max` before `(` |
| `invalid` | an unknown mnemonic or directive, a stray character, an unknown function |

The lexer reads a line the way the assembler does. `*` and `%` depend on where they are: `*`
is the address where a value is expected and multiply after one, and `%` is a binary number
or the remainder operator in the same way. The lexer only looks at one line, though, so it
can't tell a label from a constant in an operand. Both are `symbol`. The language server's
semantic tokens make that distinction, because they use the whole program.

### Analysis

```jsonc
{
  "ok": true,                        // no errors
  "start": 2048, "size": 937,        // where it runs from, bytes stored; null with errors
  "errors":   [{ "file": null, "line": 9, "start": 8, "end": 11, "message": "unknown instruction 'LDQ'", "meaning": "Not a UNC-1 mnemonic. …" }],
  "warnings": [ /* the same shape */ ],
  "lines": [{                        // main source lines that have an address or an instruction
    "line": 3, "addr": 2050, "bytes": [52, 0, 4],   // the first copy's
    "mnemonic": "STA", "mode": "absx",              // null for data and labels
    "cycles": 5, "cycles_taken": null,              // fixed cost; a branch's cost when taken
    "copies": 1,                                    // more than 1 inside .rept/.irp
    "total": 5                                      // cycles over every copy
  }],
  "blocks": [{                       // .rept/.irp blocks in the main source
    "kind": "rept", "start": 6, "end": 8,           // lines of .rept and .endr (end null if missing)
    "entered": 1, "copies": 2,                      // copies over every time it's reached
    "cycles": 4, "cycles_per_entry": 4,             // straight through, branches not taken
    "branches": false
  }],
  "symbols": [{ "name": "start@col", "value": 2050, "kind": "local", "file": null, "line": 3, "start": 0, "end": 4 }]
}
```

`file` is `null` for the main source, or the name an `.include` gave. A local label's `name`
is the name the assembler gives it, `global@local`. `value` is `null` if the program's
errors stopped the assembler working it out. With errors there are still `lines` and
`symbols`, as far as the assembler got. The cycle costs are the fixed costs of Appendix C.
The UNC-1 has no page-crossing penalty, so the only variable cost is a taken branch, which
adds 1.

## Keeping it in step

- `cargo test -p unc-asm --test editors` fails if `editors/vim/syntax/unc.vim` or
  `editors/vscode/syntaxes/unc.tmLanguage.json` differs from what the language data makes.
  Regenerate both with `UNC_UPDATE_EDITORS=1 cargo test -p unc-asm --test editors`. To print
  them, run `cargo run -p unc-asm -- --vim-syntax` or `-- --textmate`.
- `cargo test -p unc-asm --test lang` checks the manual's tables against the machine: the I/O
  registers against `unc_core::machine::io`, the ROM routines against the jump table in
  `rom/system.s`, and the directives and functions against the assembler. It also checks that
  every source in the repository analyses cleanly.
- `cargo test -p unc-lsp` drives the server over stdio on `games/snake.s`.
