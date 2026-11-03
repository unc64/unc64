; Print $00-$FF in hex, then halt.
CHROUT  = $FF03
PRHEX   = $FF0C

        .org $0800
start:  LDX #0
loop:   TXA
        JSR PRHEX
        LDA #' '
        JSR CHROUT
        INX
        BNE loop
        HLT
