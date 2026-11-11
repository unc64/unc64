; SNAKE for the UNC 16 (16K). Text mode, custom characters, 50 Hz.
;
; Controls: W A S D or the cursor keys steer, SPACE starts (title) and
; restarts (game over).
;
; The playfield is the text screen. A cell holding a space is free, a heart
; ($71) is food, and anything else (wall, body, head) kills. The snake's
; segments are also kept as screen addresses in a 256-entry ring buffer, so
; the tail can be rubbed out without searching.
;
; Steering takes each key press from KBD, so a quick tap is never missed and
; two taps inside one move both happen (the turn queue). At each move, if no
; turn is queued, the keyboard matrix steers too: every direction key held
; down is tried, and the first whose turn is allowed is taken. Hold LEFT while
; heading right (a reversal, ignored), tap UP, and the snake goes up one row
; and comes back left, the classic U-turn, without waiting for a key to
; repeat. Hold UP and LEFT together and it makes the same U-turn with no tap
; at all; hold UP and RIGHT and it climbs a staircase. HELD shows only the
; last key pressed, so it would lose the other key of the two.
;
; Sound is the beeper's timer (chapter 12): it plays a note by itself while
; the game runs on. A small sequencer, stepped once a frame, walks a table of
; notes for each effect, then closes the gate: silence between sounds.
;
; The ROM does the work it can: CLS clears the screen, NUMSTR and PRNUM print
; the scores six digits wide, RNDSEED seeds RND from NOISE at the moment
; SPACE is pressed, and RND places the hearts.
;
; Custom glyphs: $60-$63 the head facing up, right, down, left (= $60 + DIR),
; $64 a body segment, $65 a middle dot. The heart $71 beats: every 16 frames
; its 8 bytes in MYSET are swapped between UNSCII's heart and a smaller one.

SCREEN  = $0400
VMODE   = $C000
FRAME   = $C006
KBD_STROBE = $C011
BORDER  = $C020
CHARSET = $C022
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 speaker data
TIMER_LO = $C032        ; the timer's divisor: 1,000,000 / pitch
TIMER_HI = $C033
NOISE   = $C034         ; a byte from the cycle it's read on
KEYS    = $C060         ; the keyboard matrix, rows 0-8
CHARROM = $D000
ROMHEART = CHARROM+$91*8      ; UNSCII's heart
ROMARROWS = CHARROM+$96*8     ; and its arrows: left, up, right, down

; the ROM (chapter 13)
CLS     = $FF00
RND     = $FF12
NUMSTR  = $FF18
PRNUM   = $FF1B
RNDSEED = $FF1E
CURX    = $0200
CURY    = $0201

STEADY  = %00001000     ; VMODE bit 3: $A0 doesn't blink
PLAY    = %11           ; SPEAKER: gate on, data on: the timer reaches the speaker

; screen codes
HEADU   = $60           ; + DIR
HEADR   = $61
BODY    = $64
MIDDOT  = $65
FOOD    = $71           ; UNSCII's heart, in our set
ARROWS  = $76           ; UNSCII's arrows, in our set: left, up, right, down
HLINE   = $10
VLINE   = $11
CTL     = $12
CTR     = $13
CBL     = $14
CBR     = $15

STARTSPD = 8
MINSPD  = 3
MAXLEN  = 255
RED     = 2
DIGITS  = 6             ; scores show six digits, leading zeros and all

; timer divisors (chapter 12's table, an octave or two up)
C5      = 1911
G5      = 1276
C6      = 956
E6      = 758
G6      = 638
C7      = 478

; ---------------------------------------------------------------- fast page
; The tests read the game's state from here, by name.

        .org $00
STATE:  .res 1          ; 0 title, 1 playing, 2 game over
DIR:    .res 1          ; direction of the last move: 0 up, 1 right, 2 down, 3 left
QN:     .res 1          ; turns queued, 0-2
Q0:     .res 1          ; the queued directions
Q1:     .res 1
HEADI:  .res 1          ; ring index of the head
TAILI:  .res 1          ; ring index of the tail
LEN:    .res 1          ; segments, head included (3 at the start, at most 255)
GROW:   .res 1          ; segments still to grow
TICK:   .res 1          ; frames left until the next move
SPEED:  .res 1          ; frames per move: 8, one less every 4 hearts, down to 3
FOODS:  .res 1          ; hearts eaten this game
HX:     .res 1          ; head column, 0-39
HY:     .res 1          ; head row, 0-24
SCORE:  .res 3          ; 10 a heart, 24 bits: a snake that stops growing can play for ever
HISCORE: .res 3
OVERT:  .res 1          ; frames of the game-over pause still to run (SPACE ignored)
FX:     .res 1          ; where the last heart was placed
FY:     .res 1
NEW:    .res 2          ; screen address of the cell the head moves into
PTR:    .res 2          ; scratch pointer
PTR2:   .res 2          ; prlist's record pointer
TMP:    .res 1
BX:     .res 1          ; box: column, row, width, height, fill the inside?
BY:     .res 1
BW:     .res 1
BH:     .res 1
BFILL:  .res 1
ROW:    .res 1
KIND:   .res 1
KEYI:   .res 1          ; which direction key the matrix scan is on
SNDI:   .res 1          ; the next note: an offset into sounds
SNDT:   .res 1          ; frames left of the note playing; 0: silence
        .assert * <= $D0, "the fast page runs into the ROM's ($D0-$FE)"

; ---------------------------------------------------------------- start

        .org $0800
start:  STA HISCORE             ; A is 0 at start
        STA HISCORE+1
        STA HISCORE+2
        LDA #STEADY             ; text mode, and blinking is up to us
        STA VMODE
        JSR initchars
        JSR title

; fifty frames a second or it isn't a game.
;
; The main loop: wait for the frame counter to tick, then do one frame's work.
main:   LDA FRAME
@wait:  CMP FRAME
        BEQ @wait
work:   JSR pulse
        JSR sndframe
        LDA STATE
        BEQ @title
        CMP #1
        BEQ @play
        JSR overframe
        JMP main
@title: JSR titleframe
        JMP main
@play:  JSR playframe
        JMP main

; ---------------------------------------------------------------- characters

; Copy UNSCII to MYSET, redefine our glyphs, point the video chip at it.
initchars:
        LDX #0
@copy:
        .rept 4, page
        LDA CHARROM + page * $100, X
        STA MYSET + page * $100, X
        .endr
        INX
        BNE @copy
        LDX #7          ; the heart and the four arrows are in the ROM's second half
@heart: LDA ROMHEART,X  ; (256-glyph mode's $91 and $96-$99): into our set, where we draw
        STA MYSET+FOOD*8,X
        DEX
        BPL @heart
        LDX #31
@arrow: LDA ROMARROWS,X
        STA MYSET+ARROWS*8,X
        DEX
        BPL @arrow
        LDX #GLYPHN-1
@glyph: LDA glyphs,X
        STA MYSET+HEADU*8,X
        DEX
        BPL @glyph
        LDA #>MYSET
        STA CHARSET
        RTS

; Every 16 frames, swap the heart glyph between big and small: every heart on
; the screen beats at once, for 8 byte writes.
pulse:  LDA FRAME
        AND #15
        BNE @done
        LDX #7
        LDA FRAME
        AND #16
        BEQ @small
@big:   LDA ROMHEART,X
        STA MYSET+FOOD*8,X
        DEX
        BPL @big
        RTS
@small: LDA smallheart,X
        STA MYSET+FOOD*8,X
        DEX
        BPL @small
@done:  RTS

; ---------------------------------------------------------------- title

title:  LDA #0
        STA STATE
        JSR quiet
        JSR CLS
        LDY #BOXTITLE
        JSR box
        LDX #<titletxt
        LDY #>titletxt
        JSR prlist
        LDA #HISCORE
        LDX #22
        LDY #20
        JSR number
        LDA KBD_STROBE          ; forget any key pressed before now
        RTS

titleframe:
        LDX #<SCREEN+17*40+18   ; PRESS SPACE
        LDY #>SCREEN+17*40+18
        LDA #11
        JSR blink
        JSR getkey
        CMP #' '
        BEQ newgame
        RTS

; ---------------------------------------------------------------- a new game

; SPACE was pressed this frame. NOISE depends on the cycle it's read on, and
; the player chose the frame, so the seed (and every heart) is a new one.
newgame:
        LDX NOISE
        LDY NOISE
        JSR RNDSEED
        LDY #SNDSTART
        JSR sound
        LDA #0
        STA QN
        STA GROW
        STA FOODS
        STA BORDER
        STA TAILI
        STA SCORE
        STA SCORE+1
        STA SCORE+2
        LDA #$FF
        STA HEADI               ; the first pushseg makes it 0
        LDA #STARTSPD
        STA SPEED
        STA TICK
        LDA #1
        STA STATE
        STA DIR
        LDA #3
        STA LEN
        JSR CLS
        LDY #BOXPLAY
        JSR box
        LDX #<hdrtxt
        LDY #>hdrtxt
        JSR prlist
        JSR showscore
        LDA #HISCORE
        LDX #34
        LDY #0
        JSR number
        LDA #12                 ; three segments on row 12, columns 18-20
        STA HY
        LDA #18
        STA HX
@seg:   JSR headaddr
        JSR pushseg
        LDY #0
        LDA #BODY
        STA (NEW),Y
        LDA HX
        CMP #20
        BEQ @head
        INC HX
        BNE @seg
@head:  LDA #HEADR
        STA (NEW),Y
        JSR placefood
        LDA KBD_STROBE
        RTS

; ---------------------------------------------------------------- playing

playframe:
        JSR getkey
        JSR turn
        DEC TICK
        BNE @done
        LDA SPEED
        STA TICK
        JSR heldturn            ; no turn queued: a direction key held down
        JMP step                ; steers, once that turn is allowed
@done:  RTS

; heldturn: if no turn is queued, try each direction key that's down in the
; keyboard matrix (chapter 11) and queue the first turn that's allowed. The
; matrix shows every key at once, so with UP and LEFT both held while heading
; right, UP turns now and LEFT at the next move.
heldturn:
        LDX #7
@key:   LDA QN
        BNE @done               ; a turn is queued: a tap, or a held key just now
        LDY keyrow,X
        LDA KEYS,Y
        AND keybit,X
        BEQ @next               ; this key is up
        STX KEYI
        TXA
        AND #3                  ; its direction
        JSR turnto
        LDX KEYI
@next:  DEX
        BPL @key
@done:  RTS

; turn: queue the turn for key A, if it's a direction key. Up to two turns
; wait, so two quick taps inside one move both happen. A turn that repeats or
; reverses the one before it is ignored.
turn:   LDX #7
@find:  CMP dirkeys,X
        BEQ @found
        DEX
        BPL @find
        RTS
@found: TXA
        AND #3
        ; fall through

; turnto: queue a turn to direction A, by the same rules.
turnto: STA TMP                 ; the new direction
        LDA DIR
        LDX QN
        BEQ @check
        LDA Q0-1,X              ; the last queued turn
@check: CMP TMP
        BEQ @done               ; already going that way
        EOR #2
        CMP TMP
        BEQ @done               ; straight back into the neck
        CPX #2
        BCS @done               ; queue full
        LDA TMP
        STA Q0,X
        INC QN
@done:  RTS

; One move.
step:   LDA QN
        BEQ @move
        LDA Q0
        STA DIR
        LDA Q1
        STA Q0
        DEC QN
@move:  LDX DIR
        LDA HX
        CLC
        ADC dxt,X
        STA HX
        LDA HY
        CLC
        ADC dyt,X
        STA HY
        JSR headaddr
        LDY #0
        LDA (NEW),Y
        CMP #' '
        BEQ @free
        CMP #FOOD
        BEQ @food
        LDA GROW                ; solid: only the tail's cell is safe, as it moves away
        BNE @die
        LDX TAILI
        LDA NEW
        CMP BODYLO,X
        BNE @die
        LDA NEW+1
        CMP BODYHI,X
        BEQ @free
@die:   JMP gameover
@food:  INC GROW
        JSR eat                 ; the heart still covers the head's cell, so
@free:  LDA GROW                ; the new one can't land there
        BEQ @tail
        DEC GROW
        LDA LEN
        CMP #MAXLEN
        BCS @tail               ; full length: stop growing
        INC LEN
        BNE @neck
@tail:  LDX TAILI               ; rub out the tail
        LDA BODYLO,X
        STA PTR
        LDA BODYHI,X
        STA PTR+1
        LDA #' '
        LDY #0
        STA (PTR),Y
        INC TAILI
@neck:  LDX HEADI               ; the old head becomes body
        LDA BODYLO,X
        STA PTR
        LDA BODYHI,X
        STA PTR+1
        LDA #BODY
        LDY #0
        STA (PTR),Y
        JSR pushseg
        LDA DIR
        CLC
        ADC #HEADU
        STA (NEW),Y
        RTS

; NEW = the screen address of (HX, HY).
headaddr:
        LDX HY
        LDA HX
        CLC
        ADC ROWLO,X
        STA NEW
        LDA ROWHI,X
        ADC #0
        STA NEW+1
        RTS

; the ring buffer was the third try. the first two searched for the tail. never search for the tail.
;
; Push NEW onto the ring buffer as the new head.
pushseg:
        INC HEADI
        LDX HEADI
        LDA NEW
        STA BODYLO,X
        LDA NEW+1
        STA BODYHI,X
        RTS

; eat: 10 points, a gulp, and a new heart. Every fourth heart the snake gets
; faster, down to MINSPD, and the gulp becomes a trill.
eat:    LDA SCORE
        CLC
        ADC #10
        STA SCORE
        BCC @shown
        INC SCORE+1
        BNE @shown
        INC SCORE+2
@shown: JSR showscore
        LDY #SNDEAT
        INC FOODS
        LDA FOODS
        AND #3
        BNE @sound
        LDA SPEED
        CMP #MINSPD+1
        BCC @sound
        DEC SPEED
        LDY #SNDFAST
@sound: JSR sound
        JMP placefood

; a heart, not a dot. nobody chases a dot.
;
; Put a heart on a random free cell of the playfield (rows 2-23, columns 1-38).
placefood:
        JSR RND
        AND #31
        CMP #22
        BCS placefood
        ADC #2
        STA FY
        TAX
@col:   JSR RND
        AND #63
        CMP #38
        BCS @col
        ADC #1
        STA FX
        JSR at
        LDY #0
        LDA (PTR),Y
        CMP #' '
        BNE placefood
        LDA #FOOD
        STA (PTR),Y
        RTS

showscore:
        LDA #SCORE
        LDX #7
        LDY #0
        ; fall through

; number: print the 24-bit number at fast-page address A at column X, row Y,
; DIGITS wide with leading zeros, so a new game's 000000 covers the old score.
; NUMSTR makes the digits and PRNUM prints them: about 1,800 cycles, once per
; heart, so the score can stay a binary number (manual, "Keeping score fast").
number: STX CURX
        STY CURY
        TAX
        LDY 1,X                 ; the middle byte
        LDA 2,X                 ; the high byte...
        PHA
        LDA 0,X                 ; ...waits while the low byte goes in X
        TAX
        PLA
        JSR NUMSTR
        LDA #DIGITS
        JMP PRNUM

; ---------------------------------------------------------------- game over

; i always die at 212. i've stopped minding.
;
gameover:
        LDY #SNDDIE             ; the slide, the only sound until SPACE
        JSR sound
        LDA #2
        STA STATE
        LDA #50                 ; a second's pause before SPACE counts
        STA OVERT
        LDA #RED
        STA BORDER
        LDY #BOXOVER
        JSR box
        LDX #<overtxt
        LDY #>overtxt
        JSR prlist
        LDA #SCORE
        LDX #20
        LDY #11
        JSR number
        LDA HISCORE             ; HISCORE - SCORE: carry set if no new high
        CMP SCORE
        LDA HISCORE+1
        SBC SCORE+1
        LDA HISCORE+2
        SBC SCORE+2
        BCS @done
        LDX #2
@copy:  LDA SCORE,X
        STA HISCORE,X
        DEX
        BPL @copy
        LDX #<hightxt
        LDY #>hightxt
        JSR prlist
        LDA #HISCORE
        LDX #34
        LDY #0
        JMP number
@done:  RTS

overframe:
        LDA OVERT
        BEQ @ready
        DEC OVERT
        BNE @flush
        LDA #0
        STA BORDER
@flush: LDA KBD_STROBE          ; keys during the pause are thrown away
        RTS
@ready: LDX #<SCREEN+14*40+13   ; SPACE TO PLAY
        LDY #>SCREEN+14*40+13
        LDA #13
        JSR blink
        JSR getkey
        CMP #' '
        BNE @done
        JMP newgame
@done:  RTS

; getkey: A = the key waiting (taken), or 0 if none.
getkey: LDA KBD_STROBE          ; bit 7: a new key, and this read took it
        BMI @key
        LDA #0
@key:   AND #$7F
        RTS

; ---------------------------------------------------------------- sound

; One voice: the beeper's timer (chapter 12) plays each note by itself, so a
; sound costs the game nothing while it plays. An effect is a list of notes at
; sounds+offset: a length in frames, then the timer's divisor; a length of 0
; ends it. `sound` starts an effect, cutting off whatever was playing, and
; sndframe, once a frame, starts the next note when a note's time is up. When
; the effect ends the gate closes, and the speaker is silent until the next.

; sound: start the effect at sounds+Y. Its first note plays from this frame.
sound:  STY SNDI
        ; fall through

; note: start the note at sounds+SNDI, or end the effect at its 0.
note:   LDY SNDI
        LDA sounds,Y            ; the length
        BEQ quiet
        STA SNDT
        LDA sounds+1,Y
        STA TIMER_LO
        LDA sounds+2,Y
        STA TIMER_HI            ; the timer takes it at its next half-wave: no click
        LDA #PLAY
        STA SPEAKER
        INY
        INY
        INY
        STY SNDI
        RTS

; quiet: gate and data off, nothing playing.
quiet:  LDA #0
        STA SPEAKER
        STA SNDT
        RTS

; sndframe: once a frame, a few cycles. Counts the note down; at 0, the next.
sndframe:
        LDA SNDT
        BEQ @done               ; silence
        DEC SNDT
        BEQ note
@done:  RTS

; ---------------------------------------------------------------- drawing

; at: PTR = the screen address of column A, row X.
at:     CLC
        ADC ROWLO,X
        STA PTR
        LDA ROWHI,X
        ADC #0
        STA PTR+1
        RTS

; box: a box-drawing frame from the record at boxes+Y: column, row, width,
; height (both at least 3), and whether to clear the inside.
box:    LDX #0
@args:  LDA boxes,Y
        STA BX,X
        INY
        INX
        CPX #5
        BNE @args
        LDA BY
        STA ROW
        LDX #EDGETOP
        JSR boxrow
        LDA BH
        SEC
        SBC #2
        STA TMP
@mid:   INC ROW
        LDX #EDGEMID
        JSR boxrow
        DEC TMP
        BNE @mid
        INC ROW
        LDX #EDGEBOT
        ; fall through to draw the bottom row

; boxrow: one row of the box at ROW, with the left, middle and right
; characters at edges+X.
boxrow: STX KIND
        LDX ROW
        LDA BX
        JSR at
        LDX KIND
        LDY #0
        LDA edges,X
        STA (PTR),Y
        LDY BW
        DEY
        LDA edges+2,X
        STA (PTR),Y
        CPX #EDGEMID
        BNE @fill
        LDA BFILL               ; a middle row: fill only when asked
        BEQ @done
@fill:  LDA edges+1,X
        DEY
        BEQ @done
@loop:  STA (PTR),Y
        DEY
        BNE @loop
@done:  RTS

; prlist: print records from X (low) / Y (high) until a column of $FF.
; A record is: column, row, text, 0. In the text, '#' prints a body segment
; and '@' a head facing right.
prlist: STX PTR2
        STY PTR2+1
@rec:   LDY #0
        LDA (PTR2),Y
        CMP #$FF
        BEQ @done
        PHA
        INY
        LDA (PTR2),Y
        TAX
        PLA
        JSR at                  ; PTR = the first cell
        LDA #2                  ; PTR2 = the text
        JSR @skip
        LDY #0
@char:  LDA (PTR2),Y
        BEQ @end
        CMP #'#'
        BNE @nobody
        LDA #BODY
@nobody: CMP #'@'
        BNE @put
        LDA #HEADR
@put:   STA (PTR),Y
        INY
        BNE @char
@end:   INY                     ; past the 0: on to the next record
        TYA
        JSR @skip
        JMP @rec
@skip:  CLC                     ; PTR2 += A
        ADC PTR2
        STA PTR2
        BCC @done
        INC PTR2+1
@done:  RTS

; blink: every 16 frames, flip A cells from X (low) / Y (high) between normal
; and reverse video. VMODE's STEADY bit stops a reversed space ($A0) blinking
; by itself, so spaces can flip like everything else.
blink:  STX PTR
        STY PTR+1
        TAY
        LDA FRAME
        AND #15
        BNE @done
        DEY
@loop:  LDA (PTR),Y
        EOR #$80
        STA (PTR),Y
        DEY
        BPL @loop
@done:  RTS

; ---------------------------------------------------------------- data

dirkeys: .byte 'W', 'D', 'S', 'A'      ; index & 3 = direction
        .byte 30, 29, 31, 28           ; the cursor keys: up, right, down, left

; The same eight keys in the keyboard matrix (chapter 11): the key with code
; $20-$5F is bit (code AND 7) of row (code - $20) / 8, and the cursor keys,
; 28-31, are bits 0-3 of row 8.
keyrow: .irp c, 'W', 'D', 'S', 'A'
        .byte [c - $20] / 8
        .endr
        .fill 4, 8                      ; the cursor keys
keybit: .irp c, 'W', 'D', 'S', 'A'
        .byte 1 << [c & 7]
        .endr
        .irp c, 30, 29, 31, 28
        .byte 1 << [c - 28]
        .endr
dxt:    .byte 0, 1, 0, $FF
dyt:    .byte $FF, 0, 1, 0

ROWLO:
        .rept 25, n
        .byte <SCREEN + n * 40
        .endr
ROWHI:
        .rept 25, n
        .byte >SCREEN + n * 40
        .endr

; Sound effects for `sound`: notes of a length in frames and a timer divisor
; (1,000,000 / pitch), and a 0 to end. Offsets fit in Y.
sounds:
SNDEAT = * - sounds             ; a heart: a gulp, low then an octave up
        .byte 2
        .word G5
        .byte 3
        .word G6
        .byte 0
SNDFAST = * - sounds            ; every fourth heart, as the snake speeds up: a trill
        .irp n, C7, G6, C7, G6
        .byte 2
        .word n
        .endr
        .byte 4
        .word C7
        .byte 0
SNDSTART = * - sounds           ; SPACE: a rising arpeggio, then the high G held
        .irp n, C5, G5, C6, E6
        .byte 3
        .word n
        .endr
        .byte 9
        .word G6
        .byte 0
SNDDIE = * - sounds             ; a crash: a slide down, a note a frame, from 667 Hz
        .rept 32, n             ; to about 170, wobbling: a sine, a turn every 6.4 frames
        .byte 1
        .word 1500 + n * n * 5 + sin(n * 40, 100 + n * 10)
        .endr
        .byte 10                ; and a low thud to finish
        .word 9000
        .byte 0
        .assert * - sounds <= 256, "sound offsets must fit in Y"

; box records: column, row, width, height, fill
boxes:
BOXTITLE = * - boxes
        .byte 0, 0, 40, 25, 0
BOXPLAY = * - boxes
        .byte 0, 1, 40, 24, 0           ; the playfield: rows 1-24
BOXOVER = * - boxes
        .byte 10, 8, 20, 8, 1

; box rows: left, middle, right
edges:
EDGETOP = * - edges
        .byte CTL, HLINE, CTR
EDGEMID = * - edges
        .byte VLINE, ' ', VLINE
EDGEBOT = * - edges
        .byte CBL, HLINE, CBR

; Glyphs $60-$65, copied over the eighth blocks.
glyphs:
        ; $60 head facing up
        .byte %00010000, %01111100, %10111010, %11111110, %11111110, %11111110, %01111100, %00000000
        ; $61 head facing right
        .byte %01111000, %11110100, %11111100, %11111110, %11111100, %11110100, %01111000, %00000000
        ; $62 head facing down
        .byte %01111100, %11111110, %11111110, %11111110, %10111010, %01111100, %00010000, %00000000
        ; $63 head facing left
        .byte %00111100, %01011110, %01111110, %11111110, %01111110, %01011110, %00111100, %00000000
        ; $64 body segment
        .byte %01111100, %11010110, %10111010, %11010110, %10111010, %11010110, %01111100, %00000000
        ; $65 middle dot
        .byte %00000000, %00000000, %00000000, %00011000, %00011000, %00000000, %00000000, %00000000
GLYPHN  = * - glyphs

smallheart:
        .byte %00000000, %00100100, %01111110, %01111110, %00111100, %00011000, %00000000, %00000000

titletxt:
        .byte 8, 3
        .text " ### #  #  ##  #  # ####"
        .byte 0
        .byte 8, 4
        .text "#    ## # #  # # #  #   "
        .byte 0
        .byte 8, 5
        .text " ##  # ## #### ##   ### "
        .byte 0
        .byte 8, 6
        .text "   # #  # #  # # #  #   "
        .byte 0
        .byte 8, 7
        .text "###  #  # #  # #  # ####"
        .byte 0
        .byte 13, 10
        .text "########@   "
        .byte FOOD, 0
        .byte 8, 13
        .text "W A S D OR "
        .byte ARROWS+1, ARROWS, ARROWS+3, ARROWS+2
        .text " TO STEER"
        .byte 0
        .byte 5, 15
        .text "EAT THE "
        .byte FOOD
        .text ", DON'T BITE ANYTHING"
        .byte 0
        .byte 10, 17
        .text "SNAKE "
        .byte MIDDOT
        .text " PRESS SPACE"
        .byte 0
        .byte 13, 20
        .text "HI SCORE"
        .byte 0
        .byte $FF

hdrtxt: .byte 1, 0
        .text "SCORE"
        .byte 0
        .byte 17, 0
        .text "SNAKE"
        .byte 0
        .byte 31, 0
        .text "HI"
        .byte 0
        .byte $FF

overtxt:
        .byte 15, 9
        .text "GAME OVER"
        .byte 0
        .byte 14, 11
        .text "SCORE"
        .byte 0
        .byte 13, 14
        .text "SPACE TO PLAY"
        .byte 0
        .byte $FF

hightxt:
        .byte 13, 12
        .text "NEW HIGH SCORE"
        .byte 0
        .byte $FF

; ---------------------------------------------------------------- RAM
; Reserved, not loaded: the program fills these in.

BODYLO: .res 256                ; ring buffer: low bytes of segment screen addresses
BODYHI: .res 256                ; and the high bytes
        .align 256              ; between reserved space: reserved, not stored
MYSET:  .res $400               ; RAM copy of UNSCII with our glyphs; CHARSET = >MYSET
        .assert [MYSET & $FF] == 0, "the character set must start on a page"
        .assert * <= $4000, "past the end of a UNC 16's memory"
