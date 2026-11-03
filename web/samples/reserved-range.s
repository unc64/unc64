; The manual calls $C3F0-$C3FF reserved.
CHROUT  = $FF03
PRHEX   = $FF0C

        .org $0800
start:  LDX #0
loop:   LDA $C3F0,X
        JSR PRHEX
        LDA #' '
        JSR CHROUT
        INX
        CPX #16
        BNE loop
        HLT
