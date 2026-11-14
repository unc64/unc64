; FELLRIDER: a moped over the fells. For the UNC 64 with 64K and the NIECE, the
; prototype graphics chip marked NV-1 (manual chapter 17). 64K because the tables are big.
; United Northern Computers' laboratory, 1985. Needs 64K and the NIECE; no video board.
;
; A summer on the fells with fifty cubic centimetres. Ride the moped as far as you can
; over ground that gets rougher the further you go. Pick up the coins, and the gas cans
; before the tank runs dry. The run is over when the rider's head touches the ground, or
; when the gas is gone and the moped has rolled to a stop.
;
; Controls (held keys, from the keyboard matrix, so any of them go together):
;   RIGHT or D   throttle: drives the back wheel
;   LEFT or A    brake, both wheels; at a standstill, backwards
;   UP or W      lean back        DOWN or S   lean forward
;   SPACE        start, and start again
;
; The physics. Nothing here is on rails: the moped is where the sums put it.
;   The body (frame, engine and rider together) is one rigid thing with a place (X, Y),
;     a velocity (VX, VY), a pitch angle TH and a rate of turning OM. Its mass is 1 and
;     its moment of inertia 40.74, a radius of gyration of 6.4 pixels: that number makes
;     a force times an arm come out in the angle's own units with no division (`torque`).
;   Two wheels hang from it, each on its own spring and damper, 8 pixels fore and aft of
;     the centre of mass and 7 below it at full stretch. A wheel has no weight of its
;     own, but it has spin: a speed at the tyre (SL, SH), and an inertia a quarter of the
;     moped's mass.
;   Each step, for each wheel (`contact`): how far the tyre is into the ground is how far
;     its spring is squeezed (C). The spring pushes back along the ground's normal with
;     K x C and the damper with D x how fast C is changing; a bump stop ten times stiffer
;     takes over at the end of the travel. That push is the wheel's load, N.
;   Grip is the load times MU, no more. The tyre's speed over the ground less the ground's
;     speed under it is its slip, and the ground pulls on the tyre along its tangent in
;     proportion to the slip, up to the grip. So an unloaded wheel spins or locks, a
;     loaded one bites, and a wheel in the air does neither.
;   The engine turns the back wheel, and whatever turns the wheel one way turns the
;     moped the other: throttle lifts the front (hold it from a standstill and it goes
;     over backwards), brakes tip it forward, and in the air the two are how you steer
;     the pitch. Leaning is the rider's weight, a plain torque.
;   The rider's head is a third thing that can touch. When it does the run is over, and
;     for a second the same sums show the heap coming to rest.
;   Units: pixels and frames. Places and speeds have 16 bits of fraction (the sums use
;     the top 8 of them), forces are in 65536ths of a pixel per frame per frame, angles
;     in 65536ths of a circle. Gravity is 0.04. One step a frame, 50 a second, always:
;     forces from where things are, then speeds, then places (semi-implicit Euler).
;   Every multiplication is the NIECE's arithmetic unit: about 62 of them a step with
;     both wheels down. Sines are a table the assembler made, 1,024 to the circle.
;   A wheel all but still, with no key held, is still, and a moped standing on two such
;     wheels stands: otherwise rounding would have it creep downhill on the flat.
;
; The ground is three sine waves added up, of 300, 110 and 37 pixels' wavelength, times
; a roughness that grows from nothing at the start to its most 8,000 pixels on
; (`gencol`). It is worked out a column at a time as it comes into view and kept twice:
; as a height for the physics (HGL, HGH: a ring of 256 columns), and as a picture.
;
; What it shows of the NIECE:
;   the ground's picture is a strip of the sheet 256 columns wide, a ring like the
;     heights. A new column is five FILLs on the sheet (SHEET), one a pattern; the
;     whole of the ground on the page is then ONE BLIT a frame, whose source starts at
;     the camera's column and runs off the sheet's edge and round (chapter 17)
;   two more strips behind it, drawn by the chip itself at the start from the same sine
;     tables: the sky with the far fells and their snow, and the near fells. Each is
;     one BLIT a frame, moved an eighth and a quarter as fast as the ground
;   the moped tilts, and the chip can't turn a picture. So: the wheels are round, and
;     are drawn where the springs put them (squashed, by BLIT's scaling, when a spring
;     bottoms out). The fork, the swingarm and the shock are LINEs from the hubs to
;     the frame, so the suspension shows working for nothing. Only the body and rider
;     need turning, and they are turned once: at the start the processor draws them
;     at 32 angles, half a circle, onto the sheet (`spin`), and FLIPX with FLIPY, which
;     is a half turn, gives the other half. 64 angles, 5.6 degrees apart
;   coins spin by being drawn narrower; the title is drawn twice its size, and its
;     shadow is the same picture through an ink table that is all black
;   the short registers (NDXS...) wherever a place fits a byte; every command waits
;     for room in the queue, though the queue never gets near full
;   nothing is rubbed out: a frame is drawn from nothing on the page not shown, FLIP
;     shows it at the vertical blank, and the loop waits for the queue to empty, which
;     is the FLIP. One step of the physics to one frame, with no clock to watch
;
; Time, at 1 MHz (measured by crates/unc-asm/tests/game_fellrider.rs over a long ride):
;;TIMING;;
;
; Sound, the AUNT: voice 1 is the engine, its pitch the back wheel's speed; voice 2
; thumps when a wheel lands; voice 3 rings for a coin and gurgles for a can.
;
; The score and the words are ordinary text, in front of the page: a space is a window.
; Without the NIECE ($C14F doesn't read $17) FELLRIDER says so and stops.
;
; The tests find STATE, X0, Y0, VX, VY, TH, OM, SL, SH, CPL, CPH, NL, NH, GAS, COINS,
; DIST, BEST, CRASH, LATE, RMAX, HGL, HGH and the rest by name in the symbol table.

; ---------------------------------------------------------------- the machine
SCREEN  = $0400
FRAME   = $C006
KEYS    = $C060             ; the keyboard matrix, rows 0-8 (chapter 11)
V1      = $C0C0             ; the AUNT's three voices (chapter 12)
V2      = $C0C8
V3      = $C0D0
CUTOFF  = $C0D8
FILTER  = $C0D9
VOLUME  = $C0DA
NCTRL   = $C100             ; the NIECE (chapter 17)
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
NDXS    = $C119
NDYS    = $C11A
NDWS    = $C11B
NDHS    = $C11C
NPATTERN = $C11D
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

FILL    = 1                 ; NGO's commands
BLIT    = 2
LINE    = 3
FLIP    = 4
KEY     = $01               ; NFLAGS
FLIPX   = $02
FLIPY   = $04
SHEET   = $20
SHADOW  = $40               ; ink table 1: everything black
KEYC    = 4                 ; the see-through colour: purple, so black can be a tyre

; ---------------------------------------------------------------- the sheet
FARY    = 200               ; the sky and the far fells: 256 x 64, for the page's lines 0-63
FARH    = 64
NEARY   = 264               ; the near fells: 256 x 50, for lines 50-99
NEARH   = 50
NEARTOP = 50
GNDY    = 314               ; the ground: 256 x 66, for lines 34-99
GNDH    = 66
GNDTOP  = 34
BODYY   = 380               ; the body at 32 angles: four rows of eight, 32 x 32 each
WHEELX  = 160               ; beside the pages, from line 0: the wheel (two pictures, one
COINX   = 180               ;   under the other), the coin, the can, the sun, a cloud
CANX    = 188
SUNX    = 196
CLOUDX  = 204
LOGOY   = 22                ; and the title, 54 x 7 at column 160

; ---------------------------------------------------------------- the physics' numbers
; (scripts aren't needed to change these, but the tests pin how the moped behaves)
GRAV    = 2621              ; 0.04 pixels a frame a frame, in 65536ths
AXLE    = 8 * 256           ; hubs fore and aft of the centre of mass, pixels x 256
HANG    = 7 * 256           ; and below it, with the spring at full stretch
HEADB   = 3 * 256           ; the head: behind the centre of mass
HEADU   = 11 * 256          ;   and above it
TWOPI   = 1608              ; 6.283 x 256
KWHEEL  = 655               ; a spring: force for a pixel squeezed, in 65536ths
DWHEEL  = 3240              ; its damper: force for a pixel a frame
KHEAD   = 3000
DHEAD   = 9000
MUWHEEL = 282               ; grip: 1.1 of the load
MUHEAD  = 160
RWHEEL  = 5 * 256           ; a wheel's radius
RHEAD   = 640               ; and the helmet's: 2.5
CSTOP   = 1152              ; the travel: 4.5 pixels, then the bump stop
KSTOP   = 6000
CMAX    = 12                ; nothing is squeezed further than this, in whole pixels
NMAX    = 29000             ; nor pushed harder: 0.44, eleven times the moped's weight
SLIPMAX = 800
KTYRE   = 38                ; pull for slip: 38/256 of the moped's weight for a pixel a frame
DMAX    = 1800              ; the engine at a standstill: 0.027, two thirds of gravity
DFADE   = 256               ;   less this for each pixel a frame the wheel is doing
BRAKE   = 2600 * 4          ; the brake, as a change of tyre speed (a wheel's inertia is 1/4)
DREV    = 1200              ; backwards, gently
SREV    = 384               ; under 1.5 pixels a frame LEFT is backwards, not a brake
LEAN    = 10680             ; the rider's weight thrown: a torque, angle units x 256
VXMAX   = 4                 ; speeds are held to this many pixels a frame, across
VYMAX   = 7                 ;   and up and down
OMMAX   = 4000              ; and turning to this: 22 degrees a frame
STARTX  = 128
LEFTX   = 64                ; as far back as there is

BASE    = 74                ; the ground's middle line on the page
GTOP    = 36                ; the ground is never higher than this line
GBOT    = 93                ;   nor lower than this
FLAT    = 384               ; the first pixels are flat
SNOWY   = 50                ; above this line the ground is snow

TITLE   = 0                 ; STATE
RIDE    = 1
DOWN    = 2
OVER    = 3

; ---------------------------------------------------------------- the fast page
STATE   = $02
TICK    = $03               ; frames
LATE    = $04               ; frames that missed their vertical blank
LASTF   = $05
PADS    = $06               ; the keys: bit 0 brake, 1 throttle, 2 lean back, 3 lean forward
RNG     = $07
X0      = $08               ; where the centre of mass is, across: two bytes of fraction,
XL      = $0A               ;   then three of pixels
Y0      = $0D               ; and down the page: two of fraction, one of pixels (signed)
VX      = $10               ; velocity: two bytes of fraction and one of pixels, signed
VY      = $13
TH      = $16               ; pitch, nose up: a byte of fraction, then 65536ths of a circle
OM      = $19               ; and its rate, the same way
SF      = $1C               ; the wheels' tyre speeds (back, front): fraction,
SL      = $1E               ;   256ths
SH      = $20               ;   and pixels a frame
CPL     = $22               ; how far each spring is squeezed (back, front, and the head
CPH     = $25               ;   into the ground): 256ths, pixels
NL      = $28               ; the load on each
NH      = $2B
RXL     = $2E               ; where each is from the centre of mass, across
RXH     = $31
RYL     = $34               ;   and down (the wheels at full stretch)
RYH     = $37
SN      = $3A               ; sine and cosine of the pitch, 256 for 1
CS      = $3C
WR      = $3E               ; OM in radians a frame x 65536
AXA     = $40               ; this step's forces: across,
AYA     = $43               ;   down,
ALA     = $46               ;   and turning
T0      = $49
T1      = $4A
T2      = $4B
T3      = $4C
T4      = $4D
T5      = $4E
T6      = $4F
T7      = $50
GND     = $51               ; a contact: the ground's height under it
CC      = $53               ;   how far in
DC      = $55               ;   and how much further than last step
TXV     = $57               ;   the ground's tangent there, 256 long
TYV     = $59
NN      = $5B               ;   the push
FT      = $5D               ;   the pull along the ground
CT      = $5F               ;   the tangent's arm about the centre of mass
CN      = $61               ;   and the normal's
VT      = $63               ;   the contact's speed along the ground
LIM     = $65               ;   the grip
RCX     = $67               ;   the hub from the centre of mass, sprung
RCY     = $69
CRASH   = $6B               ; the head has touched
GAS     = $6C               ; two bytes: 65535 is a full tank
COINS   = $6E
DIST    = $6F               ; metres ridden, 16 pixels each
BEST    = $71
CAM     = $73               ; the page's left edge in the world: three bytes
GENR    = $76               ; the ring holds the columns from GENL to GENR - 1
GENL    = $79
HIGEN   = $7C               ; one past the furthest column ever made
COL     = $7F               ; the column being made
PTR     = $82
PTR2    = $84
SRC     = $86
VA      = $88
BY      = $8A               ; the centre of mass on the page, down
FI      = $8B               ; which of 64 angles the body is drawn at
WXS     = $8C               ; the hubs on the page
WYS     = $8E
IDX     = $90
SLIP    = $91               ; the back tyre's slip, for the dust
TIMER   = $93
STOPT   = $94               ; frames at a standstill with no gas
RMAX    = $95               ; the most the roughness goes to
WAL     = $96               ; how far each wheel has turned: 256ths, pixels
WAH     = $98
SPACED  = $9A               ; SPACE was down last frame
THUMP   = $9B               ; a wheel landed this step: how hard
DNEXT   = $9C               ; the next speck of dust
ODIST   = $9D               ; what the panel shows, so it's only written when it changes
OCOINS  = $9F
OGAS    = $A0
WHY     = $A1               ; how the run ended: 0 the head, 1 the gas
RING    = $A2               ; frames of voice 3 left
U0      = $A3               ; `spin`: the picture's place for this pixel, and this row
V0      = $A5
UU      = $A7
VV      = $A9
MOVED   = $AB               ; `scroll`: columns made to the right (1) or the left (2)
NOLATE  = $AC               ; a new run took frames to make: don't count them as late
DLIFE   = $B0               ; the dust: frames each speck has left,
DVYH    = $B8               ;   and its speed down, in pixels (the rest is after the tables)

BX      = 56                ; the centre of mass on the page, across: always here
NDUST   = 8

        .org $0800
start:  LDA NIECE
        CMP #$17
        BEQ @chip
        LDX #<nochip        ; no NIECE: say so and stop
        LDY #>nochip
        JSR PRSTR
        HLT
@chip:  JSR CLS
        LDX #39
@wait:  LDA waitmsg,X       ; the body takes three seconds to turn
        STA SCREEN + 12 * 40,X
        DEX
        BPL @wait
        LDA #KEYC
        STA NKEY
        LDA #1              ; ink table 1: all black, the title's shadow
        STA NTABLE
        LDX #15
        LDA #0
@ink:   STA NINK,X
        DEX
        BPL @ink
        LDA #0
        STA NTABLE
        STA LATE
        STA NOLATE
        STA BEST
        STA BEST+1
        STA SPACED
        LDA #255
        STA RMAX
        LDA #$5A
        STA RNG
        JSR upload
        JSR scenery
        JSR spin
        JSR sndinit
        JSR CLS
        JSR panel
        JSR newrun
        LDA #TITLE
        STA STATE
        JSR words
        LDA #1
        STA NCTRL           ; show the page
        LDA FRAME
        STA LASTF

; ---------------------------------------------------------------- a frame
; `work` to `endwork` is one frame's work; the wait after it is for the FLIP.
work:   INC TICK
        JSR keys
        JSR rules
        JSR physics
        JSR hud
        JSR sound
terrain:
        JSR scroll
draw:   JSR picture
@full:  BIT NSTATUS
        BVS @full
        LDA #FLIP
        STA NGO
endwork:
@busy:  BIT NSTATUS         ; the queue is empty when the FLIP has happened
        BMI @busy
        LDA FRAME
        TAX
        SEC
        SBC LASTF
        STX LASTF
        CMP #2
        BCC work
        LDA NOLATE
        BNE @new
        INC LATE            ; FRAME moved on by two: a frame was shown twice
        JMP work
@new:   LDA #0
        STA NOLATE
        JMP work

; A command into the queue, when there's room. A = the command.
go:     BIT NSTATUS
        BVS go
        STA NGO
        RTS

; ---------------------------------------------------------------- the keys
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
@s:     RTS

; ---------------------------------------------------------------- the rules
rules:  LDA KEYS            ; SPACE, as it goes down
        AND #1
        TAX
        EOR SPACED
        STX SPACED
        AND SPACED
        STA T0              ; 1 on the frame it's pressed
        LDA STATE
        CMP #RIDE
        BEQ @ride
        LDX #0              ; nobody's riding: no keys
        STX PADS
        CMP #DOWN
        BEQ @down
        CMP #OVER
        BEQ @over
        LDA T0              ; the title
        BEQ @done
        LDA #RIDE
        STA STATE
        JMP words
@down:  DEC TIMER
        BNE @done
        JMP runover
@over:  LDA TIMER
        BEQ @again
        DEC TIMER
        RTS
@again: LDA T0
        BEQ @done
        JSR newrun
        LDA #1
        STA NOLATE
        LDA #RIDE
        STA STATE
        JMP words
@done:  RTS
@ride:  LDA CRASH
        BEQ @up
        LDA #DOWN           ; the head touched
        STA STATE
        LDA #50
        STA TIMER
        LDA #0
        STA WHY
        STA PADS
        JMP crashsnd
@up:    SEC                 ; the engine ticking over
        LDA GAS
        SBC #3
        STA GAS
        LDA GAS+1
        SBC #0
        STA GAS+1
        BCC @dry
        LDA PADS
        AND #2
        BEQ @done
        SEC                 ; and pulling
        LDA GAS
        SBC #36
        STA GAS
        LDA GAS+1
        SBC #0
        STA GAS+1
        BCS @done
@dry:   LDA #0
        STA GAS
        STA GAS+1
        LDA PADS            ; no gas: no throttle
        AND #$0D
        STA PADS
        LDA VX+2            ; rolled to a stop? Under a sixteenth of a pixel a frame
        BEQ @slow
        CMP #$FF
        BNE @moving
        LDA VX+1
        CMP #$F0
        BCS @still
@moving:
        LDA #0
        STA STOPT
        RTS
@slow:  LDA VX+1
        CMP #$10
        BCS @moving
@still: INC STOPT
        LDA STOPT
        CMP #40
        BCC @done
        LDA #1
        STA WHY
; The run is over: the best distance, and the words.
runover:
        LDA DIST+1
        CMP BEST+1
        BCC @kept
        BNE @best
        LDA DIST
        CMP BEST
        BCC @kept
@best:  LDA DIST
        STA BEST
        LDA DIST+1
        STA BEST+1
@kept:  LDA #OVER
        STA STATE
        LDA #30
        STA TIMER
        JMP words

; ---------------------------------------------------------------- a new run
newrun: LDX #X0
        LDA #0
@zero:  STA 0,X             ; everything that moves, still
        INX
        CPX #RXL
        BNE @zero
        LDX #CRASH
@zero2: STA 0,X
        INX
        CPX #BEST
        BNE @zero2
        STA STOPT
        STA THUMP
        STA DNEXT
        STA WAL
        STA WAL+1
        STA WAH
        STA WAH+1
        STA HIGEN
        STA HIGEN+1
        STA HIGEN+2
        LDX #15
@items: STA itype,X
        DEX
        BPL @items
        LDX #NDUST-1
@dust:  STA DLIFE,X
        DEX
        BPL @dust
        LDA #STARTX
        STA XL
        LDA #BASE - 10      ; standing: the springs squeezed by its weight
        STA Y0+2
        LDA #$FF
        STA GAS
        STA GAS+1
        STA ODIST+1         ; the panel is all to write again
        STA OCOINS
        STA OGAS
        LDA #STARTX - BX    ; the camera, and the ring: 256 columns from 40 left of it
        STA CAM
        LDA #STARTX - BX - 40
        STA GENL
        STA COL
        LDA #0
        STA CAM+1
        STA CAM+2
        STA GENL+1
        STA GENL+2
        STA COL+1
        STA COL+2
        STA T7
@col:   JSR gencol
        INC COL
        BNE @same
        INC COL+1
@same:  DEC T7
        BNE @col
        LDA COL
        STA GENR
        LDA COL+1
        STA GENR+1
        LDA #0
        STA GENR+2
        RTS

; ---------------------------------------------------------------- the physics
; One step: a frame. See the top of the file for what it is; sim.py in the tests' notes
; does the same sums.
physics:
        LDA TH+1            ; the pitch's sine and cosine: 1,024 to the circle
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        STA T0
        LDA TH+2
        ASL A
        ASL A
        ORA T0
        TAY
        LDA TH+2
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        TAX
        CLC
        ADC #>sinlo
        STA PTR+1
        TXA
        CLC
        ADC #>sinhi
        STA PTR2+1
        LDA #0
        STA PTR
        STA PTR2
        LDA (PTR),Y
        STA SN
        LDA (PTR2),Y
        STA SN+1
        INC PTR+1           ; the cosine is a quarter turn on
        INC PTR2+1
        LDA (PTR),Y
        STA CS
        STA NMULA
        LDA (PTR2),Y
        STA CS+1
        STA NMULA+1
        ; where the wheels and the head are from the centre of mass. Forward is
        ; (cos, -sin) and down the moped is (sin, cos): y grows down the page.
        LDA #<AXLE
        STA NMULB
        LDA #>AXLE
        STA NMULB+1
        LDA NPROD+1         ; AXLE cos
        STA T0
        LDA NPROD+2
        STA T1
        LDA #<HANG
        STA NMULB
        LDA #>HANG
        STA NMULB+1
        LDA NPROD+1         ; HANG cos
        STA T2
        LDA NPROD+2
        STA T3
        LDA #<HEADB
        STA NMULB
        LDA #>HEADB
        STA NMULB+1
        SEC                 ; the head, across: -HEADB cos (- HEADU sin, below)
        LDA #0
        SBC NPROD+1
        STA RXL+2
        LDA #0
        SBC NPROD+2
        STA RXH+2
        LDA #<HEADU
        STA NMULB
        LDA #>HEADU
        STA NMULB+1
        SEC                 ; the head, down: -HEADU cos (+ HEADB sin)
        LDA #0
        SBC NPROD+1
        STA RYL+2
        LDA #0
        SBC NPROD+2
        STA RYH+2
        LDA SN
        STA NMULA
        LDA SN+1
        STA NMULA+1
        SEC                 ; (NMULB is still HEADU)
        LDA RXL+2
        SBC NPROD+1
        STA RXL+2
        LDA RXH+2
        SBC NPROD+2
        STA RXH+2
        LDA #<HEADB
        STA NMULB
        LDA #>HEADB
        STA NMULB+1
        CLC
        LDA RYL+2
        ADC NPROD+1
        STA RYL+2
        LDA RYH+2
        ADC NPROD+2
        STA RYH+2
        LDA #<HANG
        STA NMULB
        LDA #>HANG
        STA NMULB+1
        LDA NPROD+1         ; HANG sin
        STA T4
        LDA NPROD+2
        STA T5
        LDA #<AXLE
        STA NMULB
        LDA #>AXLE
        STA NMULB+1
        LDA NPROD+1         ; AXLE sin
        STA T6
        LDA NPROD+2
        STA T7
        SEC                 ; back wheel: (HANG sin - AXLE cos, HANG cos + AXLE sin)
        LDA T4
        SBC T0
        STA RXL
        LDA T5
        SBC T1
        STA RXH
        CLC
        LDA T2
        ADC T6
        STA RYL
        LDA T3
        ADC T7
        STA RYH
        CLC                 ; front wheel: (HANG sin + AXLE cos, HANG cos - AXLE sin)
        LDA T4
        ADC T0
        STA RXL+1
        LDA T5
        ADC T1
        STA RXH+1
        SEC
        LDA T2
        SBC T6
        STA RYL+1
        LDA T3
        SBC T7
        STA RYH+1
        LDA OM+1            ; the turning in radians, for the speed of a point
        STA NMULA
        LDA OM+2
        STA NMULA+1
        LDA #<TWOPI
        STA NMULB
        LDA #>TWOPI
        STA NMULB+1
        LDA NPROD+1
        STA WR
        LDA NPROD+2
        STA WR+1
        LDA #0              ; this step's forces: gravity, to begin with
        STA AXA
        STA AXA+1
        STA AXA+2
        STA AYA+2
        STA ALA
        STA ALA+1
        STA ALA+2
        STA THUMP
        STA SLIP
        STA SLIP+1
        LDA #<GRAV
        STA AYA
        LDA #>GRAV
        STA AYA+1
        LDA #0
        STA IDX
        JSR contact
        INC IDX
        JSR contact
        INC IDX
        JSR contact
        LDA NL+2            ; the head pushed on the ground
        ORA NH+2
        BEQ @head
        LDA #1
        STA CRASH
@head:  LDA CRASH
        BEQ @alive
        LDA #0
        STA PADS
@alive: JSR controls
        JMP move

; ---- one wheel, or the head, against the ground. IDX = which.
contact:
        LDX IDX
        CLC                 ; the column it's over, and how far across the column
        LDA X0+1
        ADC RXL,X
        STA T0
        LDA XL
        ADC RXH,X
        TAY
        STA T1
        TAX
        INX
        SEC                 ; the ground there: between this column's height and the next's
        LDA HGL,X
        SBC HGL,Y
        STA NMULA
        LDA HGH,X
        SBC HGH,Y
        STA NMULA+1
        LDA T0
        STA NMULB
        LDA #0
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC HGL,Y
        STA GND
        LDA NPROD+2
        ADC HGH,Y
        STA GND+1
        LDX IDX
        CLC                 ; its lowest point
        LDA Y0+1
        ADC RYL,X
        STA T2
        LDA Y0+2
        ADC RYH,X
        STA T3
        CLC
        LDA T2
        ADC radl,X
        STA T2
        LDA T3
        ADC radh,X
        STA T3
        SEC                 ; less the ground: how far in
        LDA T2
        SBC GND
        STA CC
        LDA T3
        SBC GND+1
        STA CC+1
        BMI @air
        ORA CC
        BNE @touch
@air:   LDA #0              ; in the air: nothing squeezed, no load
        STA CPL,X
        STA CPH,X
        STA NL,X
        STA NH,X
        RTS
@touch: LDA CC+1
        CMP #CMAX
        BCC @in
        LDA #CMAX
        STA CC+1
        LDA #0
        STA CC
@in:    SEC                 ; and how much further than last step
        LDA CC
        SBC CPL,X
        STA DC
        LDA CC+1
        SBC CPH,X
        STA DC+1
        LDA CPL,X           ; landing: it was in the air
        ORA CPH,X
        BNE @was
        LDA CC+1
        BEQ @was
        STA THUMP
@was:   LDA CC
        STA CPL,X
        LDA CC+1
        STA CPH,X
        LDY T1              ; the slope: two columns on less two columns back
        INY
        INY
        TYA
        SEC
        SBC #4
        TAX
        SEC
        LDA HGL,Y
        SBC HGL,X
        STA T2
        LDA HGH,Y
        SBC HGH,X
        CLC
        ADC #$08            ; 16ths of a pixel in 4, from -128 to 127: the table's index
        BMI @steepu
        CMP #$10
        BCS @steepd
        ASL A
        ASL A
        ASL A
        ASL A
        STA T3
        LDA T2
        LSR A
        LSR A
        LSR A
        LSR A
        ORA T3
        JMP @slope
@steepu:
        LDA #0
        BEQ @slope
@steepd:
        LDA #255
@slope: TAY
        LDA tanxl,Y         ; the tangent, 256 long: (TXV, TYV). The normal is (TYV, -TXV)
        STA TXV
        LDA tanxh,Y
        STA TXV+1
        LDA tanyl,Y
        STA TYV
        LDA tanyh,Y
        STA TYV+1
        LDX IDX
        ; the push: K x how far in + D x how fast, in 24 bits, then held to 0..NMAX
        LDA CC
        STA NMULA
        LDA CC+1
        STA NMULA+1
        LDA kl,X
        STA NMULB
        LDA kh,X
        STA NMULB+1
        LDA NPROD+1
        STA T4
        LDA NPROD+2
        STA T5
        LDA NPROD+3
        STA T6
        LDA DC
        STA NMULA
        LDA DC+1
        STA NMULA+1
        LDA dl,X
        STA NMULB
        LDA dh,X
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC T4
        STA T4
        LDA NPROD+2
        ADC T5
        STA T5
        LDA NPROD+3
        ADC T6
        STA T6
        CPX #2
        BEQ @nostop
        SEC                 ; a wheel past the end of its travel: the bump stop as well
        LDA CC
        SBC #<CSTOP
        TAY
        LDA CC+1
        SBC #>CSTOP
        BCC @nostop
        STY NMULA
        STA NMULA+1
        LDA #<KSTOP
        STA NMULB
        LDA #>KSTOP
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC T4
        STA T4
        LDA NPROD+2
        ADC T5
        STA T5
        LDA NPROD+3
        ADC T6
        STA T6
@nostop:
        LDA T6
        BMI @none           ; the damper can't pull the ground up
        BNE @most
        LDA T4
        CMP #<NMAX
        LDA T5
        SBC #>NMAX
        BCC @held
@most:  LDA #<NMAX
        STA T4
        LDA #>NMAX
        STA T5
@held:  LDA T4
        STA NN
        STA NL,X
        LDA T5
        STA NN+1
        STA NH,X
        ORA T4
        BNE @push
        RTS
@none:  LDA #0
        STA NL,X
        STA NH,X
        RTS
@push:  LDA RXL,X           ; the hub from the centre of mass, with the spring squeezed
        STA RCX
        LDA RXH,X
        STA RCX+1
        SEC
        LDA RYL,X
        SBC CC
        STA RCY
        STA NMULA
        LDA RYH,X
        SBC CC+1
        STA RCY+1
        STA NMULA+1
        ; the arms of the tangent and the normal about the centre of mass:
        ;   CT = RCY tx - RCX ty,  CN = RCY ty + RCX tx
        LDA TXV
        STA NMULB
        LDA TXV+1
        STA NMULB+1
        LDA NPROD+1
        STA CT
        LDA NPROD+2
        STA CT+1
        LDA TYV
        STA NMULB
        LDA TYV+1
        STA NMULB+1
        LDA NPROD+1
        STA CN
        LDA NPROD+2
        STA CN+1
        LDA RCX
        STA NMULA
        LDA RCX+1
        STA NMULA+1
        SEC
        LDA CT
        SBC NPROD+1
        STA CT
        LDA CT+1
        SBC NPROD+2
        STA CT+1
        LDA TXV
        STA NMULB
        LDA TXV+1
        STA NMULB+1
        CLC
        LDA CN
        ADC NPROD+1
        STA CN
        LDA CN+1
        ADC NPROD+2
        STA CN+1
        ; how fast the moped is going along the ground here: its velocity along the
        ; tangent, and its turning times the tangent's arm
        LDA VX+1
        STA NMULA
        LDA VX+2
        STA NMULA+1
        LDA NPROD+1
        STA VT
        LDA NPROD+2
        STA VT+1
        LDA VY+1
        STA NMULA
        LDA VY+2
        STA NMULA+1
        LDA TYV
        STA NMULB
        LDA TYV+1
        STA NMULB+1
        CLC
        LDA VT
        ADC NPROD+1
        STA VT
        LDA VT+1
        ADC NPROD+2
        STA VT+1
        LDA WR
        STA NMULA
        LDA WR+1
        STA NMULA+1
        LDA CT
        STA NMULB
        LDA CT+1
        STA NMULB+1
        CLC
        LDA VT
        ADC NPROD+2
        STA VT
        LDA VT+1
        ADC NPROD+3
        STA VT+1
        ; the slip: the tyre's speed less that (the head has no tyre: it scrapes)
        CPX #2
        BEQ @scrape
        SEC
        LDA SL,X
        SBC VT
        STA T4
        LDA SH,X
        SBC VT+1
        JMP @slip
@scrape:
        SEC
        LDA #0
        SBC VT
        STA T4
        LDA #0
        SBC VT+1
@slip:  STA T5
        BMI @sneg           ; held to SLIPMAX either way, so the pull fits 16 bits
        LDA T4
        CMP #<SLIPMAX
        LDA T5
        SBC #>SLIPMAX
        BCC @sok
        LDA #<SLIPMAX
        STA T4
        LDA #>SLIPMAX
        STA T5
        BNE @sok
@sneg:  LDA T4
        CMP #<[-SLIPMAX]
        LDA T5
        SBC #>[-SLIPMAX]
        BCS @sok
        LDA #<[-SLIPMAX]
        STA T4
        LDA #>[-SLIPMAX]
        STA T5
@sok:   CPX #0
        BNE @notback
        LDA T4
        STA SLIP
        LDA T5
        STA SLIP+1
@notback:
        LDA NN              ; the grip: the load times MU
        STA NMULA
        LDA NN+1
        STA NMULA+1
        LDA mul,X
        STA NMULB
        LDA muh,X
        STA NMULB+1
        LDA NPROD+1
        STA LIM
        LDA NPROD+2
        STA LIM+1
        LDA T4              ; the pull: KTYRE x slip, up to the grip either way
        STA NMULA
        LDA T5
        STA NMULA+1
        LDA #KTYRE
        STA NMULB
        LDA #0
        STA NMULB+1
        LDA NPROD
        STA FT
        LDA NPROD+1
        STA FT+1
        BMI @fneg
        LDA LIM
        CMP FT
        LDA LIM+1
        SBC FT+1
        BCS @fok
        LDA LIM
        STA FT
        LDA LIM+1
        STA FT+1
        JMP @fok
@fneg:  CLC                 ; FT + LIM < 0: too much the other way
        LDA FT
        ADC LIM
        LDA FT+1
        ADC LIM+1
        BPL @fok
        SEC
        LDA #0
        SBC LIM
        STA FT
        LDA #0
        SBC LIM+1
        STA FT+1
@fok:   CPX #2
        BEQ @nospin
        LDA FT              ; the ground's pull slows the tyre: four times it, the wheel
        STA T4              ;   being a quarter of the moped
        LDA FT+1
        STA T5
        LDY #0
        ORA #0
        BPL @fp
        DEY
@fp:    STY T6
        ASL T4
        ROL T5
        ROL T6
        ASL T4
        ROL T5
        ROL T6
        SEC
        LDA SF,X
        SBC T4
        STA SF,X
        LDA SL,X
        SBC T5
        STA SL,X
        LDA SH,X
        SBC T6
        STA SH,X
@nospin:
        ; the force on the moped: N along the normal (ty, -tx), FT along the tangent
        LDA NN
        STA NMULA
        LDA NN+1
        STA NMULA+1
        LDA TYV
        STA NMULB
        LDA TYV+1
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC AXA
        STA AXA
        LDA NPROD+2
        ADC AXA+1
        STA AXA+1
        LDY #0
        LDA NPROD+2
        BPL @p1
        DEY
@p1:    TYA
        ADC AXA+2
        STA AXA+2
        LDA TXV
        STA NMULB
        LDA TXV+1
        STA NMULB+1
        LDY #0
        LDA NPROD+2
        BPL @p2
        DEY
@p2:    STY T6
        SEC
        LDA AYA
        SBC NPROD+1
        STA AYA
        LDA AYA+1
        SBC NPROD+2
        STA AYA+1
        LDA AYA+2
        SBC T6
        STA AYA+2
        LDA CN              ; and its turning: N x CN + FT x CT (`torque`: with the moment
        STA NMULB           ;   of inertia 40.74, the product's top three bytes are the
        LDA CN+1            ;   change in OM, fraction and all)
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC ALA
        STA ALA
        LDA NPROD+2
        ADC ALA+1
        STA ALA+1
        LDA NPROD+3
        ADC ALA+2
        STA ALA+2
        LDA FT
        STA NMULA
        LDA FT+1
        STA NMULA+1
        LDA CT
        STA NMULB
        LDA CT+1
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC ALA
        STA ALA
        LDA NPROD+2
        ADC ALA+1
        STA ALA+1
        LDA NPROD+3
        ADC ALA+2
        STA ALA+2
        LDA TXV
        STA NMULB
        LDA TXV+1
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC AXA
        STA AXA
        LDA NPROD+2
        ADC AXA+1
        STA AXA+1
        LDY #0
        LDA NPROD+2
        BPL @p3
        DEY
@p3:    TYA
        ADC AXA+2
        STA AXA+2
        LDA TYV
        STA NMULB
        LDA TYV+1
        STA NMULB+1
        CLC
        LDA NPROD+1
        ADC AYA
        STA AYA
        LDA NPROD+2
        ADC AYA+1
        STA AYA+1
        LDY #0
        LDA NPROD+2
        BPL @p4
        DEY
@p4:    TYA
        ADC AYA+2
        STA AYA+2
        RTS

; ---- the engine, the brakes and the rider's weight
controls:
        LDA PADS
        AND #2
        BNE @thr
        JMP @nothr
@thr:   LDA SL              ; throttle: DMAX less DFADE for each pixel a frame of tyre
        STA NMULA
        LDA SH
        STA NMULA+1
        LDA #<DFADE
        STA NMULB
        LDA #>DFADE
        STA NMULB+1
        SEC
        LDA #<DMAX
        SBC NPROD+1
        STA T0
        LDA #>DMAX
        SBC NPROD+2
        STA T1
        BMI @nodrive        ; flat out
        LDA T0
        CMP #<DMAX
        LDA T1
        SBC #>DMAX
        BCC @drive
        LDA #<DMAX          ; (rolling backwards: no more than DMAX)
        STA T0
        LDA #>DMAX
        STA T1
        JMP @drive
@nodrive:
        JMP @lean
; The engine turns the wheel by T0,T1 (a force at the tyre; signed), and the moped the
; other way by the same force at the wheel's radius.
@drive: LDA T0
        STA T4
        LDA T1
        STA T5
        LDY #0
        ORA #0
        BPL @dp
        DEY
@dp:    STY T6
        STY T3              ; (the sign, for the turning below)
        CLC                 ; the turning: T x 5 pixels of radius
        LDA T0
        ADC ALA
        STA ALA
        LDA T1
        ADC ALA+1
        STA ALA+1
        TYA
        ADC ALA+2
        STA ALA+2
        ASL T4              ; four times it
        ROL T5
        ROL T6
        ASL T4
        ROL T5
        ROL T6
        CLC                 ; onto the tyre's speed
        LDA SF
        ADC T4
        STA SF
        LDA SL
        ADC T5
        STA SL
        LDA SH
        ADC T6
        STA SH
        CLC                 ; and the other four of the five
        LDA T4
        ADC ALA
        STA ALA
        LDA T5
        ADC ALA+1
        STA ALA+1
        LDA T6
        ADC ALA+2
        STA ALA+2
        JMP @lean
@nothr: LDA PADS
        LSR A
        BCS @brk
        JMP @coast
@brk:   SEC                 ; LEFT: either wheel doing SREV or more forwards? Then brake
        LDA SL
        SBC #<SREV
        LDA SH
        SBC #>SREV
        BPL @brake
        SEC
        LDA SL+1
        SBC #<SREV
        LDA SH+1
        SBC #>SREV
        BPL @brake
        CLC                 ; no: backwards, while the tyre's doing less than 3 SREV that way
        LDA SL
        ADC #<[3 * SREV]
        LDA SH
        ADC #>[3 * SREV]
        BMI @lean
        LDA #<[-DREV]
        STA T0
        LDA #>[-DREV]
        STA T1
        JMP @drive
@brake: LDX #1
@bwheel:
        LDA SH,X            ; each tyre's speed towards nothing by BRAKE, or all of it
        BMI @bneg
        LDA SF,X
        CMP #<BRAKE
        LDA SL,X
        SBC #>BRAKE
        LDA SH,X
        SBC #0
        BCS @bfull
        LDA SF,X
        STA T4
        LDA SL,X
        STA T5
        JMP @bdo
@bfull: LDA #<BRAKE
        STA T4
        LDA #>BRAKE
        STA T5
        JMP @bdo
@bneg:  LDA SF,X
        CMP #<[-BRAKE]
        LDA SL,X
        SBC #>[-BRAKE]
        LDA SH,X
        SBC #$FF
        BCC @bnfull
        LDA SF,X
        STA T4
        LDA SL,X
        STA T5
        JMP @bdo
@bnfull:
        LDA #<[-BRAKE]
        STA T4
        LDA #>[-BRAKE]
        STA T5
@bdo:   LDY #0
        LDA T5
        BPL @bp
        DEY
@bp:    STY T6
        SEC                 ; off the tyre
        LDA SF,X
        SBC T4
        STA SF,X
        LDA SL,X
        SBC T5
        STA SL,X
        LDA SH,X
        SBC T6
        STA SH,X
        SEC                 ; and the moped turns with the wheel it stopped: that change
        LDA ALA             ;   and a quarter of it again (5 pixels of radius over 4)
        SBC T4
        STA ALA
        LDA ALA+1
        SBC T5
        STA ALA+1
        LDA ALA+2
        SBC T6
        STA ALA+2
        LDA T5
        CMP #$80
        ROR T5
        ROR T4
        LDA T5
        CMP #$80
        ROR T5
        ROR T4
        SEC
        LDA ALA
        SBC T4
        STA ALA
        LDA ALA+1
        SBC T5
        STA ALA+1
        LDA ALA+2
        SBC T6
        STA ALA+2
        DEX
        BPL @bwheel
        JMP @lean
@coast: LDA SF              ; no key: the engine holds the back wheel back, a 64th a frame
        STA T4
        LDA SL
        STA T5
        LDA SH
        STA T6
        LDY #0
        ORA #0
        BPL @cp
        DEY
@cp:    STY T7
        ASL T4
        ROL T5
        ROL T6
        ROL T7
        ASL T4
        ROL T5
        ROL T6
        ROL T7
        SEC
        LDA SF
        SBC T5
        STA SF
        LDA SL
        SBC T6
        STA SL
        LDA SH
        SBC T7
        STA SH
@lean:  LDY #0              ; the front wheel's bearings: a 256th a frame
        LDA SH+1
        BPL @fp
        DEY
@fp:    STY T6
        SEC
        LDA SF+1
        SBC SL+1
        STA SF+1
        LDA SL+1
        SBC SH+1
        STA SL+1
        LDA SH+1
        SBC T6
        STA SH+1
        LDA PADS
        AND #3
        BNE @keys
        ; no throttle and no brake: a wheel all but still is still, and a moped
        ; standing on two still wheels stands
        LDX #1
@stick: LDA SH,X
        BEQ @spos
        CMP #$FF
        BNE @rolls
        LDA SL,X
        CMP #$FD
        BCC @rolls
        BCS @stuck
@spos:  LDA SL,X
        CMP #3
        BCS @rolls
@stuck: LDA #0
        STA SF,X
        STA SL,X
        STA SH,X
@rolls: DEX
        BPL @stick
        LDA SF
        ORA SL
        ORA SH
        ORA SF+1
        ORA SL+1
        ORA SH+1
        BNE @keys
        LDA NL
        ORA NH
        BEQ @keys
        LDA NL+1
        ORA NH+1
        BEQ @keys
        LDA VX+2
        BNE @vneg
        LDA VX+1
        BNE @keys
        BEQ @stand
@vneg:  CMP #$FF
        BNE @keys
        LDA VX+1
        CMP #$FF
        BNE @keys
@stand: LDA #0
        STA VX
        STA VX+1
        STA VX+2
@keys:  LDA PADS            ; the rider leans
        AND #4
        BEQ @noback
        CLC
        LDA ALA
        ADC #<LEAN
        STA ALA
        LDA ALA+1
        ADC #>LEAN
        STA ALA+1
        BCC @noback
        INC ALA+2
@noback:
        LDA PADS
        AND #8
        BEQ @nofwd
        SEC
        LDA ALA
        SBC #<LEAN
        STA ALA
        LDA ALA+1
        SBC #>LEAN
        STA ALA+1
        BCS @nofwd
        DEC ALA+2
@nofwd: RTS

; ---- speeds from the forces, then places from the speeds
move:   LDY #0              ; VX += AXA - VX / 256: the air
        LDA VX+2
        BPL @xp
        DEY
@xp:    STY T6
        SEC
        LDA VX
        SBC VX+1
        STA T0
        LDA VX+1
        SBC VX+2
        STA T1
        LDA VX+2
        SBC T6
        STA T2
        CLC
        LDA T0
        ADC AXA
        STA VX
        LDA T1
        ADC AXA+1
        STA VX+1
        LDA T2
        ADC AXA+2
        STA VX+2
        BMI @xneg
        CMP #VXMAX
        BCC @xok
        LDA #$FF
        STA VX
        STA VX+1
        LDA #VXMAX-1
        STA VX+2
        BNE @xok
@xneg:  CMP #<[-VXMAX]
        BCS @xok
        LDA #0
        STA VX
        STA VX+1
        LDA #<[-VXMAX]
        STA VX+2
@xok:   LDY #0
        LDA VY+2
        BPL @yp
        DEY
@yp:    STY T6
        SEC
        LDA VY
        SBC VY+1
        STA T0
        LDA VY+1
        SBC VY+2
        STA T1
        LDA VY+2
        SBC T6
        STA T2
        CLC
        LDA T0
        ADC AYA
        STA VY
        LDA T1
        ADC AYA+1
        STA VY+1
        LDA T2
        ADC AYA+2
        STA VY+2
        BMI @yneg
        CMP #VYMAX
        BCC @yok
        LDA #$FF
        STA VY
        STA VY+1
        LDA #VYMAX-1
        STA VY+2
        BNE @yok
@yneg:  CMP #<[-VYMAX]
        BCS @yok
        LDA #0
        STA VY
        STA VY+1
        LDA #<[-VYMAX]
        STA VY+2
@yok:   LDY #0              ; OM += ALA - OM / 256
        LDA OM+2
        BPL @op
        DEY
@op:    STY T6
        SEC
        LDA OM
        SBC OM+1
        STA T0
        LDA OM+1
        SBC OM+2
        STA T1
        LDA OM+2
        SBC T6
        STA T2
        CLC
        LDA T0
        ADC ALA
        STA OM
        LDA T1
        ADC ALA+1
        STA OM+1
        LDA T2
        ADC ALA+2
        STA OM+2
        BMI @oneg
        LDA OM+1
        CMP #<OMMAX
        LDA OM+2
        SBC #>OMMAX
        BCC @ook
        LDA #<OMMAX
        STA OM+1
        LDA #>OMMAX
        STA OM+2
        LDA #0
        STA OM
        BEQ @ook
@oneg:  LDA OM+1
        CMP #<[-OMMAX]
        LDA OM+2
        SBC #>[-OMMAX]
        BCS @ook
        LDA #<[-OMMAX]
        STA OM+1
        LDA #>[-OMMAX]
        STA OM+2
        LDA #0
        STA OM
@ook:   LDY #0              ; places
        LDA VX+2
        BPL @mp
        DEY
@mp:    CLC
        LDA X0
        ADC VX
        STA X0
        LDA X0+1
        ADC VX+1
        STA X0+1
        LDA XL
        ADC VX+2
        STA XL
        TYA
        ADC XL+1
        STA XL+1
        TYA
        ADC XL+2
        STA XL+2
        CLC
        LDA Y0
        ADC VY
        STA Y0
        LDA Y0+1
        ADC VY+1
        STA Y0+1
        LDA Y0+2
        ADC VY+2
        STA Y0+2
        CLC
        LDA TH
        ADC OM
        STA TH
        LDA TH+1
        ADC OM+1
        STA TH+1
        LDA TH+2
        ADC OM+2
        STA TH+2
        LDA XL+1            ; the left end of the world
        ORA XL+2
        BNE @right
        LDA XL
        CMP #LEFTX
        BCS @right
        LDA #LEFTX
        STA XL
        LDA #0
        STA X0
        STA X0+1
        STA VX
        STA VX+1
        STA VX+2
@right: LDA Y0+2            ; and the top of the sky: 100 lines above the page
        BPL @sky
        CMP #<[-100]
        BCS @sky
        LDA #<[-100]
        STA Y0+2
        LDA #0
        STA Y0
        STA Y0+1
        LDA VY+2
        BPL @sky
        LDA #0
        STA VY
        STA VY+1
        STA VY+2
@sky:   LDX #1              ; how far the wheels have turned, for their spokes
@turn:  CLC
        LDA WAL,X
        ADC SL,X
        STA WAL,X
        LDA WAH,X
        ADC SH,X
        STA WAH,X
        DEX
        BPL @turn
        RTS

; ---------------------------------------------------------------- the ground
; Brings the ring up to the camera: a column made for each pixel moved.
scroll: SEC
        LDA XL
        SBC #BX
        STA CAM
        LDA XL+1
        SBC #0
        STA CAM+1
        LDA XL+2
        SBC #0
        STA CAM+2
        LDA #0
        STA MOVED
@more:  CLC                 ; the ring must reach 168 columns right of the camera
        LDA CAM
        ADC #168
        STA T0
        LDA CAM+1
        ADC #0
        STA T1
        LDA CAM+2
        ADC #0
        STA T2
        LDA GENR
        CMP T0
        LDA GENR+1
        SBC T1
        LDA GENR+2
        SBC T2
        BCS @left
        LDA GENR
        STA COL
        LDA GENR+1
        STA COL+1
        LDA GENR+2
        STA COL+2
        JSR gencol
        INC GENR
        BNE @r1
        INC GENR+1
        BNE @r1
        INC GENR+2
@r1:    LDA #1
        STA MOVED
        BNE @more
@left:  SEC                 ; and 8 left of it
        LDA CAM
        SBC #8
        STA T0
        LDA CAM+1
        SBC #0
        STA T1
        LDA CAM+2
        SBC #0
        STA T2
        LDA T0
        CMP GENL
        LDA T1
        SBC GENL+1
        LDA T2
        SBC GENL+2
        BCS @ends
        LDA GENL
        BNE @l1
        LDA GENL+1
        BNE @l2
        DEC GENL+2
@l2:    DEC GENL+1
@l1:    DEC GENL
        LDA GENL
        STA COL
        LDA GENL+1
        STA COL+1
        LDA GENL+2
        STA COL+2
        JSR gencol
        LDA #2
        STA MOVED
        BNE @left
@ends:  LDA MOVED           ; the ring is 256 columns: what came in one side went out the other
        BEQ @done
        CMP #1
        BNE @fromleft
        SEC
        LDA GENR
        STA GENL
        LDA GENR+1
        SBC #1
        STA GENL+1
        LDA GENR+2
        SBC #0
        STA GENL+2
        RTS
@fromleft:
        CLC
        LDA GENL
        STA GENR
        LDA GENL+1
        ADC #1
        STA GENR+1
        LDA GENL+2
        ADC #0
        STA GENR+2
@done:  RTS

; Makes column COL (three bytes): its height into the ring, its picture onto the sheet.
gencol: LDA COL             ; three sines: each one's angle is the column times its step
        STA NMULA
        LDA COL+1
        STA NMULA+1
        LDA #<GF1
        STA NMULB
        LDA #>GF1
        STA NMULB+1
        LDX NPROD+1
        LDA #<GF2
        STA NMULB
        LDA #>GF2
        STA NMULB+1
        LDY NPROD+1
        CLC
        LDA wave1,X
        ADC wave2,Y
        STA T0
        LDA #<GF3
        STA NMULB
        LDA #>GF3
        STA NMULB+1
        LDY NPROD+1
        CLC
        LDA T0
        ADC wave3,Y
        STA NMULA           ; -110 to 110
        LDY #0
        ORA #0
        BPL @wp
        DEY
@wp:    STY NMULA+1
        SEC                 ; the roughness: a 32nd of the pixels past the flat, up to RMAX
        LDA COL
        SBC #<FLAT
        STA T0
        LDA COL+1
        SBC #>FLAT
        STA T1
        LDA COL+2
        SBC #0
        BCC @flat
        BNE @rough
        LDA T1
        CMP #$20
        BCS @rough
        ASL A
        ASL A
        ASL A
        STA T1
        LDA T0
        LSR A
        LSR A
        LSR A
        LSR A
        LSR A
        ORA T1
        CMP RMAX
        BCC @r
@rough: LDA RMAX
        JMP @r
@flat:  LDA #0
@r:     STA NMULB
        LDA #0
        STA NMULB+1
        LDA NPROD           ; a quarter of the product, taken from the middle line
        STA T0
        LDA NPROD+1
        CMP #$80
        ROR A
        ROR T0
        CMP #$80
        ROR A
        ROR T0
        STA T1
        LDX COL
        SEC
        LDA #0
        SBC T0
        TAY
        LDA #BASE
        SBC T1
        BMI @high
        CMP #GTOP
        BCC @high
        CMP #GBOT
        BCC @hok
        LDA #GBOT
        BNE @edge
@high:  LDA #GTOP
@edge:  LDY #0
@hok:   STA HGH,X
        STA T2              ; the line the ground starts on
        TYA
        STA HGL,X
        ; something to pick up? One place in 64 columns, the first time it's made
        TXA
        AND #63
        CMP #32
        BNE @paint
        LDA COL
        CMP HIGEN
        LDA COL+1
        SBC HIGEN+1
        LDA COL+2
        SBC HIGEN+2
        BCC @paint
        LDA COL+1           ; which place: the column / 64
        ASL A
        ASL A
        STA T0
        TXA
        ASL A
        ROL A
        ROL A
        AND #3
        ORA T0
        TAY
        AND #15
        TAX
        TYA
        AND #31
        TAY
        LDA COL+2
        BNE @far
        LDA COL+1
        CMP #2
        BCS @far
        LDA #0              ; nothing in the first 512
        BEQ @put
@far:   LDA places,Y
@put:   STA itype,X
        LDA T2
        STA ity,X
        CLC
        LDA COL
        ADC #1
        STA HIGEN
        LDA COL+1
        ADC #0
        STA HIGEN+1
        LDA COL+2
        ADC #0
        STA HIGEN+2
@paint: LDA #SHEET          ; the picture: one column of the strip, top to bottom
        STA NFLAGS
        LDA COL
        STA NDX
        LDA #0
        STA NDX+1
        STA NDW+1
        STA NDH+1
        STA NPATTERN
        LDA #1
        STA NDW
        STA NDY+1
        LDA COL             ; a tuft, on one column in eight or so
        LSR A
        LSR A
        LSR A
        EOR COL
        AND #7
        BEQ @tuft
        LDA #0
        BEQ @t
@tuft:  LDA #1
@t:     STA T3
        LDA #<GNDY          ; the sky above it: see-through
        STA NDY
        SEC
        LDA T2
        SBC #GNDTOP
        STA T4              ; the strip's line the ground starts on
        SBC T3
        STA NDH
        LDA #KEYC
        STA NCOLOUR
@f1:    BIT NSTATUS
        BVS @f1
        LDA #FILL
        STA NGO
        CLC                 ; the turf: two lines, or three with the tuft
        LDA NDH
        ADC #<GNDY
        STA NDY
        CLC
        LDA T3
        ADC #2
        STA NDH
        LDX #0              ; grass, or snow on the tops
        LDA T2
        CMP #SNOWY
        BCS @grass
        LDX #2
@grass: LDA turf,X
        STA NCOLOUR
@f2:    BIT NSTATUS
        BVS @f2
        LDA #FILL
        STA NGO
        CLC                 ; under it: three lines
        LDA T4
        ADC #<GNDY + 2
        STA NDY
        LDA #3
        STA NDH
        LDA turf+1,X
        STA NCOLOUR
@f3:    BIT NSTATUS
        BVS @f3
        LDA #FILL
        STA NGO
        CLC                 ; earth to the bottom: brown, flecked with orange
        LDA T4
        ADC #<GNDY + 5
        STA NDY
        SEC
        LDA #GNDH - 5
        SBC T4
        STA NDH
        LDA #$89
        STA NCOLOUR
        LDA #%00100000
        STA NPATTERN
@f4:    BIT NSTATUS
        BVS @f4
        LDA #FILL
        STA NGO
        LDA T4              ; and rock 16 lines down, where there's room for any
        CMP #GNDH - 16
        BCS @norock
        ADC #<GNDY + 16
        STA NDY
        SEC
        LDA #GNDH - 16
        SBC T4
        STA NDH
        LDA #$CB
        STA NCOLOUR
        LDA #%10000010
        STA NPATTERN
@f5:    BIT NSTATUS
        BVS @f5
        LDA #FILL
        STA NGO
@norock:
        RTS

turf:   .byte 13, 5         ; grass: light green over green
        .byte 1, 15         ; snow: white over light grey

; ---------------------------------------------------------------- the picture
picture:
        LDA #0              ; the sky and the far fells: an eighth of the camera
        STA NFLAGS
        STA NSY+1
        LDA CAM+1
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        STA T0
        LDA CAM
        LSR A
        LSR A
        LSR A
        ORA T0
        STA NSX
        LDA #FARY
        STA NSY
        LDA #160
        STA NSW
        STA NDWS
        LDA #FARH
        STA NSH
        STA NDHS
        LDA #0
        STA NDXS
        STA NDYS
@b1:    BIT NSTATUS
        BVS @b1
        LDA #BLIT
        STA NGO
        LDA #KEY            ; the near fells: a quarter
        STA NFLAGS
        LDA CAM+1
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        STA T0
        LDA CAM
        LSR A
        LSR A
        ORA T0
        STA NSX
        LDA #<NEARY
        STA NSY
        LDA #>NEARY
        STA NSY+1
        LDA #NEARH
        STA NSH
        STA NDHS
        LDA #NEARTOP
        STA NDYS
@b2:    BIT NSTATUS
        BVS @b2
        LDA #BLIT
        STA NGO
        LDA CAM             ; the ground: the ring from the camera's column, round the edge
        STA NSX
        LDA #<GNDY
        STA NSY
        LDA #GNDH
        STA NSH
        STA NDHS
        LDA #GNDTOP
        STA NDYS
@b3:    BIT NSTATUS
        BVS @b3
        LDA #BLIT
        STA NGO
        JSR things
        JSR moped
        JSR dust
        LDA STATE
        BNE @notitle
        JSR title
@notitle:
        RTS

; ---- the coins and the cans: four places are near enough to be on the page
things: LDA #0
        STA NSY
        STA NSY+1
        LDA CAM+1           ; the first place: the camera / 64
        ASL A
        ASL A
        STA T0
        LDA CAM
        ASL A
        ROL A
        ROL A
        AND #3
        ORA T0
        STA T1
        LDA CAM             ; its middle on the page: 32 less the camera's odd pixels
        AND #63
        STA T0
        SEC
        LDA #32
        SBC T0
        STA T2              ; -31 to 32, low byte
        LDA #0
        SBC #0
        STA T3              ; and high
        LDA #4
        STA T7
@place: LDA T1
        AND #15
        TAX
        LDA itype,X
        BNE @there
        JMP @next
@there: STA T4
        LDY #9              ; how high: 9 above the ground, or 30 for the ones to jump for
        CMP #3
        BNE @low
        LDY #30
@low:   STY T5
        SEC
        LDA ity,X
        SBC T5
        STA T5              ; its middle, down the page
        ; has the moped got it? Within 10 across and 13 up or down of the centre of mass
        LDA T3
        BNE @draw
        SEC
        LDA T2
        SBC #BX - 10
        CMP #21
        BCS @draw
        SEC
        LDA T5
        SBC BY
        CLC
        ADC #13
        CMP #27
        BCS @draw
        LDA STATE
        CMP #RIDE
        BNE @draw
        LDA #0
        STA itype,X
        LDA T4
        CMP #2
        BEQ @can
        LDA COINS
        CMP #99
        BCS @rich
        INC COINS
@rich:  JSR coinsnd
        JMP @next
@can:   CLC
        LDA GAS+1
        ADC #$70
        BCC @fill
        LDA #$FF
        STA GAS
@fill:  STA GAS+1
        JSR cansnd
        JMP @next
@draw:  LDA T4
        CMP #2
        BEQ @dcan
        LDA T1              ; a coin turns: drawn 8, 6, 4 and 2 wide and back
        ASL A
        ADC TICK
        LSR A
        LSR A
        AND #7
        TAY
        LDA #COINX
        STA NSX
        LDA #8
        STA NSW
        STA NSH
        STA NDHS
        LDA coinw,Y
        STA NDWS
        LSR A
        STA T6
        SEC
        LDA T5
        SBC #4
        STA NDYS
        JMP @at
@dcan:  LDA #CANX
        STA NSX
        LDA #8
        STA NSW
        STA NDWS
        LDA #10
        STA NSH
        STA NDHS
        LDA #4
        STA T6
        SEC
        LDA T5
        SBC #5
        STA NDYS
@at:    SEC                 ; its left edge: 16 bits, it can be off either side
        LDA T2
        SBC T6
        STA NDX
        LDA T3
        SBC #0
        STA NDX+1
@b:     BIT NSTATUS
        BVS @b
        LDA #BLIT
        STA NGO
@next:  INC T1
        CLC
        LDA T2
        ADC #64
        STA T2
        BCC @n1
        INC T3
@n1:    DEC T7
        BEQ @done
        JMP @place
@done:  RTS

coinw:  .byte 8, 6, 4, 2, 2, 4, 6, 8

; ---- the moped: three LINEs, two wheels, and the body at one of 64 angles
moped:  LDA Y0+2
        STA BY
        CLC                 ; which angle: the nearest 64th of a circle
        LDA TH+2
        ADC #2
        LSR A
        LSR A
        STA FI
        LDX #1
@hub:   CLC                 ; the hubs on the page: the spring's squeeze is what shows
        LDA X0+1
        ADC RXL,X
        LDA #BX
        ADC RXH,X
        STA WXS,X
        LDA CPL,X
        STA T0
        LDA CPH,X
        CMP #>CSTOP
        BCC @sq
        BNE @bot
        LDA T0
        CMP #<CSTOP
        BCC @sq1
@bot:   LDA #<CSTOP
        STA T0
@sq1:   LDA #>CSTOP
@sq:    STA T1
        SEC
        LDA RYL,X
        SBC T0
        STA T0
        LDA RYH,X
        SBC T1
        STA T1
        CLC
        LDA Y0+1
        ADC T0
        LDA Y0+2
        ADC T1
        STA WYS,X
        DEX
        BPL @hub
        LDA #0
        STA NFLAGS
        LDX FI
        LDA #12             ; the swingarm: from the back hub to the pivot under the seat
        STA NCOLOUR
        LDA WXS
        STA NDXS
        LDA WYS
        STA NDYS
        CLC
        LDA #BX
        ADC pvx,X
        SEC
        SBC WXS
        JSR linew
        CLC
        LDA BY
        ADC pvy,X
        SEC
        SBC WYS
        JSR lineh
        JSR goline
        INC NDYS            ; twice, for thickness
        JSR goline
        LDA #15             ; the shock: from the back hub up to the seat
        STA NCOLOUR
        LDA WYS
        STA NDYS
        CLC
        LDA #BX
        ADC skx,X
        SEC
        SBC WXS
        JSR linew
        CLC
        LDA BY
        ADC sky,X
        SEC
        SBC WYS
        JSR lineh
        JSR goline
        LDA WXS+1           ; the fork: from the front hub up to the headstock
        STA NDXS
        LDA WYS+1
        STA NDYS
        CLC
        LDA #BX
        ADC hsx,X
        SEC
        SBC WXS+1
        JSR linew
        CLC
        LDA BY
        ADC hsy,X
        SEC
        SBC WYS+1
        JSR lineh
        JSR goline
        INC NDXS
        JSR goline
        LDA #KEY            ; the wheels: which of two pictures by how far each has turned
        STA NFLAGS
        LDA #WHEELX
        STA NSX
        LDA #0
        STA NSY+1
        LDA #10
        STA NSW
        STA NSH
        LDX #1
@wheel: LDA WAH,X
        AND #4
        BEQ @w0
        LDA #10
@w0:    STA NSY
        LDA CPH,X           ; bottomed out: squashed, 12 wide and 8 high
        CMP #>CSTOP
        BCC @round
        BNE @flat
        LDA CPL,X
        CMP #<CSTOP
        BCC @round
@flat:  SEC
        LDA WXS,X
        SBC #6
        STA NDXS
        SEC
        LDA WYS,X
        SBC #3
        STA NDYS
        LDA #12
        STA NDWS
        LDA #8
        STA NDHS
        BNE @wgo
@round: SEC
        LDA WXS,X
        SBC #5
        STA NDXS
        SEC
        LDA WYS,X
        SBC #5
        STA NDYS
        LDA #10
        STA NDWS
        STA NDHS
@wgo:   BIT NSTATUS
        BVS @wgo
        LDA #BLIT
        STA NGO
        DEX
        BPL @wheel
        LDX FI              ; the body: one of 32 pictures, or the same a half turn round
        LDY #KEY
        TXA
        AND #32
        BEQ @half
        LDY #KEY | FLIPX | FLIPY
@half:  STY NFLAGS
        TXA
        AND #7
        ASL A
        ASL A
        ASL A
        ASL A
        ASL A
        STA NSX
        TXA
        AND #$18
        ASL A
        ASL A
        CLC
        ADC #<BODYY
        STA NSY
        LDA #>BODYY
        STA NSY+1
        LDA #32
        STA NSW
        STA NSH
        STA NDWS
        STA NDHS
        CLC
        LDA #BX
        ADC pcx,X
        STA NDXS
        CLC
        LDA BY
        ADC pcy,X
        STA NDYS
@bgo:   BIT NSTATUS
        BVS @bgo
        LDA #BLIT
        STA NGO
        RTS

; A LINE's width and height are signed, 16 bits: A = the low byte.
linew:  STA NDW
        LDY #0
        ORA #0
        BPL @p
        DEY
@p:     STY NDW+1
        RTS
lineh:  STA NDH
        LDY #0
        ORA #0
        BPL @p
        DEY
@p:     STY NDH+1
        RTS
goline: BIT NSTATUS
        BVS goline
        LDA #LINE
        STA NGO
        RTS

; ---- dust: thrown up by the back tyre when it slips, and by a hard landing
dust:   LDA STATE
        CMP #RIDE
        BNE @draw
        LDA NL              ; the back wheel on the ground,
        ORA NH
        BEQ @draw
        LDA THUMP
        BNE @throw          ;   landing,
        LDA TICK
        AND #1
        BNE @draw
        LDA SLIP+1          ;   or slipping by a sixth of a pixel a frame
        BMI @sn
        BNE @throw
        LDA SLIP
        CMP #40
        BCS @throw
        BCC @draw
@sn:    CMP #$FF
        BNE @throw
        LDA SLIP
        CMP #<[-40]
        BCS @draw
@throw: LDX DNEXT
        INX
        CPX #NDUST
        BCC @nx
        LDX #0
@nx:    STX DNEXT
        CLC                 ; from under the back wheel
        LDA X0+1
        ADC RXL
        STA dxl,X
        LDA XL
        ADC RXH
        STA dxh,X
        LDA #0
        STA dyl,X
        CLC
        LDA WYS
        ADC #4
        STA dyh,X
        JSR random          ; backwards if the tyre's driving, with the tyre's slip; and up
        AND #63
        STA T0
        SEC
        LDA #0
        SBC SLIP
        STA T1
        LDA #0
        SBC SLIP+1
        STA T2
        ASL T1
        ROL T2
        SEC
        LDA T1
        SBC T0
        STA dvxl,X
        LDA T2
        SBC #0
        STA dvxh,X
        JSR random
        AND #$7F
        STA dvyl,X
        LDA #$FF
        STA DVYH,X
        LDA #18
        STA DLIFE,X
        LDY #9              ; earth, or snow
        LDA GND+1
        CMP #SNOWY
        BCS @earth
        LDY #1
@earth: TYA
        STA dcol,X
@draw:  LDA #0
        STA NFLAGS
        STA NPATTERN
        STA NDX+1
        LDA #1
        STA NDHS
        LDX #NDUST-1
@speck: LDA DLIFE,X
        BEQ @nexts
        DEC DLIFE,X
        CLC
        LDA dxl,X
        ADC dvxl,X
        STA dxl,X
        LDA dxh,X
        ADC dvxh,X
        STA dxh,X
        CLC                 ; it falls, at a quarter of gravity: it's light
        LDA dvyl,X
        ADC #10
        STA dvyl,X
        BCC @g
        INC DVYH,X
@g:     CLC
        LDA dyl,X
        ADC dvyl,X
        STA dyl,X
        LDA dyh,X
        ADC DVYH,X
        STA dyh,X
        STA NDYS
        SEC
        LDA dxh,X
        SBC CAM
        CMP #160
        BCC @on
        LDA #0              ; off the page: gone
        STA DLIFE,X
        BEQ @nexts
@on:    STA NDX
        LDY #1              ; two wide while it's new
        LDA DLIFE,X
        CMP #10
        BCC @small
        INY
@small: STY NDWS
        LDA dcol,X
        STA NCOLOUR
@f:     BIT NSTATUS
        BVS @f
        LDA #FILL
        STA NGO
@nexts: DEX
        BPL @speck
        RTS

random: LDA RNG
        ASL A
        BCC @r
        EOR #$1D
@r:     STA RNG
        RTS

; ---- the title: twice its size, and its shadow first, through the black ink table
title:  LDA #WHEELX
        STA NSX
        LDA #LOGOY
        STA NSY
        LDA #0
        STA NSY+1
        LDA #54
        STA NSW
        LDA #7
        STA NSH
        LDA #108
        STA NDWS
        LDA #14
        STA NDHS
        LDA #KEY | SHADOW
        STA NFLAGS
        LDA #28
        STA NDXS
        LDA #12
        STA NDYS
        LDA #BLIT
        JSR go
        LDA #KEY
        STA NFLAGS
        LDA #26
        STA NDXS
        LDA #10
        STA NDYS
        LDA #BLIT
        JMP go

; ---------------------------------------------------------------- the words
; The panel: the top row of text, in front of a black band that's part of the sky's strip.
panel:  LDX #39
@p:     LDA paneltxt,X
        STA SCREEN,X
        DEX
        BPL @p
        RTS

; The rows under the panel, for the state the game is in.
words:  LDX #0
        LDA #' '
@clear: STA SCREEN + 5 * 40,X
        STA SCREEN + 8 * 40,X
        INX
        CPX #120
        BNE @clear
        LDA STATE
        CMP #TITLE
        BNE @nott
        LDX #39
@t:     LDA titl1,X
        STA SCREEN + 7 * 40,X
        LDA titl2,X
        STA SCREEN + 9 * 40,X
        DEX
        BPL @t
        RTS
@nott:  CMP #OVER
        BNE @done
        LDX #39
@o:     LDA over2,X
        STA SCREEN + 7 * 40,X
        LDA over3,X
        STA SCREEN + 9 * 40,X
        LDY WHY
        BNE @gas
        LDA over0,X
        JMP @w
@gas:   LDA over1,X
@w:     STA SCREEN + 5 * 40,X
        DEX
        BPL @o
        LDA DIST            ; how far, in the middle row
        STA T0
        LDA DIST+1
        STA T1
        LDA #<[SCREEN + 7 * 40 + 14]
        STA PTR
        LDA #>[SCREEN + 7 * 40 + 14]
        STA PTR+1
        JSR num4
        LDA #<[SCREEN + 7 * 40 + 28]
        STA PTR
        LDA COINS
        JMP num2
@done:  RTS

; The panel's numbers, when they change, and the gauge.
hud:    SEC                 ; metres: 16 pixels each, from the start
        LDA XL
        SBC #STARTX
        STA T0
        LDA XL+1
        SBC #0
        STA T1
        LDA XL+2
        SBC #0
        BCS @fwd
        LDA #0
        STA T0
        STA T1
@fwd:   STA T2
        LDX #4
@sh:    LSR T2
        ROR T1
        ROR T0
        DEX
        BNE @sh
        LDA T2
        BNE @cap
        LDA T0
        CMP #<10000
        LDA T1
        SBC #>10000
        BCC @dok
@cap:   LDA #<9999
        STA T0
        LDA #>9999
        STA T1
@dok:   LDA STATE           ; (the count stops when the run does)
        CMP #OVER
        BEQ @shown
        LDA T0
        STA DIST
        LDA T1
        STA DIST+1
@shown: LDA DIST
        CMP ODIST
        BNE @dist
        LDA DIST+1
        CMP ODIST+1
        BEQ @coins
@dist:  LDA DIST
        STA ODIST
        STA T0
        LDA DIST+1
        STA ODIST+1
        STA T1
        LDA #<SCREEN
        STA PTR
        LDA #>SCREEN
        STA PTR+1
        JSR num4
        LDA BEST            ; the best, as it's beaten
        STA T0
        LDA BEST+1
        STA T1
        CMP DIST+1
        BCC @beat
        BNE @best
        LDA T0
        CMP DIST
        BCS @best
@beat:  LDA DIST
        STA T0
        LDA DIST+1
        STA T1
@best:  LDA #<[SCREEN + 11]
        STA PTR
        JMP num4
@coins: LDA COINS
        CMP OCOINS
        BEQ @gas
        STA OCOINS
        LDA #<[SCREEN + 24]
        STA PTR
        LDA #>SCREEN
        STA PTR+1
        LDA COINS
        JMP num2
@gas:   CLC                 ; the gauge: sixteen halves of a square
        LDA GAS+1
        ADC #15
        ROR A
        LSR A
        LSR A
        LSR A
        CMP OGAS
        BEQ @done
        STA OGAS
        LDX #0
@cell:  CMP #2
        BCC @part
        LDY #$0F            ; a whole square
        SBC #2
        JMP @putc
@part:  CMP #1
        BCC @empty
        LDY #$05            ; its left half
        LDA #0
        BEQ @putc
@empty: LDY #'_'
@putc:  STA T0
        TYA
        STA SCREEN + 32,X
        LDA T0
        INX
        CPX #8
        BNE @cell
@done:  RTS

; T0,T1 as four digits at (PTR). The NIECE divides by ten: the quotient and the
; remainder are read before the next number goes in, because they change as it does.
num4:   LDA #0
        STA NDIVN+2
        STA NDIVN+3
        STA NDIVD+1
        LDA #10
        STA NDIVD
        LDY #3
@digit: LDA T0
        STA NDIVN
        LDA T1
        STA NDIVN+1
        LDA NREM
        ORA #'0'
        STA (PTR),Y
        LDA NQUOT
        STA T0
        LDA NQUOT+1
        STA T1
        DEY
        BPL @digit
        RTS
; A as two digits at (PTR).
num2:   STA NDIVN
        LDA #0
        STA NDIVN+1
        STA NDIVN+2
        STA NDIVN+3
        STA NDIVD+1
        LDA #10
        STA NDIVD
        LDY #0
        LDA NQUOT
        ORA #'0'
        STA (PTR),Y
        INY
        LDA NREM
        ORA #'0'
        STA (PTR),Y
        RTS

; ---------------------------------------------------------------- sound
sndinit:
        LDA #$1F            ; full volume, and the low-pass for the engine
        STA VOLUME
        LDA #1
        STA FILTER
        LDA #150
        STA CUTOFF
        LDA #2              ; voice 1, the engine: a sawtooth, held
        STA V1+3
        LDA #$F4
        STA V1+6
        LDA #1
        STA V1+4
        LDA #8              ; voice 2, the thump: noise that dies by itself
        STA V2+3
        LDA #$08
        STA V2+5
        LDA #0
        STA V2+6
        STA V3+6
        STA RING
        LDA #1              ; voice 3, the coin and the can: a triangle, plucked
        STA V3+3
        LDA #$07
        STA V3+5
        RTS

; The engine's note is the back tyre's speed: 55 Hz ticking over, an octave and a half
; up flat out, and a little more with the throttle open.
sound:  LDA SL
        STA T0
        LDA SH
        BPL @fwd
        SEC
        LDA #0
        SBC SL
        STA T0
        LDA #0
        SBC SH
@fwd:   STA T1
        ASL T0              ; x 4
        ROL T1
        ASL T0
        ROL T1
        CLC
        LDA T0
        ADC #<920
        STA T0
        LDA T1
        ADC #>920
        STA T1
        LDA PADS
        AND #2
        BEQ @shut
        CLC
        LDA T0
        ADC #<260
        STA T0
        LDA T1
        ADC #>260
        STA T1
@shut:  LDA T0
        STA V1
        LDA T1
        STA V1+1
        LDX #1              ; no gas, or the rider's off: the engine stops
        LDA GAS
        ORA GAS+1
        BEQ @dead
        LDA STATE
        CMP #DOWN
        BCC @runs
@dead:  LDX #0
@runs:  STX V1+4
        LDA THUMP           ; a wheel came down
        BEQ @nothump
        LDA STATE
        CMP #DOWN
        BCS @nothump
        LDA THUMP
        ASL A
        ASL A
        ADC #6
        STA V2+1
        LDA #0
        STA V2+4
        LDA #1
        STA V2+4
@nothump:
        LDA RING
        BEQ @quiet
        DEC RING
        BNE @quiet
        LDA #0
        STA V3+4
@quiet: RTS
coinsnd:
        LDA #$83            ; 2,000 Hz
        BNE pluck
cansnd: LDA #$28            ; 600
pluck:  STA V3+1
        LDA #0
        STA V3
        STA V3+4
        LDA #1
        STA V3+4
        LDA #6
        STA RING
        RTS
crashsnd:
        LDA #5
        STA V2+1
        LDA #$0B
        STA V2+5
        LDA #0
        STA V2+4
        LDA #1
        STA V2+4
        RTS

; ---------------------------------------------------------------- at the start
; The small pictures onto the sheet, through NVDATA: a row at a time.
upload: LDX #0
@pic:   LDA pics,X
        STA SRC
        LDA pics+1,X
        STA SRC+1
        LDA pics+2,X
        STA VA
        LDA pics+3,X
        STA VA+1
        LDA pics+4,X        ; bytes a row
        STA T0
        LDA pics+5,X        ; rows
        STA T1
@row:   LDA VA
        STA NVADDR
        LDA VA+1
        STA NVADDR+1
        LDY #0
@byte:  LDA (SRC),Y
        STA NVDATA
        INY
        CPY T0
        BNE @byte
        CLC
        LDA SRC
        ADC T0
        STA SRC
        BCC @s
        INC SRC+1
@s:     CLC
        LDA VA
        ADC #128
        STA VA
        BCC @v
        INC VA+1
@v:     DEC T1
        BNE @row
        TXA
        CLC
        ADC #6
        TAX
        CPX #PICSEND - pics
        BNE @pic
        RTS

pics:   .word wheelart, WHEELX / 2
        .byte 5, 20
        .word coinart, COINX / 2
        .byte 4, 8
        .word canart, CANX / 2
        .byte 4, 10
        .word sunart, SUNX / 2
        .byte 4, 8
        .word cloudart, CLOUDX / 2
        .byte 8, 6
        .word logoart, LOGOY * 128 + WHEELX / 2
        .byte 27, 7
PICSEND:

; The two strips of scenery, drawn by the chip on the sheet: bands of sky, a sun,
; clouds, and then a column at a time the far fells and the near ones, their tops
; from the same sine tables as the ground.
scenery:
        LDA #SHEET
        STA NFLAGS
        LDA #0
        STA NDX
        STA NDX+1
        STA NDW
        STA NDH+1
        LDA #1
        STA NDW+1           ; 256 wide
        LDX #0
@band:  LDA bands,X
        STA NDY
        LDA bands+1,X
        STA NDY+1
        LDA bands+2,X
        STA NDH
        LDA bands+3,X
        STA NCOLOUR
        LDA bands+4,X
        STA NPATTERN
        LDA #FILL
        JSR go
        TXA
        CLC
        ADC #5
        TAX
        CPX #BANDSEND - bands
        BNE @band
        LDA #0
        STA NPATTERN
        STA NDW+1
        STA NSY
        STA NSY+1
        LDA #SHEET | KEY    ; the sun, half as big again, and three clouds of three sizes
        STA NFLAGS
        LDX #0
@sky:   LDA skies,X
        STA NSX
        LDA skies+1,X
        STA NSW
        LDA skies+2,X
        STA NSH
        LDA skies+3,X
        STA NDX
        LDA skies+4,X
        STA NDY
        LDA skies+5,X
        STA NDW
        LDA skies+6,X
        STA NDH
        LDA #BLIT
        JSR go
        TXA
        CLC
        ADC #7
        TAX
        CPX #SKIESEND - skies
        BNE @sky
        LDA #SHEET
        STA NFLAGS
        LDA #1
        STA NDW
        LDA #0
        STA T5              ; the column
        STA T0              ; three angles, each stepping at its own rate
        STA T1
        STA T2
@col:   LDA T5
        STA NDX
        LDX T0              ; the far fells' top: line 38, give or take 24
        LDA wave1,X
        JSR quarter
        STA T3
        LDX T1
        LDA wave2,X
        JSR quarter
        CMP #$80
        ROR A
        CLC
        ADC T3
        STA T3
        LDX T2
        LDA wave3,X
        JSR quarter
        CLC
        ADC T3
        STA T3
        SEC
        LDA #38
        SBC T3
        STA T3
        CLC
        ADC #FARY
        STA NDY
        LDA #0
        ROL A
        STA NDY+1
        SEC
        LDA #FARH
        SBC T3
        STA NDH
        LDA #$CE            ; haze: light blue and grey, every other pixel
        STA NCOLOUR
        LDA #%01011010
        STA NPATTERN
        LDA #FILL
        JSR go
        LDA T3              ; snow on what stands above line 30: half way down to it
        CMP #30
        BCS @nosnow
        SEC
        LDA #31
        SBC T3
        LSR A
        STA NDH
        LDA #1
        STA NCOLOUR
        LDA #0
        STA NPATTERN
        LDA #FILL
        JSR go
@nosnow:
        LDX T1              ; the near fells: their top 1 to 13 lines down their strip
        LDA wave2,X
        EOR #$FF
        JSR quarter
        CMP #$80
        ROR A
        STA T4
        LDX T0
        LDA wave3,X
        JSR quarter
        CMP #$80
        ROR A
        CLC
        ADC T4
        CLC
        ADC #7
        STA T4
        LDA #<NEARY
        STA NDY
        LDA #>NEARY
        STA NDY+1
        LDA T4
        STA NDH
        LDA #KEYC
        STA NCOLOUR
        LDA #0
        STA NPATTERN
        LDA #FILL
        JSR go
        CLC
        LDA T4
        ADC #<NEARY
        STA NDY
        LDA #2
        STA NDH
        LDA #5
        STA NCOLOUR
        LDA #FILL
        JSR go
        CLC
        LDA T4
        ADC #<NEARY + 2
        STA NDY
        SEC
        LDA #NEARH - 2
        SBC T4
        STA NDH
        LDA #$B5            ; green and dark grey: bracken
        STA NCOLOUR
        LDA #%10100101
        STA NPATTERN
        LDA #FILL
        JSR go
        CLC                 ; whole turns in 256 columns, so each strip joins up with itself
        LDA T0
        ADC #2
        STA T0
        CLC
        LDA T1
        ADC #5
        STA T1
        CLC
        LDA T2
        ADC #11
        STA T2
        INC T5
        BEQ @done
        JMP @col
@done:  RTS
; A signed byte's quarter.
quarter:
        CMP #$80
        ROR A
        CMP #$80
        ROR A
        RTS

; y (two bytes), height, colours, pattern: the sky's strip from the top
bands:  .byte <FARY, 0, 4, 0, 0                         ; the panel's band: black,
        .byte <[FARY + 4], 0, 1, 11, 0                  ;   with a grey edge
        .byte <[FARY + 5], 0, 7, 6, 0                   ; blue,
        .byte <[FARY + 12], 0, 5, $E6, %01011010        ; blue and light blue,
        .byte <[FARY + 17], 0, 4, $6E, %10000010        ; light blue with a little blue,
        .byte <[FARY + 21], 0, 17, 14, 0                ; light blue,
        .byte <[FARY + 38], 0, 5, $1E, %10000010        ; whiter
        .byte <[FARY + 43], 0, 13, $1E, %01011010       ;   towards the fells
        .byte <[FARY + 56], >[FARY + 56], 8, 1, 0
BANDSEND:
; source x, width, height; to x, y (in the sky's strip), width, height
skies:  .byte SUNX, 8, 8, 200, FARY + 9, 12, 12
        .byte CLOUDX, 16, 6, 20, FARY + 12, 32, 9
        .byte CLOUDX, 16, 6, 96, FARY + 19, 24, 6
        .byte CLOUDX, 16, 6, 150, FARY + 10, 16, 6
SKIESEND:

; The body and rider at 32 angles, a 64th of a circle apart: 32 x 32 each, onto the
; sheet through NVDATA. For each pixel of a turned picture, which pixel of the
; drawing lands there: across the turned picture the place in the drawing moves by
; (cos, sin) a pixel, down it by (-sin, cos). The arithmetic unit finds where each
; picture's first pixel comes from. Three seconds.
ARTW    = 25
ARTH    = 20
spin:   LDA #0
        STA T7              ; which angle
@angle: LDA T7              ; its sine and cosine: 16 steps of the table to an angle
        ASL A
        ASL A
        ASL A
        ASL A
        TAY
        LDA T7
        LSR A
        LSR A
        LSR A
        LSR A
        TAX
        CLC
        ADC #>sinlo
        STA PTR+1
        TXA
        CLC
        ADC #>sinhi
        STA PTR2+1
        LDA #0
        STA PTR
        STA PTR2
        LDA (PTR),Y
        STA SN
        LDA (PTR2),Y
        STA SN+1
        INC PTR+1
        INC PTR2+1
        LDA (PTR),Y
        STA CS
        LDA (PTR2),Y
        STA CS+1
        ; the first pixel is (-15.5, -15.5) from the middle, and the drawing's middle
        ; is (12.5, 10): U0 = 12.5 - 15.5 cos + 15.5 sin, V0 = 10 - 15.5 sin - 15.5 cos
        LDA #31
        STA NMULB
        LDA #0
        STA NMULB+1
        LDA CS
        STA NMULA
        LDA CS+1
        STA NMULA+1
        LDA NPROD
        STA T0
        LDA NPROD+1
        CMP #$80
        ROR A
        ROR T0
        STA T1              ; 15.5 cos
        LDA SN
        STA NMULA
        LDA SN+1
        STA NMULA+1
        LDA NPROD
        STA T2
        LDA NPROD+1
        CMP #$80
        ROR A
        ROR T2
        STA T3              ; 15.5 sin
        SEC
        LDA #$80
        SBC T0
        STA U0
        LDA #12
        SBC T1
        STA U0+1
        CLC
        LDA U0
        ADC T2
        STA U0
        LDA U0+1
        ADC T3
        STA U0+1
        SEC
        LDA #0
        SBC T2
        STA V0
        LDA #10
        SBC T3
        STA V0+1
        SEC
        LDA V0
        SBC T0
        STA V0
        LDA V0+1
        SBC T1
        STA V0+1
        LDA T7              ; where on the sheet: eight to a row
        AND #7
        ASL A
        ASL A
        ASL A
        ASL A
        STA VA
        LDA T7
        AND #$18
        ASL A
        CLC
        ADC #>[BODYY * 128]
        STA VA+1
        LDA #32
        STA T6              ; rows to go
@row:   LDA VA
        STA NVADDR
        LDA VA+1
        STA NVADDR+1
        LDA U0
        STA UU
        LDA U0+1
        STA UU+1
        LDA V0
        STA VV
        LDA V0+1
        STA VV+1
        LDA #16
        STA T5              ; bytes to go: two pixels each
@pair:  JSR sample
        ASL A
        ASL A
        ASL A
        ASL A
        STA T4
        JSR sample
        ORA T4
        STA NVDATA
        DEC T5
        BNE @pair
        SEC                 ; the next row down
        LDA U0
        SBC SN
        STA U0
        LDA U0+1
        SBC SN+1
        STA U0+1
        CLC
        LDA V0
        ADC CS
        STA V0
        LDA V0+1
        ADC CS+1
        STA V0+1
        CLC
        LDA VA
        ADC #128
        STA VA
        BCC @v
        INC VA+1
@v:     DEC T6
        BNE @row
        INC T7
        LDA T7
        CMP #32
        BEQ @done
        JMP @angle
@done:  RTS
; The drawing's pixel at (UU, VV), or the see-through colour off its edge; then a
; pixel on across the turned picture.
sample: LDX #KEYC
        LDY VV+1
        CPY #ARTH
        BCS @got
        LDA UU+1
        CMP #ARTW
        BCS @got
        LDA artlo,Y
        STA PTR
        LDA arthi,Y
        STA PTR+1
        LDA UU+1
        LSR A
        TAY
        LDA (PTR),Y
        BCS @odd
        LSR A
        LSR A
        LSR A
        LSR A
@odd:   AND #15
        TAX
@got:   CLC
        LDA UU
        ADC CS
        STA UU
        LDA UU+1
        ADC CS+1
        STA UU+1
        CLC
        LDA VV
        ADC SN
        STA VV
        LDA VV+1
        ADC SN+1
        STA VV+1
        TXA
        RTS

artlo:  .rept ARTH, r
        .byte <[bikeart + r * 13]
        .endr
arthi:  .rept ARTH, r
        .byte >[bikeart + r * 13]
        .endr

; ---------------------------------------------------------------- the words themselves
nochip:   .asciiz "FELLRIDER NEEDS THE NIECE."
waitmsg:  .text "       KICKING THE MOPED OVER...        "
paneltxt: .text "0000M BEST 0000M  COINS 00  GAS ________"
titl1:    .text "  RIGHT GAS  LEFT BRAKE  UP DOWN LEAN   "
titl2:    .text "          PRESS SPACE TO RIDE           "
over0:    .text "             YOU CAME OFF!              "
over1:    .text "              OUT OF GAS                "
over2:    .text "         RODE 0000M   COINS 00          "
over3:    .text "       PRESS SPACE TO RIDE AGAIN        "

; ---------------------------------------------------------------- tables
; The wheels' and the head's numbers: back wheel, front wheel, head.
kl:     .byte <KWHEEL, <KWHEEL, <KHEAD
kh:     .byte >KWHEEL, >KWHEEL, >KHEAD
dl:     .byte <DWHEEL, <DWHEEL, <DHEAD
dh:     .byte >DWHEEL, >DWHEEL, >DHEAD
mul:    .byte <MUWHEEL, <MUWHEEL, <MUHEAD
muh:    .byte >MUWHEEL, >MUWHEEL, >MUHEAD
radl:   .byte <RWHEEL, <RWHEEL, <RHEAD
radh:   .byte >RWHEEL, >RWHEEL, >RHEAD

; What's in each of 32 places along the ground, 64 pixels apart, round and round:
; 0 nothing, 1 a coin, 2 a can, 3 a coin to jump for.
places: .byte 1, 1, 1, 0, 1, 3, 1, 0, 0, 1, 1, 2, 0, 1, 1, 1
        .byte 3, 0, 1, 1, 0, 1, 3, 1, 1, 0, 0, 2, 1, 1, 0, 3

; The ground's three waves: how many 65536ths of a circle each turns in a pixel, and
; a turn of each at its height.
GF1     = 218
GF2     = 596
GF3     = 1771
wave1:  .rept 256, i
        .byte sin(i, 60) & $FF
        .endr
wave2:  .rept 256, i
        .byte sin(i, 34) & $FF
        .endr
wave3:  .rept 256, i
        .byte sin(i, 16) & $FF
        .endr

; The ground's tangent for each slope, from -2 to 2 in 64ths: a vector 256 long.
tanxl:  .rept 256, i
        .byte <sqrt(268435456 / [4096 + [i - 128] * [i - 128]])
        .endr
tanxh:  .rept 256, i
        .byte >sqrt(268435456 / [4096 + [i - 128] * [i - 128]])
        .endr
tanyl:  .rept 256, i
        .byte <[sqrt(268435456 / [4096 + [i - 128] * [i - 128]]) * [i - 128] / 64]
        .endr
tanyh:  .rept 256, i
        .byte >[sqrt(268435456 / [4096 + [i - 128] * [i - 128]]) * [i - 128] / 64]
        .endr

; Where things are on the body at each of the 64 angles it's drawn at, from the centre
; of mass: the picture's corner, the headstock, the swingarm's pivot and the shock's
; top. A point u forward and v down the moped is (u cos + v sin, v cos - u sin).
pcx:    .rept 64, i
        .byte [[[1 - sin(i * 4, 9)] >> 1] - 16] & $FF
        .endr
pcy:    .rept 64, i
        .byte [[[1 - cos(i * 4, 9)] >> 1] - 16] & $FF
        .endr
hsx:    .rept 64, i
        .byte [[cos(i * 4, 16 * 6) + sin(i * 4, 16 * -5) + 8] >> 4] & $FF
        .endr
hsy:    .rept 64, i
        .byte [[cos(i * 4, 16 * -5) - sin(i * 4, 16 * 6) + 8] >> 4] & $FF
        .endr
pvx:    .rept 64, i
        .byte [[cos(i * 4, 16 * -1) + sin(i * 4, 16 * 3) + 8] >> 4] & $FF
        .endr
pvy:    .rept 64, i
        .byte [[cos(i * 4, 16 * 3) - sin(i * 4, 16 * -1) + 8] >> 4] & $FF
        .endr
skx:    .rept 64, i
        .byte [[cos(i * 4, 16 * -6) + sin(i * 4, 16 * -2) + 8] >> 4] & $FF
        .endr
sky:    .rept 64, i
        .byte [[cos(i * 4, 16 * -2) - sin(i * 4, 16 * -6) + 8] >> 4] & $FF
        .endr

; ---------------------------------------------------------------- the pictures
; A digit a pixel; 4 is see-through.
bikeart:
        .byte $44, $44, $44, $41, $11, $14, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $11, $11, $11, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $22, $22, $22, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $11, $11, $AA, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $11, $1A, $AA, $44, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $41, $1A, $A4, $44, $40, $00, $44, $44, $44, $44
        .byte $44, $44, $44, $22, $22, $22, $22, $22, $AF, $44, $44, $44, $44
        .byte $44, $44, $42, $22, $22, $24, $44, $44, $4F, $44, $44, $44, $44
        .byte $44, $44, $42, $22, $22, $24, $46, $64, $44, $F4, $44, $44, $44
        .byte $44, $44, $42, $22, $22, $46, $66, $66, $44, $F4, $44, $44, $44
        .byte $44, $44, $42, $22, $26, $66, $66, $67, $77, $77, $4F, $14, $44
        .byte $44, $00, $00, $00, $00, $00, $06, $61, $11, $11, $07, $F4, $44
        .byte $20, $00, $00, $00, $00, $00, $06, $67, $77, $77, $00, $44, $44
        .byte $FF, $FF, $FF, $F4, $CC, $CC, $C6, $6C, $C0, $44, $44, $44, $44
        .byte $0F, $44, $44, $44, $CF, $CC, $C6, $6C, $04, $44, $44, $44, $44
        .byte $4F, $FF, $F4, $44, $CC, $FC, $C0, $00, $C4, $44, $44, $44, $44
        .byte $44, $44, $FF, $FF, $CC, $CC, $C0, $00, $04, $44, $44, $44, $44
        .byte $44, $44, $44, $4F, $FB, $CC, $B4, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $40, $BB, $04, $44, $44, $44, $44, $44, $44
        .byte $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44, $44
wheelart:
        .byte $44, $40, $BB, $04, $44
        .byte $40, $BC, $CC, $CB, $04
        .byte $4B, $C4, $F4, $4C, $B4
        .byte $0C, $44, $F4, $44, $C0
        .byte $BC, $FF, $FF, $FF, $CB
        .byte $BC, $44, $FF, $44, $CB
        .byte $0C, $44, $F4, $44, $C0
        .byte $4B, $C4, $F4, $4C, $B4
        .byte $40, $BC, $CC, $CB, $04
        .byte $44, $40, $BB, $04, $44
        .byte $44, $4B, $00, $B4, $44
        .byte $4B, $0C, $CC, $C0, $B4
        .byte $40, $CF, $44, $FC, $04
        .byte $BC, $4F, $44, $F4, $CB
        .byte $0C, $44, $FF, $44, $C0
        .byte $0C, $44, $FF, $44, $C0
        .byte $BC, $4F, $44, $F4, $CB
        .byte $40, $CF, $44, $FC, $04
        .byte $4B, $0C, $CC, $C0, $B4
        .byte $44, $4B, $00, $B4, $44
coinart:
        .byte $44, $77, $77, $44
        .byte $47, $71, $17, $74
        .byte $77, $17, $77, $87
        .byte $77, $17, $77, $87
        .byte $77, $17, $77, $87
        .byte $77, $17, $77, $87
        .byte $47, $77, $78, $84
        .byte $44, $88, $88, $44
canart:
        .byte $44, $00, $4F, $F4
        .byte $40, $22, $0F, $44
        .byte $02, $22, $22, $20
        .byte $02, $22, $22, $20
        .byte $02, $77, $77, $24
        .byte $02, $71, $17, $20
        .byte $02, $77, $77, $20
        .byte $02, $22, $22, $20
        .byte $02, $22, $22, $20
        .byte $40, $00, $00, $04
logoart:
        .byte $11, $11, $14, $11, $11, $14, $14, $44, $44, $14, $44, $44, $11, $11, $44, $11, $11, $14, $11, $11, $44, $11, $11, $14, $11, $11, $44
        .byte $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $14, $44, $14, $44, $14, $44, $14, $14, $44, $44, $14, $44, $14
        .byte $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $14, $44, $14, $44, $14, $44, $14, $14, $44, $44, $14, $44, $14
        .byte $11, $11, $44, $11, $11, $44, $14, $44, $44, $14, $44, $44, $11, $11, $44, $44, $14, $44, $14, $44, $14, $11, $11, $44, $11, $11, $44
        .byte $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $14, $44, $44, $14, $44, $14, $44, $14, $14, $44, $44, $14, $14, $44
        .byte $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $44, $44, $14, $41, $44, $44, $14, $44, $14, $44, $14, $14, $44, $44, $14, $41, $44
        .byte $14, $44, $44, $11, $11, $14, $11, $11, $14, $11, $11, $14, $14, $44, $14, $11, $11, $14, $11, $11, $44, $11, $11, $14, $14, $44, $14
sunart:
        .byte $44, $77, $77, $44
        .byte $47, $77, $77, $74
        .byte $77, $71, $17, $77
        .byte $77, $11, $77, $77
        .byte $77, $17, $77, $77
        .byte $77, $77, $77, $77
        .byte $47, $77, $77, $74
        .byte $44, $77, $77, $44
cloudart:
        .byte $44, $44, $44, $11, $11, $44, $44, $44
        .byte $44, $41, $11, $11, $11, $11, $14, $44
        .byte $41, $11, $11, $11, $11, $11, $11, $14
        .byte $11, $11, $11, $11, $11, $11, $11, $11
        .byte $4F, $F1, $11, $11, $11, $11, $1F, $F4
        .byte $44, $4F, $FF, $FF, $FF, $FF, $F4, $44

; The sine, 1,024 to the circle and a quarter turn over for the cosine, 256 for 1:
; the assembler's sine is 256 to the circle, so three in four are between two of its.
        .align 256
sinlo:  .rept 1280, i
        .byte <[[[4 - i % 4] * sin(i / 4, 1024) + [i % 4] * sin(i / 4 + 1, 1024) + 8] >> 4]
        .endr
sinhi:  .rept 1280, i
        .byte >[[[4 - i % 4] * sin(i / 4, 1024) + [i % 4] * sin(i / 4 + 1, 1024) + 8] >> 4]
        .endr

; ---------------------------------------------------------------- memory
HGL:    .res 256            ; the ground's height, a column of the ring each: 256ths,
HGH:    .res 256            ;   and the page's line
itype:  .res 16             ; what's at each place (the place's number and 15), and
ity:    .res 16             ;   the ground's line there
dxl:    .res NDUST          ; the dust: where (the world's low byte, and 256ths),
dxh:    .res NDUST
dyl:    .res NDUST
dyh:    .res NDUST
dvxl:   .res NDUST          ;   how fast,
dvxh:   .res NDUST
dvyl:   .res NDUST
dcol:   .res NDUST          ;   and its colour
