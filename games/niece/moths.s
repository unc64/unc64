; MOTHS: a fixed shooter for the UNC 64 (64K) with the NIECE, the prototype
; graphics chip of chapter 17. United Northern Computers' laboratory, 1985.
; No video board needed: every moving thing is drawn by the NIECE.
;
; A summer night at the edge of the wood. You carry the lantern along the path,
; and the moths come for the light: thirty-two of them, hanging in ranks over
; the trees. They are far away up there, and small. One at a time, then in
; twos and threes, they peel off and swoop down at the lantern, growing as
; they come, and climb away again, shrinking. Shake sparks out of the lantern
; to turn them to dust. When the ranks are empty the EMPEROR comes, a moth
; several times the size of the others with a ring of small ones round it,
; nearer and bigger, then further and smaller, shedding dust. Then the next
; wave, faster.
;
; Controls: cursor left and right, or A and D, walk (held keys, from the
; keyboard matrix, so you can walk and fire at once). SPACE throws sparks, and
; starts the game. A moth, the EMPEROR or a speck of dust touching you costs a
; lantern; you have three.
;
; What it shows of the NIECE:
;   no erasing: every frame the whole picture is drawn again on the page that
;     isn't shown, back to front, and FLIP shows it at the next vertical
;     blank. The main loop waits for the queue to empty, which is the FLIP,
;     so the game is in step with the screen without counting a line
;   a background of three pictures kept on the sheet, each 256 wide so that
;     it joins up with itself (a source that runs off the sheet's edge
;     carries on from the other): the sky, the trees and the ground. Each is
;     one BLIT a frame, and each moves its source by a different amount as
;     you walk: an eighth, a quarter and a half of the way (`backdrop`)
;   the three pictures aren't in the program. The sky's mixtures of colour
;     go in through NVDATA, and the NIECE draws the rest itself at the start,
;     on the sheet (SHEET): the stars a pixel each, the trees from two little
;     pictures drawn at six sizes, the grass from one tuft (`sky`, `trees`,
;     `ground`)
;   up to fifty keyed BLITs a frame: 32 moths, the dust they turn into,
;     sparks, the lantern's light and its carrier, and eight fireflies that
;     are a FILL each. The old video board could show eight sprites on a line
;   scaling as part of the game: a moth is drawn as wide as it is near
;     (`SZW`, from its height on the screen), its box for the sparks grows
;     with it, and the EMPEROR's swoops are nothing but NDW and NDH. Under 18
;     pixels wide a moth is drawn from the small picture, over that from the
;     big one, so neither is stretched far
;   FLIPY: wings down is the wings-up picture upside down, so two pictures
;     make a wing-beat of four. FLIPX: the lantern-carrier turns round
;   ink tables: table 1 is all white, and a moth that's hit is drawn through
;     it for three frames before it's dust. Tables 2 and 3 make the green
;     and the red moths out of the one brown picture
;   the arithmetic unit: where a swooping moth is, is two products a frame
;     (`divers`), and the EMPEROR's ring is two more for each of its moths
;   the queue: the three big BLITs of the background go in first and the
;     chip draws them while the processor moves the game on. Every command
;     waits for room (`go`), so none is ever lost
;
; The score, the lanterns left and the words are ordinary text, in front of
; the page: a space is a window.
;
; Sound: the AUNT, a voice each for the sparks, the moths and the bad news.
;
; Paced by the FLIP: one step of the game a frame, 50 a second. `work` to
; `endwork` is one frame's work. At its fullest (the ranks full, six moths
; swooping, sparks, dust and eight puffs: 59 commands) that is about 10,600
; of the frame's 19,968 cycles, and 8,700 on the average frame: 2,400 moving
; the swoops and looking for what touched what, 2,300 drawing the ranks,
; 1,200 the swoops, 1,100 the puffs, 1,000 the background and the fireflies.
; The NIECE has about 40,000 pixels to draw then, half of the 79,872 it can
; do in a frame, and is done 10,700 cycles after the vertical blank. The
; queue gets 45 deep of its 64: the background takes the chip 5,700 cycles,
; and the processor has most of a quiet frame's commands written by then.
; LATE counts the frames that missed their vertical blank; the tests want 0.
;
; Without the NIECE ($C14F doesn't read $17), MOTHS says so and waits.
;
; Memory:
;   $0000-$00CF  variables (the fast page; the ROM's $D0-$FE untouched)
;   $0400-$07E7  the text screen: the score and the words
;   $0800-       code, tables, the pictures
;   after them   more variables (.res)
; On the sheet: the pictures in columns 160-255 beside the pages, the sky at
; line 200, the trees at 272 and the ground at 312.
;
; The tests (crates/unc-asm/tests/game_moths.rs) find STATE, PX, LIVES, SCORE
; (six digits, 0-9, the highest first), MST, LEFT, PHASE, LATE and the rest by
; name in the symbol table.

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
NINK    = $C120
NMULA   = $C130
NMULB   = $C132
NPROD   = $C134
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
GREEN   = $80           ; ink table 2
RED     = $C0           ; ink table 3
KEYC    = 4             ; the see-through colour: purple, so black can be an outline

; ---------------------------------------------------------------- tuning
TITLE   = 1             ; STATE
PLAY    = 2
OVER    = 3
NMOTH   = 32            ; the ranks: 8 across, 4 down
NCOL    = 8
COLW    = 15            ; from one moth to the next, across
ROWH    = 10            ; and down
FY      = 8             ; the top rank's line
FXMIN   = 2             ; how far the ranks sway
FXMAX   = 41
ND      = 6             ; moths swooping at once, at most
NS      = 4             ; sparks
NDU     = 6             ; specks of dust falling
NP      = 8             ; puffs
NE      = 8             ; the EMPEROR's ring
PXMIN   = 10            ; the lantern-carrier's x: his middle
PXMAX   = 150
PTOP    = 79            ; his top line: his feet are on line 96
DEPTH   = 84            ; how low a swoop goes: the line of his head
COOLT   = 9             ; frames between sparks

; ---------------------------------------------------------------- the sheet
SKYY    = 200           ; the sky: 256 x 72
SKYH    = 72
TREEY   = 272           ; the trees: 256 x 40, see-through above them
TREEH   = 40
GRNDY   = 312           ; the ground: 256 x 28, see-through above the grass
GRNDH   = 28
X_MSA   = 160           ; the pictures: where each is on the sheet
X_MSB   = 172
X_MBA   = 184
X_MBB   = 208
X_SPARK = 232
X_DUST  = 236
X_PUFF  = 240
Y_EMP   = 20
X_EMA   = 160
X_EMB   = 192
X_GLOW  = 224
Y_MAN   = 46
X_MAN   = 160
X_MOON  = 188
X_TUFT  = 202
X_FLOWER = 210
Y_LOGO  = 66
X_LOGO  = 160
Y_TREE  = 76
X_SPRUCE = 160
X_OAK   = 176

; ---------------------------------------------------------------- the fast page
STATE   = $00
PHASE   = $01           ; 0 get ready, 1 the ranks, 2 the EMPEROR
TICK    = $02           ; frames, 16 bits
TICKH   = $03
LASTF   = $04           ; FRAME at the last `work`
LATE    = $05           ; frames that took two
PX      = $06
PFACE   = $07           ; 0 facing right, FLIPX facing left
PWALK   = $08
PDEAD   = $09           ; frames until the next lantern
INVUL   = $0A           ; frames he can't be hurt for
LIVES   = $0B
LEVEL   = $0C
COOL    = $0D
FX      = $0E           ; the ranks' left edge
FDIR    = $0F
LEFT    = $10           ; moths left, the swooping ones too
LAUNCH  = $11           ; frames until the next swoop
SEEDL   = $12
SEEDH   = $13
T0      = $14           ; scratch
T1      = $15
T2      = $16
T3      = $17
T4      = $18
T5      = $19
PTR     = $1A
PT2     = $1C
CAM     = $1E           ; what the background moves by
LX      = $1F           ; the lantern's x
HUDD    = $20           ; the score line wants printing
PT      = $21           ; frames a banner stays
SPREV   = $22           ; SPACE, last frame
OT      = $23           ; frames of game over, 16 bits
OTH     = $24
DI      = $25           ; a loop's X, kept over a call
PFN     = $26           ; the puff used last
MXL     = $27           ; `moth`: its middle, 16 bits across
MXH     = $28
MY      = $29
MW      = $2A           ; its width
MPH     = $2B           ; where in the wing-beat, 0-3
MFL     = $2C           ; NFLAGS: KEY and an ink table
HCX     = $2D           ; `hitbox`: the middle and half the size
HCY     = $2E
HHW     = $2F
HHH     = $30
DSPL    = $31           ; this wave: a swoop's speed, 8.8
DSPH    = $32
LINT    = $33           ; frames between swoops
MAXD    = $34           ; how many at once
DUSTP   = $35           ; how likely a swooping moth is to shed dust
SWP     = $36           ; the spark's falling note
BST     = $37           ; the EMPEROR: 0 not here, 1 flying, 2 falling to dust
BHP     = $38
BAL     = $39           ; its way across: an angle, 8.8
BA      = $3A
BBL     = $3B           ; and its way nearer and further
BB      = $3C
BX      = $3D
BY      = $3E
BW      = $3F
BH      = $40
BFL     = $41           ; frames it's drawn white for
BDT     = $42           ; frames of falling to dust left
BRING   = $43           ; the ring's angle
BDROP   = $44           ; frames until its next dust
BDINT   = $45
EW      = $46           ; the ring's moths' width
SCORE   = $48           ; six digits
HISC    = $4E
SHX     = $54           ; sparks
SHY     = $58           ; 0: not flying
PFA     = $5C           ; puffs: the age, 0 none
DT      = $64           ; swoops: the angle, 0-128, 8.8
DTL     = $6A
DUY     = $70           ; dust: 0 none
FPEND   = $76

; ---------------------------------------------------------------- start
        .org $0800
start:  LDX #$FF
        TXS
        JSR CLS
        LDA NIECE
        CMP #NIECE_ID
        BEQ @there
        LDX #M_NONE1
        JSR puts
        LDX #M_NONE2
        JSR puts
@wait:  JMP @wait               ; nothing to draw with: wait to be switched off
@there: LDA #0
        TAX
@zero:  STA $00,X               ; RESET keeps memory, so start from nothing
        INX
        CPX #FPEND
        BNE @zero
        TAX
@zero2: STA RAM,X
        INX
        BNE @zero2
        LDA #$E1
        STA SEEDL
        LDA #$AC
        STA SEEDH
        LDX #39
@hud:   LDA hudtxt,X
        STA SCREEN,X
        DEX
        BPL @hud
        JSR setup
        JSR sound
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
@tick:  JSR backdrop            ; the NIECE starts on the big ones,
        LDA STATE               ; and the game moves on meanwhile
        CMP #PLAY
        BEQ @play
        CMP #OVER
        BEQ @over
        JSR st_title
        JMP @flip
@play:  JSR st_play
        JMP @flip
@over:  JSR st_over
@flip:  LDA #FLIP
        JSR go
endwork:
        JMP main

; A command into the queue, when there's room for it. A is the command.
go:     BIT NSTATUS             ; bit 6 goes to V
        BVS go
        STA NGO
        RTS

; ---------------------------------------------------------------- the background
; Three BLITs, 160 wide, from three pictures 256 wide: 22,720 pixels, a good
; quarter of what the NIECE can draw in a frame. Each source starts at a
; different fraction of CAM, and the sky drifts by itself as well, a pixel
; every 64 frames.
backdrop:
        LDA #0
        STA NDX
        STA NDX+1
        STA NDY
        STA NDY+1
        STA NSY+1
        STA NFLAGS              ; the sky: a plain copy
        LDA #160
        STA NDW
        STA NSW
        LDA #SKYH
        STA NDH
        STA NSH
        LDA #SKYY
        STA NSY
        LDA TICKH
        STA T0
        LDA TICK
        ASL A
        ROL T0
        ASL A
        ROL T0                  ; T0 = frames / 64
        LSR T0                  ; an even column: see `sky`
        LDA CAM
        LSR A
        LSR A
        LSR A
        LSR A
        CLC
        ADC T0
        ASL A
        STA NSX
        LDA #BLIT
        JSR go
        LDA #KEY                ; the trees, in front of it
        STA NFLAGS
        LDA #72 + 4 - TREEH
        STA NDY
        LDA #TREEH
        STA NDH
        STA NSH
        LDA #<TREEY
        STA NSY
        LDA #>TREEY
        STA NSY+1
        LDA CAM
        LSR A
        LSR A
        STA NSX
        LDA #BLIT
        JSR go
        LDA #100 - GRNDH        ; and the ground
        STA NDY
        LDA #GRNDH
        STA NDH
        STA NSH
        LDA #<GRNDY
        STA NSY
        LDA CAM
        LSR A
        STA NSX
        LDA #BLIT
        JSR go
        LDA #0
        STA NSY+1               ; the pictures are all above line 256

; Eight fireflies over the grass: a FILL of one pixel each, wandering on two
; sines, each lit for five frames in eight of its own.
        LDA #1
        STA NDW
        STA NDH
        LDA TICKH
        LSR A
        LDA TICK
        ROR A
        STA T0                  ; half speed across
        LDX #7
@fly:   TXA
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        STA T1
        ADC TICK
        LSR A
        LSR A
        LSR A
        AND #7
        CMP #5
        BCS @dark
        LDA T1
        ADC T0
        TAY
        LDA TXT,Y
        STA NDX
        TXA
        ASL A
        ASL A
        ADC T1
        ADC TICK
        TAY
        LDA LBOB,Y              ; 8 to 16,
        ADC flyy,X              ; and each has its own height
        STA NDY
        LDA flyc,X
        STA NCOLOUR
        LDA #FILL
        JSR go
@dark:  DEX
        BPL @fly
        RTS

; ---------------------------------------------------------------- the title
st_title:
        LDA TICKH               ; the background sways by itself
        LSR A
        LDA TICK
        ROR A
        TAX
        LDA BXT,X
        STA CAM
        LDA #0                  ; M O T H S, each letter three times its size
        STA NDX+1
        STA NDY+1
        LDA #24
        STA NDW
        STA NDH
        LDA #8
        STA NSW
        STA NSH
        LDA #Y_LOGO
        STA NSY
        LDA #KEY
        STA NFLAGS
        LDX #4
@letter:
        LDA logox,X
        STA NDX
        TXA
        ASL A
        ASL A
        ASL A
        STA T0
        CLC
        ADC #X_LOGO
        STA NSX
        LDA T0                  ; each bobs a little after the one before
        ASL A
        ADC TICK
        ADC TICK
        TAY
        LDA LBOB,Y
        STA NDY
        LDA #BLIT
        JSR go
        DEX
        BPL @letter
        LDX #7                  ; eight moths, round and round: big when low
@moth:  TXA
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC TICK
        TAY
        LDA TXT,Y
        STA MXL
        LDA #0
        STA MXH
        TYA
        ASL A
        STA T0
        TXA
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC T0
        TAY
        LDA TYT,Y
        STA MY
        TAY
        LDA SZW,Y
        STA MW
        TXA
        AND #3
        TAY
        LDA titfl,Y
        STA MFL
        LDA TICK
        LSR A
        LSR A
        STX DI
        CLC
        ADC DI
        AND #3
        STA MPH
        JSR moth
        LDX DI
        DEX
        BPL @moth
        LDA TICK                ; PRESS SPACE, on and off
        AND #31
        BNE @key
        LDX #M_PRESS
        LDA TICK
        AND #32
        BEQ @show
        LDX #M_BLANK14
@show:  JSR puts
@key:   JSR space
        BCC @rts
        JMP newgame
@rts:   RTS

; Carry set if SPACE has just gone down.
space:  LDA KEYS
        AND #1
        TAX
        EOR SPREV
        STX SPREV
        BEQ @no
        TXA
        BEQ @no
        SEC
        RTS
@no:    CLC
        RTS

totitle:
        LDA #TITLE
        STA STATE
        LDA #3
        STA LIVES
        JSR quiet
        JSR wipe
        LDX #M_PRESS
        JSR puts
        LDX #M_KEYS
        JSR puts
        LDX #M_NIECE
        JSR puts
        INC HUDD
        JMP hud

; Nothing flying.
quiet:  LDA #0
        STA BST
        STA LEFT
        STA PT
        LDX #NMOTH-1
@m:     STA MST,X
        DEX
        BPL @m
        LDX #7
@p:     STA PFA,X
        STA EST,X
        DEX
        BPL @p
        LDX #5
@d:     STA DMI,X
        STA DUY,X
        DEX
        BPL @d
        LDX #NS-1
@s:     STA SHY,X
        DEX
        BPL @s
        RTS

; The rows the words go on, cleared: six at once.
wipe:   LDA #' '
        LDX #39
@c:     STA SCREEN+10*40,X
        STA SCREEN+12*40,X
        STA SCREEN+14*40,X
        STA SCREEN+20*40,X
        STA SCREEN+22*40,X
        STA SCREEN+24*40,X
        DEX
        BPL @c
        RTS

; ---------------------------------------------------------------- a game
newgame:
        JSR quiet
        JSR wipe
        LDA #PLAY
        STA STATE
        LDA #0
        STA PDEAD
        STA INVUL
        STA PFACE
        STA COOL
        LDX #5
@s:     STA SCORE,X
        DEX
        BPL @s
        LDA #3
        STA LIVES
        LDA #1
        STA LEVEL
        STA FDIR
        LDA #80
        STA PX
        STA CAM
        LDA #84
        STA LX
        LDA #20
        STA FX
        INC HUDD
; falls through

; The ranks again, full, and this wave's numbers.
newwave:
        LDA #1
        LDX #NMOTH-1
@m:     STA MST,X
        DEX
        BPL @m
        LDA #NMOTH
        STA LEFT
        LDA #0
        STA PHASE
        LDX #ND-1
@d:     STA DMI,X
        DEX
        BPL @d
        LDX LEVEL
        CPX #8
        BCC @lv
        LDX #8
@lv:    LDA lvspl-1,X
        STA DSPL
        LDA lvsph-1,X
        STA DSPH
        LDA lvint-1,X
        STA LINT
        STA LAUNCH
        LDA lvmax-1,X
        STA MAXD
        LDA lvdust-1,X
        STA DUSTP
        LDA lvdrop-1,X
        STA BDINT
        LDX #M_WAVE
        JSR puts
        LDA LEVEL               ; the wave's number, two digits
        LDX #'0'
@tens:  CMP #10
        BCC @units
        SBC #10
        INX
        BNE @tens
@units: ORA #'0'
        STA SCREEN+10*40+23
        CPX #'0'
        BEQ @one
        STX SCREEN+10*40+22
@one:   LDA #100
        STA PT
        RTS

gameover:
        LDA #OVER
        STA STATE
        LDA #0
        STA OT
        STA OTH
        LDA #255
        STA PDEAD
        LDX #0                  ; a new best?
@cmp:   LDA SCORE,X
        CMP HISC,X
        BCC @not
        BNE @best
        INX
        CPX #6
        BNE @cmp
@best:  LDX #5
@copy:  LDA SCORE,X
        STA HISC,X
        DEX
        BPL @copy
@not:   INC HUDD
        LDX #M_BLANK10
        JSR puts
        LDX #M_OVER
        JMP puts

st_over:
        INC OT
        BNE @t
        INC OTH
@t:     JSR world
        JSR space
        BCC @wait
        LDA OTH                 ; not in the first second: he was still firing
        BNE @new
        LDA OT
        CMP #50
        BCC @wait
@new:   JMP newgame
@wait:  LDA OTH
        CMP #2                  ; ten seconds of it, then the title
        BCC @rts
        JMP totitle
@rts:   RTS

st_play:
        JSR input
; falls through

; Everything but the player's keys: the game over screen goes on with it.
world:  JSR shots
        JSR swarm
        JSR boss
        JSR dust
        JSR phase
        JSR dform               ; and the picture, back to front
        JSR dboss
        JSR dpuffs
        JSR dplayer
        JSR dshots
        JSR ddust
        JSR ddivers
        JMP hud

; ---------------------------------------------------------------- the lantern-carrier
input:  LDA PDEAD
        BEQ @alive
        DEC PDEAD
        BNE @rts
        LDA LIVES
        BNE @again
        JMP gameover
@again: LDA #100
        STA INVUL
@rts:   RTS
@alive: LDA INVUL
        BEQ @keys
        DEC INVUL
@keys:  LDA KEYS+8              ; cursor left, or A
        AND #%00000001
        BNE @left
        LDA KEYS+4
        AND #%00000010
        BEQ @notl
@left:  LDA #FLIPX
        STA PFACE
        INC PWALK
        LDA PX
        SEC
        SBC #2
        CMP #PXMIN
        BCS @setl
        LDA #PXMIN
@setl:  STA PX
@notl:  LDA KEYS+8              ; cursor right, or D
        AND #%00000010
        BNE @right
        LDA KEYS+4
        AND #%00010000
        BEQ @notr
@right: LDA #0
        STA PFACE
        INC PWALK
        LDA PX
        CLC
        ADC #2
        CMP #PXMAX+1
        BCC @setr
        LDA #PXMAX
@setr:  STA PX
@notr:  LDA PX
        STA CAM
        LDX PFACE               ; the lantern is on the side he faces
        CLC
        ADC lanoff,X
        STA LX
        LDA SWP                 ; the last spark's note falls
        CMP #$18
        BCC @cool
        SBC #5
        STA SWP
        STA V1+1
@cool:  LDA COOL
        BEQ @fire
        DEC COOL
        RTS
@fire:  LDA KEYS
        AND #1
        BEQ @done
        LDX #NS-1
@free:  LDA SHY,X
        BEQ @spark
        DEX
        BPL @free
@done:  RTS
@spark: LDA LX
        STA SHX,X
        LDA #PTOP-4
        STA SHY,X
        LDA #COOLT
        STA COOL
        LDA #$58                ; a high note, falling
        STA SWP
        STA V1+1
        LDA #0
        STA V1+4
        LDA #1
        STA V1+4
        RTS

; He's hit, unless he's already dust or only just back.
pkill:  LDA PDEAD
        ORA INVUL
        BNE @rts
        LDA STATE
        CMP #PLAY
        BNE @rts
        LDA #70
        STA PDEAD
        DEC LIVES
        INC HUDD
        LDA PX
        STA T1
        LDA #0
        STA T2
        LDA #PTOP+9
        STA T3
        LDA #20
        STA T4
        JSR puff
        LDA #$05                ; a low rumble
        STA V3+1
        LDA #$0A
        STA V3+5
        LDA #0
        STA V3+4
        LDA #1
        STA V3+4
@rts:   RTS

; ---------------------------------------------------------------- sparks
; Each goes up four lines a frame. The ranks are a grid, so a spark doesn't
; look at 32 moths: its place says which one it could be touching (`COLOF`
; and `ROWOF`), and it looks at that one.
shots:  LDX #NS-1
@s:     LDA SHY,X
        BEQ @next
        SEC
        SBC #4
        CMP #5
        BCS @up
        LDA #0                  ; off the top
@up:    STA SHY,X
        BEQ @next
        SEC
        SBC #FY-5
        BCC @next
        CMP #44
        BCS @next
        TAY
        LDA ROWOF,Y
        STA T0
        LDA SHX,X
        SEC
        SBC FX
        BCC @next
        ADC #0                  ; one more: a spark is three wide
        CMP #120
        BCS @next
        TAY
        LDA COLOF,Y
        BMI @next               ; between two columns
        ORA T0
        TAY
        LDA MST,Y
        CMP #1
        BNE @next
        LDA #0                  ; a moth in the ranks, hit
        STA MST,Y
        STA SHY,X
        STA T2
        DEC LEFT
        CLC
        LDA FX
        ADC SLOTX,Y
        ADC #6
        STA T1
        LDA SLOTY,Y
        ADC #4
        STA T3
        LDA #12
        STA T4
        LDA ROWPTS,Y
        STA T5
        JSR puff
        STX DI
        LDA T5
        LDX #4                  ; tens
        JSR addsc
        LDX DI
        JSR s_hit
@next:  DEX
        BPL @s
        RTS

; Carry set if a spark is in the box round (HCX, HCY), HHW and HHH each way;
; then Y is the spark. Changes A and Y only.
hitbox: LDY #NS-1
@s:     LDA SHY,Y
        BEQ @next
        CLC
        ADC #3
        SEC
        SBC HCY
        BCS @down
        EOR #$FF
        ADC #1
@down:  CMP HHH
        BCS @next
        LDA SHX,Y
        SEC
        SBC HCX
        BCS @right
        EOR #$FF
        ADC #1
@right: CMP HHW
        BCS @next
        SEC
        RTS
@next:  DEY
        BPL @s
        CLC
        RTS

; ---------------------------------------------------------------- the ranks and the swoops
swarm:  LDA TICK                ; the ranks sway, a pixel every other frame
        AND #1
        BNE @still
        LDA FX
        CLC
        ADC FDIR
        STA FX
        CMP #FXMIN
        BEQ @turn
        CMP #FXMAX
        BNE @still
@turn:  LDA FDIR
        EOR #$FF
        CLC
        ADC #1
        STA FDIR
@still: LDA PHASE
        CMP #1
        BNE divers
        JSR launch
; falls through

; A swoop is half a turn of an angle, DT, from 0 to 128. The moth's height is
; its place in the ranks plus the sine of the angle times how far down it has
; to go, so it goes down and comes back. Across, it goes to where the lantern
; was when it left and back the same way (DDX times the same sine), with a
; swing to one side on the way down and the other on the way up (DSW times
; the sine of twice the angle). Three products, from the arithmetic unit.
divers: LDX #ND-1
@d:     LDA DMI,X
        BNE @on
@next:  DEX
        BPL @d
        RTS
@on:    CLC
        LDA DTL,X
        ADC DSPL
        STA DTL,X
        LDA DT,X
        ADC DSPH
        STA DT,X
        CMP #129
        BCC @fly
        LDY DMI,X               ; home: back in the ranks
        LDA #1
        STA MST-1,Y
        LDA #0
        STA DMI,X
        BEQ @next
@fly:   TAY
        LDA S1,Y
        STA NMULA
        LDA #0
        STA NMULA+1
        STA NMULB+1
        LDY DMI,X
        LDA #DEPTH-4
        SEC
        SBC SLOTY-1,Y
        STA NMULB               ; how far down
        LDA NPROD+1
        CLC
        ADC SLOTY-1,Y
        ADC #4
        STA DCY,X
        LDA DDX,X               ; how far across, signed
        STA NMULB
        ORA #$7F
        BMI @neg
        LDA #0
@neg:   STA NMULB+1
        CLC
        LDA FX
        ADC SLOTX-1,Y
        ADC #6
        CLC
        ADC NPROD+1
        STA DCXL,X
        LDA #0
        ADC NPROD+2
        STA DCXH,X
        LDY DT,X                ; the swing
        LDA S2,Y
        STA NMULA
        LDA S2H,Y
        STA NMULA+1
        LDA DSW,X
        STA NMULB
        ORA #$7F
        BMI @neg2
        LDA #0
@neg2:  STA NMULB+1
        CLC
        LDA DCXL,X
        ADC NPROD+1
        STA DCXL,X
        LDA DCXH,X
        ADC NPROD+2
        STA DCXH,X
        BEQ @seen
        JMP @next               ; off the side: nothing can touch it
@seen:  LDY DCY,X
        LDA SZW,Y               ; as wide as it's near
        STA DCW,X
        LDA DT,X                ; on the way down, it may shed dust
        CMP #36
        BCC @box
        CMP #56
        BCS @box
        JSR rnd
        CMP DUSTP
        BCS @box
        LDA DCXL,X
        STA T1
        LDA DCY,X
        STA T3
        JSR drop
@box:   LDA DCXL,X
        STA HCX
        LDA DCY,X
        STA HCY
        LDY DCW,X
        LDA WHH,Y
        CLC
        ADC #3
        STA HHH
        TYA
        LSR A
        CLC
        ADC #2
        STA HHW
        JSR hitbox
        BCC @man
        LDA #0                  ; a spark got it
        STA SHY,Y
        STA T2
        LDY DMI,X
        STA MST-1,Y
        STA DMI,X
        DEC LEFT
        LDA DCXL,X
        STA T1
        LDA DCY,X
        STA T3
        LDA DCW,X
        STA T4
        JSR puff
        STX DI
        LDA #1
        LDX #3                  ; a hundred: it was coming for you
        JSR addsc
        LDX DI
        JSR s_hit
        JMP @next
@man:   LDA DCY,X               ; low enough to touch him?
        CMP #DEPTH-12
        BCC @far
        LDA PX
        SEC
        SBC DCXL,X
        BCS @pos
        EOR #$FF
        ADC #1
@pos:   CMP HHW
        BCS @far
        JSR pkill
@far:   JMP @next

; Now and then one leaves the ranks.
launch: DEC LAUNCH
        BNE @rts
        LDA LINT
        STA LAUNCH
        LDA LEFT                ; the last few don't hang about
        CMP #8
        BCS @many
        LSR LAUNCH
@many:  LDA PDEAD
        BNE @rts
        LDX MAXD
        DEX
@slot:  LDA DMI,X
        BEQ @pick
        DEX
        BPL @slot
@rts:   RTS
@pick:  JSR rnd
        AND #NMOTH-1
        TAY
        LDA #NMOTH
        STA T0
@look:  LDA MST,Y
        CMP #1
        BEQ @got
        INY
        TYA
        AND #NMOTH-1
        TAY
        DEC T0
        BNE @look
        RTS
@got:   LDA #2
        STA MST,Y
        TYA
        CLC
        ADC #1
        STA DMI,X
        LDA #0
        STA DT,X
        STA DTL,X
        CLC
        LDA FX
        ADC SLOTX,Y
        ADC #6
        STA T0
        LDA PX                  ; aim at the lantern: up to 100 either way
        SEC
        SBC T0
        BCS @east
        CMP #<[-100]
        BCS @aim
        LDA #<[-100]
        BNE @aim
@east:  CMP #101
        BCC @aim
        LDA #100
@aim:   STA DDX,X
        JSR rnd
        AND #31
        CLC
        ADC #24
        STA DSW,X
        JSR rnd
        BPL @done
        LDA #0
        SEC
        SBC DSW,X
        STA DSW,X
@done:  RTS

; ---------------------------------------------------------------- dust
; A speck falls from (T1, T3), if there's room for one more.
drop:   LDY #NDU-1
@d:     LDA DUY,Y
        BEQ @free
        DEY
        BPL @d
        RTS
@free:  LDA T1
        STA DUX,Y
        LDA T3
        STA DUY,Y
        RTS

dust:   LDX #NDU-1
@d:     LDA DUY,X
        BEQ @next
        CLC
        ADC #2
        CMP #100
        BCC @fall
        LDA #0
@fall:  STA DUY,X
        CMP #PTOP+3
        BCC @next
        CMP #PTOP+18
        BCS @next
        LDA DUX,X
        SEC
        SBC PX
        BCS @pos
        EOR #$FF
        ADC #1
@pos:   CMP #6
        BCS @next
        LDA PDEAD
        ORA INVUL
        BNE @next
        LDA #0
        STA DUY,X
        JSR pkill
@next:  DEX
        BPL @d
        RTS

; A puff at (T1/T2, T3), for a moth T4 wide. The oldest goes if all eight are
; in use. Changes A and Y only.
puff:   LDY PFN
        INY
        CPY #NP
        BCC @ok
        LDY #0
@ok:    STY PFN
        LDA T1
        STA PFX,Y
        LDA T2
        STA PFXH,Y
        LDA T3
        STA PFY,Y
        LDA T4
        STA PFW,Y
        LDA #1
        STA PFA,Y
        RTS

; ---------------------------------------------------------------- the waves
phase:  LDA PT
        BEQ @which
        DEC PT
        BNE @which
        LDX #M_BLANK10          ; the banner has been read
        JSR puts
        LDA PHASE
        BNE @which
        INC PHASE               ; and the moths start
@which: LDA PHASE
        CMP #1
        BNE @emp
        LDA LEFT
        BNE @rts
        LDA #2                  ; the ranks are empty: the EMPEROR
        STA PHASE
        STA BRING
        LDA #1
        STA BST
        LDA LEVEL
        CMP #8
        BCC @hp
        LDA #8
@hp:    ASL A
        ASL A
        ADC #10
        STA BHP
        LDA #0
        STA BA
        STA BFL
        LDA #192                ; from far away
        STA BB
        LDA BDINT
        STA BDROP
        LDX #NE-1
        LDA #1
@ring:  STA EST,X
        DEX
        BPL @ring
        LDA #100
        STA PT
        LDX #M_EMP
        JMP puts
@emp:   CMP #2
        BNE @rts
        LDA BST
        BNE @rts
        INC LEVEL               ; it's dust: the next wave
        JMP newwave
@rts:   RTS

; ---------------------------------------------------------------- the EMPEROR
; Two angles. BA takes it from side to side. BB brings it nearer and takes it
; away: its width is BWT of that, from 40 to 88 pixels (the picture is 32), and
; the nearer it is the lower it flies, down to the lantern. Its ring of eight
; turns round it, as wide as it is, the far side behind it and the near side
; in front.
boss:   LDA BST
        BNE @here
        RTS
@here:  CMP #2
        BNE @alive
        JMP @dying
@alive: CLC
        LDA BAL
        ADC #$40
        STA BAL
        LDA BA
        ADC #1
        STA BA
        CLC
        LDA BBL
        ADC #$A0
        STA BBL
        BCC @same
        INC BB
@same:  LDX BA
        LDA BXT,X
        STA BX
        LDX BB
        LDA BWT,X
        STA BW
        JSR bsize
        LDA BFL
        BEQ @ring
        DEC BFL
@ring:  INC BRING
        LDA BW                  ; the ring: BW * 5/8 out, as twice that
        LSR A
        LSR A
        CLC
        ADC BW
        STA NMULA
        LDA #0
        STA NMULA+1
        LDA BW
        LSR A
        LSR A
        LSR A
        CLC
        ADC #10
        STA EW
        LSR A
        CLC
        ADC #2
        STA HHW
        LDA #8
        STA HHH
        LDX #NE-1
@e:     LDA EST,X
        BNE @eon
        JMP @enext
@eon:   TXA
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC BRING
        TAY
        LDA COS8,Y
        STA NMULB
        ORA #$7F
        BMI @neg
        LDA #0
@neg:   STA NMULB+1
        CLC
        LDA BX
        ADC NPROD+1
        STA EXL,X
        LDA #0
        ADC NPROD+2
        STA EXH,X
        LDA SIN8,Y
        STA NMULB
        ORA #$7F
        BMI @neg2
        LDA #0
@neg2:  STA NMULB+1
        STA EBK,X               ; $FF: the far side, behind it
        LDA NPROD+1
        CMP #$80
        ROR A                   ; half as far up and down: a ring seen from in front
        CLC
        ADC BY
        STA EY,X
        LDA EXH,X
        BNE @enext
        LDA EXL,X
        STA HCX
        LDA EY,X
        STA HCY
        JSR hitbox
        BCC @enext
        LDA #0                  ; one of the ring, hit
        STA SHY,Y
        STA EST,X
        STA T2
        LDA EXL,X
        STA T1
        LDA EY,X
        STA T3
        LDA EW
        STA T4
        JSR puff
        STX DI
        LDA #5
        LDX #4
        JSR addsc
        LDX DI
        JSR s_hit
@enext: DEX
        BMI @self
        JMP @e
@self:  LDA BX                  ; and itself: its body and the thick of its wings
        STA HCX
        LDA BY
        STA HCY
        LDA BW
        LSR A
        STA T0
        LSR A
        LSR A
        EOR #$FF
        SEC
        ADC T0
        STA HHW
        LDA BH
        LSR A
        STA HHH
        JSR hitbox
        BCC @touch
        LDA #0
        STA SHY,Y
        LDA #4
        STA BFL
        LDA #1
        LDX #4
        JSR addsc
        LDA #$14                ; a knock
        STA V3+1
        LDA #$06
        STA V3+5
        LDA #0
        STA V3+4
        LDA #1
        STA V3+4
        DEC BHP
        BNE @touch
        LDA #2                  ; that was the last of it
        STA BST
        LDA #90
        STA BDT
        LDX #NE-1
@off:   LDA EST,X
        BEQ @onext
        LDA #0
        STA EST,X
        STA T2
        LDA EXL,X
        STA T1
        LDA EY,X
        STA T3
        LDA EW
        STA T4
        LDA EXH,X
        BNE @onext
        JSR puff
@onext: DEX
        BPL @off
        RTS
@touch: LDA BH                  ; its wings on the lantern?
        LSR A
        CLC
        ADC BY
        CMP #PTOP+3
        BCC @shed
        LDA PX
        SEC
        SBC BX
        BCS @pos
        EOR #$FF
        ADC #1
@pos:   CMP HHW
        BCS @shed
        JSR pkill
@shed:  DEC BDROP
        BNE @rts
        LDA BDINT
        STA BDROP
        JSR rnd
        AND #31
        SEC
        SBC #16
        CLC
        ADC BX
        STA T1
        LDA BH
        LSR A
        LSR A
        CLC
        ADC BY
        STA T3
        JSR drop
@rts:   RTS
@dying: DEC BDT                 ; it shrinks away, in puffs
        BEQ @gone
        LDA BDT
        LSR A
        BCC @puffs
        LDA BW
        CMP #10
        BCC @puffs
        DEC BW
        JSR bsize
@puffs: LDA BDT
        AND #7
        BNE @rts
        JSR rnd
        AND #31
        SEC
        SBC #16
        CLC
        ADC BX
        STA T1
        LDA #0
        STA T2
        JSR rnd
        AND #15
        SEC
        SBC #8
        CLC
        ADC BY
        STA T3
        LDA #20
        STA T4
        JSR puff
        JMP s_hit
@gone:  LDA #0
        STA BST
        LDA #1
        LDX #2                  ; a thousand
        JMP addsc

; BH and BY from BW: three quarters as high as wide, and lower when nearer.
bsize:  LDA BW
        LSR A
        LSR A
        STA T0
        LDA BW
        SEC
        SBC T0
        STA BH
        LDA BW
        SEC
        SBC #40
        BCS @down
        LDA #0
@down:  LSR A
        CLC
        ADC #26
        STA BY
        RTS

; ---------------------------------------------------------------- drawing
; The ranks. Every one is the same size, so the sizes are stored once and a
; moth is five stores and its command: about 80 cycles each.
dform:  LDA LEFT
        BEQ @rts
        LDA #0
        STA NDX+1
        STA NDY+1
        STA NSY
        LDA #12
        STA NDW
        STA NSW
        LDA #9
        STA NDH
        STA NSH
        LDA TICK
        LSR A
        LSR A
        LSR A
        STA T0                  ; a step of the wing-beat every eight frames
        LDX #NMOTH-1
@m:     LDA MST,X
        CMP #1
        BNE @next
        CLC
        LDA SLOTX,X
        ADC FX
        STA NDX
        LDA SLOTY,X
        STA NDY
        TXA
        ADC T0                  ; each a step behind its neighbour: a ripple
        AND #3
        TAY
        LDA FRX,Y
        STA NSX
        LDA FRF,Y
        ORA ROWFL,X
        STA NFLAGS
@room:  BIT NSTATUS             ; `go`, without the call
        BVS @room
        LDA #BLIT
        STA NGO
@next:  DEX
        BPL @m
@rts:   RTS

; One moth, its middle at (MXL/MXH, MY), MW wide, at step MPH of its
; wing-beat, with MFL's ink. Changes A, X and Y.
moth:   LDY MW
        TYA
        LSR A
        STA T0
        SEC
        LDA MXL
        SBC T0
        STA NDX
        LDA MXH
        SBC #0
        STA NDX+1
        SEC
        LDA MY
        SBC WHH,Y
        STA NDY
        LDA #0
        SBC #0
        STA NDY+1
        STY NDW
        LDA WH,Y
        STA NDH
        LDA #0
        STA NSY
        LDX MPH
        CPY #18
        BCS @big
        LDA FRX,X               ; far: the small picture
        STA NSX
        LDA #12
        STA NSW
        LDA #9
        STA NSH
        BNE @flags
@big:   LDA FRXB,X              ; near: the big one
        STA NSX
        LDA #24
        STA NSW
        LDA #18
        STA NSH
@flags: LDA FRF,X
        ORA MFL
        STA NFLAGS
        LDA #BLIT
        JMP go

ddivers:
        LDX #ND-1
@d:     LDA DMI,X
        BEQ @next
        TAY
        LDA ROWFL-1,Y
        STA MFL
        LDA DCXL,X
        STA MXL
        LDA DCXH,X
        STA MXH
        LDA DCY,X
        STA MY
        LDA DCW,X
        STA MW
        STX DI
        LDA TICK                ; they beat faster when they swoop
        LSR A
        LSR A
        CLC
        ADC DI
        AND #3
        STA MPH
        JSR moth
        LDX DI
@next:  DEX
        BPL @d
        RTS

; The EMPEROR: the far half of its ring, itself, the near half.
dboss:  LDA BST
        BNE @here
        RTS
@here:  LDA #$FF
        JSR dring
        LDA BW
        LSR A
        STA T0
        SEC
        LDA BX
        SBC T0
        STA NDX
        LDA #0
        SBC #0
        STA NDX+1
        LDA BH
        LSR A
        STA T0
        SEC
        LDA BY
        SBC T0
        STA NDY
        LDA #0
        SBC #0
        STA NDY+1
        LDA BW
        STA NDW
        LDA BH
        STA NDH
        LDA #32
        STA NSW
        LDA #24
        STA NSH
        LDA #Y_EMP
        STA NSY
        LDA TICK
        LSR A
        LSR A
        LSR A
        AND #3
        TAX
        LDA EMX,X
        STA NSX
        LDA FRF,X
        ORA #KEY
        LDY BFL                 ; white when it's hit,
        BNE @white
        LDY BST                 ; and flickering as it goes
        CPY #2
        BNE @ink
        TAY
        LDA TICK
        AND #2
        BEQ @asis
        TYA
@white: ORA #WHITE
        BNE @ink
@asis:  TYA
@ink:   STA NFLAGS
        LDA #BLIT
        JSR go
        LDA #0
; falls through

; The ring's moths whose EBK is A.
dring:  STA T5
        LDX #NE-1
@e:     LDA EST,X
        BEQ @next
        LDA EBK,X
        CMP T5
        BNE @next
        LDA EXL,X
        STA MXL
        LDA EXH,X
        STA MXH
        LDA EY,X
        STA MY
        LDA EW
        STA MW
        LDA #KEY|GREEN
        STA MFL
        STX DI
        LDA TICK
        LSR A
        LSR A
        CLC
        ADC DI
        AND #3
        STA MPH
        JSR moth
        LDX DI
@next:  DEX
        BPL @e
        RTS

; A puff is the moth in white for three frames, then the dust picture,
; two pixels bigger every frame and turned a different way up, for twelve.
dpuffs: LDX #NP-1
@p:     LDA PFA,X
        BNE @on
        JMP @next
@on:    INC PFA,X
        CMP #4
        BCS @burst
        LDA PFX,X
        STA MXL
        LDA PFXH,X
        STA MXH
        LDA PFY,X
        STA MY
        LDA PFW,X
        STA MW
        LDA #1
        STA MPH
        LDA #KEY|WHITE
        STA MFL
        STX DI
        JSR moth
        LDX DI
        JMP @next
@burst: CMP #16
        BCC @grow
        LDA #0
        STA PFA,X
        BEQ @next
@grow:  SEC
        SBC #4
        ASL A
        CLC
        ADC PFW,X
        STA T0
        LSR A
        STA T1
        SEC
        LDA PFX,X
        SBC T1
        STA NDX
        LDA PFXH,X
        SBC #0
        STA NDX+1
        SEC
        LDA PFY,X
        SBC T1
        STA NDY
        LDA #0
        SBC #0
        STA NDY+1
        LDA T0
        STA NDW
        STA NDH
        LDA #12
        STA NSW
        STA NSH
        LDA #X_PUFF
        STA NSX
        LDA #0
        STA NSY
        LDA PFA,X
        AND #FLIPX|FLIPY
        ORA #KEY
        STA NFLAGS
        LDA #BLIT
        JSR go
@next:  DEX
        BMI @rts
        JMP @p
@rts:   RTS

; The lantern's light, flickering between two sizes, and its carrier.
dplayer:
        LDA PDEAD
        BNE @gone
        LDA INVUL               ; just back: he blinks
        AND #4
        BEQ @here
@gone:  RTS
@here:  LDA TICK
        LSR A
        AND #2
        CLC
        ADC #16
        STA NDW
        STA NDH
        LSR A
        STA T0
        SEC
        LDA LX
        SBC T0
        STA NDX
        LDA #0
        SBC #0
        STA NDX+1
        SEC
        LDA #PTOP+2
        SBC T0
        STA NDY
        LDA #0
        STA NDY+1
        LDA #X_GLOW
        STA NSX
        LDA #Y_EMP
        STA NSY
        LDA #16
        STA NSW
        STA NSH
        LDA #KEY
        STA NFLAGS
        LDA #BLIT
        JSR go
        SEC
        LDA PX
        SBC #7
        STA NDX
        LDA #0
        STA NDX+1
        LDA #PTOP
        STA NDY
        LDA #14
        STA NDW
        STA NSW
        LDA #18
        STA NDH
        STA NSH
        LDA #Y_MAN
        STA NSY
        LDA PWALK
        AND #8
        BEQ @step
        LDA #14
@step:  CLC
        ADC #X_MAN
        STA NSX
        LDA PFACE
        ORA #KEY
        STA NFLAGS
        LDA #BLIT
        JSR go
@rts:   RTS

dshots: LDA #0
        STA NDX+1
        STA NDY+1
        STA NSY
        LDA #4
        STA NDW
        STA NSW
        LDA #6
        STA NDH
        STA NSH
        LDA #X_SPARK
        STA NSX
        LDA #KEY
        STA NFLAGS
        LDX #NS-1
@s:     LDA SHY,X
        BEQ @next
        STA NDY
        LDA SHX,X
        SEC
        SBC #1
        STA NDX
        LDA #BLIT
        JSR go
@next:  DEX
        BPL @s
        RTS

ddust:  LDA #0
        STA NDX+1
        STA NDY+1
        STA NSY
        LDA #4
        STA NDW
        STA NSW
        STA NDH
        STA NSH
        LDA #X_DUST
        STA NSX
        LDA TICK                ; they tumble
        LSR A
        AND #FLIPX|FLIPY
        ORA #KEY
        STA NFLAGS
        LDX #NDU-1
@d:     LDA DUY,X
        BEQ @next
        SEC
        SBC #2
        STA NDY
        LDA DUX,X
        SEC
        SBC #2
        STA NDX
        LDA #0
        SBC #0
        STA NDX+1
        LDA #BLIT
        JSR go
@next:  DEX
        BPL @d
        RTS

; ---------------------------------------------------------------- the score
; A (1-9) more in digit X of the score (5 units, 4 tens, 3 hundreds...).
addsc:  CLC
        ADC SCORE,X
@carry: CMP #10
        BCC @store
        SBC #10
        STA SCORE,X
        DEX
        BMI @done
        LDA SCORE,X
        ADC #0                  ; the carry is set: one more
        BNE @carry
@store: STA SCORE,X
@done:  INC HUDD
        RTS

hud:    LDA HUDD
        BEQ @rts
        LDA #0
        STA HUDD
        LDX #5
@d:     LDA SCORE,X
        ORA #'0'
        STA SCREEN+7,X
        LDA HISC,X
        ORA #'0'
        STA SCREEN+20,X
        DEX
        BPL @d
        LDA LIVES
        ORA #'0'
        STA SCREEN+39
@rts:   RTS

; Message X: the words, where its first two bytes say.
puts:   LDA msgs,X
        STA PTR
        LDA msgs+1,X
        STA PTR+1
        LDY #0
        LDA (PTR),Y
        STA PT2
        INY
        LDA (PTR),Y
        STA PT2+1
        CLC
        LDA PTR
        ADC #2
        STA PTR
        BCC @same
        INC PTR+1
@same:  LDY #0
@c:     LDA (PTR),Y
        BEQ @done
        STA (PT2),Y
        INY
        BNE @c
@done:  RTS

; A random byte: a 16-bit xorshift (7, 9, 8). Changes A only.
rnd:    LDA SEEDH
        LSR A
        LDA SEEDL
        ROR A
        EOR SEEDH
        STA SEEDH
        ROR A
        EOR SEEDL
        STA SEEDL
        EOR SEEDH
        STA SEEDH
        RTS

; ---------------------------------------------------------------- sound
; Three voices that die away by themselves (sustain 0), so a sound is a pitch
; and the gate off and on.
sound:  LDA #15
        STA VOLUME
        LDA #128
        STA V1+2                ; sparks: a square wave
        LDA #4
        STA V1+3
        LDA #$06
        STA V1+5
        LDA #8                  ; moths: noise
        STA V2+3
        LDA #$07
        STA V2+5
        LDA #2                  ; bad news: a sawtooth
        STA V3+3
        LDA #0
        STA V1+6
        STA V2+6
        STA V3+6
        RTS

s_hit:  LDA #$30
        STA V2+1
        LDA #0
        STA V2+4
        LDA #1
        STA V2+4
        RTS

; ---------------------------------------------------------------- setting up the sheet
setup:  LDA #0
        STA NFLAGS
        STA NDX+1
        STA NDY+1
        STA NDW+1
        STA NDH+1
        STA NSY+1
        LDA #KEYC
        STA NKEY
        LDX #1                  ; the ink tables
@table: STX NTABLE
        LDY #15
@ink:   LDA #1                  ; 1: all white
        CPX #1
        BEQ @set
        LDA ink2,Y              ; 2: green moths
        CPX #2
        BEQ @set
        LDA ink3,Y              ; 3: red ones
@set:   STA NINK,Y
        DEY
        BPL @ink
        INX
        CPX #4
        BNE @table

; The pictures, through NVDATA: a byte is two pixels, and a line of the sheet
; is 128 bytes.
        LDX #0
@pic:   LDA pics,X
        STA PTR
        LDA pics+1,X
        STA PTR+1
        ORA PTR
        BEQ sky
        LDA pics+3,X            ; the line: 128 bytes each
        LSR A
        STA T1
        LDA #0
        ROR A
        STA T0
        LDA pics+2,X            ; the column: two to a byte
        LSR A
        ORA T0
        STA T0
        LDA pics+4,X
        STA T2
        LDA pics+5,X
        STA T3
@row:   LDA T0
        STA NVADDR
        LDA T1
        STA NVADDR+1
        LDY #0
@byte:  LDA (PTR),Y
        STA NVDATA
        INY
        CPY T2
        BNE @byte
        CLC
        LDA PTR
        ADC T2
        STA PTR
        BCC @same
        INC PTR+1
@same:  CLC
        LDA T0
        ADC #128
        STA T0
        BCC @same2
        INC T1
@same2: DEC T3
        BNE @row
        TXA
        CLC
        ADC #6
        TAX
        BNE @pic

; The sky: 72 lines of 128 bytes, straight through NVDATA: a line of the
; sheet is the whole width, so NVADDR runs on from one to the next. Black at
; the top goes to blue and then to the last of the sunset through mixtures:
; `skyrow` has the byte for each line, and lines of 06 and 60 by turns make a
; chessboard. Every mixture repeats every two pixels, so the sky looks the
; same wherever a copy starts, as long as that's an even column.
sky:    LDA #<[SKYY*128]
        STA NVADDR
        LDA #>[SKYY*128]
        STA NVADDR+1
        LDX #0
@line:  LDA skyrow,X
        LDY #128
@byte:  STA NVDATA
        DEY
        BNE @byte
        INX
        CPX #SKYH
        BNE @line
        LDA #SHEET
        STA NFLAGS
        LDA #1
        STA NDH
        LDA #0                  ; the stars: 90 of them, a pixel each
        LDA #1
        STA NDW
        LDX #90
@star:  JSR rnd
        STA NDX
        JSR rnd
        AND #31
        STA T0
        JSR rnd
        AND #15
        CLC
        ADC T0
        ADC #SKYY
        STA NDY
        JSR rnd
        AND #7
        TAY
        LDA starc,Y
        STA NCOLOUR
        LDA #FILL
        JSR go
        DEX
        BNE @star
        LDA #112                ; and the moon
        STA NDX
        LDA #SKYY+7
        STA NDY
        LDA #14
        STA NDW
        STA NDH
        STA NSW
        STA NSH
        LDA #X_MOON
        STA NSX
        LDA #Y_MAN
        STA NSY
        LDA #SHEET|KEY
        STA NFLAGS
        LDA #BLIT
        JSR go

; The trees: a strip of the see-through colour, black along the bottom, and
; seventeen trees from two pictures, each drawn at the size `treetab` says.
trees:  LDA #SHEET
        STA NFLAGS
        LDA #0
        STA NDX
        STA NDW
        LDA #1
        STA NDW+1
        STA NDY+1
        LDA #<TREEY
        STA NDY
        LDA #TREEH
        STA NDH
        LDA #KEYC
        STA NCOLOUR
        LDA #FILL
        JSR go
        LDA #<[TREEY+TREEH-5]
        STA NDY
        LDA #5
        STA NDH
        LDA #0
        STA NCOLOUR
        LDA #FILL
        JSR go
        LDA #0
        STA NDW+1
        LDA #Y_TREE
        STA NSY
        LDX #0
@tree:  LDA treetab,X           ; x, signed: one of them hangs over the left edge
        STA NDX
        ORA #$7F
        BMI @neg
        LDA #0
@neg:   STA NDX+1
        LDA treetab+1,X         ; x past 127
        BEQ @near
        LDA #0
        STA NDX+1
@near:  LDA treetab+2,X
        STA NDW
        LDA treetab+3,X
        STA NDH
        SEC
        LDA #<[TREEY+TREEH-4]
        SBC NDH
        STA NDY
        LDA treetab+4,X         ; which: bit 7 an oak, bit 1 FLIPX
        BMI @oak
        LDY #X_SPRUCE
        STY NSX
        LDY #16
        STY NSW
        LDY #28
        BNE @kind
@oak:   LDY #X_OAK
        STY NSX
        LDY #24
        STY NSW
        LDY #22
@kind:  STY NSH
        AND #FLIPX
        ORA #SHEET|KEY
        STA NFLAGS
        LDA #BLIT
        JSR go
        TXA
        CLC
        ADC #5
        TAX
        CPX #treeend-treetab
        BNE @tree

; The ground: grass with tufts along its top, and the path he walks on,
; both speckled.
ground: LDA #SHEET
        STA NFLAGS
        LDA #0
        STA NDX
        STA NDW
        LDA #1
        STA NDW+1
        STA NDY+1
        LDX #0
@band:  LDA bands,X             ; three FILLs: line, lines, colour
        STA NDY
        LDA bands+1,X
        STA NDH
        LDA bands+2,X
        STA NCOLOUR
        LDA #FILL
        JSR go
        INX
        INX
        INX
        CPX #9
        BNE @band
        LDA #0
        STA NDW+1
        LDA #1
        STA NDH
        LDX #200                ; speckles: the grass
@speck: JSR rnd
        STA NDX
        JSR rnd
        AND #7
        STA T0
        JSR rnd
        AND #3
        SEC
        ADC T0
        ADC #<[GRNDY+6]
        STA NDY
        JSR rnd
        AND #3
        TAY
        LDA grassc,Y
        STA NCOLOUR
        TYA
        LSR A
        ADC #1
        STA NDW
        LDA #FILL
        JSR go
        DEX
        BNE @speck
        LDX #120                ; and the path
@stone: JSR rnd
        STA NDX
        JSR rnd
        AND #7
        SEC
        ADC #<[GRNDY+18]
        STA NDY
        JSR rnd
        AND #3
        TAY
        LDA pathc,Y
        STA NCOLOUR
        TYA
        ADC #1
        STA NDW
        LDA #FILL
        JSR go
        DEX
        BNE @stone
        LDA #SHEET|KEY          ; 32 tufts along the top of the grass
        STA T5
        LDA #8
        STA NDW
        STA NSW
        LDA #6
        STA NDH
        STA NSH
        LDA #X_TUFT
        STA NSX
        LDA #Y_MAN
        STA NSY
        LDX #31
@tuft:  JSR rnd
        AND #3
        STA T0
        TXA
        ASL A
        ASL A
        ASL A
        ORA T0
        STA NDX
        JSR rnd
        AND #1
        CLC
        ADC #<GRNDY
        STA NDY
        JSR rnd
        AND #FLIPX
        ORA T5
        STA NFLAGS
        LDA #BLIT
        JSR go
        DEX
        BPL @tuft
        LDA #SHEET|KEY          ; and daisies
        STA NFLAGS
        LDA #4
        STA NDW
        STA NSW
        STA NDH
        STA NSH
        LDA #X_FLOWER
        STA NSX
        LDX #18
@daisy: JSR rnd
        STA NDX
        JSR rnd
        AND #7
        CLC
        ADC #<[GRNDY+4]
        STA NDY
        LDA #BLIT
        JSR go
        DEX
        BNE @daisy
        LDA #0
        STA NDY+1
        STA NFLAGS
        RTS

; ---------------------------------------------------------------- tables
flyy:   .byte 42, 46, 52, 50, 60, 57, 66, 70
flyc:   .byte 7, 13, 7, 7, 13, 7, 1, 13
lanoff: .byte 4, 0, <[-4]               ; the lantern from his middle: by PFACE
logox:  .byte 12, 40, 68, 96, 124
titfl:  .byte KEY, KEY|GREEN, KEY, KEY|RED
FRX:    .byte X_MSA, X_MSB, X_MSA, X_MSB        ; the wing-beat: up, level, down, level
FRXB:   .byte X_MBA, X_MBB, X_MBA, X_MBB
EMX:    .byte X_EMA, X_EMB, X_EMA, X_EMB
FRF:    .byte 0, 0, FLIPY, 0                    ; down is up, upside down

; The waves, 1 to 8: a swoop's speed (8.8, of 128 in all), frames between
; swoops, how many at once, the chance of dust, frames between the EMPEROR's.
lvspl:  .byte $20, $50, $80, $B0, $E0, $10, $40, $70
lvsph:  .byte 1, 1, 1, 1, 1, 2, 2, 2
lvint:  .byte 60, 50, 42, 36, 30, 26, 22, 18
lvmax:  .byte 3, 4, 4, 5, 5, 6, 6, 6
lvdust: .byte 8, 12, 16, 20, 24, 28, 32, 40
lvdrop: .byte 40, 34, 30, 26, 22, 18, 15, 12

ink2:   .byte 0, 1, 2, 3, 4, 5, 6, 1, 13, 5, 10, 11, 12, 13, 14, 15     ; orange 8 to light green, yellow 7 to white
ink3:   .byte 0, 1, 2, 3, 4, 5, 6, 10, 2, 9, 10, 11, 12, 13, 14, 15     ; orange to red, yellow to pink
starc:  .byte 1, 15, 15, 12, 12, 14, 7, 12
grassc: .byte 13, 11, 13, 11
pathc:  .byte 8, 11, 8, 0
bands:  .byte <GRNDY, GRNDH, KEYC       ; see-through,
        .byte <[GRNDY+6], 12, 5         ; grass
        .byte <[GRNDY+18], 10, 9        ; and the path

; The sky, line by line: the byte the line is made of.
skyrow: .byte $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $06, $00, $06, $00, $06, $00      ; a quarter blue
        .byte $06, $60, $06, $60, $06, $60      ; half
        .byte $06, $66, $06, $66, $06, $66      ; three quarters
        .byte $66, $66, $66, $66, $66, $66, $66, $66
        .byte $66, $66, $66, $66, $66, $66, $66, $66
        .byte $64, $66, $64, $66, $64           ; and so to purple
        .byte $46, $64, $46, $64, $46
        .byte $64, $44, $64, $44, $64
        .byte $44, $44, $44, $44, $44
        .byte $4A, $44, $4A, $A4, $4A, $AA      ; and a little pink
skyend:
        .assert skyend - skyrow == SKYH

; The trees: x (low byte), 1 if x is past 127, width, height, and which:
; $80 an oak, FLIPX mirrored. The first is the last again, over the join.
treetab:
        .byte <[-6], 0, 20, 35, 0
        .byte 8, 0, 16, 28, 0
        .byte 22, 0, 24, 22, $80
        .byte 40, 0, 12, 21, 0
        .byte 50, 0, 16, 28, FLIPX
        .byte 64, 0, 30, 27, $80
        .byte 92, 0, 20, 35, 0
        .byte 108, 0, 12, 21, 0
        .byte 118, 0, 24, 22, $80|FLIPX
        .byte 140, 1, 16, 28, 0
        .byte 152, 1, 14, 24, 0
        .byte 166, 1, 36, 33, $80
        .byte 198, 1, 16, 28, FLIPX
        .byte 210, 1, 20, 35, 0
        .byte 226, 1, 24, 22, $80
        .byte 244, 1, 12, 21, 0
        .byte 250, 1, 20, 35, 0
treeend:

; The pictures: where the rows are, the column and line on the sheet, bytes a
; row and rows.
pics:   .word p_msa
        .byte X_MSA, 0, 6, 9
        .word p_msb
        .byte X_MSB, 0, 6, 9
        .word p_mba
        .byte X_MBA, 0, 12, 18
        .word p_mbb
        .byte X_MBB, 0, 12, 18
        .word p_spark
        .byte X_SPARK, 0, 2, 6
        .word p_dust
        .byte X_DUST, 0, 2, 4
        .word p_puff
        .byte X_PUFF, 0, 6, 12
        .word p_ema
        .byte X_EMA, Y_EMP, 16, 24
        .word p_emb
        .byte X_EMB, Y_EMP, 16, 24
        .word p_glow
        .byte X_GLOW, Y_EMP, 8, 16
        .word p_man1
        .byte X_MAN, Y_MAN, 7, 18
        .word p_man2
        .byte X_MAN+14, Y_MAN, 7, 18
        .word p_moon
        .byte X_MOON, Y_MAN, 7, 14
        .word p_tuft
        .byte X_TUFT, Y_MAN, 4, 6
        .word p_flower
        .byte X_FLOWER, Y_MAN, 2, 4
        .word p_logo
        .byte X_LOGO, Y_LOGO, 20, 8
        .word p_spruce
        .byte X_SPRUCE, Y_TREE, 8, 28
        .word p_oak
        .byte X_OAK, Y_TREE, 12, 22
        .word 0

; The words: where on the screen, and what.
M_NONE1 = 0
M_NONE2 = 2
M_PRESS = 4
M_BLANK14 = 6
M_KEYS  = 8
M_NIECE = 10
M_WAVE  = 12
M_EMP   = 14
M_BLANK10 = 16
M_OVER  = 18
msgs:   .word m_none1, m_none2, m_press, m_blank14, m_keys, m_niece, m_wave, m_emp, m_blank10, m_over
m_none1: .word SCREEN+10*40+9
        .asciiz "MOTHS NEEDS THE NIECE."
m_none2: .word SCREEN+12*40+6
        .asciiz "SWITCH OFF, FIT IT, TRY AGAIN."
m_press: .word SCREEN+14*40+14
        .asciiz "PRESS SPACE"
m_blank14: .word SCREEN+14*40+14
        .asciiz "           "
m_keys: .word SCREEN+22*40+3
        .asciiz "LEFT, RIGHT: WALK    SPACE: SPARKS"
m_niece: .word SCREEN+24*40+6
        .asciiz "A DEMONSTRATION OF THE NIECE"
m_wave: .word SCREEN+10*40+17
        .asciiz "WAVE   "
m_emp:  .word SCREEN+10*40+14
        .asciiz "THE EMPEROR"
m_blank10: .word SCREEN+10*40+14
        .asciiz "           "
m_over: .word SCREEN+12*40+15
        .asciiz "GAME OVER"

; The first row of the screen: CLS left it blank, and this is put there once.
hudtxt: .text " SCORE 000000    HI 000000    LANTERNS 3"
hudend:
        .assert hudend - hudtxt == 40

; A moth's place in the ranks, its ink and what it's worth (tens): the top
; rank red, the bottom one green.
SLOTX:  .rept NMOTH, k
        .byte [k % NCOL] * COLW
        .endr
SLOTY:  .rept NMOTH, k
        .byte FY + [k / NCOL] * ROWH
        .endr
ROWFL:  .rept NMOTH, k
        .byte KEY + RED * [k / NCOL == 0] + GREEN * [k / NCOL == 3]
        .endr
ROWPTS: .rept NMOTH, k
        .byte 4 - k / NCOL
        .endr
; Which column a spark is in, by how far it is from the ranks' left edge (and
; one more), or $FF between two; and which rank, times 8, by its line.
COLOF:  .rept 120, k
        .byte [k % COLW <= 13] * [k / COLW] + [k % COLW > 13] * $FF
        .endr
ROWOF:  .rept 44, k
        .byte min(k / ROWH, 3) * NCOL
        .endr
; How wide a moth is drawn with its middle on line y: 12 in the ranks, 36 at
; the bottom of a swoop. And for a width, the height and half the height.
SZW:    .rept 128, k
        .byte 12 + min(24, max(0, k - 40) * 24 / [DEPTH - 40])
        .endr
WH:     .rept 64, k
        .byte k * 3 / 4
        .endr
WHH:    .rept 64, k
        .byte k * 3 / 8
        .endr
; Sines for a swoop, the angle from 0 to 128: twice the sine, and the sine of
; twice the angle, 16 bits.
S1:     .rept 129, k
        .byte 2 * sin(k, 127)
        .endr
S2:     .rept 129, k
        .byte <sin(2 * k, 127)
        .endr
S2H:    .rept 129, k
        .byte >sin(2 * k, 127)
        .endr
SIN8:   .rept 256, k
        .byte sin(k, 127)
        .endr
COS8:   .rept 256, k
        .byte cos(k, 127)
        .endr
BXT:    .rept 256, k                    ; the EMPEROR's x, and the title's sway
        .byte 80 + sin(k, 52)
        .endr
BWT:    .rept 256, k                    ; and its width
        .byte 64 + sin(k, 24)
        .endr
TXT:    .rept 256, k                    ; the title's moths
        .byte 80 + sin(k, 68)
        .endr
TYT:    .rept 256, k
        .byte 50 + sin(k, 30)
        .endr
LBOB:   .rept 256, k                    ; and its letters
        .byte 12 + sin(k, 4)
        .endr

; ---------------------------------------------------------------- the pictures
; A digit is a pixel: 0 black, 1 white, 2 red, 7 yellow, 8 orange, 9 brown,
; A pink, C grey, F light grey, and 4 is the see-through colour.
; the small moth, wings up: 12 x 9. Upside down it's wings down
p_msa:  .byte $88, $44, $44, $44, $44, $88
        .byte $87, $84, $44, $44, $48, $78
        .byte $87, $78, $44, $44, $87, $78
        .byte $48, $87, $8F, $F8, $78, $84
        .byte $44, $88, $81, $18, $88, $44
        .byte $44, $48, $8F, $F8, $84, $44
        .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
; the small moth, wings level
p_msb:  .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
        .byte $88, $78, $8F, $F8, $87, $88
        .byte $87, $78, $81, $18, $87, $78
        .byte $88, $78, $8F, $F8, $87, $88
        .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44
; the big moth, wings up: 24 x 18, for when they come near
p_mba:  .byte $40, $04, $44, $44, $44, $44, $44, $44, $44, $44, $40, $04
        .byte $08, $80, $44, $44, $44, $44, $44, $44, $44, $44, $08, $80
        .byte $08, $78, $04, $44, $44, $44, $44, $44, $44, $40, $87, $80
        .byte $08, $77, $80, $44, $44, $44, $44, $44, $44, $08, $77, $80
        .byte $08, $77, $78, $04, $44, $44, $44, $44, $40, $87, $77, $80
        .byte $08, $71, $07, $80, $44, $44, $44, $44, $08, $70, $17, $80
        .byte $40, $87, $77, $88, $00, $04, $40, $00, $88, $77, $78, $04
        .byte $44, $08, $88, $88, $99, $0F, $F0, $99, $88, $88, $80, $44
        .byte $44, $40, $08, $89, $99, $2F, $F2, $99, $98, $80, $04, $44
        .byte $44, $44, $00, $89, $99, $2F, $F2, $99, $98, $00, $44, $44
        .byte $44, $44, $44, $00, $99, $0F, $F0, $99, $00, $44, $44, $44
        .byte $44, $44, $44, $44, $00, $44, $44, $00, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
; the big moth, wings level
p_mbb:  .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $40, $00, $00, $00, $44, $44, $44, $44, $00, $00, $00, $04
        .byte $08, $87, $78, $88, $00, $0F, $F0, $00, $88, $87, $78, $80
        .byte $08, $71, $07, $88, $99, $2F, $F2, $99, $88, $70, $17, $80
        .byte $08, $71, $07, $88, $99, $2F, $F2, $99, $88, $70, $17, $80
        .byte $08, $87, $78, $88, $00, $0F, $F0, $00, $88, $87, $78, $80
        .byte $40, $00, $00, $00, $44, $44, $44, $44, $00, $00, $00, $04
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
; the EMPEROR, wings up: 32 x 24, with an eye on each wing
p_ema:  .byte $44, $00, $04, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $40, $00, $44
        .byte $40, $99, $90, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $09, $99, $04
        .byte $09, $88, $89, $04, $44, $44, $44, $44, $44, $44, $44, $44, $40, $98, $88, $90
        .byte $09, $88, $88, $90, $44, $44, $44, $44, $44, $44, $44, $44, $09, $88, $88, $90
        .byte $09, $87, $78, $89, $04, $44, $44, $44, $44, $44, $44, $40, $98, $87, $78, $90
        .byte $09, $87, $11, $78, $90, $44, $44, $44, $44, $44, $44, $09, $87, $11, $78, $90
        .byte $09, $87, $10, $17, $89, $04, $44, $44, $44, $44, $40, $98, $71, $01, $78, $90
        .byte $09, $87, $11, $78, $89, $00, $04, $44, $44, $40, $00, $98, $87, $11, $78, $90
        .byte $40, $98, $77, $88, $89, $9F, $F0, $44, $44, $0F, $F9, $98, $88, $77, $89, $04
        .byte $40, $98, $88, $AA, $89, $9C, $FF, $04, $40, $FF, $C9, $98, $AA, $88, $89, $04
        .byte $44, $09, $98, $AA, $89, $90, $CF, $F0, $0F, $FC, $09, $98, $AA, $89, $90, $44
        .byte $44, $40, $09, $98, $89, $04, $0C, $2F, $F2, $C0, $40, $98, $89, $90, $04, $44
        .byte $44, $44, $40, $09, $99, $04, $0C, $2F, $F2, $C0, $40, $99, $90, $04, $44, $44
        .byte $44, $44, $44, $40, $00, $04, $0C, $FF, $FF, $C0, $40, $00, $04, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $0C, $FF, $FF, $C0, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $40, $CF, $FC, $04, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $0F, $F0, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
; the EMPEROR, wings level
p_emb:  .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $00, $00, $00, $00, $44, $4F, $44, $44, $F4, $44, $00, $00, $00, $00, $44
        .byte $40, $98, $88, $88, $89, $00, $FF, $04, $40, $FF, $00, $98, $88, $88, $89, $04
        .byte $09, $88, $77, $78, $89, $90, $CF, $F0, $0F, $FC, $09, $98, $87, $77, $88, $90
        .byte $09, $87, $10, $17, $AA, $90, $C2, $F0, $0F, $2C, $09, $AA, $71, $01, $78, $90
        .byte $09, $87, $10, $17, $AA, $90, $C2, $F0, $0F, $2C, $09, $AA, $71, $01, $78, $90
        .byte $09, $88, $77, $78, $89, $90, $CF, $F0, $0F, $FC, $09, $98, $87, $77, $88, $90
        .byte $40, $98, $88, $88, $89, $00, $CF, $04, $40, $FC, $00, $98, $88, $88, $89, $04
        .byte $44, $00, $00, $00, $00, $44, $0C, $F0, $0F, $C0, $44, $00, $00, $00, $00, $44
        .byte $44, $44, $44, $44, $44, $44, $40, $F4, $4F, $04, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
; the lantern-carrier, facing right: 14 x 18. FLIPX turns him round
p_man1: .byte $44, $44, $44, $44, $44, $00, $44
        .byte $44, $44, $44, $44, $40, $77, $04
        .byte $44, $88, $84, $44, $40, $17, $04
        .byte $48, $88, $88, $44, $40, $77, $04
        .byte $99, $99, $99, $94, $44, $00, $44
        .byte $4A, $AA, $A4, $44, $44, $C4, $44
        .byte $4A, $A0, $A4, $44, $44, $C4, $44
        .byte $4A, $AA, $A4, $44, $4C, $44, $44
        .byte $44, $33, $34, $44, $AC, $44, $44
        .byte $43, $33, $33, $33, $AC, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $46, $33, $36, $64, $44, $44, $44
        .byte $44, $CC, $4C, $C4, $44, $44, $44
        .byte $44, $CC, $4C, $C4, $44, $44, $44
        .byte $44, $CC, $4C, $C4, $44, $44, $44
        .byte $40, $00, $40, $00, $44, $44, $44
; and mid-stride
p_man2: .byte $44, $44, $44, $44, $44, $00, $44
        .byte $44, $44, $44, $44, $40, $77, $04
        .byte $44, $88, $84, $44, $40, $17, $04
        .byte $48, $88, $88, $44, $40, $77, $04
        .byte $99, $99, $99, $94, $44, $00, $44
        .byte $4A, $AA, $A4, $44, $44, $C4, $44
        .byte $4A, $A0, $A4, $44, $44, $C4, $44
        .byte $4A, $AA, $A4, $44, $4C, $44, $44
        .byte $44, $33, $34, $44, $AC, $44, $44
        .byte $43, $33, $33, $33, $AC, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $43, $33, $33, $64, $44, $44, $44
        .byte $46, $33, $36, $64, $44, $44, $44
        .byte $44, $CC, $CC, $44, $44, $44, $44
        .byte $44, $CC, $CC, $C4, $44, $44, $44
        .byte $4C, $C4, $4C, $C4, $44, $44, $44
        .byte $40, $04, $44, $00, $04, $44, $44
; the lantern's light: 16 x 16, a pixel in eight
p_glow: .byte $44, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $74, $47, $44, $44, $44
        .byte $44, $44, $74, $44, $44, $47, $44, $44
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $44, $44, $74, $44, $44, $47, $44, $44
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $74, $44, $74, $44, $44, $47, $44, $47
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $74, $44, $74, $44, $44, $47, $44, $47
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $44, $44, $74, $44, $44, $47, $44, $44
        .byte $44, $74, $44, $74, $47, $44, $47, $44
        .byte $44, $44, $74, $44, $44, $47, $44, $44
        .byte $44, $44, $44, $74, $47, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44
; a spark: 3 x 6 (in a cell 4 wide)
p_spark:.byte $47, $44
        .byte $41, $44
        .byte $71, $74
        .byte $47, $44
        .byte $48, $44
        .byte $42, $44
; dust off a wing: 4 x 4
p_dust: .byte $4F, $C4
        .byte $F1, $FC
        .byte $CF, $FC
        .byte $4C, $C4
; a moth gone to dust: 12 x 12, drawn bigger each frame
p_puff: .byte $44, $44, $4F, $44, $44, $44
        .byte $44, $14, $44, $44, $74, $44
        .byte $44, $44, $48, $44, $4F, $44
        .byte $47, $44, $F4, $14, $44, $44
        .byte $44, $44, $44, $74, $48, $41
        .byte $F4, $41, $44, $44, $4F, $44
        .byte $44, $44, $F4, $47, $44, $44
        .byte $48, $44, $44, $41, $44, $74
        .byte $44, $47, $4F, $44, $44, $44
        .byte $14, $44, $44, $44, $84, $F4
        .byte $44, $4F, $44, $14, $44, $44
        .byte $44, $44, $44, $74, $41, $44
; the moon: 14 x 14
p_moon: .byte $44, $44, $41, $11, $14, $44, $44
        .byte $44, $41, $11, $11, $11, $14, $44
        .byte $44, $11, $11, $FF, $11, $11, $44
        .byte $41, $11, $FC, $CF, $11, $11, $14
        .byte $41, $1F, $CC, $F1, $11, $11, $14
        .byte $11, $11, $1F, $11, $11, $F1, $11
        .byte $11, $11, $11, $11, $11, $CF, $11
        .byte $11, $11, $11, $11, $1F, $F1, $11
        .byte $11, $F1, $11, $11, $11, $11, $11
        .byte $41, $FC, $F1, $11, $11, $1F, $14
        .byte $41, $1F, $11, $11, $1F, $CF, $14
        .byte $44, $11, $11, $11, $FF, $11, $44
        .byte $44, $41, $11, $11, $11, $14, $44
        .byte $44, $44, $41, $11, $14, $44, $44
; a tuft of grass: 8 x 6
p_tuft: .byte $44, $D4, $44, $44
        .byte $44, $54, $4D, $44
        .byte $D4, $54, $54, $44
        .byte $54, $54, $54, $45
        .byte $55, $55, $54, $55
        .byte $55, $55, $55, $55
; a daisy: 3 x 4
p_flower:.byte $41, $44
        .byte $17, $14
        .byte $41, $44
        .byte $4D, $44
; M O T H S: five letters of 8 x 8, drawn three times the size
p_logo: .byte $77, $44, $47, $74, $47, $77, $77, $44, $77, $77, $77, $74, $77, $44, $47, $74, $47, $77, $77, $44
        .byte $77, $74, $77, $74, $77, $44, $47, $74, $77, $77, $77, $74, $77, $44, $47, $74, $77, $44, $47, $74
        .byte $77, $77, $77, $74, $77, $44, $47, $74, $44, $77, $74, $44, $77, $44, $47, $74, $77, $44, $44, $44
        .byte $77, $47, $47, $74, $77, $44, $47, $74, $44, $77, $74, $44, $77, $77, $77, $74, $47, $77, $77, $44
        .byte $77, $44, $47, $74, $77, $44, $47, $74, $44, $77, $74, $44, $77, $77, $77, $74, $44, $44, $47, $74
        .byte $77, $44, $47, $74, $77, $44, $47, $74, $44, $77, $74, $44, $77, $44, $47, $74, $44, $44, $47, $74
        .byte $77, $44, $47, $74, $77, $44, $47, $74, $44, $77, $74, $44, $77, $44, $47, $74, $77, $44, $47, $74
        .byte $88, $44, $48, $84, $48, $88, $88, $44, $44, $88, $84, $44, $88, $44, $48, $84, $48, $88, $88, $44
; a spruce against the sky: 16 x 28
p_spruce:.byte $44, $44, $44, $40, $04, $44, $44, $44
        .byte $44, $44, $44, $40, $04, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $40, $00, $00, $04, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $40, $00, $00, $04, $44, $44
        .byte $44, $44, $00, $00, $00, $00, $44, $44
        .byte $44, $44, $40, $00, $00, $04, $44, $44
        .byte $44, $44, $00, $00, $00, $00, $44, $44
        .byte $44, $40, $00, $00, $00, $00, $04, $44
        .byte $44, $44, $00, $00, $00, $00, $44, $44
        .byte $44, $40, $00, $00, $00, $00, $04, $44
        .byte $44, $00, $00, $00, $00, $00, $00, $44
        .byte $44, $40, $00, $00, $00, $00, $04, $44
        .byte $44, $00, $00, $00, $00, $00, $00, $44
        .byte $40, $00, $00, $00, $00, $00, $00, $04
        .byte $44, $00, $00, $00, $00, $00, $00, $44
        .byte $40, $00, $00, $00, $00, $00, $00, $04
        .byte $00, $00, $00, $00, $00, $00, $00, $00
        .byte $40, $00, $00, $00, $00, $00, $00, $04
        .byte $00, $00, $00, $00, $00, $00, $00, $00
        .byte $44, $00, $00, $00, $00, $00, $00, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $44, $44, $44
; an oak: 24 x 22
p_oak:  .byte $44, $44, $44, $44, $40, $00, $04, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $00, $00, $00, $00, $04, $00, $04, $44, $44
        .byte $44, $44, $00, $00, $00, $00, $00, $00, $00, $00, $04, $44
        .byte $44, $40, $00, $00, $00, $00, $00, $00, $00, $00, $00, $44
        .byte $44, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $04
        .byte $40, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $04
        .byte $40, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $40, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $40, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $04
        .byte $44, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $44
        .byte $44, $40, $00, $00, $00, $00, $00, $00, $00, $00, $04, $44
        .byte $44, $44, $40, $00, $00, $40, $00, $00, $00, $04, $44, $44
        .byte $44, $44, $44, $40, $00, $04, $00, $00, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $00, $00, $44, $44, $44, $44, $44

; ---------------------------------------------------------------- more variables
RAM:
MST:    .res NMOTH              ; 0 gone, 1 in the ranks, 2 swooping
DMI:    .res ND                 ; swoops: the moth and one, 0 none
DDX:    .res ND                 ; how far across it goes, signed
DSW:    .res ND                 ; and its swing, signed
DCXL:   .res ND                 ; where it is now: its middle
DCXH:   .res ND
DCY:    .res ND
DCW:    .res ND                 ; and how wide
DUX:    .res NDU                ; dust
PFX:    .res NP                 ; puffs
PFXH:   .res NP
PFY:    .res NP
PFW:    .res NP
EST:    .res NE                 ; the ring: 1 there
EXL:    .res NE
EXH:    .res NE
EY:     .res NE
EBK:    .res NE                 ; $FF behind the EMPEROR
RAMEND:
        .assert RAMEND - RAM <= 256
