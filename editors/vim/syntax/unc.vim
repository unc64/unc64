" Vim syntax file
" Language:    UNC assembly (the UNC 64's assembler: Programmer's Reference Guide, chapter 6)
" Maintainer:  the unc-64 repository, editors/vim
"
" Generated from unc_asm::lang by `cargo run -p unc-asm -- --vim-syntax`. Don't edit it by
" hand: crates/unc-asm/tests/editors.rs fails when it differs from what that prints.

if exists("b:current_syntax")
  finish
endif

let s:cpo_save = &cpo
set cpo&vim

" Mnemonics, directives, functions and registers are in any case; names aren't.
syn case ignore

syn keyword uncMnemonic BRK NOP HLT RTS RTI JSR JMP TAX TAY TXA TYA TXS TSX INX INY DEX DEY PHA PLA PHP PLP CLC
syn keyword uncMnemonic SEC CLI SEI CLV LDA STA ADC SBC AND ORA EOR CMP LDX LDY STX STY CPX CPY INC DEC ASL LSR
syn keyword uncMnemonic ROL ROR BIT
syn keyword uncBranch BEQ BNE BCC BCS BMI BPL BVC BVS

syn match uncDirectiveBad "^\%(\s*@\=\h\w*:\)\=\s*\zs\.\h\w*"
syn match uncDirective "\.\%(align\|ascii\|asciiz\|assert\|byte\|db\|dsb\|dw\|endr\|fill\|include\|irp\|org\|rept\|res\|text\|word\)\>"
syn match uncFunction "\<\%(sin\|cos\|sqrt\|abs\|min\|max\)\ze\s*("

" X and Y after the last comma, A alone after a shift or rotate.
syn match uncRegister "\%(,\s*\)\@<=[xy]\>\ze\s*\%(;\|$\)"
syn match uncRegister "\%(\<\%(asl\|lsr\|rol\|ror\)\s\+\)\@<=a\>\ze\s*\%(;\|$\)"

syn case match

syn match uncOperator "[-+*/%&|^~<>=!#\[\](),:]"
syn match uncPaste "\\\h\w*"

" The names the manual gives addresses (I/O registers, ROM routines, system variables).
" The UNC has no built-in symbol table: a program defines the ones it uses.
syn match uncSystem "\<\%(BACKGROUND\|BEEPER\|BITMAP\|BOARD\|BORDER\|CHARSET\|CHROUT\|CLS\|COLOUR1\|COLOUR2\)\>"
syn match uncSystem "\<\%(COLOUR3\|COLOURMODE\|CURLIN\|CURSAV\|CURX\|CURY\|CUTOFF\|DATAL\|DATAP\|DST\)\>"
syn match uncSystem "\<\%(ENV3\|FILTER\|FORSP\|FORSTK\|FRAME\|GETKEY\|GFXS\|GOSP\|GOSTK\|HELD\)\>"
syn match uncSystem "\<\%(IBUF\|INBUF\|INKM\|IRQVEC\|IRQ_ENABLE\|IRQ_STATUS\|KBD\|KBD_STROBE\|KEYS\|LAYER\)\>"
syn match uncSystem "\<\%(LAYERPAGE\|LAYERSX\|LAYERSY\|LINK\|LOAD\|NMIVEC\|NOISE\|NUMBUF\|NUMSTR\|PRDEC\)\>"
syn match uncSystem "\<\%(PRGEND\|PRHEX\|PRNUM\|PRSTR\|PTR\|RAMTOP\|RASTER\|RASTER_CMP\|RASTER_HI\|READY\)\>"
syn match uncSystem "\<\%(RLCOL\|RLMOV\|RLROW\|RND\|RNDSEED\|RUNFLG\|SCROLLX\|SCROLLY\|SEED\|SHIFT_AMOUNT\)\>"
syn match uncSystem "\<\%(SHIFT_FLIP\|SHIFT_IN\|SHIFT_OUT\|SOUND\|SPEAKER\|SPRBANK\|SPRHIT\|SPRHITBG\|SPRLINE\|SPRTAB\)\>"
syn match uncSystem "\<\%(SRC\|STRP\|TAPE\|TAPEDATA\|TBORD\|TCHK\|TEXTROWS\|THDR\|TIMER_HI\|TIMER_LO\)\>"
syn match uncSystem "\<\%(TMPA\|TNAME\|TNAMED\|TOVR\|VARS\|VMODE\|VOICE3\|VOLUME\|WAVETABLE\)\>"

" Definitions: `name:` and `@name:` at the start of a line, `NAME = value` on its own.
syn match uncLabel "^\s*\zs\h\w*\%(\\\h\w*\)*\ze\s*:"
syn match uncLocal "@\h\w*"
syn match uncConstant "^\s*\zs\h\w*\ze\s*=[^=]"

syn match uncNumber "\$\x\+\>"
syn match uncNumber "%[01]\+\>"
syn match uncNumber "\<\d\+\>"
syn match uncChar "'\%(\\.\|[^\\']\)'"
syn region uncString start=+"+ skip=+\\.+ end=+"+ oneline
syn keyword uncTodo contained TODO FIXME XXX NOTE
syn match uncComment ";.*$" contains=uncTodo

hi def link uncMnemonic     Statement
hi def link uncBranch       Conditional
hi def link uncDirective    PreProc
hi def link uncDirectiveBad Error
hi def link uncFunction     Function
hi def link uncRegister     Type
hi def link uncOperator     Operator
hi def link uncPaste        Special
hi def link uncLabel        Label
hi def link uncLocal        Identifier
hi def link uncConstant     Constant
hi def link uncSystem       Special
hi def link uncNumber       Number
hi def link uncChar         Character
hi def link uncString       String
hi def link uncTodo         Todo
hi def link uncComment      Comment

let b:current_syntax = "unc"

let &cpo = s:cpo_save
unlet s:cpo_save
