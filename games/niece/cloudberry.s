; CLOUDBERRY: a platformer that scrolls, for the UNC 64 (64K) with the NIECE, the
; prototype graphics chip of chapter 17, the one marked NV-1. No video board needed.
; United Northern Computers' laboratory, 1985. Not for sale.
;
; Late summer on the fells, and the cloudberries are ripe. You're out with a pail,
; across the bog on its duckboards, through the birch wood and over the stones, to
; the hut at the far end of the level; and then, in the evening, back over the high
; stones to the other hut. Pick what you pass. A lemming bites if you walk into it,
; and is flat if you land on it. A cloud of midges is best left alone. The bog is
; wet: fall in and you're back where you last stood, a life short. Three lives.
;
; Controls: cursor left and right, or A and D, run. SPACE, cursor up or W jumps: a
; tap is a hop, and holding it a leap of three tiles. They're held keys, from the
; keyboard matrix, so running and jumping go together. SPACE starts the game.
;
; A berry is 50 points, a flattened lemming 100, and every second left on the clock
; when you reach the hut is 10. The clock running out costs nothing but those.
;
; What it shows of the NIECE:
;   scrolling a pixel at a time, through a level 2,048 pixels long (12.8 pages),
;     with ONE BLIT a frame. The stretch of level round the camera is kept on the
;     sheet as a ring 256 pixels wide, and the page's 160 columns are copied from
;     it starting at the camera's x modulo 256, which is the camera's low byte. A
;     BLIT's source that runs off the sheet's right edge carries on from the left,
;     so the ring never has to be moved or copied: see `backdrop`
;   the ring is fed a column of tiles at a time, as the camera comes to them:
;     twelve BLITs of 8 x 8 with SHEET, from the tiles kept elsewhere on the sheet
;     (`column`). A destination doesn't wrap as a source does, so the ring is 32
;     whole tiles wide and a column never crosses its edge
;   three more layers behind the level, each one BLIT from a strip 256 wide that
;     joins up with itself: clouds at an eighth of the level's speed (and drifting),
;     the far fells at a quarter, the near hills at a half. The level's see-through
;     colour (KEY) lets them show. None of the three is in the program as a
;     picture: the NIECE draws them on the sheet at the start, the fells and hills
;     a column at a time from tables of sines (`fells`, `hills`), the trees and
;     clouds from three small pictures at several sizes
;   patterned FILL (NPATTERN): the sky is five bands of two colours mixed, paling
;     towards the fells, and the lake is stripes (`sky`). The fells and the hills
;     are mixtures too. Seven FILLs a frame and no picture of a sky anywhere
;   the short registers, NDXS, NDYS, NDWS and NDHS, wherever the number fits a
;     byte. (NDXS is signed, so a place on the right of the page, 128-159, still
;     takes both bytes of NDX: `sprset`, `pdraw`.)
;   things by BLIT: the picker in six pictures, mirrored with FLIPX to face left,
;     drawn 10 x 16 for a moment as he jumps and 14 x 12 as he lands (the same
;     12 x 14 picture: the chip squeezes it); berries that bob, lemmings in two
;     steps, midges whose one cloud is turned over by FLIPX and FLIPY
;   ink tables: table 1 is all white, and he's drawn through it every few frames
;     while he can't be hurt; a flattened lemming too. Tables 2 and 3 are the
;     evening: the second level's clouds, fells and hills are the first's, with
;     other inks, under another seven FILLs of sky
;   the queue: every command waits for room (bit 6 of NSTATUS), so none is ever
;     lost, even when a level's first 21 columns go in as 252 BLITs at once
;
; The score, the berries, the lives and the clock are ordinary text on the top
; line, in front of a black strip of the page; a line of words has a black box
; FILLed behind it.
;
; The map is the processor's business: where he can stand, what stops him, what
; a lemming turns round at (`player`, `wall`, `tileat`). He moves in 256ths of
; a pixel, gravity is less while he's rising with the key held, a press of jump
; counts for six frames before he lands and the ground for six after he's left
; it, and a duckboard holds him only from above. Flat out he runs a pixel a
; frame, a whole number, and the camera goes exactly as he does, so nothing
; judders (`camera`).
;
; Sound: the AUNT. Voice 1 the jump, voice 2 berries and bad news, voice 3 a tune.
;
; Time, at 1 MHz. Paced by the FLIP: one step of the game a frame, 50 a second;
; `work` to `endwork` is one frame's work. Walking each level from its start to
; its hut, a frame's work is 4,000 cycles of the 19,968 on average and 7,000 at
; the worst (a new column of tiles, ten things on the page and a landing all at
; once): 500 the sky, 800 moving him, 400 the backdrop's four BLITs, 600 a new
; column when there is one, up to 2,900 the berries and the creatures, 200 drawing
; him, 200 the score. The NIECE has 49,800 pixels to draw in a level's fullest
; frame, of the 79,872 it can: 16,000 of sky, 16,000 of backdrop, 15,360 of
; level, and the rest things (the title, with its words' boxes, is 51,900). It's
; done 12,500 cycles after the vertical blank, 13,000 on the title, and the queue
; is never more than 27 deep of its 64. So 50 frames a second, with two thirds of
; the processor and a third of the chip to spare. LATE counts frames that missed
; their vertical blank; the tests want 0.
;
; Twice the picture stands still: setting up the sheet at the start takes half a
; second, and unpacking a level into the map takes ten frames, with OUT TO THE
; FELLS on the screen. The frame count starts again after each (`resync`).
;
; Without the NIECE ($C14F doesn't read $17), CLOUDBERRY says so and waits.
;
; Memory:
;   $0000-$007A  variables (the fast page; the ROM's $D0-$FE untouched)
;   $0400-$07E7  the text screen: the score and the words
;   $0800-       code, tables, the words, the two levels, the pictures
;   after them   the things, and the level unpacked: 12 pages, a row each (.res)
; On the sheet, under the two pages: the fells at line 200, the clouds at 232, the
; ring at 256, the tiles at 352, the pictures of things at 384, the hills at 464.
;
; The tests (crates/unc-asm/tests/game_cloudberry.rs) find STATE, PXL, PY, CAML,
; LIVES, SCORE, MAP, ETYPE, LATE and the rest by name in the symbol table.

; ---------------------------------------------------------------- the machine
SCREEN  = $0400
FRAME   = $C006
KEYS    = $C060         ; the keyboard matrix, rows 0-8 (chapter 11)
V1      = $C0C0         ; the AUNT's three voices (chapter 12)
V2      = $C0C8
V3      = $C0D0
VOLUME  = $C0DA
NCTRL   = $C100         ; the NIECE (chapter 17)
NSTATUS = $C101
NGO     = $C103
NVADDR  = $C104
NVDATA  = $C106
NKEY    = $C107
NDX     = $C108
NDY     = $C10A
NDW     = $C10C
NDH     = $C10E
NSX     = $C110
NSY     = $C111
NSW     = $C113
NSH     = $C114
NCOLOUR = $C115
NFLAGS  = $C116
NTABLE  = $C118
NDXS    = $C119         ; the short ways in: both bytes of NDX, NDY, NDW, NDH
NDYS    = $C11A         ; with one store
NDWS    = $C11B
NDHS    = $C11C
NPATTERN = $C11D
NINK    = $C120
NIECE   = $C14F
NIECE_ID = $17
CLS     = $FF00

FILL    = 1             ; NGO's commands
BLIT    = 2
FLIP    = 4
KEY     = $01           ; NFLAGS
FLIPX   = $02
FLIPY   = $04
SHEET   = $20
WHITE   = $40           ; ink table 1: everything white
DUSK    = $80           ; ink table 2: the far fells in the evening
DARK    = $C0           ; ink table 3: the near hills in the evening
KEYC    = 4             ; the see-through colour: purple, so black can be an outline

; ---------------------------------------------------------------- tuning
TITLE   = 1             ; STATE
PLAY    = 2
DYING   = 3
CLEAR   = 4
OVER    = 5
NLEVEL  = 2
TOPY    = 4             ; the level's top line on the page: the score is above it
PW      = 6             ; the picker's box: what the map stops
PH      = 12
ACC     = $20           ; 256ths of a pixel a frame, a frame
FRIC    = $20
GRAV    = $28
JUMPV   = $02E0         ; up, at the start of a jump: 26 pixels high if SPACE is held
MAXFALL = $0300
JBUFT   = 6             ; frames a press of SPACE is remembered before landing
COYT    = 6             ; frames after the edge that a jump still counts
INVT    = 100           ; frames he can't be hurt for, after being hurt
CAMMAX  = 256 * 8 - 160 ; the camera's last place: 1,888
LEADR   = 60            ; where he is on the page when the camera has caught up,
LEADL   = 94            ; facing right and facing left: more room ahead than behind
NENT    = 128           ; things in a level, at most
E_BERRY = 1             ; ETYPE
E_LEM   = 2
E_MIDGE = 3
E_FLAT  = 4             ; a lemming that's been jumped on
SOLID   = $80           ; a tile's flags: bit 7 so that BMI finds it
ONEWAY  = $40           ; holds him up, but he can come up through it
GOAL    = $10           ; the hut's door

; ---------------------------------------------------------------- the sheet
FELLY   = 200           ; the far fells: 256 x 32, see-through above them
CLOUDY  = 232           ; the clouds: 256 x 20
RINGY   = 256           ; the ring: 256 x 96, the stretch of the level round the camera
TILEY   = 352           ; the tiles: 32 to a row, four rows
SPRY    = 384           ; the pictures of things that move
NEARY   = 464           ; the near hills: 256 x 48
NEARH   = 48

T_HUT   = 20            ; the hut's first tile: 4 x 4 of them, 20-23, 52-55 ...

P_STAND = 0             ; the pictures, as `picx` and the rest have them
P_RUNA  = 1
P_RUNB  = 2
P_RUNC  = 3
P_JUMP  = 4
P_FALL  = 5
P_BERRY = 6
P_LEM   = 7             ; and 8
P_MIDGE = 9             ; and 10
P_SPRUCE = 11
P_CLOUD = 12
P_BIRCH = 13

; ---------------------------------------------------------------- the fast page
STATE   = $00
TICK    = $01           ; frames, 16 bits
TICKH   = $02
LASTF   = $03           ; FRAME at the last `work`
LATE    = $04           ; frames that took two
LEVEL   = $05
PXF     = $06           ; where he is: 256ths, and 16 bits of pixels
PXL     = $07
PXH     = $08
PYF     = $09
PY      = $0A           ; his top line, in the level: 0-95 is on the page
VXL     = $0B           ; his speed, 8.8, signed
VXH     = $0C
VYL     = $0D
VYH     = $0E
PFACE   = $0F           ; 0 facing right, FLIPX facing left
GROUND  = $10           ; 1: something under his feet
COYOTE  = $11
JBUF    = $12
JPREV   = $13           ; SPACE, last frame
PANIM   = $14           ; where in the run
SQT     = $15           ; frames of squash or stretch left
SQK     = $16           ; 1 squashed (landing), 2 stretched (jumping)
INVUL   = $17
LIVES   = $18
INP     = $19           ; bit 0 left, bit 1 right, bit 2 jump
CAML    = $1A           ; the camera: the level's column at the page's left edge
CAMH    = $1B
CAMT    = $1C           ; CAM / 8 when the ring was last added to
CAMSPD  = $1D           ; the most the camera moves in a frame
PSX     = $1E           ; where he is on the page
OLDY    = $1F
T0      = $20           ; scratch
T1      = $21
T2      = $22
T3      = $23
T4      = $24
T5      = $25
MP      = $26           ; a row of the map (the low byte stays 0)
PTR     = $28
PT2     = $2A
EFROM   = $2C           ; the things looked at this frame: from, and up to
ETO     = $2D
EI      = $2E           ; the one in hand
ETY     = $2F
SX      = $30           ; its place on the page, 16 bits
SXH     = $31
EYV     = $32           ; and its top line in the level
SFL     = $33           ; NFLAGS for `spr`
HITS    = $34           ; 1: things touch him (0 on the title and when he's lost)
BOBT    = $35
SAFEL   = $36           ; the last place he stood: where he comes back
SAFEH   = $37
SAFEY   = $38
DT      = $39           ; frames of DYING left
SPREV   = $3A
BANT    = $3B           ; frames the level's name stays
ADIR    = $3C           ; the title's camera: 0 going right, 1 going left
CT      = $3D           ; CLEAR: 0 counting the time, 1 waiting for SPACE
OT      = $3E           ; frames of game over, 16 bits
OTH     = $3F
NBOX    = $40           ; words on the screen, and the dark boxes behind them
BOXX    = $41
BOXY    = $45
BOXW    = $49
BOXAL   = $4D           ; where the words are, to rub them out
BOXAH   = $51
BOXL    = $55
BERRIES = $59           ; two digits
NBERRY  = $5B           ; how many the level has, two digits
TIMEV   = $5D           ; three digits
TIMEC   = $60
SCORE   = $61           ; five digits
JSND    = $66           ; the sounds: frames left
JF      = $67
BSND    = $68
HSND    = $69
HF      = $6A
TUNEP   = $6B           ; the tune: which, where in it, frames to the next note
TUNEI   = $6D
TUNET   = $6E
TUNEON  = $6F
LC      = $70           ; `loadlevel`: chunk, row, column, character
LR      = $71
LCOL    = $72
LI      = $73
ECNT    = $74
RC      = $75
RN      = $76
WASG    = $77
SKYF    = $78           ; NFLAGS for the clouds and fells and for the near hills: an
HILLF   = $79           ; ink table each, for the evening
PSXH    = $7A           ; not 0: he's off the page (the camera is on its way)
FPEND   = $7B

; ---------------------------------------------------------------- start
        .org $0800
start:  LDX #$FF
        TXS
        JSR CLS
        LDA #0
        TAX
@zero:  STA $00,X               ; RESET keeps memory, so start from nothing
        INX
        CPX #FPEND
        BNE @zero
        LDA NIECE
        CMP #NIECE_ID
        BEQ @there
        LDX #M_NONE1
        JSR say
        LDX #M_NONE2
        JSR say
@wait:  JMP @wait               ; nothing to draw with: wait to be switched off
@there: LDX #M_WAIT
        JSR say
        JSR sound
        JSR setup
        JSR totitle
        LDA #FLIP               ; wait for the sheet to be drawn, and for a
        JSR go                  ; vertical blank: the first frame starts there
@drawn: BIT NSTATUS
        BMI @drawn
        LDA #1
        STA NCTRL               ; show the page
        LDA FRAME
        STA LASTF

; One frame. The queue is empty when the FLIP at its end has happened, and
; that's the vertical blank: so `work` starts there, 50 times a second, as
; long as the processor and the NIECE are both done before the next one.
main:   BIT NSTATUS
        BMI main
work:   LDA FRAME
        TAX
        SEC
        SBC LASTF
        STX LASTF
        CMP #2
        BCC @intime
        LDA LATE                ; FRAME moved on by two: a frame was shown twice
        CMP #255
        BEQ @intime
        INC LATE
@intime:
        INC TICK
        BNE @tick
        INC TICKH
@tick:  JSR sky                 ; the NIECE starts on the sky,
        LDA STATE               ; and the game moves on meanwhile
        CMP #PLAY
        BNE @n1
        JSR st_play
        JMP @flip
@n1:    CMP #TITLE
        BNE @n2
        JSR st_title
        JMP @flip
@n2:    CMP #DYING
        BNE @n3
        JSR st_dying
        JMP @flip
@n3:    CMP #CLEAR
        BNE @n4
        JSR st_clear
        JMP @flip
@n4:    JSR st_over
@flip:  JSR hud
        JSR noises
        LDA #FLIP
        JSR go
endwork:
        JMP main

; A command into the queue, when there's room for it. A is the command.
go:     BIT NSTATUS             ; bit 6 goes to V
        BVS go
        STA NGO
        RTS

; ---------------------------------------------------------------- the sky
; Everything behind the level that doesn't move: the strip the score is on,
; then six FILLs down the page, most of them a mixture of two colours
; (NPATTERN): the sky paling towards the fells, and the lake in stripes. They
; don't depend on where the camera is, so they go in first and the chip draws
; them while the processor works out where the camera is going.
sky:    LDA #0
        STA NFLAGS
        STA NPATTERN
        STA NCOLOUR             ; black
        STA NDXS
        STA NDYS
        LDA #160
        STA NDWS
        LDA #TOPY
        STA NDHS
        LDA #FILL
        JSR go
        LDX #0
@band:  LDA BANDS,X
        STA NDYS
        LDA BANDS+1,X
        STA NDHS
        LDA BANDS+2,X
        STA NCOLOUR
        LDA BANDS+3,X
        STA NPATTERN
        LDA #FILL
        JSR go
        INX
        INX
        INX
        INX
        CPX #6 * 4
        BNE @band
        RTS

; ---------------------------------------------------------------- the backdrop
; Four BLITs (and a FILL), each 160 wide from a picture 256 wide that joins
; up with itself: a source that runs off the sheet's right edge carries on
; from the left. The clouds move an eighth as fast as the level and drift by
; themselves, the far fells a quarter, the near hills a half. The fourth is
; the level: the ring, from the camera's x modulo 256, which is its low byte.
backdrop:
        LDA #0
        STA NDXS
        STA NSY+1
        LDA #160
        STA NDWS
        STA NSW
        LDA SKYF                ; the clouds and the fells: the evening's ink
        STA NFLAGS
        LDA CAMH                ; T1 = camera / 2, T2 = / 4, A = / 8
        STA T0
        LDA CAML
        LSR T0
        ROR A
        STA T1
        LSR T0
        ROR A
        STA T2
        LSR T0
        ROR A
        STA T3
        LDA TICKH               ; the clouds' drift: a pixel every 16 frames
        STA T0
        LDA TICK
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        CLC
        ADC T3
        STA NSX
        LDA #CLOUDY
        STA NSY
        LDA #6
        STA NDYS
        LDA #20
        STA NDHS
        STA NSH
        LDA #BLIT
        JSR go
        LDA T2                  ; the far fells
        STA NSX
        LDA #FELLY
        STA NSY
        LDA #28
        STA NDYS
        LDA #32
        STA NDHS
        STA NSH
        LDA #BLIT
        JSR go
        LDA #0                  ; the hillside under the near hills: a FILL
        STA NFLAGS              ; of their own mixture of two colours. A
        LDA #84                 ; pattern starts from the page's corner and
        STA NDYS                ; the hills' from wherever they've got to,
        LDA #16                 ; so when that's an odd column the pattern
        STA NDHS                ; is turned inside out to match
        LDA BANDS+26
        STA NCOLOUR
        LDA T1
        LSR A
        LDA BANDS+27
        BCC @even
        EOR #$FF
@even:  STA NPATTERN
        LDA #FILL
        JSR go
        LDA HILLF               ; the near hills
        STA NFLAGS
        LDA #1
        STA NSY+1               ; everything from here on is below line 256
        LDA T1
        STA NSX
        LDA #<NEARY
        STA NSY
        LDA #84 - NEARH
        STA NDYS
        LDA #NEARH
        STA NDHS
        STA NSH
        LDA #BLIT
        JSR go
        LDA #KEY                ; the level
        STA NFLAGS
        LDA CAML
        STA NSX
        LDA #<RINGY
        STA NSY
        LDA #TOPY
        STA NDYS
        LDA #96
        STA NDHS
        STA NSH
        LDA #BLIT
        JMP go

; ---------------------------------------------------------------- the ring
; The ring is 256 pixels of the level, kept on the sheet: 32 columns of
; tiles. Column c of the level is in the ring at x = (c AND 31) * 8, so the
; page's 160 pixels from the camera on are the ring's from CAM AND 255 on,
; round the edge and all, and nothing in the ring ever moves. The page shows
; 21 columns at most; when the camera comes to a new one, that column is
; drawn into the ring where the one 32 columns back was: twelve BLITs of 8 x 8
; from the tiles, with SHEET. A destination doesn't carry on round the
; sheet's edge as a source does, which is why a column is never drawn across
; it: 256 is 32 tiles exactly.
scroll: LDA CAMH
        STA T0
        LDA CAML
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        CMP CAMT
        BEQ @rts
        BCC @left
        STA CAMT                ; going right: the new column is the 21st
        CLC
        ADC #20
        JMP column
@left:  STA CAMT                ; going left: it's the first
        JMP column
@rts:   RTS

; Column A of the level into the ring.
column: TAX
        AND #31
        ASL A
        ASL A
        ASL A
        STA NDX                 ; up to 248: more than NDXS can say
        LDA #0
        STA NDX+1
        LDA #1
        STA NDY+1               ; the ring is at line 256
        STA NSY+1
        LDA #SHEET              ; no KEY: an empty tile rubs out what was there
        STA NFLAGS
        LDA #8
        STA NDWS
        STA NDHS
        STA NSW
        STA NSH
        .rept 12, row
        LDY MAP + row * 256,X
        LDA tilex,Y
        STA NSX
        LDA tiley,Y
        STA NSY
        LDA #row * 8
        STA NDY
        BIT NSTATUS             ; room in the queue? (bit 6 to V)
        BVS * - 3
        LDA #BLIT
        STA NGO
        .endr
        RTS

; The 21 columns the page can see, from column CAMT: at the start of a level.
ringfill:
        LDA CAMT
        STA RC
        LDA #21
        STA RN
@col:   LDA RC
        JSR column
        INC RC
        DEC RN
        BNE @col
        RTS

; ---------------------------------------------------------------- the states
st_title:
        LDA ADIR                ; the camera wanders along the level and back
        BNE @back
        INC CAML
        BNE @r1
        INC CAMH
@r1:    LDA CAMH
        CMP #>CAMMAX
        BNE @go
        LDA CAML
        CMP #<CAMMAX
        BNE @go
        INC ADIR
        BNE @go
@back:  LDA CAML
        BNE @b1
        DEC CAMH
@b1:    DEC CAML
        LDA CAML
        ORA CAMH
        BNE @go
        STA ADIR
@go:    JSR scene
        LDA #68                 ; a cloudberry, three times the size
        STA SX
        LDA #0
        STA SXH
        LDA TICK
        LSR A
        LSR A
        LSR A
        AND #7
        TAY
        LDA bob,Y
        CLC
        ADC #46
        STA T0
        LDA #KEY
        STA SFL
        LDY #P_BERRY
        JSR sprset
        LDA #24
        STA NDWS
        STA NDHS
        JSR blit
        JSR boxes
        JSR space
        BCC @rts
        JMP newgame
@rts:   RTS

st_play:
        JSR input
        JSR player
        LDA STATE
        CMP #PLAY
        BNE @other              ; he fell, or he's home
        LDA INVUL
        BEQ @i
        DEC INVUL
@i:     JSR camera
        JSR scene
        JSR pdraw
        JSR clock
        LDA BANT
        BEQ @rts
        JSR boxes
        DEC BANT
        BNE @rts
        JSR hush
@rts:   RTS
@other: JSR camera
        JSR scene
        JMP pdraw

; Lost: he hops and falls off the page, or is under it already, and after a
; moment comes back where he last stood, or it's over.
st_dying:
        LDA PY
        CMP #110
        BCS @under
        CLC                     ; falling, through everything
        LDA VYL
        ADC #GRAV
        STA VYL
        BCC @v
        INC VYH
@v:     CLC
        LDA PYF
        ADC VYL
        STA PYF
        LDA PY
        ADC VYH
        CMP #$E0                ; off the top: stay on the page
        BCC @y
        LDA #0
@y:     STA PY
@under: JSR scene
        JSR pdraw
        LDA DT
        BEQ @done
        DEC DT
        RTS
@done:  LDA PY
        CMP #104
        BCC @rts                ; still falling
        LDA LIVES
        BEQ @over
        LDA SAFEL               ; back where he last stood, and the camera
        STA PXL                 ; goes back there quickly
        LDA SAFEH
        STA PXH
        LDA SAFEY
        STA PY
        LDA #0
        STA PXF
        STA PYF
        STA VXL
        STA VXH
        STA VYL
        STA VYH
        LDA #INVT
        STA INVUL
        LDA #6
        STA CAMSPD
        LDA #1
        STA HITS
        LDA #PLAY
        STA STATE
@rts:   RTS
@over:  LDA #OVER
        STA STATE
        LDA #0
        STA OT
        STA OTH
        STA TUNEON
        JSR hush
        LDX #M_OVER
        JSR say
        LDX #M_AGAIN
        JMP say

; Home: the time left is counted into the score, ten points a second, two
; seconds a frame, and then SPACE goes on.
st_clear:
        JSR scene
        JSR pdraw
        JSR boxes
        LDA CT
        BNE @wait
        LDA #2
        STA T5
@count: LDA TIMEV
        ORA TIMEV+1
        ORA TIMEV+2
        BEQ @counted
        JSR timedown
        LDX #3                  ; ten points
        LDA #1
        JSR addsc
        LDA TICK
        AND #3
        BNE @quiet
        LDA #$60
        JSR s_blip
@quiet: DEC T5
        BNE @count
        RTS
@counted:
        INC CT
        LDX #M_SPACE
        JMP say
@wait:  JSR space
        BCC @rts
        INC LEVEL
        LDA LEVEL
        CMP #NLEVEL
        BCC @next
        JMP totitle             ; home with both pails
@next:  JMP startlevel
@rts:   RTS

st_over:
        JSR scene
        JSR boxes
        INC OT
        BNE @o
        INC OTH
@o:     LDA OTH
        CMP #2                  ; ten seconds: back to the title by itself
        BCC @sp
        JMP totitle
@sp:    JSR space
        BCC @rts
        JMP newgame
@rts:   RTS

; The level and what's in it: the ring's new column if there is one, the
; backdrop, and the things.
scene:  JSR scroll
        JSR backdrop
        JMP ents

; C set if SPACE (or RETURN) has just been pressed.
space:  LDA KEYS
        AND #1
        STA T0
        LDA KEYS+8
        AND #$10
        ORA T0
        TAX
        BEQ @no
        LDA SPREV
        BNE @no
        STX SPREV
        SEC
        RTS
@no:    STX SPREV
        CLC
        RTS

totitle:
        JSR hush
        JSR hudoff
        LDA #0
        STA LEVEL
        STA HITS
        STA ADIR
        JSR loadlevel
        JSR place
        LDA #200                ; he isn't in the picture
        STA PY
        LDA #TITLE
        STA STATE
        LDA #<tune1
        STA TUNEP
        LDA #>tune1
        STA TUNEP+1
        JSR tunestart
        LDX #M_NAME
        JSR say
        LDX #M_SUB
        JSR say
        LDX #M_PRESS
        JSR say
        LDX #M_KEYS
        JSR say
        JMP resync

newgame:
        LDA #3
        STA LIVES
        LDA #0
        STA LEVEL
        LDX #4
@sc:    STA SCORE,X
        DEX
        BPL @sc

startlevel:
        JSR hush
        LDX #M_READY
        JSR say
        JSR loadlevel
        JSR place
        LDA #0
        STA BERRIES
        STA BERRIES+1
        STA TIMEV+2
        STA TIMEC
        STA INVUL
        STA CT
        LDA #2
        STA TIMEV
        LDA #5
        STA TIMEV+1             ; 250 seconds
        LDA #1
        STA HITS
        LDA #PLAY
        STA STATE
        LDA #<tune1
        STA TUNEP
        LDA #>tune1
        STA TUNEP+1
        JSR tunestart
        JSR hush
        JSR hudline
        LDA LEVEL
        ASL A
        CLC
        ADC #M_LEV1
        TAX
        JSR say
        LDA #120
        STA BANT

; Unpacking a level takes a few frames, with the picture standing still.
; Start counting frames again from the next vertical blank.
resync: LDA FRAME
@w:     CMP FRAME
        BEQ @w
        LDA FRAME
        STA LASTF
        RTS

; He's at the start of the level, in the air, and the ring is drawn for it.
place:  LDA #0
        STA PXF
        STA PXH
        STA PYF
        STA VXL
        STA VXH
        STA VYL
        STA VYH
        STA PFACE
        STA GROUND
        STA COYOTE
        STA JBUF
        STA SQT
        STA CAML
        STA CAMH
        STA CAMT
        LDA #20
        STA PXL
        STA SAFEL
        LDA #0
        STA SAFEH
        LDA #44
        STA PY
        STA SAFEY
        LDA #2
        STA CAMSPD
        JMP ringfill

toclear:
        LDA #CLEAR
        STA STATE
        LDA #0
        STA HITS
        STA VXL
        STA VXH
        STA CT
        STA INVUL
        JSR hush
        LDX #M_HOME
        LDA LEVEL
        CMP #NLEVEL - 1
        BNE @say
        LDX #M_WON
@say:   JSR say
        LDA #<tune2
        STA TUNEP
        LDA #>tune2
        STA TUNEP+1
        JMP tunestart

; ---------------------------------------------------------------- the keys
; Held keys, from the matrix: cursor left and right or A and D; SPACE, cursor
; up or W to jump.
input:  LDA KEYS+8
        AND #%00000111          ; left, right, up
        STA INP
        LDA KEYS+4
        AND #%00000010          ; A
        BEQ @na
        LDA INP
        ORA #1
        STA INP
@na:    LDA KEYS+4
        AND #%00010000          ; D
        BEQ @nd
        LDA INP
        ORA #2
        STA INP
@nd:    LDA KEYS
        AND #%00000001          ; SPACE
        BEQ @ns
        LDA INP
        ORA #4
        STA INP
@ns:    LDA KEYS+6
        AND #%10000000          ; W
        BEQ @nw
        LDA INP
        ORA #4
        STA INP
@nw:    RTS

; ---------------------------------------------------------------- the picker
; One frame of him: the keys into speed, speed into place, and the map
; stopping him. Across first, then down, so a wall and a floor are two plain
; questions and a corner is never both at once.
player: LDA GROUND
        STA WASG
        LDA INP                 ; a press of jump is remembered for a few
        AND #4                  ; frames, so one just before landing counts
        TAX
        BEQ @held
        LDA JPREV
        BNE @held
        LDA #JBUFT
        STA JBUF
@held:  STX JPREV
        LDA JBUF
        BEQ @nob
        DEC JBUF
@nob:   LDA GROUND              ; and the ground is remembered for a few
        BEQ @air                ; frames after he's run off the end of it
        LDA #COYT
        STA COYOTE
        BNE @cj
@air:   LDA COYOTE
        BEQ @cj
        DEC COYOTE
@cj:    LDA JBUF
        BEQ @nojump
        LDA COYOTE
        BEQ @nojump
        LDA #<[-JUMPV]
        STA VYL
        LDA #>[-JUMPV]
        STA VYH
        LDA #0
        STA JBUF
        STA COYOTE
        STA GROUND
        LDA #2                  ; taller and thinner as he goes up
        STA SQK
        LDA #6
        STA SQT
        JSR s_jump
@nojump:
        LDA INP
        AND #3
        CMP #2
        BEQ @right
        CMP #1
        BEQ @left
        LDA VXH                 ; neither, or both: slow down
        BMI @fneg
        ORA VXL
        BEQ @hdone
        SEC
        LDA VXL
        SBC #FRIC
        STA VXL
        LDA VXH
        SBC #0
        STA VXH
        BPL @hdone
        BMI @zero
@fneg:  CLC
        LDA VXL
        ADC #FRIC
        STA VXL
        LDA VXH
        ADC #0
        STA VXH
        BMI @hdone
@zero:  LDA #0
        STA VXL
        STA VXH
        BEQ @hdone
@right: LDA #0
        STA PFACE
        CLC
        LDA VXL
        ADC #ACC
        STA VXL
        LDA VXH
        ADC #0
        STA VXH
        BMI @hdone
        BEQ @hdone
        LDA #0                  ; a pixel a frame and no more: at a whole
        STA VXL                 ; number of pixels the scroll doesn't judder
        LDA #1
        STA VXH
        BNE @hdone
@left:  LDA #FLIPX
        STA PFACE
        SEC
        LDA VXL
        SBC #ACC
        STA VXL
        LDA VXH
        SBC #0
        STA VXH
        BPL @hdone
        CMP #$FF
        BEQ @hdone
        LDA #0
        STA VXL
        LDA #$FF
        STA VXH
@hdone: CLC                     ; across
        LDA PXF
        ADC VXL
        STA PXF
        LDA PXL
        ADC VXH
        STA PXL
        LDA VXH
        BMI @xneg
        LDA PXH
        ADC #0
        STA PXH
        LDA VXH
        ORA VXL
        BEQ @hcd                ; standing still
        CLC                     ; going right: his right edge
        LDA PXL
        ADC #PW - 1
        STA T1
        LDA PXH
        ADC #0
        STA T2
        JSR wall
        BCC @hcd
        LDA T1                  ; back to the left of that column
        AND #$F8
        SEC
        SBC #PW
        STA PXL
        LDA T2
        SBC #0
        STA PXH
        JMP @stopx
@xneg:  LDA PXH
        ADC #$FF
        STA PXH
        LDA PXL                 ; going left: his left edge
        STA T1
        LDA PXH
        STA T2
        JSR wall
        BCC @hcd
        LDA PXL                 ; on to the right of that column
        AND #$F8
        CLC
        ADC #8
        STA PXL
        BCC @stopx
        INC PXH
@stopx: LDA #0
        STA PXF
        STA VXL
        STA VXH
@hcd:   LDA VYH                 ; down. Gravity: three times as much while
        BPL @g1                 ; he's going up with the key let go, so a
        LDA INP                 ; tap is a hop and a held key a leap
        AND #4
        BNE @g1
        LDA #GRAV * 3
        BNE @g2
@g1:    LDA #GRAV
@g2:    CLC
        ADC VYL
        STA VYL
        BCC @g3
        INC VYH
@g3:    LDA VYH
        BMI @vok
        CMP #>MAXFALL
        BCC @vok
        LDA #>MAXFALL
        STA VYH
        LDA #0
        STA VYL
@vok:   LDA PY
        STA OLDY
        CLC
        LDA PYF
        ADC VYL
        STA PYF
        LDA PY
        ADC VYH
        STA PY
        LDA #0
        STA GROUND
        JSR cols                ; T4, T5: the columns his two edges are in
        LDA VYH
        BPL @down
        LDA PY                  ; going up: his head
        CMP #$E0
        BCC @head
        LDA #0                  ; the top of the level
        STA PY
        BEQ @bonk
@head:  LSR A
        LSR A
        LSR A
        STA T3
        TAX
        LDY T4
        JSR tileat
        BMI @bonkt
        LDX T3
        LDY T5
        JSR tileat
        BPL @vdone
@bonkt: LDA PY
        AND #$F8
        CLC
        ADC #8
        STA PY
@bonk:  LDA #0
        STA VYL
        STA VYH
        STA PYF
        JMP @vdone
@down:  CLC                     ; going down, or standing: the line under
        LDA PY                  ; his feet
        ADC #PH
        STA T1
        LSR A
        LSR A
        LSR A
        STA T3
        TAX
        LDY T4
        JSR tileat
        JSR holds
        BCS @land
        LDX T3
        LDY T5
        JSR tileat
        JSR holds
        BCC @vdone
@land:  LDA T1                  ; on top of that row
        AND #$F8
        SEC
        SBC #PH
        STA PY
        LDA WASG
        BNE @soft
        LDA VYH                 ; he came down fast: shorter and wider
        CMP #2
        BCC @soft
        LDA #1
        STA SQK
        LDA #6
        STA SQT
        JSR s_land
@soft:  LDA #0
        STA VYL
        STA VYH
        STA PYF
        LDA #1
        STA GROUND
@vdone: LDA PY
        CMP #104
        BCC @inlevel
        JMP fell                ; under the page
@inlevel:
        LDA GROUND
        BEQ @goal
        LDA TICK                ; somewhere to come back to
        AND #7
        BNE @goal
        LDA PXL
        STA SAFEL
        LDA PXH
        STA SAFEH
        LDA PY
        STA SAFEY
@goal:  LDA VXL                 ; the run: a step every pixel he moves
        ORA VXH
        BEQ @door
        INC PANIM
@door:  LDA GROUND              ; standing with his middle in the hut's door?
        BEQ @rts
        CLC
        LDA PXL
        ADC #3
        STA T1
        LDA PXH
        ADC #0
        STA T0
        LDA T1
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        TAY
        CLC
        LDA PY
        ADC #6
        LSR A
        LSR A
        LSR A
        TAX
        JSR tileat
        AND #GOAL
        BEQ @rts
        JMP toclear
@rts:   RTS

; Does the tile whose flags are in A hold him up? T1 is the line under his
; feet. A plank does only if his feet were above it a frame ago: on the way
; up he goes through. C set if it holds.
holds:  BMI @yes
        AND #ONEWAY
        BEQ @no
        LDA T1
        AND #$F8
        STA T0
        CLC
        LDA OLDY
        ADC #PH
        CMP T0
        BEQ @yes
        BCC @yes
@no:    CLC
        RTS
@yes:   SEC
        RTS

; Is there a wall in the column of the level's pixel T2:T1, at his head, his
; middle or his feet? C set if so.
wall:   LDA T2
        STA T3
        LDA T1
        LSR T3
        ROR A
        LSR T3
        ROR A
        LSR T3
        ROR A
        STA T4
        LDA PY
        JSR @at
        BMI @hit
        CLC
        LDA PY
        ADC #6
        JSR @at
        BMI @hit
        CLC
        LDA PY
        ADC #PH - 1
        JSR @at
        BMI @hit
        CLC
        RTS
@hit:   SEC
        RTS
@at:    LSR A
        LSR A
        LSR A
        TAX
        LDY T4

; The flags of the tile in row X, column Y of the map, in A and in N and Z.
; Above and below the map there's nothing.
tileat: CPX #12
        BCS @none
        LDA rowhi,X
        STA MP+1
        LDA (MP),Y
        TAX
        LDA tflag,X
        RTS
@none:  LDA #0
        RTS

; T4 and T5: the columns of the map his left and right edges are in.
cols:   LDA PXH
        STA T0
        LDA PXL
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        STA T4
        CLC
        LDA PXL
        ADC #PW - 1
        TAX
        LDA PXH
        ADC #0
        STA T0
        TXA
        LSR T0
        ROR A
        LSR T0
        ROR A
        LSR T0
        ROR A
        STA T5
        RTS

; Into the bog, or off the bottom of the fell.
fell:   JSR s_hurt
        LDA #0
        STA HITS
        STA VXL
        STA VXH
        LDA #120                ; under the page, and staying there
        STA PY
        LDA #30
        STA DT
        LDA LIVES
        BEQ @none
        DEC LIVES
@none:  LDA #DYING
        STA STATE
        RTS

; A lemming's teeth or a cloud of midges. He's knocked back, away from it,
; and flashes; with no lives left he's knocked off the page.
hurt:   JSR s_hurt
        DEC LIVES
        BEQ @last
        LDA #INVT
        STA INVUL
        LDA #0
        STA GROUND
        STA VXL
        STA VYL
        LDA #$FE
        STA VYH
        LDX #$FF                ; it's to his right: he goes left
        LDA SX
        CMP PSX
        BCS @kx
        LDX #1
@kx:    STX VXH
        RTS
@last:  LDA #0
        STA HITS
        STA VXL
        STA VXH
        STA VYL
        LDA #$FD
        STA VYH
        LDA #40
        STA DT
        LDA #DYING
        STA STATE
        RTS

; ---------------------------------------------------------------- the camera
; The camera wants him a little behind the middle of the page, so there's
; more to see ahead of him than behind, and goes there at two pixels a frame
; at most. Once it's there it moves exactly as he does, a pixel for a pixel,
; so he stands still on the page and the level goes by under him: no
; rounding of one against the other, and nothing to judder.
camera: SEC
        LDA PFACE
        BNE @fl
        LDA PXL
        SBC #LEADR
        JMP @hi
@fl:    LDA PXL
        SBC #LEADL
@hi:    STA T0
        LDA PXH
        SBC #0
        STA T1
        BPL @pos
        LDA #0                  ; the level's left end
        STA T0
        STA T1
@pos:   LDA T0
        CMP #<CAMMAX
        LDA T1
        SBC #>CAMMAX
        BCC @in
        LDA #<CAMMAX            ; and its right end
        STA T0
        LDA #>CAMMAX
        STA T1
@in:    SEC                     ; how far it has to go
        LDA T0
        SBC CAML
        STA T2
        LDA T1
        SBC CAMH
        BMI @goleft
        BNE @far                ; more than 255 to the right
        LDA T2
        BEQ @there
        CMP CAMSPD
        BCC @add
@far:   LDA CAMSPD
@add:   CLC
        ADC CAML
        STA CAML
        BCC @done
        INC CAMH
        BCS @done
@goleft:
        CMP #$FF
        BNE @farl
        LDA T2
        EOR #$FF
        CLC
        ADC #1                  ; how far, as a positive number
        BEQ @farl               ; (256 exactly)
        CMP CAMSPD
        BCC @sub
@farl:  LDA CAMSPD
@sub:   STA T2
        SEC
        LDA CAML
        SBC T2
        STA CAML
        BCS @done
        DEC CAMH
        BCC @done
@there: LDA #2                  ; caught up: back to its usual speed
        STA CAMSPD
@done:  SEC
        LDA PXL
        SBC CAML
        STA PSX
        LDA PXH
        SBC CAMH
        STA PSXH
        RTS

; ---------------------------------------------------------------- drawing him
; One BLIT, 12 x 14 from the picture for what he's doing, mirrored when he
; faces left, and through the white ink table every few frames while he
; can't be hurt. For a few frames after a jump starts he's drawn 10 x 16, and
; after a hard landing 14 x 12: the same picture, squeezed by the NIECE.
pdraw:  LDA PSXH
        BNE @off
        LDA PY
        CMP #104
        BCC @on
@off:   RTS
@on:    LDA #KEY
        ORA PFACE
        STA SFL
        LDA INVUL
        BEQ @nf
        LDA TICK
        AND #4
        BEQ @nf
        LDA SFL
        ORA #WHITE
        STA SFL
@nf:    LDY #P_STAND
        LDA STATE
        CMP #DYING
        BEQ @fall
        LDA GROUND
        BNE @onground
        LDY #P_JUMP
        LDA VYH
        BMI @pic
@fall:  LDY #P_FALL
        BNE @pic
@onground:
        LDA VXL
        ORA VXH
        BEQ @pic
        LDA PANIM
        LSR A
        LSR A
        AND #3
        TAX
        LDY runseq,X
@pic:   LDA picx,Y
        STA NSX
        LDA picy,Y
        STA NSY
        LDA #12
        STA NSW
        LDA #14
        STA NSH
        LDA SFL
        STA NFLAGS
        LDX #0                  ; as he is
        LDA SQT
        BEQ @size
        DEC SQT
        LDX SQK
@size:  LDA pdw,X
        STA NDWS
        LDA pdh,X
        STA NDHS
        CLC
        LDA PY
        ADC pdy,X
        STA NDYS
        SEC
        LDA PSX
        SBC pdx,X
        STA NDX                 ; up to 157, more than NDXS can say
        LDA #0
        SBC #0
        STA NDX+1
        JMP blit

; ---------------------------------------------------------------- the things
; Berries, lemmings and midges are kept in the order the level has them, left
; to right by the 256 pixels they start in, and EFIRST says where each 256
; begins. Only the ones that start in the camera's 256, the one before or the
; one after are looked at: thirty at most, of which the page shows a dozen.
ents:   LDA TICK
        LSR A
        LSR A
        STA BOBT
        LDX CAMH
        LDA EFIRST+2,X
        STA ETO
        TXA
        BEQ @c0
        DEX
@c0:    LDA EFIRST,X
        TAX
        CPX ETO
        BCS @rts
@loop:  LDA ETYPE,X
        BEQ @next
        SEC
        LDA EXL,X
        SBC CAML
        STA SX
        LDA EXH,X
        SBC CAMH
        BEQ @on
        CMP #$FF
        BNE @next
        LDA SX                  ; off the left edge: by less than its width?
        CMP #$F4
        BCC @next
        LDA #$FF
        BNE @vis
@on:    LDA SX
        CMP #160
        BCS @next
        LDA #0
@vis:   STA SXH
        STX EI
        LDA ETYPE,X
        STA ETY
        CMP #E_BERRY
        BNE @n1
        JSR berry
        JMP @back
@n1:    CMP #E_LEM
        BNE @n2
        JSR lemming
        JMP @back
@n2:    CMP #E_MIDGE
        BNE @n3
        JSR midge
        JMP @back
@n3:    JSR flat
@back:  LDX EI
@next:  INX
        CPX ETO
        BCC @loop
@rts:   RTS

; Is thing ETY, at SX on the page and EYV in the level, touching him? C clear
; if it is. One subtraction and one comparison each way: the difference of
; the two left edges, moved up by the thing's width, is under the sum of the
; widths exactly when the boxes overlap.
touch:  LDA HITS
        BEQ @no
        LDA SXH
        ORA PSXH
        BNE @no
        LDY ETY
        SEC
        LDA SX
        SBC PSX
        CLC
        ADC hxo,Y
        CMP hxs,Y
        BCS @no
        SEC
        LDA EYV
        SBC PY
        CLC
        ADC hyo,Y
        CMP hys,Y
        RTS
@no:    SEC
        RTS

; A cloudberry, bobbing. Touched, it's picked.
berry:  TXA
        CLC
        ADC BOBT
        AND #7
        TAY
        LDA bob,Y
        CLC
        ADC EY,X
        STA EYV
        JSR touch
        BCS @draw
        LDX EI
        LDA #0
        STA ETYPE,X
        INC BERRIES+1
        LDA BERRIES+1
        CMP #10
        BNE @sc
        LDA #0
        STA BERRIES+1
        INC BERRIES
@sc:    LDX #3                  ; fifty points
        LDA #5
        JSR addsc
        JMP s_berry
@draw:  CLC
        LDA EYV
        ADC #TOPY
        STA T0
        LDA #KEY
        STA SFL
        LDY #P_BERRY
        JMP spr

; A lemming walks a pixel every other frame, and turns round at a wall or
; where the ground stops. Land on it and it's flat; walk into it and it bites.
lemming:
        TXA
        EOR TICK
        AND #1
        BEQ @step               ; half of them move on each frame
        JMP @still
@step:  LDA ES,X
        BNE @left
        CLC                     ; the pixel in front of its nose
        LDA EXL,X
        ADC #12
        STA T1
        LDA EXH,X
        ADC #0
        JMP @look
@left:  SEC
        LDA EXL,X
        SBC #1
        STA T1
        LDA EXH,X
        SBC #0
@look:  STA T3
        LDA T1
        LSR T3
        ROR A
        LSR T3
        ROR A
        LSR T3
        ROR A
        STA T4
        LDA EY,X
        LSR A
        LSR A
        LSR A
        STA T5
        TAX
        LDY T4
        JSR tileat
        BMI @turn               ; a wall
        LDX T5
        INX
        LDY T4
        JSR tileat
        AND #SOLID | ONEWAY
        BEQ @turn               ; nothing to walk on
        LDX EI
        LDA ES,X
        BNE @wl
        CLC
        LDA EXL,X
        ADC #1
        STA EXL,X
        BCC @still
        LDA EXH,X
        ADC #0
        STA EXH,X
        BNE @still
@wl:    SEC
        LDA EXL,X
        SBC #1
        STA EXL,X
        BCS @still
        LDA EXH,X
        SBC #0
        STA EXH,X
        JMP @still
@turn:  LDX EI
        LDA ES,X
        EOR #FLIPX
        STA ES,X
@still: LDX EI
        LDA EY,X
        STA EYV
        JSR touch
        BCS @draw
        LDA VYH                 ; is he coming down on it?
        BMI @bite
        ORA VYL
        BEQ @bite
        CLC
        LDA PY
        ADC #6
        CMP EYV
        BCS @bite
        LDX EI                  ; flat, and he bounces
        LDA #E_FLAT
        STA ETYPE,X
        LDA #14
        STA ES,X
        LDA #$FE
        STA VYH
        LDA #0
        STA VYL
        LDX #2                  ; a hundred points
        LDA #1
        JSR addsc
        JMP s_stomp
@bite:  LDA INVUL
        BNE @draw
        JSR hurt
@draw:  LDX EI
        LDA ES,X
        ORA #KEY
        STA SFL
        TXA
        ASL A
        ASL A
        ASL A
        CLC
        ADC TICK
        LSR A
        LSR A
        LSR A
        AND #1
        CLC
        ADC #P_LEM
        TAY
        CLC
        LDA EYV
        ADC #TOPY
        STA T0
        JMP spr

; A flat lemming: its picture four lines high, white, for a few frames.
flat:   LDA ES,X
        SEC
        SBC #1
        STA ES,X
        BNE @draw
        STA ETYPE,X
        RTS
@draw:  LDA #KEY | WHITE
        STA SFL
        CLC
        LDA EY,X
        ADC #TOPY
        STA T0
        LDY #P_LEM
        JSR sprset
        CLC
        LDA T0
        ADC #5
        STA NDYS
        LDA #3
        STA NDHS
        JMP blit

; A cloud of midges hangs where it is, swaying on two sines, and can't be
; jumped on. Its one picture is drawn a different way up every other frame,
; which is all the swarming there is.
midge:  TXA
        ASL A
        ASL A
        ASL A
        STA T0
        LDA TICK
        LSR A
        CLC
        ADC T0
        AND #31
        TAY
        LDA hover,Y
        CLC
        ADC EY,X
        STA EYV
        LDA TICK
        LSR A
        LSR A
        SEC
        SBC T0
        AND #31
        TAY
        LDA hover,Y
        ASL A
        BMI @neg
        CLC
        ADC SX
        STA SX
        BCC @t
        INC SXH
        JMP @t
@neg:   CLC
        ADC SX
        STA SX
        BCS @t
        DEC SXH
@t:     JSR touch
        BCS @draw
        LDA INVUL
        BNE @draw
        JSR hurt
@draw:  LDA TICK
        AND #FLIPX | FLIPY      ; mirrored, upside down, both, neither
        ORA #KEY
        STA SFL
        LDA TICK
        LSR A
        LSR A
        LSR A
        AND #1
        CLC
        ADC #P_MIDGE
        TAY
        CLC
        LDA EYV
        ADC #TOPY
        STA T0

; Picture Y at SX across and T0 down the page, with SFL for NFLAGS.
spr:    JSR sprset
blit:   BIT NSTATUS             ; room in the queue?
        BVS blit
        LDA #BLIT
        STA NGO
        RTS
sprset: LDA SX
        STA NDX
        LDA SXH
        STA NDX+1
        LDA T0
        STA NDYS
        LDA picx,Y
        STA NSX
        LDA picy,Y
        STA NSY
        LDA picw,Y
        STA NSW
        STA NDWS
        LDA pich,Y
        STA NSH
        STA NDHS
        LDA SFL
        STA NFLAGS
        RTS

; ---------------------------------------------------------------- the score
; A to digit X of the score (0 the ten thousands, 4 the ones), carrying.
addsc:  CLC
        ADC SCORE,X
        STA SCORE,X
@carry: CMP #10
        BCC @rts
        SBC #10
        STA SCORE,X
        DEX
        BMI @rts
        LDA SCORE,X
        ADC #0                  ; C is set: one more
        STA SCORE,X
        BNE @carry
@rts:   RTS

; A second off the clock every fifty frames.
clock:  INC TIMEC
        LDA TIMEC
        CMP #50
        BCC clockrts
        LDA #0
        STA TIMEC
timedown:
        LDX #2
@d:     LDA TIMEV,X
        BNE @dec
        DEX
        BPL @d
        RTS
@dec:   SEC
        SBC #1
        STA TIMEV,X
@nines: INX
        CPX #3
        BCS clockrts
        LDA #9
        STA TIMEV,X
        BNE @nines
clockrts:
        RTS

; The top line of the text screen, in front of the page's black strip.
hud:    LDA STATE
        CMP #TITLE
        BEQ @rts
        LDA BERRIES
        ORA #'0'
        STA SCREEN+9
        LDA BERRIES+1
        ORA #'0'
        STA SCREEN+10
        LDA NBERRY
        ORA #'0'
        STA SCREEN+12
        LDA NBERRY+1
        ORA #'0'
        STA SCREEN+13
        LDA LIVES
        ORA #'0'
        STA SCREEN+22
        LDX #2
@t:     LDA TIMEV,X
        ORA #'0'
        STA SCREEN+30,X
        DEX
        BPL @t
        LDX #4
@s:     LDA SCORE,X
        ORA #'0'
        STA SCREEN+35,X
        DEX
        BPL @s
@rts:   RTS

hudline:
        LDX #39
@c:     LDA hudtxt,X
        STA SCREEN,X
        DEX
        BPL @c
        RTS
hudoff: LDX #39
        LDA #' '
@c:     STA SCREEN,X
        DEX
        BPL @c
        RTS

; ---------------------------------------------------------------- words
; Message X (one of the M_ numbers) onto the text screen, and a dark box
; behind it on the page from now on: the text is in front of the page, and
; white on a pale sky wants something behind it.
say:    LDA msgs,X
        STA PTR
        LDA msgs+1,X
        STA PTR+1
        LDY #0
        LDA (PTR),Y             ; the row
        TAX
        LDA scrlo,X
        STA PT2
        LDA scrhi,X
        STA PT2+1
        TXA
        ASL A
        ASL A
        SEC
        SBC #1
        LDX NBOX
        STA BOXY,X
        INY
        LDA (PTR),Y             ; the column
        CLC
        ADC PT2
        STA PT2
        STA BOXAL,X
        LDA PT2+1
        ADC #0
        STA PT2+1
        STA BOXAH,X
        LDA (PTR),Y             ; a letter is four of the page's pixels wide
        ASL A
        ASL A
        SEC
        SBC #2
        STA BOXX,X
        INY
        LDA (PTR),Y             ; how many letters
        STA BOXL,X
        STA T0
        ASL A
        ASL A
        CLC
        ADC #4
        STA BOXW,X
        INC NBOX
        CLC
        LDA PTR
        ADC #3
        STA PTR
        BCC @same
        INC PTR+1
@same:  LDY #0
@c:     LDA (PTR),Y
        STA (PT2),Y
        INY
        CPY T0
        BNE @c
        RTS

; The words off the screen again, and their boxes with them.
hush:   LDX NBOX
        BEQ @rts
        DEX
        LDA BOXAL,X
        STA PT2
        LDA BOXAH,X
        STA PT2+1
        LDY BOXL,X
        LDA #' '
@c:     DEY
        STA (PT2),Y
        BNE @c
        STX NBOX
        JMP hush
@rts:   RTS

; The boxes: a FILL each, six lines high.
boxes:  LDX NBOX
        BEQ @rts
        LDA #0
        STA NFLAGS
        STA NPATTERN
        STA NCOLOUR
        LDA #6
        STA NDHS
@b:     DEX
        LDA BOXX,X
        STA NDXS
        LDA BOXY,X
        STA NDYS
        LDA BOXW,X
        STA NDWS
        LDA #FILL
        JSR go
        TXA
        BNE @b
@rts:   RTS

; ---------------------------------------------------------------- sound
; Voice 1 is his: the jump, a note sliding up. Voice 2 is what happens to
; him: a berry's two high notes, a lemming's squeak, the bad news sliding
; down. Voice 3 plays the tune. All three die away by themselves (sustain
; 0), so a sound is a pitch and the gate off and on.
sound:  LDA #15
        STA VOLUME
        LDA #128
        STA V1+2
        STA V2+2
        LDA #48
        STA V3+2
        LDA #4                  ; pulse
        STA V1+3
        LDA #1                  ; triangle
        STA V2+3
        LDA #4
        STA V3+3
        LDA #$07
        STA V1+5
        LDA #$08
        STA V2+5
        LDA #$09
        STA V3+5
        LDA #0
        STA V1+6
        STA V2+6
        STA V3+6
        LDA #6                  ; the tune stays under the rest
        STA V3+7
        RTS

s_jump: LDA #$10
        LDX #7
slide:  STA JF
        STA V1+1
        STX JSND
        LDA #0
        STA V1
        STA V1+4
        LDA #1
        STA V1+4
        RTS
s_land: LDA #$06
        LDX #2
        BNE slide
s_stomp:
        LDA #$30
        LDX #5
        BNE slide

s_berry:
        LDA #5
        STA BSND
        LDA #$56
s_blip: STA V2+1
        LDA #0
        STA HSND
        STA V2
        STA V2+4
        LDA #1
        STA V2+3
        STA V2+4
        RTS

s_hurt: LDA #16
        STA HSND
        LDA #0
        STA BSND
        STA V2+4
        LDA #$28
        STA HF
        STA V2+1
        LDA #2                  ; a sawtooth
        STA V2+3
        LDA #1
        STA V2+4
        RTS

; Once a frame: the slides, the berry's second note, and the tune's next
; note when it's due.
noises: LDA JSND
        BEQ @nj
        DEC JSND
        LDA JF
        CLC
        ADC #3
        STA JF
        STA V1+1
@nj:    LDA BSND
        BEQ @nb
        DEC BSND
        BNE @nb
        LDA #$73
        JSR s_blip
@nb:    LDA HSND
        BEQ @nh
        DEC HSND
        LDA HF
        SEC
        SBC #2
        STA HF
        STA V2+1
@nh:    LDA TUNEON
        BEQ @rts
        DEC TUNET
        BNE @rts
        LDA #9
        STA TUNET
        LDY TUNEI
        LDA (TUNEP),Y
        CMP #$FE
        BCC @note
        BEQ @stop
        LDY #0                  ; $FF: round again
        LDA (TUNEP),Y
@note:  INY
        STY TUNEI
        TAX
        LDA #0
        STA V3+4
        TXA
        BEQ @rts                ; a rest
        LDA notelo-1,X
        STA V3
        LDA notehi-1,X
        STA V3+1
        LDA #1
        STA V3+4
@rts:   RTS
@stop:  LDA #0
        STA TUNEON
        RTS

tunestart:
        LDA #0
        STA TUNEI
        LDA #1
        STA TUNET
        STA TUNEON
        RTS

; ---------------------------------------------------------------- a level
; A level is 256 columns of 12 tiles, written at the end of this file as
; eight pieces 32 columns wide, a character a tile. It's unpacked into MAP, a
; byte a tile and a page of memory a row, so the tile at (column, row) is one
; indexed load; the berries and the creatures come out of it into the list
; of things as they're met.
loadlevel:
        LDA #>MAP
        STA MP+1
        LDA #0
        STA MP
        TAY
        LDX #12
@clr:   STA (MP),Y
        INY
        BNE @clr
        INC MP+1
        DEX
        BNE @clr
@ce:    STA ETYPE,X
        INX
        BPL @ce
        STA ECNT
        STA NBERRY
        STA NBERRY+1
        LDA LEVEL
        ASL A
        TAX
        LDA levtab,X
        STA PTR
        LDA levtab+1,X
        STA PTR+1
        LDA LEVEL               ; its sky, and its fells' ink
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        TAX
        LDY #0
@sky:   LDA skies,X
        STA BANDS,Y
        INX
        INY
        CPY #28
        BNE @sky
        LDA skies,X
        STA SKYF
        LDA skies+1,X
        STA HILLF
        LDA #0
        STA LC
@chunk: LDX LC
        LDA ECNT
        STA EFIRST,X
        LDA #0
        STA LR
@row:   LDX LR
        LDA rowhi,X
        STA MP+1
        LDA LC
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        STA LCOL
        LDA #0
        STA LI
@cell:  LDY LI
        LDA (PTR),Y
        TAX
        LDA TRANS,X
        BEQ @nextc
        BMI @thing
        LDY LCOL
        STA (MP),Y
@nextc: INC LCOL
        INC LI
        LDA LI
        CMP #32
        BNE @cell
        CLC
        LDA PTR
        ADC #32
        STA PTR
        BCC @s
        INC PTR+1
@s:     INC LR
        LDA LR
        CMP #12
        BNE @row
        INC LC
        LDA LC
        CMP #8
        BNE @chunk
        LDA ECNT
        STA EFIRST+8
        STA EFIRST+9
        RTS
@thing: CMP #$FF
        BEQ @hut
        AND #$7F
        LDX ECNT
        STA ETYPE,X
        CMP #E_BERRY
        BNE @nb
        INC NBERRY+1
        LDA NBERRY+1
        CMP #10
        BNE @nb
        LDA #0
        STA NBERRY+1
        INC NBERRY
@nb:    LDA LCOL
        ASL A
        ASL A
        ASL A
        STA EXL,X
        LDA LCOL
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        STA EXH,X
        LDA LR
        ASL A
        ASL A
        ASL A
        STA EY,X
        LDA #0
        STA ES,X
        INC ECNT
        JMP @nextc
@hut:   LDA #T_HUT              ; the hut: sixteen tiles from its top left
        STA T0
        LDX LR
        LDA #4
        STA T1
@hrow:  LDA rowhi,X
        STA MP+1
        LDY LCOL
        LDA T0
        STA (MP),Y
        INY
        CLC
        ADC #1
        STA (MP),Y
        INY
        ADC #1
        STA (MP),Y
        INY
        ADC #1
        STA (MP),Y
        CLC
        LDA T0
        ADC #32
        STA T0
        INX
        DEC T1
        BNE @hrow
        LDX LR
        LDA rowhi,X
        STA MP+1
        JMP @nextc

; ---------------------------------------------------------------- setting up the sheet
; Once, at the start: the tiles and the pictures go in through NVDATA, and
; the NIECE draws the three backdrops itself, on the sheet.
setup:  LDA #KEYC
        STA NKEY
        LDA #1                  ; ink table 1: every colour white
        STA NTABLE
        LDX #15
@white: STA NINK,X
        DEX
        BPL @white
        LDA #2                  ; table 2: the evening's colours
        STA NTABLE
        LDX #15
@dusk:  LDA duskink,X
        STA NINK,X
        DEX
        BPL @dusk
        LDA #3                  ; table 3: and its shadows
        STA NTABLE
        LDX #15
@dark:  LDA darkink,X
        STA NINK,X
        DEX
        BPL @dark
        LDA #0
        TAX
@tr:    STA TRANS,X             ; which tile, or which thing, a character of
        INX                     ; a level stands for
        BNE @tr
@pair:  LDY chars,X
        BEQ @paired
        LDA chars+1,X
        STA TRANS,Y
        INX
        INX
        BNE @pair
@paired:
        LDA #SHEET              ; the tiles' and the pictures' part of the sheet
        STA NFLAGS              ; starts see-through: tile 0 is nothing at all
        LDA #0
        STA NPATTERN
        STA NDX
        STA NDX+1
        STA NDW
        LDA #1
        STA NDW+1
        STA NDY+1
        LDA #<TILEY
        STA NDY
        LDA #NEARY - TILEY
        STA NDHS
        LDA #KEYC
        STA NCOLOUR
        LDA #FILL
        JSR go
@filled:
        BIT NSTATUS             ; NVDATA doesn't wait its turn in the queue,
        BMI @filled             ; so wait for the FILL before writing over it
        JSR upload
        JSR fells
        JSR hills
        JMP clouds

; The pictures: each is its width and height, where it goes (x, and the line
; less 256), and then its rows, a character a pixel: a hex digit for a
; colour, and a dot for the see-through one.
upload: LDA #<pics
        STA PTR
        LDA #>pics
        STA PTR+1
@pic:   LDY #0
        LDA (PTR),Y
        BNE @more
        RTS
@more:  LSR A
        STA T0                  ; bytes a row
        INY
        LDA (PTR),Y
        STA T1                  ; rows
        INY
        LDA (PTR),Y
        LSR A
        STA T2
        INY
        LDA (PTR),Y             ; the line, less 256: 128 bytes a line, and
        LSR A                   ; line 256 is byte $8000
        ORA #$80
        STA T4
        LDA #0
        ROR A
        ORA T2
        STA T3
        CLC
        LDA PTR
        ADC #4
        STA PTR
        BCC @row
        INC PTR+1
@row:   LDA T3
        STA NVADDR
        LDA T4
        STA NVADDR+1
        LDY #0
        LDX T0
@px:    LDA (PTR),Y
        JSR nibble
        ASL A
        ASL A
        ASL A
        ASL A
        STA T5
        INY
        LDA (PTR),Y
        JSR nibble
        ORA T5
        STA NVDATA              ; two pixels, and NVADDR moves on
        INY
        DEX
        BNE @px
        TYA
        CLC
        ADC PTR
        STA PTR
        BCC @same
        INC PTR+1
@same:  CLC
        LDA T3
        ADC #128
        STA T3
        BCC @next
        INC T4
@next:  DEC T1
        BNE @row
        JMP @pic

; A character of a picture to its colour.
nibble: CMP #'.'
        BEQ @key
        CMP #'A'
        BCC @digit
        SBC #'A' - 10
        RTS
@digit: AND #$0F
        RTS
@key:   LDA #KEYC
        RTS

; The far fells: two ridges, drawn a column at a time from two tables of
; heights the assembler worked out from sines (so they join up after 256),
; each column a FILL one pixel wide with SHEET. The far ridge is a mixture of
; light blue and grey, with what's left of the snow on its tops; the nearer
; one is grey and green.
fells:  LDA #SHEET
        STA NFLAGS
        LDA #0
        STA NPATTERN
        STA NDX
        STA NDX+1
        STA NDY+1
        STA NDW
        LDA #1
        STA NDW+1               ; 256 wide
        LDA #FELLY
        STA NDY
        LDA #52
        STA NDHS
        LDA #KEYC               ; the fells' and the clouds' strips start
        STA NCOLOUR             ; see-through
        LDA #FILL
        JSR go
        LDA #1
        STA NDWS
        LDX #0
@col:   STX NDX
        LDA fellh,X
        STA NDHS
        STA T0
        SEC
        LDA #FELLY + 32
        SBC T0
        STA NDY
        LDA #$CE                ; grey among light blue
        STA NCOLOUR
        LDA #%10100101
        STA NPATTERN
        LDA #FILL
        JSR go
        LDA T0                  ; snow on what's over 22 high: half of the
        SEC                     ; height over that, ragged at its edge
        SBC #22
        BCC @nosnow
        LSR A
        BEQ @nosnow
        STA NDHS
        LDA #$E1
        STA NCOLOUR
        LDA #%00000000
        STA NPATTERN
        LDA #FILL
        JSR go
        CLC
        LDA NDY
        ADC NDHS
        STA NDY
        LDA #2
        STA NDHS
        LDA #%10100101
        STA NPATTERN
        LDA #FILL
        JSR go
@nosnow:
        LDA fell2h,X            ; the nearer ridge
        STA NDHS
        STA T0
        SEC
        LDA #FELLY + 32
        SBC T0
        STA NDY
        LDA #$5C                ; green among grey
        STA NCOLOUR
        LDA #%10000010
        STA NPATTERN
        LDA #FILL
        JSR go
        INX
        BNE @col
        RTS

; The near hills, the same way: a dark mixture, green along the crest and a
; lighter line at its edge, and spruces and birches standing on them, each one BLIT of a little
; picture at one of three sizes.
hills:  LDA #SHEET
        STA NFLAGS
        LDA #0
        STA NPATTERN
        STA NDX
        STA NDX+1
        STA NDW
        LDA #1
        STA NDW+1
        STA NDY+1
        LDA #<NEARY
        STA NDY
        LDA #NEARH
        STA NDHS
        LDA #KEYC
        STA NCOLOUR
        LDA #FILL
        JSR go
        LDA #0
        STA NDW+1
        LDA #1
        STA NDW
        LDX #0
@col:   STX NDX
        LDA nearh,X
        STA NDH
        STA T0
        SEC
        LDA #<[NEARY + NEARH]
        SBC T0
        STA NDY
        LDA #$B5                ; dark grey among green: the slopes in shade
        STA NCOLOUR
        LDA #%10100101
        STA NPATTERN
        LDA #FILL
        JSR go
        LDA #5                  ; green: the crest in the light
        STA NDH
        LDA #0
        STA NPATTERN
        LDA #FILL
        JSR go
        LDA #1                  ; and its edge, lighter still
        STA NDH
        LDA #$5D
        STA NCOLOUR
        LDA #%10100101
        STA NPATTERN
        LDA #FILL
        JSR go
        INX
        BNE @col
        LDA #SHEET | KEY        ; the trees
        STA NFLAGS
        LDA #1
        STA NSY+1
        LDX #0
@tree:  LDA trees,X
        BEQ @done
        STA NDX
        LDA trees+1,X           ; its size, and which
        AND #$7F
        STA NDW
        ASL A
        STA NDH
        STA T0
        LDY #P_SPRUCE
        LDA trees+1,X
        BPL @which
        LDY #P_BIRCH
@which: LDA picx,Y
        STA NSX
        LDA picy,Y
        STA NSY
        LDA picw,Y
        STA NSW
        LDA pich,Y
        STA NSH
        LDA NDW                 ; its foot on the hill under its middle
        LSR A
        CLC
        ADC trees,X
        TAY
        SEC
        LDA #<[NEARY + NEARH + 1]
        SBC nearh,Y
        SEC
        SBC T0
        STA NDY
        LDA #BLIT
        JSR go
        INX
        INX
        BNE @tree
@done:  LDA #0
        STA NDY+1
        RTS

; The clouds: one picture, drawn at a few sizes along the strip.
clouds: LDA #SHEET | KEY
        STA NFLAGS
        LDA #0
        STA NDX+1
        STA NDY+1
        LDA #1
        STA NSY+1
        LDY #P_CLOUD
        LDA picx,Y
        STA NSX
        LDA picy,Y
        STA NSY
        LDA picw,Y
        STA NSW
        LDA pich,Y
        STA NSH
        LDX #0
@cloud: LDA cloudtab,X
        BEQ @done
        STA NDX
        LDA cloudtab+1,X
        CLC
        ADC #CLOUDY
        STA NDY
        LDA cloudtab+2,X
        STA NDWS
        LDA cloudtab+3,X
        STA NDHS
        LDA #BLIT
        JSR go
        INX
        INX
        INX
        INX
        BNE @cloud
@done:  LDA #0
        STA NSY+1
        RTS

; ---------------------------------------------------------------- tables
runseq: .byte P_RUNA, P_RUNB, P_RUNC, P_RUNB
pdw:    .byte 12, 14, 10        ; how he's drawn: as he is, squashed, stretched
pdh:    .byte 14, 12, 16
pdx:    .byte 3, 4, 2           ; left of his box
pdy:    .byte TOPY - 2, TOPY, TOPY - 4
bob:    .byte 0, 0, 1, 2, 2, 2, 1, 0
hover:  .rept 32, i
        .byte sin(i * 8, 4)
        .endr

; Where the boxes of the things are: see `touch`. By ETYPE.
hxo:    .byte 0, 6, 9, 9
hxs:    .byte 0, 11, 13, 13
hyo:    .byte 0, 6, 7, 9
hys:    .byte 0, 17, 17, 19

; The pictures on the sheet: x, the line less 256, and the size.
picx:   .byte 0, 12, 24, 36, 48, 60, 72, 80, 92, 104, 116, 128, 136, 168
picy:   .byte 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128
picw:   .byte 12, 12, 12, 12, 12, 12, 8, 12, 12, 12, 12, 8, 32, 8
pich:   .byte 14, 14, 14, 14, 14, 14, 8, 8, 8, 12, 12, 16, 10, 16

; A tile's place on the sheet.
tilex:  .rept 128, i
        .byte [i & 31] * 8
        .endr
tiley:  .rept 128, i
        .byte <[TILEY + [i >> 5] * 8]
        .endr

; What a tile does: 128 of them, by number.
tflag:  .byte 0, SOLID, SOLID, SOLID, SOLID, ONEWAY, 0, 0     ; empty, grass, peat, rock top, rock, plank, water, water
        .byte 0, 0, 0, 0, 0, 0, 0, 0                            ; tuft, cotton grass, heather, stone, toadstool, trunk, crown
        .byte 0, SOLID, ONEWAY, 0                               ; crown, mossy rock, branch
        .fill 12
        .fill 32
        .fill 22
        .byte GOAL, GOAL                                        ; the hut's door, 86 and 87
        .fill 8
        .fill 22
        .byte GOAL, GOAL                                        ; 118 and 119
        .fill 8

; A character of a level, and the tile it's for. $80 and over are things.
chars:  .byte '#', 1, '=', 2, 'R', 3, 'r', 4, '-', 5, '~', 6, 'w', 7
        .byte 't', 8, 'c', 9, 'h', 10, 's', 11, 'u', 12, 'I', 13
        .byte '{', 14, '^', 15, '}', 16, '(', 46, '%', 47, ')', 48
        .byte 'm', 17, 'b', 18
        .byte 'o', $80 | E_BERRY, 'L', $80 | E_LEM, 'M', $80 | E_MIDGE, 'H', $FF
        .byte 0

; The rows of the map, and of the text screen.
rowhi:  .rept 12, i
        .byte [>MAP] + i
        .endr
scrlo:  .rept 25, i
        .byte <[SCREEN + i * 40]
        .endr
scrhi:  .rept 25, i
        .byte >[SCREEN + i * 40]
        .endr

; The backdrops' shapes: heights, a column each, from sines whose periods
; divide 256.
fellh:  .rept 256, i
        .byte 10 + [12 - abs(sin(i * 2 + 30, 12))] + [6 - abs(sin(i * 5, 6))] + sin(i * 3 + 100, 3)
        .endr
fell2h: .rept 256, i
        .byte 7 + sin(i * 3 + 50, 3) + sin(i * 4, 2) + [4 - abs(sin(i * 6 + 77, 4))]
        .endr
nearh:  .rept 256, i
        .byte 22 + sin(i * 2 + 200, 6) + sin(i * 5 + 30, 3) + sin(i * 3, 1)
        .endr

; The trees on the near hills: x, and the width (twice that high), with bit
; 7 for a birch. None across the strip's right edge.
trees:  .byte 6, 8, 16, 6, 23, 10, 34, 7, 52, $80 | 8, 60, 5, 66, 8, 75, 6, 96, 9
        .byte 104, $80 | 6, 110, 7, 118, 5, 140, 8, 147, 10, 157, 6, 164, $80 | 8
        .byte 180, 7, 188, 5, 196, 9, 206, 6, 213, $80 | 7, 228, 8, 236, 6, 242, 9
        .byte 0

; The clouds: x, y in the strip, width and height.
cloudtab:
        .byte 8, 6, 32, 10
        .byte 58, 1, 48, 14
        .byte 120, 9, 24, 7
        .byte 150, 3, 40, 12
        .byte 206, 8, 32, 9
        .byte 0

; The sky, a level each: seven FILLs of y, height, the two colours and the
; pattern; then NFLAGS for the far fells and for the near hills. 32 bytes a
; level.
skies:  .byte TOPY, 10, $6E, %10000010          ; light blue with a little blue
        .byte 14, 14, $EE, 0                    ; light blue
        .byte 28, 10, $1E, %10000010            ; paling
        .byte 38, 10, $1E, %10100101
        .byte 48, 12, $E1, %10000010            ; nearly white over the fells
        .byte 60, 24, $6E, %11110000            ; the lake: stripes
        .byte 84, 16, $B5, %10100101            ; the hillside (`backdrop` draws it)
        .byte KEY, KEY, 0, 0
        .byte TOPY, 10, $46, %10100101          ; the evening: purple and blue
        .byte 14, 10, $4A, %10100101            ; purple and pink
        .byte 24, 10, $8A, %10100101            ; pink and orange
        .byte 34, 12, $78, %10000010            ; orange
        .byte 46, 14, $78, %10100101            ; orange and yellow
        .byte 60, 24, $86, %11110000            ; the lake, lit
        .byte 84, 16, $BB, 0                    ; the hillside, in shadow
        .byte KEY | DUSK, KEY | DARK, 0, 0

; Ink table 2: the clouds and the far fells in the evening. Light blue to
; purple, grey to dark grey, green to blue, and white, the clouds and the
; snow, to light red.
duskink:
        .byte 0, 10, 2, 3, 4, 6, 6, 7, 8, 9, 10, 11, 11, 13, 4, 15
; Ink table 3: the near hills in the evening. Green to dark grey, the trunks
; to black, light green to green, white to light grey.
darkink:
        .byte 0, 15, 2, 3, 4, 11, 6, 7, 8, 0, 10, 11, 12, 5, 14, 15

; The tune's notes: A3 C4 D4 E4 G4 A4 C5 D5 E5 G5, as FREQ (Hz x 16.777).
notelo: .byte <3691, <4389, <4927, <5530, <6577, <7382, <8779, <9854, <11060, <13153
notehi: .byte >3691, >4389, >4927, >5530, >6577, >7382, >8779, >9854, >11060, >13153
tune1:  .byte 6, 0, 7, 6, 5, 0, 4, 0, 5, 6, 7, 8, 9, 0, 0, 0
        .byte 9, 0, 8, 7, 8, 0, 6, 0, 7, 6, 5, 4, 6, 0, 0, 0
        .byte 4, 0, 5, 6, 7, 0, 6, 5, 4, 0, 2, 4, 5, 0, 0, 0
        .byte 6, 0, 7, 8, 9, 8, 7, 6, 5, 4, 5, 7, 6, 0, 0, 0
        .byte $FF
tune2:  .byte 4, 6, 7, 9, 7, 9, 10, 0, 10, 0, 0, $FE
; ---------------------------------------------------------------- the words
; Each is its row and column on the text screen, how many letters, and the letters.
M_NONE1 = 0
M_NONE2 = 2
M_WAIT  = 4
M_NAME  = 6
M_SUB   = 8
M_PRESS = 10
M_KEYS  = 12
M_READY = 14
M_LEV1  = 16
M_LEV2  = 18
M_HOME  = 20
M_WON   = 22
M_SPACE = 24
M_OVER  = 26
M_AGAIN = 28
msgs:   .word m_none1, m_none2, m_wait, m_name, m_sub, m_press, m_keys, m_ready
        .word m_lev1, m_lev2, m_home, m_won, m_space, m_over, m_again
m_none1:.byte 10, 6, 27
        .text "CLOUDBERRY NEEDS THE NIECE."
m_none2:.byte 12, 5, 30
        .text "SWITCH OFF, FIT IT, TRY AGAIN."
m_wait: .byte 12, 10, 19
        .text "OUT TO THE FELLS..."
m_name: .byte 4, 10, 19
        .text "C L O U D B E R R Y"
m_sub:  .byte 6, 9, 22
        .text "THE FELLS, LATE SUMMER"
m_press:.byte 19, 14, 11
        .text "PRESS SPACE"
m_keys: .byte 22, 2, 36
        .text "CURSOR KEYS OR A D: RUN  SPACE: JUMP"
m_ready:.byte 12, 10, 19
        .text "OUT TO THE FELLS..."
m_lev1: .byte 6, 7, 26
        .text "1: THE BOG AND THE BIRCHES"
m_lev2: .byte 6, 8, 23
        .text "2: OVER THE HIGH STONES"
m_home: .byte 6, 8, 24
        .text "THE HUT, AND A FULL PAIL"
m_won:  .byte 6, 8, 24
        .text "HOME. JAM FOR THE WINTER"
m_space:.byte 9, 14, 11
        .text "PRESS SPACE"
m_over: .byte 10, 15, 9
        .text "GAME OVER"
m_again:.byte 13, 9, 22
        .text "SPACE FOR ANOTHER WALK"
hudtxt: .text " BERRIES 00/00  LIVES 3  TIME 000  00000"

; ---------------------------------------------------------------- the levels
; A character a tile: # grass, = peat, R r rock, m moss on rock, - a duckboard and b a
; branch (he can jump up through both), ~ w water, { ^ } ( % ) a birch's crown and I its
; trunk, t c h s u things that grow, o a cloudberry, L a lemming, M midges, H the hut's
; top left corner. Twelve rows of 32 to a piece, eight pieces to a level.
levtab: .word level1, level2
; The bog and the birches.
level1:
        ; columns 0-31
        .byte "r                               "
        .byte "r                               "
        .byte "r                               "
        .byte "r            {^}                "
        .byte "r            (%)        o o     "
        .byte "r             I                 "
        .byte "r         o   I       -----     "
        .byte "r        o o  I    o            "
        .byte "r   t  c      I  h   t    s  ct "
        .byte "r###############################"
        .byte "r==============================="
        .byte "r==============================="
        ; columns 32-63
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "                  o             "
        .byte "                 o o       {^}  "
        .byte "                           (%)  "
        .byte "           o    -----       I   "
        .byte "                            I   "
        .byte "  h  t c       R t  L   c   I t "
        .byte "##########   ###################"
        .byte "==========~~~==================="
        .byte "==========www==================="
        ; columns 64-95
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "              o o o             "
        .byte "            RRRRRRR          o  "
        .byte "            rrrrrrr     o       "
        .byte "        RRRRrrrrrrrRRR          "
        .byte " t    c rrrrrrrrrrrrrr   h  t L "
        .byte "################################"
        .byte "================================"
        .byte "================================"
        ; columns 96-127
        .byte "                                "
        .byte "                                "
        .byte "             M                  "
        .byte "                                "
        .byte "             o                  "
        .byte "          o     o     o         "
        .byte "            ---                 "
        .byte "                                "
        .byte "  t                          c  "
        .byte "####  ---   ---   ---   --  ####"
        .byte "====~~~~~~~~~~~~~~~~~~~~~~~~===="
        .byte "====wwwwwwwwwwwwwwwwwwwwwwww===="
        ; columns 128-159
        .byte "                                "
        .byte "    {^}          {^}            "
        .byte "    (%)    o     (%)       {^}  "
        .byte "    (%)   {^}    (%)  o    (%)  "
        .byte "     I  o (%)  o  I        (%)  "
        .byte "   bbIbb   I     bIbb   o   I   "
        .byte "     I     Ibb    I        bIb  "
        .byte "     I  o  I      I   o     I   "
        .byte " t   I     I  L   I  u   L  I  t"
        .byte "################################"
        .byte "================================"
        .byte "================================"
        ; columns 160-191
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "                  o             "
        .byte "            o    RRR            "
        .byte "           RRR   rrr     o      "
        .byte "       o   rrr   rrr            "
        .byte "      RRR  rrr   rrr    RRR     "
        .byte "  t   rrr  rrr   rrr    rrr   c "
        .byte "####  rrr  rrr   rrr    rrr  ###"
        .byte "====~~rrr~~rrr~~~rrr~~~~rrr~~==="
        .byte "====wwrrrwwrrrwwwrrrwwwwrrrww==="
        ; columns 192-223
        .byte "                                "
        .byte "                                "
        .byte "          M                     "
        .byte "                                "
        .byte "         ooo        ooo         "
        .byte "                                "
        .byte "         ---        ---         "
        .byte "   o                      o     "
        .byte "  t  L       c         L h   t  "
        .byte "########  ########   ###########"
        .byte "========~~========~~~==========="
        .byte "========ww========www==========="
        ; columns 224-255
        .byte "                               r"
        .byte "                               r"
        .byte "                               r"
        .byte "                               r"
        .byte "     o o o                     r"
        .byte "                    H          r"
        .byte "            {^}                r"
        .byte "   o        (%)                r"
        .byte " t  c  h  t  I   s          t  r"
        .byte "###############################r"
        .byte "===============================r"
        .byte "===============================r"
        .assert * - level1 == 8 * 12 * 32
; Over the high stones.
level2:
        ; columns 0-31
        .byte "r                               "
        .byte "r                               "
        .byte "r                               "
        .byte "r                   o           "
        .byte "r                  o o          "
        .byte "r                               "
        .byte "r             o          ---    "
        .byte "r       mmm       mmm           "
        .byte "r t  s  rrr   h   rrr      L    "
        .byte "rmmmmmmmrrrmmmmmmmrrrmmmmmmmmmmm"
        .byte "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr"
        .byte "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr"
        ; columns 32-63
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "       o         o              "
        .byte "                      o         "
        .byte "                                "
        .byte "             mm     o     mmm   "
        .byte "   L         rr   t       rrr  c"
        .byte "mmmmmm   mmmmrr  mmmm   mmrrrmmm"
        .byte "rrrrrr   rrrrrr  rrrr   rrrrrrrr"
        .byte "rrrrrr   rrrrrr  rrrr   rrrrrrrr"
        ; columns 64-95
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "              o o               "
        .byte "                                "
        .byte "         o   mmmm      o        "
        .byte "        mmm  rrrr  --           "
        .byte "    mm  rrr  rrrr       mmmm    "
        .byte " t  rr  rrr  rrrr   L   rrrr  t "
        .byte "mmmmrr  rrr  rrrrmmmmmmmrrrrmmmm"
        .byte "rrrrrr  rrr  rrrrrrrrrrrrrrrrrrr"
        .byte "rrrrrr  rrr  rrrrrrrrrrrrrrrrrrr"
        ; columns 96-127
        .byte "                                "
        .byte "                                "
        .byte "             M                  "
        .byte "                                "
        .byte "                                "
        .byte "          o  o  o  o  o         "
        .byte "            ---   ---           "
        .byte "                                "
        .byte "  c                          t  "
        .byte "mmmm  ---   ---   ---   ---  mmm"
        .byte "rrrr~~~~~~~~~~~~~~~~~~~~~~~~~rrr"
        .byte "rrrrwwwwwwwwwwwwwwwwwwwwwwwwwrrr"
        ; columns 128-159
        .byte "                                "
        .byte "   {^}              {^}         "
        .byte "   (%)       o      (%)     o   "
        .byte "   (%)               I          "
        .byte "    I     o        bbIbb        "
        .byte "  bbIb               I          "
        .byte "    I           o    I          "
        .byte "    I    mmm         I     mmm  "
        .byte " t  I  L rrr  u   t  I  L  rrr h"
        .byte "mmmmmmmmmrrrmmmmmmmmmmmmmmmrrrmm"
        .byte "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr"
        .byte "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr"
        ; columns 160-191
        .byte "                                "
        .byte "                                "
        .byte "                                "
        .byte "          o     o               "
        .byte "                       o        "
        .byte "         mmm   mmm              "
        .byte "    o    rrr   rrr    mmm       "
        .byte "   mmm   rrr   rrr    rrr   mm  "
        .byte "   rrr   rrr   rrr    rrr   rr  "
        .byte "mm rrr   rrr   rrr    rrr   rrmm"
        .byte "rr rrr   rrr   rrr    rrr   rrrr"
        .byte "rr rrr   rrr   rrr    rrr   rrrr"
        ; columns 192-223
        .byte "                                "
        .byte "                                "
        .byte "          M        M            "
        .byte "                                "
        .byte "         ooo      ooo           "
        .byte "                                "
        .byte "         ---      ---           "
        .byte "                           o    "
        .byte "  L      t L      c     L     t "
        .byte "mmmmmm  mmmmmm   mmmmm  mmmmmmmm"
        .byte "rrrrrr  rrrrrr   rrrrr  rrrrrrrr"
        .byte "rrrrrr  rrrrrr   rrrrr  rrrrrrrr"
        ; columns 224-255
        .byte "                               r"
        .byte "                               r"
        .byte "                               r"
        .byte "                               r"
        .byte "    o o o o                    r"
        .byte "                    H          r"
        .byte "         {^}                   r"
        .byte "         (%)                   r"
        .byte " t  h     I   s  c          t  r"
        .byte "mmmmmmmm#######################r"
        .byte "rrrrrrrr=======================r"
        .byte "rrrrrrrr=======================r"
        .assert * - level2 == 8 * 12 * 32

; ---------------------------------------------------------------- the pictures
; Each is its width and height, its x on the sheet and its line less 256, and then its
; rows: a character a pixel, a hex digit for a colour (Appendix A) and a dot for the
; see-through one. 0 black, 1 white, 2 red, 3 cyan, 5 green, 6 blue, 7 yellow, 8 orange,
; 9 brown, A light red, B dark grey, C grey, D light green, E light blue, F light grey.
pics:
        ; tile 1: grass on peat
        .byte 8, 8, 8, 96
        .byte "D5DD5D5D"
        .byte "5D5D55D5"
        .byte "55955595"
        .byte "59995999"
        .byte "99989999"
        .byte "99999909"
        .byte "98999999"
        .byte "99990999"
        ; tile 2: peat
        .byte 8, 8, 16, 96
        .byte "99999999"
        .byte "99899999"
        .byte "99999099"
        .byte "90999999"
        .byte "99999989"
        .byte "99999999"
        .byte "98990999"
        .byte "99999999"
        ; tile 3: the top of a rock
        .byte 8, 8, 24, 96
        .byte "F1FF1FFF"
        .byte "FFFFFFCF"
        .byte "FCFFFFFF"
        .byte "FFFFCBFF"
        .byte "CFFFFCFF"
        .byte "FFCFFFFC"
        .byte "FFFFFCFF"
        .byte "CFFCFFFC"
        ; tile 4: rock
        .byte 8, 8, 32, 96
        .byte "CCFCCCCC"
        .byte "CCCCCBCC"
        .byte "CBCCCCCC"
        .byte "CCCCFCCB"
        .byte "CCBCCCCC"
        .byte "FCCCCCBC"
        .byte "CCCCBCCC"
        .byte "CCBCCCCF"
        ; tile 5: a duckboard
        .byte 8, 8, 40, 96
        .byte "78888887"
        .byte "88988988"
        .byte "99999999"
        .byte ".0....0."
        .byte "........"
        .byte "........"
        .byte "........"
        .byte "........"
        ; tile 6: the top of the water
        .byte 8, 8, 48, 96
        .byte "........"
        .byte "........"
        .byte "E1EE3EE1"
        .byte "6E66E666"
        .byte "66666E66"
        .byte "6E666666"
        .byte "666E6666"
        .byte "66666666"
        ; tile 7: water
        .byte 8, 8, 56, 96
        .byte "66666666"
        .byte "66E66666"
        .byte "66666666"
        .byte "6666E666"
        .byte "66666666"
        .byte "E6666666"
        .byte "66666E66"
        .byte "66666666"
        ; tile 8: a tuft of grass
        .byte 8, 8, 64, 96
        .byte "........"
        .byte "........"
        .byte "........"
        .byte "........"
        .byte ".D....D."
        .byte ".D.D.D5."
        .byte "D5D5D5D."
        .byte "5D5D55D5"
        ; tile 9: cotton grass
        .byte 8, 8, 72, 96
        .byte "........"
        .byte ".1...1.."
        .byte "111.111."
        .byte ".1...1.1"
        .byte ".5..5.11"
        .byte ".5..5..1"
        .byte ".5.5..5."
        .byte "55.5..5."
        ; tile 10: heather
        .byte 8, 8, 80, 96
        .byte "........"
        .byte "........"
        .byte ".A..A..."
        .byte "A2A.2A.A"
        .byte ".5A2A52A"
        .byte ".52A525."
        .byte "5.5255.5"
        .byte ".5.55.5."
        ; tile 11: a stone
        .byte 8, 8, 88, 96
        .byte "........"
        .byte "........"
        .byte "........"
        .byte "........"
        .byte "..FFF..."
        .byte ".FFFCC.."
        .byte "FFFCCCB."
        .byte "FCCCCBBB"
        ; tile 12: a toadstool
        .byte 8, 8, 96, 96
        .byte "........"
        .byte "........"
        .byte "..222..."
        .byte ".21212.."
        .byte ".22222.."
        .byte "..111..."
        .byte "..111..."
        .byte "..111..."
        ; tile 13: a birch trunk
        .byte 8, 8, 104, 96
        .byte "..11F1.."
        .byte "..1101.."
        .byte "..11F1.."
        .byte "..0111.."
        .byte "..11F1.."
        .byte "..1111.."
        .byte "..110F.."
        .byte "..11F1.."
        ; tiles 14-16 and 46-48: a birch's crown, six tiles
        .byte 24, 16, 112, 96
        .byte ".......D5DD5D..........."
        .byte ".....D5D5D5D5D5D........"
        .byte "...5D5DD5D75D5DD5D......"
        .byte "..5DD5D5D5D5DD5D5D5....."
        .byte ".D5D5D75D5DD5D5D75D5D..."
        .byte ".5D5DD5D5D5D5D5DD5D5D5.."
        .byte "D5D5D5D5DD5D75D5D5D5DD5."
        .byte "5DD5D5D5D5D5D5D5DD5D5D5D"
        .byte ".D5D75D5D5DD5D5D5D5D75D."
        .byte ".5D5D5DD5D5D5D75D5D5D5D."
        .byte "..D5D5D5D5D5D5D5D5DD5D.."
        .byte "...5DD5D.5D5DD5D5.D5D..."
        .byte "....D5....D5D5D....5...."
        .byte "..........5D1D.........."
        .byte "..........11F1.........."
        .byte "..........1101.........."
        ; tile 17: moss on rock
        .byte 8, 8, 136, 96
        .byte "D5DD5D5D"
        .byte "5D5D55D5"
        .byte "F5FF5FCF"
        .byte "FCFFFFFF"
        .byte "FFFFCBFF"
        .byte "CFFFFCFF"
        .byte "FFCFFFFC"
        .byte "CFFCFFFC"
        ; tile 18: a birch branch
        .byte 8, 7, 144, 96
        .byte ".D5..5D."
        .byte "11F1111F"
        .byte "B11F11B1"
        .byte ".5...D5."
        .byte ".....5.."
        .byte "........"
        .byte "........"
        ; tiles 20-23, 52-55, 84-87, 116-119: the hut, sixteen tiles
        .byte 32, 32, 160, 96
        .byte "......................BBBB......"
        .byte "......................BCCB......"
        .byte "......................BCCB......"
        .byte "......D5DD5D5DDD5D5DD5BCCB......"
        .byte ".....5D5D55D5D5D5D55D5BCCBD....."
        .byte "....D55D5D5D55D5D5D5D55D5D5D...."
        .byte "...5D5D5D55D5D5D5D55D5D5D5D55..."
        .byte "..D5D55D5D5D5D55D5D5D5D5D55D5D.."
        .byte ".5D5D5D5D55D5D5D5D5D55D5D5D5D5D."
        .byte ".000000000000000000000000000000."
        .byte ".09999999999999999999999999990.."
        .byte "..0000000000000000000000000000.."
        .byte "...12222222222222222222222221..."
        .byte "...12222222222222222222222221..."
        .byte "...1A2A2A2A2A2A2A2A2A2A2A2A21..."
        .byte "...12211111111122222222222221..."
        .byte "...12217771777122222222222221..."
        .byte "...12217771777122211111111221..."
        .byte "...12211111111122219999991221..."
        .byte "...12217771777122219899891221..."
        .byte "...12217771777122219899891221..."
        .byte "...12211111111122219899891221..."
        .byte "...1A2A2A2A2A2A2A219899891A21..."
        .byte "...12222222222222219899891221..."
        .byte "...12222222222222219899781221..."
        .byte "...12222222222222219899891221..."
        .byte "...1A2A2A2A2A2A2A219899891A21..."
        .byte "...12222222222222219899891221..."
        .byte "...12222222222222219899891221..."
        .byte "...12222222222222219899891221..."
        .byte "...1A2A2A2A2A2A2A219899891A21..."
        .byte "...CCBCCBCCBCCBCCB19999991CBC..."
        ; the picker, standing (he faces right; FLIPX turns him round)
        .byte 12, 14, 0, 128
        .byte ".....11....."
        .byte "....2222...."
        .byte "...222222..."
        .byte "...2222222.."
        .byte "...0AAA0A..."
        .byte "...AAAAAA..."
        .byte "....AAAA...."
        .byte "...777777..."
        .byte "..07777778.."
        .byte "..A777777A.."
        .byte "...777777..."
        .byte "....6666...."
        .byte "....66.66..."
        .byte "...000.000.."
        ; running: one leg out
        .byte 12, 14, 12, 128
        .byte ".....11....."
        .byte "....2222...."
        .byte "...222222..."
        .byte "...2222222.."
        .byte "...0AAA0A..."
        .byte "...AAAAAA..."
        .byte "....AAAA...."
        .byte "..A777777..."
        .byte "...7777778A."
        .byte "...777777..."
        .byte "...777777..."
        .byte "...666666..."
        .byte "..66....66.."
        .byte ".00......000"
        ; running: legs together
        .byte 12, 14, 24, 128
        .byte "............"
        .byte ".....11....."
        .byte "....2222...."
        .byte "...222222..."
        .byte "...2222222.."
        .byte "...0AAA0A..."
        .byte "...AAAAAA..."
        .byte "....AAAA...."
        .byte "...777777..."
        .byte "...7777A7..."
        .byte "...777777..."
        .byte "....6666...."
        .byte "....666....."
        .byte "....0000...."
        ; running: the other leg out
        .byte 12, 14, 36, 128
        .byte ".....11....."
        .byte "....2222...."
        .byte "...222222..."
        .byte "...2222222.."
        .byte "...0AAA0A..."
        .byte "...AAAAAA..."
        .byte "....AAAA...."
        .byte "...777777A.."
        .byte ".A87777777.."
        .byte "...777777..."
        .byte "...777777..."
        .byte "...666666..."
        .byte "..66....66.."
        .byte ".000.....00."
        ; jumping
        .byte 12, 14, 48, 128
        .byte ".....11....."
        .byte ".A..2222..A."
        .byte ".7.222222.7."
        .byte ".7.2222222.."
        .byte "..70AAA0A7.."
        .byte "...AAAAAA..."
        .byte "....AAAA...."
        .byte "...777777..."
        .byte "...777777..."
        .byte "...777777..."
        .byte "...666666..."
        .byte "..66...66..."
        .byte "..00...66..."
        .byte ".......00..."
        ; falling
        .byte 12, 14, 60, 128
        .byte ".....11....."
        .byte "....2222...."
        .byte "...222222..."
        .byte "...2222222.."
        .byte "...0AAA0A..."
        .byte "A..AAAAAA..A"
        .byte ".77.AAAA.77."
        .byte "..77777777.."
        .byte "...777777..."
        .byte "...777777..."
        .byte "...666666..."
        .byte "...66..66..."
        .byte "..66....66.."
        .byte "..00....00.."
        ; a cloudberry
        .byte 8, 8, 72, 128
        .byte "....5..."
        .byte "...D5D.."
        .byte "..7878.."
        .byte ".787878."
        .byte ".871787."
        .byte ".787878."
        .byte "..8787.."
        .byte "........"
        ; a lemming
        .byte 12, 8, 80, 128
        .byte "............"
        .byte "....000000.."
        .byte "..0000000000"
        .byte ".08880000010"
        .byte ".8888800000A"
        .byte ".78888880077"
        .byte "..77777777.."
        .byte "..0..0.0..0."
        ; and its other step
        .byte 12, 8, 92, 128
        .byte "............"
        .byte "....000000.."
        .byte "..0000000000"
        .byte ".08880000010"
        .byte ".8888800000A"
        .byte ".78888880077"
        .byte "..77777777.."
        .byte "...0.0...0.0"
        ; midges
        .byte 12, 12, 104, 128
        .byte "....0......."
        .byte ".0.....0..B."
        .byte "......0....."
        .byte "..B.0....0.."
        .byte ".......B...."
        .byte ".0..0.0...0."
        .byte "...B....0..."
        .byte ".0....B..0.."
        .byte "....0......."
        .byte "..0....0.B.."
        .byte ".....0......"
        .byte "........0..."
        ; midges, a moment later
        .byte 12, 12, 116, 128
        .byte "......0....."
        .byte "..0......0.."
        .byte ".B...0......"
        .byte "....0..B..0."
        .byte ".0.......0.."
        .byte "....B.0....."
        .byte ".0.....0..B."
        .byte "...0.0......"
        .byte ".B......0.0."
        .byte ".....0......"
        .byte "..0....B...."
        .byte "......0....."
        ; a spruce, for the near hills
        .byte 8, 16, 128, 128
        .byte "...05..."
        .byte "...05..."
        .byte "..0055.."
        .byte "..0055.."
        .byte ".000555."
        .byte "..0055.."
        .byte ".000555."
        .byte "0000555."
        .byte ".000555."
        .byte "00005555"
        .byte ".000555."
        .byte "00005555"
        .byte "00005555"
        .byte "...09..."
        .byte "...09..."
        .byte "...09..."
        ; a cloud
        .byte 32, 10, 136, 128
        .byte "..........111111................"
        .byte ".......11111111111....1111......"
        .byte ".....111111111111111111111111..."
        .byte "...1111111111111111111111111111."
        .byte "..111111111111111111111111111111"
        .byte ".1111111111111111111111111111111"
        .byte "11111111111111111111111111111111"
        .byte "1111FFF1111111111FFFF11111111FF1"
        .byte ".FFFFFFFFFF11FFFFFFFFFFFF1FFFFF."
        .byte "...FFFFFFFFFFFFFFFFFFFFFFFFF...."
        ; a birch, for the near hills
        .byte 8, 16, 168, 128
        .byte "..D5D..."
        .byte ".D5DD5.."
        .byte "D5D5D5D."
        .byte ".DD5D5D5"
        .byte "D5D15D5."
        .byte ".D5D1D5D"
        .byte "..5D15D."
        .byte "...D1D.."
        .byte "....1..."
        .byte "....1..."
        .byte "...01..."
        .byte "....1..."
        .byte "....10.."
        .byte "....1..."
        .byte "....1..."
        .byte "....1..."
        .byte 0

; ---------------------------------------------------------------- memory
RAM:
BANDS:  .res 28                 ; the sky's seven FILLs, for this level
EFIRST: .res 10                 ; the first thing of each 256 pixels of the level
ETYPE:  .res NENT               ; the things: 0 gone, or E_BERRY and the rest
EXL:    .res NENT               ; where, in the level
EXH:    .res NENT
EY:     .res NENT
ES:     .res NENT               ; a lemming's way (0 or FLIPX), or frames left flat
TRANS:  .res 256                ; a level's character to its tile
        .align 256
MAP:    .res 12 * 256           ; the level: a byte a tile, a page a row
RAMEND:
