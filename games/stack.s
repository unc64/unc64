; STACK: a falling-block puzzle for the UNC 16 (16K), in text mode with UNSCII.
;
; Pieces are trominoes: the straight three (I3) and the corner (L3), in four
; rotations each. Every 8th piece is a BOMB: where it lands it clears the 3 x 3
; cells around itself, and what was above falls into the hole. Full rows clear.
;
; Controls: A/D or cursor left/right move, W or cursor up rotates, S or cursor
; down soft-drops, SPACE hard-drops (and starts and restarts). The game reads the
; keyboard matrix, so keys work together: slide while dropping, turn while sliding.
; A held move waits, then repeats; a held soft drop repeats. See `input`.
;
; Sound comes from the beeper's timer, one note at a time from a table, stepped
; once a frame: it costs a few dozen cycles and never holds the game up. See `sound`.
;
; Paced by polling FRAME ($C006): one tick per 50 Hz frame, no interrupts.
; The ROM prints the numbers (NUMSTR and PRNUM) and rolls the dice (RND, seeded
; from NOISE at the moment SPACE starts a game).
;
; The state is in the fast page, laid out with .res below; the tests read and poke it.
; Screen: the well's inside is columns 12-20, rows 3-20; the panel is at column 24.

CLS     = $FF00         ; ROM routines (chapter 13)
RND     = $FF12
NUMSTR  = $FF18
PRNUM   = $FF1B
RNDSEED = $FF1E
CURX    = $0200         ; the ROM's cursor, where PRNUM prints
CURY    = $0201

SCREEN  = $0400
VMODE   = $C000
FRAME   = $C006
KBDS    = $C011
BORDER  = $C020
CHARSET = $C022
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 speaker data (chapter 12)
TIMER_LO = $C032
TIMER_HI = $C033
NOISE   = $C034
KEYS    = $C060         ; the keyboard matrix, rows 0-8 (chapter 11)
CHARROM = $D000

STEADY  = 1 << 3        ; VMODE bit 3: $A0 doesn't blink
K_LEFT  = 28            ; the cursor keys' codes
K_RIGHT = 29
K_UP    = 30
K_DOWN  = 31
MOVEDLY = 8             ; frames a move is held before it repeats (then every 2nd frame)
SOFTDLY = 2             ; and the soft drop

WELLX   = 12            ; screen column of well column 0
WELLY   = 3             ; screen row of well row 0
NEXTX   = 25            ; screen column and row of the next piece's 3 x 3 box
NEXTY   = 14
WIDTH   = 9
HEIGHT  = 18

; Glyphs. Code $00 is redrawn as the empty-cell dot, so an empty board cell (0)
; can be copied to the screen as it is.
G_EMPTY = $00
G_GHOST = $78
G_BLKI  = $79
G_BLKL  = $7A
G_BOMB  = $7B
G_DOT   = $7C           ; middle dot for the title
G_FIRST = $78
G_COUNT = 5             ; $78-$7C

; The actions, in the order of the key table. Bit n of NOW, LAST and EDGE is action n.
A_LEFT  = 0
A_RIGHT = 1
A_TURN  = 2
A_SOFT  = 3
A_HARD  = 4
NACT    = 5

; ---------------------------------------------------------------- fast page
        .org $00
STATE:  .res 1          ; 0 title, 1 playing, 2 game over
PX:     .res 1          ; piece column: left edge of its 3 x 3 box, well columns 0-8
PY:     .res 1          ; piece row: top of the box, well rows 0-17 (0 = top)
ROT:    .res 1          ; rotation 0-3
TYPE:   .res 1          ; 0 = I3, 1 = L3, 2 = bomb
NTYPE:  .res 1          ; the next piece's type
NROT:   .res 1          ; and its rotation
FALLT:  .res 1          ; frames until the piece next falls one row
LEVEL:  .res 1          ; 1-15
LVLCNT: .res 1          ; lines still needed for the next level
PCOUNT: .res 1          ; number of the next piece; it's a bomb when this is a multiple of 8
GOTMR:  .res 1          ; game over: frames before SPACE is accepted
FLASHT: .res 1          ; frames of border flash left after a bomb
SCORE:  .res 3          ; 24 bits, low byte first, as NUMSTR takes them
LINES:  .res 3          ; the same (follows SCORE: a new game clears both together)
HISC:   .res 3          ; the high score

NOW:    .res 1          ; the actions whose keys are down this frame (bit n: action n)
LAST:   .res 1          ; and last frame
EDGE:   .res 1          ; pressed since last frame
KIN:    .res 1          ; what KBD held this frame
DIR:    .res 1          ; A_LEFT or A_RIGHT: the move that repeats while held
HOLD:   .res NACT       ; frames each action has been down, from 0 on the press
HPIECE: .res 1          ; PCOUNT when the soft drop went down: it stops at a new piece
PANEL:  .res 1          ; not 0: redraw the panel at the start of the next frame

SNDT:   .res 1          ; frames left of the note playing; 0: silent, the gate closed
SNDI:   .res 1          ; the next note: its offset in `sounds`
SNDPRI: .res 1          ; the priority of the effect playing (0 when silent)
SNDNZ:  .res 1          ; a rough note's divisor high byte, or 0 for a steady note

TX:     .res 1          ; trial position for fits
TY:     .res 1
TROT:   .res 1
CNT:    .res 1
TMP:    .res 1
TMP2:   .res 1
MASK:   .res 1
INV:    .res 1          ; ORed into text: $80 prints reverse video
CLRD:   .res 1
KILLS:  .res 1
DSTR:   .res 1
DOFF:   .res 1
CCOL:   .res 4          ; cells of the last fits: entries 1-3
CROW:   .res 4
CIDX:   .res 4          ; board index of each cell
BTOP:   .res 1
BBOT:   .res 1
BN:     .res 1
ECOL:   .res 1
ECNT:   .res 1
DMIN:   .res 1
DCUR:   .res 1
VEC:    .res 2          ; an action's handler, for JMP (VEC)
PTR:    .res 2          ; screen pointer
TXTP:   .res 2          ; text pointer
PS:     .res 2          ; board row pointers for clearing lines
PD:     .res 2
        .assert * <= $D0, "the variables run into $D0-$FE, which the ROM's routines use"

; ---------------------------------------------------------------- memory
; Both blocks must start on a page: CHARSET can only point at one, and the board's
; rows are reached as (PS),Y with only the low byte changing. .align makes it so,
; and costs nothing here: next to .res, its padding is reserved, not stored.
        .org $0800
        .align 256
CHARS:  .res 1024       ; our character set: UNSCII copied in, glyphs $00 and $78-$7C redrawn
        .align 256
BOARD:  .res WIDTH*HEIGHT ; 18 rows x 9 columns, row 0 on top. 0 = empty, else the glyph shown

start:  LDX #2              ; A is 0 in the start state
@nohi:  STA HISC,X          ; no high score yet
        DEX
        BPL @nohi
        JSR initchars
        LDA VMODE
        ORA #STEADY         ; the GAME OVER box is reversed spaces: keep them still
        STA VMODE
        JSR title
mainlp: LDA FRAME
@wait:  CMP FRAME
        BEQ @wait
frame_go:
        JSR tick
frame_end:
        JMP mainlp

; ---------------------------------------------------------------- per frame
tick:   JSR sound           ; the note playing goes on, or the next one starts
        LDA STATE
        BEQ @space
        CMP #1
        BEQ play
        LDA GOTMR           ; game over
        BEQ @space
        DEC GOTMR
        BIT KBDS            ; swallow keys (auto-repeat of the hard drop)
        RTS
@space: LDA KBDS            ; SPACE starts a game; other keys are taken and ignored
        CMP #$80 | ' '
        BNE @rts
        LDX NOISE           ; seed the dice from the cycle SPACE was seen on:
        LDY NOISE           ; never the same twice (RNDSEED turns 0 into its own seed)
        JSR RNDSEED
        JMP newgame
@rts:   RTS

play:   LDA PANEL           ; a frame after a lock, not in its frame: printing is slow
        BEQ @flash
        JSR drawpanel
@flash: LDA FLASHT          ; after a bomb: flash the border
        BEQ @keys
        DEC FLASHT
        LDA FLASHT
        AND #1
        BEQ @bord
        LDA #2              ; red
@bord:  STA BORDER
@keys:  JSR input
        BCS @locked
        DEC FALLT           ; gravity
        BNE @draw
        JSR down
        BCC @draw
@locked: LDA STATE
        CMP #1
        BNE @rts            ; game over drew its own screen
@draw:  JMP draw
@rts:   RTS

; a game that eats a key press has lost the player. read every key.
;
; ---------------------------------------------------------------- keys
; The keyboard matrix ($C060-$C068) shows every key that's down, so each action
; has its own state: NOW (down), EDGE (pressed since last frame) and HOLD (frames
; down). The moves and the soft drop repeat while held, on the game's own timing:
; a move after MOVEDLY frames, the soft drop after SOFTDLY, then every 2nd frame.
;
; KBD still matters for two things the matrix can't see between frames: a tap
; that went down and up again, and a key let go and pressed again. A KBD press
; whose action has been down for MOVEDLY frames or more is the host repeating a
; held key (no host starts repeating sooner): the game ignores it, keeping its
; own time. KBD is read first, so a key that goes down between the two reads is
; seen by the matrix now and left in KBD for next frame, where it's ignored.
; Carry set if the piece locked.
input:  LDA KBDS
        STA KIN
        JSR readkeys
        LDA LAST
        EOR #$FF
        AND NOW
        STA EDGE
        LDA NOW
        STA LAST
        LDA KIN             ; a key in KBD?
        BPL @count
        AND #$7F
        JSR findkey         ; X = its action
        BMI @count          ; not one of ours
        LDA bits,X
        BIT NOW
        BEQ @tap            ; not down now: it came and went between frames
        BIT EDGE
        BNE @count          ; the press the matrix sees as well
        LDY HOLD,X
        CPY #MOVEDLY
        BCS @count          ; held a while: the host repeating it
@tap:   ORA EDGE            ; a press of its own
        STA EDGE
@count: LDX #NACT-1         ; count the frames each action is held
@cnt:   LDA bits,X
        BIT EDGE
        BNE @zero           ; just pressed: 0
        BIT NOW
        BEQ @next           ; up: it starts again from 0 on its next press
        INC HOLD,X
        BPL @next
        LDA #$40            ; held for 2.5 seconds: wind back, still past every
        BNE @set            ; delay, and even as $80 was: the repeats keep step
@zero:  LDA #0
@set:   STA HOLD,X
@next:  DEX
        BPL @cnt

; Moves. The direction pressed last moves and repeats. Let it go while the other
; is still down, and the other takes over, with its delay from the start.
        LDA EDGE
        AND #1 << A_LEFT | 1 << A_RIGHT
        BEQ @nonew
        LDX #A_LEFT         ; left, if both came at once
        AND #1 << A_LEFT
        BNE @dir
        INX
@dir:   STX DIR
        JSR act
        JMP @turn
@nonew: LDX DIR
        LDA NOW
        AND bits,X
        BNE @rep            ; still down
        TXA
        EOR #1              ; let go: the other direction
        TAX
        LDA NOW
        AND bits,X
        BEQ @turn           ; isn't down either
        STX DIR             ; takes over, timed from now
        LDA #0
        STA HOLD,X
        BEQ @turn           ; (always)
@rep:   LDA HOLD,X
        CMP #MOVEDLY
        BCC @turn           ; not held long enough yet
        LSR A
        BCS @turn           ; every second frame
        JSR act             ; (moves never lock)
@turn:  LDX #A_TURN         ; rotation: once a press
        LDA EDGE
        AND #1 << A_TURN
        BEQ @soft
        JSR act
@soft:  LDX #A_SOFT         ; the soft drop: on the press...
        LDA EDGE
        AND #1 << A_SOFT
        BEQ @sheld
        LDA PCOUNT
        STA HPIECE
        JMP @sgo
@sheld: LDA NOW             ; ...and while held, for the same piece
        AND #1 << A_SOFT
        BEQ @hard
        LDA HPIECE
        CMP PCOUNT
        BNE @hard           ; it landed: a new piece needs a new press
        LDA HOLD+A_SOFT
        CMP #SOFTDLY
        BCC @hard
        LSR A
        BCS @hard           ; every second frame
@sgo:   JSR act
        BCS @rts            ; it locked
@hard:  LDA EDGE            ; the hard drop: once a press
        AND #1 << A_HARD
        BEQ @none
        LDX #A_HARD
        JMP act
@none:  CLC
@rts:   RTS

; Do action X. The handlers return carry set if the piece locked.
act:    LDA actlo,X
        STA VEC
        LDA acthi,X
        STA VEC+1
        JSR setT            ; they all start from where the piece is
        JMP (VEC)

; NOW = the actions whose keys are down in the matrix, and A the same. One row
; read a key, each key's row and bit from the key table.
readkeys: LDA #0
        STA NOW
        LDX #[NACT - 1] * 6 ; the last action's first key: two keys an action, 3 bytes a key
@act:   LDY keytab+1,X      ; its row
        LDA KEYS,Y
        AND keytab+2,X      ; and its bit
        BNE @down
        LDY keytab+4,X      ; the action's other key
        LDA KEYS,Y
        AND keytab+5,X
@down:  CMP #1              ; carry set if either is down
        ROL NOW             ; the last action ends up in bit NACT-1, the first in bit 0
        TXA
        SEC
        SBC #6
        TAX
        BCS @act
        LDA NOW
        RTS

; X = the action for key code A, or negative (N set) if it isn't one.
findkey: LDX #[NKEYS - 1] * 3
        LDY #NKEYS-1
@scan:  CMP keytab,X
        BEQ @found
        DEX
        DEX
        DEX
        DEY
        BPL @scan
        RTS
@found: TYA                 ; key Y is action Y / 2
        LSR A
        TAX
        RTS

left:   DEC TX
        JMP move
rotate: LDA TROT            ; no kicks: a blocked rotation is refused
        CLC
        ADC #1
        AND #3
        STA TROT
        LDX #S_TURN
        JMP shift
right:  INC TX
move:   LDX #S_MOVE
shift:  TXA                 ; refused or not, nothing locks. Only a move that
        PHA                 ; happens ticks: pushing at a wall is silent.
        JSR trymove
        PLA
        BCS @no
        TAX
        JSR sfx
@no:    CLC
        RTS

soft:   JSR down
        BCS @rts            ; it was on the floor: it locked
        LDA #1              ; a point a row
        JSR addscore
        INC PANEL
        CLC
@rts:   RTS

hard:   JSR pcells
        JSR dropdist
        PHA
        CLC
        ADC PY
        STA PY
        PLA
        ASL A               ; 2 points a row
        JSR addscore
        JSR lock
        SEC
        RTS

; ---------------------------------------------------------------- sound
; One voice: the beeper's timer (chapter 12). An effect is a list of notes in
; `sounds`; `sfx` starts one and `sound`, once a frame, counts each note's frames
; down and starts the next. Between effects the gate is closed: silence.
;
; A new effect replaces the one playing unless that one matters more: every
; effect starts with its priority, so a tick never cuts a line clear short.

; Start effect X (an S_ constant: where it starts in `sounds`). Changes A, X, Y.
sfx:    LDA sounds,X        ; its priority
        CMP SNDPRI
        BCC @rts            ; something more important is playing
        STA SNDPRI
        INX                 ; its first note, at once
        JMP note
@rts:   RTS

; One frame of sound. A rough note gets a new random divisor every frame: the
; bomb's noise, made by the timer instead of by flipping the speaker in a loop.
sound:  LDA SNDT
        BEQ @rts            ; silent
        LDA SNDNZ
        BEQ @count
        LDX NOISE           ; low byte: anything
        LDA NOISE
        AND #15             ; high byte: the note's, plus 0-15
        CLC
        ADC SNDNZ
        STX TIMER_LO
        STA TIMER_HI        ; (the timer takes the divisor when the high byte comes)
@count: DEC SNDT
        BEQ @next
@rts:   RTS
@next:  LDX SNDI            ; this note is over: the next
; Start the note at offset X in `sounds`: frames (bit 7: rough), then the divisor.
; A divisor of 0 is a rest; a length of 0 ends the effect.
note:   LDA sounds,X
        BEQ hush
        AND #$7F
        STA SNDT
        LDY #%11            ; gate open, speaker data on: the timer plays
        LDA sounds+1,X
        STA TIMER_LO
        ORA sounds+2,X
        BNE @tone
        LDY #%01            ; a rest: the timer runs, the speaker doesn't follow
@tone:  LDA sounds+2,X
        STA TIMER_HI
        STY SPEAKER
        LDY #0              ; steady, unless bit 7 says rough
        LDA sounds,X
        BPL @steady
        LDY sounds+2,X
@steady: STY SNDNZ
        TXA
        CLC
        ADC #3
        STA SNDI
@rts:   RTS

; Silence: the gate closed, the speaker in, nothing playing.
hush:   LDA #0
        STA SPEAKER
        STA SNDT
        STA SNDPRI
        RTS

; ---------------------------------------------------------------- pieces
setT:   LDA PX
        STA TX
        LDA PY
        STA TY
        LDA ROT
        STA TROT
        RTS

; Try the trial position: if it fits, make it the piece's. Carry set if refused.
trymove: JSR fits
        BCS @no
        LDA TX
        STA PX
        LDA TY
        STA PY
        LDA TROT
        STA ROT
        CLC
@no:    RTS

; One row down, or lock if it can't go. Restarts the fall timer. Carry set if it locked.
down:   JSR resetfall
        JSR setT
        INC TY
        JSR trymove
        BCC @rts
        JSR lock
        SEC
@rts:   RTS

; Cells of the current piece into CCOL/CROW/CIDX.
pcells: JSR setT
        ; fall through
; Does TYPE at TX,TY,TROT fit? Carry clear if so. Fills CCOL/CROW/CIDX 1-3
; for the cells checked. Columns wrap: TX = $FF + 0 is $FF, off the well.
fits:   LDA TYPE
        ASL A
        ASL A
        ORA TROT
        TAX
        LDA shofs,X
        TAY                 ; Y walks the shape's dx,dy pairs
        LDX #3              ; X counts cells 3..1
@cell:  LDA shapes,Y
        CLC
        ADC TX
        CMP #WIDTH
        BCS @bad
        STA CCOL,X
        LDA shapes+1,Y
        CLC
        ADC TY
        CMP #HEIGHT
        BCS @bad
        STA CROW,X
        STY TMP
        TAY
        LDA rowoff,Y
        CLC
        ADC CCOL,X
        STA CIDX,X
        TAY
        LDA BOARD,Y
        BNE @bad
        LDY TMP
        INY
        INY
        DEX
        BNE @cell
        CLC
        RTS
@bad:   SEC
        RTS

; Rows the current piece can fall (CIDX must hold its cells). A = distance.
dropdist: LDA #HEIGHT
        STA DMIN
        LDX #3
@cell:  LDA #0
        STA DCUR
        LDA CIDX,X
@down:  CLC
        ADC #WIDTH
        CMP #WIDTH*HEIGHT
        BCS @end
        TAY
        LDA BOARD,Y
        BNE @end
        INC DCUR
        TYA
        JMP @down
@end:   LDA DCUR
        CMP DMIN
        BCS @next
        STA DMIN
@next:  DEX
        BNE @cell
        LDA DMIN
        RTS

resetfall: LDX LEVEL
        LDA speeds-1,X
        STA FALLT
        RTS

; Deal the next piece into NTYPE/NROT.
dealnext: INC PCOUNT
        LDA PCOUNT
        AND #7
        BNE @rand
        LDA #2              ; a bomb: its rotation doesn't matter
        STA NTYPE
        RTS
@rand:  JSR RND
        AND #1
        STA NTYPE
        JSR RND
        AND #3
        STA NROT
        RTS

; The next piece enters at the top. If it doesn't fit, the game is over.
spawn:  LDA NTYPE
        STA TYPE
        LDA NROT
        STA ROT
        LDA #3
        STA PX
        LDA #0
        STA PY
        JSR resetfall
        JSR dealnext
        INC PANEL
        JSR pcells
        BCC @rts
        JMP gameover
@rts:   RTS

; Put the piece into the board (or set off the bomb), clear rows, spawn.
lock:   JSR pcells
        LDA TYPE
        CMP #2
        BNE @stamp
        JSR explode
        JMP @lines
@stamp: LDY TYPE
        LDA glyphs,Y
        LDX #3
@cell:  LDY CIDX,X
        STA BOARD,Y
        DEX
        BNE @cell
        LDX #S_LAND         ; a thud (a line clear, if there is one, plays over it)
        JSR sfx
@lines: JSR clearlines
        JMP spawn

; the bomb was meant to be a bug. it stayed.
;
; ---------------------------------------------------------------- the bomb
; Clear rows BR-1..BR+1 of columns BC-1..BC+1, then in each of those columns
; drop everything above the blast down into the hole. 5 points a block.
explode: LDA CROW+1
        BEQ @top
        SEC
        SBC #1
@top:   STA BTOP
        LDA CROW+1
        CMP #HEIGHT-1
        BCS @bot
        ADC #1              ; carry is clear
@bot:   STA BBOT
        SEC
        SBC BTOP
        CLC
        ADC #1
        STA BN              ; rows in the blast: 2 or 3
        LDA #0
        STA KILLS
        LDA CCOL+1
        SEC
        SBC #1
        STA ECOL            ; may be $FF: off the well, skipped
        LDA #3
        STA ECNT
@col:   LDA ECOL
        CMP #WIDTH
        BCS @next
        JSR settlecol
@next:  INC ECOL
        DEC ECNT
        BNE @col
        LDA KILLS
        ASL A
        ASL A
        CLC
        ADC KILLS
        JSR addscore
        LDA #10             ; the border flashes for the next 10 frames
        STA FLASHT
        LDX #S_BOMB         ; and the timer roars
        JMP sfx

settlecol: LDY BTOP         ; count what the blast destroys
@count: LDA rowoff,Y
        CLC
        ADC ECOL
        TAX
        LDA BOARD,X
        BEQ @empty
        INC KILLS
@empty: CPY BBOT
        INY
        BCC @count
        LDY BBOT            ; row y takes row y - BN, from the bottom up
@row:   TYA
        SEC
        SBC BN
        BCC @zero
        TAX
        LDA rowoff,X
        CLC
        ADC ECOL
        TAX
        LDA BOARD,X
        JMP @put
@zero:  LDA #0
@put:   STA TMP
        LDA rowoff,Y
        CLC
        ADC ECOL
        TAX
        LDA TMP
        STA BOARD,X
        DEY
        BPL @row
        RTS

; three cells a piece. four was greed.
;
; ---------------------------------------------------------------- rows
; One pass from the bottom: full rows are skipped, the others copied down to DSTR.
clearlines: LDA #0
        STA CLRD
        LDA #>BOARD
        STA PS+1
        STA PD+1
        LDA #HEIGHT-1
        STA DSTR
        LDY #HEIGHT-1
@row:   LDX rowoff,Y
        LDA #WIDTH
        STA CNT
@cell:  LDA BOARD,X
        BEQ @keep
        INX
        DEC CNT
        BNE @cell
        INC CLRD            ; full: leave it behind
        JMP @next
@keep:  CPY DSTR
        BEQ @same
        STY TMP
        LDA rowoff,Y
        STA PS
        LDY DSTR
        LDA rowoff,Y
        STA PD
        LDY #WIDTH-1
@copy:  LDA (PS),Y
        STA (PD),Y
        DEY
        BPL @copy
        LDY TMP
@same:  DEC DSTR
@next:  DEY
        BPL @row
        LDA CLRD
        BNE @some
        RTS
@some:  LDY DSTR            ; empty rows 0..DSTR
        LDA rowoff,Y
        CLC
        ADC #WIDTH-1
        TAX
        LDA #0
@empty: STA BOARD,X
        DEX
        CPX #$FF
        BNE @empty
        LDA CLRD            ; lines
        CLC
        ADC LINES
        STA LINES
        BCC @pts
        INC LINES+1
        BNE @pts
        INC LINES+2
@pts:   LDX CLRD            ; score: 10/30/60/100 times the level
        CPX #4
        BCC @mul
        LDX #4
@mul:   LDY LEVEL
@add:   LDA linepts,X
        JSR addscore
        DEY
        BNE @add
        LDA linesfx,X       ; a chime: grander for more rows at once
        TAX
        JSR sfx
        LDA LVLCNT          ; level up every 10 lines
        SEC
        SBC CLRD
@lvl:   BEQ @up
        BPL @left
@up:    LDX LEVEL
        CPX #15
        BCS @top
        INC LEVEL
        PHA
        LDX #S_LEVEL        ; a fanfare, over the chime
        JSR sfx
        PLA
@top:   CLC
        ADC #10             ; sets the flags for @lvl
        JMP @lvl
@left:  STA LVLCNT
        RTS

; ---------------------------------------------------------------- numbers
; The score is a plain 24-bit binary number, printed by the ROM: NUMSTR turns it
; into eight digits and PRNUM prints them, zero-padded to a fixed width. That is
; the slow way (manual chapter 13, "Keeping score fast": about 2,000 cycles for
; six digits), and STACK can afford it: the panel is redrawn only after a lock or
; a soft drop, at most every second frame and never in the frame of a lock, and
; a binary score is easy to add the level-times points to and to compare with
; the high score. A score that changed every frame would want a row of digits.

; Add A (0-255) to SCORE. Changes A only. (16,777,215 is out of reach.)
addscore: CLC
        ADC SCORE
        STA SCORE
        BCC @rts
        INC SCORE+1
        BNE @rts
        INC SCORE+2
@rts:   RTS

; The ROM's cursor to column X, row Y.
at:     STX CURX
        STY CURY
        RTS

; Print the 24-bit number at fast-page address X (SCORE, LINES or HISC) at the
; cursor, A digits wide: padded with zeros, and never cut short.
prnum:  PHA                 ; the width
        LDY 1,X             ; NUMSTR takes the middle byte in Y,
        LDA 2,X             ; the high byte in A
        PHA
        LDA 0,X             ; and the low byte in X
        TAX
        PLA
        JSR NUMSTR
        PLA
        JMP PRNUM

; ---------------------------------------------------------------- screens
newgame: LDA #0
        LDX #5
@zero:  STA SCORE,X         ; score and lines
        DEX
        BPL @zero
        TAX
@clear: STA BOARD,X
        INX
        CPX #WIDTH*HEIGHT
        BNE @clear
        STA PCOUNT
        STA FLASHT
        STA BORDER
        LDA #1
        STA LEVEL
        STA STATE
        LDA #10
        STA LVLCNT
        JSR hush            ; cut the game-over tune short
        JSR readkeys        ; the keys down now (SPACE, at least) aren't presses
        STA LAST
        JSR drawplay
        JSR dealnext
        JMP spawn

gameover: LDA #2
        STA STATE
        LDA #50
        STA GOTMR
        LDA #0
        STA BORDER
        STA FLASHT
        LDX #S_OVER         ; the sad trombone
        JSR sfx
        LDX #2              ; a new high score? The high byte first.
@cmp:   LDA SCORE,X
        CMP HISC,X
        BCC @draw
        BNE @new
        DEX
        BPL @cmp
        BMI @draw           ; (always) the same score
@new:   LDX #2
@copy:  LDA SCORE,X
        STA HISC,X
        DEX
        BPL @copy
@draw:  JSR drawwell
        JSR drawpanel
        LDX #27
        LDY #21
        JSR prhi
        LDA #$80
        STA INV
        LDX #<overtxt
        LDY #>overtxt
        JSR putlist
        LDA #0
        STA INV
        RTS

title:  LDA #0
        STA STATE
        STA BORDER
        STA INV
        JSR CLS
        JSR bigtitle
        LDX #<titletxt
        LDY #>titletxt
        JSR putlist
        LDX #19
        LDY #22
        ; fall through
; The high score at column X, row Y, six digits.
prhi:   JSR at
        LDX #HISC
        LDA #6
        JMP prnum

; The well's walls (its floor is in playtxt), the panel's labels and the high score.
drawplay: JSR CLS
        LDX #<playtxt
        LDY #>playtxt
        JSR putlist
        LDX #0
@wall:  LDA rowlo,X
        STA PTR
        LDA rowhi,X
        STA PTR+1
        LDA #$11            ; vertical line
        LDY #WELLX-1
        STA (PTR),Y
        LDY #WELLX+WIDTH
        STA (PTR),Y
        INX
        CPX #HEIGHT
        BNE @wall
        LDX #27
        LDY #21
        JMP prhi

; Board, ghost and piece: every frame.
draw:   JSR drawwell
        JSR pcells
        JSR dropdist
        STA DOFF
        LDA #G_GHOST
        JSR drawcells
        LDA #0
        STA DOFF
        LDY TYPE
        LDA glyphs,Y
        JMP drawcells

; Copy the board to the screen. An empty cell is 0, which is the dot glyph.
drawwell: LDA #<SCREEN+WELLY*40+WELLX
        STA PTR
        LDA #>SCREEN+WELLY*40+WELLX
        STA PTR+1
        LDX #0
        LDA #HEIGHT
        STA CNT
@row:   LDY #0
@cell:  LDA BOARD,X
        STA (PTR),Y
        INX
        INY
        CPY #WIDTH
        BNE @cell
        LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @next
        INC PTR+1
@next:  DEC CNT
        BNE @row
        RTS

; Draw glyph A at the cells in CROW/CCOL, DOFF rows lower.
drawcells: STA TMP
        LDX #3
@cell:  LDA CROW,X
        CLC
        ADC DOFF
        TAY
        LDA rowlo,Y
        STA PTR
        LDA rowhi,Y
        STA PTR+1
        LDA CCOL,X
        CLC
        ADC #WELLX
        TAY
        LDA TMP
        STA (PTR),Y
        DEX
        BNE @cell
        RTS

; Score, lines and level, the bomb countdown and the next piece. The numbers
; have fixed widths and only grow during a game, so each new one covers the old.
drawpanel: LDA #0
        STA PANEL
        LDX #24
        LDY #4
        JSR at
        LDX #SCORE
        LDA #6              ; 000000
        JSR prnum
        LDX #24
        LDY #7
        JSR at
        LDX #LINES
        LDA #3              ; 000
        JSR prnum
        LDX #24
        LDY #10
        JSR at
        LDX LEVEL           ; one byte: the middle and high bytes are 0
        LDY #0
        TYA
        JSR NUMSTR
        LDA #2              ; 01
        JSR PRNUM
        LDA PCOUNT          ; pieces until the bomb: ((8 - PCOUNT) AND 7) + 1
        EOR #$FF
        CLC
        ADC #1
        AND #7
        CLC
        ADC #'1'
        STA SCREEN+19*40+32
        LDA #' '            ; the next piece
        LDX #2
@clear: STA SCREEN+NEXTY*40+NEXTX,X
        STA SCREEN+[NEXTY+1]*40+NEXTX,X
        STA SCREEN+[NEXTY+2]*40+NEXTX,X
        DEX
        BPL @clear
        LDA NTYPE
        ASL A
        ASL A
        ORA NROT
        TAX
        LDA shofs,X
        TAY
        LDX NTYPE
        LDA glyphs,X
        STA TMP
        LDA #3
        STA CNT
@cell:  LDX shapes+1,Y        ; the well's row table reaches the box's rows
        LDA rowlo+NEXTY-WELLY,X
        STA PTR
        LDA rowhi+NEXTY-WELLY,X
        STA PTR+1
        STY TMP2
        LDA shapes,Y
        CLC
        ADC #NEXTX
        TAY
        LDA TMP
        STA (PTR),Y
        LDY TMP2
        INY
        INY
        DEC CNT
        BNE @cell
        RTS

; STACK in 3 x 5 block letters, rows 3-7, columns 10-28.
bigtitle: LDA #<SCREEN+3*40+10
        STA PTR
        LDA #>SCREEN+3*40+10
        STA PTR+1
        LDX #0
@row:   LDY #0
        LDA #5
        STA CNT
@let:   LDA font,X
        STA TMP
        LDA #4
        STA MASK
@bit:   LDA TMP
        AND MASK
        BEQ @sp
        LDA #G_BLKI
        JMP @st
@sp:    LDA #' '
@st:    STA (PTR),Y
        INY
        LSR MASK
        BNE @bit
        INY
        INX
        DEC CNT
        BNE @let
        LDA PTR
        CLC
        ADC #40
        STA PTR
        BCC @next
        INC PTR+1
@next:  CPX #25
        BNE @row
        RTS

; Print a list of records: .word screen address, text, 0. A zero word ends it.
putlist: STX TXTP
        STY TXTP+1
@rec:   LDY #0
        LDA (TXTP),Y
        STA PTR
        INY
        LDA (TXTP),Y
        BEQ @done
        STA PTR+1
        LDA TXTP
        CLC
        ADC #2
        STA TXTP
        BCC @text
        INC TXTP+1
@text:  LDY #0
@ch:    LDA (TXTP),Y
        BEQ @eos
        ORA INV
        STA (PTR),Y
        INY
        BNE @ch
@eos:   INY
        TYA
        CLC
        ADC TXTP
        STA TXTP
        BCC @rec
        INC TXTP+1
        JMP @rec
@done:  RTS

; Copy UNSCII to RAM, redraw our glyphs, switch the video chip over.
initchars: LDX #0
@copy:  .rept 4, page
        LDA CHARROM+page*$100,X
        STA CHARS+page*$100,X
        .endr
        INX
        BNE @copy
        LDX #7
@empty: LDA g_empty,X
        STA CHARS+G_EMPTY*8,X
        DEX
        BPL @empty
        LDX #G_COUNT*8-1
@ours:  LDA glyphdata,X
        STA CHARS+G_FIRST*8,X
        DEX
        BPL @ours
        LDA #>CHARS
        STA CHARSET
        RTS

; ---------------------------------------------------------------- data
; The keys, two for each action, in the order of the A_ constants: left, right,
; rotate, soft drop, hard drop (SPACE alone, so twice). For each key: its code, as
; KBD gives it, then its row of the matrix and its bit in that row, worked out by
; the assembler from the code (chapter 11): row 8 for the cursor keys (28-31) in
; bits 0-3, else row (code - 32) / 8 and bit code AND 7.
keytab: .irp k, 'A', K_LEFT, 'D', K_RIGHT, 'W', K_UP, 'S', K_DOWN, ' ', ' '
        .byte k, [k < 32] * 8 + [k >= 32] * [[k - 32] / 8], 1 << [[k < 32] * [k - 28] + [k >= 32] * [k & 7]]
        .endr
NKEYS   = [* - keytab] / 3
        .assert NKEYS == 2 * NACT, "two keys for each action"
bits:   .byte 1, 2, 4, 8, 16    ; action n's bit in NOW, LAST and EDGE
actlo:  .byte <left, <right, <rotate, <soft, <hard
acthi:  .byte >left, >right, >rotate, >soft, >hard

; Shapes: three dx,dy pairs inside a 3 x 3 box. The bomb is one cell three times.
shapes:
I3_FLAT: .byte 0,1, 1,1, 2,1        ; I3 flat, in the middle row
I3_UP:  .byte 1,0, 1,1, 1,2         ; I3 upright, in the middle column
L3_TR:  .byte 0,0, 0,1, 1,1         ; L3, gap top right
L3_BR:  .byte 0,0, 1,0, 0,1         ; L3, gap bottom right
L3_BL:  .byte 0,0, 1,0, 1,1         ; L3, gap bottom left
L3_TL:  .byte 1,0, 0,1, 1,1         ; L3, gap top left
BOMB1:  .byte 1,0, 1,0, 1,0         ; bomb
; Where each shape starts in `shapes`, for type * 4 + rotation: the I3 flat,
; upright, flat, upright; the L3 with its gap turning clockwise; the bomb, any way up.
shofs:  .irp s, I3_FLAT, I3_UP, I3_FLAT, I3_UP, L3_TR, L3_BR, L3_BL, L3_TL, BOMB1, BOMB1, BOMB1, BOMB1
        .byte s - shapes
        .endr
glyphs: .byte G_BLKI, G_BLKL, G_BOMB

speeds: .byte 45, 38, 32, 27, 23, 19, 16, 13, 11, 9, 7, 6, 5, 4, 3
linepts: .byte 0, 10, 30, 60, 100

; Board offset of each row.
rowoff: .rept HEIGHT, r
        .byte r * WIDTH
        .endr
; Screen address of column 0 for well rows 0-17, and the floor (row 18).
        .assert NEXTY >= WELLY && NEXTY + 2 <= WELLY + HEIGHT, "the next box needs rows the table has"
rowlo:  .rept HEIGHT + 1, r
        .byte <SCREEN + [WELLY + r] * 40
        .endr
rowhi:  .rept HEIGHT + 1, r
        .byte >SCREEN + [WELLY + r] * 40
        .endr

; ---------------------------------------------------------------- sounds
; Each effect: its priority, then notes of three bytes: frames (with ROUGH, a new
; random divisor every frame from NOISE, the high byte from the note's plus 0-15),
; then the timer's divisor, 1,000,000 / pitch (0: a rest). A length of 0 ends it.
; The S_ constants are where each effect starts, for `sfx`.
ROUGH   = $80
C4      = 3822          ; the divisors of chapter 12's table
DS4     = 3214
E4      = 3034
F4      = 2863
FS4     = 2703
G4      = 2551
C5      = C4 / 2        ; an octave up: half the divisor
E5      = E4 / 2
G5      = G4 / 2
C6      = C5 / 2
E6      = E5 / 2
G6      = G5 / 2
C7      = C6 / 2

sounds:
s_move: .byte 1                     ; a move: a tick, 1,600 Hz for a frame
        .byte 1, <625, >625, 0
s_turn: .byte 1                     ; a turn: a chirp, up
        .byte 1, <800, >800, 1, <500, >500, 0
s_land: .byte 2                     ; landing: a low thud
        .byte 1, <5000, >5000, 2, <8000, >8000, 0
s_line1: .byte 3                    ; one row: two notes up
        .byte 3, <G5, >G5, 6, <C6, >C6, 0
s_line2: .byte 3                    ; two rows: a major arpeggio
        .irp n, E5, G5, C6
        .byte 3, <n, >n
        .endr
        .byte 8, <E6, >E6, 0
s_line3: .byte 3                    ; three or more: two octaves up and a trill
        .irp n, C5, E5, G5, C6, E6, G6, C7, G6
        .byte 2, <n, >n
        .endr
        .byte 12, <C7, >C7, 0
s_bomb: .byte 4                     ; the bomb: a crack, then a falling roar
        .byte 2 | ROUGH, 0, $02, 5 | ROUGH, 0, $10, 10 | ROUGH, 0, $30, 0
s_level: .byte 5                    ; a new level: a fanfare
        .irp n, C5, E5, G5
        .byte 3, <n, >n
        .endr
        .byte 4, <C6, >C6, 2, 0, 0, 3, <G5, >G5, 12, <C6, >C6, 0
s_over: .byte 6                     ; game over: wah, wah, wah, waaah
        .irp n, G4, FS4, F4
        .byte 12, <n, >n, 3, 0, 0
        .endr
        .irp n, E4, DS4, E4, DS4
        .byte 4, <n, >n
        .endr
        .byte 24, <E4, >E4, 0
        .assert * - sounds <= 256, "sfx and note reach the notes as sounds,X"
S_MOVE  = s_move - sounds
S_TURN  = s_turn - sounds
S_LAND  = s_land - sounds
S_LINE1 = s_line1 - sounds
S_LINE2 = s_line2 - sounds
S_LINE3 = s_line3 - sounds
S_BOMB  = s_bomb - sounds
S_LEVEL = s_level - sounds
S_OVER  = s_over - sounds
linesfx: .byte 0, S_LINE1, S_LINE2, S_LINE3, S_LINE3    ; by rows cleared, as linepts

; S T A C K, five letters per pixel row, 3 bits each.
font:   .byte 7, 7, 7, 7, 5
        .byte 4, 2, 5, 4, 5
        .byte 7, 2, 7, 4, 6
        .byte 1, 2, 5, 4, 5
        .byte 7, 2, 5, 7, 5

g_empty: .byte %00000000, %00000000, %00000000, %00010000
        .byte %00000000, %00000000, %00000000, %00000000
glyphdata:
        .byte %11000011, %10000001, %00000000, %00000000   ; $78 ghost
        .byte %00000000, %00000000, %10000001, %11000011
        .byte %11111110, %11111110, %11111110, %11111110   ; $79 I3 block
        .byte %11111110, %11111110, %11111110, %00000000
        .byte %11111111, %10000001, %10111101, %10111101   ; $7A L3 block
        .byte %10111101, %10111101, %10000001, %11111111
        .byte %00000101, %00000010, %00011100, %01111110   ; $7B bomb
        .byte %11111011, %11111111, %01111110, %00111100
        .byte %00000000, %00000000, %00000000, %00011000   ; $7C middle dot
        .byte %00011000, %00000000, %00000000, %00000000
        .assert * - glyphdata == G_COUNT * 8, "G_COUNT doesn't match the glyphs"

titletxt:
        .word SCREEN+10*40+10
        .text "STACK "
        .byte G_DOT
        .text " PRESS SPACE"
        .byte 0
        .word SCREEN+12*40+7
        .asciiz "A/D MOVE  W ROTATE  S DROP"
        .word SCREEN+13*40+11
        .asciiz "OR THE CURSOR KEYS"
        .word SCREEN+14*40+12
        .asciiz "SPACE HARD DROP"
        .word SCREEN+16*40+6
        .text "EVERY 8TH PIECE IS A BOMB "
        .byte G_BOMB, 0
        .word SCREEN+17*40+6
        .asciiz "IT BLASTS THE 3X3 AROUND IT"
        .word SCREEN+22*40+16
        .asciiz "HI"
        .word 0

playtxt:
        .word SCREEN+[WELLY+HEIGHT]*40+WELLX-1
        .byte $14           ; the well's floor, between two corners
        .fill WIDTH, $10
        .byte $15, 0
        .word SCREEN+1*40+12
        .asciiz "S T A C K"
        .word SCREEN+3*40+24
        .asciiz "SCORE"
        .word SCREEN+6*40+24
        .asciiz "LINES"
        .word SCREEN+9*40+24
        .asciiz "LEVEL"
        .word SCREEN+12*40+24
        .asciiz "NEXT"
        .word SCREEN+13*40+24
        .byte $12, $10, $10, $10, $13, 0
        .word SCREEN+14*40+24
        .byte $11, ' ', ' ', ' ', $11, 0
        .word SCREEN+15*40+24
        .byte $11, ' ', ' ', ' ', $11, 0
        .word SCREEN+16*40+24
        .byte $11, ' ', ' ', ' ', $11, 0
        .word SCREEN+17*40+24
        .byte $14, $10, $10, $10, $15, 0
        .word SCREEN+19*40+24
        .asciiz "BOMB IN"
        .word SCREEN+21*40+24
        .asciiz "HI"
        .word 0

overtxt:
        .word SCREEN+9*40+WELLX
        .asciiz "         "
        .word SCREEN+10*40+WELLX
        .asciiz "GAME OVER"
        .word SCREEN+11*40+WELLX
        .asciiz "         "
        .word SCREEN+12*40+WELLX
        .asciiz "  SPACE  "
        .word SCREEN+13*40+WELLX
        .asciiz "         "
        .word 0
code_end:
        .assert code_end <= $4000, "STACK has to fit a 16K UNC 16"
