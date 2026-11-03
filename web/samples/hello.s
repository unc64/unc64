; Print a string through the ROM, then halt.
PRSTR   = $FF09

        .org $0800
start:  LDX #<msg
        LDY #>msg
        JSR PRSTR
        HLT

msg:    .text "HELLO FROM THE UNC 64!"
        .byte 13, 0
