; A test cassette (the game's autoplay loads it too, with --cassettes): prints a line and
; goes back to BASIC's READY. Built with: unc-asm hello.s --tape hello.tap --name HELLO
PRSTR   = $FF09         ; X/Y = a string ending in 0
READY   = $FF21         ; back to BASIC

        .org $3000
start:  LDX #<msg
        LDY #>msg
        JSR PRSTR
        JMP READY
msg:    .text "HELLO FROM A CASSETTE"
        .byte 13, 0
