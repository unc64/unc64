; DEBUG: a formation shooter for the UNC 16, in bitmap mode.
; It's 1984. The build is full of bugs. You are the cursor.
;
; Bugs march in a 5 x 8 formation, step down at each edge and speed up as
; they thin out. They drop exceptions ("!"). A shot bug sometimes drops a
; patch ("+"): catch it for a double shot. A bug that reaches your row
; crashes the program.
;
; Controls: A / D or the cursor keys move. A tap moves one column; a held key
; keeps moving, twice as fast after 16 frames, and stops when it comes up.
; SPACE fires (held, it keeps firing, even while you steer) and starts. P
; toggles the border profiler: a green band for the frame's work, then a line
; of yellow for the sound's.
;
; Sounds, on the beeper's timer: a zap for your shot, a crunch and a blip for a
; squashed bug (higher for the rows worth more), a heartbeat of four falling
; notes for the march, a rumble and a wail when an exception hits you, a chord
; for a patch, a fanfare for a wave cleared, and a sad trombone for game over.
;
; What it shows off, with the manual's chapters:
;   the keyboard matrix, to move and fire at once (10)
;   the shifter, to draw every sprite at any x from one copy of it (7)
;   the beeper's timer, for sound effects that cost nothing while they play (11)
;   NOISE and RNDSEED, for a different game every time (11, 12)
;   a score kept as decimal digits, and NUMSTR and PRNUM for the rest (12)
;
; Screen: VMODE = 7, bitmap + mixed, 256 glyphs. The playfield is bitmap lines 0-167,
; the HUD is text rows 21-24 underneath. The bitmap stays at $2000.
;
; Memory map (UNC 16):
;   $0010-$0095  game variables (fast page, .res; the ROM's $D0-$FE untouched)
;   $0400-$07E7  text screen; rows 21-24 are the HUD
;   $0800-       code, then data: texts, sound effects, sprites (progend ends it)
;   $2000-$3AFF  the bitmap's 168 shown lines (and the rest of their last page)
;   $3B00-$3CA7  bitmap line addresses, built at start-up in the bitmap's
;                hidden bottom lines: RAM that mixed mode doesn't show
;
; X positions are in 2-pixel units (0-159), so they fit a byte. Y is in pixels.
;
; State for tests and debuggers, all in the fast page (unc-asm --sym lists
; the addresses):
;   STATE   0 title, 1 playing, 2 hit pause, 3 game over, 4 wave pause
;   CX      cannon x (2-px units, 4-151)
;   LIVES, WAVE, COUNT (bugs alive)
;   ALIVE   5 bytes, top row first; bit c = column c alive
;   FXB     formation x of column 0, 2-px units, plus 64
;   FY      formation top y; DIR 0 right, 1 left
;   SCORE   6 decimal digits, most significant first; HISC the same
;   SACT/SX/SY  2 shot slots; SOFF pixels from 2 * SX to the shot's bar (3
;           single, 0 and 6 double), SHIT which 2-px unit of the shot hits
;   BACT/BX/BY  3 exception slots;  PACT/PX/PY  the patch
;   DBL     double-shot time left (counts down every other frame)
;   ROWX    5 bytes: x each row is drawn at (biased like FXB)
;   ROWY    5 bytes: y each row is drawn at
;   REASON  why the game ended: 1 crashed, 2 out of lives
;   NOBOMB  test hook: not 0 = bugs drop no exceptions
;   BTIM    frames to the next exception
;   LATE    frames dropped since the game started (the main loop counts them)
;   PROF    profiler on
;   RATE    march speed: row redraws a frame x 64 (one row a frame = 64, max 128)
;   BEAT    march cycles so far: a heartbeat note every 4
;   SNDT    frames left of the sound step playing (0: quiet); SNDPRI its priority;
;           SNDP the next step
;   Labels fbegin and fdone bracket one frame's work, for timing it.

VMODE   = $C000
FRAME   = $C006
KBDSTB  = $C011
BORDER  = $C020
BGCOL   = $C021
BITMAP  = $C023
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 speaker data (chapter 12)
TIMER_LO = $C032        ; the timer's divisor: 1,000,000 / pitch
TIMER_HI = $C033        ; writing the high byte loads it
NOISE   = $C034         ; a random byte, from the cycle it's read on
SHIFT_IN = $C050        ; the shifter (chapter 8): push a byte into the window
SHIFT_AMT = $C051       ; how far right to move it: x AND 7
SHIFT_OUT = $C052       ; the window, moved: the next byte for the bitmap
KEYS    = $C060         ; the keyboard matrix: rows 0-8, a bit per key, 1 while down
GLYPHS  = $D000
RND     = $FF12         ; ROM: A = a random byte
NUMSTR  = $FF18         ; ROM: X/Y/A = a number; its 8 digits into NUMBUF
PRNUM   = $FF1B         ; ROM: print NUMSTR's digits, A digits wide
RNDSEED = $FF1E         ; ROM: X/Y = RND's new seed
CURX    = $0200         ; the ROM's cursor, where PRNUM prints
CURY    = $0201

; Where the keys are in the matrix (chapter 11): a key with code $20-$5F is in
; row (code - $20) / 8, bit code AND 7. Row 8 has LEFT in bit 0, RIGHT in bit 1.
MAT_SP  = KEYS + [' ' - $20] / 8        ; row 0
BIT_SP  = 1 << [' ' & 7]                ; bit 0
MAT_A   = KEYS + ['A' - $20] / 8        ; row 4
BIT_A   = 1 << ['A' & 7]                ; bit 1
MAT_D   = KEYS + ['D' - $20] / 8        ; row 4
BIT_D   = 1 << ['D' & 7]                ; bit 4
MAT_CUR = KEYS + 8

BM      = $2000         ; the bitmap
LINES   = 168           ; bitmap lines shown above the text in mixed mode
CLRPAGES = 27           ; whole pages clrbm clears: the shown lines and a little more
TROW21  = $0748         ; text rows shown under the bitmap in mixed mode
TROW23  = $0798
TROW24  = $07C0
HUDSC   = TROW21+6      ; where the score's six digits show
HUDHI   = TROW21+17     ; and the high score's

CY      = 156           ; cannon top line
GROUND  = 165
SSPD    = 6             ; shot speed, pixels a frame
TAP     = 8             ; frames a key press moves the cannon: one column
DBLT    = 200           ; double shot: 200 x 2 frames = 8 s
LIVES0  = 3             ; lives at the start
KLEFT   = 28            ; cursor keys
KRIGHT  = 29
ARROWL  = $96           ; screen codes: arrows (UNSCII's second half: 256-glyph mode)
ARROWR  = $98
PROFCOL = 5             ; the profiler's colours: green for the game,
SNDCOL  = 7             ; yellow for the sound

; ---- fast page: reserved, not stored; the start-up clears it ----
        .org $10
VARS:
STATE:  .res 1
CX:     .res 1
LIVES:  .res 1
WAVE:   .res 1
COUNT:  .res 1
ALIVE:  .res 5
FXB:    .res 1
FY:     .res 1
DIR:    .res 1
CURROW: .res 1
ANIM:   .res 1
MACC:   .res 1
SCORE:  .res 6
HISC:   .res 6
SACT:   .res 2
SX:     .res 2
SY:     .res 2
SOFF:   .res 2          ; pixels from 2 * SX to the shot's bar
SHIT:   .res 2          ; unit offset that does the hitting
BACT:   .res 3
BX:     .res 3
BY:     .res 3
PACT:   .res 1
PX:     .res 1
PY:     .res 1
DBL:    .res 1
ROWX:   .res 5
ROWY:   .res 5
CMIN:   .res 1
CMAX:   .res 1
TIMER:  .res 1
MDIR:   .res 1          ; 1 right, $FF left
MT:     .res 1          ; frames left to move
MRUN:   .res 1          ; frames moved without stopping
HDIR:   .res 1          ; the way the held keys steer: $FF, 0 or 1
FIREB:  .res 1
BTIM:   .res 1
REASON: .res 1
NOBOMB: .res 1
FLASH:  .res 1
FLCOL:  .res 1
LASTF:  .res 1
LATE:   .res 1
PROF:   .res 1
KEY:    .res 1          ; this frame's new press, or 0
JV:     .res 2
EVHIT:  .res 1
EVCRASH: .res 1
TANIM:  .res 1
DMODE:  .res 1
CMASK:  .res 1
SLOT:   .res 1
KROW:   .res 1
KCOL:   .res 1
KHX:    .res 1
RATE:   .res 1
BEAT:   .res 1
SNDT:   .res 1          ; the sound: frames left of this step, 0 when quiet
SNDP:   .res 2          ; the next step
SNDPRI: .res 1          ; the effect's priority, 0 when quiet
SNDN:   .res 1          ; a noise step's mask, 0 for a tone
SNDB:   .res 1          ; a noise step's base
SPTR:   .res 2
PTR:    .res 2
SRC:    .res 2
GPTR:   .res 2
T0:     .res 1
T1:     .res 1
T2:     .res 1
T3:     .res 1
T4:     .res 1
T5:     .res 1
T6:     .res 1
T7:     .res 1
DRR:    .res 1
DNY:    .res 1
DIDX:   .res 1
DBX:    .res 1
DNC:    .res 1
DMSK:   .res 1
DW:     .res 1
TL:     .res 1
TR:     .res 1
SPX:    .res 1
SPO:    .res 1
SPY:    .res 1
SPS:    .res 1
SPN:    .res 1
BCOL:   .res 1
BLINE:  .res 1
SCALE:  .res 1
EXP:    .res 4
VEND:
        .assert VEND <= $D0, "the variables run into the ROM's fast page"

        .org $0800
; The loader starts us with SP = $FF, interrupts off and A = X = Y = 0.
start:  LDX #VEND-VARS
@clr:   STA VARS-1,X    ; every variable 0
        DEX
        BNE @clr
        STX BORDER      ; black
        STX BGCOL
        LDA #<BM        ; the bitmap line addresses: line y starts at BM + 40y
        STA LTLO
        LDA #>BM
        STA LTHI
@line:  LDA LTLO,X      ; X = 0 from the loop above
        CLC
        ADC #40
        STA LTLO+1,X
        LDA LTHI,X
        ADC #0
        STA LTHI+1,X
        INX
        CPX #LINES-1
        BNE @line
        LDA #>BM
        STA BITMAP
        LDA #7          ; bitmap + mixed, 256 glyphs (for the arrows)
        STA VMODE
        JSR title
        LDA FRAME
        STA LASTF

; ---------------------------------------------------------------- main loop
main:   LDA LASTF
@wait:  CMP FRAME
        BEQ @wait
        LDA FRAME
        TAX
        SEC
        SBC LASTF
        STX LASTF
        CMP #2
        BCC fbegin
        INC LATE        ; the last frame's work overran
fbegin: LDA PROF
        BEQ @keys
        LDA #PROFCOL    ; profiler: the frame's work in green...
        STA BORDER
@keys:  LDA KBDSTB      ; a new press, taken
        BMI @new
        LDA #0
@new:   AND #$7F
        STA KEY
        CMP #'P'
        BNE @run
        LDA PROF
        EOR #1
        STA PROF
@run:   LDA STATE
        ASL A
        TAX
        LDA states,X
        STA JV
        LDA states+1,X
        STA JV+1
        JSR jumpv
        LDA PROF
        BEQ @sound
        LDA #SNDCOL     ; ...then the sound's share in yellow
        STA BORDER
@sound: JSR sound       ; once a frame, after everything that may start a sound
        JSR border
fdone:  JMP main
jumpv:  JMP (JV)

states: .word st_title, st_play, st_hit, st_over, st_wave

; P shows where the frame goes. i never trusted a game i couldn't profile.
;
border: LDA PROF
        BEQ @flash
        LDA #0          ; profiler: the band ends where the work ends
        STA BORDER
        RTS
@flash: LDA FLASH
        BEQ @black
        DEC FLASH
        LDA FLCOL
        STA BORDER
        RTS
@black: LDA #0
        STA BORDER
        RTS

; ---------------------------------------------------------------- states
st_title: LDA FRAME
        AND #15
        BNE @key
        LDA TANIM
        EOR #1
        STA TANIM
        JSR tbugs
@key:   LDA KEY
        CMP #' '
        BNE @done
        JMP newgame
@done:  RTS

st_play: JSR input
        LDA #0
        STA DMODE
        JSR movers      ; rub out shots, exceptions, patch
        JSR march
        LDA #$FF
        STA CMASK
        JSR cannon
        JSR fire
        JSR mshots
        JSR mbombs
        JSR mpatch
        JSR spawnb
        LDA #1
        STA DMODE
        JSR movers      ; and draw them where they are now
        JSR timers
        LDA EVCRASH
        BEQ @hit
        LDA #1
        JMP gameover
@hit:   LDA EVHIT
        BEQ @clear
        JMP gohit
@clear: LDA COUNT
        BNE @done
        JMP goclear
@done:  RTS

st_hit: DEC TIMER
        BEQ @back
        LDA #2
        STA FLCOL
        LDA #1
        STA FLASH
        LDA TIMER       ; the cannon blinks
        AND #4
        BEQ @mask
        LDA #$FF
@mask:  STA CMASK
        JMP drawcan
@back:  LDA #1
        STA STATE
        LDA #$FF
        STA CMASK
        JSR drawcan
        LDX #<m_blank
        LDY #>m_blank
        JMP msg

st_over: LDA TIMER
        BEQ @key
        DEC TIMER
        RTS
@key:   LDA KEY
        CMP #' '
        BNE @done
        JMP newgame
@done:  RTS

st_wave: DEC TIMER
        BNE @done
        JSR newwave
        LDA #1
        STA STATE
        LDX #<m_wave
        LDY #>m_wave
        JMP msg
@done:  RTS

; ---------------------------------------------------------------- transitions
title:  LDA #0
        STA STATE
        JSR clrbm
        JSR clrtext
        LDX #<t_list
        LDY #>t_list
        JSR bmlist
        JSR tbugs
        LDX #<m_title
        LDY #>m_title
        JSR msg
        LDX #<m_help
        LDY #>m_help
        LDA #<TROW24
        STA PTR
        LDA #>TROW24
        STA PTR+1
        JMP puttxt2

; A new game. RND gets a new seed from NOISE, read just after SPACE was taken:
; the moment the player pressed it is never the same twice, so neither is the
; game (chapter 12).
newgame: LDX NOISE
        LDY NOISE
        JSR RNDSEED
        JSR hush        ; the game-over tune may still be playing
        LDA #0
        LDX #5
@score: STA SCORE,X
        DEX
        BPL @score
        .irp v, DBL, PACT, SACT, SACT+1, BACT, BACT+1, BACT+2, MDIR, MT, FIREB, REASON, FLASH
        STA v
        .endr
        LDA #LIVES0
        STA LIVES
        LDA #1
        STA WAVE
        LDA #77
        STA CX
        JSR clrbm       ; 34,600 cycles: starting a game takes 3 frames
        JSR clrtext
        JSR ground
        JSR hudlbl      ; score 0, LIVES0 lives and wave 1 are in the label
        JSR showhi
        JSR newwave
        LDA #$FF
        STA CMASK
        JSR drawcan
        LDX #<m_go
        LDY #>m_go
        JSR msg
        LDA #1
        STA STATE
        LDA FRAME       ; don't count the slow start as dropped frames
        STA LASTF
        LDA #0
        STA LATE
        RTS

newwave: LDA #$FF
        LDX #4
@alive: STA ALIVE,X
        DEX
        BPL @alive
        LDA #40
        STA COUNT
        LDA WAVE        ; FY = 16 + 6 * (min(WAVE, 7) - 1)
        CMP #7
        BCC @fy
        LDA #7
@fy:    SEC
        SBC #1
        STA T0
        ASL A
        ADC T0
        ASL A
        CLC
        ADC #16
        STA FY
        LDA #113        ; column 0 at x 49 (98 px): centred
        STA FXB
        LDA #0
        .irp v, DIR, ANIM, MACC, CMIN, EVHIT, EVCRASH
        STA v
        .endr
        LDA #7
        STA CMAX
        LDA #$FF
        STA CURROW
        LDA #40
        STA BTIM
        LDX #4
@row:   LDA FY
        CLC
        ADC rowoff,X
        STA ROWY,X
        LDA FXB
        STA ROWX,X
        STX T7
        JSR drawrow
        LDX T7
        DEX
        BPL @row
        RTS

gohit:  LDA #0
        STA EVHIT
        LDX #FX_HIT
        JSR sfx
        JSR killmov
        DEC LIVES
        JSR prlives
        LDA LIVES
        BNE @pause
        LDA #2
        JMP gameover
@pause: LDA #2
        STA STATE
        LDA #50
        STA TIMER
        LDA #0
        .irp v, MDIR, MT, FIREB, DBL
        STA v
        .endr
        LDX #<m_hit
        LDY #>m_hit
        JMP msg

goclear: LDX #FX_CLEAR
        JSR sfx
        JSR killmov
        INC WAVE
        JSR prwave
        LDA #4
        STA STATE
        LDA #60
        STA TIMER
        LDX #<m_clear
        LDY #>m_clear
        JMP msg

; A = reason: 1 a bug reached the cannon, 2 out of lives.
gameover: STA REASON
        LDX #FX_OVER    ; first: the big text below takes 3 frames
        JSR sfx
        LDA #3
        STA STATE
        LDA #50
        STA TIMER
        JSR killmov
        JSR hiscore
        LDA #64
        STA T0
        LDA #36
        STA T1
        JSR clrlines
        LDA REASON
        CMP #1
        BNE @fatal
        LDX #<t_crash
        LDY #>t_crash
        JSR bmlist
        LDX #<m_crash
        LDY #>m_crash
        JMP msg
@fatal: LDX #<t_fatal
        LDY #>t_fatal
        JSR bmlist
        LDX #<m_fatal
        LDY #>m_fatal
        JMP msg

; ---------------------------------------------------------------- input and cannon
; Steering reads the keyboard matrix (chapter 11), which shows every key that's
; down at once, so the cannon keeps moving while SPACE fires. A or LEFT held
; steers left, D or RIGHT right, and both at once cancel out. A tap quicker than
; a frame never shows in the matrix, so a fresh press in KBD also starts a move
; of TAP frames: one column, however short the tap.
input:  LDA MAT_A
        AND #BIT_A
        BEQ @a
        LDA #1          ; A counts as LEFT...
@a:     ORA MAT_CUR     ; LEFT is bit 0 of row 8, RIGHT bit 1
        STA HDIR
        LDA MAT_D
        AND #BIT_D
        BEQ @d
        LDA #2          ; ...and D as RIGHT
@d:     ORA HDIR
        AND #%11        ; UP, DOWN and the rest of row 8 don't steer
        TAX
        LDA steer,X
        STA HDIR
        LDA KEY         ; a fresh press: a tap
        JSR keydir
        BEQ @held
        CMP MDIR
        BNE @tap
        LDX MRUN        ; a host's repeat of the way it's moving: no new tap
        BNE @held
@tap:   STA MDIR
        LDA #TAP
        STA MT
        LDA #0
        STA MRUN
@held:  LDA HDIR
        BEQ @fire
        CMP MDIR
        BEQ @keep
        STA MDIR        ; turned round
        LDA #0
        STA MRUN
@keep:  LDA MT          ; keep moving this frame
        BNE @fire
        INC MT
@fire:  LDA MAT_SP
        AND #BIT_SP
        BNE @shoot      ; SPACE held: keep firing
        LDA KEY
        CMP #' '        ; a tap too quick for the matrix fires too
        BNE @done
@shoot: LDA #8          ; fire, or fire as soon as the last shot is gone
        STA FIREB
@done:  RTS

; A = key code. Returns A = $FF for left (A or cursor left), 1 for right
; (D or cursor right), 0 (and Z set) for anything else.
keydir: CMP #'A'
        BEQ @left
        CMP #KLEFT
        BEQ @left
        CMP #'D'
        BEQ @right
        CMP #KRIGHT
        BEQ @right
        LDA #0
        RTS
@left:  LDA #$FF
        RTS
@right: LDA #1
        RTS

cannon: LDA MT
        BNE @move
        STA MRUN        ; stopped: the next move starts slow
        JMP drawcan
@move:  DEC MT
        INC MRUN
        BNE @speed
        DEC MRUN
@speed: LDA #1          ; 2 px a frame, 4 once it's been moving 16 frames
        LDX MRUN
        CPX #16
        BCC @step
        LDA #2
@step:  STA T0
        LDA CX
        LDX MDIR
        BMI @left
        CLC
        ADC T0
        CMP #152
        BCC @put
        LDA #151
        BCS @put
@left:  SEC
        SBC T0
        CMP #4
        BCS @put
        LDA #4
@put:   STA CX

; The cannon, through the shifter: 4 bytes a line, paper either side of its
; two, so it rubs itself out as it moves. CMASK $00 draws it blank.
drawcan: LDA CX
        ASL A           ; x = 2 * CX pixels: the shifter keeps x AND 7
        STA SHIFT_AMT
        LDA CX
        LSR A
        LSR A           ; x / 8
        CLC
        ADC #<[BM + CY*40 - 1]  ; PTR: one byte left of it, on its top line
        STA PTR
        LDA #>[BM + CY*40 - 1]
        ADC #0
        STA PTR+1
        LDX #0
@line:  LDY #0
        TYA
        STA (PTR),Y     ; paper to its left
        LDA spr_CANNON,X
        STA SHIFT_IN    ; the row into the window...
        LDA SHIFT_OUT   ; ...moved into place: its left byte
        AND CMASK
        INY
        STA (PTR),Y
        LDA #0
        STA SHIFT_IN    ; paper after it...
        LDA SHIFT_OUT   ; ...and out comes its right byte
        AND CMASK
        INY
        STA (PTR),Y
        INY
        LDA #0
        STA (PTR),Y     ; paper to its right
        LDA PTR         ; down a line
        CLC
        ADC #40
        STA PTR
        BCC @same
        INC PTR+1
@same:  INX
        CPX #8
        BNE @line
        RTS

fire:   LDA FIREB
        BEQ @done
        DEC FIREB
        LDA SACT
        ORA SACT+1
        BNE @done       ; one volley in the air at a time
        STA FIREB
        LDX #FX_SHOT
        JSR sfx
        LDA DBL
        BNE @double
        LDX #0
        LDA #3          ; one shot, from the middle of the cannon: 3 px in
        LDY #1
        JMP newshot
@done:  RTS
@double: LDX #0
        LDA #0          ; two, from its edges: 0 and 6 px in
        LDY #0
        JSR newshot
        LDX #1
        LDA #6
        LDY #3
newshot: STA SOFF,X
        STY SHIT,X
        LDA #1
        STA SACT,X
        LDA CX
        STA SX,X
        LDA #CY-4
        STA SY,X
        RTS

; ---------------------------------------------------------------- shots
mshots: LDX #1
@shot:  LDA SACT,X
        BEQ @next
        LDA SY,X
        SEC
        SBC #SSPD
        BCC @off
        STA SY,X
        STX SLOT
        JSR shothit
        LDX SLOT
@next:  DEX
        BPL @shot
        RTS
@off:   LDA #0
        STA SACT,X
        JMP @next

; Does shot SLOT overlap a live bug? Checks the rows as they're drawn.
shothit: LDA SX,X
        CLC
        ADC SHIT,X
        ADC #64
        STA T3
        LDA SY,X
        STA T4
        LDY #4
@row:   LDA ALIVE,Y
        BEQ @next
        LDA ROWY,Y
        CLC
        ADC #7
        CMP T4
        BCC @next       ; the row's bottom is above the shot
        LDA T4
        CLC
        ADC #3
        CMP ROWY,Y
        BCC @next       ; the shot's bottom is above the row
        LDA T3
        SEC
        SBC ROWX,Y
        BCC @next
        CMP #64
        BCS @next
        STA T5
        AND #7
        CMP #4
        BCS @next       ; between two bugs
        LDA T5
        LSR A
        LSR A
        LSR A
        TAX
        LDA ALIVE,Y
        AND bits,X
        BEQ @next
        STY KROW
        STX KCOL
        JSR kill
        LDX SLOT
        LDA #0
        STA SACT,X
        RTS
@next:  DEY
        BPL @row
        RTS

; squashing one should feel better than it does at work.
;
; Squash the bug at KROW, KCOL.
kill:   LDX KCOL
        LDA bits,X
        EOR #$FF
        LDY KROW
        AND ALIVE,Y
        STA ALIVE,Y
        DEC COUNT
        LDA KCOL
        ASL A
        ASL A
        ASL A
        CLC
        ADC ROWX,Y
        SEC
        SBC #64
        STA KHX
        LSR A
        LSR A
        STA T1
        LDA ROWY,Y
        STA T2
        LDA #8
        STA T6
@line:  LDY T2
        LDA LTLO,Y
        CLC
        ADC T1
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDY #0
        TYA
        STA (PTR),Y
        INY
        STA (PTR),Y
        INC T2
        DEC T6
        BNE @line
        LDY KROW
        LDA rowpts,Y
        JSR addscore
        LDA #2
        STA FLASH
        LDA #12
        STA FLCOL
        LDA KROW        ; the squash sound for its row: sq0 to sq4
        ASL A
        ADC #FX_SQUASH
        TAX
        JSR sfx
        LDA PACT
        BNE @done
        JSR RND
        CMP #40         ; about 1 in 6 drops a patch
        BCS @done
        LDA #1
        STA PACT
        LDA KHX
        STA PX
        LDY KROW
        LDA ROWY,Y
        CLC
        ADC #8
        STA PY
@done:  RTS

; Add A tens (1-9) to the score. It's kept as decimal digits, as in the manual's
; "Keeping score fast": the UNC-1 has no decimal mode, so a binary score would
; need NUMSTR's 2,000 cycles to show, in the middle of a busy frame. This way
; only the digits that change are written to the HUD: about 40 cycles, and 30
; more for each carry.
addscore: LDX #4        ; the tens digit
@digit: CLC
        ADC SCORE,X
        CMP #10
        BCC @last       ; under 10: the last digit to change
        SBC #10         ; CMP left the carry set
        STA SCORE,X
        ORA #'0'
        STA HUDSC,X
        LDA #1          ; carry one into the digit to the left
        DEX
        BPL @digit
        RTS             ; past 999990: round to 000000
@last:  STA SCORE,X
        ORA #'0'
        STA HUDSC,X
        RTS

; A new high score? Compare the digits from the highest down: the first pair
; that differs decides.
hiscore: LDX #0
@cmp:   LDA SCORE,X
        CMP HISC,X
        BCC showhi
        BNE @copy
        INX
        CPX #6
        BNE @cmp
        RTS
@copy:  LDX #5
@digit: LDA SCORE,X
        STA HISC,X
        DEX
        BPL @digit
showhi: LDX #5          ; show the high score's digits
@digit: LDA HISC,X
        ORA #'0'
        STA HUDHI,X
        DEX
        BPL @digit
        RTS

; The lives and the wave, which change now and then: NUMSTR turns the number
; into digits and PRNUM prints them, a fixed number of digits wide, at the
; ROM's cursor. About 900 cycles for a number this small.
prlives: LDA LIVES
        LDX #31         ; LIVES n: one digit
        LDY #1
        BNE hudnum
prwave: LDA WAVE
        CMP #100
        BCC @fit
        LDA #99         ; PRNUM never cuts a number short: keep it to 2 digits
@fit:   LDX #38         ; WAVE nn
        LDY #2
; A = the number, X = the column on HUD row 21, Y = digits, zeros in front.
hudnum: STX CURX
        STY T0
        LDY #21
        STY CURY
        TAX             ; NUMSTR: X low byte, Y middle, A high
        LDA #0
        TAY
        JSR NUMSTR
        LDA T0
        JMP PRNUM

; ---------------------------------------------------------------- exceptions and patch
mbombs: LDX #2
@bomb:  LDA BACT,X
        BEQ @next
        LDA BY,X
        CLC
        ADC #2
        STA BY,X
        CMP #159
        BCS @gone
        CMP #CY-5
        BCC @next
        LDA BX,X
        SEC
        SBC CX
        CLC
        ADC #2
        CMP #5
        BCS @next
        LDA #1          ; it hit the cannon
        STA EVHIT
@gone:  LDA #0
        STA BACT,X
@next:  DEX
        BPL @bomb
        RTS

; patches fall slower than exceptions. that part is true to life.
;
mpatch: LDA PACT
        BEQ @done
        INC PY
        LDA PY
        CMP #159
        BCS @gone
        CMP #CY-5
        BCC @done
        LDA PX
        SEC
        SBC CX
        CLC
        ADC #3
        CMP #7
        BCS @done
        LDA #DBLT       ; caught
        STA DBL
        LDA #0
        STA PACT
        LDX #FX_PATCH
        JSR sfx
        LDX #<m_patch
        LDY #>m_patch
        JMP msg
@gone:  LDA #0
        STA PACT
@done:  RTS

spawnb: LDA NOBOMB
        BNE @out
        DEC BTIM
        BNE @out
        JSR RND
        AND #15
        STA T0
        LDA WAVE        ; next one in 39 - 3 * min(WAVE, 9) + 0..15 frames
        CMP #9
        BCC @wave
        LDA #9
@wave:  STA T1
        ASL A
        ADC T1
        STA T1
        LDA #39
        SEC
        SBC T1
        CLC
        ADC T0
        STA BTIM
        LDX #2
@slot:  LDA BACT,X
        BEQ @free
        DEX
        BPL @slot
@out:   RTS
@free:  STX T2
        LDA ALIVE
        ORA ALIVE+1
        ORA ALIVE+2
        ORA ALIVE+3
        ORA ALIVE+4
        STA T3
        BEQ @done
        JSR RND
        AND #7
        TAX
@col:   LDA bits,X      ; the first column from a random one that has a bug
        AND T3
        BNE @found
        INX
        TXA
        AND #7
        TAX
        JMP @col
@found: STX T4
        LDY #4
@low:   LDA ALIVE,Y     ; its lowest bug drops it
        AND bits,X
        BNE @drop
        DEY
        JMP @low
@drop:  LDX T2
        LDA T4
        ASL A
        ASL A
        ASL A
        CLC
        ADC ROWX,Y
        SEC
        SBC #64
        STA BX,X
        LDA ROWY,Y
        CLC
        ADC #8
        STA BY,X
        LDA #1
        STA BACT,X
@done:  RTS

timers: LDA DBL
        BEQ @done
        LDA FRAME
        AND #1
        BNE @done
        DEC DBL
        BNE @done
        LDX #<m_blank
        LDY #>m_blank
        JMP msg
@done:  RTS

; Rub out everything that moves and forget it.
killmov: LDA #0
        STA DMODE
        JSR movers
        LDA #0
        .irp v, SACT, SACT+1, BACT, BACT+1, BACT+2, PACT
        STA v
        .endr
        RTS

; Draw (DMODE 1) or rub out (DMODE 0) the shots, exceptions and patch.
movers: LDX #1
@shot:  LDA SACT,X
        BEQ @nshot
        LDA SX,X
        STA SPX
        LDA SOFF,X
        STA SPO
        LDA SY,X
        STA SPY
        STX T7
        LDA #spr_SHOT-spr
        LDY #4
        JSR sdo
        LDX T7
@nshot: DEX
        BPL @shot
        LDA #0          ; exceptions and the patch sit at even x
        STA SPO
        LDX #2
@bomb:  LDA BACT,X
        BEQ @nbomb
        LDA BX,X
        STA SPX
        LDA BY,X
        STA SPY
        STX T7
        LDA #spr_BOMB-spr
        LDY #6
        JSR sdo
        LDX T7
@nbomb: DEX
        BPL @bomb
        LDA PACT
        BEQ @done
        LDA PX
        STA SPX
        LDA PY
        STA SPY
        LDA #spr_PATCH-spr
        LDY #6
        JMP sdo
@done:  RTS

; Draw (DMODE 1) or rub out (DMODE 0) a small sprite through the shifter:
; A = which (its offset in spr), Y = its lines (up to 6), at x = 2 * SPX + SPO
; pixels and line SPY. Drawing lights its pixels and leaves the rest; rubbing
; out turns them off.
sdo:    STA SPS
        STY SPN
        LDA SPX
        ASL A           ; x = 2 * SPX + SPO, low byte: the shifter needs only
        CLC             ; x AND 7
        ADC SPO
        STA SHIFT_AMT
        LDA SPO         ; x / 8 = (SPX + SPO / 2) / 4, rounding down: 2 * SPX
        LSR A           ; is even, so SPO's odd pixel can't carry x past a byte
        CLC
        ADC SPX
        LSR A
        LSR A
        LDY SPY
        CLC
        ADC LTLO,Y      ; PTR: the byte under the sprite's top-left corner
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDX SPS         ; X: the sprite's row
        LDY #0          ; Y: the line's offset from PTR, 40 bytes a line (201
        LDA DMODE       ; at most, for six lines: no pointer sums per line)
        BEQ sclr        ; A line costs 67 cycles to draw, wherever x is
sor:    LDA spr,X
        STA SHIFT_IN    ; the row into the shifter's window...
        LDA SHIFT_OUT   ; ...moved right by x AND 7: the part in the left byte
        ORA (PTR),Y
        STA (PTR),Y
        LDA #0
        STA SHIFT_IN    ; paper after it...
        LDA SHIFT_OUT   ; ...and out comes the part that spills into the next
        INY
        ORA (PTR),Y
        STA (PTR),Y
        TYA             ; down a line: 40 bytes on from the left byte
        CLC
        ADC #39
        TAY
        INX
        DEC SPN
        BNE sor
        RTS
sclr:   LDA spr,X       ; the same, but turning the sprite's pixels off
        STA SHIFT_IN
        LDA SHIFT_OUT
        EOR #$FF
        AND (PTR),Y
        STA (PTR),Y
        LDA #0
        STA SHIFT_IN
        LDA SHIFT_OUT
        EOR #$FF
        INY
        AND (PTR),Y
        STA (PTR),Y
        TYA
        CLC
        ADC #39
        TAY
        INX
        DEC SPN
        BNE sclr
        RTS

; every bug i ever fixed marches in this formation. some of them twice.
;
; ---------------------------------------------------------------- the formation
; The march: up to two row redraws a frame, bottom row first. A full pass over
; the live rows is a cycle; each cycle moves the formation 2 px or steps it down.
march:  LDA COUNT
        BEQ @done
        LDA WAVE        ; RATE = 50 + 6 * (min(WAVE, 9) - 1) + 2 * (40 - COUNT)
        CMP #9
        BCC @wave
        LDA #9
@wave:  SEC
        SBC #1
        STA T0
        ASL A
        ADC T0
        ASL A
        ADC #50
        STA RATE
        LDA #40
        SEC
        SBC COUNT
        ASL A
        CLC
        ADC RATE
        CMP #129
        BCC @acc
        LDA #128
@acc:   CLC
        ADC MACC
        STA MACC
@step:  LDA MACC
        CMP #64
        BCC @done
        SBC #64
        STA MACC
        JSR rowstep
        JMP @step
@done:  RTS

rowstep: LDX CURROW
@find:  BMI @cycle
        LDA ALIVE,X
        BNE @draw
        DEX
        JMP @find
@cycle: JSR newcycle
        LDX #4
        JMP @find
@draw:  STX DRR
        DEX
        STX CURROW
        LDX DRR
        JSR drawrow
        LDX DRR
        LDA ROWY,X
        CLC
        ADC #8
        CMP #CY+1
        BCC @done
        LDA #1          ; a bug reached the cannon's row
        STA EVCRASH
@done:  RTS

; A new cycle of the march. Every 4th, the heartbeat plays its next note, so it
; speeds up as the swarm does: about two a second at the start of wave 1, ten
; or more for the last few bugs.
newcycle: LDA ANIM
        EOR #1
        STA ANIM
        INC BEAT
        LDA BEAT
        AND #3
        BNE @edges
        LDA BEAT        ; note (BEAT / 4) AND 3, times 2 for the table
        LSR A           ; (C clear: BEAT AND 3 is 0)
        AND #%110
        ADC #FX_BEAT
        TAX
        JSR sfx
@edges: LDA ALIVE
        ORA ALIVE+1
        ORA ALIVE+2
        ORA ALIVE+3
        ORA ALIVE+4
        STA T0
        LDX #0
@min:   LSR A
        BCS @gotmin
        INX
        JMP @min
@gotmin: STX CMIN
        LDA T0
        LDX #7
@max:   ASL A
        BCS @gotmax
        DEX
        JMP @max
@gotmax: STX CMAX
        LDA DIR
        BNE @left
        LDA CMAX        ; moving right: the right-hand bug's x
        ASL A
        ASL A
        ASL A
        CLC
        ADC FXB
        SEC
        SBC #64
        CMP #151
        BCS @down
        INC FXB
        RTS
@left:  LDA CMIN        ; moving left: the left-hand bug's x
        ASL A
        ASL A
        ASL A
        CLC
        ADC FXB
        SEC
        SBC #64
        CMP #5
        BCC @down
        DEC FXB
        RTS
@down:  LDA FY
        CLC
        ADC #4
        STA FY
        LDA DIR
        EOR #1
        STA DIR
        RTS

; Redraw row X at FXB, FY + its offset. Writes whole bytes from one byte left of
; column CMIN to one byte right of column CMAX, so the old picture is overwritten,
; and rubs out the lines it stepped down from. Bugs are 16 px apart, two bytes
; each: the shifter makes a line's two bytes once and they're stored for every
; live bug in the row.
drawrow: STX DRR
        LDA FY
        CLC
        ADC rowoff,X
        STA DNY
        LDA rowspec,X   ; the sprite: species * 2 + ANIM, 8 bytes each
        ASL A
        ORA ANIM
        ASL A
        ASL A
        ASL A
        STA DIDX
        LDA FXB         ; x = 2 * (FXB - 64) + 16 * column pixels, and
        ASL A           ; x AND 7 = 2 * FXB AND 7: the shifter keeps the 7
        STA SHIFT_AMT
        LDA CMIN
        ASL A
        ASL A
        ASL A
        CLC
        ADC FXB
        SEC
        SBC #64
        LSR A
        LSR A
        SEC
        SBC #1
        STA DBX
        LDA CMAX
        SEC
        SBC CMIN
        CLC
        ADC #1
        STA DNC
        ASL A
        ADC #2
        STA DW
        LDA ALIVE,X
        LDY CMIN
@shift: CPY #0
        BEQ @mask
        LSR A
        DEY
        JMP @shift
@mask:  STA DMSK
        LDA ROWY,X
        CMP DNY
        BCS @draw
        STA T1
@rub:   LDY T1          ; rub out the lines it stepped down from
        LDA LTLO,Y
        CLC
        ADC DBX
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDA #0
        LDY DW
        DEY
@rubb:  STA (PTR),Y
        DEY
        BPL @rubb
        INC T1
        LDA T1
        CMP DNY
        BNE @rub
@draw:  LDX DRR
        LDA FXB
        STA ROWX,X
        LDA DNY
        STA ROWY,X
        STA T1
        LDA #8
        STA T2
@line:  LDY T1
        LDA LTLO,Y
        CLC
        ADC DBX
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDX DIDX
        LDA spr,X
        STA SHIFT_IN    ; this line of the bug into the window...
        LDA SHIFT_OUT   ; ...moved into place: its left byte
        STA TL
        LDA #0
        STA SHIFT_IN    ; paper after it, and out comes its right byte
        LDA SHIFT_OUT
        STA TR
        INC DIDX
        LDA DMSK
        STA T3
        LDY #0
        TYA
        STA (PTR),Y
        INY
        LDX DNC
@bug:   LSR T3
        BCC @gap
        LDA TL
        STA (PTR),Y
        INY
        LDA TR
        STA (PTR),Y
        INY
        DEX
        BNE @bug
        JMP @end
@gap:   LDA #0
        STA (PTR),Y
        INY
        STA (PTR),Y
        INY
        DEX
        BNE @bug
@end:   LDA #0
        STA (PTR),Y
        INC T1
        DEC T2
        BNE @line
        RTS

; ---------------------------------------------------------------- sound
; One voice, the beeper's timer (chapter 12). An effect is a list of steps, each
; held for some frames. `sound` runs once a frame and touches the timer only
; when a step changes, so a note costs nothing while it plays and the game never
; waits for one. Each effect has a priority: a new one starts only if nothing
; more important is playing. A squash cuts off a shot, a shot cuts off the
; march, and never the other way round.
;
; An effect is its priority, then steps of three bytes, then a 0:
;   frames, divisor low, divisor high   a tone of 1,000,000 / divisor Hz
;   frames, base, NOISY + mask          noise: a new random divisor every frame,
;                                       NOISE for the low byte and NOISE AND
;                                       mask OR base for the high byte
; A divisor of REST (1) holds the speaker still: a gap inside an effect. When
; the effect ends the gate closes: SPEAKER = 0, the timer stopped, silence.
;
; The sequencer steps at the end of each frame's work, so the profiler can show
; what it costs, and an effect started during the frame begins there: every
; step lasts exactly its frames.

; Step the sequencer: about 20 cycles when quiet, 30 while a note holds, 50 for
; a noise step, 100 when a step starts.
sound:  LDA SNDT        ; frames left of this step; 0: quiet
        BEQ @done
        DEC SNDT
        BEQ step        ; this step is over: start the next
        LDA SNDN        ; a noise step: a new pitch every frame
        BNE rattle
@done:  RTS
; Start the step at SNDP, and move SNDP on to the one after it.
step:   LDY #0
        LDA (SNDP),Y    ; frames; 0 ends the effect
        BEQ hush
        STA SNDT
        INY
        LDA (SNDP),Y
        STA TIMER_LO    ; the divisor's low byte: it waits for the high one
        STA SNDB        ; (or a noise step's base)
        INY
        LDA (SNDP),Y
        BMI @noise
        STA TIMER_HI    ; the new divisor takes over at the end of a half-wave
        LDA #0          ; a tone: no noise
        STA SNDN
        BEQ @next
@noise: AND #$7F
        STA SNDN        ; the noise's mask
        JSR rattle
@next:  LDA SNDP        ; 3 bytes on: the next step
        CLC
        ADC #3
        STA SNDP
        BCC @gate
        INC SNDP+1
@gate:  LDA #%11        ; gate on, data on: the timer drives the speaker
        STA SPEAKER
        RTS
; Silence: gate and data off, and nothing playing.
hush:   LDA #0
        STA SPEAKER
        STA SNDT
        STA SNDPRI
        RTS
; A noise step's new random divisor.
rattle: LDA NOISE
        STA TIMER_LO
        LDA NOISE       ; read 4 cycles later: an unrelated byte
        AND SNDN
        ORA SNDB
        STA TIMER_HI
        RTS

; Start sound effect X (an FX_ number) at the end of this frame, unless
; something more important is playing; a new effect as important as the one
; playing replaces it. Changes A, X and Y.
sfx:    LDA fxtab,X
        STA SPTR
        LDA fxtab+1,X
        STA SPTR+1
        LDY #0
        LDA (SPTR),Y    ; its priority
        CMP SNDPRI
        BCC @skip       ; something more important is playing
        STA SNDPRI
        LDA SPTR        ; its first step comes after the priority
        CLC
        ADC #1
        STA SNDP
        LDA SPTR+1
        ADC #0
        STA SNDP+1
        LDA #1          ; this frame's sound step starts it
        STA SNDT
@skip:  RTS

; ---------------------------------------------------------------- HUD (text rows)
hudlbl: LDX #<m_hud
        LDY #>m_hud
        STX SRC
        STY SRC+1
        LDA #<TROW21
        STA PTR
        LDA #>TROW21
        STA PTR+1
        JSR puttxt
        LDX #<m_help
        LDY #>m_help
        LDA #<TROW24
        STA PTR
        LDA #>TROW24
        STA PTR+1
puttxt2: STX SRC
        STY SRC+1
        JMP puttxt

; Message line (text row 23): X/Y = a zero-terminated string.
msg:    STX SRC
        STY SRC+1
        LDA #$20
        LDY #39
@clr:   STA TROW23,Y
        DEY
        BPL @clr
        LDA #<TROW23
        STA PTR
        LDA #>TROW23
        STA PTR+1
puttxt: LDY #0
@char:  LDA (SRC),Y
        BEQ @done
        STA (PTR),Y
        INY
        BNE @char
@done:  RTS

clrtext: LDA #$20
        LDX #160
@clr:   STA TROW21-1,X
        DEX
        BNE @clr
        RTS

; ---------------------------------------------------------------- bitmap helpers
; Clear the bitmap's 168 shown lines, and the rest of their last page: 27 pages
; from $2000, 5 cycles a byte.
clrbm:  LDA #0
        TAX
@byte:
        .rept CLRPAGES, page
        STA BM + page*256,X
        .endr
        INX
        BNE @byte
        RTS
        .assert CLRPAGES*256 >= LINES*40, "clrbm misses some of the shown lines"

; Clear T1 lines from line T0.
clrlines: LDY T0
        LDA LTLO,Y
        STA PTR
        LDA LTHI,Y
        STA PTR+1
        LDA #0
        LDY #39
@byte:  STA (PTR),Y
        DEY
        BPL @byte
        INC T0
        DEC T1
        BNE clrlines
        RTS

ground: LDY #GROUND
        LDA LTLO,Y
        STA PTR
        LDA LTHI,Y
        STA PTR+1
        LDA #$FF
        LDY #39
@byte:  STA (PTR),Y
        DEY
        BPL @byte
        RTS

; A list of bitmap texts: col, line, scale (1, 2 or 4), "TEXT", 0; ... $FF.
bmlist: STX SRC
        STY SRC+1
@item:  LDY #0
        LDA (SRC),Y
        CMP #$FF
        BEQ @done
        STA BCOL
        INY
        LDA (SRC),Y
        STA BLINE
        INY
        LDA (SRC),Y
        STA SCALE
        LDA SRC
        CLC
        ADC #3
        STA SRC
        LDA SRC+1
        ADC #0
        STA SRC+1
        JSR bmstr
        INC SRC
        BNE @item
        INC SRC+1
        JMP @item
@done:  RTS

; Draw the string at SRC into the bitmap, glyphs from the character ROM.
bmstr:  LDY #0
        LDA (SRC),Y
        BNE @glyph
        RTS
@glyph: LDX #0
        STX GPTR+1
        ASL A
        ROL GPTR+1
        ASL A
        ROL GPTR+1
        ASL A
        ROL GPTR+1
        STA GPTR
        LDA GPTR+1
        CLC
        ADC #>GLYPHS
        STA GPTR+1
        LDA SCALE
        CMP #1
        BEQ bsfast
        LDA BLINE
        STA T1
        LDA #0
        STA T2
@row:   LDY T2
        LDA (GPTR),Y
        JSR expand
        LDA SCALE
        STA T3
@line:  LDY T1
        LDA LTLO,Y
        CLC
        ADC BCOL
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDY #0
@byte:  LDA EXP,Y
        STA (PTR),Y
        INY
        CPY SCALE
        BNE @byte
        INC T1
        DEC T3
        BNE @line
        INC T2
        LDA T2
        CMP #8
        BNE @row
        LDA BCOL
        CLC
        ADC SCALE
        STA BCOL
        INC SRC
        BNE bmstr
        INC SRC+1
        JMP bmstr

; Scale 1: copy the glyph's 8 bytes straight down the byte column.
bsfast: LDY BLINE
        LDA LTLO,Y
        CLC
        ADC BCOL
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDX #8
        LDY #0
@line:  LDA (GPTR),Y
        STA (PTR),Y
        INC GPTR        ; GPTR is 8-aligned: no carry out of its low byte
        LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @next
        INC PTR+1
@next:  DEX
        BNE @line
        INC BCOL
        INC SRC
        BNE @done
        INC SRC+1
@done:  JMP bmstr

; Widen glyph byte A by SCALE into EXP.
expand: LDX SCALE
        CPX #1
        BNE @wide
        STA EXP
        RTS
@wide:  STA T4
        CPX #2
        BNE @four
        LSR A
        LSR A
        LSR A
        LSR A
        TAX
        LDA dbl2,X
        STA EXP
        LDA T4
        AND #15
        TAX
        LDA dbl2,X
        STA EXP+1
        RTS
@four:  LDX #0
@pair:  LDA #0
        ASL T4
        ROL A
        ASL T4
        ROL A
        TAY
        LDA dbl4,Y
        STA EXP,X
        INX
        CPX #4
        BNE @pair
        RTS

; Copy SPN bytes from SRC down one byte column at BCOL, BLINE.
blit:   LDA BLINE
        STA T1
        LDA #0
        STA T2
@line:  LDY T1
        LDA LTLO,Y
        CLC
        ADC BCOL
        STA PTR
        LDA LTHI,Y
        ADC #0
        STA PTR+1
        LDY T2
        LDA (SRC),Y
        LDY #0
        STA (PTR),Y
        INC T1
        INC T2
        LDA T2
        CMP SPN
        BNE @line
        RTS

; The title's legend: three bugs (legs in frame TANIM), an exception, a patch,
; at x a multiple of 8, so they're copied straight from spr.
tbugs:  LDA #12
        STA BCOL
        LDA #8
        STA SPN
        LDX #0
@bug:   STX T7
        TXA
        ASL A
        ORA TANIM
        ASL A
        ASL A
        ASL A           ; 8 bytes a sprite
        CLC
        ADC #<spr
        STA SRC
        LDA #>spr
        ADC #0
        STA SRC+1
        LDA tbline,X
        STA BLINE
        JSR blit
        LDX T7
        INX
        CPX #3
        BNE @bug
        LDA #<spr_BOMB
        STA SRC
        LDA #>spr_BOMB
        STA SRC+1
        LDA #105
        STA BLINE
        LDA #6
        STA SPN
        JSR blit
        LDA #<spr_PATCH
        STA SRC
        LDA #>spr_PATCH
        STA SRC+1
        LDA #117
        STA BLINE
        JMP blit

; ---------------------------------------------------------------- data
bits:   .byte $01, $02, $04, $08, $10, $20, $40, $80
rowoff: .byte 0, 12, 24, 36, 48
rowspec: .byte 0, 1, 1, 2, 2
rowpts: .byte 3, 2, 2, 1, 1     ; tens: 30, 20, 20, 10, 10
tbline: .byte 64, 76, 88
dbl2:   .byte $00, $03, $0C, $0F, $30, $33, $3C, $3F
        .byte $C0, $C3, $CC, $CF, $F0, $F3, $FC, $FF
dbl4:   .byte $00, $0F, $F0, $FF
steer:  .byte 0, $FF, 1, 0      ; held: none, left, right, both

t_list: .byte 10, 8, 4
        .asciiz "DEBUG"
        .byte 4, 48, 1
        .asciiz "1984. THE BUILD IS FULL OF BUGS."
        .byte 15, 64, 1
        .asciiz "30 POINTS"
        .byte 15, 76, 1
        .asciiz "20 POINTS"
        .byte 15, 88, 1
        .asciiz "10 POINTS"
        .byte 15, 104, 1
        .asciiz "EXCEPTION: LOSE A LIFE"
        .byte 15, 116, 1
        .asciiz "PATCH: DOUBLE SHOT"
        .byte 9, 136, 1
        .byte "A/D ", ARROWL, ARROWR, " MOVE  SPACE FIRE", 0
        .byte 14, 150, 1
        .asciiz "PRESS SPACE"
        .byte $FF
t_crash: .byte 5, 70, 2
        .asciiz "PROGRAM CRASHED"
        .byte 9, 88, 1
        .asciiz "PRESS SPACE TO REBOOT"
        .byte $FF
t_fatal: .byte 5, 70, 2
        .asciiz "FATAL EXCEPTION"
        .byte 9, 88, 1
        .asciiz "PRESS SPACE TO REBOOT"
        .byte $FF

m_hud:  .byte "SCORE 000000  HI 000000  LIVES ", '0' + LIVES0, " WAVE 01", 0
m_help: .byte "A/D ", ARROWL, ARROWR, " MOVE  SPACE FIRE  P PROFILER", 0
m_title: .asciiz "          DEBUG - PRESS SPACE"
m_go:   .asciiz "          SQUASH EVERY BUG"
m_wave: .asciiz "          NEW BUILD. MORE BUGS."
m_blank: .byte 0
m_hit:  .asciiz "          UNCAUGHT EXCEPTION!"
m_clear: .asciiz "          ALL BUGS FIXED. RECOMPILING..."
m_patch: .asciiz "          PATCHED: DOUBLE SHOT"
m_crash: .asciiz "PROGRAM CRASHED          PRESS SPACE"
m_fatal: .asciiz "FATAL EXCEPTION          PRESS SPACE"

; ---------------------------------------------------------------- sound effects
; Priorities: a higher one cuts off a lower one.
PRI_MARCH = 1
PRI_SHOT = 2
PRI_SQUASH = 3          ; a bug squashed, or a patch caught
PRI_HIT = 4             ; an exception hits you, or the wave is clear
PRI_OVER = 5
NOISY   = $80
REST    = 1             ; a divisor that makes no wave: silence

; Divisors (1,000,000 / pitch), from chapter 12's table: halve for an octave up.
E2      = 12136
F2      = 11452
G2      = 10204
A2      = 9092
E3      = 6068
F3      = 5726
FS3     = 5406
G3      = 5102
G4      = 2551
C5      = 1911
E5      = 1517
G5      = 1276
C6      = 956

; The effects by number: an effect's number is its place in fxtab, in bytes.
fxtab:
fx_beat: .word beat0, beat1, beat2, beat3       ; the march's heartbeat
fx_shot: .word shot
fx_squash: .word sq0, sq1, sq2, sq3, sq4        ; a bug squashed, by row
fx_patch: .word patch
fx_hit: .word hit
fx_clear: .word clear
fx_over: .word over
FX_BEAT = fx_beat - fxtab
FX_SHOT = fx_shot - fxtab
FX_SQUASH = fx_squash - fxtab
FX_PATCH = fx_patch - fxtab
FX_HIT  = fx_hit - fxtab
FX_CLEAR = fx_clear - fxtab
FX_OVER = fx_over - fxtab

; The march: four low notes falling, one every 4 cycles of the march.
beat0:  .byte PRI_MARCH, 3, <A2, >A2, 0
beat1:  .byte PRI_MARCH, 3, <G2, >G2, 0
beat2:  .byte PRI_MARCH, 3, <F2, >F2, 0
beat3:  .byte PRI_MARCH, 3, <E2, >E2, 0

; Your shot: a zap falling from 2,800 Hz to 1,100 in six frames.
shot:   .byte PRI_SHOT
        .irp d, 360, 450, 560, 680, 800, 900
        .byte 1, <d, >d
        .endr
        .byte 0

; A bug squashed: two frames of crunch, then a blip pitched by its row, the
; top row's highest: sq0 (30 points) to sq4 (10 points).
        .irp r, 0, 1, 2, 3, 4
sq\r:   .byte PRI_SQUASH
        .byte 2, $02, NOISY + $07       ; 490 to 1,950 Hz, a new one each frame
        .byte 2, <[700 + 200*r], >[700 + 200*r]
        .byte 3, <[1000 + 300*r], >[1000 + 300*r]
        .byte 0
        .endr

; A patch caught: up a C major chord.
patch:  .byte PRI_SQUASH
        .irp d, C5, E5, G5
        .byte 2, <d, >d
        .endr
        .byte 6, <C6, >C6, 0

; An exception hits you: a low rumble, then a wail falling more than an octave.
hit:    .byte PRI_HIT
        .byte 4, $10, NOISY + $1F       ; 120 to 240 Hz
        .rept 14, i
        .byte 1, <[1200 + 160*i], >[1200 + 160*i]
        .endr
        .byte 0

; A wave cleared: a fanfare.
clear:  .byte PRI_HIT
        .irp d, G4, C5, E5
        .byte 3, <d, >d
        .endr
        .byte 6, <G5, >G5, 3, <E5, >E5, 12, <G5, >G5, 0

; Game over: a crash, then a sad trombone, G, F sharp, F, and a long E that
; wobbles, worked out by sin(): 120 either way, a turn every 8 frames.
over:   .byte PRI_OVER
        .byte 6, $10, NOISY + $3F       ; 60 to 240 Hz
        .irp d, G3, FS3, F3
        .byte 10, <d, >d
        .byte 2, <REST, >REST
        .endr
        .rept 16, i
        .byte 2, <[E3 + sin(64*i, 120)], >[E3 + sin(64*i, 120)]
        .endr
        .byte 0

; ---------------------------------------------------------------- sprites
; Every sprite once, 8 rows of 8 pixels as one 64-bit constant, top row in the
; high byte. The shifter moves them to any x as they're drawn, so there are no
; shifted copies. They're all left-right symmetric, so SHIFT_FLIP has nothing to
; mirror; and a double shot's two bars are the single shot's bar, drawn 3 px
; either side of it.
MITE1   = $00183C7EDB7E2442     ; top row, 30 points; legs in
MITE2   = $00183C7EDB7E5A81     ;   and out
BEETLE1 = $243CDB5ADB3C4200     ; rows 2-3, 20 points
BEETLE2 = $243C5ADB5A3C2400
ANT1    = $81423C5A3C993C42     ; rows 4-5, 10 points
ANT2    = $42423C5A3C5A3C81
CANNON  = $183C6600FFFFFFFF     ; a caret on a block cursor
SHOT    = $C0C0C0C000000000     ; a shot: a 2 px bar, 4 lines
BOMB    = $1818181800180000     ; an exception: "!", 6 lines
PATCH   = $18187E7E18180000     ; a patch: "+", 6 lines

; Sprite s's rows are at spr + 8s: the bugs are 0-5 (species * 2 + legs), and
; the label spr_NAME is each one's first row.
spr:
        .irp s, MITE1, MITE2, BEETLE1, BEETLE2, ANT1, ANT2, CANNON, SHOT, BOMB, PATCH
spr_\s:
        .rept 8, row
        .byte s >> 56 - 8*row & $FF
        .endr
        .endr
progend:
        .assert [spr_MITE2 - spr == 8] && [spr_ANT1 - spr == 32], "bug k's rows are at spr + 8k"
        .assert progend <= BM, "the program runs into the bitmap"

; ---------------------------------------------------------------- tables built at start-up
; Reserved, not stored: in the bitmap's bottom lines, which mixed mode doesn't
; show, past the pages clrbm clears. Each table starts a page, so line y's
; address is at $3B00 + y and $3C00 + y in the monitor.
        .org BM + CLRPAGES*256
LTLO:   .res LINES      ; bitmap line addresses: low bytes
        .align 256      ; reserved too, between two .res: costs the program nothing
LTHI:   .res LINES      ; and high bytes
        .assert [GROUND < LINES] && [CY + 8 <= GROUND], "the ground is off the shown bitmap"
        .assert LTHI + LINES <= $4000, "the tables run past the UNC 16's memory"
