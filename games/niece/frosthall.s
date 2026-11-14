; FROSTHALL: a walk round a hall of timber, stone and ice, in three dimensions.
; For the UNC 64 (64K) with the NIECE, the prototype graphics chip marked NV-1 (manual
; chapter 17).
; United Northern Computers' laboratory, 1984. Not for sale.
;
; You stand in the hall and look along it. Cursor keys or W A S D: UP walks on, DOWN
; walks back, LEFT and RIGHT turn. The panel says how many pictures a second you're
; getting, and 1, 2 and 3 choose how the walls are worked out:
;   1  a ray for every column: 80 columns two pixels wide. 12 to 18 pictures a second.
;   2  a ray for every other column, and the ones between filled in (below). As it
;      starts: 16 to 26 a second, and you'd be hard put to tell it from 1.
;   3  40 columns four pixels wide. Coarser: 21 to 32 a second.
;
; There is no 3D in the NIECE. It copies rectangles and it does sums (chapter 17), and
; this program is what they add up to:
;
;   Rays. From where you stand, a ray goes out through a column of the picture and steps
;     from grid line to grid line of the map until it's in a wall (`cast`, `ystep`). How
;     far away the wall is, straight ahead, is how tall it's drawn there. The rays'
;     directions are a table (`rstl`...), each column a small step on from the last, so a
;     ray costs no multiplying to aim.
;   The arithmetic unit does what a 1 MHz processor can't afford: for a ray, two
;     divisions (one over each direction: how far between grid lines), two multiplications
;     (how far to the first one), and when the wall is found two more divisions (how far
;     away, how tall) and a multiplication (where along it). Every one is a few stores
;     and loads.
;   A wall column is one BLIT: one pixel's width of the wall's picture, 64 high, drawn two
;     wide and as tall as the wall, however tall. Near a wall that's hundreds of pixels,
;     most of them off the page; the NIECE cuts those off and takes the right slice.
;   Distance is an ink table: tables 1-3 hold every colour one, two and three steps darker,
;     and a wall takes its table from how tall it is and which way it faces.
;   The things in the hall are one BLIT each, the whole picture at the size its distance
;     gives it, with ZTEST: every wall column wrote its depth into the depth line
;     (ZWRITE; 255 less its height, so taller is nearer), and the NIECE leaves out the
;     columns where a wall is nearer. They're drawn
;     farthest first, so a near one covers a far one.
;   Nothing is rubbed out. Each picture is drawn from nothing on the page that isn't
;     shown, and FLIP shows it when the beam is off the screen.
;
; Time, at 1 MHz: a ray costs the processor 600 cycles before it has taken a step, nearly
; all of it storing and loading the NIECE's registers, and 33 or 36 a step: 800 to 900
; in most of the hall. Eighty of those and the rest come to four frames or more, which is
; why mode 2 doesn't cast them all. On a flat wall, height goes evenly from column to
; column, so where two rays two columns apart met the same flat wall, in the same cell
; or the next, the column between is half way between them, for 190 cycles instead of
; 900 (`wpairs`). Where they met different walls there's an edge between them, and that
; column gets a ray of its own. The NIECE's own time doesn't come into it: the whole
; picture is about 30,000 pixels, 7,500 cycles of drawing spread over 50,000 of sums.
; Walking and turning go by the frame counter, not by the picture, so you move at the
; same speed however long a picture takes.
;
; The pictures, the ink tables and the map at the end of this file are made by
; scripts/frosthall-art.py.

FRAME   = $C006
KEYS    = $C060
NCTRL   = $C100
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
NDEPTH  = $C117
NTABLE  = $C118
NINK    = $C120
NMULA   = $C130
NMULB   = $C132
NPROD   = $C134
NDIVN   = $C138
NDIVD   = $C13C
NQUOT   = $C140
NREM    = $C144
NIECE   = $C14F
CLS     = $FF00
PRSTR   = $FF09
SCREEN  = $0400

FILL    = 1
BLIT    = 2
FLIP    = 4
KEY     = $01               ; NFLAGS
ZTEST   = $08
ZWRITE  = $10

VIEWH   = 84                ; the hall takes the page's top 84 lines; the rest is the panel
HORIZON = 42
HSCALE  = 121 * 256         ; a wall one cell away is 121 pixels tall: as wide as it is
KEYCOL  = 4                 ; the things' see-through colour
MARGIN  = 64                ; how near a wall you can walk: a quarter of a cell
TURN    = 2                 ; 256ths of a circle a frame
FPSAT   = SCREEN + 22 * 40 + 29

; ---- the fast page ----
ANG     = $02               ; which way you face, in 256ths of a circle
PXL     = $03               ; where you are: cells in the high byte, 256ths in the low
PXH     = $04
PYL     = $05
PYH     = $06
RDX     = $08               ; this column's ray: 3 bytes, the top two a number with 8 bits
RDY     = $0B               ;   of fraction, the bottom one keeping the steps exact
RSX     = $0E               ; what the ray changes by from one column to the next
RSY     = $11
DXL     = $14               ; how far along the ray from one grid line to the next,
DXH     = $15               ;   across (X) and down (Y)
DYL     = $16
DYH     = $17
DL      = $18               ; the next line across less the next line down, along the ray
DH      = $19
AXL     = $1A               ; the ray's two parts without their signs
AXH     = $1B
MP      = $1C               ; the map's row the ray is in: a page a row, so its low byte is 0
SCRX    = $20               ; the column being drawn
T0      = $22
T1      = $23
T2      = $24
T3      = $25
LASTF   = $26               ; FRAME when the last picture started
DELTA   = $27               ; frames since then
TACC    = $28               ; frames since the count was shown
FCOUNT  = $29               ; pictures since then
COLW    = $2A               ; a column's width: 2, or 4 in mode 3
PADS    = $2B               ; the keys: bit 0 left, 1 right, 2 on, 3 back
SRC     = $2C               ; upload: where from
VA      = $2E               ;   and where on the sheet
CL      = $30               ; cosine and sine of ANG, 256 for 1
CH      = $31
SL      = $32
SH      = $33
TDXL    = $34               ; a thing: where it is from you
TDXH    = $35
TDYL    = $36
TDYH    = $37
DEPL    = $38               ;   how far ahead
DEPH    = $39
LATL    = $3A               ;   and how far to the right
LATH    = $3B
NXL     = $3C               ; walking: where the step would put you
NXH     = $3D
AYL     = $3E
AYH     = $3F
PYM     = $40               ; the page of the map's row you're in
MODE    = $41               ; 1 a ray a column, 2 a ray every other column, 3 wide columns
RS2X    = $42               ; the ray's step for two columns
RS2Y    = $45
CUR     = $48               ; what this ray found, and (PREV) the one two columns back:
CSIDE   = CUR               ;   0 a wall facing left or right, 32 one facing up or down
CPLANE  = CUR+1             ;   which flat wall: its column of the map, or its row's page
CALONG  = CUR+2             ;   and which cell along it
CHL     = CUR+3             ;   how tall
CHH     = CUR+4
CWL     = CUR+5             ;   where along the wall: 256ths, and the cell
CWH     = CUR+6
CTEX    = CUR+7             ;   its picture
PREV    = $50
PSIDE   = PREV
PPLANE  = PREV+1
PALONG  = PREV+2
PHL     = PREV+3
PHH     = PREV+4
PWL     = PREV+5
PWH     = PREV+6
PTEX    = PREV+7
CFLIP   = $58               ; $FF if the picture is drawn mirrored on this face
NVIS    = $59               ; how many things are on the list to draw

        .org $0800
start:  LDA NIECE
        CMP #$17
        BEQ @chip
        LDX #<nochip        ; no NIECE: say so and stop
        LDY #>nochip
        JSR PRSTR
        HLT
@chip:  JSR CLS
        LDX #159            ; the panel's four rows of text
@text:  LDA panel,X
        STA SCREEN + 21 * 40,X
        DEX
        CPX #$FF
        BNE @text
        JSR upload
        LDA #KEYCOL         ; registers that never change
        STA NKEY
        LDA #64
        STA NSH
        LDA #0
        STA NDIVN+2
        STA NDIVN+3
        STA ANG
        STA TACC
        STA FCOUNT
        LDA #$80            ; the middle of the starting cell
        STA PXL
        STA PYL
        LDA #STARTX
        STA PXH
        LDA #STARTY
        STA PYH
        LDA #2
        STA MODE
        LDA FRAME
        STA LASTF
        LDA #1
        STA NCTRL           ; show the page

; ---- a picture ----
main:   LDA FRAME           ; how long did the last one take?
        TAX
        SEC
        SBC LASTF
        STX LASTF
        CMP #9
        BCC @short
        LDA #8
@short: STA DELTA
        INC FCOUNT
        CLC
        ADC TACC
        STA TACC
        CMP #50             ; a second's worth of frames: show the count
        BCC @count
        SBC #50
        STA TACC
        JSR showfps
@count: JSR keys
        JSR render
        JMP main

; Pictures in the last second, as two digits in the panel. The NIECE divides by ten.
showfps:
        LDA FCOUNT
        STA NDIVN
        LDA #0
        STA NDIVN+1
        STA NDIVD+1
        STA FCOUNT
        LDA #10
        STA NDIVD
        CLC
        LDA NQUOT
        ADC #'0'
        STA FPSAT
        CLC
        LDA NREM
        ADC #'0'
        STA FPSAT+1
        RTS

; ---- the keys, and walking: once for each frame the last picture took ----
keys:   LDA KEYS+8          ; LEFT RIGHT UP DOWN are bits 0-3
        AND #$0F
        STA PADS
        LDA KEYS+4          ; @ A B C D E F G
        AND #$02
        BEQ @a
        LDA #1
        ORA PADS
        STA PADS
@a:     LDA KEYS+4
        AND #$10
        BEQ @d
        LDA #2
        ORA PADS
        STA PADS
@d:     LDA KEYS+6          ; P Q R S T U V W
        BPL @w
        LDA #4
        ORA PADS
        STA PADS
@w:     LDA KEYS+6
        AND #$08
        BEQ @s
        LDA #8
        ORA PADS
        STA PADS
@s:     LDA KEYS+2          ; 0 1 2 3 4 5 6 7
        LSR A
        LDX #1
@mode:  LSR A
        BCC @nomode
        STX MODE
@nomode:
        INX
        CPX #4
        BNE @mode
        LDA DELTA
        BEQ @done
        STA T3
@tick:  LDA PADS
        LSR A
        BCC @left
        DEC ANG
        DEC ANG
@left:  LSR A
        BCC @right
        INC ANG
        INC ANG
@right: LSR A
        BCC @on
        LDA ANG
        JSR walk
@on:    LDA PADS
        AND #$08
        BEQ @back
        LDA ANG
        EOR #$80            ; the other way
        JSR walk
@back:  DEC T3
        BNE @tick
@done:  RTS

; One frame's step in direction A. Across and down are tried on their own, so walking
; into a wall at an angle slides along it. A step is taken if the cell MARGIN ahead of
; it is floor, and has nothing standing in it.
walk:   STA T2
        TAX
        LDY #0
        LDA mvx,X           ; the step across: a signed byte
        BPL @xp
        DEY
@xp:    CLC
        ADC PXL
        STA NXL
        TYA
        ADC PXH
        STA NXH
        LDA mvx,X
        BMI @xm
        CLC
        LDA NXL
        ADC #MARGIN
        LDA NXH
        ADC #0
        JMP @xc
@xm:    SEC
        LDA NXL
        SBC #MARGIN
        LDA NXH
        SBC #0
@xc:    TAY
        CLC
        LDA PYH
        ADC #>walkbuf
        STA MP+1
        LDA (MP),Y
        BNE @xwall
        LDA NXL
        STA PXL
        LDA NXH
        STA PXH
@xwall: SEC
        LDA T2
        SBC #64             ; the sine is the cosine a quarter turn back
        TAX
        LDY #0
        LDA mvx,X
        BPL @yp
        DEY
@yp:    CLC
        ADC PYL
        STA NXL
        TYA
        ADC PYH
        STA NXH
        LDA mvx,X
        BMI @ym
        CLC
        LDA NXL
        ADC #MARGIN
        LDA NXH
        ADC #0
        JMP @yc
@ym:    SEC
        LDA NXL
        SBC #MARGIN
        LDA NXH
        SBC #0
@yc:    CLC
        ADC #>walkbuf
        STA MP+1
        LDY PXH
        LDA (MP),Y
        BNE @ywall
        LDA NXL
        STA PYL
        LDA NXH
        STA PYH
@ywall: RTS

; ---- drawing ----
render: LDA #0              ; the ceiling and the floor: six bands, darker towards
        STA NFLAGS          ;   the far end of the hall
        STA NDX
        STA NDX+1
        STA NDY+1
        STA NDW+1
        STA NDH+1
        LDA #160
        STA NDW
        LDX #0
@band:  LDA bands,X
        STA NDY
        LDA bands+1,X
        STA NDH
        LDA bands+2,X
        STA NCOLOUR
        LDA #FILL
        STA NGO
        INX
        INX
        INX
        CPX #18
        BNE @band

        LDA #1              ; the walls: each column from 1 pixel's width of its picture
        STA NSW
        LDX ANG             ; the leftmost ray, and the step to the next
        LDA rstl,X
        STA RDX
        LDA rstm,X
        STA RDX+1
        LDA rsth,X
        STA RDX+2
        LDA rspl,X
        STA RSX
        ASL A
        STA RS2X
        LDA rspm,X
        STA RSX+1
        ROL A
        STA RS2X+1
        LDA rsph,X
        STA RSX+2
        ROL A
        STA RS2X+2
        TXA
        SEC
        SBC #64             ; down the map is across, a quarter turn back
        TAX
        LDA rstl,X
        STA RDY
        LDA rstm,X
        STA RDY+1
        LDA rsth,X
        STA RDY+2
        LDA rspl,X
        STA RSY
        ASL A
        STA RS2Y
        LDA rspm,X
        STA RSY+1
        ROL A
        STA RS2Y+1
        LDA rsph,X
        STA RSY+2
        ROL A
        STA RS2Y+2
        LDA #0
        STA SCRX
        CLC
        LDA PYH
        ADC #>mapbuf
        STA PYM
        LDA #2
        LDX MODE
        CPX #3
        BNE @width
        LDA #4
@width: STA NDW
        STA COLW
        CPX #2
        BEQ wpairs

; Modes 1 and 3: a ray for every column, 80 of them two wide or 40 of them four wide.
wall1:  JSR cast
        LDA MODE
        CMP #3
        BEQ @wide
        JSR fwd1
        JMP @next
@wide:  JSR fwd2
@next:  CLC
        LDA SCRX
        ADC COLW
        STA SCRX
        CMP #160
        BCC wall1
        JMP things

; Mode 2: a ray for every other column. If it met the same flat wall as the ray two
; columns back, in the same cell or the one beside it, the column between them needs no
; ray: on a flat wall, height goes evenly from column to column, so the one between is
; half way between its neighbours, in height and in where along the wall it is. Where the
; two rays met different walls there's an edge between them, and the column gets its ray.
wpairs: JSR cast            ; column 0
wpair:  JSR fwd2
        CLC
        LDA SCRX
        ADC #4
        STA SCRX
        CMP #160
        BCC @more
        JMP wlast
@more:  JSR cast
        LDA CSIDE
        CMP PSIDE
        BNE @edge
        LDA CPLANE
        CMP PPLANE
        BNE @edge
        SEC
        LDA CALONG
        SBC PALONG
        CLC
        ADC #1
        CMP #3
        BCC @same
@edge:  JMP wedge
@same:  CLC                 ; the height between: half the two together
        LDA PHL
        ADC CHL
        STA T0
        LDA PHH
        ADC CHH
        ROR A
        ROR T0
        STA NDH+1
        LSR A
        STA T1
        LDA T0
        STA NDH
        ROR A
        EOR #$FF
        SEC
        ADC #HORIZON
        STA NDY
        LDA T1
        EOR #$FF
        ADC #0
        STA NDY+1
        CLC                 ; and the place along the wall: cell and 256ths
        LDA PWL
        ADC CWL
        STA T0
        LDA PWH
        ADC CWH
        ROR A
        ROR T0
        LDY CTEX
        CMP CWH             ; in this ray's cell, or the last one's?
        BEQ @mine
        LDY PTEX
        LDA picyl-1,Y
        STA NSY
        LDA picyh-1,Y
        STA NSY+1
@mine:  LDA T0
        EOR CFLIP
        LSR A
        LSR A
        CLC
        ADC picx-1,Y
        STA NSX
        SEC
        LDA SCRX
        SBC #2
        STA NDX
        LDA #BLIT           ; (its depth and its ink table are this ray's, still in
        STA NGO             ;   the registers)
        .irp v, 0, 1, 2, 3, 4, 5, 6, 7
        LDA CUR+v
        STA PREV+v
        .endr
        JMP wpair
wedge:  .irp v, 0, 1, 2, 3, 4, 5, 6, 7
        LDA CUR+v
        STA PREV+v
        .endr
        JSR back1           ; an edge: the column between gets a ray of its own
        DEC SCRX
        DEC SCRX
        JSR cast
        JSR fwd1
        INC SCRX
        INC SCRX
        JMP wpair
wlast:  JSR back1           ; the last column has no neighbour to its right
        LDA #158
        STA SCRX
        JSR cast
        JMP things

; The ray moves on a column, on two, and back one.
fwd1:   CLC
        LDA RDX
        ADC RSX
        STA RDX
        LDA RDX+1
        ADC RSX+1
        STA RDX+1
        LDA RDX+2
        ADC RSX+2
        STA RDX+2
        CLC
        LDA RDY
        ADC RSY
        STA RDY
        LDA RDY+1
        ADC RSY+1
        STA RDY+1
        LDA RDY+2
        ADC RSY+2
        STA RDY+2
        RTS
fwd2:   CLC
        LDA RDX
        ADC RS2X
        STA RDX
        LDA RDX+1
        ADC RS2X+1
        STA RDX+1
        LDA RDX+2
        ADC RS2X+2
        STA RDX+2
        CLC
        LDA RDY
        ADC RS2Y
        STA RDY
        LDA RDY+1
        ADC RS2Y+1
        STA RDY+1
        LDA RDY+2
        ADC RS2Y+2
        STA RDY+2
        RTS
back1:  SEC
        LDA RDX
        SBC RSX
        STA RDX
        LDA RDX+1
        SBC RSX+1
        STA RDX+1
        LDA RDX+2
        SBC RSX+2
        STA RDX+2
        SEC
        LDA RDY
        SBC RSY
        STA RDY
        LDA RDY+1
        SBC RSY+1
        STA RDY+1
        LDA RDY+2
        SBC RSY+2
        STA RDY+2
        RTS

; One column's ray, and its BLIT at column SCRX. The ray is (RDX, RDY), not of length 1:
; its part straight ahead is 1, so "how far along the ray" is already how far away
; straight ahead, which is what makes a flat wall look flat.
cast:   LDA #$FF            ; 65,535 over each direction: how far along the ray from one
        STA NDIVN           ;   grid line to the next
        STA NDIVN+1
        LDA #0
        STA NMULA+1         ; (what's left of a cell is less than 1)
        LDA RDX+2
        BMI rxneg
        STA NDIVD+1
        STA AXH
        LDA RDX+1
        STA NDIVD
        STA AXL
        LDA opiny           ; going right: the map's column goes up
        STA xstep
        LDA PXL
        EOR #$FF            ; what's left of this cell, to the right
        JMP rxset
rxneg:  SEC
        LDA #0
        SBC RDX+1
        STA NDIVD
        STA AXL
        LDA #0
        SBC RDX+2
        STA NDIVD+1
        STA AXH
        LDA opdey
        STA xstep
        LDA PXL             ; what's left of it, to the left
rxset:  STA NMULA
        LDA NQUOT+1
        BPL rxok
        LDA #$FF            ; all but straight up the map: further than the hall is long
        STA DXL
        LDA #$7F
        BNE rxhi
rxok:   LDX NQUOT
        STX DXL
rxhi:   STA DXH
        STA NMULB+1
        LDA DXL
        STA NMULB
        LDA NPROD+1         ; to the first grid line across: that much of a whole step
        STA DL
        LDA NPROD+2
        STA DH

        LDA RDY+2           ; and the same down the map
        BMI ryneg
        STA NDIVD+1
        STA AYH
        LDA RDY+1
        STA NDIVD
        STA AYL
        LDA opinc           ; going down: the map's next row is its next page
        STA ystep
        LDA PYL
        EOR #$FF
        JMP ryset
ryneg:  SEC
        LDA #0
        SBC RDY+1
        STA NDIVD
        STA AYL
        LDA #0
        SBC RDY+2
        STA NDIVD+1
        STA AYH
        LDA opdec
        STA ystep
        LDA PYL
ryset:  STA NMULA
        LDA NQUOT+1
        BPL ryok
        LDA #$FF
        STA DYL
        LDA #$7F
        BNE ryhi
ryok:   LDX NQUOT
        STX DYL
ryhi:   STA DYH
        STA NMULB+1
        LDA DYL
        STA NMULB
        LDA PYM             ; the cell you're in: its row of the map, and its column
        STA MP+1
        LDY PXH
        SEC                 ; D: the first line across less the first line down. Which is
        LDA DL              ;   nearer is its sign, and that's all the stepping needs
        SBC NPROD+1
        STA DL
        LDA DH
        SBC NPROD+2
        STA DH
        BMI xstep

; From grid line to grid line, whichever is nearer along the ray, until the cell is a
; wall. D keeps the difference between the next line down and the next across: a step
; down takes DY off it, a step across adds DX, and its sign says which comes next. This
; loop is where a ray's time goes: 36 cycles for a step down, 33 for one across.
ystep:  INC MP+1            ; (or DEC)
        LDA (MP),Y
        BNE hity
        SEC
        LDA DL
        SBC DYL
        STA DL
        LDA DH
        SBC DYH
        STA DH
        BPL ystep
xstep:  INY                 ; (or DEY)
        LDA (MP),Y
        BNE xhit
        CLC
        LDA DL
        ADC DXL
        STA DL
        LDA DH
        ADC DXH
        STA DH
        BMI xstep
        BPL ystep
xhit:   JMP hitx

; A wall, met from above or below. How far away, straight ahead: how far down the map its
; face is from you, over the ray's part down the map. Then where along the wall that is:
; where you are across, and that far along the ray's part across.
hity:   STA CTEX
        LDA RDY+2
        BMI hyup
        SEC                 ; its near face is the top of its row
        LDA #0
        SBC PYL
        STA NDIVN+1
        LDA MP+1
        SBC PYM
        JMP hyset
hyup:   LDA PYL             ; its near face is the bottom of its row
        STA NDIVN+1
        CLC
        LDA PYM
        SBC MP+1
hyset:  STA NDIVN+2
        LDA #0
        STA NDIVN
        LDA AYL
        STA NDIVD
        LDA AYH
        STA NDIVD+1
        LDA NQUOT           ; (both bytes read before NDIVD changes under them)
        LDX NQUOT+1
        STA NMULA
        STA NDIVD
        STX NMULA+1
        STX NDIVD+1
        LDA RDX+1
        STA NMULB
        LDA RDX+2
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC PXL
        STA CWL
        LDA NPROD+2
        ADC PXH
        STA CWH
        LDA MP+1            ; which flat wall, and which cell along it
        STA CPLANE
        STY CALONG
        LDX #0
        LDA RDY+2
        BPL hyok
        DEX                 ; seen from below: the picture the right way round
hyok:   STX CFLIP
        LDX #32             ; these walls are a step darker
        JMP wall

hitx:   STA CTEX            ; a wall, met from the left or the right: the same, turned
        LDA RDX+2
        BMI hxleft
        SEC
        LDA #0
        SBC PXL
        STA NDIVN+1
        TYA
        SBC PXH
        JMP hxset
hxleft: LDA PXL
        STA NDIVN+1
        STY T0
        CLC
        LDA PXH
        SBC T0
hxset:  STA NDIVN+2
        LDA #0
        STA NDIVN
        LDA AXL
        STA NDIVD
        LDA AXH
        STA NDIVD+1
        LDA NQUOT           ; (both bytes read before NDIVD changes under them)
        LDX NQUOT+1
        STA NMULA
        STA NDIVD
        STX NMULA+1
        STX NDIVD+1
        LDA RDY+1
        STA NMULB
        LDA RDY+2
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC PYL
        STA CWL
        LDA NPROD+2
        ADC PYH
        STA CWH
        STY CPLANE
        LDA MP+1
        STA CALONG
        LDX #$FF
        LDA RDX+2
        BPL hxok
        INX
hxok:   STX CFLIP
        LDX #0

; X is 0, or 32 for a wall facing up or down the map. NDIVD already holds how far away.
wall:   STX CSIDE
        LDA CWL             ; which of the picture's 64 columns
        EOR CFLIP
        LSR A
        LSR A
        LDY CTEX
        CLC
        ADC picx-1,Y
        STA NSX
        LDA picyl-1,Y
        STA NSY
        LDA picyh-1,Y
        STA NSY+1
        LDA #>HSCALE        ; how tall: HSCALE over how far (its low byte is 0, as
        STA NDIVN+1         ;   NDIVN's is now)
        LDA #0
        STA NDIVN+2
        LDA NQUOT
        STA NDH
        STA CHL
        LDX NQUOT+1
        STX NDH+1
        STX CHH
        BNE wbig            ; the depth line's number, from the height: 255 less it, so
        CMP #255            ;   taller is nearer; and the ink table, from the same
        BCC wsmall
wbig:   LDA #254
wsmall: TAY
        EOR #$FF
        STA NDEPTH
        CPY #32
        BCC wtable
        LDY #31
wtable: TYA
        ORA CSIDE
        TAY
        LDA wflags,Y
        STA NFLAGS
        TXA                 ; half of it above the horizon: its top is HORIZON - half,
        LSR A               ;   often far above the page
        STA T0
        LDA CHL
        ROR A
        EOR #$FF
        SEC
        ADC #HORIZON
        STA NDY
        LDA T0
        EOR #$FF
        ADC #0
        STA NDY+1
        LDA SCRX
        STA NDX
        LDA #BLIT           ; (the queue can't fill: a column takes the NIECE 54 cycles
        STA NGO             ;   at most, and a ray many times that)
        RTS

; ---- the things in the hall ----
; For each: how far ahead of you it is and how far to the right, from where it is and
; the cosine and sine of the way you face. Two multiplications say how far ahead; the
; things behind you stop there, and the rest take two more.
things: LDA #64             ; the whole picture this time
        STA NSW
        LDA #>HSCALE
        STA NDIVN+1
        LDA #0
        STA NDIVN
        STA NDIVN+2
        LDX ANG
        LDA cosl,X
        STA CL
        LDA cosh,X
        STA CH
        TXA
        SEC
        SBC #64
        TAX
        LDA cosl,X
        STA SL
        LDA cosh,X
        STA SH
        LDA #0
        STA NVIS
        LDX #THINGS-1
tloop:  SEC                 ; it stands in the middle of its cell
        LDA #$80
        SBC PXL
        STA TDXL
        STA NMULB
        LDA thingx,X
        SBC PXH
        STA TDXH
        STA NMULB+1
        SEC
        LDA #$80
        SBC PYL
        STA TDYL
        LDA thingy,X
        SBC PYH
        STA TDYH
        LDA CL              ; ahead = across * cosine + down * sine
        STA NMULA
        LDA CH
        STA NMULA+1
        LDA NPROD+1
        STA DEPL
        LDA NPROD+2
        STA DEPH
        LDA SL
        STA NMULA
        LDA SH
        STA NMULA+1
        LDA TDYL
        STA NMULB
        LDA TDYH
        STA NMULB+1
        CLC
        LDA DEPL
        ADC NPROD+1
        STA DEPL
        STA NDIVD
        LDA DEPH
        ADC NPROD+2
        STA NDIVD+1
        BMI tnot            ; behind you, or too near to see round: not drawn
        BNE tsize
        LDA DEPL
        CMP #48
        BCS tsize
tnot:   JMP tskip
tsize:  LDA TDXL            ; right = down * cosine - across * sine
        STA NMULB
        LDA TDXH
        STA NMULB+1
        LDA NPROD+1
        STA LATL
        LDA NPROD+2
        STA LATH
        LDA CL
        STA NMULA
        LDA CH
        STA NMULA+1
        LDA TDYL
        STA NMULB
        LDA TDYH
        STA NMULB+1
        SEC                 ; (both bytes worked out before NMULA changes: NPROD is
        LDA NPROD+1         ;   always the answer for what the registers hold now)
        SBC LATL
        TAY
        LDA NPROD+2
        SBC LATH
        STY NMULA
        STA NMULA+1
        LDA NQUOT           ; its size on the page: a wall's height at that distance
        STA NMULB
        STA thl,X
        LDA NQUOT+1
        STA NMULB+1
        STA thh,X
        LSR A
        STA T0              ; half its size
        LDA thl,X
        ROR A
        STA T1
        CLC                 ; its middle: 80 + right * size / 256
        LDA NPROD+1
        ADC #80
        STA T2
        LDA NPROD+2
        ADC #0
        STA T3
        SEC                 ; its left edge
        LDA T2
        SBC T1
        STA txl,X
        LDA T3
        SBC T0
        STA txh,X
        BMI tleft
        BNE tskip           ; 256 or more: off the right
        LDA txl,X
        CMP #160
        BCS tskip
        BCC tnear
tleft:  CLC                 ; off the left unless its right edge is on the page
        LDA txl,X
        ADC thl,X
        LDA txh,X
        ADC thh,X
        BMI tskip
tnear:  LDA thh,X           ; how near, 1-254, from its size like the walls' depth
        BNE tbig
        LDA thl,X
        CMP #254
        BCC tset
tbig:   LDA #253
tset:   CLC
        ADC #1
        STA td,X
        LDY NVIS            ; onto the list of the ones to draw
        TXA
        STA vis,Y
        INC NVIS
tskip:  DEX
        BMI tdraw
        JMP tloop

; Farthest first, until none is left: each one BLIT, see-through where the picture is
; KEYCOL, and only in the columns where no wall is nearer.
tdraw:  LDY NVIS
        BNE @some
        JMP panelf
@some:  DEY
        LDA #$FF
        STA T0
@far:   LDX vis,Y
        LDA td,X
        CMP T0
        BCS @next
        STA T0
        STY T1
@next:  DEY
        BPL @far
        LDY T1              ; off the list: the last one takes its place
        LDX vis,Y
        STX T2
        DEC NVIS
        LDX NVIS
        LDA vis,X
        STA vis,Y
        LDX T2
        LDA txl,X
        STA NDX
        LDA txh,X
        STA NDX+1
        LDA thl,X
        STA NDW
        STA NDH
        LDA thh,X
        STA NDW+1
        STA NDH+1
        LSR A
        STA T3
        LDA thl,X
        ROR A
        EOR #$FF
        SEC
        ADC #HORIZON
        STA NDY
        LDA T3
        EOR #$FF
        ADC #0
        STA NDY+1
        LDY thingp,X
        LDA picx,Y
        STA NSX
        LDA picyl,Y
        STA NSY
        LDA picyh,Y
        STA NSY+1
        SEC                 ; its depth: 256 less how near, so 2 at its biggest
        LDA #0
        SBC T0
        STA NDEPTH
        LDY T0
        DEY
        CPY #32
        BCC @table
        LDY #31
@table: LDA tflags,Y
        STA NFLAGS
@full:  BIT NSTATUS
        BVS @full
        LDA #BLIT
        STA NGO
        JMP tdraw

; The panel under the picture: black, over whatever hung down into it. Then show it.
panelf: LDA #0
        STA NFLAGS
        STA NDX
        STA NDX+1
        STA NDY+1
        STA NDW+1
        STA NDH+1
        STA NCOLOUR
        LDA #VIEWH
        STA NDY
        LDA #160
        STA NDW
        LDA #100 - VIEWH
        STA NDH
@full:  BIT NSTATUS
        BVS @full
        LDA #FILL
        STA NGO
@full2: BIT NSTATUS
        BVS @full2
        LDA #FLIP           ; when the beam has left the picture; what's drawn next
        STA NGO             ;   waits behind it in the queue, for the other page
        RTS

; The instructions `xstep` and `ystep` are made of, to copy their first bytes from.
opiny:  INY
opdey:  DEY
opinc:  INC MP+1
opdec:  DEC MP+1

; ---- start-up: the pictures onto the sheet, the ink tables into the chip ----
; Picture p goes at (64 * (p AND 3), 200 + 64 * (p / 4)): four across, under the pages.
upload: LDA #<pictures
        STA SRC
        LDA #>pictures
        STA SRC+1
        LDX #0
@pic:   LDA picvl,X
        STA VA
        LDA picvh,X
        STA VA+1
        LDA #64
        STA T0
@row:   LDA VA
        STA NVADDR
        LDA VA+1
        STA NVADDR+1
        LDY #0
@byte:  LDA (SRC),Y
        STA NVDATA
        INY
        CPY #32
        BNE @byte
        CLC
        LDA SRC
        ADC #32
        STA SRC
        BCC @src
        INC SRC+1
@src:   CLC
        LDA VA
        ADC #128            ; the sheet's next line
        STA VA
        BCC @va
        INC VA+1
@va:    DEC T0
        BNE @row
        INX
        CPX #PICTURES
        BNE @pic
        LDA #<map           ; the map, a page a row: a step down it is one INC
        STA SRC
        LDA #>map
        STA SRC+1
        LDA #0
        STA MP
        LDA #>mapbuf
        STA MP+1
        LDX #MAPROWS
@maprow:
        LDY #31
@cell:  LDA (SRC),Y
        STA (MP),Y
        DEY
        BPL @cell
        CLC                 ; and again for walking, where the things are in the way too
        LDA MP+1
        ADC #MAPROWS
        STA MP+1
        LDY #31
@cell2: LDA (SRC),Y
        STA (MP),Y
        DEY
        BPL @cell2
        SEC
        LDA MP+1
        SBC #MAPROWS
        STA MP+1
        CLC
        LDA SRC
        ADC #32
        STA SRC
        BCC @msrc
        INC SRC+1
@msrc:  INC MP+1
        DEX
        BNE @maprow
        LDX #THINGS-1       ; a thing's cell can't be walked into
@thing: CLC
        LDA thingy,X
        ADC #>walkbuf
        STA MP+1
        LDY thingx,X
        LDA #1
        STA (MP),Y
        DEX
        BPL @thing
        LDX #0              ; tables 1-3: darker, darker still, darkest
        LDY #1
@table: STY NTABLE
        LDA #16
        STA T0
        STY T1
        LDY #0
@ink:   LDA inks,X
        STA NINK,Y
        INX
        INY
        CPY #16
        BNE @ink
        LDY T1
        INY
        CPY #4
        BNE @table
        RTS

nochip: .asciiz "FROSTHALL NEEDS THE NIECE."

panel:  .text "                                        "
        .text "  FROSTHALL                  -- PICTURES"
        .text "  ARROWS OR W A S D WALK        A SECOND"
        .text "  1 EVERY COLUMN  2 HALF  3 WIDE        "
        .assert * - panel == 160, "the panel is four rows of 40"

; The ceiling and the floor: where each band starts, how tall it is, its colour.
bands:  .byte 0, 14, 9
        .byte 14, 14, 11
        .byte 28, 14, 0
        .byte 42, 14, 0
        .byte 56, 14, 11
        .byte 70, 14, 12

; Where each picture is on the sheet, and where its first byte is.
picx:   .rept 8, p
        .byte [p & 3] * 64
        .endr
picyl:  .rept 8, p
        .byte <[200 + [p >> 2] * 64]
        .endr
picyh:  .rept 8, p
        .byte >[200 + [p >> 2] * 64]
        .endr
picvl:  .rept 8, p
        .byte <[[200 + [p >> 2] * 64] * 128 + [p & 3] * 32]
        .endr
picvh:  .rept 8, p
        .byte >[[200 + [p >> 2] * 64] * 128 + [p & 3] * 32]
        .endr

; A wall's flags by how tall it is (0-31, 31 for anything taller), then the same for the
; darker side: ZWRITE, and ink table 0 for 20 and up (six cells away or less), 1 down to
; 9 (thirteen cells), 2 beyond; the darker side one more.
wflags: .rept 64, i
        .byte ZWRITE | [[[i & 31] < 20] + [[i & 31] < 9] + [i >> 5]] << 6
        .endr
; A thing's, by its size.
tflags: .rept 32, i
        .byte KEY | ZTEST | [[i < 20] + [i < 9]] << 6
        .endr

; The ray through the leftmost column for each way you can face, 65,536 for 1: straight
; ahead, less the picture's half width to the right (0.66 of a cell at one cell away). And
; the step from a column to the next: a fortieth of that half width.
rstl:   .rept 256, i
        .byte <[cos(i, 65536) + 40 * sin(i, 1081)]
        .endr
rstm:   .rept 256, i
        .byte <[[[cos(i, 65536) + 40 * sin(i, 1081)] & $FFFFFF] >> 8]
        .endr
rsth:   .rept 256, i
        .byte <[[[cos(i, 65536) + 40 * sin(i, 1081)] & $FFFFFF] >> 16]
        .endr
rspl:   .rept 256, i
        .byte <[-sin(i, 1081)]
        .endr
rspm:   .rept 256, i
        .byte <[[[-sin(i, 1081)] & $FFFFFF] >> 8]
        .endr
rsph:   .rept 256, i
        .byte <[[[-sin(i, 1081)] & $FFFFFF] >> 16]
        .endr
; Cosines, 256 for 1 (the sine is the cosine a quarter turn back), and a frame's step.
cosl:   .rept 256, i
        .byte <[cos(i, 256)]
        .endr
cosh:   .rept 256, i
        .byte <[[cos(i, 256) & $FFFF] >> 8]
        .endr
mvx:    .rept 256, i
        .byte <[cos(i, 14)]
        .endr

; ---- BEGIN DATA: made by scripts/frosthall-art.py, not by hand ----
; The pictures: 64 x 64 each, a digit a pixel, 32 bytes a line. Walls first, in the order
; of the map's numbers, then the things. Colour 4 is the things' see-through colour.
PICTURES = 7
pictures:
; timber
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$89,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $08,$9B,$99,$99,$98,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$98,$99,$BB,$B9,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$98,$99,$99,$98,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$9B,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$98,$99,$99,$98,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$9B,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$98,$99,$99,$98,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$9B,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B
        .byte $08,$98,$99,$99,$98,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$9B,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$98,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$9B,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$98,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$9B,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$98,$99,$9B,$BB,$9B
        .byte $08,$99,$99,$99,$99,$98,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$9B,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$9B,$99,$9B,$0B,$9B
        .byte $08,$99,$99,$99,$99,$98,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$98,$98,$99,$9B,$08,$99,$99,$9B,$99,$9B,$BB,$9B
        .byte $08,$99,$99,$99,$99,$98,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$98,$99,$99,$9B,$98,$99,$9B,$08,$99,$99,$99,$99,$9B,$BB,$9B
        .byte $08,$99,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$98,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$98,$9B,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$9B,$BB,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$9B,$9B,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$9B,$0B,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$98,$9B,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$9B,$BB,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$98,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$9B,$BB,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$98,$98,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$98,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$9B,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$98,$99,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$9B,$99,$9B
        .byte $08,$99,$98,$99,$99,$99,$99,$9B,$08,$98,$99,$98,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$98,$99,$9B
        .byte $77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88,$88,$88,$88,$8F,$B8,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$9B,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$B9,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$98,$9B,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$B9,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$B9,$99,$99,$99,$9B,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$9B,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$9B,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
        .byte $08,$99,$99,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$98,$99,$99,$99,$99,$9B,$08,$99,$99,$99,$99,$99,$98,$9B
; stone
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$01,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$0F,$FF,$FF,$FF
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CF,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CF
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FC,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FC,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$BC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CB,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FC,$FF,$FB,$01,$FF,$FF,$FF,$CF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FC,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$FC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $01,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$01,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$0F,$FF,$FF,$FF
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FB,$0F,$CC,$CC,$BC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FC,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CF,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FB,$FF,$FF,$FB,$FF,$FF,$BF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$BF,$FB,$01,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$CF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FF,$FF,$FF,$FF,$FF,$FF,$BF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC
        .byte $0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$01,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$BC,$CC,$CC,$CC,$CC
        .byte $CC,$BC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$FC,$CF,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$BC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$BC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$FC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FC,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$FC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$BF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CB
        .byte $CC,$CC,$FC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$FC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CB,$CC,$CC,$CC,$CF,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CF,$CF,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CF,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CB,$BC,$CC,$CC,$CB,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$BB,$CB,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$BC,$CB,$0F,$CC,$CF,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$BC,$CC,$CF,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CF,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CB,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$BC,$FC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$FC,$CC,$CC,$CC,$CC,$CB,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB
; ice
        .byte $66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66
        .byte $63,$33,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$63,$33,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13
        .byte $63,$33,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$63,$33,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E
        .byte $63,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$63,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE
        .byte $63,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$31,$3E,$EE,$63,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$31,$3E,$EE
        .byte $63,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$61,$EE,$E3,$13,$EE,$EE,$63,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$E3,$13,$EE,$EE
        .byte $63,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$31,$3E,$EE,$EE,$63,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$31,$3E,$EE,$EE
        .byte $61,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$13,$13,$EE,$EE,$EE,$61,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$61,$EE,$E3,$13,$EE,$EE,$EE
        .byte $61,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$61,$3E,$EE,$EE,$EE,$61,$33,$33,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$31,$3E,$EE,$EE,$EE
        .byte $63,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$16,$1E,$EE,$EE,$EE,$63,$33,$3E,$EE,$EE,$EE,$EE,$EE,$61,$EE,$E3,$13,$EE,$EE,$EE,$EE
        .byte $63,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$36,$1E,$EE,$EE,$EE,$63,$33,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$31,$3E,$EE,$EE,$EE,$EE
        .byte $63,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$61,$EE,$EE,$EE,$EE,$63,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$13,$13,$EE,$EE,$EE,$EE,$EE
        .byte $63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$36,$1E,$EE,$EE,$EE,$EE,$63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$11,$3E,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$E6,$1E,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$13,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$61,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$16,$1E,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$61,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$61,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$EE
        .byte $6E,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$E6,$6E,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$E6
        .byte $6E,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$66,$6E,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$66
        .byte $6E,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$E6,$66,$6E,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$E6,$66
        .byte $6E,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$66,$66,$6E,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$66,$66
        .byte $6E,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$6E,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66
        .byte $6E,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$6E,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66
        .byte $6E,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66,$6E,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66
        .byte $63,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66,$63,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66
        .byte $61,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66,$66,$61,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66,$66
        .byte $63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66,$66,$63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66,$66
        .byte $66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66,$66
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$63,$33,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$63,$33,$33,$31,$13,$33,$3E,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$63,$33,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$63,$33,$33,$11,$33,$33,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$63,$33,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$63,$33,$31,$13,$33,$3E,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$63,$33,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$31,$3E,$EE,$63,$33,$11,$33,$33,$E6,$1E,$EE
        .byte $EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$63,$31,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$61,$E3,$13,$EE,$EE,$63,$31,$13,$33,$3E,$EE,$61,$EE
        .byte $EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$63,$11,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$31,$3E,$EE,$EE,$63,$11,$33,$33,$EE,$EE,$61,$EE
        .byte $EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$61,$13,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$13,$13,$EE,$EE,$EE,$61,$13,$33,$3E,$EE,$E6,$1E,$EE
        .byte $EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$61,$33,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$61,$3E,$EE,$EE,$EE,$61,$33,$33,$EE,$EE,$61,$EE,$EE
        .byte $EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$63,$33,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$13,$EE,$EE,$EE,$EE,$63,$33,$3E,$EE,$E6,$1E,$EE,$EE
        .byte $EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$63,$33,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$36,$1E,$EE,$EE,$EE,$EE,$63,$33,$EE,$EE,$61,$EE,$EE,$EE
        .byte $EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$63,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$61,$EE,$EE,$EE,$EE,$63,$3E,$EE,$EE,$61,$EE,$EE,$EE
        .byte $EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$E6,$1E,$EE,$EE,$EE,$63,$EE,$EE,$EE,$61,$EE,$EE,$EE
        .byte $E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$E6,$1E,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E6,$1E,$EE,$EE
        .byte $31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E6,$1E,$EE,$EE
        .byte $13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$61,$EE,$EE,$E3
        .byte $3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$61,$EE,$EE,$31
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E6,$1E,$E3,$13
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$61,$EE,$31,$3E
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E6,$13,$13,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$61,$31,$3E,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$EE,$6E,$EE,$EE,$EE,$E6,$13,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$6E,$EE,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$E6,$6E,$EE,$EE,$EE,$36,$1E,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$6E,$EE,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$61,$EE,$EE,$EE,$66,$6E,$EE,$EE,$E3,$16,$1E,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$6E,$EE,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$E6,$66,$6E,$EE,$EE,$31,$36,$1E,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$6E,$EE,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$1E,$EE,$EE,$66,$66,$6E,$EE,$E3,$13,$EE,$61,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$6E,$EE,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$6E,$EE,$31,$3E,$EE,$EE,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$EE,$66,$66,$66,$6E,$E3,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$6E,$E3,$13,$EE,$EE,$EE,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$E6,$66,$66,$66,$6E,$31,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66,$6E,$31,$3E,$EE,$EE,$EE,$EE,$EE
        .byte $EE,$EE,$EE,$EE,$66,$66,$66,$66,$63,$13,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66,$63,$13,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $EE,$EE,$EE,$E6,$66,$66,$66,$66,$61,$3E,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$E6,$66,$66,$66,$66,$61,$3E,$EE,$EE,$EE,$EE,$EE,$EE
        .byte $EE,$EE,$EE,$66,$66,$66,$66,$66,$63,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$EE,$66,$66,$66,$66,$66,$63,$EE,$EE,$EE,$EE,$EE,$EE,$EE
; banner
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$11,$1B,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$77,$FC,$FF,$FF,$BF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$77,$CC,$CC,$CC,$CF,$CC
        .byte $FF,$FF,$BF,$FF,$FF,$77,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$77,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FC,$FF,$FF,$77,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$77,$CC,$BC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$BF,$77,$FF,$FF,$A7,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$AA,$7A,$0F,$CC,$77,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CB,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$BC,$CC,$CC
        .byte $FF,$CF,$FF,$FF,$BF,$FF,$CF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $BF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$A7,$22,$22,$22,$22,$22,$22,$27,$72,$22,$22,$22,$22,$22,$22,$7A,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$A7,$22,$22,$22,$22,$22,$22,$27,$72,$22,$22,$22,$22,$22,$22,$7A,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $FF,$FF,$FF,$FB,$0F,$FF,$FF,$FF,$A7,$22,$22,$22,$27,$22,$22,$27,$72,$22,$22,$72,$22,$22,$22,$7A,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$77,$72,$22,$27,$72,$22,$27,$77,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$27,$77,$22,$77,$77,$22,$77,$72,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$77,$77,$77,$77,$77,$77,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$27,$77,$77,$77,$77,$72,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$27,$78,$87,$78,$87,$72,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$77,$78,$87,$78,$87,$77,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$22,$22,$7A,$FC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CB,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$77,$77,$77,$77,$77,$77,$77,$77,$77,$77,$22,$22,$7A,$CC,$BC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$77,$77,$77,$77,$77,$77,$22,$22,$22,$22,$7A,$CC,$BC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$27,$78,$88,$88,$87,$72,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CB,$CC,$A7,$22,$22,$22,$22,$27,$77,$77,$77,$77,$72,$22,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CF,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$22,$77,$77,$77,$77,$77,$77,$22,$22,$22,$22,$7A,$CC,$CC,$FC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$A7,$22,$22,$22,$27,$77,$22,$77,$77,$22,$77,$72,$22,$22,$22,$7A,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$A7,$22,$22,$22,$77,$72,$22,$27,$72,$22,$27,$77,$22,$22,$22,$7A,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$A7,$22,$22,$22,$27,$22,$22,$27,$72,$22,$22,$72,$22,$22,$22,$7A,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$A7,$22,$22,$22,$22,$22,$22,$27,$72,$22,$22,$22,$22,$22,$22,$7A,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$27,$72,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$BC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$BC,$BC,$CC,$CC,$CC
        .byte $CF,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CF
        .byte $CC,$CC,$CC,$CC,$CC,$CB,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CB,$CC,$CC,$CC
        .byte $CB,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$BC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$A7,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$A7,$22,$22,$22,$22,$22,$22,$2B,$B2,$22,$22,$22,$22,$22,$22,$7A,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB
        .byte $00,$00,$00,$00,$00,$00,$00,$00,$A7,$22,$22,$22,$22,$22,$22,$00,$00,$22,$22,$22,$22,$22,$22,$7A,$00,$00,$00,$00,$00,$00,$00,$00
        .byte $01,$11,$11,$11,$11,$11,$11,$11,$A7,$22,$22,$22,$22,$22,$2F,$FF,$FF,$F2,$22,$22,$22,$22,$22,$7A,$0F,$FF,$FF,$FF,$FF,$FF,$FF,$FF
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$22,$CC,$CC,$CC,$CC,$22,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$2C,$CC,$CC,$CC,$CC,$C2,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$FC,$CC,$CF,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$22,$CC,$CC,$CC,$CC,$CC,$CC,$22,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$CF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$2F,$CC,$CC,$CC,$CC,$CB,$CC,$C2,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$22,$0F,$CC,$CC,$CB,$CB,$CC,$CC,$CC,$22,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$A7,$22,$22,$2B,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$C2,$22,$22,$7A,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$CF,$FB,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FC,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$BC,$CC,$CC,$CC,$FC
        .byte $01,$FF,$FC,$FF,$FF,$FF,$FC,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CB,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$BC,$CC,$CC,$CC,$CC
        .byte $01,$FB,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $01,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CC,$CB,$0F,$CC,$CC,$CC,$CC,$CC,$CC,$CC
        .byte $0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$0B,$BB,$BB,$BB,$BB,$BB,$BB,$BB
; barrel
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$88,$88,$88,$88,$88,$88,$88,$88,$88,$88,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$99,$99,$99,$99,$99,$99,$99,$99,$99,$99,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$4C,$CC,$CC,$CC,$CC,$CF,$FF,$FF,$FF,$FF,$FC,$CC,$CC,$CC,$CC,$C4,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$4C,$CC,$CC,$CC,$CC,$CF,$FF,$FF,$FF,$FF,$FC,$CC,$CC,$CC,$CC,$C4,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$98,$8B,$87,$77,$B8,$88,$9B,$99,$99,$B9,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$98,$8B,$87,$77,$B8,$88,$9B,$99,$99,$B9,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$99,$88,$B8,$77,$7B,$88,$88,$B9,$99,$9B,$99,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$98,$8B,$87,$77,$B8,$88,$9B,$99,$99,$B9,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$98,$8B,$87,$77,$B8,$88,$9B,$99,$99,$B9,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$4C,$CC,$CC,$CC,$CC,$CF,$FF,$FF,$FF,$FF,$FC,$CC,$CC,$CC,$CC,$C4,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$4C,$CC,$CC,$CC,$CC,$CF,$FF,$FF,$FF,$FF,$FC,$CC,$CC,$CC,$CC,$C4,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$9B,$98,$88,$B7,$77,$8B,$88,$99,$B9,$99,$90,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$88,$8B,$77,$88,$B8,$99,$9B,$99,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$40,$00,$09,$B9,$88,$8B,$77,$88,$B8,$99,$9B,$99,$00,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
; brazier
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$74,$44,$74,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$74,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$82,$44,$44,$47,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$88,$24,$44,$47,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$74,$22,$88,$77,$77,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$72,$88,$77,$77,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$77,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$77,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$77,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$87,$77,$88,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$78,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$22,$88,$82,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$42,$28,$24,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$FF,$FF,$FF,$FF,$FF,$F2,$28,$2F,$FF,$FF,$FF,$FF,$FF,$FF,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$FF,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$FC,$CC,$CC,$CC,$CC,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$4F,$CC,$CC,$CC,$CC,$BB,$BB,$BB,$BB,$BB,$BB,$B4,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$FC,$CC,$CC,$CC,$BB,$BB,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$4F,$CC,$CC,$CC,$BB,$BB,$BB,$BB,$BB,$B4,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$FC,$CC,$CC,$BB,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$4F,$CC,$CC,$BB,$BB,$BB,$BB,$B4,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$FC,$CC,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$4F,$CC,$BB,$BB,$BB,$B4,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BC,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$BB,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
; crystal
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$41,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$11,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$11,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$11,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$41,$E0,$44,$44,$44,$41,$13,$EE,$EE,$EE,$60,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$43,$E0,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$43,$13,$EE,$EE,$EE,$60,$44,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$41,$13,$EE,$EE,$EE,$60,$44,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$13,$13,$EE,$EE,$EE,$66,$04,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$13,$EE,$04,$44,$44,$13,$13,$EE,$EE,$EE,$66,$04,$44,$44,$41,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$33,$EE,$04,$44,$44,$33,$13,$EE,$EE,$EE,$66,$04,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$33,$13,$EE,$EE,$EE,$66,$04,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$33,$13,$EE,$EE,$EE,$66,$04,$44,$44,$43,$E0,$44,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$43,$1E,$EE,$60,$44,$44,$33,$13,$EE,$EE,$EE,$66,$04,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$41,$1E,$EE,$60,$44,$44,$13,$13,$EE,$EE,$EE,$66,$04,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$41,$1E,$EE,$60,$44,$41,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$41,$1E,$EE,$60,$44,$41,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$44,$13,$EE,$04,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$43,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$44,$33,$EE,$04,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$43,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$43,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$31,$3E,$EE,$E6,$04,$43,$31,$3E,$EE,$EE,$EE,$E6,$60,$44,$43,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$11,$3E,$EE,$E6,$04,$13,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$41,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$41,$13,$EE,$EE,$EE,$60,$13,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$41,$1E,$EE,$60,$44,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$41,$13,$EE,$EE,$EE,$60,$13,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$11,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$33,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$33,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$43,$13,$EE,$EE,$EE,$60,$33,$13,$3E,$EE,$EE,$EE,$EE,$66,$04,$31,$3E,$EE,$E6,$04,$44,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$00,$44,$44,$44,$44,$44
        .byte $44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44,$44
; The ink tables 1-3: every colour one, two and three steps darker.
inks:
        .byte 0,15,9,14,4,11,11,8,9,11,2,0,11,5,6,12
        .byte 0,12,11,6,4,0,0,9,11,0,9,0,0,11,11,11
        .byte 0,11,0,11,4,0,0,11,0,0,11,0,0,0,0,0
; The hall: 24 rows of 32. 0 is floor; 1-4 a wall, and which picture it wears.
MAPROWS = 24
map:
        .byte 1,1,1,1,1,1,1,1,1,1,1,1,2,2,2,2,2,2,2,2,2,2,2,2,3,3,3,3,3,3,3,3    ; 11111111111122222222222233333333
        .byte 1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,3    ; 1..........1....b.....2........3
        .byte 1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,3    ; 1..P.......1..........2...c....3
        .byte 1,0,0,0,0,0,0,0,0,0,0,4,0,0,0,0,2,2,0,0,0,0,2,0,0,0,0,0,0,0,0,3    ; 1..........4....22....2........3
        .byte 1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,2,2,0,0,0,0,0,0,0,0,0,3,3,0,0,3    ; 1....b.....1....22.........33..3
        .byte 1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,3,0,0,0,0,3,3,0,0,3    ; 1..........1..........3....33..3
        .byte 1,1,1,1,0,0,1,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,0,3    ; 1111..111111..........3........3
        .byte 2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,3,0,0,0,0,0,0,0,0,3    ; 2.........f.......2...3..c.....3
        .byte 2,0,0,2,2,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,3,3,3,3,3,0,0,3,3,3    ; 2..22.............2...33333..333
        .byte 2,0,0,2,2,0,0,0,0,0,2,2,2,2,0,0,2,2,0,0,0,0,0,0,0,0,3,0,0,0,0,3    ; 2..22.....2222..22........3....3
        .byte 2,0,0,0,0,0,0,0,0,0,2,0,0,2,0,0,2,2,0,0,0,0,0,0,0,0,3,0,0,0,0,3    ; 2.........2..2..22..b.....3....3
        .byte 2,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,4,0,0,0,0,3    ; 2.....b......2............4..f.3
        .byte 2,0,0,2,2,2,2,2,2,2,0,0,0,2,0,0,0,0,1,1,1,1,1,0,0,0,3,0,0,0,0,3    ; 2..2222222...2....11111...3....3
        .byte 2,0,0,0,0,0,0,0,0,2,0,0,0,2,0,0,0,0,1,0,0,0,1,0,0,0,3,0,0,0,0,3    ; 2........2...2....1...1...3....3
        .byte 2,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,3,3,0,0,3,3    ; 2.....f..2........1.c.....33..33
        .byte 2,0,0,0,0,0,0,0,0,2,0,0,0,2,0,0,0,0,1,0,0,0,1,0,0,0,0,0,0,0,0,3    ; 2........2...2....1...1........3
        .byte 2,2,2,2,2,0,0,2,2,2,2,2,2,2,0,0,0,0,1,1,1,1,1,0,0,0,3,3,3,3,3,3    ; 22222..2222222....11111...333333
        .byte 3,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,4,0,0,0,0,1    ; 3.........3......2........4....1
        .byte 3,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,1,0,0,0,0,1    ; 3..c......3......2........1..b.1
        .byte 3,0,0,0,0,0,0,0,0,0,3,0,0,3,3,3,3,3,3,3,0,0,0,0,0,0,1,0,0,0,0,1    ; 3.........3..3333333......1....1
        .byte 3,0,0,0,0,3,3,0,0,0,3,0,0,3,0,0,0,0,0,3,0,0,0,0,0,0,0,0,0,0,0,1    ; 3....33...3..3.....3...........1
        .byte 3,0,0,0,0,3,3,0,0,0,0,0,0,3,0,0,0,0,0,3,0,0,0,0,0,0,1,0,0,0,0,1    ; 3....33......3..c..3......1....1
        .byte 3,0,0,0,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,3,0,0,0,0,0,0,1,0,0,0,0,1    ; 3............3.....3......1....1
        .byte 3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,1,1,1,1,1,1,1,1,1,1,1,1    ; 33333333333333333333111111111111
; Where you start, and the things: the cell each stands in the middle of, and its picture.
STARTX  = 3
STARTY  = 2
THINGS  = 13
thingx: .byte 16,26,5,10,25,20,6,29,6,20,3,29,16
thingy: .byte 1,2,4,7,7,10,11,11,14,14,18,18,21
thingp: .byte 4,6,4,5,6,4,4,5,5,6,6,4,6
; ---- END DATA ----

; Each thing this picture: how near it is, its left edge and its size; and the list of
; the ones to draw.
vis:    .res THINGS
td:     .res THINGS
txl:    .res THINGS
txh:    .res THINGS
thl:    .res THINGS
thh:    .res THINGS

; The map again, built at start-up: a page for each row, its 32 cells at the start.
        .align 256
mapbuf: .res MAPROWS * 256
walkbuf: .res MAPROWS * 256     ; the same with the things' cells filled: for walking
        .assert * <= $C000, "it all fits under the I/O"
