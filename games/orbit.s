; ORBIT: a rock-splitting shooter around a small planet, for the UNC 16 (16K).
; Bitmap in mixed mode: the arena is the top 168 lines, the text rows 21-24 are the HUD.
;
; Controls: A / D or cursor left / right rotate, W or cursor up thrusts, SPACE fires,
; starts and restarts. The controls are read as held keys from the keyboard matrix
; (chapter 11), so the ship can turn, thrust and fire all at once. A tap turns one 1/32
; step and a hold turns smoothly after HOLDDLY frames; a tap of thrust is a short burst
; and a hold burns until the key comes up; SPACE fires whenever the gun is ready, so
; holding it keeps firing. A press too short for the matrix to catch between two frames
; still counts, from KBD. Start and restart are presses, taken from KBD.
;
; Sound: one voice, the timer beeper (chapter 12), stepped once a frame by a small
; sequencer (sound). Each effect has a priority: the ship blowing up beats a new wave,
; which beats a rock breaking (big over small), which beats a shot. The engine rumbles
; underneath while the ship thrusts, whenever nothing else plays; otherwise the gate is
; closed and the speaker is still.
;
; The arena is 256 x 168 pixels, centred on the screen (screen x = arena x + 32), so
; every coordinate is one byte and x wraps by itself. The planet sits at (128, 84).
; Positions and velocities are 8.8 fixed point; gravity comes from a 22 x 22 table.
; Everything is drawn a pixel at a time, with XOR so that drawing again rubs it out: the
; ship as lines, the rocks as pictures compiled to code, and the bullets, 2 x 2 dots,
; through the shifter (chapter 8).
;
; Memory: the fast page and the hidden bitmap lines are laid out with .res just below, so
; the assembler keeps the addresses; nothing there is loaded.
; $0000-$0087  object table, 17 objects x 8 fields: 0 ship, 1-4 bullets, 5-16 rocks
; $0088-$00CE  game variables (VAREND checks they stay below BASIC's $D0, which the ROM's
;              NUMSTR, PRNUM and RND use)
; $0400-$07E7  text screen; rows 21-24 ($0748-$07E7) are the HUD; the rows above are
;              under the arena, never shown in mixed mode: FASTY lives there
; $0800-$1FFF  code and tables (6K)
; $2000-$3A3F  the bitmap's visible 168 lines; the gravity table is loaded at its start,
;              past the code, and copied up to GTAB before the bitmap is cleared
; $3A40-$3FFF  the bitmap's hidden lines and the RAM past them (under the text in mixed
;              mode, so free): the object fields the fast page has no room for, the
;              sound's state, the plotting tables, built at start, and the gravity table
;
; The tests find STATE, SHIPDIR, LIVES, WAVE, SCORE (six digits, 0-9 each, the highest
; first), INVUL, THRUST, MAXLINE (worst raster line at the end of a frame's work), OVERRUN
; (frames whose work spilled into the next), NOCHK, SHOTS, TICK, KIND and SNDPRI by name in
; the symbol table.

; ---------------------------------------------------------------- hardware and ROM
VMODE   = $C000
RASTER  = $C001
RASTHI  = $C002
FRAME   = $C006
STROBE  = $C011         ; KBD_STROBE: the last key pressed, taken
BORDER  = $C020
BACKGND = $C021
BITMAP  = $C023
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 the speaker's data (chapter 12)
TIMER_LO = $C032        ; the timer's divisor: the low byte first, then
TIMER_HI = $C033        ; the high byte, which is the write that counts
NOISE   = $C034         ; a byte from the cycle it's read on
SHIFT_IN = $C050        ; the shifter (chapter 8): push a byte into the window...
SHIFT_AMT = $C051       ; ...pixels to move it right (x AND 7)...
SHIFT_OUT = $C052       ; ...and the window, moved: the next byte for the bitmap
KEYS    = $C060         ; the keyboard matrix, rows 0-8 (chapter 11)
RND     = $FF12         ; A = a random byte
NUMSTR  = $FF18         ; X/Y/A (low to high) -> eight digits in NUMBUF
PRNUM   = $FF1B         ; print NUMSTR's digits at the cursor, A wide
RNDSEED = $FF1E         ; X/Y -> RND's seed
CURX    = $0200
CURY    = $0201
BMP     = $2000
; The text rows under the arena ($0400-$0747), never shown in mixed mode: free RAM.
FASTY   = $0400         ; the ship's lines step by +-40 (line)
EXR     = $0401         ; the explosion
EXX     = $0402
EXY     = $0403
EXDR    = $0404
MERGE   = $0405         ; the ship's lines XOR twice: at PTR, and at PTR2 offset by DX (line)
DLO     = $0406         ; PTR2 - PTR at a line's start: 40 x the rows between the places
DHI     = $0407
DX      = $0408         ; the old place's x - the new's, -8 to 8
MOVRUN  = $0409         ; tables loaded past the code (MOVLD) and copied here at start
MOVED   = MOVRUN - MOVLD ; so a table's address here is its label there + MOVED
ROW21   = $0400+21*40
ROW22   = $0400+22*40
ROW23   = $0400+23*40
ROW24   = $0400+24*40
SCOREPOS = ROW22+7      ; the score's six digits on row 22
WAVECOL = 23            ; the wave's two digits, row 22
LIVESCOL = 37           ; the ships left, one digit, row 22

; ---------------------------------------------------------------- tuning
NOBJ    = 17
BUL0    = 1
NBUL    = 4
ROCK0   = 5
AREA_H  = 168
HOLDDLY = 8             ; a turn key held this many frames starts turning smoothly
THRTAP  = 12            ; a fresh thrust: frames of burn, however short the press
COOLT   = 6             ; frames between shots
BLIFE   = 44            ; bullet lifetime, frames
INVULT  = 120           ; frames of safety after (re)spawning
WAVEDLY = 50            ; frames between clearing a wave and the next
ARENA0  = 51            ; the raster line the arena's first line is drawn on (chapter 9)
VBLANK  = 251           ; the beam leaves the picture: FRAME goes up, a frame's work starts
RBIG    = 32            ; lines a big rock's picture takes to draw or rub out (1,800 cycles,
RSMALL  = 18            ; and the looking), and a small one's (950): measured, and a line
RMARGIN = 12            ; lines to spare before the next frame's work at 219
LOGL0   = 80            ; LOGL before it's measured: the work takes the whole arena
NSTARS  = 28

; ---------------------------------------------------------------- the fast page
        .org $0000
OXL:    .res NOBJ       ; the object table, indexed by X
OXH:    .res NOBJ       ; position, 8.8 (x 0-255, y 0-167)
OYL:    .res NOBJ
OYH:    .res NOBJ
OVXL:   .res NOBJ       ; velocity, 8.8 signed
OVXH:   .res NOBJ
OVYL:   .res NOBJ
OVYH:   .res NOBJ
PTR:    .res 2          ; bitmap byte pointer
TP:     .res 2          ; table pointer
LX:     .res 1          ; line: current point
LY:     .res 1          ;   (and the rocks' current line)
ADX:    .res 1
ADY:    .res 1
ERR:    .res 1
CNT:    .res 1
DLX:    .res 1          ; line: signed deltas
DLY:    .res 1
CX:     .res 1          ; shape centre
CY:     .res 1
T0:     .res 1
T1:     .res 1
T2:     .res 1
T3:     .res 1
T4:     .res 1
T5:     .res 1
STATE:  .res 1          ; 0 title, 1 play, 2 dying, 3 game over
KEYIN:  .res 1          ; key pressed this frame (KBD), 0 if none
LASTFR: .res 1
TICK:   .res 1
SHIPDIR: .res 1         ; 0-31; 0 right, 8 down, 24 up
PAD:    .res 1          ; the controls down this frame: PLEFT, PRIGHT, PTHRUST, PFIRE
OLDPAD: .res 1          ; and last frame
HOLDN:  .res 1          ; frames the steering keys have been as they are
THRUST: .res 1          ; frames of thrust left
BURN:   .res 1          ; not 0: the engine burns this frame (the flame and the rumble)
COOL:   .res 1          ; frames until the gun is ready
LIVES:  .res 1
WAVE:   .res 1
SCORE:  .res 6          ; six decimal digits, 0-9 each, the highest first
INVUL:  .res 1
TIMER:  .res 1
WAVET:  .res 1
SHOTS:  .res 1
GIX:    .res 1          ; physics scratch
GIY:    .res 1
SGX:    .res 1
SGY:    .res 1
GX:     .res 1
GY:     .res 1
AGX:    .res 1
AGY:    .res 1
MAXLINE: .res 2
OVERRUN: .res 1
NOCHK:  .res 1          ; this frame did heavy set-up: don't time it
RRDR:   .res 1          ; next rock to redraw
SDRAWN: .res 1          ; the ship as drawn
SDX:    .res 1
SDY:    .res 1
SDDIR:  .res 1
SDFL:   .res 1
FLAME:  .res 1
SFLAG:  .res 1
SDIRT:  .res 1
PTR2:   .res 2          ; a second bitmap pointer, for the ship's old place (drawship)
        .res 2          ; (free)
OBJ:    .res 1
HITX:   .res 1
PIDX:   .res 1
VAREND:
        .assert VAREND <= $D0, "the variables run into $D0-$FE, which NUMSTR, PRNUM and RND use"

; ---------------------------------------------------------------- the hidden bitmap lines
        .org BMP + AREA_H*40
KIND:   .res NOBJ       ; ship 1 alive; bullet = frames to live; rock 2 big, 1 small, 0 none
DK:     .res NOBJ       ; what is drawn there now (same codes; bullets 1)
DXP:    .res NOBJ       ; where it was drawn
DYP:    .res NOBJ
BDIR:   .res NOBJ       ; a bullet's direction
PD:     .res NOBJ       ; distance to the planet
SNDPRI: .res 1          ; the effect playing, which is also its priority: 0 none
SNDP:   .res 1          ; its next step, as an offset into SFX
SNDN:   .res 1          ; frames left of the step playing
SNDHI:  .res 1          ; the step's divisor, high byte (bit 7: noise)
SNDLO:  .res 1          ;   and low byte (for noise: how far the high byte jitters)
FIELDEND:
        .res 8          ; arena x -> byte column and bit. Both run 8 entries past each
XCOL:   .res 256 + 8    ; end, wrapped round, so a sprite centred at X can use XCOL+dx,X
        .res 8          ; for any dx from -8 to 8
XMASK:  .res 256 + 8
YLO:    .res AREA_H     ; y -> line address
YHI:    .res AREA_H
LOGL:   .res 1          ; how long the last frame's work took, in 2-line steps (wline)
GTAB:   .res 22*22      ; gravity (GTABLD, copied here at start)
HIDEND:
        .assert HIDEND <= $4000, "the hidden tables run past the end of RAM"

; ---------------------------------------------------------------- start
; The loader starts us with SP = $FF, interrupts off and A = X = Y = 0 (chapter 6).
        .org $0800
start:  LDX #VAREND
@fp:    DEX
        STA $00,X
        BNE @fp
        LDX #FIELDEND-KIND
@hid:   DEX
        STA KIND,X
        BNE @hid
        STA FASTY       ; (A is 0) the text rows under the arena hold whatever the screen
        STA MERGE       ; did: these two are read before they're written
        LDX #22*11      ; the gravity table, up from the bitmap before it's cleared
@gt:    LDA GTABLD-1,X
        STA GTAB-1,X
        LDA GTABLD+22*11-1,X
        STA GTAB+22*11-1,X
        DEX
        BNE @gt
        LDX #MOVN       ; and the moved tables, into the text rows under the arena
@mv:    LDA MOVLD-1,X
        STA MOVRUN-1,X
        LDA MOVLD+MOVN-1,X
        STA MOVRUN+MOVN-1,X
        LDA MOVLD+2*MOVN-1,X
        STA MOVRUN+2*MOVN-1,X
        DEX
        BNE @mv
        JSR gentab
        LDA #LOGL0
        STA LOGL
        LDA #0
        STA BORDER
        STA BACKGND
        LDA #>BMP
        STA BITMAP
        LDA #6          ; bitmap + mixed
        STA VMODE
        JSR gotitle
        LDA FRAME
        STA LASTFR

; ---------------------------------------------------------------- the frame loop
; The work starts as the beam leaves the arena, at line ARENA0 + AREA_H (219): from
; there it doesn't draw the arena again for 144 lines, 9,216 cycles (chapter 8,
; "Tearing"), more than the 112 from VBLANK. Once a frame: FRAME must have gone up since
; the last work began.
main:   LDA FRAME
        CMP LASTFR
        BEQ main
@beam:  LDA RASTER      ; line 219 (lines 256-311 read 0-55: no mistaking it)
        CMP #ARENA0+AREA_H
        BNE @beam
        LDA FRAME
work:   STA LASTFR      ; a new frame: its work runs from here to endwork
        INC TICK
        JSR sound       ; the sound first, so its timer writes come at the same moment
                        ; every frame: an effect started last frame plays from now
        LDA STROBE      ; take the key pressed since last frame: bit 7 says there was one
        BMI @key
        LDA #0
@key:   AND #$7F
        STA KEYIN
        LDA STATE
        ASL A
        TAX
        LDA STJMP+1,X   ; to the state's routine: its address on the stack, high byte
        PHA             ; first, and RTS goes there (this RTS adds nothing to it)
        LDA STJMP,X
        PHA
        RTS

; endwork: every state ends here. Count overruns, keep the worst raster line.
endwork: LDA NOCHK
        BNE @setup
        LDA FRAME       ; FRAME goes up once during the work, at VBLANK; twice is late
        SEC
        SBC LASTFR
        CMP #2
        BCC @time
        INC OVERRUN     ; too late for this frame: skip to the next one rather than
        LDA FRAME       ; start mid-frame
        STA LASTFR
        JMP main
@time:  LDA RASTER      ; RASTER first: reading it latches bit 8 for RASTHI
        LDX RASTHI
        CPX MAXLINE+1
        BCC @done
        BNE @worst
        CMP MAXLINE
        BCC @done
@worst: STA MAXLINE
        STX MAXLINE+1
@done:  JMP main
@setup: LDA #0
        STA NOCHK
        LDA FRAME       ; set-up frames may be long: start afresh
        STA LASTFR
        JMP main

STJMP:  .word st_title, st_play, st_dying, st_over

; ---------------------------------------------------------------- states
st_title:
        LDA KEYIN
        CMP #' '
        BNE @demo
        JSR newgame
        JMP endwork
@demo:  JSR rocks
        JSR physall
        JSR rockplanet
        JSR countrocks
        BNE @late
        JSR rocksclear
        BNE @late
        LDA #3
        JSR spawnrocks
@late:  JSR rockslate
        JMP endwork

; Every state draws first, from line 219, while the beam is out of the arena (144 lines),
; and only then works out the next frame (input, physics, collisions), while the beam is
; drawing the arena: the picture is a frame behind the game, and gets the whole window.
; What's left of the frame goes on the rocks that waited, behind the beam (rockslate).
st_play:
        JSR drawbul     ; the shots before the ship: they're quick, and drawn while the
        JSR drawship    ; beam is still in the border (chapter 9); XOR, so the order
        JSR rocks       ; doesn't change the picture
        JSR wline       ; timed, for the first rocks' deadline next frame (LOGL)
        PHA
        JSR pinput
        JSR timers
        JSR physall
        JSR clampv
        JSR collide
        JSR wavecheck
        JSR wline
        STA T0
        PLA
        EOR #$FF        ; the end - the start
        SEC
        ADC T0
        CMP LOGL        ; LOGL holds the longest, and lets go a step every fourth frame
        BCS @long
        LDA TICK
        AND #0
        BNE @kept
        LDA LOGL
        SEC
        SBC #1
@long:  STA LOGL
@kept:
        JSR rockslate
        JMP endwork

st_dying:
        JSR drawbul
        JSR drawship
        JSR explode
        JSR rocks
        JSR btimers
        JSR physall
        JSR collide
        JSR rockslate
        DEC TIMER
        BNE @done
        LDA LIVES
        BEQ @over
        JSR respawn
        JMP endwork
@over:  JSR gameover
@done:  JMP endwork

st_over:
        JSR drawbul
        JSR rocks
        JSR btimers
        JSR physall
        JSR collide
        JSR rockslate
        LDA TIMER
        BEQ @wait
        DEC TIMER
        JMP endwork
@wait:  LDA KEYIN
        CMP #' '
        BNE @done
        JSR newgame
@done:  JMP endwork

; ---------------------------------------------------------------- set-up
gotitle: LDA #0
        STA STATE
        JSR clrobj
        JSR clrbmp
        JSR drawframe
        JSR stars
        JSR drawplanet
        JSR drawlogo
        JSR clrtext
        LDX #<txtitle
        LDY #>txtitle
        JSR print
        LDX #<txkeys
        LDY #>txkeys
        JSR print
        LDA #3
        JSR spawnrocks
        LDA #1
        STA NOCHK
        RTS

; newgame: seed RND from the moment SPACE was taken, then set up wave 1. NOISE depends on
; the cycle it's read on (chapter 12), and the player decides which frame that is, so
; every game's stars and rocks are different. RNDSEED turns a seed of 0 into a good one.
newgame: LDX NOISE
        LDY NOISE
        JSR RNDSEED
        JSR clrobj
        JSR clrbmp
        JSR drawframe
        JSR stars
        JSR drawplanet
        JSR clrtext
        LDX #<txhud
        LDY #>txhud
        JSR print
        LDX #<txkeys
        LDY #>txkeys
        JSR print
        LDA #0          ; 000000, as txhud shows it
        LDX #5
@score: STA SCORE,X
        DEX
        BPL @score
        STA SHOTS
        STA SNDPRI      ; whatever was playing gives way to the new wave's tune
        LDA #3
        STA LIVES
        LDA #1
        STA WAVE
        JSR hudwave
        JSR hudlives
        JSR respawn
        LDA #FX_WAVE
        JSR sfx
        LDA #3
        JSR spawnrocks
        LDA #1
        STA NOCHK
        RTS

respawn: LDA #128
        STA OXH
        LDA #44
        STA OYH
        LDA #$80
        STA OXL
        STA OYL
        LDA #SHIPV0L    ; a circular orbit, clockwise
        STA OVXL
        LDA #SHIPV0H
        STA OVXH
        LDA #0
        STA OVYL
        STA OVYH
        STA THRUST
        LDA #24
        STA SHIPDIR
        LDA #1
        STA KIND
        STA STATE
        LDA #INVULT
        STA INVUL
        RTS

gameover: LDA #3
        STA STATE
        LDA #60
        STA TIMER
        LDX #<txover
        LDY #>txover
        JMP print

clrobj: LDA #0
        LDX #NOBJ-1
@obj:   STA KIND,X
        STA DK,X
        DEX
        BPL @obj
        STA SDRAWN
        STA EXDR
        STA WAVET
        STA THRUST
        STA BURN
        STA INVUL
        STA COOL
        LDA #ROCK0
        STA RRDR
        RTS

; never spawn a rock on the ship. players forgive anything but that.
;
; spawnrocks: A big rocks on the ring around the planet, each on a circular orbit
; (clockwise or not, at random) with a little random wobble.
spawnrocks:
        STA T4
        TAY
        LDA SPACING,Y
        STA T3
        JSR RND
        AND #31
        STA T2
        LDX #ROCK0
@rock:  LDY T2
        LDA SPX,Y
        CLC
        ADC #128
        STA OXH,X
        LDA SPY,Y
        CLC
        ADC #84
        STA OYH,X
        LDA #$80
        STA OXL,X
        STA OYL,X
        JSR RND
        AND #1
        BEQ @ccw
        LDA ORBXL,Y
        STA OVXL,X
        LDA ORBXH,Y
        STA OVXH,X
        LDA ORBYL,Y
        STA OVYL,X
        LDA ORBYH,Y
        STA OVYH,X
        JMP @wobble
@ccw:   LDA #0
        SEC
        SBC ORBXL,Y
        STA OVXL,X
        LDA #0
        SBC ORBXH,Y
        STA OVXH,X
        LDA #0
        SEC
        SBC ORBYL,Y
        STA OVYL,X
        LDA #0
        SBC ORBYH,Y
        STA OVYH,X
@wobble: JSR RND
        AND #$3F
        SEC
        SBC #$20
        JSR addvx
        JSR RND
        AND #$3F
        SEC
        SBC #$20
        JSR addvy
        LDA #2
        STA KIND,X
        LDA T2
        CLC
        ADC T3
        AND #31
        STA T2
        INX
        DEC T4
        BNE @rock
        RTS

SPACING: .byte 0, 0, 16, 11, 8, 6

; addvx / addvy: the velocity of object X += signed A / 256. Uses Y.
addvx:  LDY #0
        CMP #$80
        BCC @add
        DEY
@add:   CLC
        ADC OVXL,X
        STA OVXL,X
        TYA
        ADC OVXH,X
        STA OVXH,X
        RTS
addvy:  LDY #0
        CMP #$80
        BCC @add
        DEY
@add:   CLC
        ADC OVYL,X
        STA OVYL,X
        TYA
        ADC OVYH,X
        STA OVYH,X
        RTS

; ---------------------------------------------------------------- input
; pinput: the controls, read as held keys from the keyboard matrix (chapter 11), so the
; ship can turn, thrust and fire all at once. PAD gets one bit for each control that's down:
PLEFT   = 1             ; A or cursor left
PRIGHT  = 2             ; D or cursor right
PTHRUST = 4             ; W or cursor up
PFIRE   = 8             ; SPACE
; The cursor keys sit in row 8, bits 0-2 (LEFT, RIGHT, UP), the same bits as PLEFT,
; PRIGHT and PTHRUST, so that row goes straight in.
        .assert PLEFT == 1 && PRIGHT == 2 && PTHRUST == 4, "row 8 of the matrix is LEFT, RIGHT, UP"
pinput: LDA #0
        STA PAD
        .irp k, ' ', 'W', 'D', 'A'      ; each key's bit rolls into PAD from the right, so
        LDA KEYS + [k - ' '] / 8        ; SPACE ends up in bit 3 and A in bit 0. Rows 0-7
        AND #1 << [k & 7]               ; are the codes $20-$5F, eight to a row
        CMP #1                          ; carry set: the key is down
        ROL PAD
        .endr
        LDA KEYS+8                      ; row 8: LEFT, RIGHT, UP in bits 0-2
        AND #PLEFT | PRIGHT | PTHRUST
        ORA PAD
        STA PAD
        LDA KEYIN       ; a key pressed and let go between two frames never shows in the
        BEQ @steer      ; matrix, but KBD has it: it counts as down for this frame
        LDX #TAPBIT-TAPKEY-1
@tap:   CMP TAPKEY,X
        BEQ @tapped
        DEX
        BPL @tap
        BMI @steer
@tapped: LDA TAPBIT,X
        ORA PAD
        STA PAD
; Steering: a step on the first frame the steering keys are as they are, then a pause, so
; a tap is exactly one step; held, a step every other frame from the HOLDDLYth on.
@steer: LDA PAD
        EOR OLDPAD
        AND #PLEFT | PRIGHT
        BEQ @same
        LDA #0          ; the steering keys changed: start counting again
        STA HOLDN
        BEQ @turn
@same:  INC HOLDN
        BNE @held
        DEC HOLDN       ; held for ever: stay at 255
@held:  LDA HOLDN
        CMP #HOLDDLY
        BCC @thrust
        LSR A
        BCS @thrust
@turn:  LDA PAD
        AND #PLEFT | PRIGHT
        TAX
        LDA SHIPDIR
        CLC
        ADC TURNS,X     ; none, left, right, or both: none
        AND #31
        STA SHIPDIR
@thrust: LDA PAD        ; thrust: a fresh press burns THRTAP frames, so a tap pushes;
        AND #PTHRUST    ; held, it keeps burning, and stops when the key comes up
        BEQ @fire
        AND OLDPAD
        BNE @burn
        LDA #THRTAP
        STA THRUST
        BNE @fire
@burn:  LDA THRUST
        BNE @fire
        INC THRUST
@fire:  LDA PAD
        STA OLDPAD
        AND #PFIRE      ; the gun fires whenever it's ready while SPACE is down
        BEQ @done
        JMP fire
@done:  RTS
TURNS:  .byte 0, -1, 1, 0
TAPKEY: .byte 'A', 28, 'D', 29, 'W', 30, ' '        ; 28-30: the cursor keys
TAPBIT: .byte PLEFT, PLEFT, PRIGHT, PRIGHT, PTHRUST, PTHRUST, PFIRE

; fire: a bullet from the ship's nose, if the gun is ready and a bullet is free.
fire:   LDA COOL
        BNE @no
        LDA KIND
        BEQ @no
        LDX #BUL0
@free:  LDA KIND,X
        ORA DK,X
        BEQ @shoot
        INX
        CPX #BUL0+NBUL
        BNE @free
@no:    RTS
@shoot: LDY SHIPDIR
        TYA
        STA BDIR,X
        LDA OYH
        STA CY
        LDA SVY0,Y
        JSR addy
        STA OYH,X
        LDA OXH
        CLC
        ADC SVX0,Y
        STA OXH,X
        LDA #$80
        STA OXL,X
        STA OYL,X
        LDA OVXL
        CLC
        ADC BVXL,Y
        STA OVXL,X
        LDA OVXH
        ADC BVXH,Y
        STA OVXH,X
        LDA OVYL
        CLC
        ADC BVYL,Y
        STA OVYL,X
        LDA OVYH
        ADC BVYH,Y
        STA OVYH,X
        LDA #BLIFE
        STA KIND,X
        LDA #COOLT
        STA COOL
        INC SHOTS
        LDA #FX_SHOT
        JMP sfx

; timers: thrust, the gun, safety, then the bullets' lives.
timers: LDA THRUST
        STA BURN        ; the engine burns this frame if there's thrust left
        BEQ @gun
        DEC THRUST
        JSR dothrust
@gun:   LDA COOL
        BEQ @safe
        DEC COOL
@safe:  LDA INVUL
        BEQ btimers
        DEC INVUL
btimers: LDX #BUL0
@bul:   LDA KIND,X
        BEQ @next
        SEC             ; no DEC abs,X on the UNC-1
        SBC #1
        STA KIND,X
@next:  INX
        CPX #BUL0+NBUL
        BNE @bul
        RTS

dothrust: LDY SHIPDIR
        LDX #0
        LDA THRX,Y
        BPL @x
        DEX
@x:     CLC
        ADC OVXL
        STA OVXL
        TXA
        ADC OVXH
        STA OVXH
        LDX #0
        LDA THRY,Y
        BPL @y
        DEX
@y:     CLC
        ADC OVYL
        STA OVYL
        TXA
        ADC OVYH
        STA OVYH
        RTS

; clampv: the ship's speed stays within -3..+3 px/frame on each axis.
clampv: LDA OVXH
        BMI @xneg
        CMP #3
        BCC @y
        LDA #2
        STA OVXH
        LDA #$FF
        STA OVXL
        JMP @y
@xneg:  CMP #$FD
        BCS @y
        LDA #$FD
        STA OVXH
        LDA #0
        STA OVXL
@y:     LDA OVYH
        BMI @yneg
        CMP #3
        BCC @done
        LDA #2
        STA OVYH
        LDA #$FF
        STA OVYL
        RTS
@yneg:  CMP #$FD
        BCS @done
        LDA #$FD
        STA OVYH
        LDA #0
        STA OVYL
@done:  RTS

; ---------------------------------------------------------------- physics
physall: LDX #NOBJ-1
@obj:   LDA KIND,X
        BEQ @next
        JSR phys1
@next:  DEX
        BPL @obj
        RTS

; the three lines are in here. the table does the rest.
;
; phys1: gravity, then motion, for object X. Also PD,X = distance to the planet.
phys1:  LDA OXH,X
        SEC
        SBC #128
        STA SGX
        BPL @ax
        EOR #$FF
        CLC
        ADC #1
@ax:    STA AGX
        LSR A
        LSR A
        CMP #22
        BCC @cx
        LDA #21
@cx:    STA GIX
        LDA OYH,X
        SEC
        SBC #84
        STA SGY
        BPL @ay
        EOR #$FF
        CLC
        ADC #1
@ay:    STA AGY
        LSR A
        LSR A
        CMP #22
        BCC @cy
        LDA #21
@cy:    STA GIY
        TAY             ; TP = row GIY: the y pull is G[GIY][GIX] (growp, in line)
        LDA GROWL,Y
        STA TP
        LDA GROWH,Y
        STA TP+1
        LDY GIX
        LDA (TP),Y
        STA GY
        LDA GROWL,Y     ; and row GIX: the x pull is G[GIX][GIY]
        STA TP
        LDA GROWH,Y
        STA TP+1
        LDY GIY
        LDA (TP),Y
        STA GX
        LDA AGX         ; distance ~ max + min/2
        CMP AGY
        BCS @xbig
        LSR A
        CLC
        ADC AGY
        JMP @dist
@xbig:  LDA AGY
        LSR A
        CLC
        ADC AGX
@dist:  STA PD,X
        LDA SGX
        BMI @left
        LDA OVXL,X      ; right of the planet: pull left
        SEC
        SBC GX
        STA OVXL,X
        LDA OVXH,X
        SBC #0
        STA OVXH,X
        JMP @vy
@left:  LDA OVXL,X
        CLC
        ADC GX
        STA OVXL,X
        LDA OVXH,X
        ADC #0
        STA OVXH,X
@vy:    LDA SGY
        BMI @above
        LDA OVYL,X      ; below the planet: pull up
        SEC
        SBC GY
        STA OVYL,X
        LDA OVYH,X
        SBC #0
        STA OVYH,X
        JMP @move
@above: LDA OVYL,X
        CLC
        ADC GY
        STA OVYL,X
        LDA OVYH,X
        ADC #0
        STA OVYH,X
@move:  CLC             ; x wraps at 256 on its own
        LDA OXL,X
        ADC OVXL,X
        STA OXL,X
        LDA OXH,X
        ADC OVXH,X
        STA OXH,X
        CLC             ; y wraps at 168
        LDA OYL,X
        ADC OVYL,X
        STA OYL,X
        LDA OYH,X
        ADC OVYH,X
        CMP #AREA_H
        BCC @sety
        LDY OVYH,X
        BMI @up
        SBC #AREA_H
        JMP @sety
@up:    CLC
        ADC #AREA_H
@sety:  STA OYH,X
        RTS

; growp: TP = row A of GTAB, from a table: there's no multiply. Keeps X.
growp:  TAY
        LDA GROWL,Y
        STA TP
        LDA GROWH,Y
        STA TP+1
        RTS

; ---------------------------------------------------------------- collisions
; odist: A ~ distance between objects X and Y on the torus (255 if clearly far).
odist:  LDA OXH,X
        SEC
        SBC OXH,Y
        BPL @dx
        EOR #$FF
        CLC
        ADC #1
@dx:    CMP #24
        BCS @far
        STA T0
        LDA OYH,X
        SEC
        SBC OYH,Y
        BCS @dy
        EOR #$FF
        ADC #1
@dy:    CMP #84
        BCC @near
        EOR #$FF
        SEC
        ADC #AREA_H
@near:  CMP #24
        BCS @far
        CMP T0
        BCC @ybig
        STA T1
        LDA T0
        LSR A
        CLC
        ADC T1
        RTS
@ybig:  LSR A
        CLC
        ADC T0
        RTS
@far:   LDA #255
        RTS

collide: LDA KIND       ; the ship
        BEQ cbul
        LDA PD
        CMP #13
        BCS @safe
        JSR killship
        JMP cbul
@safe:  LDA INVUL
        BNE cbul
        LDY #ROCK0
@rock:  LDA KIND,Y
        BEQ @next
        LDA OXH         ; 24 or more apart across (most are)? Then no need to look
        SEC             ; closer (with the borrow: |dx| < 24 is dx + 24 < 48, mod 256)
        SBC OXH,Y
        ADC #23
        CMP #48
        BCS @next
        LDX #0
        JSR odist
        LDX KIND,Y
        CMP SHIPR,X
        BCS @next
        JSR killship
        JMP cbul
@next:  INY
        CPY #NOBJ
        BNE @rock
cbul:   LDX #BUL0       ; bullets: the planet swallows them, rocks break
@bul:   LDA KIND,X
        BEQ @nextb
        LDA PD,X
        CMP #11
        BCS @rocks
        LDA #0
        STA KIND,X
        JMP @nextb
@rocks: LDY #ROCK0
@rock:  LDA KIND,Y
        BEQ @nextr
        LDA OXH,X       ; 24 or more apart across? (as for the ship)
        SEC
        SBC OXH,Y
        ADC #23
        CMP #48
        BCS @nextr
        JSR odist
        STX OBJ
        LDX KIND,Y
        CMP BULR,X
        LDX OBJ
        BCS @nextr
        JSR hitrock
        JMP @nextb
@nextr: INY
        CPY #NOBJ
        BNE @rock
@nextb: INX
        CPX #BUL0+NBUL
        BNE @bul
rockplanet: LDX #ROCK0  ; rocks that fall into the planet are gone (no score)
@rock:  LDY KIND,X
        BEQ @next
        LDA PD,X
        CMP ROCKPR,Y
        BCS @next
        LDA #0
        STA KIND,X
@next:  INX
        CPX #NOBJ
        BNE @rock
        RTS

SHIPR:  .byte 0, 8, 12  ; hit distances by rock kind
BULR:   .byte 0, 6, 10
ROCKPR: .byte 0, 13, 17

; hitrock: bullet X hits rock Y. A big rock splits in two, kicked apart across the
; bullet's path, for 20 points; a small one is gone, for 50. Keeps X.
hitrock: STX HITX
        LDA #0
        STA KIND,X
        LDA KIND,Y
        CMP #2
        BEQ @split
        LDA #0
        STA KIND,Y
        LDA #FX_SMALL
        JSR sfx
        LDA #5          ; 5 tens
        LDX #4
        JSR addpts
        LDX HITX
        RTS
@split: LDA #1
        STA KIND,Y
        LDA BDIR,X
        CLC
        ADC #8
        AND #31
        STY T4
        TAY             ; the kick: the bullet's velocity turned 90 degrees, / 8
        LDA BVXL,Y
        STA GIX
        LDA BVXH,Y
        STA SGX
        LDA BVYL,Y
        STA GIY
        LDA BVYH,Y
        STA SGY
        LDY #3
@shift: LDA SGX         ; signed shift right: copy the sign into the carry first
        CMP #$80
        ROR SGX
        ROR GIX
        LDA SGY
        CMP #$80
        ROR SGY
        ROR GIY
        DEY
        BNE @shift
        LDY T4
        LDX #ROCK0
@free:  LDA KIND,X
        ORA DK,X
        BEQ @new
        INX
        CPX #NOBJ
        BNE @free
        JMP @old        ; no room: just the one small rock
@new:   LDA OXL,Y
        STA OXL,X
        LDA OXH,Y
        STA OXH,X
        LDA OYL,Y
        STA OYL,X
        LDA OYH,Y
        STA OYH,X
        LDA OVXL,Y
        STA OVXL,X
        LDA OVXH,Y
        STA OVXH,X
        LDA OVYL,Y
        STA OVYL,X
        LDA OVYH,Y
        STA OVYH,X
        LDA #1
        STA KIND,X
        LDA OVXL,X      ; the new one: v -= kick
        SEC
        SBC GIX
        STA OVXL,X
        LDA OVXH,X
        SBC SGX
        STA OVXH,X
        LDA OVYL,X
        SEC
        SBC GIY
        STA OVYL,X
        LDA OVYH,X
        SBC SGY
        STA OVYH,X
@old:   LDX T4          ; the old one: v += kick
        LDA OVXL,X
        CLC
        ADC GIX
        STA OVXL,X
        LDA OVXH,X
        ADC SGX
        STA OVXH,X
        LDA OVYL,X
        CLC
        ADC GIY
        STA OVYL,X
        LDA OVYH,X
        ADC SGY
        STA OVYH,X
        LDA #FX_BIG
        JSR sfx
        LDA #2          ; 2 tens
        LDX #4
        JSR addpts
        LDY T4
        LDX HITX
        RTS

; you'll die here more than anywhere. so did i.
;
killship: LDA #0
        STA KIND
        STA THRUST
        STA BURN
        STA EXR
        STA EXDR
        LDA OXH
        STA EXX
        LDA OYH
        STA EXY
        DEC LIVES
        JSR hudlives
        LDA #2
        STA STATE
        LDA #60
        STA TIMER
        LDA #FX_DIE
        JMP sfx

; wavecheck: when the last rock is gone, wait, then send the next wave.
wavecheck: LDA WAVET
        BNE @wait
        JSR countrocks
        BNE @done
        LDA #WAVEDLY
        STA WAVET
@done:  RTS
@wait:  DEC WAVET
        BNE @done
        JSR rocksclear
        BEQ @next
        INC WAVET       ; the last rock is still on screen: next frame
        RTS
@next:  INC WAVE
        JSR hudwave
        LDA #FX_WAVE
        JSR sfx
        LDA WAVE
        CLC
        ADC #2
        CMP #6
        BCC @n
        LDA #5
@n:     JMP spawnrocks

; countrocks: A = live rocks (Z set if none).
countrocks: LDA #0
        STA T0
        LDX #ROCK0
@rock:  LDA KIND,X
        BEQ @next
        INC T0
@next:  INX
        CPX #NOBJ
        BNE @rock
        LDA T0
        RTS

; rocksclear: Z set if no rock is drawn on the screen.
rocksclear: LDA #0
        LDX #ROCK0
@rock:  ORA DK,X
        INX
        CPX #NOBJ
        BNE @rock
        CMP #0
        RTS

; ---------------------------------------------------------------- sound
; One voice: the timer plays a square wave of 1,000,000 / divisor Hz while SPEAKER is %11
; (chapter 12), and costs nothing between writes. An effect is a list of steps in SFX,
; each three bytes: frames, then the divisor's high and low bytes; a 0 ends it. A high
; byte with bit 7 set makes the step noise: every frame the high byte is its low 7 bits
; plus a random amount (NOISE AND the third byte) and the low byte is random, so the pitch
; jumps about fifty times a second: a crunch, or with a small range, a rumble.
; Effects are numbered by priority, highest last. A new one starts only if it's at least
; as important as the one playing, so explosions beat shots and shots beat the engine.
FX_SHOT = 1
FX_SMALL = 2
FX_BIG  = 3
FX_WAVE = 4
FX_DIE  = 5
NZ      = $80           ; a noise step
ENGINE  = NZ | $14      ; the engine's rumble: divisors $1400-$23FF, 110-195 Hz
ENGJIT  = $0F

; sfx: start effect A, unless something more important is playing. Changes A and X.
sfx:    CMP SNDPRI
        BCC @no
        STA SNDPRI
        TAX
        LDA SFXAT-1,X   ; where its steps start
        STA SNDP
        LDA #0          ; no frames left: the sequencer takes the first step next frame
        STA SNDN
@no:    RTS

; sound: the sequencer, once a frame. Steps the effect playing; when it ends, or if none
; is, the engine rumbles while it burns, and otherwise the gate closes: silence.
; 60 cycles a frame for a tone, about 85 for noise. Changes A, X and T0.
sound:  LDA SNDPRI
        BEQ @engine     ; no effect
        LDA SNDN
        BNE @play       ; frames left of this step
        LDX SNDP        ; the next step
        LDA SFX,X
        BEQ @over       ; a 0: the effect is over
        STA SNDN
        LDA SFX+1,X
        STA SNDHI
        LDA SFX+2,X
        STA SNDLO
        INX
        INX
        INX
        STX SNDP
@play:  DEC SNDN
        LDX SNDLO
        LDA SNDHI
@voice: BPL @tone       ; A = high byte, X = low byte: a plain tone
        AND #$7F        ; noise: X is how far the high byte jitters
        STA T0
        TXA
        AND NOISE
        CLC
        ADC T0
        LDX NOISE       ; and any low byte
@tone:  STX TIMER_LO    ; the low byte first: writing TIMER_HI is what loads the divisor
        STA TIMER_HI
        LDA #%11        ; gate on, data on: the timer drives the speaker
        STA SPEAKER
        RTS
@over:  LDA #0
        STA SNDPRI
@engine: LDA BURN       ; nothing playing: the engine, if it burns this frame
        BEQ @quiet
        LDX #ENGJIT
        LDA #ENGINE
        BMI @voice      ; always: ENGINE has bit 7 set
@quiet: STA SPEAKER     ; A is 0: gate off, data off. The speaker rests
        RTS

SFXAT:  .irp s, S_SHOT, S_SMALL, S_BIG, S_WAVE, S_DIE    ; where each effect starts in SFX,
        .byte s - SFX                                    ; in the order of their numbers
        .endr
SFX:
S_SHOT: .byte 1, >400, <400             ; a falling "pew", a frame a step: 2,500 Hz...
        .byte 1, >550, <550
        .byte 1, >750, <750
        .byte 1, >1000, <1000
        .byte 1, >1350, <1350, 0        ; ...to 740 Hz
S_SMALL: .byte 2, NZ | $03, $03         ; a small rock: a short, high crack
        .byte 3, NZ | $06, $07, 0
S_BIG:  .byte 3, NZ | $06, $07          ; a big rock: a deeper crunch, falling
        .byte 4, NZ | $0C, $0F
        .byte 5, NZ | $18, $1F, 0
S_WAVE: .byte 5, >1911, <1911           ; a new wave: C E G C, rising
        .byte 5, >1517, <1517
        .byte 5, >1276, <1276
        .byte 10, >956, <956, 0
S_DIE:  .byte 4, NZ | $04, $07          ; the ship: a long roar, falling away
        .byte 6, NZ | $0A, $0F
        .byte 8, NZ | $14, $1F
        .byte 10, NZ | $28, $3F
        .byte 12, NZ | $50, $3F, 0
SFXEND:
        .assert SFXEND - SFX <= 256, "SFX is indexed by X: 256 bytes at most"

; ---------------------------------------------------------------- score and HUD
; addpts: add A (0-9) to score digit X (5 the ones, 4 the tens ... 0 the hundred
; thousands), carrying into the digits to its left, and show each digit that changes.
; The score changes in the busiest frames, so it's kept as a row of decimal digits, the
; way the manual's "Keeping score fast" does it (chapter 13): about 34 cycles a hit, where
; NUMSTR and PRNUM would take 2,000. Past 999999 it rolls over to 000000. Changes A, X.
addpts: CLC
        ADC SCORE,X
        CMP #10
        BCC @last       ; under 10: no carry, and this is the last digit to change
        SBC #10         ; CMP left the carry set
        STA SCORE,X
        ORA #'0'        ; the digit's character
        STA SCOREPOS,X
        LDA #1          ; carry one...
        DEX             ; ...into the digit to the left
        BPL addpts
        RTS
@last:  STA SCORE,X
        ORA #'0'
        STA SCOREPOS,X
        RTS

; hudwave / hudlives: the wave in two digits and the ships in one, printed by the ROM's
; NUMSTR and PRNUM (chapter 13). They change once a wave or once a life, so the ROM's
; 900 or so cycles are fine. Change A, X, Y and T0.
hudwave: LDA WAVE
        LDX #WAVECOL
        LDY #2
        BNE hudnum
hudlives: LDA LIVES
        LDX #LIVESCOL
        LDY #1
; hudnum: A printed on row 22 from column X, exactly Y digits, padded with zeros.
hudnum: STX CURX
        STY T0
        TAX             ; NUMSTR takes the number low byte first: X, then Y, then A
        LDY #22
        STY CURY
        LDY #0
        TYA
        JSR NUMSTR      ; the eight digits into NUMBUF
        LDA T0
        JMP PRNUM       ; printed T0 wide; printing leaves no cursor behind

; print: X/Y = address of a screen address (word), then text, then 0.
print:  STX TP
        STY TP+1
        LDY #0
        LDA (TP),Y
        SEC
        SBC #2
        STA PTR
        INY
        LDA (TP),Y
        SBC #0
        STA PTR+1
        INY
@chr:   LDA (TP),Y
        BEQ @done
        STA (PTR),Y
        INY
        BNE @chr
@done:  RTS

clrtext: LDA #$20        ; only rows 21-24: in mixed mode the rows above are under the
        LDX #4*40       ; arena, never shown, and free RAM (FASTY)
@page:  STA ROW21-1,X
        DEX
        BNE @page
        LDA #$10        ; a rule under the arena
        LDX #39
@rule:  STA ROW21,X
        DEX
        BPL @rule
        RTS

txtitle: .word ROW23+10
        .text "ORBIT "
        .byte $7F               ; the diamond
        .text " PRESS SPACE"
        .byte 0
txkeys: .word ROW24+1
        .text "A/D <> TURN   W ^"
        .text " THRUST   SPACE FIRE"
        .byte 0
txhud:  .word ROW22
        .asciiz " SCORE 000000     WAVE 01      SHIPS 3"
txover: .word ROW23+8
        .text "GAME OVER "
        .byte $7F
        .text " PRESS SPACE"
        .byte 0

; ---------------------------------------------------------------- drawing objects
; drawship: redraw the ship if anything about it changed (XOR: erase, then draw).
drawship: LDA #0
        STA T4
        LDA KIND
        BEQ @flame
        LDA INVUL       ; blinks while it can't be hit
        BEQ @shown
        AND #4
        BEQ @flame
@shown: INC T4
@flame: LDA #0
        STA FLAME
        LDA BURN
        BEQ @drawn
        LDA TICK
        AND #2
        STA FLAME
@drawn: LDA SDRAWN
        BEQ @draw
        LDA T4
        BEQ @erase
        LDA SDX
        CMP OXH
        BNE @move
        LDA SDY
        CMP OYH
        BNE @move
        LDA SDDIR
        CMP SHIPDIR
        BNE @erase      ; (turned: shipmove would say no)
        LDA SDFL
        CMP FLAME
        BNE @move
        RTS
@move:  JSR shipmove    ; moved, or the flame changed: both in one walk, if it can
        BCC @erase
        RTS
@erase: LDA SDX
        STA CX
        LDA SDY
        STA CY
        LDY SDDIR
        LDA SDFL
        JSR shipxor
        LDA #0
        STA SDRAWN
@draw:  LDA T4
        BEQ @done
        LDA OXH
        STA CX
        STA SDX
        LDA OYH
        STA CY
        STA SDY
        LDY SHIPDIR
        STY SDDIR
        LDA FLAME
        STA SDFL
        JSR shipxor
        LDA #1
        STA SDRAWN
@done:  RTS

; shipxor: the ship at CX, CY, direction Y; flame if A isn't 0.
shipxor: STA SFLAG
        STY SDIRT
        LDA CY          ; clear of the arena's top and bottom edges (8 rows each way)?
        CMP #8          ; Then its lines step down and up by adding 40 (FASTY)
        BCC @slow
        CMP #AREA_H-8
        BCS @slow
        LDA #1
        BNE @fast
@slow:  LDA #0
@fast:  STA FASTY
        JSR shiphull
        LDA SFLAG
        BEQ @done
        JSR shipflame
@done:  LDA #0          ; every other line steps the careful way
        STA FASTY
        RTS

; shiphull: the ship's outline, nose to rear left to notch to rear right and back, at CX,
; CY, direction SDIRT. After the first line, each goes on from where the last ended.
shiphull: LDY SDIRT
        LDA SVX0,Y
        STA T0
        LDA SVY0,Y
        STA T1
        LDA SVX1,Y
        STA T2
        LDA SVY1,Y
        STA T3
        JSR seg
        LDY SDIRT
        LDA SVX2,Y
        STA T2
        LDA SVY2,Y
        STA T3
        JSR segc
        LDY SDIRT
        LDA SVX3,Y
        STA T2
        LDA SVY3,Y
        STA T3
        JSR segc
        LDY SDIRT
        LDA SVX0,Y
        STA T2
        LDA SVY0,Y
        STA T3
        JMP segc

; shipflame: the flame, at CX, CY, direction SDIRT.
shipflame: LDY SDIRT
        LDA SVX4,Y
        STA T0
        LDA SVY4,Y
        STA T1
        LDA SVX2,Y
        STA T2
        LDA SVY2,Y
        STA T3
        JMP seg

; shipmove: the ship, drawn at SDX, SDY, now at OXH, OYH: if it's facing the same way, both
; places are clear of the arena's edges and they're within 8 pixels each way, rub it out
; there and draw it here in one walk round the outline (MERGE): each line's work done
; once, and two XORs a pixel, the old place's PTR2 and column (DX) a fixed way off. The
; flame goes in the same walk if it's the same in both, else on its own. Carry set if it
; did; carry clear and nothing drawn if not.
shipmove: LDA SDDIR
        CMP SHIPDIR
        BNE @no
        LDA SDY
        CMP #8
        BCC @no
        CMP #AREA_H-8
        BCS @no
        LDA OYH
        CMP #8
        BCC @no
        CMP #AREA_H-8
        BCS @no
        LDA SDX         ; DX = the old x - the new, -8 to 8
        SEC
        SBC OXH
        STA DX
        CLC
        ADC #8
        CMP #17
        BCS @no
        LDA SDY         ; the rows between, -8 to 8: DLO/DHI = 40 x that
        SEC
        SBC OYH
        CLC
        ADC #8
        CMP #17
        BCC @go
@no:    CLC
        RTS
@go:    SBC #7          ; (carry clear: - 8) the rows between, again
        LDY #0
        STY DLO
        STY DHI
        TAY
        BEQ @patch
        BMI @neg
@pos:   LDA DLO
        CLC
        ADC #40
        STA DLO
        BCC @p1
        INC DHI
@p1:    DEY
        BNE @pos
        BEQ @patch
@neg:   LDA DLO
        SEC
        SBC #40
        STA DLO
        BCS @n1
        DEC DHI
@n1:    INY
        BNE @neg
@patch: LDY #0          ; DX into the loops' second XOR: XCOL + DX and XMASK + DX
        LDA DX
        BPL @dxp
        DEY             ; (Y: its high byte)
@dxp:   CLC
        ADC #<XCOL
        STA xm2_c+1
        STA ymd2_c+1
        STA ymu2_c+1
        TYA
        ADC #>XCOL
        STA xm2_c+2
        STA ymd2_c+2
        STA ymu2_c+2
        LDA DX
        CLC
        ADC #<XMASK
        STA xm2_m+1
        STA ymd2_m+1
        STA ymu2_m+1
        TYA
        ADC #>XMASK
        STA xm2_m+2
        STA ymd2_m+2
        STA ymu2_m+2
        LDA OXH         ; the walk, at the new place
        STA CX
        LDA OYH
        STA CY
        LDA SHIPDIR
        STA SDIRT
        LDA #1
        STA FASTY
        STA MERGE
        JSR shiphull
        LDA SDFL
        CMP FLAME
        BNE @flames
        TAX
        BEQ @done
        JSR shipflame   ; the same flame in both places
@done:  LDA #0
        STA MERGE
        STA FASTY
        LDA OXH
        STA SDX
        LDA OYH
        STA SDY
        LDA FLAME
        STA SDFL
        SEC
        RTS
@flames: LDA #0         ; the flame in one place only: on its own
        STA MERGE
        LDA SDFL
        BEQ @new
        LDA SDX
        STA CX
        LDA SDY
        STA CY
        JSR shipflame   ; the old one, rubbed out
@new:   LDA FLAME
        BEQ @done
        LDA OXH
        STA CX
        LDA OYH
        STA CY
        JSR shipflame   ; the new one
        JMP @done

; drawbul: bullets are 2 x 2 dots.
drawbul: LDX #BUL0
@bul:   STX OBJ
        LDA DK,X
        BEQ @draw
        LDA KIND,X
        BEQ @erase
        LDA OXH,X
        CMP DXP,X
        BNE @erase
        LDA OYH,X
        CMP DYP,X
        BEQ @next
@erase: LDA DXP,X
        STA T0
        LDA DYP,X
        JSR bdot
        LDX OBJ
        LDA #0
        STA DK,X
@draw:  LDA KIND,X
        BEQ @next
        LDA OXH,X
        STA DXP,X
        STA T0
        LDA OYH,X
        STA DYP,X
        JSR bdot
        LDX OBJ
        LDA #1
        STA DK,X
@next:  LDX OBJ
        INX
        CPX #BUL0+NBUL
        BNE @bul
        RTS

; bdot: XOR a 2 x 2 dot with its top-left corner at (T0, A). Its two pixels are in one
; byte unless x AND 7 is 7: then through the shifter (chapter 7). The dot's line,
; %11000000, goes in and comes out moved right by x AND 7, for byte column XCOL[x]; a 0
; pushed after it brings out what spilled into the next column. 219 cycles; the push of 0
; also leaves the window empty, ready for the next line.
; Uses A, X, Y, T2 and T3.
bdot:   TAY             ; the top line
        LDX T0
        JSR setline     ; LY and PTR: the top line (keeps X)
        TXA
        AND #7
        CMP #7
        BEQ @spill      ; its two pixels in two bytes: through the shifter
        LDA XMASK,X     ; else both in one byte: its two bits, XORed in (140 cycles)
        ORA XMASK+1,X
        STA T2
        LDY XCOL,X
        STY T3
        EOR (PTR),Y
        STA (PTR),Y
        JSR ydown       ; the line below, wrapping at the bottom (keeps X)
        LDY T3
        LDA T2
        EOR (PTR),Y
        STA (PTR),Y
        RTS
@spill: STX SHIFT_AMT   ; only bits 0-2 count: x AND 7
        LDA XCOL,X
        STA T2          ; the dot's byte column
        LDA XCOL+8,X
        STA T3          ; and the next, wrapped round the arena
        JSR @line
        JSR ydown       ; the line below, wrapping at the bottom
@line:  LDA #%11000000
        STA SHIFT_IN
        LDA SHIFT_OUT
        LDY T2
        EOR (PTR),Y
        STA (PTR),Y
        LDA #0
        STA SHIFT_IN
        LDA SHIFT_OUT   ; the spill
        BEQ @done
        LDY T3
        EOR (PTR),Y
        STA (PTR),Y
@done:  RTS

; rocks: bring the rocks on the screen up to date, round robin, as far as the beam and the
; time allow: twice a frame, before the frame's work (rocks) and after it, while the beam
; draws the arena (rockslate). A rock is drawn only where the beam won't catch it half
; done (beamok), and only if it's done by the pass's deadline, DEADL: for the first pass,
; in time for the frame's work after it (as long as the last frame's took, LOGL, and a
; margin); for the late pass, before the next frame's work at 219. A rock the beam is in
; the way of is passed over, and the next pass starts with it. When the frame is busy,
; some rocks wait: the picture lags, the physics never does.
rockslate:
        LDA RASTER      ; too near the next frame's work for even a small rock? Then
        LDY RASTHI      ; don't even look (RASTER first: it latches RASTHI)
        BNE @go
        CMP #ARENA0+AREA_H-RMARGIN-RSMALL
        BCC @go
        CMP #VBLANK
        BCS @go
        RTS
@go:    LDA #ARENA0+AREA_H-RMARGIN
        BNE rocksgo
rocks:  LDA LOGL        ; 2-line steps
        ASL A
        EOR #$FF        ; DEADL = 219 - 2 LOGL - 24: lines to spare
        SEC
        ADC #ARENA0+AREA_H-2*RMARGIN
        BCS rocksgo
        LDA #0          ; the work takes the whole arena: no time in it
rocksgo: STA DEADL
        LDA #$FF
        STA RFIRST
        LDA #NOBJ-ROCK0
        STA PIDX
        LDX RRDR
@rock:  STX OBJ
        LDA RASTER      ; no time left for even a small rock? Then stop looking
        LDY RASTHI      ; (RASTER first: it latches RASTHI)
        BNE @look       ; (the bottom border, or this frame's HUD: early)
        CMP #ARENA0+AREA_H
        BCS @look
        ADC #RSMALL     ; (carry clear)
        CMP DEADL
        BCS @end
@look:  JSR rock1
        LDX OBJ
        INX
        CPX #NOBJ
        BNE @count
        LDX #ROCK0
@count: DEC PIDX
        BNE @rock
@end:   LDA RFIRST      ; next time, from the first that waited (or from here)
        BMI @all
        TAX
@all:   STX RRDR
        RTS

RFIRST  = T1            ; rocks: the first rock that waited this pass, or $FF
RLEAD   = T4            ; rocks: lines the job in hand takes (beamok)
DEADL   = T5            ; rocks: the line of the next frame this pass must be done by

; rock1: bring rock X (= OBJ) up to date, if the beam and the time allow; if not, it
; waits (RFIRST). Rubbing out the old picture and drawing the new one go together, when
; the beam is clear of both for the whole job, so the rock is never missing from a frame.
; A rock that jumped across the arena's edge, top to bottom, would wait a long time for
; that: its two jobs are done each on its own. RLD: lines a job takes, by kind.
rock1:  LDA KIND,X
        CMP DK,X
        BNE @redraw
        TAY             ; nothing there, nothing drawn?
        BNE @there
        RTS
@there: LDA OXH,X
        CMP DXP,X
        BNE @redraw
        LDA OYH,X
        CMP DYP,X
        BNE @redraw
        RTS             ; drawn where it is
@redraw: LDY DK,X
        BEQ @new        ; nothing drawn yet: only the new one
        LDA KIND,X
        BEQ @old        ; gone: only rub out the old one
        LDA DYP,X       ; within 16 rows of each other, up or down? (with the borrow,
        SEC             ; 16 above counts as clear)
        SBC OYH,X
        ADC #15
        CMP #33
        BCS @old        ; no, clear of each other: one job at a time
        LDA RLD,Y       ; both jobs (Y = DK)
        LDY KIND,X
        CLC
        ADC RLD,Y
        STA RLEAD
        LDA DYP,X
        JSR beamok
        BCC @wait
        LDA OYH,X
        JSR beamok
        BCC @wait
        JSR rkerase     ; (Z set)
        BEQ @draw
@old:   LDA RLD,Y       ; (Y = DK)
        STA RLEAD
        LDA DYP,X
        JSR beamok
        BCC @wait
        JSR rkerase
@new:   LDY KIND,X      ; the new one (DK is 0 here)
        BEQ @done
        LDA RLD,Y
        STA RLEAD
        LDA OYH,X
        JSR beamok
        BCC @wait
@draw:  LDA KIND,X
        STA DK,X
        PHA
        LDA OXH,X
        STA CX
        STA DXP,X
        LDA OYH,X
        STA CY
        STA DYP,X
        PLA
        JMP rockxor
@wait:  LDA RFIRST      ; the beam's in the way, or the time's up: this rock waits
        BPL @done
        STX RFIRST
@done:  RTS

RLD:    .byte 0, RSMALL, RBIG

; rkerase: rub out rock X's picture (OBJ = X), and say nothing's drawn there. Returns
; with Z set.
rkerase: LDY DXP,X
        STY CX
        LDY DYP,X
        STY CY
        LDA DK,X
        JSR rockxor
        LDX OBJ
        LDA #0
        STA DK,X
        RTS

; beamok: can a rock's picture centred on row A be drawn or rubbed out now, in RLEAD
; lines, without the beam catching it half done (chapter 9), and done by line DEADL of the
; next frame? Carry set if so. The picture's rows, 8 above to 8 below, are T2 to T3. One
; at the arena's edge wraps: its rows are T2 to the bottom and the top to T3, with T3 < T2.
; The beam mustn't be in them, nor reach them within RLEAD lines. Uses A, Y, T2, T3.
beamok: SEC
        SBC #8
        BCC @wtop       ; over the top edge
        STA T2
        ADC #15         ; carry set: + 16
        CMP #AREA_H
        BCC @b
        SBC #AREA_H     ; over the bottom edge (carry set): the top part ends on y + 8 - 168
        BCS @b          ; (always)
@wtop:  ADC #AREA_H     ; the bottom part starts on y - 8 + 168 (and carry is set)
        STA T2
        SBC #AREA_H-16  ; and the top part ends on y + 8
@b:     STA T3
        LDA FRAME       ; how far the work is: 0 before VBLANK, 1 after; 2 is too late
        SEC
        SBC LASTFR
        CMP #2
        BCS @no
        LSR A           ; C: past VBLANK
        LDA RASTER      ; RASTER first: reading it latches bit 8 for RASTHI
        LDY RASTHI
        BNE @lead       ; lines 256-311 (A 0-55): the bottom border, before the arena
        BCC @yes        ; before VBLANK: lines 219-250, the HUD; the arena's drawn
        CMP #VBLANK
        BCS @lead0      ; 251-255: the beam has just left the picture
        CMP #ARENA0+AREA_H
        BCS @no         ; 219-250 again: the next frame's, too late
        TAY             ; lines 0-218, the next frame: in time?
        CLC
        ADC RLEAD
        BCS @no
        CMP DEADL
        BCS @no
        TYA
        CMP #ARENA0
        BCC @lead       ; 0-50, the top border
        SBC #ARENA0     ; in the arena (carry set), on arena line A
        CMP T3
        BEQ @no
        BCC @up         ; above T3
        LDY T3          ; below it: the beam has passed the picture, or, wrapped, its top
        CPY T2          ; part
        BCS @yes
        BCC @ahead
@up:    LDY T3          ; wrapped, and the beam in its top part?
        CPY T2
        BCC @no
@ahead: CLC             ; done before the beam gets to the picture (or its bottom part)?
        ADC RLEAD
        CMP T2
        BCC @yes
@no:    CLC
        RTS
@lead0: LDA #0          ; (as if from line 256)
@lead:  CLC             ; before the arena, A lines from line 0 (or 256): when the job's
        ADC RLEAD       ; done, is the beam still above the picture?
        CMP #ARENA0
        BCC @yes        ; still in the border
        LDY T3          ; (wrapped: its top part is the arena's first rows)
        CPY T2
        BCC @no
        SBC #ARENA0     ; carry set: on arena line A
        CMP T2
        BCS @no
@yes:   SEC
        RTS

; wline: lines since line 219, where a frame's work starts, in 2-line steps (0-155).
wline:  LDA RASTER      ; RASTER first: it latches bit 8 for RASTHI
        LDY RASTHI
        BNE @hi
        CMP #ARENA0+AREA_H
        BCC @next
        SBC #ARENA0+AREA_H ; 219-255: 0-36 (carry set)
        LSR A
        RTS
@hi:    LSR A           ; 256-311 (A 0-55): 37-92
        ADC #18
        RTS
@next:  LSR A           ; 0-218, the next frame: 93-311
        ADC #46
        RTS

; rockxor: XOR rock A (2 big, 1 small) centred at (CX, CY).
; The rocks are thin outlines, about three pixels a line, so they're compiled to code a
; pixel at a time (RKBIG, RKSMALL) rather than pushed through the shifter a byte at a time:
; through the shifter a big rock took 2,334 cycles, against 1,777 for the compiled code.
; A rock clear of the arena's top and bottom edges steps down its lines by adding 40 to
; PTR (ydn40); one that wraps goes by ydown. rstep is patched to jump to the one it needs.
rockxor: TAY
        LDA CY
        CMP RHALF,Y
        BCC @wrap       ; its top line above the arena's
        CMP REDGE,Y
        BCS @wrap       ; its bottom line below it
        LDA #<ydn40
        LDX #>ydn40
        BNE @set
@wrap:  LDA #<ydown
        LDX #>ydown
@set:   STA rstep+1
        STX rstep+2
        CPY #2
        BEQ @big
        JMP RKSMALL
@big:   JMP RKBIG

RHALF:  .byte 0, 4, 8           ; rows above and below the centre, by kind
REDGE:  .byte 0, AREA_H-4, AREA_H-8

rstep:  JMP ydown       ; (patched by rockxor)

; ydn40: PTR one line down, no wrapping (rows are 40 bytes). Keeps X.
ydn40:  LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @r
        INC PTR+1
@r:     RTS

; neg: A = -A.
neg:    EOR #$FF
        CLC
        ADC #1
        RTS

; rocktop: the rocks' first line, A lines above CY: LY and PTR set, and X = CX.
rocktop: EOR #$FF
        SEC
        ADC CY
        BCS @in
        ADC #AREA_H
@in:    TAY
        LDX CX
        JMP setline

; explode: the ship's death: a ring of dots that grows for 30 frames.
explode: LDA EXDR
        BEQ @grow
        JSR exdraw
        LDA #0
        STA EXDR
@grow:  LDA TIMER
        CMP #31
        BCC @done
        INC EXR
        LDA EXR
        STA EXDR
        JSR exdraw
@done:  RTS

exdraw: LDA EXY
        STA CY
        LDA EXDR
        STA T2          ; r
        JSR neg
        STA T0          ; -r
        LDA T2
        LSR A
        LSR A
        STA T3
        LDA T2
        SEC
        SBC T3
        STA T3          ; k = 3r/4: no multiply needed
        JSR neg
        STA T1          ; -k
        LDA T2
        STA T4
        LDA #0
        JSR dot
        LDA T0
        STA T4
        LDA #0
        JSR dot
        LDA #0
        STA T4
        LDA T2
        JSR dot
        LDA #0
        STA T4
        LDA T0
        JSR dot
        LDA T3
        STA T4
        LDA T3
        JSR dot
        LDA T3
        STA T4
        LDA T1
        JSR dot
        LDA T1
        STA T4
        LDA T3
        JSR dot
        LDA T1
        STA T4
        LDA T1
dot:    JSR addy        ; plot at (EXX + T4, CY + A)
        TAY
        LDA EXX
        CLC
        ADC T4
        TAX
        JMP plot

; ---------------------------------------------------------------- graphics
; addy: A = CY + signed A, wrapped into 0-167. Keeps X and Y.
addy:   STA T5
        CLC
        ADC CY
        BIT T5
        BMI @neg
        CMP #AREA_H
        BCC @done
        SBC #AREA_H
@done:  RTS
@neg:   BCS @done
        ADC #AREA_H
        RTS

; seg: a line from (CX + T0, CY + T1) to (CX + T2, CY + T3), last point not drawn.
; It leaves T0, T1 = T2, T3, so the next call continues from the end point.
seg:    LDA CX
        CLC
        ADC T0
        STA LX
        LDA T1
        JSR addy
        TAY
        JSR setline     ; LY and PTR (keeps X)
        LDA MERGE
        BEQ @one
        LDA PTR         ; and PTR2, the old place's: DLO/DHI off
        CLC
        ADC DLO
        STA PTR2
        LDA PTR+1
        ADC DHI
        STA PTR2+1
@one:   LDX LX
; segc: the same, from where the last line ended: X and PTR are its end point.
segc:   LDA T2
        SEC
        SBC T0
        STA DLX
        LDA T3
        SEC
        SBC T1
        STA DLY
        JSR linec
        LDA T2          ; ready for the next segment
        STA T0
        LDA T3
        STA T1
        RTS

; line: Bresenham from (LX, LY) by (DLX, DLY), each -127..127. The end point is not
; drawn, so a closed shape XORs every corner exactly once. x wraps at 256 by itself,
; y at 168 by hand. The x step (INX/DEX), the y step routine and the ink (EOR/ORA)
; are patched into the loops: self-modifying code, so those labels are global.
line:   LDY LY
        LDA YLO,Y
        STA PTR
        LDA YHI,Y
        STA PTR+1
        LDX LX
linec:  LDA DLX         ; (from X and PTR: where the last line ended)
        BPL @right
        JSR neg
        STA ADX
        LDA #$18        ; DEX
        BNE @xstep
@right: STA ADX
        LDA #$16        ; INX
@xstep: STA CNT         ; the x step, patched into the one loop that runs (CNT is its
        LDA DLY         ; count soon)
        BMI @up
        STA ADY
        LDY FASTY
        BNE @fdown
        LDA #<ydown
        STA xm_ys+1
        LDA #>ydown
        STA xm_ys+2
        LDA ADX
        CMP ADY
        BCS @x
        LDA CNT
        STA ymd_inc
        JMP ymd
@x:     LDA CNT
        STA xm_inc
        JMP xmaj
@fdown: LDY MERGE
        BNE @mdown
        LDA #<ydn40     ; clear of the edges: a line down is PTR + 40
        STA xm_ys+1
        LDA #>ydn40
        STA xm_ys+2
        LDA ADX
        CMP ADY
        BCS @x
        LDA CNT
        STA ymdf_inc
        JMP ymdf
@up:    JSR neg
        STA ADY
        LDY FASTY
        BNE @fup
        LDA #<yup
        STA xm_ys+1
        LDA #>yup
        STA xm_ys+2
        LDA ADX
        CMP ADY
        BCS @x
        LDA CNT
        STA ymu_inc
        JMP ymu
@fup:   LDY MERGE
        BNE @mup
        LDA #<yup40
        STA xm_ys+1
        LDA #>yup40
        STA xm_ys+2
        LDA ADX
        CMP ADY
        BCS @x
        LDA CNT
        STA ymuf_inc
        JMP ymuf
@mdown: LDA #<ydn2      ; two places at once (MERGE)
        STA xm2_ys+1
        LDA #>ydn2
        STA xm2_ys+2
        LDA ADX
        CMP ADY
        BCS @mx
        LDA CNT
        STA ymd2_inc
        JMP ymd2
@mup:   LDA #<yup2
        STA xm2_ys+1
        LDA #>yup2
        STA xm2_ys+2
        LDA ADX
        CMP ADY
        BCS @mx
        LDA CNT
        STA ymu2_inc
        JMP ymu2
@mx:    LDA CNT
        STA xm2_inc
        JMP xm2
xmaj:   LDA ADX
        BEQ lnret
        STA CNT
        LSR A
        STA ERR
xm_lp:  LDY XCOL,X
        LDA XMASK,X
xm_op:  EOR (PTR),Y
        STA (PTR),Y
xm_inc: INX
        LDA ERR
        SEC
        SBC ADY
        BCS xm_ok
        ADC ADX
        STA ERR
xm_ys:  JSR ydown
        DEC CNT
        BNE xm_lp
        RTS
xm_ok:  STA ERR
        DEC CNT
        BNE xm_lp
lnret:  RTS

ymd:    LDA ADY
        STA CNT
        LSR A
        STA ERR
ymd_lp: LDY XCOL,X
        LDA XMASK,X
ymd_op: EOR (PTR),Y
        STA (PTR),Y
        LDY LY
        INY
        CPY #AREA_H
        BCC @in
        LDY #0
@in:    STY LY
        LDA YLO,Y
        STA PTR
        LDA YHI,Y
        STA PTR+1
        LDA ERR
        SEC
        SBC ADX
        BCS ymd_e
        ADC ADY
ymd_inc: INX
ymd_e:  STA ERR
        DEC CNT
        BNE ymd_lp
        RTS

ymu:    LDA ADY
        STA CNT
        LSR A
        STA ERR
ymu_lp: LDY XCOL,X
        LDA XMASK,X
ymu_op: EOR (PTR),Y
        STA (PTR),Y
        LDY LY
        DEY
        CPY #$FF
        BNE @in
        LDY #AREA_H-1
@in:    STY LY
        LDA YLO,Y
        STA PTR
        LDA YHI,Y
        STA PTR+1
        LDA ERR
        SEC
        SBC ADX
        BCS ymu_e
        ADC ADY
ymu_inc: INX
ymu_e:  STA ERR
        DEC CNT
        BNE ymu_lp
        RTS

; ymdf / ymuf: ymd and ymu clear of the arena's edges (FASTY): PTR steps by 40, LY is
; left behind. Always XOR: only the ship uses them.
ymdf:   LDA ADY
        STA CNT
        LSR A
        STA ERR
ymdf_lp: LDY XCOL,X
        LDA XMASK,X
        EOR (PTR),Y
        STA (PTR),Y
        LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @e
        INC PTR+1
@e:     LDA ERR
        SEC
        SBC ADX
        BCS ymdf_e
        ADC ADY
ymdf_inc: INX
ymdf_e: STA ERR
        DEC CNT
        BNE ymdf_lp
        RTS

ymuf:   LDA ADY
        STA CNT
        LSR A
        STA ERR
ymuf_lp: LDY XCOL,X
        LDA XMASK,X
        EOR (PTR),Y
        STA (PTR),Y
        LDA PTR
        SEC
        SBC #40
        STA PTR
        BCS @e
        DEC PTR+1
@e:     LDA ERR
        SEC
        SBC ADX
        BCS ymuf_e
        ADC ADY
ymuf_inc: INX
ymuf_e: STA ERR
        DEC CNT
        BNE ymuf_lp
        RTS

; xm2 / ymd2 / ymu2: xmaj, ymdf and ymuf XORing twice a pixel: at PTR, X, and at PTR2,
; X + DX (patched into xm2_c, xm2_m and the rest by shipmove). For the ship (MERGE).
xm2:    LDA ADX
        BEQ xm2_r
        STA CNT
        LSR A
        STA ERR
xm2_lp: LDY XCOL,X
        LDA XMASK,X
        EOR (PTR),Y
        STA (PTR),Y
xm2_c:  LDY XCOL,X
xm2_m:  LDA XMASK,X
        EOR (PTR2),Y
        STA (PTR2),Y
xm2_inc: INX
        LDA ERR
        SEC
        SBC ADY
        BCS xm2_ok
        ADC ADX
        STA ERR
xm2_ys: JSR ydn2
        DEC CNT
        BNE xm2_lp
xm2_r:  RTS
xm2_ok: STA ERR
        DEC CNT
        BNE xm2_lp
        RTS

ymd2:   LDA ADY
        STA CNT
        LSR A
        STA ERR
ymd2_lp: LDY XCOL,X
        LDA XMASK,X
        EOR (PTR),Y
        STA (PTR),Y
ymd2_c: LDY XCOL,X
ymd2_m: LDA XMASK,X
        EOR (PTR2),Y
        STA (PTR2),Y
        LDA PTR         ; both a line down (ydn2, in line)
        CLC
        ADC #40
        STA PTR
        BCC @d1
        INC PTR+1
@d1:    LDA PTR2
        CLC
        ADC #40
        STA PTR2
        BCC @d2
        INC PTR2+1
@d2:    LDA ERR
        SEC
        SBC ADX
        BCS ymd2_e
        ADC ADY
ymd2_inc: INX
ymd2_e: STA ERR
        DEC CNT
        BNE ymd2_lp
        RTS

ymu2:   LDA ADY
        STA CNT
        LSR A
        STA ERR
ymu2_lp: LDY XCOL,X
        LDA XMASK,X
        EOR (PTR),Y
        STA (PTR),Y
ymu2_c: LDY XCOL,X
ymu2_m: LDA XMASK,X
        EOR (PTR2),Y
        STA (PTR2),Y
        LDA PTR         ; both a line up (yup2, in line)
        SEC
        SBC #40
        STA PTR
        BCS @u1
        DEC PTR+1
@u1:    LDA PTR2
        SEC
        SBC #40
        STA PTR2
        BCS @u2
        DEC PTR2+1
@u2:    LDA ERR
        SEC
        SBC ADX
        BCS ymu2_e
        ADC ADY
ymu2_inc: INX
ymu2_e: STA ERR
        DEC CNT
        BNE ymu2_lp
        RTS

; ydn2 / yup2: PTR and PTR2 one line down / up, no wrapping. Keep X.
ydn2:   LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @a
        INC PTR+1
@a:     LDA PTR2
        CLC
        ADC #40
        STA PTR2
        BCC @b
        INC PTR2+1
@b:     RTS
yup2:   LDA PTR
        SEC
        SBC #40
        STA PTR
        BCS @a
        DEC PTR+1
@a:     LDA PTR2
        SEC
        SBC #40
        STA PTR2
        BCS @b
        DEC PTR2+1
@b:     RTS

; yup40: PTR one line up, no wrapping. Keeps X.
yup40:  LDA PTR
        SEC
        SBC #40
        STA PTR
        BCS @r
        DEC PTR+1
@r:     RTS

; ydown / yup: LY one line down / up, wrapping, and PTR to its start. setline: LY = Y.
; The rocks step down their lines with ydown too. Keep X.
ydown:  LDY LY
        INY
        CPY #AREA_H
        BCC setline
        LDY #0
setline: STY LY
        LDA YLO,Y
        STA PTR
        LDA YHI,Y
        STA PTR+1
        RTS
yup:    LDY LY
        DEY
        CPY #$FF
        BNE setline
        LDY #AREA_H-1
        BNE setline

; plot: XOR (or OR) the pixel at arena X, Y. Uses A, Y.
plot:   LDA YLO,Y
        STA PTR
        LDA YHI,Y
        STA PTR+1
        LDY XCOL,X
        LDA XMASK,X
plotop: EOR (PTR),Y
        STA (PTR),Y
        RTS

; inkor / inkxor: patch the drawing loops to OR (for the static scenery) or XOR.
inkor:  LDA #$56        ; ORA (fp),Y
        BNE ink
inkxor: LDA #$5E        ; EOR (fp),Y
ink:    STA plotop
        STA xm_op
        STA ymd_op
        STA ymu_op
        RTS

; gentab: the plotting tables. Arena x -> byte column and bit; y -> line address.
gentab: LDX #0
@x:     TXA
        LSR A
        LSR A
        LSR A
        CLC
        ADC #4          ; the arena starts 32 pixels in
        STA XCOL,X
        TXA
        AND #7
        TAY
        LDA BITS,Y
        STA XMASK,X
        INX
        BNE @x
        LDX #7          ; the 8 entries past each end are the other end's
@ends:  LDA XCOL+248,X
        STA XCOL-8,X
        LDA XCOL,X
        STA XCOL+256,X
        LDA XMASK,X     ; the bits repeat every 8
        STA XMASK-8,X
        STA XMASK+256,X
        DEX
        BPL @ends
        LDA #<BMP
        STA T0
        LDA #>BMP
        STA T1
        INX
@y:     LDA T0
        STA YLO,X
        LDA T1
        STA YHI,X
        LDA T0
        CLC
        ADC #40
        STA T0
        BCC @same
        INC T1
@same:  INX
        CPX #AREA_H
        BNE @y
        RTS
BITS:   .rept 8, b
        .byte $80 >> b
        .endr

; clrbmp: clear the visible 168 lines, $2000-$3A3F: 26 pages, then 64 bytes.
clrbmp: LDA #0
        TAY
@page:
        .rept 26, p
        STA BMP + p*256,Y
        .endr
        INY
        BNE @page
        LDY #$3F
@rest:  STA BMP + 26*256,Y
        DEY
        BPL @rest
        RTS

; drawframe: dotted walls just outside the arena, screen x 31 and 288.
drawframe: LDX #0
@line:  TXA
        AND #1
        BNE @next
        LDA YLO,X
        STA PTR
        LDA YHI,X
        STA PTR+1
        LDY #3
        LDA #$01
        STA (PTR),Y
        LDY #36
        LDA #$80
        STA (PTR),Y
@next:  INX
        CPX #AREA_H
        BNE @line
        RTS

stars:  LDA #NSTARS
        STA T4
@star:  JSR RND
        TAX
        JSR RND
        AND #$7F
        CLC
        ADC #20
        TAY
        JSR plot
        DEC T4
        BNE @star
        RTS

; drawplanet: a filled disc of radius 10 at (128, 84), then three dark craters.
drawplanet: JSR inkor
        LDX #0
@row:   STX PIDX
        LDA PLW,X
        STA T0
        LDA #128
        SEC
        SBC T0
        STA LX
        TXA
        CLC
        ADC #84-10
        STA LY
        LDA T0
        ASL A
        CLC
        ADC #1
        STA DLX
        LDA #0
        STA DLY
        JSR line
        LDX PIDX
        INX
        CPX #21
        BNE @row
        JSR inkxor
        LDX #0
@crater: STX PIDX
        LDA CRATER+1,X
        TAY
        LDA CRATER,X
        TAX
        JSR plot
        LDX PIDX
        INX
        INX
        CPX #CRATEND-CRATER
        BNE @crater
        RTS
CRATER: .byte 123,80, 124,80, 123,81, 124,81, 131,86, 132,86, 132,87, 126,89, 135,80
CRATEND:
; planet: half-width of each line, dy = -10..10
PLW:    .byte 3,5,6,7,8,9,9,10,10,10,10,10,10,10,9,9,8,7,6,5,3

; drawlogo: the title, drawn twice one pixel apart for bold strokes.
drawlogo: JSR inkor
        LDA #0
        STA T5
@pass:  LDX #0
@stroke: LDA LOGO,X
        BEQ @again
        STA CNT
        LDA LOGO+1,X
        CLC
        ADC T5
        STA T0
        LDA LOGO+2,X
        STA T1
        INX
        INX
        INX
@seg:   DEC CNT
        BEQ @end
        STX PIDX
        LDA CNT
        PHA             ; line uses CNT
        LDA T0
        STA LX
        LDA T1
        STA LY
        LDA LOGO,X
        CLC
        ADC T5
        STA T2
        SEC
        SBC T0
        STA DLX
        LDA LOGO+1,X
        STA T3
        SEC
        SBC T1
        STA DLY
        JSR line
        PLA
        STA CNT
        LDA T2
        STA T0
        LDA T3
        STA T1
        LDX PIDX
        INX
        INX
        JMP @seg
@end:   STX PIDX
        LDX T0
        LDY T1
        JSR plot
        LDX PIDX
        JMP @stroke
@again: LDA T5
        BNE @done
        INC T5
        JMP @pass
@done:  JMP inkxor
; the title logo: strokes of (count, x, y, x, y...), 0 ends; arena coordinates
LOGO:   .byte 9,59,14,69,14,75,20,75,34,69,40,59,40,53,34,53,20,59,14,7,85,40,85,14
        .byte 101,14,107,19,107,24,101,28,85,28,2,95,28,107,40,7,117,14,117,40,133,40,139,35
        .byte 139,31,134,27,117,27,5,134,27,138,23,138,18,134,14,117,14,2,149,14,171,14,2,160
        .byte 14,160,40,2,149,40,171,40,2,181,14,203,14,2,192,14,192,40,0

; ---------------------------------------------------------------- generated tables
; The assembler works these out from sin(), cos() and sqrt() (chapter 6). Angles are in
; 256ths of a turn, so direction d (0-31, 1/32 turn each) is angle 8 * d.
; ---------------------------------------------------------------- pasted tables
SHIPV0L = 226
SHIPV0H = 0             ; spawn vx 0.884 px/frame: a circular orbit at r=40

; ---------------------------------------------------------------- the rocks
; The rocks as pictures, one constant per line of pixels, top to bottom. The leftmost
; bit is 8 (big) or 4 (small) pixels left of the rock's centre.
RB0     = %00000001110000000
RB1     = %00000110001110000
RB2     = %00011000000010000
RB3     = %01100000000001000
RB4     = %10000000000001000
RB5     = %01000000000000110
RB6     = %01000000000000001
RB7     = %00100000000000001
RB8     = %01000000000000001
RB9     = %01000000000000010
RB10    = %10000000000000010
RB11    = %10000000000000010
RB12    = %01000000000000100
RB13    = %00100000000000100
RB14    = %00100001100001000
RB15    = %00010110011001000
RB16    = %00001000000110000

RS0     = %000111110
RS1     = %001000010
RS2     = %001000010
RS3     = %010000001
RS4     = %010000001
RS5     = %100000001
RS6     = %110000010
RS7     = %001110010
RS8     = %000001100

; RKBIG / RKSMALL: XOR the rock at (CX, CY), compiled from the pictures by .rept: four
; instructions per lit pixel and no loop. X stays CX throughout, because the x tables run
; 8 entries past each end. RB\r pastes the line number onto RB to pick line r's picture.
; Clobber A, X, Y, PTR, LY.
RKBIG:  LDA #8
        JSR rocktop
        .rept 17, r
        .rept r > 0
        JSR rstep
        .endr
        .rept 17, c
        .rept RB\r >> [16 - c] & 1
        LDY XCOL-8+c,X
        LDA XMASK-8+c,X
        EOR (PTR),Y
        STA (PTR),Y
        .endr
        .endr
        .endr
        RTS

RKSMALL: LDA #4
        JSR rocktop
        .rept 9, r
        .rept r > 0
        JSR rstep
        .endr
        .rept 9, c
        .rept RS\r >> [8 - c] & 1
        LDY XCOL-4+c,X
        LDA XMASK-4+c,X
        EOR (PTR),Y
        STA (PTR),Y
        .endr
        .endr
        .endr
        RTS

codeend:
        .assert codeend <= BMP, "the program runs into the bitmap at $2000"

; ---------------------------------------------------------------- loaded past the code
; The gravity table is loaded into the bitmap, just past the code, and start copies it up
; into the hidden lines (GTAB) before the bitmap is cleared: so the code has its room.
        .org BMP
; GTAB: gravity, the pull along one axis in cell (i, j) of the quarter plane, 4 pixels a
; side, in 1/256 px/frame^2: row i, column j, i the cell along the pull. It's
; 8000 / r^2 * (4i + 1.5) / r, the inverse square, with r = max(9, the distance to the
; cell's centre). In whole numbers, with u = 8i + 3 and w = 8j + 3 (twice the centre's
; offsets) and s^2 = max(u^2 + w^2, 18^2): 32000 * u / s^3, rounded. The sqrt is of
; s^2 * 2^20, so it's s to 10 binary places, which is exact enough for every cell.
GTABLD:                 ; (loaded here, past the code; start copies it to GTAB)
        .rept 22, i
        .rept 22, j
        .byte [[32000 * [8*i + 3] << 11] / [max([8*i+3]*[8*i+3] + [8*j+3]*[8*j+3], 324) * sqrt(max([8*i+3]*[8*i+3] + [8*j+3]*[8*j+3], 324) << 20)] + 1] / 2
        .endr
        .endr

; SVXn / SVYn: the ship's vertices for all 32 directions, x then y: 0 the nose, 1 the rear
; left, 2 the notch, 3 the rear right, 4 the flame's tip. Each is a radius (in 1/1000
; pixel) at an angle from the nose, rounded half away from zero, so that a quarter turn
; is exact: 7.5 at the nose, 6 at 100/256 of a turn (140.6 degrees) either side, 2.2 for
; the notch and 6.5 for the flame, straight behind.
SVR0    = 7500
SVA0    = 0
SVR1    = 6000
SVA1    = 100
SVR2    = 2200
SVA2    = 128
SVR3    = 6000
SVA3    = -100
SVR4    = 6500
SVA4    = 128
; ---------------------------------------------------------------- moved tables
; Loaded past the code, after the gravity table, and copied at start to MOVRUN, in the
; text rows under the arena, where the code finds them: each table's address there is its
; label here + MOVED. Room for the code.
MOVLD:
        .irp v, 0, 1, 2, 3, 4
SVXL\v:
        .rept 32, d
        .byte [cos(8*d + SVA\v, SVR\v) + [cos(8*d + SVA\v, SVR\v) >= 0] * 1000 - 500] / 1000
        .endr
SVYL\v:
        .rept 32, d
        .byte [sin(8*d + SVA\v, SVR\v) + [sin(8*d + SVA\v, SVR\v) >= 0] * 1000 - 500] / 1000
        .endr
        .endr
SVX0    = SVXL0 + MOVED
SVY0    = SVYL0 + MOVED
SVX1    = SVXL1 + MOVED
SVY1    = SVYL1 + MOVED
SVX2    = SVXL2 + MOVED
SVY2    = SVYL2 + MOVED
SVX3    = SVXL3 + MOVED
SVY3    = SVYL3 + MOVED
SVX4    = SVXL4 + MOVED
SVY4    = SVYL4 + MOVED

; THR, BVL/BVH, RINGX: R * sin for directions 0-39, so the same table 8 entries on is
; R * cos: THRX = THR+8 is the x part, THRY = THR the y part.
; THR: thrust, 11/256 px/frame^2.
THRL:
        .rept 40, d
        .byte sin(8*d, 11)
        .endr
; BVL / BVH: bullet speed, 2.5 px/frame in 8.8.
BVLL:
        .rept 40, d
        .byte <sin(8*d, 640)
        .endr
BVHL:
        .rept 40, d
        .byte >sin(8*d, 640)
        .endr
; RINGX / RINGY: the rocks' spawn ring, 64 across and 61 down.
RINGXL:
        .rept 40, d
        .byte sin(8*d, 64)
        .endr
RINGYL:
        .rept 32, d
        .byte sin(8*d, 61)
        .endr
THR     = THRL + MOVED
BVL     = BVLL + MOVED
BVH     = BVHL + MOVED
RINGX   = RINGXL + MOVED
RINGY   = RINGYL + MOVED
THRX    = THR+8
THRY    = THR
BVXL    = BVL+8
BVXH    = BVH+8
BVYL    = BVL
BVYH    = BVH
SPX     = RINGX+8
SPY     = RINGY


; GROWL / GROWH: the address of each GTAB row (there's no multiply instruction).
GROWLL:
        .rept 22, row
        .byte <GTAB + 22*row
        .endr
GROWHL:
        .rept 22, row
        .byte >GTAB + 22*row
        .endr

GROWL   = GROWLL + MOVED
GROWH   = GROWHL + MOVED
; ORB: the circular-orbit speed sqrt(g r) at each spawn point, along the tangent, where g
; is the pull GTAB gives in that point's cell. It still comes from a script: the assembler
; can't look a value up in GTAB (it has no arrays), so it can't find g.
; circular-orbit velocity X at the spawn ring, 8.8
ORBXLL: .byte 0,221,183,151,123,99,83,82,79,82,83,99,123,151,183,221
        .byte 0,35,73,105,133,157,173,174,177,174,173,157,133,105,73,35
ORBXHL: .byte 0,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255
        .byte 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
; circular-orbit velocity Y at the spawn ring, 8.8
ORBYLL: .byte 181,178,176,158,133,105,71,35,0,221,185,151,123,98,80,78
        .byte 75,78,80,98,123,151,185,221,0,35,71,105,133,158,176,178
ORBYHL: .byte 0,0,0,0,0,0,0,0,0,255,255,255,255,255,255,255
        .byte 255,255,255,255,255,255,255,255,0,0,0,0,0,0,0,0
ORBXL   = ORBXLL + MOVED
ORBXH   = ORBXHL + MOVED
ORBYL   = ORBYLL + MOVED
ORBYH   = ORBYHL + MOVED
MOVEND:
MOVN    = [MOVEND - MOVLD + 2] / 3  ; copied in three runs of MOVN
        .assert MOVRUN + 3*MOVN <= ROW21, "the moved tables run into the HUD's rows"
