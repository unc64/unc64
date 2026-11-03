; Custom characters: copy UNSCII to RAM, redraw one glyph as an owl.
SCREEN  = $0400
CHARSET = $C022
CHARROM = $D000
MYSET   = $3800         ; 1K at the top of a UNC 16's memory
OWL     = $7F           ; the glyph to redraw: the note

        .org $0800
start:  LDX #0
copy:   LDA CHARROM,X       ; copy all 1,024 bytes, a quarter per pass
        STA MYSET,X
        LDA CHARROM+$100,X
        STA MYSET+$100,X
        LDA CHARROM+$200,X
        STA MYSET+$200,X
        LDA CHARROM+$300,X
        STA MYSET+$300,X
        INX
        BNE copy
        LDX #7
def:    LDA owl,X           ; eight rows, bottom row first
        STA MYSET+OWL*8,X
        DEX
        BPL def
        LDA #>MYSET
        STA CHARSET         ; the video chip now reads our set
        LDX #39
        LDA #OWL
row:    STA SCREEN+12*40,X  ; a row of owls
        DEX
        BPL row
        HLT

owl:    .byte %01000010
        .byte %01111110
        .byte %11011011
        .byte %11111111
        .byte %01100110
        .byte %01111110
        .byte %00100100
        .byte %00000000
