; Play a tune on the timer: the first two lines of Frere Jacques.
; Click the page first: browsers only play sound after a click or a key.
FRAME   = $C006
SPEAKER = $C031
TIMER_LO = $C032
TIMER_HI = $C033
LEFT    = $10           ; frames left to wait

        .org $0800
start:  LDX #0
note:   LDA tune+2,X    ; the note's length in frames; 0 ends the tune
        BEQ done
        STA LEFT
        LDA tune,X
        STA TIMER_LO
        LDA tune+1,X
        STA TIMER_HI    ; the new divisor takes over at the next half-wave
        LDA #%11        ; gate on, speaker data on: the timer plays
        STA SPEAKER
        JSR wait
        LDA #%01        ; speaker data off for a moment: the notes don't run together
        STA SPEAKER
        LDA #2
        STA LEFT
        JSR wait
        INX
        INX
        INX
        JMP note
done:   LDA #0          ; gate off, data off: silence
        STA SPEAKER
        HLT

wait:   LDA FRAME       ; wait LEFT frames
@tick:  CMP FRAME
        BEQ @tick
        DEC LEFT
        BNE wait
        RTS

C4      = 3822          ; 1,000,000 / 261.6 Hz
D4      = 3405
E4      = 3034
F4      = 2863
G4      = 2551
tune:   .word C4
        .byte 18
        .word D4
        .byte 18
        .word E4
        .byte 18
        .word C4
        .byte 18
        .word C4
        .byte 18
        .word D4
        .byte 18
        .word E4
        .byte 18
        .word C4
        .byte 18
        .word E4
        .byte 18
        .word F4
        .byte 18
        .word G4
        .byte 38
        .word E4
        .byte 18
        .word F4
        .byte 18
        .word G4
        .byte 38
        .byte 0, 0, 0
