; AURORA: a horizontally scrolling shooter for the UNC 64 with the video board.
; United Northern Computers' demonstration cassette for the board, 1984.
;
; You fly LUMI low over the fells at night, under the northern lights, and on
; into the ice caves and the iron gorge. Things come at you in waves: WISPS
; that drift in sine waves, SKUAS that come in level and then dive at you,
; GROWLERS (chunks of ice that tumble and take a beating), and FROSTEYES, gun
; turrets grown into the rock. At the end of each stage the WARDEN wakes: a big
; eye, ringed by two SHARDS that stop your shots. Three stages, then round again,
; faster and meaner.
;
; Controls: cursor keys or W A S D fly (held keys, from the keyboard matrix, so
; you can fly diagonally and fire at once). SPACE fires: a tap is a shot, and
; holding it charges the LANCE (the ship's nose sparks, the bar in the HUD fills),
; which goes off when you let go and goes through everything it meets. Fully
; charged, the LANCE is a wide sprite, four times the size. SPACE also starts.
;
; Shoot down every one of a wave and it leaves a capsule: FAN (three shots at
; once), TWIN (two bolts in every shot) or a FIREFLY, a drone that follows you,
; fires with you and soaks up bullets. Losing a ship loses them. A ship more
; every 20,000 points.
;
; What it shows of the video board (manual chapter 15):
;   the sprite table: up to 24 things flying at once in entries 0-23 (the ship
;     is always entry 0), and the HUD, 8 more, in entries 24-31
;   eight a line: when SPRLINE says a line had more than eight, the table's
;     order turns round by one every frame for a while, so everyone flickers
;     instead of somebody vanishing (`table`)
;   SPRHIT and SPRHITBG on entry 0: the ship only looks for what it touched
;     when the board says its pixels met something (`shipcol`)
;   wide: the WARDEN, the full LANCE and the title's letters are 32 x 32
;   flips: GROWLERS tumble by flipping one image; a SKUA diving up is the
;     diving-down picture upside down
;   behind: SKUAS climb out of the ground and drop out of the roof, behind it
;   SCROLLX: the terrain moves a pixel a frame, and every eighth frame its
;     codes move a cell (`coarse`)
;   the layer, with its own scroll (LAYERSX): the aurora and the stars, at a
;     quarter of the speed, so the sky is far away
;   COLOUR1-3: the layer's ink is colour 1, and it cycles through the aurora's
;     greens; the terrain and the text are colour 3, a new one every stage;
;     colour 2 is white, for the bright parts of the sprites
;
; Without the board, BOARD ($C0BF) doesn't read $15, and AURORA says so and
; waits, instead of flying blind.
;
; The HUD is sprites. SCROLLX moves the whole picture, text and all, so a HUD
; in text would shake by up to seven pixels with every step of the terrain.
; Eight sprites in the top 16 lines don't move: each shows two characters,
; drawn into its image from the character set when they change (`hud`), one
; sprite a frame at most. Nothing else flies above line 16, so the HUD's band
; has exactly eight sprites a line, never more.
;
; The terrain is text: rock, its edges, slopes, spruces, icicles and the
; FROSTEYES are characters of our own set. Each new column is made at the
; right edge (`newcol`): the roof and the ground walk up and down within what
; the stage's style allows, a cell at a time, with a slope glyph where they
; change. Only the rows that hold terrain are moved at a coarse step: a count
; of columns by height says how deep the roof and the ground reach on screen,
; and how high the solid rock (which looks the same moved) goes, and the copy
; loop is entered at the first row to move and left after the last
; (`shiftrows`). The worst step moves 15 rows, about 7,000 cycles; the caves
; usually move ten or twelve.
;
; Sound: one voice, the beeper's timer (chapter 12), stepped once a frame by a
; little sequencer (`sound`). Effects have priorities; when nothing plays and
; the LANCE is charging, the timer hums, higher as it charges.
;
; Paced by FRAME: one step of the game a frame, 50 a second. `work` to
; `endwork` is one frame's work: about 5,500 of the 19,968 cycles on the
; average frame, and 18,500 at the very worst (a coarse step in the gorge,
; eight shots, eight enemies, five bullets and a banner, all at once). A
; coarse step's frame leaves the HUD to the next one.
;
; On tape it's called AURORA: LOAD "AURORA" and press PLAY, with the board in.
;
; Memory (a UNC 16 is enough):
;   $0000-$00CF  variables, the shots', the enemies' and the bullets' hot
;                fields (the fast page; the ROM's $D0-$FE untouched)
;   $0400-$07E7  the text screen: the terrain, in rows 2-24
;   $0800-       code, tables, the stage scripts, the glyphs, then the
;                sprite images, on a page of their own (SPRBANK points there)
;   $3600-$36FF  more variables (.res, not on the tape)
;   $3700-$377F  the sprite table, where the board looks after reset
;   $3780-$37FF  the terrain's ring of heights
;   $3800-$3BFF  our character set: UNSCII copied, with our glyphs on top
;                (the board's images would be here; ours are in the program),
;                and a few buffers in glyphs we never show
;   $3C00-$3FE7  the layer, ditto
;
; The tests (crates/unc-asm/tests/game_aurora.rs) find STATE, SHX, SHY, LIVES,
; SCORE (six digits, 0-9, the highest first), FINE, ENT, SOT, BUT, TACT, LATE,
; NOCHK and the rest by name in the symbol table.

; ---------------------------------------------------------------- the machine
SCREEN  = $0400
VMODE   = $C000
FRAME   = $C006
KBDS    = $C011         ; KBD_STROBE: the last key pressed, taken
BORDER  = $C020
BGCOL   = $C021
CHARSET = $C022
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 speaker data (chapter 12)
TIMER_LO = $C032        ; the timer's divisor: the low byte first,
TIMER_HI = $C033        ; then the high one, which loads it
NOISE   = $C034         ; a byte from the cycle it's read on
KEYS    = $C060         ; the keyboard matrix, rows 0-8 (chapter 11)
SPRTAB  = $C0A0         ; the video board (chapter 15)
SPRBANK = $C0A1
SPRHIT  = $C0A4
SPRHITBG = $C0A5
SPRLINE = $C0A6
SCROLLX = $C0A8
SCROLLY = $C0A9
LAYER   = $C0AA
LAYERPG = $C0AB
LAYERSX = $C0AC
LAYERSY = $C0AD
COLMODE = $C0B0
COLOUR1 = $C0B1
COLOUR2 = $C0B2
COLOUR3 = $C0B3
BOARD   = $C0BF         ; $15 with the board in
BOARD_ID = $15
CHARROM = $D000

TABLE   = $3700         ; the sprite table: entry n at TABLE + n * 4
LAYERP  = $3C00         ; the layer: 40 x 25 codes
MYSET   = $3800         ; our character set (where the board's images would be)
RAM     = $3600         ; variables the fast page has no room for

; ---------------------------------------------------------------- tuning
NSH     = 8             ; shots
NEN     = 8             ; enemies, explosions and capsules
NBU     = 5             ; their bullets
NSTAR   = 14
SXMIN   = 2             ; the ship's x, in 2-pixel units
SXMAX   = 140
SYMIN   = 16            ; and its y, in pixels: nothing flies in the HUD's band
SYMAX   = 184
COOLT   = 5             ; frames between volleys
CHGSHOW = 12            ; frames of holding SPACE before the nose sparks
CHGMIN  = 22            ; let go after this many and the LANCE goes off
CHGMAX  = 64            ; full: the wide LANCE
DEATHT  = 110           ; frames from blowing up to the next ship
INVULT  = 120           ; frames of blinking safety
FLICKT  = 25            ; frames of turning the table round after SPRLINE
BANROW  = 12            ; the row the banners scroll along
ROOFTOP = 2             ; the terrain's first row

; Things in the enemies' slots
E_WISP  = 1
E_SKUA  = 2
E_GROWL = 3
E_WARDEN = 4
E_SHARD = 5
E_EXPL  = 6
E_PICK  = 7
F_SHOOT = 1             ; shots hurt it
F_BLOCK = 2             ; shots stop on it
F_HURT  = 4             ; it hurts the ship
F_PICK  = 8             ; the ship picks it up

; Weapons
W_FAN   = 1
W_TWIN  = 2

; Sprite attributes (byte 3 of an entry)
A_SHOWN = $80
A_FLIPY = $10
A_FLIPX = $08
A_BEHIND = $04
A_WIDE  = $02

; The stage scripts' records: type, count, parameter, frames after
R_WISP  = E_WISP
R_SKUA  = E_SKUA
R_GROWL = E_GROWL
R_STYLE = 8             ; count: the terrain's new style
R_BANNER = 9            ; count: which banner
R_BOSS  = 10
R_LOOP  = 11            ; back to the start of the title's script

; Terrain styles (see `styles`)
S_TITLE = 0
S_FELLS = 1
S_CAVE  = 2
S_GORGE = 3
S_FLAT  = 4
S_LOW   = 5

; Sound effects, by priority (see `fxtab`)
FX_SHOT = 0
FX_TICK = 1
FX_POP  = 2
FX_LANCE = 3
FX_BOOM = 4
FX_PICK = 5
FX_STAGE = 6
FX_WARN = 7
FX_DIE  = 8
FX_OVER = 9

I_SHIP  = 0
I_SHIPC = 1
I_SHIPF = 2
I_FLY0  = 3
I_FLY1  = 4
I_SHOT  = 5
I_TWIN  = 6
I_LANCE = 7
I_WISP0 = 8
I_WISP1 = 9
I_SKUA  = 10
I_SKUAD = 11
I_GROWL = 12
I_BUL   = 13
I_EXP0  = 14
I_EXP1  = 15
I_EXP2  = 16
I_EXP3  = 17
I_PICKF = 18
I_PICKT = 19
I_PICKY = 20
I_WARD0 = 21
I_WARD1 = 22
I_SHARD = 23
I_DYN   = 24       ; images 24-31: built at run time (the HUD, the title's letters)
G_BAR0   = $00
G_BAR1   = $01
G_BAR2   = $02
G_BAR3   = $03
G_BAR4   = $04
G_BAR5   = $05
G_BAR6   = $06
G_BAR7   = $07
G_BAR8   = $08
G_ROCK   = $60
G_CEIL   = $61
G_CDN    = $62
G_CUP    = $63
G_FLOOR  = $64
G_FUP    = $65
G_FDN    = $66
G_EYEF   = $67
G_EYEC   = $68
G_PITF   = $69
G_PITC   = $6A
G_TREE   = $6B
G_ICE    = $6C
G_LIFE   = $6D
G_CTIP   = $6E
G_FTIP   = $6F
G_FRINGE = $70
G_FRINGE2= $71
G_RAY    = $72
G_RAY2   = $73
G_FADE   = $74
G_FADE2  = $75
G_STAR   = $78
G_STAR2  = $79
G_STAR3  = $7A

; ---------------------------------------------------------------- the fast page
        .org $00
STATE:  .res 1          ; 0 no board, 1 title, 2 playing, 3 game over
TICK:   .res 1          ; frames, counted by the game
LASTF:  .res 1          ; FRAME when the last frame's work began
LATE:   .res 1          ; frames of play whose work ran into the next frame
NOCHK:  .res 1          ; not 0: this frame did set-up (a new screen), not play
KEYIN:  .res 1          ; the key pressed this frame (KBD), or 0
PAD:    .res 1          ; the controls down: bit 0 left, 1 right, 2 up, 3 down, 4 fire
OLDPAD: .res 1          ; and last frame
SCR:    .res 1          ; the scroll's counter: SCROLLX is its bits 0-2
FINE:   .res 1          ; SCROLLX as written
SCRON:  .res 1          ; not 0: the terrain scrolls
LAYON:  .res 1          ; not 0: the layer scrolls
SHX:    .res 1          ; the ship: x in 2-pixel units
SHY:    .res 1          ;   and y in pixels
SHIPST: .res 1          ; 0 flying, 1 blown up (or no ship)
INVUL:  .res 1          ; frames of safety left
DEADT:  .res 1          ; frames until the next ship
CHG:    .res 1          ; frames SPACE has been held: the LANCE's charge
COOL:   .res 1          ; frames until the guns are ready
LIVES:  .res 1
LIFEK:  .res 1          ; ships given for points so far: the score / 20,000
STAGE:  .res 1          ; 0, 1, 2, 3 ...
DIFF:   .res 1          ; how mean: the stage, up to 7
WEAP:   .res 1          ; W_FAN, W_TWIN
DRON:   .res 1          ; 1: the FIREFLY is with us
DRX:    .res 1
DRY:    .res 1
SCORE:  .res 6          ; six decimal digits, the highest first
HISC:   .res 6
RNDL:   .res 1          ; the dice: a 16-bit xorshift
RNDH:   .res 1
HIT:    .res 1          ; SPRHIT, SPRHITBG as read this frame
HITBG:  .res 1
FLICK:  .res 1          ; frames left of turning the table round
ROTN:   .res 1          ; how far round it is
NOBJ:   .res 1          ; entries 1-23 in use this frame
TEND:   .res 1          ; (NOBJ + 1) * 4: where the table's writing wraps
PTR:    .res 2
PTR2:   .res 2
GP1:    .res 2          ; glyph pointers
GP2:    .res 2
SRC:    .res 2
T0:     .res 1          ; scratch
T1:     .res 1
T2:     .res 1
T3:     .res 1
T4:     .res 1
T5:     .res 1
T6:     .res 1
T7:     .res 1
SX0:    .res 1          ; where to put an explosion or a capsule
SY0:    .res 1
SV0:    .res 1
JV:     .res 2          ; a routine, for JMP (JV)
SNDT:   .res 1          ; frames left of the sound's step, 0 when quiet
SNDP:   .res 2          ; its next step
SNDPRI: .res 1          ; the effect playing, which is its priority + 1 (0 quiet)
SNDN:   .res 1          ; a noise step's mask, 0 for a tone
SNDB:   .res 1          ; a noise step's base
SPTR:   .res 2
HUM:    .res 1          ; not 0: the charge hum has the timer
RP:     .res 1          ; the terrain's ring: the slot of screen column 0
GC:     .res 1          ; the column being made
CA:     .res 1          ; roof heights (rows) of the columns before the new one
CB:     .res 1
FA:     .res 1          ; and the ground's
FB:     .res 1
MAXC:   .res 1          ; the deepest roof on screen (rows, with what hangs under it)
MAXF:   .res 1          ; the highest ground, the same
MINC:   .res 1          ; the shallowest roof and the lowest ground, the same
MINF:   .res 1
PTCH:   .res 2          ; the instruction shiftrows has made a JMP, or 0
NBOX:   .res 1          ; shootable things this frame, in BX0-BSL
STYLE:  .res 1          ; the terrain's style
AUB:    .res 1          ; the aurora: its lower edge (layer row)
AUL:    .res 1          ;   columns left of this curtain
AUG:    .res 1          ;   columns left of the gap after it
WAVEP:  .res 2          ; the stage script's next record
WAVET:  .res 1          ; frames until the script's next move
WTYPE:  .res 1          ; the wave coming in
WLEFT:  .res 1          ;   how many more
WPARAM: .res 1
WPOST:  .res 1          ;   frames after its last
WSER:   .res 1          ; the wave's number, 1-4 round
BANNER: .res 1          ; columns of banner still to scroll by
BANP:   .res 2          ; the banner's next character
BOSS:   .res 1          ; the WARDEN's slot + 1, or 0
CLEART: .res 1          ; frames until the next stage
OVERT:  .res 1          ; game over: frames so far (stops at 255)
LSER:   .res 1          ; LANCEs fired, so each hits a thing once
WIPE:   .res 1          ; the next aurora column to rub out, in a cave
TCOL:   .res 4          ; FROSTEYES: column, row, in use
TACT:   .res 4
CNTC:   .res 8          ; columns on screen with each roof height, 0-7
CNTF:   .res 9          ;   and ground height, 0-8
SOT:    .res NSH        ; shots: 0 none, 1 shot, 2 TWIN, 3 LANCE, 4 wide LANCE
SOX:    .res NSH
SOY:    .res NSH
ENT:    .res NEN        ; enemies: 0 none, or E_...
ENX:    .res NEN        ; x in 2-pixel units: 0-159 on the screen, 160-199 waiting
ENY:    .res NEN        ;   at the right, 249-255 going off at the left
ENC:    .res NEN        ; a counter each: phase, timer, frame
BUT:    .res NBU        ; bullets: 0 none, 1 flying
BUX:    .res NBU
BUY:    .res NBU
VEND:
        .assert VEND <= $D0, "the variables run into the ROM's $D0-$FE"

; ---------------------------------------------------------------- more variables
        .org RAM
SOVY:   .res NSH        ; a shot's y step, pixels a frame (signed)
SOLS:   .res NSH        ; a LANCE's number (LSER)
ENXF:   .res NEN        ; fractions of x and y
ENYF:   .res NEN
ENVX:   .res NEN        ; steps a frame, signed: x in 1/16 units, y in 1/16 pixels
ENVY:   .res NEN
ENHP:   .res NEN        ; hits left
ENB:    .res NEN        ; a WISP's middle line; a SHARD's angle; a capsule's kind
ENS:    .res NEN        ; a state; a SHARD's WARDEN
ENIMG:  .res NEN        ; the image it shows
ENATT:  .res NEN        ; its flips, wide, behind
ENWS:   .res NEN        ; its wave's number, 0 if none
ENLH:   .res NEN        ; the last LANCE that hit it
BUXF:   .res NBU
BUYF:   .res NBU
BUVX:   .res NBU
BUVY:   .res NBU
BX0:    .res NEN        ; the boxes of the things shots can hit, this frame
BX1:    .res NEN
BY0:    .res NEN
BY1:    .res NEN
BSL:    .res NEN        ;   and whose they are
BFLASH: .res 1          ; frames of the WARDEN's eye shut
OVERT2: .res 1          ;   and quarter seconds
AURI:   .res 1          ; the aurora's colour, a step every 8 frames
HUDK:   .res 1          ; the HUD sprite to look at next
LOGOPH: .res 1          ; the title's letters bob
FLASHB: .res 1          ; frames of red border
FLASH2: .res 1          ; frames of colour 2 in yellow
PKN:    .res 1          ; the next capsule: 0 FAN, 1 TWIN, 2 FIREFLY
TROW:   .res 4
RAMEND:
        .assert RAMEND <= TABLE, "the variables run into the sprite table"

; ...after the sprite table, in the rest of its page,
        .org TABLE+$80
RINGC:  .res 64         ; roof and ground heights of the columns, round a ring:
RINGF:  .res 64         ;   screen column c is slot (RP + c) AND 63
        .assert * <= MYSET, "the ring runs into the character set"

; ...and in our character set, over glyphs we never show: $10-$1F (UNSCII's
; lines and corners) and $5B-$5F ([ \ ] ^ _). `initset` copies UNSCII over
; them, so each is set before it's used.
        .org MYSET+$10*8
COLB:   .res 25         ; the new column, row by row
HUDC:   .res 16         ; the HUD's 16 characters as they should be
HUDS:   .res 16         ;   and as they are drawn
STC:    .res NSTAR      ; the stars: column, row, glyph
STR:    .res NSTAR
STG:    .res NSTAR
WKILL:  .res 4          ; kills in each of the last four waves
WSIZE:  .res 4          ;   out of
PSAVE:  .res 3          ; the instruction under shiftrows' JMP
OPENT:  .res 1          ; the first line no terrain reaches, and the line after the last
OPENB:  .res 1
        .assert * <= MYSET+$20*8, "the buffers run past glyph $1F"
        .org MYSET+$5B*8
BANBUF: .res 40         ; the stage's banner
        .assert * <= MYSET+$60*8, "the banner runs into glyph $60"

; ---------------------------------------------------------------- start
; The loader starts us with SP = $FF, interrupts off and A = X = Y = 0 (chapter 6).
        .org $0800
start:  LDA BOARD
        CMP #BOARD_ID
        BEQ @board
        JMP noboard
@board: LDA #0
        LDX #VEND
@fp:    DEX
        STA $00,X
        BNE @fp
@ram:   STA RAM,X           ; X is 0: a page of variables, the table and
        STA RAM+$100,X      ; the page after it
        STA DYN,X           ; the images we draw at run time
        STA DYN+$100,X
        INX
        BNE @ram
        JSR initset
        LDA #>MYSET
        STA CHARSET
        LDA #%00001000      ; text, and $A0 doesn't blink (we never show it)
        STA VMODE
        LDA #>IMAGES
        STA SPRBANK
        LDA #>TABLE
        STA SPRTAB
        LDA #>LAYERP
        STA LAYERPG
        LDA #0
        STA BGCOL
        STA BORDER
        STA SCROLLY
        STA LAYERSY
        STA COLMODE
        LDA #1              ; colour 2: white
        STA COLOUR2
        LDA #$E1            ; the dice, until SPACE seeds them
        STA RNDL
        LDA #$AC
        STA RNDH
        JSR gotitle
        LDA FRAME
        STA LASTF

; ---------------------------------------------------------------- the main loop
; Wait for the frame counter, then do one frame's work: fifty a second.
main:   LDA LASTF
@wait:  CMP FRAME
        BEQ @wait
        LDA FRAME
        TAX
        SEC
        SBC LASTF
        STX LASTF
        CMP #2
        BCC work
        LDA NOCHK           ; a set-up frame may take two; play may not
        BNE work
        INC LATE
work:   LDA #0
        STA NOCHK
        INC TICK
        LDA KBDS            ; a new press, taken
        BMI @key
        LDA #0
@key:   AND #$7F
        STA KEYIN
        JSR latch
        LDA STATE
        ASL A
        TAX
        LDA states,X
        STA JV
        LDA states+1,X
        STA JV+1
        JSR jumpv
        JSR sound           ; after everything that may start a sound
endwork: JMP main
jumpv:  JMP (JV)

states: .word noboard, st_title, st_play, st_over

; What the board found as the last frame ended: who touched what, and whether a
; line had more than eight sprites. Reading clears them, so read them once.
latch:  LDA SPRHIT
        STA HIT
        LDA SPRHITBG
        STA HITBG
        LDA SPRLINE         ; bit 7: a line had too many
        BPL @calm
        LDA #FLICKT
        STA FLICK
        RTS
@calm:  LDA FLICK
        BEQ @r
        DEC FLICK
@r:     RTS

; ---------------------------------------------------------------- no board
; Without the board every address in $C080-$C0BF reads $FF. Sprites, the
; scroll and the colours would all be missing, so say so, and wait.
noboard:
        LDA #$20
        LDX #0
@cls:   STA SCREEN,X
        STA SCREEN+$100,X
        STA SCREEN+$200,X
        STA SCREEN+$2E8,X
        INX
        BNE @cls
        LDX #<t_nb
        LDY #>t_nb
        JSR scrlist
@idle:  LDA FRAME           ; wait, politely, a frame at a time
@w:     CMP FRAME
        BEQ @w
        JMP @idle

; ---------------------------------------------------------------- the title
gotitle:
        LDA #1
        STA STATE
        STA NOCHK
        STA SCRON
        STA SHIPST          ; no ship
        STA LAYER           ; the layer on
        LDA #0
        STA LAYON           ; and still: the title's words are on it
        STA LAYERSX
        STA BANNER
        STA DEADT
        STA CHG
        JSR quiet
        JSR clearall
        JSR clrscreen
        LDA #S_TITLE
        STA STYLE
        JSR terrain0
        JSR layer0
        LDX #<t_title
        LDY #>t_title
        JSR laylist
        JSR showhi
        JSR logo0
        LDA #14             ; light blue fells
        STA COLOUR3
        LDA #<sc_title
        STA WAVEP
        LDA #>sc_title
        STA WAVEP+1
        LDA #60
        STA WAVET
        LDA #0
        STA WLEFT
        STA OVERT
        LDA KBDS            ; forget a key pressed before now
        RTS

; The high score on the title, in the layer.
showhi: LDX #5
@d:     LDA HISC,X
        ORA #'0'
        STA LAYERP+15*40+18,X
        DEX
        BPL @d
        RTS

st_title:
        JSR scroll
        JSR waves
        JSR enemies
        JSR logo
        JSR table
        JSR colours
        LDA TICK            ; PRESS SPACE blinks
        AND #31
        BNE @key
        LDA TICK
        AND #32
        BEQ @on
        LDX #10
        LDA #$20
@off:   STA LAYERP+12*40+15,X
        DEX
        BPL @off
        BMI @key
@on:    LDX #10
@txt:   LDA t_press,X
        STA LAYERP+12*40+15,X
        DEX
        BPL @txt
@key:   LDA KEYIN
        CMP #' '
        BNE @r
        JMP newgame
@r:     RTS

; The title's letters: AURORA in six wide sprites, entries 24-29, which bob.
logo:   INC LOGOPH
        LDX #5
@l:     LDA LOGOPH
        ASL A
        CLC
        ADC logoph,X
        TAY
        LDA SIN,Y           ; -32..32
        CMP #$80            ; / 8, keeping the sign
        ROR A
        CMP #$80
        ROR A
        CMP #$80
        ROR A
        CLC
        ADC #22
        LDY logoent,X
        STA TABLE,Y
        DEX
        BPL @l
        RTS

; Draws the letters A, U, R and O into images I_DYN to I_DYN+3, each pixel of
; the character set's glyph four pixels of the image (2 x 2), and the image
; drawn wide: 28 x 28 pixels on the screen. The top rows are white, the
; bottom colour 3, like snow on the fells, over the aurora behind them.
logo0:  LDX #3
@let:   LDA logoch,X
        JSR glyphp          ; GP1 -> the glyph
        TXA                 ; PTR -> image I_DYN + X
        JSR dynp
        LDY #0
@row:   STY T0              ; glyph row
        LDA (GP1),Y
        STA T1
        LDA lettercm,Y
        STA T2
        TYA                 ; image offset: 8 per glyph row (two rows of 4)
        ASL A
        ASL A
        ASL A
        TAY
        LDA T1
        JSR dbl
        LDA T1
        ASL A
        ASL A
        ASL A
        ASL A
        JSR dbl
        ; the same row again, below it
        TYA
        SEC
        SBC #4
        TAY
        LDA T1
        JSR dbl0
        LDA T1
        ASL A
        ASL A
        ASL A
        ASL A
        JSR dbl0
        LDY T0
        INY
        CPY #8
        BNE @row
        DEX
        BPL @let
        ; the table entries 24-29: wide, shown
        LDX #5
@ent:   LDY logoent,X
        LDA #24
        STA TABLE,Y
        LDA logox,X
        STA TABLE+1,Y
        LDA logoimg,X
        STA TABLE+2,Y
        LDA #A_SHOWN+A_WIDE
        STA TABLE+3,Y
        DEX
        BPL @ent
        LDA #0              ; 30 and 31: nothing
        STA TABLE+30*4+3
        STA TABLE+31*4+3
        RTS

; dbl: the high nibble of A, 4 pixels, doubled: 8 pixels, two bytes at (PTR),Y
; and (PTR),Y+1, in colour mask T2. Y moves on 2. dbl0 writes them 4 bytes on
; (the next image row) and moves Y on 2 as well.
dbl:    PHA
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        STY T3
        TAY
        LDA half,Y
        AND T2
        LDY T3
        STA (PTR),Y
        INY
        PLA
        LSR A
        LSR A
        LSR A
        LSR A
        AND #3
        STY T3
        TAY
        LDA half,Y
        AND T2
        LDY T3
        STA (PTR),Y
        INY
        RTS
dbl0:   PHA
        TYA
        CLC
        ADC #4
        TAY
        PLA
        JSR dbl
        TYA
        SEC
        SBC #4
        TAY
        RTS

; ---------------------------------------------------------------- a new game
; SPACE was pressed on this frame. NOISE depends on the cycle it's read on, and
; the player chose the frame: every game is a new one.
newgame:
        LDA NOISE
        STA RNDL
        LDA NOISE
        ORA #1              ; never 0, or the dice stick
        STA RNDH
        LDA #2
        STA STATE
        LDA #1
        STA NOCHK
        STA LAYON
        LDA #0
        LDX #5
@sc:    STA SCORE,X
        DEX
        BPL @sc
        STA STAGE
        STA WEAP
        STA CHG
        STA COOL
        STA LIFEK
        STA SHIPST
        STA BANNER
        STA FLASHB
        STA PKN
        LDA #3
        STA LIVES
        JSR clearall
        JSR layer0          ; stars and the aurora, no words
        ; the HUD: entries 24-31 along the top, and their images drawn now
        LDX #0
@clr:   LDA #0
        STA DYN,X
        STA DYN+$100,X
        INX
        BNE @clr
        LDX #7
@hud:   TXA
        ASL A
        ASL A
        TAY
        LDA #0
        STA TABLE+24*4,Y
        LDA hudx,X
        STA TABLE+24*4+1,Y
        TXA
        CLC
        ADC #I_DYN
        STA TABLE+24*4+2,Y
        LDA hudatt,X
        STA TABLE+24*4+3,Y
        DEX
        BPL @hud
        LDX #15
        LDA #$FF            ; nothing drawn yet
@hs:    STA HUDS,X
        DEX
        BPL @hs
        JSR hudtext
        LDX #7
@hd:    TXA
        TAY
        JSR hudone
        DEX
        BPL @hd
        LDA #' '            ; rows 9-16 have no terrain: GAME OVER goes
        LDX #0
@txt:   STA SCREEN+9*40,X   ; 320 cells: 256, then 64
        CPX #64
        BCS @t1
        STA SCREEN+9*40+256,X
@t1:    INX
        BNE @txt
        JSR spawnship
        LDA #16             ; SPACE is down: let go, don't charge from it
        STA OLDPAD
        JMP stagestart

; A stage: its colour, its script, its banner. DIFF follows the stage, to 7.
stagestart:
        LDA STAGE
        CMP #8
        BCC @d
        LDA #7
@d:     STA DIFF
        LDA STAGE           ; the stage's place in the round of three
@mod:   CMP #3
        BCC @m
        SBC #3
        BCS @mod
@m:     TAX
        LDA stcol,X
        STA COLOUR3
        TXA
        ASL A
        TAX
        LDA stwave,X
        STA WAVEP
        LDA stwave+1,X
        STA WAVEP+1
        LDA stname,X
        STA SRC
        LDA stname+1,X
        STA SRC+1
        LDA #90
        STA WAVET
        LDA #0
        STA WLEFT
        STA BOSS
        STA CLEART
        ; the banner: STAGE n, two spaces, the name
        LDX #0
@st:    LDA t_stage,X
        BEQ @num
        STA BANBUF,X
        INX
        BNE @st
@num:   LDA STAGE
        CLC
        ADC #1
        LDY #'0'-1
@tens:  INY
        SEC
        SBC #10
        BCS @tens
        ADC #10+'0'
        STA T0
        CPY #'0'
        BEQ @one
        TYA
        STA BANBUF,X
        INX
@one:   LDA T0
        STA BANBUF,X
        INX
        LDA #' '
        STA BANBUF,X
        INX
        STA BANBUF,X
        INX
        LDY #0
@nm:    LDA (SRC),Y
        STA BANBUF,X
        BEQ @go
        INX
        INY
        BNE @nm
@go:    LDA #<BANBUF
        LDY #>BANBUF
        JSR banner
        LDA #FX_STAGE
        JMP sfx

; banner: A/Y = a message (0 at the end) to scroll along row BANROW with the
; terrain, a character every coarse step, as the sign at the side of the road.
banner: STA BANP
        STY BANP+1
        LDY #0
@len:   LDA (BANP),Y
        BEQ @n
        INY
        BNE @len
@n:     TYA
        CLC
        ADC #40
        STA BANNER
        RTS

; ---------------------------------------------------------------- playing
st_play:
        JSR shipcol         ; first, with the picture the board has just checked
        JSR input
        JSR moveship
        JSR fire
        JSR scroll
        JSR shots
        JSR enemies
        JSR bullets
        JSR turrets
        JSR waves
        JSR drone
        JSR life
        JSR table
        JSR hud
        JMP colours

; The controls, from the keyboard matrix: everything held, at once. A press too
; quick for the matrix to catch between two frames still counts, from KBD.
input:  LDA KEYS+8          ; row 8: LEFT RIGHT UP DOWN in bits 0-3
        AND #$0F
        STA T0
        LDA KEYS+4          ; row 4: @ A B C D E F G
        LSR A               ; A: bit 1 to bit 0, left
        AND #1
        ORA T0
        STA T0
        LDA KEYS+4
        AND #%00010000      ; D: right
        BEQ @d
        LDA #2
        ORA T0
        STA T0
@d:     LDA KEYS+6          ; row 6: P Q R S T U V W
        BPL @w              ; bit 7: W, up
        LDA #4
        ORA T0
        STA T0
        LDA KEYS+6
@w:     AND #%00001000      ; S: down
        BEQ @s
        LDA #8
        ORA T0
        STA T0
@s:     LDA KEYS            ; row 0, bit 0: SPACE
        AND #1
        BEQ @tap
        LDA #16
        ORA T0
        STA T0
@tap:   LDA KEYIN
        BEQ @done
        LDX #9
@k:     CMP tapkey,X
        BEQ @hit
        DEX
        BPL @k
        BMI @done
@hit:   LDA tapbit,X
        ORA T0
        STA T0
@done:  LDA T0
        STA PAD
        RTS

moveship:
        LDA SHIPST
        BNE @r
        LDA PAD
        LSR A
        BCC @nl
        LDX SHX
        CPX #SXMIN+1
        BCC @nl
        DEC SHX
@nl:    LDA PAD
        AND #2
        BEQ @nr
        LDX SHX
        CPX #SXMAX
        BCS @nr
        INC SHX
@nr:    LDA PAD
        AND #4
        BEQ @nu
        LDA SHY
        CMP #SYMIN+2
        BCC @nu
        DEC SHY
        DEC SHY
@nu:    LDA PAD
        AND #8
        BEQ @r
        LDA SHY
        CMP #SYMAX
        BCS @r
        INC SHY
        INC SHY
@r:     RTS

; SPACE: a press fires a volley; held, it charges; let go, the LANCE.
fire:   LDA COOL
        BEQ @c
        DEC COOL
@c:     LDA SHIPST
        BNE @none
        LDA PAD
        AND #16
        BEQ @up
        LDA OLDPAD
        AND #16
        BNE @held
        JSR volley
        LDA #0
        STA CHG
        BEQ @done
@held:  LDA CHG
        CMP #CHGMAX
        BCS @done
        INC CHG
        BNE @done
@up:    LDA OLDPAD
        AND #16
        BEQ @none
        LDA CHG
        CMP #CHGMIN
        BCC @none
        JSR lance
@none:  LDA #0
        STA CHG
@done:  LDA PAD
        STA OLDPAD
        RTS

; One shot, three with the FAN, and one from the FIREFLY.
volley: LDA COOL
        BNE @r
        LDA #COOLT
        STA COOL
        LDX #1
        LDA WEAP
        AND #W_TWIN
        BEQ @t
        INX
@t:     STX T3
        LDA SHX
        CLC
        ADC #5
        STA T1
        LDA SHY
        STA T2
        LDA #0
        JSR shoot
        LDA WEAP
        AND #W_FAN
        BEQ @fly
        LDA SHY
        SEC
        SBC #3
        STA T2
        LDA #$FE            ; 2 pixels up a frame
        JSR shoot
        LDA SHY
        CLC
        ADC #3
        STA T2
        LDA #2
        JSR shoot
@fly:   LDA DRON
        BEQ @snd
        LDA #1
        STA T3
        LDA DRX
        CLC
        ADC #4
        STA T1
        LDA DRY
        STA T2
        LDA #0
        JSR shoot
@snd:   LDA #FX_SHOT
        JMP sfx
@r:     RTS

; shoot: a shot of type T3 at (T1, T2), A pixels down a frame, if a slot's free.
shoot:  LDX #NSH-1
@f:     LDY SOT,X
        BEQ @got
        DEX
        BPL @f
        RTS
@got:   STA SOVY,X
        LDA T3
        STA SOT,X
        LDA T1
        STA SOX,X
        LDA T2
        STA SOY,X
        LDA LSER
        STA SOLS,X
        RTS

lance:  INC LSER
        LDX #3
        LDA SHX
        CLC
        ADC #5
        STA T1
        LDA SHY
        STA T2
        LDA CHG
        CMP #CHGMAX
        BCC @s
        LDX #4              ; full: the wide one, centred on the ship
        LDA SHX
        CLC
        ADC #3
        STA T1
        LDA SHY
        SEC
        SBC #8
        CMP #SYMIN
        BCS @y
        LDA #SYMIN
@y:     STA T2
        LDA #6
        STA FLASH2
@s:     STX T3
        LDA #0
        JSR shoot
        LDA #FX_LANCE
        JMP sfx

; The FIREFLY keeps above and behind the ship (below, near the top), a unit
; and two pixels a frame at most, so it trails as the ship moves.
drone:  LDA DRON
        BEQ @r
        LDA SHX
        SEC
        SBC #7
        BCS @x
        LDA #0
@x:     CMP DRX
        BEQ @y
        BCC @l
        INC DRX
        BNE @y
@l:     DEC DRX
@y:     LDA SHY
        CMP #40
        BCS @above
        ADC #14
        BNE @ty
@above: SBC #14
@ty:    SEC
        SBC DRY
        BEQ @r
        BCC @up
        CMP #2
        BCC @d1
        INC DRY
@d1:    INC DRY
@r:     RTS
@up:    CMP #$FF
        BEQ @u1
        DEC DRY
@u1:    DEC DRY
        RTS

; The ship blown up: the next one, or the end.
life:   LDA SHIPST
        BEQ @alive
        DEC DEADT
        BNE @r
        LDA LIVES
        BNE spawnship
        JMP gameover
@alive: LDA INVUL
        BEQ @r
        DEC INVUL
@r:     RTS

; A ship at the left, in the middle of the gap there, blinking and safe.
spawnship:
        LDA #0
        STA SHIPST
        STA CHG
        LDA #12
        STA SHX
        LDA RP              ; the gap in screen column 8
        CLC
        ADC #8
        AND #63
        TAX
        LDA RINGC,X
        CLC
        ADC #27
        SEC
        SBC RINGF,X
        ASL A
        ASL A
        SEC
        SBC #8
        STA SHY
        LDA #INVULT
        STA INVUL
        RTS

die:    LDA #1
        STA SHIPST
        LDA #DEATHT
        STA DEADT
        DEC LIVES
        LDA #0
        STA WEAP
        STA DRON
        STA CHG
        LDA #12
        STA FLASHB
        LDA #FX_DIE
        JMP sfx

; What the ship touched, as the board saw it at the end of the last frame.
; SPRHIT bit 0: its pixels met another sprite's, so look at what's near it.
; SPRHITBG bit 0: they met the picture's ink: the terrain, or only a star or
; the aurora in the layer behind it, so look at the screen's codes.
shipcol:
        LDA SHIPST
        BNE @r
        LDA HIT
        LSR A
        BCC @terr
        JSR touch
        BCC @terr
        LDA INVUL
        BNE @terr
        JMP die
@terr:  LDA INVUL
        BNE @r
        LDA HITBG
        LSR A
        BCC @r
        JSR terrhit
        BCC @r
        JMP die
@r:     RTS

; touch: the ship's box against the enemies' and the bullets'. Capsules are
; picked up. C=1: something that hurts.
touch:  LDA SHX
        CLC
        ADC #2
        STA T0
        ADC #3
        STA T1
        LDA SHY
        CLC
        ADC #5
        STA T2
        ADC #5
        STA T3
        LDA #0
        STA T4
        LDX #NEN-1
@e:     LDY ENT,X
        BEQ @n
        LDA eflag,Y
        AND #F_HURT+F_PICK
        BEQ @n
        JSR overlap
        BCC @n
        LDA eflag,Y
        AND #F_PICK
        BEQ @hurt
        JSR collect
        JMP @n
@hurt:  INC T4
@n:     DEX
        BPL @e
        LDX #NBU-1
@b:     LDA BUT,X
        BEQ @bn
        LDA BUX,X
        CLC
        ADC #3
        CMP T1
        BEQ @b1
        BCS @bn
@b1:    LDA BUX,X
        CLC
        ADC #4
        CMP T0
        BCC @bn
        LDA BUY,X
        CLC
        ADC #6
        CMP T3
        BEQ @b2
        BCS @bn
@b2:    LDA BUY,X
        CLC
        ADC #9
        CMP T2
        BCC @bn
        INC T4
        LDA #0
        STA BUT,X
@bn:    DEX
        BPL @b
        LDA T4
        CMP #1
        RTS

; overlap: enemy X (of type Y) against the box T0-T1 (x), T2-T3 (y).
; C=1 if they overlap. Keeps X and Y.
overlap:
        LDA ENX,X
        CMP #160
        BCS @no
        CLC
        ADC ebx0,Y
        CMP T1
        BEQ @x1
        BCS @no
@x1:    LDA ENX,X
        CLC
        ADC ebx1,Y
        CMP T0
        BCC @no
        LDA ENY,X
        CLC
        ADC eby0,Y
        CMP T3
        BEQ @y1
        BCS @no
@y1:    LDA ENY,X
        CLC
        ADC eby1,Y
        CMP T2
        RTS                 ; C=1 unless it ends above the box
@no:    CLC
        RTS

; A capsule (X) picked up.
collect:
        LDA ENB,X
        BEQ @fan
        CMP #1
        BEQ @twin
        LDA DRON
        BNE @pts
        LDA #1
        STA DRON
        LDA SHX
        STA DRX
        LDA SHY
        STA DRY
        BNE @pts
@fan:   LDA #W_FAN
        BNE @w
@twin:  LDA #W_TWIN
@w:     ORA WEAP
        STA WEAP
@pts:   LDA #0
        STA ENT,X
        LDA #50             ; 500 points
        LDY #4
        JSR addscore
        LDA #FX_PICK
        JMP sfx

; terrhit: are any of the ship's outline points on a rock glyph's ink?
; C=1 if so.
terrhit:
        LDX #7
@p:     LDA SHX
        ASL A
        STA T4
        LDA #0
        ROL A
        STA T5
        LDA T4
        CLC
        ADC shpx,X
        STA T4
        BCC @c
        INC T5
@c:     LDA SHY
        CLC
        ADC shpy,X
        STA T7
        JSR cellpx
        CMP #G_ROCK         ; terrain is $60 and up
        BCC @next
        JSR glyphp
        LDA T4
        AND #7
        STA T6
        LDA T7
        AND #7
        TAY
        LDA (GP1),Y
        LDY T6
        AND bitmask,Y
        BNE @hit
@next:  DEX
        BPL @p
        CLC
        RTS
@hit:   SEC
        RTS

; cellpx: the screen code under picture pixel (T5:T4, T7), or $20 left of the
; first column (the strip the fine scroll uncovers). PTR -> the row, Y = the
; column; T4 = px - FINE (its low 3 bits: the pixel's column in the glyph).
; Keeps X.
cellpx: LDA T7
        LSR A
        LSR A
        LSR A
        TAY
        LDA ROWLO,Y
        STA PTR
        LDA ROWHI,Y
        STA PTR+1
        LDA T4
        SEC
        SBC FINE
        STA T4
        LDA T5
        SBC #0
        BCC @off
        LSR A
        LDA T4
        ROR A
        LSR A
        LSR A
        TAY
        CPY #40
        BCS @off
        LDA (PTR),Y
        RTS
@off:   LDA #$20
        RTS

; glyphp: GP1 -> glyph A in our set. Keeps X.
glyphp: STA GP1
        LDA #0
        STA GP1+1
        ASL GP1
        ROL GP1+1
        ASL GP1
        ROL GP1+1
        ASL GP1
        ROL GP1+1
        LDA GP1+1
        CLC
        ADC #>MYSET
        STA GP1+1
        RTS

; ---------------------------------------------------------------- shots
shots:  JSR boxes
        LDX #NSH-1
@loop:  LDY SOT,X
        BEQ @next
        LDA SOX,X
        CLC
        ADC sospd,Y
        STA SOX,X
        CMP #160
        BCS @kill
        LDA SOY,X
        CLC
        ADC SOVY,X
        STA SOY,X
        CMP #SYMIN
        BCC @kill
        CMP #192
        BCS @kill
        ; its tip against the terrain, unless it's in the rows no terrain reaches
        LDA SOY,X
        CLC
        ADC soty,Y
        STA T7
        CMP OPENT
        BCC @rock
        CMP OPENB
        BCC @air
@rock:  LDA SOX,X
        CLC
        ADC sotx,Y
        ASL A
        STA T4
        LDA #0
        ROL A
        STA T5
        JSR cellpx
        CMP #G_ROCK
        BCC @air
        JSR rockhit
        JMP @kill
@air:   JSR shothit
        BCC @next
@kill:  LDA #0
        STA SOT,X
@next:  DEX
        BPL @loop
        RTS

; A shot met the terrain at the cell (PTR),Y, code A. A FROSTEYE goes.
rockhit:
        CMP #G_EYEF
        BEQ @eye
        CMP #G_EYEC
        BEQ @eye
        RTS
@eye:   CLC
        ADC #G_PITF-G_EYEF  ; the eye becomes its pit
        STA (PTR),Y
        STY T6              ; the column
        LDA T7
        LSR A
        LSR A
        LSR A
        STA T5              ; the row
        TXA
        PHA
        LDX #3
@t:     LDA TACT,X
        BEQ @tn
        LDA TCOL,X
        CMP T6
        BNE @tn
        LDA TROW,X
        CMP T5
        BNE @tn
        LDA #0
        STA TACT,X
@tn:    DEX
        BPL @t
        ; the explosion, moving with the terrain: x = (col * 8 + FINE - 4) / 2
        LDA T6
        ASL A
        ASL A
        STA T4
        LDA FINE
        SEC
        SBC #4
        CMP #$80
        ROR A
        CLC
        ADC T4
        STA SX0
        LDA T5
        ASL A
        ASL A
        ASL A
        SEC
        SBC #4
        CMP #SYMIN
        BCS @y
        LDA #SYMIN
@y:     STA SY0
        LDA #$F8            ; half a unit left a frame: a pixel, like the rock
        STA SV0
        JSR boom
        PLA
        TAX
        LDA #20             ; 200 points
        LDY #4
        JSR addscore
        LDA #FX_BOOM
        JMP sfx

; boxes: the boxes of everything a shot can hit, worked out once a frame
; (BX0-BY1, and BSL whose), so each shot only compares.
boxes:  LDA #0
        STA NBOX
        LDX #NEN-1
@l:     LDY ENT,X
        BEQ @n
        LDA eflag,Y
        AND #F_SHOOT+F_BLOCK
        BEQ @n
        LDA ENX,X
        CMP #160
        BCS @n
        STX T6
        STY T7
        LDY NBOX
        TXA
        STA BSL,Y
        LDX T7              ; X: the type, for its box
        LDA ebx0,X
        LDX T6
        CLC
        ADC ENX,X
        STA BX0,Y
        LDX T7
        LDA ebx1,X
        LDX T6
        CLC
        ADC ENX,X
        STA BX1,Y
        LDX T7
        LDA eby0,X
        LDX T6
        CLC
        ADC ENY,X
        STA BY0,Y
        LDX T7
        LDA eby1,X
        LDX T6
        CLC
        ADC ENY,X
        STA BY1,Y
        INC NBOX
@n:     DEX
        BPL @l
        RTS

; shothit: shot X against the boxes. C=1 if the shot is spent.
shothit:
        LDY NBOX
        BNE @go
        CLC
        RTS
@go:    STX T6
        LDY SOT,X
        LDA SOX,X
        CLC
        ADC sobx0,Y
        STA T0
        LDA SOX,X
        CLC
        ADC sobx1,Y
        STA T1
        LDA SOY,X
        CLC
        ADC soby0,Y
        STA T2
        LDA SOY,X
        CLC
        ADC soby1,Y
        STA T3
        LDA sodmg,Y
        STA T4
        LDY NBOX
@e:     DEY
        BMI @miss
        LDA BX0,Y           ; starts right of the shot?
        CMP T1
        BEQ @x1
        BCS @e
@x1:    LDA BX1,Y           ; ends left of it?
        CMP T0
        BCC @e
        LDA BY0,Y
        CMP T3
        BEQ @y1
        BCS @e
@y1:    LDA BY1,Y
        CMP T2
        BCC @e
        LDX BSL,Y           ; touching: what is it now?
        STY T5
        LDY ENT,X
        LDA eflag,Y
        LDY T5
        AND #F_SHOOT+F_BLOCK
        BEQ @e              ; shot down already this frame
        AND #F_SHOOT
        BEQ @spent          ; a SHARD: the shot stops
        TYA
        PHA
        LDY T6
        LDA SOT,Y
        CMP #3
        BCC @hit            ; a shot or a TWIN: spent on the first thing
        LDA SOLS,Y          ; a LANCE hits each thing once, and goes on
        CMP ENLH,X
        BEQ @next
        STA ENLH,X
        JSR damage
@next:  PLA
        TAY
        JMP @e
@hit:   JSR damage
        PLA
@spent: LDX T6
        SEC
        RTS
@miss:  LDX T6
        CLC
        RTS

; damage: enemy X loses T4 hits. Keeps X, T0-T4 and T6.
damage: LDA ENHP,X
        SEC
        SBC T4
        STA ENHP,X
        BEQ @dead
        BCC @dead
        LDA ENT,X
        CMP #E_WARDEN
        BNE @r
        LDA #4
        STA BFLASH
        LDA #FX_TICK
        JMP sfx
@dead:  JMP kill
@r:     RTS

; kill: enemy X is shot down. It scores, counts for its wave, and turns into
; an explosion. Keeps X and T0-T6.
kill:   LDY ENT,X
        LDA escore,Y
        LDY #4
        JSR addscore
        LDA ENWS,X          ; the whole wave down? a capsule
        BEQ @nowave
        AND #3
        TAY
        LDA WKILL,Y
        CLC
        ADC #1
        STA WKILL,Y
        CMP WSIZE,Y
        BNE @nowave
        JSR droppick
@nowave:
        LDA #0
        STA ENATT,X
        LDA ENT,X
        CMP #E_WARDEN
        BNE @x
        JSR bossdead
        LDA #A_WIDE         ; the WARDEN's explosion is wide too
        STA ENATT,X
@x:     LDA #E_EXPL
        STA ENT,X
        LDA #0
        STA ENC,X
        STA ENVY,X
        LDA #$F8
        STA ENVX,X
        LDA #I_EXP0
        STA ENIMG,X
        LDA #FX_POP
        JMP sfx

; A capsule where enemy X was.
droppick:
        LDA ENX,X
        STA SX0
        LDA ENY,X
        STA SY0
        TXA
        PHA
        JSR freeslot
        BCS @r
        LDA #E_PICK
        JSR newthing
        LDA PKN
        STA ENB,X
        CLC
        ADC #I_PICKF
        STA ENIMG,X
        LDA #$F8
        STA ENVX,X
        LDY PKN
        INY
        CPY #3
        BCC @k
        LDY #0
@k:     STY PKN
@r:     PLA
        TAX
        RTS

; The WARDEN (X) is down: 5,000 points, explosions round it, the next stage.
bossdead:
        LDA #0
        STA BOSS
        LDA #50
        LDY #3              ; the hundreds: 50 x 100
        JSR addscore
        LDA ENY,X
        STA SY0
        LDA #0
        STA SV0
        LDA ENX,X
        STA SX0
        JSR boom
        LDA ENY,X
        CLC
        ADC #16
        STA SY0
        JSR boom
        LDA ENX,X
        CLC
        ADC #8
        STA SX0
        JSR boom
        LDA ENY,X
        STA SY0
        JSR boom
        LDA #150
        STA CLEART
        LDA #10
        STA FLASH2
        LDA #<t_clear
        LDY #>t_clear
        JSR banner
        LDA #FX_BOOM
        JMP sfx

; boom: an explosion at (SX0, SY0), moving SV0 a frame, if a slot's free.
; Keeps X.
boom:   TXA
        PHA
        JSR freeslot
        BCS @r
        LDA #E_EXPL
        JSR newthing
        LDA SV0
        STA ENVX,X
        LDA #I_EXP0
        STA ENIMG,X
@r:     PLA
        TAX
        RTS

; newthing: slot X becomes a thing of type A at (SX0, SY0), standing still.
newthing:
        STA ENT,X
        LDA SX0
        STA ENX,X
        LDA SY0
        STA ENY,X
        LDA #0
        STA ENC,X
        STA ENXF,X
        STA ENYF,X
        STA ENVX,X
        STA ENVY,X
        STA ENATT,X
        STA ENS,X
        STA ENWS,X
        STA ENLH,X
        LDA #1
        STA ENHP,X
        RTS

; freeslot: X = a free enemy slot, C=0; or C=1 if none.
freeslot:
        LDX #NEN-1
@l:     LDA ENT,X
        BEQ @got
        DEX
        BPL @l
        SEC
        RTS
@got:   CLC
        RTS

; ---------------------------------------------------------------- the score
; addscore: adds A to the digit Y (4: tens, 3: hundreds), carrying up.
; Keeps X. Then the high score, and a ship every 20,000.
addscore:
        STX T7
@d:     CLC
        ADC SCORE,Y
        LDX #0
@m:     CMP #10
        BCC @put
        SBC #10
        INX
        BNE @m
@put:   STA SCORE,Y
        TXA
        BEQ @hi
        DEY
        BPL @d
        LDX #5              ; past 999,999: it stays there
        LDA #9
@max:   STA SCORE,X
        DEX
        BPL @max
@hi:    LDX #0
@cmp:   LDA SCORE,X
        CMP HISC,X
        BCC @life
        BNE @copy
        INX
        CPX #6
        BNE @cmp
        BEQ @life
@copy:  LDX #5
@c:     LDA SCORE,X
        STA HISC,X
        DEX
        BPL @c
@life:  LDA SCORE           ; score / 20,000: hundred-thousands * 5 + ten-thousands / 2
        ASL A
        ASL A
        ADC SCORE
        STA T5
        LDA SCORE+1
        LSR A
        CLC
        ADC T5
        CMP LIFEK
        BEQ @r
        BCC @r
        STA LIFEK
        LDA LIVES
        CMP #9
        BCS @r
        INC LIVES
        LDA #FX_PICK
        JSR sfx
@r:     LDX T7
        RTS

; ---------------------------------------------------------------- enemies
enemies:
        LDX #NEN-1
@loop:  LDA ENT,X
        BEQ @next
        ASL A
        TAY
        LDA etab-2,Y
        STA JV
        LDA etab-1,Y
        STA JV+1
        JSR jumpv
@next:  DEX
        BPL @loop
        RTS

etab:   .word e_wisp, e_skua, e_growl, e_warden, e_shard, e_expl, e_pick

; emove: x += ENVX / 16 units, y += ENVY / 16 pixels. Keeps X.
emove:  LDA ENVX,X          ; the step's low byte: v * 16
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC ENXF,X
        STA ENXF,X
        LDY ENVX,X          ; its high byte: v / 16, keeping the sign
        LDA ENX,X
        ADC DHI,Y
        STA ENX,X
        LDA ENVY,X
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC ENYF,X
        STA ENYF,X
        LDY ENVY,X
        LDA ENY,X
        ADC DHI,Y
        STA ENY,X
        RTS

; gone: C=1 (and the slot freed) if X has flown off the left.
gone:   LDA ENX,X
        CMP #200
        BCC @no
        CMP #249
        BCS @no
        LDA #0
        STA ENT,X
        SEC
        RTS
@no:    CLC
        RTS

; keepy: y no higher than line 16, out of the HUD's band.
; keepy: y between lines 16 and 184: out of the HUD's band, and not off the
; bottom (where it would come round to the top). C=1 if it had to be moved.
keepy:  LDA ENY,X
        CMP #SYMIN
        BCC @top
        CMP #SYMAX+1
        BCC @r
        CMP #230            ; 230-255: above the top, come round
        BCS @top
        LDA #SYMAX
        STA ENY,X
        RTS
@top:   LDA #SYMIN
        STA ENY,X
        SEC
@r:     RTS

; A WISP glides left on a sine wave round its middle line.
e_wisp: JSR emove
        JSR gone
        BCS @r
        LDA ENC,X
        CLC
        ADC #3
        STA ENC,X
        TAY
        LDA SIN,Y
        CLC
        ADC ENB,X
        STA ENY,X
        JSR keepy
        LDA TICK
        AND #8
        LSR A
        LSR A
        LSR A
        CLC
        ADC #I_WISP0
        STA ENIMG,X
        LDY DIFF
        LDA wispfire,Y
        JMP chance
@r:     RTS

; A SKUA climbs out of the ground (or drops out of the roof) behind the
; terrain, flies level, and dives at you when it gets close.
e_skua: JSR emove
        JSR gone
        BCS @r
        LDA ENS,X
        BEQ @inside         ; climbing out: it may start under the ground
        JSR keepy
        BCC @inside
        LDA #0              ; dived off the top or the bottom: gone
        STA ENT,X
        RTS
@inside:
        LDA ENS,X
        BEQ @emerge
        CMP #1
        BEQ @glide
@r:     RTS
@emerge:
        DEC ENC,X
        BNE @r
        LDA #1
        STA ENS,X
        LDA #0
        STA ENVY,X
        STA ENATT,X         ; out in front now
        RTS
@glide: LDA ENX,X
        CMP #160
        BCS @r
        SEC
        SBC SHX
        BCC @dive           ; past the ship already
        CMP #44
        BCS @r
@dive:  LDA #2
        STA ENS,X
        LDA #I_SKUAD
        STA ENIMG,X
        LDA #$E8            ; 1.5 units left a frame
        STA ENVX,X
        LDA SHY
        CMP ENY,X
        BCS @down
        LDA #$E8            ; up: the diving picture, upside down
        STA ENVY,X
        LDA #A_FLIPY
        STA ENATT,X
        BNE @shot
@down:  LDA #24
        STA ENVY,X
@shot:  LDA DIFF
        BEQ @r
        JMP efire

; A GROWLER drifts and tumbles: the same picture, flipped four ways in turn.
e_growl:
        JSR emove
        JSR gone
        BCS @r
        JSR keepy           ; at the top or the bottom, it bounces
        BCC @t
        LDA #0
        SEC
        SBC ENVY,X
        STA ENVY,X
@t:     INC ENC,X
        LDA ENC,X
        LSR A
        LSR A
        LSR A
        AND #3
        TAY
        LDA tumble,Y
        STA ENATT,X
@r:     RTS

; The WARDEN comes in from the right, then sways and fires fans of three.
e_warden:
        LDA ENS,X
        BNE @fight
        JSR emove
        LDA ENX,X
        CMP #160
        BCS @r
        CMP #110
        BCS @r
        LDA #1
        STA ENS,X
        LDA #0
        STA ENVX,X
@r:     RTS
@fight: INC ENC,X
        LDY ENC,X
        LDA SIN,Y           ; y = 64 + 1.5 sin: 16 to 112
        STA T0
        CMP #$80
        ROR A
        CLC
        ADC T0
        CLC
        ADC #64
        STA ENY,X
        TYA                 ; x = 102 + sin(phase / 2) / 4
        LSR A
        CLC
        ADC #64
        TAY
        LDA SIN,Y
        CMP #$80
        ROR A
        CMP #$80
        ROR A
        CLC
        ADC #102
        STA ENX,X
        LDA #I_WARD0
        LDY BFLASH
        BEQ @img
        DEC BFLASH
        LDA #I_WARD1
@img:   STA ENIMG,X
        LDA ENC,X           ; fire every 64 frames, every 32 from stage 4
        LDY DIFF
        AND bossmask,Y
        CMP #16
        BNE @r
        LDA ENX,X
        CLC
        ADC #4
        STA T6
        LDA ENY,X
        CLC
        ADC #8
        STA T7
        LDA #0
        STA T3
        JSR fireat
        LDA #1
        STA T3
        JSR fireat
        LDA #$FF
        STA T3
        JMP fireat

; A SHARD circles its WARDEN, 32 pixels out, and goes when it goes.
e_shard:
        LDY ENS,X
        LDA ENT,Y
        CMP #E_WARDEN
        BEQ @ok
        LDA ENX,X           ; the WARDEN is gone: so is the SHARD
        STA SX0
        LDA ENY,X
        STA SY0
        LDA #0
        STA SV0
        STA ENT,X
        JMP boom
@ok:    LDA ENC,Y           ; the angle: the WARDEN's phase x 2, plus our own
        ASL A
        CLC
        ADC ENB,X
        STA T0
        TAY
        LDA SIN,Y
        STA T1              ; sin
        LDA T0
        CLC
        ADC #64
        TAY
        LDA SIN,Y           ; cos / 2: x is in 2-pixel units
        CMP #$80
        ROR A
        LDY ENS,X
        CLC
        ADC ENX,Y
        CLC
        ADC #4
        STA ENX,X
        LDA ENY,Y
        CLC
        ADC #8
        CLC
        ADC T1
        STA ENY,X
        JMP keepy

e_expl: JSR emove
        INC ENC,X
        LDA ENC,X
        CMP #16
        BCS @end
        LSR A
        LSR A
        CLC
        ADC #I_EXP0
        STA ENIMG,X
        RTS
@end:   LDA #0
        STA ENT,X
        RTS

e_pick: JSR emove
        JMP gone

; chance: fire at the ship from enemy X if a random byte is below A.
chance: STA T0
        JSR rnd
        CMP T0
        BCS efr
; efire: enemy X fires at the ship, from its middle.
efire:  LDA ENX,X
        CMP #150
        BCS efr
        STA T6
        LDA ENY,X
        STA T7
        LDA #0
        STA T3
        JMP fireat
efr:    RTS

; fireat: a bullet from (T6, T7) at the ship, aimed, then turned T3 sixteenths
; of a circle. Only in play, only at a ship. Keeps X.
fireat: LDA STATE
        CMP #2
        BNE @r
        LDA SHIPST
        BNE @r
        LDY #NBU-1
@f:     LDA BUT,Y
        BEQ @got
        DEY
        BPL @f
@r:     RTS
@got:   STY T5
        JSR aim
        CLC
        ADC T3
        AND #15
        STA T0
        LDY DIFF
        LDA bspd,Y          ; 16 directions a speed
        CLC
        ADC T0
        TAY
        LDA VDX,Y
        STA T0
        LDA VDY,Y
        LDY T5
        STA BUVY,Y
        LDA T0
        STA BUVX,Y
        LDA T6
        STA BUX,Y
        LDA T7
        STA BUY,Y
        LDA #$80
        STA BUXF,Y
        STA BUYF,Y
        LDA #1
        STA BUT,Y
        RTS

; aim: A = the direction (0-15, 0 right, 4 down) from (T6, T7) to the ship.
; The angle from the ratio of the distances, in five steps a quadrant.
; Keeps X.
aim:    LDA #0
        STA T0
        LDA SHX
        SEC
        SBC T6
        BCS @x
        EOR #$FF
        ADC #1
        INC T0              ; bit 0: the ship is to the left
@x:     STA T1
        LDA SHY
        SEC
        SBC T7
        BCS @y
        EOR #$FF
        ADC #1
        INC T0
        INC T0              ; bit 1: the ship is above
@y:     LSR A               ; y / 2: the same units as x
        STA T2
        LDA T1              ; b < a / 4: level
        LSR A
        LSR A
        CMP T2
        BCS @o0
        LDA T1              ; b < a * 5 / 8
        LSR A
        STA T4
        LSR A
        LSR A
        CLC
        ADC T4
        CMP T2
        BCS @o1
        LDA T1              ; b < a * 3 / 2
        LSR A
        CLC
        ADC T1
        BCS @o2
        CMP T2
        BCS @o2
        LDA T2              ; b / 4 < a
        LSR A
        LSR A
        CMP T1
        BCC @o3
        LDA #4
        BNE @q
@o0:    LDA #0
        BEQ @q
@o1:    LDA #1
        BNE @q
@o2:    LDA #2
        BNE @q
@o3:    LDA #3
@q:     STA T4
        LDA T0
        BEQ @rd             ; right and down: as it is
        CMP #1
        BEQ @ld
        CMP #2
        BEQ @ru
        LDA T4              ; left and up: 8 + o
        CLC
        ADC #8
        RTS
@ld:    LDA #8              ; left and down: 8 - o
        SEC
        SBC T4
        RTS
@ru:    LDA #16             ; right and up: 16 - o
        SEC
        SBC T4
        AND #15
        RTS
@rd:    LDA T4
        RTS

; ---------------------------------------------------------------- bullets
bullets:
        LDX #NBU-1
@l:     LDA BUT,X
        BEQ @n
        LDA BUVX,X
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC BUXF,X
        STA BUXF,X
        LDY BUVX,X
        LDA BUX,X
        ADC DHI,Y
        STA BUX,X
        CMP #160
        BCC @y
        CMP #249
        BCC @kill
@y:     LDA BUVY,X
        ASL A
        ASL A
        ASL A
        ASL A
        CLC
        ADC BUYF,X
        STA BUYF,X
        LDY BUVY,X
        LDA BUY,X
        ADC DHI,Y
        STA BUY,X
        CMP #SYMIN
        BCC @kill
        CMP #194
        BCS @kill
        LDA DRON            ; the FIREFLY soaks bullets up
        BEQ @n
        LDA BUX,X
        SEC
        SBC DRX
        CLC
        ADC #4
        CMP #9
        BCS @n
        LDA BUY,X
        SEC
        SBC DRY
        CLC
        ADC #8
        CMP #17
        BCS @n
@kill:  LDA #0
        STA BUT,X
@n:     DEX
        BPL @l
        RTS

; One FROSTEYE a frame, in turn, may fire, if it's well on the screen.
turrets:
        LDA SHIPST
        BNE @r
        LDA TICK
        AND #3
        TAX
        LDA TACT,X
        BEQ @r
        LDA TCOL,X
        CMP #6
        BCC @r
        CMP #38
        BCS @r
        JSR rnd
        LDY DIFF
        CMP turfire,Y
        BCS @r
        ; from the eye's cell: x = (col * 8 + FINE - 4) / 2, y = row * 8 - 4
        LDA TCOL,X
        ASL A
        ASL A
        STA T6
        LDA FINE
        SEC
        SBC #4
        CMP #$80
        ROR A
        CLC
        ADC T6
        STA T6
        LDA TROW,X
        ASL A
        ASL A
        ASL A
        SEC
        SBC #4
        STA T7
        LDA #0
        STA T3
        JMP fireat
@r:     RTS

; ---------------------------------------------------------------- waves
; The stage's script: records of four bytes, read one at a time. A wave comes
; in one at a time, then the script waits.
waves:  LDA CLEART
        BEQ @w
        DEC CLEART
        BNE @r
        INC STAGE
        JMP stagestart
@w:     LDA BOSS
        BNE @r              ; nothing else while the WARDEN is out
        DEC WAVET
        BNE @r
        LDA WLEFT
        BEQ @rec
        JSR spawn
        DEC WLEFT
        BEQ @last
        LDX WTYPE
        LDA wspace,X
        STA WAVET
@r:     RTS
@last:  LDA WPOST
        STA WAVET
        RTS
@rec:   LDY #0
        LDA (WAVEP),Y
        STA T0
        INY
        LDA (WAVEP),Y
        STA T1
        INY
        LDA (WAVEP),Y
        STA T2
        INY
        LDA (WAVEP),Y
        STA WAVET
        LDA WAVEP
        CLC
        ADC #4
        STA WAVEP
        BCC @t
        INC WAVEP+1
@t:     LDA T0
        CMP #R_STYLE
        BEQ @style
        CMP #R_BANNER
        BEQ @ban
        CMP #R_BOSS
        BEQ @boss
        CMP #R_LOOP
        BEQ @loop
        STA WTYPE           ; a wave
        LDA T2
        STA WPARAM
        LDA WAVET
        STA WPOST
        LDA DIFF            ; more of them later on
        LSR A
        CLC
        ADC T1
        STA WLEFT
        LDA #1
        STA WAVET
        LDY WSER
        INY
        TYA
        AND #3
        STA WSER
        TAY
        LDA WLEFT
        STA WSIZE,Y
        LDA #0
        STA WKILL,Y
        LDA STATE           ; on the title, no capsules
        CMP #2
        BEQ @r2
        STA WSIZE,Y         ; (never reached)
@r2:    RTS
@style: LDA T1
        STA STYLE
        RTS
@ban:   LDA T1
        ASL A
        TAX
        LDA banners,X
        LDY banners+1,X
        JSR banner
        LDA #FX_WARN
        JMP sfx
@boss:  JSR spawnboss
        BCC @bok
        LDA WAVEP           ; no room: try again in a while
        SEC
        SBC #4
        STA WAVEP
        BCS @b2
        DEC WAVEP+1
@b2:    LDA #20
        STA WAVET
@bok:   RTS
@loop:  LDA #<sc_title
        STA WAVEP
        LDA #>sc_title
        STA WAVEP+1
        RTS

; spawn: one of the wave coming in, at the right edge.
spawn:  JSR freeslot
        BCC @ok
        RTS
@ok:    LDA #164
        STA SX0
        LDA #0
        STA SY0
        LDA WTYPE
        JSR newthing
        LDY WTYPE
        LDA ehp,Y
        STA ENHP,X
        LDA WSER
        LDY STATE
        CPY #2
        BEQ @ws
        LDA #0              ; the title's waves aren't anybody's
@ws:    STA ENWS,X
        JSR gaptop          ; T0: the gap's top line, T1: its bottom
        LDA WTYPE
        CMP #E_SKUA
        BEQ @skua
        CMP #E_GROWL
        BEQ @growl
        ; a WISP: its middle line from the gap's middle and the wave's offset
        LDA T0
        CLC
        ADC T1
        ROR A
        SEC
        SBC #8
        CLC
        ADC WPARAM
        CMP #24
        BCS @w1
        LDA #24
@w1:    CMP #168
        BCC @w2
        LDA #168
@w2:    STA ENB,X
        STA ENY,X
        LDA DIFF
        CLC
        ADC #10
        EOR #$FF
        ADC #1
        STA ENVX,X
        LDA #I_WISP0
        STA ENIMG,X
        RTS
@skua:  LDA #I_SKUA
        STA ENIMG,X
        LDA DIFF
        ASL A
        CLC
        ADC #14
        EOR #$FF
        ADC #1
        STA ENVX,X
        LDA #16             ; frames climbing out
        STA ENC,X
        LDA WPARAM          ; 0 from the roof, 1 from the ground, 2 both in turn
        CMP #2
        BCC @side
        LDA WLEFT
        AND #1
@side:  BNE @ground
        LDA T0              ; from the roof
        SEC
        SBC #12
        CMP #SYMIN
        BCS @rf
        LDA #SYMIN
@rf:    STA ENY,X
        LDA #16             ; a pixel down a frame
        BNE @mv
@ground:
        LDA T1
        SEC
        SBC #4
        STA ENY,X
        LDA #$F0            ; a pixel up a frame
@mv:    STA ENVY,X
        LDA #A_BEHIND
        STA ENATT,X
        RTS
@growl: LDA #I_GROWL
        STA ENIMG,X
        LDA DIFF
        LSR A
        CLC
        ADC ENHP,X
        STA ENHP,X
        LDA #$FA
        STA ENVX,X
        JSR rnd
        AND #7
        SEC
        SBC #4
        STA ENVY,X
        LDA T1              ; somewhere in the gap
        SEC
        SBC T0
        SEC
        SBC #16
        STA T2
        JSR rnd
@gy:    CMP T2
        BCC @gok
        SBC T2
        BCS @gy
@gok:   CLC
        ADC T0
        STA ENY,X
        RTS

; gaptop: T0 = the first open line under the roof at the right edge, T1 the
; first line of the ground there.
gaptop: LDA CB
        CLC
        ADC #ROOFTOP
        ASL A
        ASL A
        ASL A
        STA T0
        LDA #25
        SEC
        SBC FB
        ASL A
        ASL A
        ASL A
        STA T1
        RTS

; The WARDEN and its two SHARDS: three slots, or C=1.
spawnboss:
        LDY #0
        LDX #NEN-1
@c:     LDA ENT,X
        BNE @u
        INY
@u:     DEX
        BPL @c
        CPY #3
        BCS @ok
        RTS                 ; C=0 here: say no room
@ok:    JSR freeslot
        LDA #164
        STA SX0
        LDA #64
        STA SY0
        LDA #E_WARDEN
        JSR newthing
        STX T7
        INX
        STX BOSS
        LDX T7
        LDA #$F8
        STA ENVX,X
        LDA #A_WIDE
        STA ENATT,X
        LDA #I_WARD0
        STA ENIMG,X
        LDY DIFF
        LDA bosshp,Y
        STA ENHP,X
        LDA #0
        STA BFLASH
        LDA #0
        STA T6
@sh:    JSR freeslot
        LDA #E_SHARD
        JSR newthing
        LDA T7
        STA ENS,X
        LDA T6
        STA ENB,X
        LDA #I_SHARD
        STA ENIMG,X
        LDA T6
        EOR #128
        STA T6
        BNE @sh
        CLC
        RTS

; ---------------------------------------------------------------- scrolling
; The terrain: SCROLLX counts 7 down to 0, a pixel a frame; past 0 it's 7
; again and the codes move a cell. The layer: a quarter of the speed, from the
; same counter, offset so its coarse steps never fall on the terrain's.
scroll: LDA SCRON
        BEQ @layer
        DEC SCR
        LDA SCR
        AND #7
        STA FINE
        STA SCROLLX
        CMP #7
        BNE @layer
        JSR coarse
@layer: LDA LAYON
        BEQ @r
        LDA CNTC            ; deep in a cave (no column of open sky on the
        BEQ @wipe           ; screen): the aurora goes, a column a frame
        LDA #0
        STA WIPE
        BEQ @lsx
@wipe:  LDX WIPE
        CPX #40
        BCS @lsx
        INC WIPE
        LDA #$20
        .rept 7, k
        STA LAYERP+[2+k]*40,X
        .endr
@lsx:   LDA SCR
        CLC
        ADC #4
        LSR A
        LSR A
        AND #7
        STA LAYERSX
        LDA SCR
        CLC
        ADC #4
        AND #31
        CMP #31
        BNE @r
        JMP laycoarse
@r:     RTS

; The coarse step: every row that has terrain moves a cell left, the column
; leaving at the left is forgotten, and a new one is made at the right.
coarse: JSR shiftrows
        LDX RP              ; the column leaving: its heights leave the counts
        LDA RINGC,X
        TAY
        LDA CNTC,Y
        SEC
        SBC #1
        STA CNTC,Y
        LDA RINGF,X
        TAY
        LDA CNTF,Y
        SEC
        SBC #1
        STA CNTF,Y
        INX
        TXA
        AND #63
        STA RP
        LDA #39
        STA GC
        JSR newcol
        JSR maxht
        LDX #3              ; the FROSTEYES move with it
@t:     LDA TACT,X
        BEQ @tn
        DEC TCOL,X
        BPL @tn
        LDA #0
        STA TACT,X
@tn:    DEX
        BPL @t
        LDA BANNER
        BEQ @r
        DEC BANNER
        LDY #0
        LDA (BANP),Y
        BEQ @sp
        INC BANP
        BNE @put
        INC BANP+1
        BNE @put
@sp:    LDA #' '
@put:   STA SCREEN+BANROW*40+39
@r:     RTS

; shiftrows: the rows with terrain in them move a cell left, and the banner's
; row while it runs. Rows that are rock all the way across (inside the
; shallowest roof, under the lowest ground) look the same moved, so they stay.
; Each block is one loop over the columns with its rows unrolled, top to
; bottom, 10 cycles a cell: the JMP at its top is changed to enter at the
; first row that's needed, and the first row that isn't gets a JMP to the
; loop's end over its first instruction, put back afterwards. The roof's
; rows run from MINC (the shallowest roof, counting what hangs under it, so
; its last row of solid rock is MINC - 1) to 1 + MAXC; the ground's from
; 25 - MAXF to 26 - MINF.
shiftrows:
        LDX MAXC
        BEQ shfloor
        LDA MINC            ; the first row: MINC, at least row 2
        CMP #ROOFTOP
        BCS @c1
        LDA #ROOFTOP
@c1:    TAY
        LDA cjlo-ROOFTOP,Y
        STA cjump+1
        LDA cjhi-ROOFTOP,Y
        STA cjump+2
        CPX #7              ; the row after 1 + MAXC: 2 + MAXC, crow(MAXC)
        BCS @c2
        LDA #>ctail
        STA T6
        LDA cjlo,X
        LDY cjhi,X
        LDX #<ctail
        JSR patch
@c2:    LDX #0
cjump:  JMP crow0
        .rept 7, k          ; rows 2 down to 8
crow\k: LDA SCREEN+[2+k]*40+1,X
        STA SCREEN+[2+k]*40,X
        .endr
ctail:  INX
        CPX #39
        BNE cjump
        JSR unpatch
shfloor:
        LDX MAXF
        BEQ shban
        LDA fjlo-1,X        ; the first row: 25 - MAXF, frow(8 - MAXF)
        STA fjump+1
        LDA fjhi-1,X
        STA fjump+2
        LDX MINF            ; the row after 26 - MINF: 27 - MINF, frow(10 - MINF)
        CPX #3
        BCC @f2
        LDA #>ftail
        STA T6
        LDA fxlo-3,X
        LDY fxhi-3,X
        LDX #<ftail
        JSR patch
@f2:    LDX #0
fjump:  JMP frow0
        .rept 8, k          ; rows 17 down to 24
frow\k: LDA SCREEN+[17+k]*40+1,X
        STA SCREEN+[17+k]*40,X
        .endr
ftail:  INX
        CPX #39
        BNE fjump
        JSR unpatch
shban:  LDA BANNER
        BEQ shdone
        LDX #0
@b:     LDA SCREEN+BANROW*40+1,X
        STA SCREEN+BANROW*40,X
        INX
        CPX #39
        BNE @b
shdone: RTS

; patch: the instruction at A/Y (low, high) becomes JMP to X/T6 (low, high),
; its three bytes kept for unpatch, which puts them back (and does nothing if
; nothing is patched).
patch:  STA PTCH
        STY PTCH+1
        STX T7
        LDY #2
@s:     LDA (PTCH),Y
        STA PSAVE,Y
        DEY
        BPL @s
        LDY #0
        LDA #$06            ; JMP
        STA (PTCH),Y
        INY
        LDA T7
        STA (PTCH),Y
        INY
        LDA T6
        STA (PTCH),Y
        RTS
unpatch:
        LDA PTCH+1
        BEQ @r
        LDY #2
@p:     LDA PSAVE,Y
        STA (PTCH),Y
        DEY
        BPL @p
        LDA #0
        STA PTCH+1
@r:     RTS

cjlo:   .rept 7, k
        .byte <[crow0 + 6 * k]
        .endr
cjhi:   .rept 7, k
        .byte >[crow0 + 6 * k]
        .endr
fjlo:   .rept 8, k          ; MAXF 1-8: frow(8 - MAXF)
        .byte <[frow0 + 6 * [7 - k]]
        .endr
fjhi:   .rept 8, k
        .byte >[frow0 + 6 * [7 - k]]
        .endr
fxlo:   .rept 6, k          ; MINF 3-8: frow(10 - MINF)
        .byte <[frow0 + 6 * [7 - k]]
        .endr
fxhi:   .rept 6, k
        .byte >[frow0 + 6 * [7 - k]]
        .endr
        .assert crow1 - crow0 == 6 && frow1 - frow0 == 6, "a row's copy is 6 bytes"

; maxht: MAXC, MINC, MAXF and MINF from the counts of columns by height.
maxht:  LDX #7
@c:     LDA CNTC,X
        BNE @cf
        DEX
        BNE @c
@cf:    STX MAXC
        LDX #0
@cm:    LDA CNTC,X
        BNE @cmf
        INX
        BNE @cm
@cmf:   STX MINC
        LDX #8
@f:     LDA CNTF,X
        BNE @ff
        DEX
        BNE @f
@ff:    STX MAXF
        LDX #0
@fm:    LDA CNTF,X
        BNE @fmf
        INX
        BNE @fm
@fmf:   STX MINF
        LDA MAXC            ; the lines no terrain reaches: OPENT to OPENB - 1
        CLC
        ADC #ROOFTOP
        ASL A
        ASL A
        ASL A
        STA OPENT
        LDA #25
        SEC
        SBC MAXF
        ASL A
        ASL A
        ASL A
        STA OPENB
        RTS

; terrain0: a whole screen of terrain, column by column, for a new screen.
terrain0:
        LDA #0
        STA RP
        STA CA
        STA CB
        STA GC
        LDX #16
@z:     STA CNTC,X
        DEX
        BPL @z
        LDX #3
@t:     STA TACT,X
        DEX
        BPL @t
        LDA #2
        STA FA
        STA FB
@g:     JSR newcol
        INC GC
        LDA GC
        CMP #40
        BNE @g
        LDA #7
        STA SCR
        STA FINE
        STA SCROLLX
        JMP maxht

; newcol: makes column GC: the roof and the ground each go up a cell, down a
; cell or stay, within what the style allows, and not so close that the gap
; is too narrow. Where they change, a slope glyph, and column GC - 1's edge is
; mended to meet it. Column GC - 1 may get a spruce, an icicle or a FROSTEYE,
; if it's flat on both sides.
newcol: LDA STYLE
        ASL A
        ASL A
        ASL A
        TAX                 ; X: the style's record
        LDA CB
        STA T0
        CMP stcmin,X
        BCS @c1
        INC T0
        BNE @cd
@c1:    CMP stcmax,X
        BEQ @c2
        BCC @c2
        DEC T0
        JMP @cd
@c2:    JSR rnd
        CMP #36
        BCS @c3
        LDA T0
        CMP stcmin,X
        BEQ @cd
        DEC T0
        JMP @cd
@c3:    CMP #220
        BCC @cd
        LDA T0
        CMP stcmax,X
        BEQ @cd
        INC T0
@cd:    LDA FB              ; the ground
        STA T1
        CMP stfmin,X
        BCS @f1
        INC T1
        BNE @fd
@f1:    CMP stfmax,X
        BEQ @f2
        BCC @f2
        DEC T1
        JMP @fd
@f2:    JSR rnd
        CMP #50
        BCS @f3
        LDA T1
        CMP stfmin,X
        BEQ @fd
        DEC T1
        JMP @fd
@f3:    CMP #206
        BCC @fd
        LDA T1
        CMP stfmax,X
        BEQ @fd
        INC T1
@fd:    LDY #2              ; not too narrow: undo a rise, or come down
@sum:   LDA T0
        CLC
        ADC T1
        CMP stsum,X
        BEQ @ok
        BCC @ok
        LDA T1
        CMP FB
        BEQ @feq
        BCS @fdec
        BCC @cdec
@feq:   CMP #2
        BCC @cdec
@fdec:  DEC T1
        DEY
        BNE @sum
        BEQ @ok
@cdec:  LDA T0
        BEQ @ok
        CMP CB
        BCC @ok
        DEC T0
        DEY
        BNE @sum
@ok:    LDA #$20            ; the column: air,
        LDY #24
@air:   STA COLB,Y
        DEY
        CPY #ROOFTOP
        BCS @air
        LDA T0              ; the roof,
        BEQ @noroof
        CLC
        ADC #ROOFTOP-1
        TAY                 ; its edge's row
        LDA T0
        CMP CB
        BEQ @flatc
        BCC @flatc
        LDA #G_CDN
        BNE @ce
@flatc: LDA #G_CEIL
@ce:    STA COLB,Y
        LDA #G_ROCK
@rf:    DEY
        CPY #ROOFTOP
        BCC @noroof
        STA COLB,Y
        BCS @rf
@noroof:
        LDA #25             ; the ground
        SEC
        SBC T1
        TAY
        LDA T1
        CMP FB
        BEQ @flatf
        BCC @flatf
        LDA #G_FUP
        BNE @fe
@flatf: LDA #G_FLOOR
@fe:    STA COLB,Y
        LDA #G_ROCK
@ff:    INY
        CPY #25
        BCS @drawn
        STA COLB,Y
        BCC @ff
@drawn: LDA GC
        BNE @mend
        JMP @ring
@mend:  LDA T0              ; the roof went up: GC - 1's edge slopes up to meet it
        CMP CB
        BCS @m1
        LDA CB
        CLC
        ADC #ROOFTOP-1
        JSR cellprev
        LDA (PTR),Y
        CMP #G_CDN
        BEQ @ctip
        LDA #G_CUP
        BNE @cst
@ctip:  LDA #G_CTIP
@cst:   STA (PTR),Y
@m1:    LDA T1              ; the ground went down: the same
        CMP FB
        BCS @deco
        LDA #25
        SEC
        SBC FB
        JSR cellprev
        LDA (PTR),Y
        CMP #G_FUP
        BEQ @ftip
        LDA #G_FDN
        BNE @fst
@ftip:  LDA #G_FTIP
@fst:   STA (PTR),Y
@deco:  LDA CB              ; something under a flat roof?
        BEQ @dfl
        CMP CA
        BNE @dfl
        CMP T0
        BNE @dfl
        JSR rnd
        CMP stturr,X
        BCS @ice
        LDA CB
        CLC
        ADC #ROOFTOP
        JSR newturret
        BCS @dfl
        LDA #G_EYEC
        BNE @cput
@ice:   LDA stkind,X
        BEQ @dfl
        JSR rnd
        CMP stdec,X
        BCS @dfl
        LDA #G_ICE
@cput:  STA T2
        LDA CB
        CLC
        ADC #ROOFTOP
        JSR cellprev
        LDA T2
        STA (PTR),Y
        LDY GC
        DEY
        TYA
        CLC
        ADC RP
        AND #63
        TAY                 ; its ring slot: one row deeper
        LDA RINGC,Y
        STY T3
        TAY
        LDA CNTC,Y
        SEC
        SBC #1
        STA CNTC,Y
        INY
        LDA CNTC,Y
        CLC
        ADC #1
        STA CNTC,Y
        TYA
        LDY T3
        STA RINGC,Y
@dfl:   LDA FB              ; something on flat ground?
        CMP FA
        BNE @ring
        CMP T1
        BNE @ring
        JSR rnd
        CMP stturr,X
        BCS @tree
        LDA #24
        SEC
        SBC FB
        JSR newturret
        BCS @ring
        LDA #G_EYEF
        BNE @fput
@tree:  LDA stkind,X
        BNE @ring
        JSR rnd
        CMP stdec,X
        BCS @ring
        LDA #G_TREE
@fput:  STA T2
        LDA #24
        SEC
        SBC FB
        JSR cellprev
        LDA T2
        STA (PTR),Y
        LDY GC
        DEY
        TYA
        CLC
        ADC RP
        AND #63
        TAY
        LDA RINGF,Y
        STY T3
        TAY
        LDA CNTF,Y
        SEC
        SBC #1
        STA CNTF,Y
        INY
        LDA CNTF,Y
        CLC
        ADC #1
        STA CNTF,Y
        TYA
        LDY T3
        STA RINGF,Y
@ring:  LDA GC              ; the new column's heights into the ring and counts
        CLC
        ADC RP
        AND #63
        TAY
        LDA T0
        STA RINGC,Y
        LDA T1
        STA RINGF,Y
        LDY T0
        LDA CNTC,Y
        CLC
        ADC #1
        STA CNTC,Y
        LDY T1
        LDA CNTF,Y
        CLC
        ADC #1
        STA CNTF,Y
        LDA CB
        STA CA
        LDA T0
        STA CB
        LDA FB
        STA FA
        LDA T1
        STA FB
        LDX GC              ; and onto the screen, rows 2-24
        .rept 23, k
        LDA COLB+2+k
        STA SCREEN+[2+k]*40,X
        .endr
        RTS

; cellprev: PTR -> screen row A, Y = column GC - 1.
cellprev:
        TAY
        LDA ROWLO,Y
        STA PTR
        LDA ROWHI,Y
        STA PTR+1
        LDY GC
        DEY
        RTS

; newturret: a FROSTEYE at column GC - 1, row A, if one of the four is free
; and the game is on. C=1 if not.
newturret:
        LDY STATE
        CPY #2
        BNE @no
        LDY #3
@f:     PHA
        LDA TACT,Y
        BEQ @got
        PLA
        DEY
        BPL @f
@no:    SEC
        RTS
@got:   PLA
        STA TROW,Y
        LDA #1
        STA TACT,Y
        LDA GC
        SEC
        SBC #1
        STA TCOL,Y
        CLC
        RTS

; ---------------------------------------------------------------- the layer
; laycoarse: the layer moves a cell left: the aurora's rows all along, and
; each star by itself. A star that leaves at the left comes back at the right,
; on a new row.
laycoarse:
        LDX #0
@l:     .rept 7, k
        LDA LAYERP+[2+k]*40+1,X
        STA LAYERP+[2+k]*40,X
        .endr
        INX
        CPX #39
        BNE @l
        LDA #39
        STA GC
        JSR aurcol
        LDX #NSTAR-1
@s:     LDY STR,X
        LDA LROWLO,Y
        STA PTR
        LDA LROWHI,Y
        STA PTR+1
        LDY STC,X
        LDA #$20
        STA (PTR),Y
        DEY
        BPL @put
        JSR newstar
        LDY #39
@put:   TYA
        STA STC,X
        LDA STG,X
        STA (PTR),Y
        DEX
        BPL @s
        RTS

; newstar: star X gets a new row (9-23) and glyph; PTR -> its row.
newstar:
        JSR rnd
        AND #15
        CMP #15
        BCC @r
        LDA #7
@r:     CLC
        ADC #9
        STA STR,X
        TAY
        LDA LROWLO,Y
        STA PTR
        LDA LROWHI,Y
        STA PTR+1
        JSR rnd
        AND #3
        CMP #3
        BCC @g
        LDA #0
@g:     CLC
        ADC #G_STAR
        STA STG,X
        RTS

; aurcol: the aurora in layer column GC, rows 2-8: curtains of a few columns
; with gaps between, each a bright lower edge that wanders up and down, rays
; above it, and the rays fading out at the top. Under a roof, no sky.
aurcol: LDX GC
        LDA #$20
        .rept 7, k
        STA LAYERP+[2+k]*40,X
        .endr
        LDA CB
        BEQ @sky
        RTS
@sky:   LDA AUG
        BEQ @curt
        DEC AUG
        BNE @r
        JSR rnd
        AND #7
        CLC
        ADC #5
        STA AUL
@r:     RTS
@curt:  DEC AUL
        BNE @walk
        JSR rnd
        AND #3
        CLC
        ADC #2
        STA AUG
@walk:  JSR rnd
        CMP #70
        BCS @u
        LDA AUB
        CMP #5
        BEQ @draw
        DEC AUB
        BNE @draw
@u:     CMP #186
        BCC @draw
        LDA AUB
        CMP #8
        BEQ @draw
        INC AUB
@draw:  LDA GC
        AND #1
        STA T0              ; the curtain's fold: two of each glyph
        LDY AUB
        LDA #G_FRINGE
        JSR @put
        DEY
        LDA #G_RAY
        JSR @put
        DEY
        LDA #G_RAY
        JSR @put
        DEY
        LDA #G_FADE
@put:   CLC
        ADC T0
        PHA
        LDA LROWLO,Y
        STA PTR
        LDA LROWHI,Y
        STA PTR+1
        STY T1
        LDY GC
        PLA
        STA (PTR),Y
        LDY T1
        RTS

; layer0: the layer for a new screen: the aurora along the top, stars below.
layer0: LDA #$20
        LDX #0
@c:     STA LAYERP,X
        STA LAYERP+$100,X
        STA LAYERP+$200,X
        STA LAYERP+$2E8,X
        INX
        BNE @c
        LDA #6
        STA AUB
        LDA #7
        STA AUL
        LDA #0
        STA AUG
        STA GC
@a:     JSR aurcol
        INC GC
        LDA GC
        CMP #40
        BNE @a
        LDX #NSTAR-1
@s:     JSR newstar
        JSR rnd
        AND #63
        CMP #40
        BCC @x
        SBC #24
@x:     STA STC,X
        TAY
        LDA STG,X
        STA (PTR),Y
        DEX
        BPL @s
        RTS

; ---------------------------------------------------------------- the table
; table: the sprite table for this frame. Entry 0 is the ship, always: it has
; the collision bits that matter. Entries 1 to NOBJ are everything else that
; flies. When SPRLINE has said a line had more than eight, they go in starting
; one further round each frame, so that whoever is ninth on a crowded line
; isn't the same sprite two frames running: they flicker, nobody vanishes.
table:  LDA STATE
        CMP #2
        BNE @noship
        LDA SHIPST
        BNE @dead
        LDA INVUL
        AND #4
        BNE @noship
        LDX #I_SHIP
        LDA CHG
        CMP #CHGSHOW
        BCC @img
        LDX #I_SHIPC
        CMP #CHGMAX
        BCC @img
        LDA TICK
        AND #4
        BEQ @img
        LDX #I_SHIPF
@img:   STX TABLE+2
        LDA SHY
        STA TABLE
        LDA SHX
        ASL A
        STA TABLE+1
        LDA #A_SHOWN/2
        ROL A               ; shown, and x bit 8
        STA TABLE+3
        JMP @dyn
@dead:  LDA #DEATHT         ; the ship blowing up, slowly
        SEC
        SBC DEADT
        CMP #32
        BCS @noship
        LSR A
        LSR A
        LSR A
        CLC
        ADC #I_EXP0
        TAX
        BNE @img
@noship:
        LDA #0
        STA TABLE+3
@dyn:   LDA #0              ; how many?
        STA NOBJ
        LDA DRON
        BEQ @c0
        INC NOBJ
@c0:    LDX #NSH-1
@c1:    LDA SOT,X
        BEQ @c1n
        INC NOBJ
@c1n:   DEX
        BPL @c1
        LDX #NEN-1
@c2:    LDA ENT,X
        BEQ @c2n
        INC NOBJ
@c2n:   DEX
        BPL @c2
        LDX #NBU-1
@c3:    LDA BUT,X
        BEQ @c3n
        INC NOBJ
@c3n:   DEX
        BPL @c3
        LDA NOBJ
        BNE @some
        JMP @hide
@some:  LDA FLICK
        BEQ @norot
        INC ROTN
        LDA ROTN
        CMP NOBJ
        BCC @rot
@norot: LDA #0
        STA ROTN
@rot:   LDA NOBJ
        CLC
        ADC #1
        ASL A
        ASL A
        STA TEND
        LDA ROTN
        CLC
        ADC #1
        ASL A
        ASL A
        TAY
        LDA DRON
        BEQ @shots
        LDA DRX
        STA T1
        LDA TICK
        LSR A
        LSR A
        AND #1
        CLC
        ADC #I_FLY0
        STA T2
        LDA #A_SHOWN
        STA T3
        LDA DRY
        JSR put
@shots: LDX #NSH-1
@sl:    LDA SOT,X
        BEQ @sn
        STX T0
        TAX
        LDA soimg,X
        STA T2
        LDA soatt,X
        STA T3
        LDX T0
        LDA SOX,X
        STA T1
        LDA SOY,X
        JSR put
@sn:    DEX
        BPL @sl
        LDX #NEN-1
@el:    LDA ENT,X
        BEQ @en
        LDA ENX,X
        STA T1
        LDA ENIMG,X
        STA T2
        LDA ENATT,X
        ORA #A_SHOWN
        STA T3
        LDA ENY,X
        JSR put
@en:    DEX
        BPL @el
        LDA #I_BUL
        STA T2
        LDA #A_SHOWN
        STA T3
        LDX #NBU-1
@bl:    LDA BUT,X
        BEQ @bn
        LDA BUX,X
        STA T1
        LDA BUY,X
        JSR put
@bn:    DEX
        BPL @bl
@hide:  LDA NOBJ            ; the entries not used, up to 23: hidden
        CLC
        ADC #1
        ASL A
        ASL A
        TAY
        LDA #0
@h:     CPY #24*4
        BCS @done
        STA TABLE+3,Y
        INY
        INY
        INY
        INY
        BNE @h
@done:  RTS

; put: an entry at offset Y: y = A, x = T1 (units), image T2, attributes T3.
; Y moves to the next entry, round from the last in use to entry 1.
put:    STA TABLE,Y
        LDA T1
        ASL A
        STA TABLE+1,Y
        LDA T2
        STA TABLE+2,Y
        LDA #0
        ROL A               ; x bit 8
        ORA T3
        STA TABLE+3,Y
        TYA
        CLC
        ADC #4
        CMP TEND
        BNE @n
        LDA #4
@n:     TAY
        RTS

; ---------------------------------------------------------------- the HUD
; hud: the 16 characters it should show, then at most one sprite redrawn.
hud:    JSR hudtext
        LDA FINE            ; a coarse step's frame is busy enough
        CMP #7
        BEQ @r
        LDX #8
@f:     LDA HUDK
        ASL A
        TAY
        LDA HUDC,Y
        CMP HUDS,Y
        BNE @draw
        LDA HUDC+1,Y
        CMP HUDS+1,Y
        BNE @draw
        LDA HUDK
        CLC
        ADC #1
        AND #7
        STA HUDK
        DEX
        BNE @f
@r:     RTS
@draw:  LDY HUDK
        JMP hudone

; hudtext: score, ship and lives, the charge bar, high score.
hudtext:
        LDX #5
@d:     LDA SCORE,X
        ORA #'0'
        STA HUDC,X
        LDA HISC,X
        ORA #'0'
        STA HUDC+10,X
        DEX
        BPL @d
        LDA #G_LIFE
        STA HUDC+6
        LDA LIVES
        ORA #'0'
        STA HUDC+7
        LDA CHG             ; 0-16 eighths of a cell, over two cells
        LSR A
        LSR A
        CMP #9
        BCC @one
        SBC #8
        STA HUDC+9
        LDA #8
        STA HUDC+8
        RTS
@one:   STA HUDC+8
        LDA #0
        STA HUDC+9
        RTS

; hudone: draws HUD sprite Y: its two characters into rows 4-11 of its image,
; each pixel of a glyph a pixel of the sprite, in the sprite's colour.
; Keeps X.
hudone: TXA
        PHA
        STY T5
        TYA
        ASL A
        TAY
        LDA HUDC,Y
        STA HUDS,Y
        JSR glyphp
        LDA GP1
        STA GP2
        LDA GP1+1
        STA GP2+1
        LDA HUDC+1,Y
        STA HUDS+1,Y
        JSR glyphp
        LDA T5
        JSR dynp
        LDA PTR
        CLC
        ADC #16             ; row 4
        STA PTR
        LDY T5
        LDA hudcm,Y
        STA T7
        LDA #0
        STA T6
@r:     LDY T6
        LDA (GP2),Y
        STA T0
        LDA (GP1),Y
        STA T1
        TYA
        ASL A
        ASL A
        TAY
        LDA T0
        LSR A
        LSR A
        LSR A
        LSR A
        TAX
        LDA exp1,X
        AND T7
        STA (PTR),Y
        INY
        LDA T0
        AND #15
        TAX
        LDA exp1,X
        AND T7
        STA (PTR),Y
        INY
        LDA T1
        LSR A
        LSR A
        LSR A
        LSR A
        TAX
        LDA exp1,X
        AND T7
        STA (PTR),Y
        INY
        LDA T1
        AND #15
        TAX
        LDA exp1,X
        AND T7
        STA (PTR),Y
        INC T6
        LDA T6
        CMP #8
        BNE @r
        PLA
        TAX
        RTS

; dynp: PTR -> image I_DYN + A.
dynp:   STA PTR+1           ; A * 64: low byte (A AND 3) * 64, high byte A / 4
        LSR A
        LSR A
        CLC
        ADC #>DYN
        PHA
        LDA PTR+1
        AND #3
        LSR A
        ROR A
        ROR A
        STA PTR
        PLA
        STA PTR+1
        RTS

; ---------------------------------------------------------------- colours
; The aurora's colour 1 steps along its list every 8 frames: the layer, and
; every sprite pixel drawn in 1, shimmer together.
colours:
        LDA TICK
        AND #7
        BNE @b
        INC AURI
        LDA AURI
        AND #15
        TAX
        LDA aurpal,X
        STA COLOUR1
@b:     LDA FLASHB
        BEQ @bk
        DEC FLASHB
        LDA #2
        BNE @bs
@bk:    LDA #0
@bs:    STA BORDER
        LDA FLASH2
        BEQ @w
        DEC FLASH2
        LDA #7
        BNE @c2
@w:     LDA #1
@c2:    STA COLOUR2
        RTS

; ---------------------------------------------------------------- game over
gameover:
        LDA #3
        STA STATE
        LDA #0
        STA OVERT
        STA OVERT2
        STA SCRON           ; the world stops
        STA BANNER
        STA CHG
        LDX #23*4           ; everything but the HUD goes
@h:     STA TABLE+3,X
        DEX
        DEX
        DEX
        DEX
        BPL @h
        JSR clearthings
        LDA #' '
        LDX #39
@row:   STA SCREEN+BANROW*40,X
        STA SCREEN+[BANROW-2]*40,X
        STA SCREEN+[BANROW+2]*40,X
        DEX
        BPL @row
        LDX #8
@t:     LDA t_over,X
        STA SCREEN+[BANROW-2]*40+16,X
        DEX
        BPL @t
        LDA #FX_OVER
        JMP sfx

st_over:
        JSR colours
        LDA OVERT
        CMP #255
        BCS @q
        INC OVERT
@q:     LDA TICK
        AND #15
        BNE @k
        INC OVERT2
        LDX #10             ; PRESS SPACE blinks
        LDA TICK
        AND #16
        BEQ @on
@off:   LDA #' '
        STA SCREEN+[BANROW+1]*40+15,X
        DEX
        BPL @off
        BMI @k
@on:    LDA t_press,X
        STA SCREEN+[BANROW+1]*40+15,X
        DEX
        BPL @on
@k:     LDA OVERT
        CMP #40
        BCC @r
        LDA KEYIN
        CMP #' '
        BNE @time
        JMP newgame
@time:  LDA OVERT2          ; 30 seconds: back to the title
        CMP #94
        BCC @r
        JMP gotitle
@r:     RTS

; ---------------------------------------------------------------- set-up helpers
; clearall: nothing flies, and the table shows nothing.
clearall:
        LDA #0
        LDX #127
@t:     STA TABLE,X
        DEX
        BPL @t
clearthings:
        LDA #0
        LDX #NSH-1
@s:     STA SOT,X
        DEX
        BPL @s
        LDX #NEN-1
@e:     STA ENT,X
        DEX
        BPL @e
        LDX #NBU-1
@b:     STA BUT,X
        DEX
        BPL @b
        STA DRON
        STA BOSS
        STA CLEART
        STA FLICK
        STA ROTN
        RTS

clrscreen:
        LDA #$20
        LDX #0
@c:     STA SCREEN,X
        STA SCREEN+$100,X
        STA SCREEN+$200,X
        STA SCREEN+$2E8,X
        INX
        BNE @c
        RTS

; laylist / scrlist: X/Y -> a list of texts: column, row, characters, 0;
; the list ends with a column of $FF. Into the layer, or onto the screen.
laylist:
        LDA #>LAYERP
        BNE lists
scrlist:
        LDA #>SCREEN
lists:  STA T0
        STX SRC
        STY SRC+1
@next:  LDY #0
        LDA (SRC),Y
        CMP #$FF
        BEQ @r
        STA T1              ; column
        INY
        LDA (SRC),Y         ; row: base + row * 40 + column
        TAX
        LDA ROWLO,X
        CLC
        ADC T1
        STA PTR
        LDA ROWHI,X
        ADC #0
        SEC
        SBC #>SCREEN
        CLC
        ADC T0
        STA PTR+1
        LDA SRC
        CLC
        ADC #2
        STA SRC
        BCC @c
        INC SRC+1
@c:     LDY #0
@ch:    LDA (SRC),Y
        BEQ @end
        STA (PTR),Y
        INY
        BNE @ch
@end:   INY
        TYA
        CLC
        ADC SRC
        STA SRC
        BCC @next
        INC SRC+1
        BNE @next
@r:     RTS

; initset: UNSCII into RAM, then our glyphs over it: each a code and 8 rows,
; the list ended by $FF.
initset:
        LDX #0
@c:     LDA CHARROM,X
        STA MYSET,X
        LDA CHARROM+$100,X
        STA MYSET+$100,X
        LDA CHARROM+$200,X
        STA MYSET+$200,X
        LDA CHARROM+$300,X
        STA MYSET+$300,X
        INX
        BNE @c
        LDA #<glyphs
        STA SRC
        LDA #>glyphs
        STA SRC+1
@g:     LDY #0
        LDA (SRC),Y
        CMP #$FF
        BEQ @r
        JSR glyphp
        LDY #1
@row:   LDA (SRC),Y
        DEY
        STA (GP1),Y
        INY
        INY
        CPY #9
        BNE @row
        LDA SRC
        CLC
        ADC #9
        STA SRC
        BCC @g
        INC SRC+1
        BNE @g
@r:     RTS

; rnd: A = a random byte, from a 16-bit xorshift (7, 9, 8). Keeps X and Y.
rnd:    LDA RNDH
        LSR A
        LDA RNDL
        ROR A
        EOR RNDH
        STA RNDH
        ROR A
        EOR RNDL
        STA RNDL
        EOR RNDH
        STA RNDH
        RTS

; ---------------------------------------------------------------- sound
; One voice, the beeper's timer. An effect is its priority, then steps of
; three bytes, then a 0:
;   frames, divisor low, divisor high   a tone of 1,000,000 / divisor Hz
;   frames, base, $80 + mask            noise: each frame a new divisor,
;                                       NOISE low, (NOISE AND mask) OR base high
; A new effect starts only if nothing more important is playing. The
; sequencer touches the timer only when a step starts (or for noise), so a
; note costs nothing while it plays. With nothing playing, a charging LANCE
; hums, higher as it charges.
sound:  LDA SNDT
        BEQ @idle
        DEC SNDT
        BEQ step
        LDA SNDN
        BNE rattle
        RTS
@idle:  LDA STATE
        CMP #2
        BNE @off
        LDA CHG
        CMP #CHGSHOW
        BCC @off
        LSR A               ; 4,000 down to about 1,500: the charge's pitch
        TAY
        LDA #0
        STA TIMER_LO
        LDA hum,Y
        CPY #CHGMAX/2
        BCC @h
        LDA TICK            ; full: it warbles
        AND #2
        CLC
        ADC #5
@h:     STA TIMER_HI
        LDA #%11
        STA SPEAKER
        STA HUM
        RTS
@off:   LDA HUM
        BEQ @r
        LDA #0
        STA HUM
        STA SPEAKER
@r:     RTS
; Start the step at SNDP, and move SNDP on.
step:   LDY #0
        LDA (SNDP),Y
        BEQ hush
        STA SNDT
        INY
        LDA (SNDP),Y
        STA TIMER_LO
        STA SNDB
        INY
        LDA (SNDP),Y
        BMI @noise
        STA TIMER_HI
        LDA #0
        STA SNDN
        BEQ @next
@noise: AND #$7F
        STA SNDN
        JSR rattle
@next:  LDA SNDP
        CLC
        ADC #3
        STA SNDP
        BCC @gate
        INC SNDP+1
@gate:  LDA #%11
        STA SPEAKER
        LDA #0
        STA HUM
        RTS
hush:   LDA #0
        STA SPEAKER
        STA SNDT
        STA SNDPRI
        RTS
rattle: LDA NOISE
        STA TIMER_LO
        LDA NOISE
        AND SNDN
        ORA SNDB
        STA TIMER_HI
        RTS

; sfx: start effect A at the end of this frame's work, unless something more
; important plays. Keeps X.
sfx:    ASL A
        TAY
        LDA fxtab,Y
        STA SPTR
        LDA fxtab+1,Y
        STA SPTR+1
        LDY #0
        LDA (SPTR),Y
        CMP SNDPRI
        BCC @skip
        STA SNDPRI
        LDA SPTR
        CLC
        ADC #1
        STA SNDP
        LDA SPTR+1
        ADC #0
        STA SNDP+1
        LDA #1
        STA SNDT
@skip:  RTS

quiet:  LDA #0
        STA SPEAKER
        STA SNDT
        STA SNDPRI
        STA HUM
        RTS

fxtab:  .word fx_shot, fx_tick, fx_pop, fx_lance, fx_boom, fx_pick, fx_stage, fx_warn, fx_die, fx_over
fx_shot:
        .byte 1
        .byte 1, <600, >600
        .byte 1, <850, >850
        .byte 1, <1200, >1200
        .byte 0
fx_tick:
        .byte 1
        .byte 1, <3000, >3000
        .byte 0
fx_pop: .byte 2
        .byte 2, $02, $80+$01
        .byte 3, $04, $80+$03
        .byte 2, <2400, >2400
        .byte 0
fx_lance:
        .byte 3
        .byte 2, <900, >900
        .byte 2, <700, >700
        .byte 2, <520, >520
        .byte 2, <400, >400
        .byte 3, <320, >320
        .byte 4, $03, $80+$03
        .byte 0
fx_boom:
        .byte 4
        .byte 3, $04, $80+$07
        .byte 5, $08, $80+$0F
        .byte 8, $10, $80+$1F
        .byte 0
fx_pick:
        .byte 5
        .byte 3, <956, >956
        .byte 3, <758, >758
        .byte 3, <638, >638
        .byte 6, <478, >478
        .byte 0
fx_stage:
        .byte 6
        .byte 7, <1276, >1276
        .byte 7, <956, >956
        .byte 7, <758, >758
        .byte 7, <638, >638
        .byte 3, 1, 0
        .byte 14, <478, >478
        .byte 0
fx_warn:
        .byte 6
        .byte 10, <1136, >1136
        .byte 10, <1517, >1517
        .byte 10, <1136, >1136
        .byte 10, <1517, >1517
        .byte 10, <1136, >1136
        .byte 10, <1517, >1517
        .byte 0
fx_die: .byte 7
        .byte 4, $04, $80+$07
        .byte 8, $10, $80+$0F
        .byte 12, $20, $80+$1F
        .byte 20, $40, $80+$3F
        .byte 0
fx_over:
        .byte 8
        .byte 12, <1276, >1276
        .byte 4, 1, 0
        .byte 12, <1517, >1517
        .byte 4, 1, 0
        .byte 12, <1911, >1911
        .byte 4, 1, 0
        .byte 30, <3822, >3822
        .byte 0

; the charge's hum: the timer's high byte for CHG / 2
hum:    .rept 32, i
        .byte 16 - i / 3
        .endr

; ---------------------------------------------------------------- tables
ROWLO:  .rept 25, r
        .byte <[SCREEN + r * 40]
        .endr
ROWHI:  .rept 25, r
        .byte >[SCREEN + r * 40]
        .endr
LROWLO: .rept 25, r
        .byte <[LAYERP + r * 40]
        .endr
LROWHI: .rept 25, r
        .byte >[LAYERP + r * 40]
        .endr
bitmask: .byte $80, $40, $20, $10, $08, $04, $02, $01
; a nibble's 4 pixels, each 2 bits of 11 (masked to the colour after)
exp1:   .rept 16, n
        .byte [[n >> 3] & 1] * $C0 | [[n >> 2] & 1] * $30 | [[n >> 1] & 1] * $0C | [n & 1] * $03
        .endr
; 2 pixels doubled: 4 pixels of 11
half:   .byte $00, $0F, $F0, $FF

; a signed step (1/16 of a unit or pixel) is v * 16 added to an 8.8 number:
; the low byte is v shifted left four times, the high byte from this table
DHI:    .rept 256, v
        .byte >[[v - 256 * [v >> 7]] * 16]
        .endr
; sine, 256 steps a circle, -32 to 32
SIN:    .rept 256, ang
        .byte <sin(ang, 32)
        .endr
; bullets: 16 directions at four speeds, 1.25 to 2 pixels a frame:
; x in 1/16 units (1/8 pixel), y in 1/16 pixels
VDX:    .rept 64, i
        .byte <cos([i & 15] * 16, 10 + 2 * [i >> 4])
        .endr
VDY:    .rept 64, i
        .byte <sin([i & 15] * 16, 20 + 4 * [i >> 4])
        .endr

; the ship's outline, for the terrain: pixels from its top-left
shpx:   .byte 2, 2, 7, 7, 10, 10, 12, 0
shpy:   .byte 2, 13, 4, 11, 6, 9, 7, 7

; the keys a quick tap counts for
tapkey: .byte 28, 'A', 29, 'D', 30, 'W', 31, 'S', ' ', ' '
tapbit: .byte 1, 1, 2, 2, 4, 4, 8, 8, 16, 16

; shots, by type: speed, box (units, pixels), tip, damage, picture, attributes
sospd:  .byte 0, 4, 4, 5, 5
sobx0:  .byte 0, 2, 2, 1, 2
sobx1:  .byte 0, 7, 7, 7, 15
soby0:  .byte 0, 6, 3, 4, 8
soby1:  .byte 0, 8, 12, 11, 23
sotx:   .byte 0, 7, 7, 7, 15
soty:   .byte 0, 7, 7, 7, 15
sodmg:  .byte 0, 1, 2, 4, 8
soimg:  .byte 0, I_SHOT, I_TWIN, I_LANCE, I_LANCE
soatt:  .byte 0, A_SHOWN, A_SHOWN, A_SHOWN, A_SHOWN+A_WIDE

; the enemies' slots, by type: what they are, their boxes, points (tens), hits
;                   -  WISP  SKUA  GROWL WARDEN SHARD EXPL  PICK
eflag:  .byte 0, F_SHOOT+F_HURT, F_SHOOT+F_HURT, F_SHOOT+F_HURT, F_SHOOT+F_HURT, F_BLOCK+F_HURT, 0, F_PICK
ebx0:   .byte 0, 1, 0, 1, 2, 2, 0, 1
ebx1:   .byte 0, 6, 7, 6, 13, 5, 0, 6
eby0:   .byte 0, 3, 3, 1, 4, 4, 0, 2
eby1:   .byte 0, 12, 12, 14, 27, 11, 0, 10
escore: .byte 0, 10, 15, 30, 0, 0, 0, 0
ehp:    .byte 0, 1, 1, 4, 1, 1, 1, 1
wspace: .byte 0, 12, 22, 45     ; frames between one of a wave and the next

; how mean, by DIFF (the stage, to 7)
wispfire: .byte 1, 2, 2, 3, 3, 4, 4, 5
turfire:  .byte 14, 18, 22, 26, 30, 34, 38, 42
bspd:     .byte 0, 0, 16, 16, 32, 32, 48, 48
bosshp:   .byte 24, 30, 36, 42, 48, 54, 60, 66
bossmask: .byte 63, 63, 63, 31, 31, 31, 31, 31
tumble: .byte 0, A_FLIPX, A_FLIPX+A_FLIPY, A_FLIPY

aurpal: .byte 5, 5, 13, 13, 3, 13, 5, 5, 13, 3, 14, 3, 13, 5, 4, 5

; the HUD: x of each sprite, its attributes (x bit 8), its colour
hudx:   .byte 8, 24, 40, 72, 104, <264, <280, <296
hudatt: .byte A_SHOWN, A_SHOWN, A_SHOWN, A_SHOWN, A_SHOWN, A_SHOWN+1, A_SHOWN+1, A_SHOWN+1
hudcm:  .byte $AA, $AA, $AA, $FF, $55, $FF, $FF, $FF

; the title's letters
logoch: .byte 'A', 'U', 'R', 'O'
logoimg: .byte I_DYN+0, I_DYN+1, I_DYN+2, I_DYN+3, I_DYN+2, I_DYN+0
logox:  .rept 6, k
        .byte 52 + 36 * k
        .endr
logoent: .rept 6, k
        .byte [24 + k] * 4
        .endr
logoph: .rept 6, k
        .byte k * 24
        .endr
lettercm: .byte $AA, $AA, $AA, $AA, $FF, $FF, $FF, $FF

; terrain styles, 8 bytes each: roof min, max; ground min, max; most the two
; may add up to; chance (of 256) of a FROSTEYE on a flat cell; of a spruce
; (kind 0) or an icicle (kind 1); kind
styles:
stcmin = styles
stcmax = styles+1
stfmin = styles+2
stfmax = styles+3
stsum  = styles+4
stturr = styles+5
stdec  = styles+6
stkind = styles+7
        .byte 0, 0, 1, 5, 12, 0, 70, 0      ; S_TITLE
        .byte 0, 0, 1, 6, 12, 14, 60, 0     ; S_FELLS
        .byte 2, 5, 2, 5, 10, 34, 70, 1     ; S_CAVE
        .byte 3, 6, 3, 6, 12, 40, 50, 1     ; S_GORGE
        .byte 0, 0, 1, 2, 12, 0, 40, 0      ; S_FLAT
        .byte 1, 3, 1, 4, 9, 24, 60, 1      ; S_LOW

; the stages: terrain colour, name, script
stcol:  .byte 15, 14, 10
stname: .word t_fells, t_caves, t_gorge
stwave: .word sc_fells, sc_caves, sc_gorge

; The scripts: type, how many, parameter, frames to wait after.
; WISP: the parameter moves its line up (-) or down from the gap's middle.
; SKUA: 0 from the roof, 1 from the ground, 2 both in turn.
sc_title:
        .byte R_STYLE, S_TITLE, 0, 120
        .byte R_WISP, 5, 0, 180
        .byte R_WISP, 5, <-30, 200
        .byte R_WISP, 5, 30, 200
        .byte R_LOOP, 0, 0, 1
sc_fells:
        .byte R_STYLE, S_FELLS, 0, 60
        .byte R_WISP, 5, 0, 70
        .byte R_WISP, 5, <-28, 70
        .byte R_SKUA, 3, 0, 80
        .byte R_WISP, 6, 28, 60
        .byte R_GROWL, 2, 0, 100
        .byte R_SKUA, 4, 1, 80
        .byte R_WISP, 6, 0, 60
        .byte R_SKUA, 4, 2, 90
        .byte R_GROWL, 3, 0, 100
        .byte R_WISP, 6, <-28, 40
        .byte R_WISP, 6, 28, 90
        .byte R_STYLE, S_FLAT, 0, 90
        .byte R_BANNER, 0, 0, 200
        .byte R_BOSS, 0, 0, 1
sc_caves:
        .byte R_STYLE, S_LOW, 0, 50
        .byte R_STYLE, S_CAVE, 0, 60
        .byte R_WISP, 6, 0, 60
        .byte R_SKUA, 4, 2, 80
        .byte R_GROWL, 3, 0, 90
        .byte R_WISP, 6, <-16, 40
        .byte R_WISP, 6, 16, 80
        .byte R_SKUA, 6, 2, 80
        .byte R_GROWL, 4, 0, 100
        .byte R_WISP, 8, 0, 80
        .byte R_STYLE, S_LOW, 0, 40
        .byte R_STYLE, S_FELLS, 0, 60
        .byte R_STYLE, S_FLAT, 0, 90
        .byte R_BANNER, 0, 0, 200
        .byte R_BOSS, 0, 0, 1
sc_gorge:
        .byte R_STYLE, S_LOW, 0, 30
        .byte R_STYLE, S_GORGE, 0, 60
        .byte R_SKUA, 4, 2, 60
        .byte R_WISP, 6, 0, 50
        .byte R_GROWL, 3, 0, 80
        .byte R_SKUA, 6, 2, 60
        .byte R_WISP, 8, <-16, 40
        .byte R_WISP, 8, 16, 70
        .byte R_GROWL, 4, 0, 80
        .byte R_SKUA, 6, 2, 70
        .byte R_WISP, 8, 0, 80
        .byte R_STYLE, S_LOW, 0, 40
        .byte R_STYLE, S_FLAT, 0, 90
        .byte R_BANNER, 0, 0, 200
        .byte R_BOSS, 0, 0, 1

banners: .word t_warn

; ---------------------------------------------------------------- texts
t_nb:   .byte 6, 9
        .text "AURORA NEEDS THE VIDEO BOARD."
        .byte 0
        .byte 7, 12
        .text "SWITCH OFF, FIT THE BOARD,"
        .byte 0
        .byte 7, 13
        .text "SWITCH ON AND LOAD AGAIN."
        .byte 0
        .byte 7, 16
        .text "UNITED NORTHERN COMPUTERS"
        .byte 0, $FF
t_title:
        .byte 3, 0
        .text "(C) 1984 UNITED NORTHERN COMPUTERS"
        .byte 0
        .byte 3, 9
        .text "A DEMONSTRATION OF THE VIDEO BOARD"
        .byte 0
        .byte 15, 15
        .text "HI"
        .byte 0
        .byte 5, 17
        .text "ARROWS OR WASD FLY, SPACE FIRES"
        .byte 0
        .byte 4, 18
        .text "HOLD SPACE TO CHARGE THE LANCE"
        .byte 0, $FF
t_press: .text "PRESS SPACE"
t_over: .text "GAME OVER"
t_stage: .text "STAGE "
        .byte 0
t_fells: .text "THE FELLS"
        .byte 0
t_caves: .text "THE ICE CAVES"
        .byte 0
t_gorge: .text "THE IRON GORGE"
        .byte 0
t_warn: .text "WARNING ... THE WARDEN WAKES"
        .byte 0
t_clear: .text "STAGE CLEAR"
        .byte 0

; ---------------------------------------------------------------- glyphs
; Our characters, over UNSCII's: a code, then 8 rows (bit 7 the leftmost).
; $00-$08 the HUD's charge bar; $60-$6F the terrain (colour 3, on the screen);
; $70-$7A the aurora and the stars (colour 1, in the layer).
glyphs:
        .byte $00            ; G_BAR0: the charge bar, 0 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $01            ; G_BAR1: the charge bar, 1 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %10000000       ; #.......
        .byte %10000000       ; #.......
        .byte %10000000       ; #.......
        .byte %10000000       ; #.......
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $02            ; G_BAR2: the charge bar, 2 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11000000       ; ##......
        .byte %11000000       ; ##......
        .byte %11000000       ; ##......
        .byte %11000000       ; ##......
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $03            ; G_BAR3: the charge bar, 3 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11100000       ; ###.....
        .byte %11100000       ; ###.....
        .byte %11100000       ; ###.....
        .byte %11100000       ; ###.....
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $04            ; G_BAR4: the charge bar, 4 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11110000       ; ####....
        .byte %11110000       ; ####....
        .byte %11110000       ; ####....
        .byte %11110000       ; ####....
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $05            ; G_BAR5: the charge bar, 5 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11111000       ; #####...
        .byte %11111000       ; #####...
        .byte %11111000       ; #####...
        .byte %11111000       ; #####...
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $06            ; G_BAR6: the charge bar, 6 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11111100       ; ######..
        .byte %11111100       ; ######..
        .byte %11111100       ; ######..
        .byte %11111100       ; ######..
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $07            ; G_BAR7: the charge bar, 7 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11111110       ; #######.
        .byte %11111110       ; #######.
        .byte %11111110       ; #######.
        .byte %11111110       ; #######.
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $08            ; G_BAR8: the charge bar, 8 of 8
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %11111111       ; ########
        .byte %11111111       ; ########
        .byte %11111111       ; ########
        .byte %11111111       ; ########
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $60            ; G_ROCK: rock
        .byte %11111111       ; ########
        .byte %11101111       ; ###.####
        .byte %11111111       ; ########
        .byte %10111111       ; #.######
        .byte %11111111       ; ########
        .byte %11111011       ; #####.##
        .byte %11111111       ; ########
        .byte %11111101       ; ######.#
        .byte $61            ; G_CEIL: the cave roof, flat, with icicles
        .byte %11111111       ; ########
        .byte %11101111       ; ###.####
        .byte %11111111       ; ########
        .byte %10111011       ; #.###.##
        .byte %01111010       ; .####.#.
        .byte %01100110       ; .##..##.
        .byte %00100010       ; ..#...#.
        .byte %00100000       ; ..#.....
        .byte $62            ; G_CDN: the roof going down
        .byte %11111111       ; ########
        .byte %01111111       ; .#######
        .byte %00111111       ; ..######
        .byte %00011101       ; ...###.#
        .byte %00001111       ; ....####
        .byte %00000111       ; .....###
        .byte %00000011       ; ......##
        .byte %00000001       ; .......#
        .byte $63            ; G_CUP: the roof going up
        .byte %11111111       ; ########
        .byte %11111110       ; #######.
        .byte %11111100       ; ######..
        .byte %10111100       ; #.####..
        .byte %11111000       ; #####...
        .byte %11100000       ; ###.....
        .byte %11000000       ; ##......
        .byte %10000000       ; #.......
        .byte $64            ; G_FLOOR: the ground, flat, with snow on it
        .byte %00011000       ; ...##...
        .byte %01111110       ; .######.
        .byte %11111111       ; ########
        .byte %11111111       ; ########
        .byte %11011111       ; ##.#####
        .byte %11111111       ; ########
        .byte %11111101       ; ######.#
        .byte %11111111       ; ########
        .byte $65            ; G_FUP: the ground going up
        .byte %00000001       ; .......#
        .byte %00000011       ; ......##
        .byte %00000111       ; .....###
        .byte %00001111       ; ....####
        .byte %00011111       ; ...#####
        .byte %00111111       ; ..######
        .byte %01110111       ; .###.###
        .byte %11111111       ; ########
        .byte $66            ; G_FDN: the ground going down
        .byte %10000000       ; #.......
        .byte %11000000       ; ##......
        .byte %11100000       ; ###.....
        .byte %11110000       ; ####....
        .byte %11111000       ; #####...
        .byte %11111100       ; ######..
        .byte %11101110       ; ###.###.
        .byte %11111111       ; ########
        .byte $67            ; G_EYEF: a FROSTEYE on the ground
        .byte %00000000       ; ........
        .byte %00111100       ; ..####..
        .byte %01100110       ; .##..##.
        .byte %11011011       ; ##.##.##
        .byte %11011011       ; ##.##.##
        .byte %01100110       ; .##..##.
        .byte %01111110       ; .######.
        .byte %11111111       ; ########
        .byte $68            ; G_EYEC: a FROSTEYE on the roof
        .byte %11111111       ; ########
        .byte %01111110       ; .######.
        .byte %01100110       ; .##..##.
        .byte %11011011       ; ##.##.##
        .byte %11011011       ; ##.##.##
        .byte %01100110       ; .##..##.
        .byte %00111100       ; ..####..
        .byte %00000000       ; ........
        .byte $69            ; G_PITF: what a FROSTEYE leaves on the ground
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00100100       ; ..#..#..
        .byte %01011010       ; .#.##.#.
        .byte %01111110       ; .######.
        .byte %11111111       ; ########
        .byte $6A            ; G_PITC: and on the roof
        .byte %11111111       ; ########
        .byte %01111110       ; .######.
        .byte %01011010       ; .#.##.#.
        .byte %00100100       ; ..#..#..
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $6B            ; G_TREE: a spruce on the fells
        .byte %00010000       ; ...#....
        .byte %00010000       ; ...#....
        .byte %00111000       ; ..###...
        .byte %00010000       ; ...#....
        .byte %00111000       ; ..###...
        .byte %01111100       ; .#####..
        .byte %00111000       ; ..###...
        .byte %01111100       ; .#####..
        .byte $6C            ; G_ICE: a long icicle
        .byte %01000010       ; .#....#.
        .byte %01000010       ; .#....#.
        .byte %01000010       ; .#....#.
        .byte %01000000       ; .#......
        .byte %01000000       ; .#......
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $6D            ; G_LIFE: a ship, for the lives
        .byte %00000000       ; ........
        .byte %11000000       ; ##......
        .byte %01111000       ; .####...
        .byte %01111110       ; .######.
        .byte %01111000       ; .####...
        .byte %11000000       ; ##......
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $6E            ; G_CTIP: the roof down and up again: a point
        .byte %11111111       ; ########
        .byte %01111110       ; .######.
        .byte %00111100       ; ..####..
        .byte %00111000       ; ..###...
        .byte %00011000       ; ...##...
        .byte %00011000       ; ...##...
        .byte %00010000       ; ...#....
        .byte %00010000       ; ...#....
        .byte $6F            ; G_FTIP: the ground up and down again: a peak
        .byte %00010000       ; ...#....
        .byte %00011000       ; ...##...
        .byte %00111000       ; ..###...
        .byte %00111100       ; ..####..
        .byte %01111100       ; .#####..
        .byte %01111110       ; .######.
        .byte %11111101       ; ######.#
        .byte %11111111       ; ########
        .byte $70            ; G_FRINGE: the aurora's bright lower edge, with rays above it
        .byte %01000100       ; .#...#..
        .byte %01001100       ; .#..##..
        .byte %11001101       ; ##..##.#
        .byte %11011101       ; ##.###.#
        .byte %11111110       ; #######.
        .byte %11111111       ; ########
        .byte %01101110       ; .##.###.
        .byte %00010000       ; ...#....
        .byte $71            ; G_FRINGE2: the edge, another fold
        .byte %00010010       ; ...#..#.
        .byte %10010010       ; #..#..#.
        .byte %10110110       ; #.##.##.
        .byte %10111110       ; #.#####.
        .byte %11111111       ; ########
        .byte %11111110       ; #######.
        .byte %11011011       ; ##.##.##
        .byte %00000100       ; .....#..
        .byte $72            ; G_RAY: rays
        .byte %01000100       ; .#...#..
        .byte %01000000       ; .#......
        .byte %01000100       ; .#...#..
        .byte %00000100       ; .....#..
        .byte %01000100       ; .#...#..
        .byte %01000101       ; .#...#.#
        .byte %01000001       ; .#.....#
        .byte %01000101       ; .#...#.#
        .byte $73            ; G_RAY2: rays, another fold
        .byte %00010010       ; ...#..#.
        .byte %10010010       ; #..#..#.
        .byte %10000010       ; #.....#.
        .byte %10010000       ; #..#....
        .byte %10010010       ; #..#..#.
        .byte %00010010       ; ...#..#.
        .byte %10010010       ; #..#..#.
        .byte %10000010       ; #.....#.
        .byte $74            ; G_FADE: where the rays fade out
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000100       ; .....#..
        .byte %00000000       ; ........
        .byte %01000000       ; .#......
        .byte %00000100       ; .....#..
        .byte %01000000       ; .#......
        .byte %01000100       ; .#...#..
        .byte $75            ; G_FADE2: fading, another fold
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00010000       ; ...#....
        .byte %00000010       ; ......#.
        .byte %00010000       ; ...#....
        .byte %10000010       ; #.....#.
        .byte %10010010       ; #..#..#.
        .byte $78            ; G_STAR: a star
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00010000       ; ...#....
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $79            ; G_STAR2: a brighter star
        .byte %00000000       ; ........
        .byte %00010000       ; ...#....
        .byte %00111000       ; ..###...
        .byte %00010000       ; ...#....
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte $7A            ; G_STAR3: two faint stars
        .byte %00000000       ; ........
        .byte %00000100       ; .....#..
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %00000000       ; ........
        .byte %01000000       ; .#......
        .byte %00000000       ; ........
        .byte $FF

; ---------------------------------------------------------------- sprite images
; 16 x 16, four pixels a byte (bits 7-6 the leftmost), 64 bytes each.
; 0 is see-through; 1 is the aurora's colour (it shimmers), 2 white, 3 the
; stage's colour. SPRBANK points at this page.
        .align 256
IMAGES:
; the ship, LUMI: hull 2, canopy and fins 3, the engine 1 (it shimmers)
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $0F, $F0, $00, $00     ; ..3333..........
        .byte $02, $AA, $00, $00     ; ...22222........
        .byte $2A, $AA, $F0, $00     ; .222222233......
        .byte $6A, $AA, $FC, $00     ; 12222222333.....
        .byte $5A, $AA, $AA, $80     ; 1122222222222...
        .byte $5A, $AA, $AA, $80     ; 1122222222222...
        .byte $6A, $AA, $A8, $00     ; 12222222222.....
        .byte $2A, $AA, $A0, $00     ; .222222222......
        .byte $02, $AA, $00, $00     ; ...22222........
        .byte $0F, $F0, $00, $00     ; ..3333..........
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; charging: a spark at the nose
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $0F, $F0, $00, $00     ; ..3333..........
        .byte $02, $AA, $00, $00     ; ...22222........
        .byte $2A, $AA, $F0, $00     ; .222222233......
        .byte $6A, $AA, $FC, $04     ; 12222222333...1.
        .byte $5A, $AA, $AA, $B7     ; 1122222222222313
        .byte $5A, $AA, $AA, $B7     ; 1122222222222313
        .byte $6A, $AA, $A8, $04     ; 12222222222...1.
        .byte $2A, $AA, $A0, $00     ; .222222222......
        .byte $02, $AA, $00, $00     ; ...22222........
        .byte $0F, $F0, $00, $00     ; ..3333..........
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; fully charged: the spark is a ball
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $0F, $F0, $00, $10     ; ..3333.......1..
        .byte $02, $AA, $00, $74     ; ...22222....131.
        .byte $2A, $AA, $F1, $FD     ; .222222233.13331
        .byte $6A, $AA, $FC, $EB     ; 12222222333.3223
        .byte $5A, $AA, $AA, $AB     ; 1122222222222223
        .byte $5A, $AA, $AA, $AB     ; 1122222222222223
        .byte $6A, $AA, $A8, $EB     ; 12222222222.3223
        .byte $2A, $AA, $A1, $FD     ; .222222222.13331
        .byte $02, $AA, $00, $74     ; ...22222....131.
        .byte $0F, $F0, $00, $10     ; ..3333.......1..
        .byte $3F, $C0, $00, $00     ; .3333...........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; FIREFLY, the drone that follows you
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $40, $01, $00     ; ....1......1....
        .byte $00, $10, $04, $00     ; .....1....1.....
        .byte $00, $0A, $A0, $00     ; ......2222......
        .byte $00, $2F, $F8, $00     ; .....233332.....
        .byte $00, $BE, $BE, $00     ; ....23322332....
        .byte $00, $BE, $BE, $00     ; ....23322332....
        .byte $00, $2F, $F8, $00     ; .....233332.....
        .byte $00, $0A, $A0, $00     ; ......2222......
        .byte $00, $10, $04, $00     ; .....1....1.....
        .byte $00, $40, $01, $00     ; ....1......1....
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; FIREFLY, wings down
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $01, $00, $00, $40     ; ...1........1...
        .byte $00, $52, $85, $00     ; ....11.22.11....
        .byte $00, $2F, $F8, $00     ; .....233332.....
        .byte $00, $BE, $BE, $00     ; ....23322332....
        .byte $00, $BE, $BE, $00     ; ....23322332....
        .byte $00, $2F, $F8, $00     ; .....233332.....
        .byte $00, $52, $85, $00     ; ....11.22.11....
        .byte $01, $00, $00, $40     ; ...1........1...
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; a shot
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $05, $5A, $AA, $FC     ; ..1111222222333.
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; a TWIN shot: two bolts
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $05, $5A, $AA, $FC     ; ..1111222222333.
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $05, $5A, $AA, $FC     ; ..1111222222333.
        .byte $00, $05, $6A, $B0     ; ......11122223..
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; the LANCE, a charged shot that goes through things (wide when full)
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $01, $55, $40     ; .......111111...
        .byte $00, $56, $AA, $BC     ; ....11122222233.
        .byte $15, $AA, $AA, $BF     ; .111222222222333
        .byte $5A, $AB, $FF, $A8     ; 112222233333222.
        .byte $5A, $AB, $FF, $A8     ; 112222233333222.
        .byte $15, $AA, $AA, $BF     ; .111222222222333
        .byte $00, $56, $AA, $BC     ; ....11122222233.
        .byte $00, $01, $55, $40     ; .......111111...
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; WISP: drifts in a sine wave
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $2A, $80, $00     ; .....2222.......
        .byte $02, $BF, $E8, $40     ; ...22333322.1...
        .byte $0B, $FF, $FE, $04     ; ..2333333332..1.
        .byte $2F, $AF, $AF, $85     ; .233223322332.11
        .byte $2F, $83, $83, $80     ; .2332..32..32...
        .byte $2F, $AF, $AF, $84     ; .233223322332.1.
        .byte $0B, $FF, $FE, $41     ; ..23333333321..1
        .byte $02, $BF, $E8, $04     ; ...22333322...1.
        .byte $00, $2A, $80, $05     ; .....2222.....11
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; WISP, the tail flicks
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $2A, $80, $04     ; .....2222.....1.
        .byte $02, $BF, $E8, $10     ; ...22333322..1..
        .byte $0B, $FF, $FE, $10     ; ..2333333332.1..
        .byte $2F, $AF, $AF, $81     ; .233223322332..1
        .byte $2F, $83, $83, $90     ; .2332..32..321..
        .byte $2F, $AF, $AF, $81     ; .233223322332..1
        .byte $0B, $FF, $FE, $44     ; ..23333333321.1.
        .byte $02, $BF, $E8, $10     ; ...22333322..1..
        .byte $00, $2A, $80, $10     ; .....2222....1..
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; SKUA: comes in level, then dives at you
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $0F, $00     ; ..........33....
        .byte $00, $00, $3F, $00     ; .........333....
        .byte $00, $00, $FA, $00     ; ........3322....
        .byte $00, $0A, $A8, $00     ; ......22222.....
        .byte $02, $AA, $A9, $55     ; ...2222222211111
        .byte $FA, $AA, $AA, $44     ; 3322222222221.1.
        .byte $02, $AA, $A9, $55     ; ...2222222211111
        .byte $00, $0A, $A8, $00     ; ......22222.....
        .byte $00, $00, $FA, $00     ; ........3322....
        .byte $00, $00, $3F, $00     ; .........333....
        .byte $00, $00, $0F, $00     ; ..........33....
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; SKUA diving (upside down: diving up)
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $03, $C0, $00     ; .......33.......
        .byte $00, $0F, $C0, $00     ; ......333.......
        .byte $00, $3E, $80, $00     ; .....3322.......
        .byte $05, $AA, $A0, $00     ; ..11222222......
        .byte $11, $AA, $AA, $00     ; .1.122222222....
        .byte $44, $2A, $AA, $A0     ; 1.1..222222222..
        .byte $00, $3A, $AA, $A8     ; .....3222222222.
        .byte $00, $0E, $AA, $AC     ; ......322222223.
        .byte $00, $03, $AA, $3C     ; .......32222.33.
        .byte $00, $00, $EA, $0C     ; ........3222..3.
        .byte $00, $00, $3A, $80     ; .........3222...
        .byte $00, $00, $0E, $80     ; ..........322...
        .byte $00, $00, $03, $80     ; ...........32...
        .byte $00, $00, $00, $C0     ; ............3...
        .byte $00, $00, $00, $00     ; ................
; GROWLER, a chunk of drifting ice (it tumbles by flipping)
        .byte $00, $0F, $FC, $00     ; ......33333.....
        .byte $00, $FA, $AB, $C0     ; ....332222233...
        .byte $03, $AA, $AA, $B0     ; ...32222222223..
        .byte $0E, $A5, $6A, $AC     ; ..3222111222223.
        .byte $0E, $96, $AA, $A3     ; ..322112222222.3
        .byte $3A, $5A, $AA, $B0     ; .3221122222223..
        .byte $3A, $6A, $AA, $AC     ; .32212222222223.
        .byte $EA, $AA, $AA, $AC     ; 322222222222223.
        .byte $EA, $AA, $95, $AC     ; 322222222111223.
        .byte $3A, $AA, $A5, $6C     ; .32222222211123.
        .byte $3A, $AA, $AA, $B0     ; .3222222222223..
        .byte $0E, $AA, $AA, $B0     ; ..322222222223..
        .byte $03, $AA, $AA, $C0     ; ...3222222223...
        .byte $03, $EA, $BC, $00     ; ...33222233.....
        .byte $00, $3F, $C0, $00     ; .....3333.......
        .byte $00, $00, $00, $00     ; ................
; their bullet
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $01, $40, $00     ; .......11.......
        .byte $00, $06, $90, $00     ; ......1221......
        .byte $00, $06, $90, $00     ; ......1221......
        .byte $00, $01, $40, $00     ; .......11.......
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; an explosion, frame 1
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $02, $00, $00     ; .......2........
        .byte $00, $0B, $80, $00     ; ......232.......
        .byte $00, $2F, $E0, $00     ; .....23332......
        .byte $00, $0B, $80, $00     ; ......232.......
        .byte $00, $02, $00, $00     ; .......2........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; frame 2
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $01, $00, $00     ; .......1........
        .byte $00, $42, $04, $00     ; ....1..2..1.....
        .byte $00, $2B, $A0, $00     ; .....22322......
        .byte $00, $AF, $E8, $00     ; ....2233322.....
        .byte $06, $FF, $F9, $00     ; ..1233333321....
        .byte $00, $AF, $E8, $00     ; ....2233322.....
        .byte $00, $2B, $A0, $00     ; .....22322......
        .byte $00, $42, $04, $00     ; ....1..2..1.....
        .byte $00, $01, $00, $00     ; .......1........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; frame 3
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $01, $00, $00     ; .......1........
        .byte $04, $02, $00, $40     ; ..1....2....1...
        .byte $02, $0B, $82, $00     ; ...2..232..2....
        .byte $00, $B3, $38, $00     ; ....23.3.32.....
        .byte $00, $30, $30, $00     ; .....3...3......
        .byte $1B, $C0, $0F, $90     ; .1233.....3321..
        .byte $00, $30, $30, $00     ; .....3...3......
        .byte $00, $B3, $38, $00     ; ....23.3.32.....
        .byte $02, $0B, $82, $00     ; ...2..232..2....
        .byte $04, $02, $00, $40     ; ..1....2....1...
        .byte $00, $01, $00, $00     ; .......1........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; frame 4
        .byte $00, $01, $00, $00     ; .......1........
        .byte $04, $00, $00, $40     ; ..1.........1...
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $40, $04, $00     ; ....1.....1.....
        .byte $10, $00, $00, $01     ; .1.............1
        .byte $00, $00, $00, $00     ; ................
        .byte $40, $00, $00, $00     ; 1...............
        .byte $00, $00, $00, $01     ; ...............1
        .byte $00, $00, $00, $00     ; ................
        .byte $10, $00, $00, $01     ; .1.............1
        .byte $00, $40, $04, $00     ; ....1.....1.....
        .byte $00, $00, $00, $00     ; ................
        .byte $04, $00, $00, $40     ; ..1.........1...
        .byte $00, $01, $00, $00     ; .......1........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; a capsule: FAN, the three-way shot
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $0C, $2A, $A8, $30     ; ..3..222222..3..
        .byte $0C, $28, $00, $30     ; ..3..22......3..
        .byte $0C, $2A, $A0, $30     ; ..3..22222...3..
        .byte $0C, $28, $00, $30     ; ..3..22......3..
        .byte $0C, $28, $00, $30     ; ..3..22......3..
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; a capsule: TWIN, two bolts at once
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $0C, $2A, $A8, $30     ; ..3..222222..3..
        .byte $0C, $02, $80, $30     ; ..3....22....3..
        .byte $0C, $02, $80, $30     ; ..3....22....3..
        .byte $0C, $02, $80, $30     ; ..3....22....3..
        .byte $0C, $02, $80, $30     ; ..3....22....3..
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; a capsule: a FIREFLY
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $0C, $40, $01, $30     ; ..3.1......1.3..
        .byte $0C, $1A, $A1, $30     ; ..3..12222.1.3..
        .byte $0C, $0B, $E0, $30     ; ..3...2332...3..
        .byte $0C, $1B, $E0, $30     ; ..3..12332...3..
        .byte $0C, $4A, $A1, $30     ; ..3.1.2222.1.3..
        .byte $03, $00, $00, $C0     ; ...3........3...
        .byte $00, $FF, $FF, $00     ; ....33333333....
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
; the WARDEN: drawn wide, 32 x 32
        .byte $00, $3F, $FC, $00     ; .....333333.....
        .byte $03, $EA, $AB, $C0     ; ...3322222233...
        .byte $0E, $A5, $5A, $B0     ; ..322211112223..
        .byte $3A, $55, $55, $AC     ; .32211111111223.
        .byte $39, $5F, $F5, $6C     ; .32111333311123.
        .byte $E9, $7E, $AD, $6B     ; 3221133222311223
        .byte $E5, $F9, $6B, $5B     ; 3211332112231123
        .byte $E5, $E9, $7B, $5B     ; 3211322113231123
        .byte $E5, $E9, $7B, $5B     ; 3211322113231123
        .byte $E5, $FA, $AF, $5B     ; 3211332222331123
        .byte $E9, $7F, $FD, $6B     ; 3221133333311223
        .byte $39, $55, $55, $6C     ; .32111111111123.
        .byte $3A, $55, $55, $AC     ; .32211111111223.
        .byte $0E, $A5, $5A, $B0     ; ..322211112223..
        .byte $03, $EA, $AB, $C0     ; ...3322222233...
        .byte $00, $3F, $FC, $00     ; .....333333.....
; the WARDEN, hit: the eye shuts
        .byte $00, $3F, $FC, $00     ; .....333333.....
        .byte $03, $EA, $AB, $C0     ; ...3322222233...
        .byte $0E, $A5, $5A, $B0     ; ..322211112223..
        .byte $3A, $55, $55, $AC     ; .32211111111223.
        .byte $39, $55, $55, $6C     ; .32111111111123.
        .byte $E9, $55, $55, $6B     ; 3221111111111223
        .byte $E5, $55, $55, $5B     ; 3211111111111123
        .byte $E5, $FF, $FF, $5B     ; 3211333333331123
        .byte $E5, $AA, $AA, $5B     ; 3211222222221123
        .byte $E5, $55, $55, $5B     ; 3211111111111123
        .byte $E9, $55, $55, $6B     ; 3221111111111223
        .byte $39, $55, $55, $6C     ; .32111111111123.
        .byte $3A, $55, $55, $AC     ; .32211111111223.
        .byte $0E, $A5, $5A, $B0     ; ..322211112223..
        .byte $03, $EA, $AB, $C0     ; ...3322222233...
        .byte $00, $3F, $FC, $00     ; .....333333.....
; a SHARD: they circle the WARDEN and stop your shots
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $03, $00, $00     ; .......3........
        .byte $00, $0E, $C0, $00     ; ......323.......
        .byte $00, $0E, $C0, $00     ; ......323.......
        .byte $00, $3A, $B0, $00     ; .....32223......
        .byte $00, $39, $B0, $00     ; .....32123......
        .byte $00, $E5, $6C, $00     ; ....3211123.....
        .byte $00, $39, $B0, $00     ; .....32123......
        .byte $00, $3A, $B0, $00     ; .....32223......
        .byte $00, $0E, $C0, $00     ; ......323.......
        .byte $00, $0E, $C0, $00     ; ......323.......
        .byte $00, $03, $00, $00     ; .......3........
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
        .byte $00, $00, $00, $00     ; ................
DYN:    .res 8 * 64         ; I_DYN-I_DYN+7: the HUD's eight, or the title's letters
PROGEND:
        .assert PROGEND <= RAM, "the program runs into the variables at $3600"
        .assert DYN == IMAGES + I_DYN * 64, "the images we draw follow the ones we load"
