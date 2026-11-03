; Change the border colour on every raster line.
; The border is drawn line by line, so you see bars.
RASTER  = $C001
BORDER  = $C020

        .org $0800
start:  SEI
loop:   LDA RASTER
        LSR A
        LSR A
        LSR A
        STA BORDER
        JMP loop
