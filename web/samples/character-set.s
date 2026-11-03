; Show all 256 screen codes in a 16 x 16 grid.
; The codes go straight into screen memory: printing them with CHROUT
; would act on 8, 12 and 13 instead of showing them.
CLS     = $FF00
CHROUT  = $FF03
PRSTR   = $FF09
PRHEX   = $FF0C
SCREEN  = $0400
ROW     = $10           ; pointer to the current grid row
CODE    = $12           ; the next code to show

        .org $0800
start:  JSR CLS
        LDX #<head
        LDY #>head
        JSR PRSTR       ; title and column digits: rows 0-2
        LDA #0
labels: PHA             ; row labels $00-$F0: rows 3-18
        JSR PRHEX
        LDA #13
        JSR CHROUT
        PLA
        CLC
        ADC #16
        BNE labels      ; $F0 + 16 wraps to 0: done
        LDA #<[SCREEN+3*40+4]
        STA ROW         ; the grid starts at column 4 of row 3
        LDA #>[SCREEN+3*40+4]
        STA ROW+1
        LDA #0
        STA CODE
grid:   LDY #0          ; Y steps two columns per code
cell:   LDA CODE
        STA (ROW),Y
        INC CODE
        INY
        INY
        CPY #32
        BNE cell
        CLC
        LDA ROW         ; next row: 40 bytes on
        ADC #40
        STA ROW
        LDA ROW+1
        ADC #0
        STA ROW+1
        LDA CODE
        BNE grid        ; CODE wraps to 0 after 256 codes
        HLT

head:   .text "UNC 64 CHARACTER SET"
        .byte 13, 13
        .text "    0 1 2 3 4 5 6 7 8 9 A B C D E F"
        .byte 13, 0
