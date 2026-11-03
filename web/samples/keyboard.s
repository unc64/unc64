; Read keys and print them twice. Click the screen and type.
CHROUT  = $FF03
GETKEY  = $FF06

        .org $0800
start:  JSR GETKEY
        JSR CHROUT
        JSR CHROUT
        JMP start
