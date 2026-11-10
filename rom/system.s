; UNC 64 SYSTEM ROM, Rev A. $E000-$FFFF.
; Boots into UNC BASIC Rev A, a small integer BASIC (manual chapter 2).
; Programs call the screen, keyboard and number routines through the jump table at $FF00,
; so the ROM can change underneath them.

SCREEN  = $0400
CURX    = $0200         ; cursor column 0-39
CURY    = $0201         ; cursor row 0-24
TMPA    = $0202
RAMTOP  = $0203         ; $40 on a 16K machine, $C0 on 64K (high byte of first byte past RAM)
RUNFLG  = $0204         ; nonzero while a stored BASIC program runs, 0 in direct mode
CURLIN  = $0205         ; 2 bytes: number of the BASIC line being run
FORSP   = $0207         ; bytes in use on the FOR stack
GOSP    = $0208         ; bytes in use on the GOSUB stack
PRGEND  = $0209         ; 2 bytes: address of the program's end marker (a zero length byte)
SEED    = $020B         ; 2 bytes: RND state
INKM    = $020D         ; INK: 1 draws, 0 erases, 2 flips
GFXS    = $020E         ; graphics scratch
CURSAV  = $020F         ; the screen code under the line editor's cursor
GOSTK   = $0210         ; GOSUB stack: 16 frames x 5 bytes, $0210-$025F
VARS    = $0260         ; A-Z: 26 x 2 bytes, $0260-$0293
FORSTK  = $0294         ; FOR stack: 8 frames x 10 bytes, $0294-$02E3
TNAME   = $02E4         ; LOAD, SAVE: the name, 16 characters, space-padded
TNAMED  = $02F4         ; LOAD: nonzero if a name was given
THDR    = $02F5         ; LOAD, SAVE: the header, 24 bytes: version, type, name (16),
TLOAD   = $0307         ;   load address,
TLEN    = $0309         ;   length,
TENTRY  = $030B         ;   entry address
TCHK    = $030D         ; LOAD, SAVE: the bytes so far, XORed
TOVR    = $030E         ; LOAD: $40 if a byte went by unread
TBORD   = $030F         ; LOAD, SAVE: the border colour before
DATAP   = $0310         ; READ: 2 bytes, where the next DATA item starts; high byte 0 before
                        ;   the first READ and after RESTORE (and RUN, NEW)
DATAL   = $0312         ; READ: 2 bytes, the line that DATA item is on
IRQVEC  = $0314         ; IRQ handler, called through RAM so programs can hook it
NMIVEC  = $0316         ; and the NMI's: the RESTORE key
IBUF    = $0320         ; the line you type: 120 characters + 0, $0320-$0398
INBUF   = $03A0         ; INPUT's line: 40 characters + 0, $03A0-$03C8
NUMBUF  = $03D0         ; NUMSTR's eight digits, $03D0-$03D7
RLCOL   = $03D8         ; READLN: the column and
RLROW   = $03D9         ;   the row it began at (SCROLL keeps the row right)
RLMOV   = $03F3         ; READLN: not 0 once the cursor has been moved, so the line is
                        ;   the screen's and no longer just what was typed
LINK    = $03DA         ; 25 bytes, one a row, $03DA-$03F2: 1 if the row carries on the
                        ;   line of the row above (it wrapped), 0 if a line starts on it.
                        ;   Row 0's is always 0.
K_INS   = 20            ; the INSERT key
SEEDBOOT = $ACE1        ; RND's start-up seed
PROG    = $0800         ; BASIC program text starts here

; Fast page. BASIC uses $D0-$F6; the screen routines use $F7-$FE.
PX      = $D0           ; graphics: the pixel to plot (2 bytes each, signed)
PY      = $D2
GP      = $D4           ; graphics: the bitmap byte's address
GX0     = $D6           ; graphics: circle centre
GY0     = $D8
GE      = $DA           ; graphics: line and circle error term
TPTR    = $D0           ; LOAD: where the next byte goes (shares the graphics bytes)
TCNT    = $D2           ; LOAD: bytes still to come
LFROM   = $DC           ; LIST range; LINE's end point; CIRCLE's x and y
LTO     = $DE
TXT     = $E0           ; the interpreter's text pointer
LINP    = $E2           ; the program line being run
ACC     = $E4           ; expression accumulator
ARG     = $E6           ; second operand
NUML    = ACC           ; NUMSTR's number, 3 bytes: $E4-$E6
NUMM    = ACC+1
NUMH    = ARG
TMP     = $E8
CNT     = $EA
DISP    = $EC           ; dispatch / SYS address
LP      = $EE           ; line pointer for searching and editing
T1      = $F0
T2      = $F1
T3      = $F2           ; 2 bytes
BUFP    = $F4           ; READLN's buffer
BUFN    = $F6           ; READLN's maximum length
SRC     = $F7           ; fast-page pointers reserved for the screen routines: $F7-$FE
DST     = $F9
PTR     = $FB
STRP    = $FD

KBD     = $C010
KBDSTRB = $C011
KEYS8   = $C068         ; the keyboard matrix's row 8: the cursor keys in bits 0-3
BORDER  = $C020
BG      = $C021
IRQST   = $C005
VMODE   = $C000
BMREG   = $C023         ; high byte of the bitmap's address
TROWS   = $C024         ; TEXTROWS: the text rows under the bitmap in mixed mode, 4 at reset
FRAME   = $C006         ; frame counter: one up every frame
SPEAKER = $C031         ; bit 0 the timer's gate, bit 1 the speaker's data
TIMER_LO = $C032        ; the timer's divisor
TIMER_HI = $C033
TAPE    = $C040         ; tape deck: bit 0 PLAY, 1 tape in, 2 motor, 3 RECORD, 4 room for a
                        ; byte (recording), 6 overrun, 7 byte ready (playing)
TDATA   = $C041         ; the byte off the tape, reading takes it; recording, the next byte
MOTOR   = $04           ; TAPE bit 2: write it to run the motor
RECPLAY = $09           ; TAPE bits 0 and 3: RECORD and PLAY down
ROOM    = $10           ; TAPE bit 4: recording, and TDATA is empty
IRQEN   = $C004
CHARSET = $C022
SPRTAB  = $C0A0         ; the video board's sprite table's page: 0 for no sprites
SCROLLX = $C0A8
SCROLLY = $C0A9
LAYER   = $C0AA
COLMODE = $C0B0
COLOUR3 = $C0B3         ; the colour of text
VBOARD  = $C0BF         ; the video board's id: $15 with the board in, $FF without

; Tokens: $80 + position in KWTAB. Statements first, in STTAB order.
T_PRINT = $80
T_REM   = $8A
NSTMT   = 24
T_THEN  = $98
T_TO    = $99
T_STEP  = $9A
T_PEEK  = $9B
T_RND   = $9C
T_ABS   = $9D
T_AND   = $9E
T_OR    = $9F
T_POINT = $A0
T_LOAD  = $A1           ; after the functions, so the older tokens keep their numbers
T_SAVE  = $A2
T_DATA  = $A3           ; DATA, READ, RESTORE and GET: after SAVE, for the same reason
T_READ  = $A4
T_RESTORE = $A5
T_GET   = $A6

E_SYN   = 0
E_UNDEF = 1
E_NEXT  = 2
E_RET   = 3
E_DIV   = 4
E_OOM   = 5
E_LOAD  = 6
E_DATA  = 7

        .org $E000
RESET:  LDX #$FF
        TXS
        JSR VECS
        STA RUNFLG
        LDA #$40        ; probe for the 64K expansion: does $4000 hold a value?
        STA RAMTOP
        LDA #$55
        STA $4000
        LDA $4000
        CMP #$55
        BNE R16K
        LDA #$C0
        STA RAMTOP
R16K:   JSR NEW0
        JSR SEED0
        JSR CLS
        LDX #<BANNER
        LDY #>BANNER
        JSR PRSTR
        LDA RAMTOP
        CMP #$C0
        BEQ R64K
        LDX #<MSG16
        LDY #>MSG16
        JMP RMSG
R64K:   LDX #<MSG64
        LDY #>MSG64
RMSG:   JSR PRSTR
        LDA VBOARD      ; the video board answers $15
        CMP #$15
        BNE RBAS
        LDX #<MSGVB
        LDY #>MSGVB
        JSR PRSTR
RBAS:   LDX #<MSGBAS
        LDY #>MSGBAS
        JSR PRSTR
        JSR FREE
        JSR PRUNS
        LDX #<MSGFREE
        LDY #>MSGFREE
        JSR PRSTR
        CLI
        JSR CRIF        ; READY., and with it the switch-on beep
        LDX #<MREADY
        LDY #>MREADY
        JSR PRSTR
        JSR CHIME
        JMP MAIN

; CHIME: the switch-on beep, 300 ms of 880 Hz from the timer (divisor 1,000,000 / 880 = 1,136),
; then silence: the gate and the speaker off, as they were.
CHIME:  LDA #<1136
        STA TIMER_LO
        LDA #>1136
        STA TIMER_HI
        LDA #%11        ; gate on, speaker data on: the timer plays
        STA SPEAKER
        LDX #15         ; 15 frames at 50 a second
CHW:    LDA FRAME
CHT:    CMP FRAME
        BEQ CHT
        DEX
        BNE CHW
        LDA #0
        STA SPEAKER
        RTS

; ---------------------------------------------------------------- screen and keyboard

; CLS: clear the screen, cursor home. Changes A and X.
CLS:    LDA #$20
        LDX #0
CLSL:   STA SCREEN,X
        STA SCREEN+$100,X
        STA SCREEN+$200,X
        STA SCREEN+$2E8,X
        INX
        BNE CLSL
        LDA #0
        STA CURX
        STA CURY
        LDX #24         ; every row starts a line
CLSK:   STA LINK,X
        DEX
        BPL CLSK
        RTS

; CHROUT: print the character in A. 13 = new line, 12 = clear screen, 8 = backspace.
; Draws no cursor: the line editor shows one while it waits (CURON). Leaves PTR at the
; cursor's row. Preserves A, X, Y.
CHROUT: STA TMPA
        PHA
        TXA
        PHA
        TYA
        PHA
        LDA TMPA
        CMP #13
        BEQ NEWLN
        CMP #12
        BEQ DOCLS0
        CMP #8
        BEQ BKSP
        JSR CURPTR
        LDY CURX
        LDA TMPA
        STA (PTR),Y
        INC CURX
        LDA CURX
        CMP #40
        BNE CDONE1      ; still on this row: PTR is right
        LDX CURY        ; it wrapped: the next row carries the line on, unless this is
        LDA LINK,X      ; the line's third row already (a line is three rows at most)
        BEQ WRAP1
        LDA LINK-1,X
WRAP1:  EOR #1
        BPL NEWL1
NEWLN:  LDA #0          ; a new line starts on the next row
NEWL1:  PHA
        LDA #0
        STA CURX
        INC CURY
        LDA CURY
        CMP #25
        BNE NEWL2
        JSR SCROLL
        DEC CURY
NEWL2:  LDX CURY
        PLA
        STA LINK,X
        JMP CDONE
BKSP:   LDA CURX
        BEQ CDONE
        DEC CURX
        JMP CDONE
DOCLS0: JSR CLS
CDONE:  JSR CURPTR
CDONE1: PLA
        TAY
        PLA
        TAX
        PLA
        RTS

; CURON: show the line editor's cursor (a reversed space, which the video chip blinks)
; at CURX/CURY, keeping what was under it in CURSAV. Changes A and X; keeps Y.
CURON:  TYA
        PHA
        JSR CURPTR
        LDY CURX
        LDA (PTR),Y
        STA CURSAV
        LDA #$A0
        STA (PTR),Y
        PLA
        TAY
        RTS

; CUROFF: take the cursor away, putting back what was under it. Changes X; keeps A and Y.
CUROFF: PHA
        TYA
        PHA
        JSR CURPTR
        LDY CURX
        LDA CURSAV
        STA (PTR),Y
        PLA
        TAY
        PLA
        RTS

; CURPTR: PTR = start of the cursor's row. Changes X.
CURPTR: LDX CURY
        LDA ROWLO,X
        STA PTR
        LDA ROWHI,X
        STA PTR+1
        RTS

; SCROLL: move rows 1-24 up one row, clear row 24, and the rows' links with them.
; The caller says whether row 24 carries a line on (LINK+24). Changes A, X, Y, SRC, DST.
SCROLL: LDX #0
SCROW:  JSR ROWSD       ; SRC = row X, DST = the row below
        LDA LINK+1,X
        STA LINK,X
        LDY #39
SCOPY:  LDA (DST),Y
        STA (SRC),Y
        DEY
        BPL SCOPY
        INX
        CPX #24
        BNE SCROW
        LDA #$20
        LDY #39
SCLR:   STA (DST),Y
        DEY
        BPL SCLR
        LDA #0          ; row 0 starts a line now
        STA LINK
        DEC RLROW       ; and the row READLN began on has moved up too
        RTS

; GETKEY: wait for a key, return its code in A.
GETKEY: LDA KBD
        BPL GETKEY
        AND #$7F
        BIT KBDSTRB
        RTS

; PRSTR: print the zero-terminated string at X (low) / Y (high). Up to 255 characters.
PRSTR:  STX STRP
        STY STRP+1
        LDY #0
PSLOOP: LDA (STRP),Y
        BEQ PSDONE
        JSR CHROUT
        INY
        BNE PSLOOP
PSDONE: RTS

; PRHEX: print A as two hex digits.
PRHEX:  PHA
        LSR A
        LSR A
        LSR A
        LSR A
        JSR HEXD
        PLA
        AND #$0F
HEXD:   CMP #10
        BCC HEXD1
        ADC #6
HEXD1:  ADC #$30
        JMP CHROUT

; VECS: the interrupt vectors back to the ROM's own handlers, the border and background
; black. Leaves A = 0.
VECS:   LDA #<DEFIRQ
        STA IRQVEC
        LDA #>DEFIRQ
        STA IRQVEC+1
        LDA #<DEFNMI
        STA NMIVEC
        LDA #>DEFNMI
        STA NMIVEC+1
        LDA #0
        STA BORDER
        STA BG
        RTS

; The RESTORE key pulls the processor's NMI line. Alone it does nothing. With STOP held
; (bit 7 of the matrix's row 8) it's the way out of anything: the machine's registers go
; back to the way they are at switch-on (text, the ROM's characters, no interrupts, no
; sound, the tape's motor off, the video board's sprites, layer, scroll and colour mode
; off and its text white), the screen is cleared, and BASIC says READY. with the program
; and its variables as they were.
NMI:    JMP (NMIVEC)
DEFNMI: BIT KEYS8
        BMI RESTORE
        RTI
RESTORE: JSR VECS
        STA VMODE
        STA IRQEN
        STA SPEAKER
        STA TAPE
        STA SPRTAB
        STA LAYER
        STA COLMODE
        STA SCROLLX
        STA SCROLLY
        LDA #$FF
        STA IRQST
        LDA #$D0
        STA CHARSET
        LDA #1
        STA COLOUR3
        BIT KBDSTRB     ; (STOP itself isn't a key press BASIC should see)
        JSR CLS
        JMP WARM

IRQ:    JMP (IRQVEC)
DEFIRQ: PHA             ; acknowledge every source, or the IRQ line stays high forever
        LDA #$FF
        STA IRQST
        PLA
        RTI

; CRIF: start a new line unless the cursor is already at the left edge.
CRIF:   LDA CURX
        BEQ CRIFX
        LDA #13
        JSR CHROUT
CRIFX:  RTS

; READLN: the screen editor. The cursor goes anywhere on the screen, and RETURN hands
; over the line it is on, as the screen shows it: up to three rows, read into (BUFP), at
; most BUFN characters, without the spaces after its end, zero-terminated. On the line
; READLN began on, that's from the column it began at (after INPUT's prompt). A line
; typed without moving the cursor is taken as typed (RLEND). Carry set if
; the STOP key (3) was pressed instead. The cursor shows only while it waits for a key.
READLN: LDA CURX
        STA RLCOL
        LDA CURY
        STA RLROW
        LDA #0
        STA RLMOV
RLKEY:  JSR CURON
        JSR GETKEY
        JSR CUROFF      ; (PTR = the cursor's row)
        CMP #13
        BEQ RLEND
        CMP #3
        BNE RLK1
        JMP RLSTOP
RLK1:   CMP #8
        BNE RLK2
        JMP RLDEL
RLK2:   CMP #K_INS
        BNE RLK3
        JMP RLINS
RLK3:   CMP #$20
        BCS RLK4
        JMP RLCUR
RLK4:   CMP #$60
        BCS RLKEY
        PHA             ; a character
        LDX CURY
        LDA CURX
        CMP #39
        BNE RLC2
        CPX #24         ; at the right edge: the line needs the next row
        BEQ RLC1
        LDA LINK+1,X
        BNE RLC2        ; and has it
RLC1:   JSR GROW
        BCC RLC2
        PLA             ; three rows, full: the last cell takes it and the cursor stays
        LDY #39         ; (so the cursor no longer marks the end of what was typed)
        STA (PTR),Y
        STA RLMOV
        BNE RLKEY
RLC2:   PLA
        JSR CHROUT
        JMP RLKEY

; RETURN. If the cursor was never moved, the line is what was typed: from where READLN
; began to the cursor, whatever else a program left on the row. Once it has been moved,
; the line is the screen's: the whole line the cursor is on.
RLEND:  LDX CURY
        LDA RLMOV
        BNE RLSCR
        LDA CURX        ; the cursor, counted from the start of the row READLN began on
RLT0:   CPX RLROW
        BEQ RLT1
        CLC
        ADC #40
        DEX
        BPL RLT0
RLT1:   STA T1          ; T1: the cell after the last one to read
        LDA ROWLO,X
        STA PTR
        LDA ROWHI,X
        STA PTR+1
        LDY RLCOL
        BPL RLE2
RLSCR:  LDA LINK,X      ; back to the line's first row
        BEQ RLS1
        DEX
        BNE RLSCR
RLS1:   STX CURY
        JSR LEND        ; PTR = the first row, T1 = the line's last cell
        INC T1
        LDY #0
        LDA CURY
        STX CURY        ; the cursor leaves from the line's last row
        CMP RLROW
        BNE RLE2
        LDY RLCOL       ; on the row READLN began on: from where it began
RLE2:   STY T2          ; T2: the next cell to read
        LDA #0
        STA T3          ; T3: characters stored
        STA T3+1        ; T3+1: of them, up to the last that isn't a space
RLE3:   LDY T2
        CPY T1
        BCS RLE5
        LDA (PTR),Y
        AND #$7F        ; reversed reads as plain
        INC T2
        LDY T3
        CPY BUFN
        BCS RLE5
        STA (BUFP),Y
        INY
        STY T3
        CMP #' '
        BEQ RLE3
        STY T3+1
        BNE RLE3
RLE5:   LDY T3+1
        CLC
RLOUT:  LDA #0
        STA (BUFP),Y
        PHP
        LDA #13
        JSR CHROUT
        PLP
        RTS
RLSTOP: JSR LEND
        STX CURY
        LDY #0
        SEC
        BCS RLOUT

; DELETE: rub out the character before the cursor, and pull the rest of the line left.
RLDEL:  LDX CURY
        LDA CURX
        BNE RLD1
        LDA LINK,X      ; at the left edge: back onto the row above, if it's the same line
        BEQ RLD4
        DEC CURY
        LDA #40
        STA CURX
RLD1:   DEC CURX
        JSR LEND
        LDY CURX
RLD2:   CPY T1
        BEQ RLD3
        INY
        LDA (PTR),Y
        DEY
        STA (PTR),Y
        INY
        BNE RLD2
RLD3:   LDA #' '
        STA (PTR),Y
RLD4:   JMP RLKEY

; INSERT: open a space at the cursor, pushing the rest of the line right. A line full to
; its last cell takes another row; one of three rows doesn't open.
RLINS:  STA RLMOV
        JSR LEND
        LDY T1
        LDA (PTR),Y
        CMP #' '
        BEQ RLI1
        JSR GROW
        BCS RLD4
        JSR LEND
        LDY T1
RLI1:   CPY CURX
        BEQ RLD3
        DEY
        LDA (PTR),Y
        INY
        STA (PTR),Y
        DEY
        BPL RLI1

; The cursor keys, 28-31: left, right, up, down. They stop at the screen's edges, and
; under GR at the first row you can see.
RLCUR:  STA RLMOV
        CMP #29
        BEQ RLRT
        BCS RLVERT
        CMP #28
        BNE RLD4
        LDA CURX        ; left
        BNE RLL1
        JSR TOPROW
        BCS RLD4
        DEC CURY
        LDA #40
        STA CURX
RLL1:   DEC CURX
        JMP RLKEY
RLRT:   LDA CURX        ; right
        CMP #39
        BNE RLR1
        LDA CURY
        CMP #24
        BEQ RLD4
        INC CURY
        LDA #$FF
        STA CURX
RLR1:   INC CURX
        JMP RLKEY
RLVERT: CMP #31
        BEQ RLDN
        JSR TOPROW      ; up
        BCS RLD4
        DEC CURY
        JMP RLKEY
RLDN:   LDA CURY        ; down
        CMP #24
        BEQ RLD4
        INC CURY
        JMP RLKEY

; TOPROW: carry set if the cursor is on the top row: 0, or under GR the first that shows.
TOPROW: LDA VMODE
        AND #6
        CMP #6
        BNE TOP0
        JSR GRTOP
        BNE TOP1
TOP0:   LDA #0
TOP1:   CMP CURY
        RTS

; LEND: PTR = the cursor's row; T1 = the last cell of its line, counted from PTR (39, 79 or
; 119: only the rows from the cursor's on count); X = that row. Changes A and Y.
LEND:   JSR CURPTR
        LDA #39
LEN1:   CPX #24
        BEQ LEN2
        LDY LINK+1,X
        BEQ LEN2
        INX
        CLC
        ADC #40
        CMP #119
        BNE LEN1
LEN2:   STA T1
        RTS

; GROW: give the line whose last row is X one more row, blank: the rows below move down
; (the last one off the screen), or at the bottom everything moves up, the cursor with
; it. Carry set, and nothing done, if the line has three rows. Changes everything but
; CURX; PTR is stale after it.
GROW:   LDA LINK,X
        BEQ GROW1
        LDA LINK-1,X
        BNE GROWNO
GROW1:  CPX #24
        BNE GROW2
        JSR SCROLL
        DEC CURY
        LDX #23
        BNE GROW6
GROW2:  STX T2
        LDX #24
GROW3:  DEX             ; rows 23 down to T2+1 each move to the row below
        CPX T2
        BEQ GROW5
        JSR ROWSD
        LDA LINK,X
        STA LINK+1,X
        LDY #39
GROW4:  LDA (SRC),Y
        STA (DST),Y
        DEY
        BPL GROW4
        BMI GROW3
GROW5:  INX             ; and row T2+1 is blank
        JSR ROWSD
        LDY #39
        LDA #' '
GROW7:  STA (SRC),Y
        DEY
        BPL GROW7
        DEX
GROW6:  LDA #1
        STA LINK+1,X
        CLC
        RTS
GROWNO: SEC
        RTS

; ROWSD: SRC = row X, DST = the row below it.
ROWSD:  LDA ROWLO,X
        STA SRC
        LDA ROWHI,X
        STA SRC+1
        LDA ROWLO+1,X
        STA DST
        LDA ROWHI+1,X
        STA DST+1
        RTS

; ---------------------------------------------------------------- the shell

READY:  LDA TROWS       ; READY. and the cursor want two rows: a picture with fewer text
        CMP #2          ; rows under it gets GR's four back, so they can be seen
        BCS RDY1
        LDA #4
        STA TROWS
RDY1:   JSR CRIF
        LDX #<MREADY
        LDY #>MREADY
        JSR PRSTR
RDYW:   LDA KEYS8       ; a game may end with a cursor key still held to steer it: wait
        AND #$0F        ; for it to be let go, and forget it, or the editor's cursor
        BNE RDYW        ; would wander off with it
        LDA KBD         ; (a key not yet taken has bit 7 set)
        CMP #$80+28
        BCC MAIN
        CMP #$80+32
        BCS MAIN
        BIT KBDSTRB
MAIN:   LDX #$FF
        TXS
        LDA #0
        STA RUNFLG
        LDA #<IBUF
        STA BUFP
        LDA #>IBUF
        STA BUFP+1
        LDA #120
        STA BUFN
        JSR READLN
        BCS MAIN        ; STOP while typing: start the line again
        JSR CRUNCH
        LDA #<IBUF
        STA TXT
        LDA #>IBUF
        STA TXT+1
        JSR PEEKC
        BEQ MAIN        ; an empty line
        CMP #'0'
        BCC DIRECT
        CMP #'9'+1
        BCS DIRECT
        JSR EDITLN      ; a numbered line: store it, no READY
        JMP MAIN
DIRECT: JMP STMT

; CRUNCH: turn keywords in IBUF into tokens, in place.
CRUNCH: LDX #0          ; read position
        LDY #0          ; write position
CRLOOP: LDA IBUF,X
        BEQ CRDONE
        CMP #'"'
        BEQ CRQUOT
        CMP #'?'
        BEQ CRQMARK
        CMP #'A'
        BCC CRCOPY
        CMP #'Z'+1
        BCS CRCOPY
        STY CNT
        JSR KWMATCH
        LDY CNT
        BCC CRCHR
        STA IBUF,Y
        INY
        CMP #T_REM
        BEQ CRREST      ; a remark is kept exactly as typed
        JMP CRLOOP
CRQMARK: LDA #T_PRINT   ; ? is short for PRINT
        STA IBUF,Y
        INX
        INY
        JMP CRLOOP
CRCHR:  LDA IBUF,X
CRCOPY: STA IBUF,Y
        INX
        INY
        JMP CRLOOP
CRQUOT: STA IBUF,Y      ; copy a string to its closing quote
        INX
        INY
CRQ1:   LDA IBUF,X
        BEQ CRDONE
        STA IBUF,Y
        INX
        INY
        CMP #'"'
        BNE CRQ1
        JMP CRLOOP
CRREST: LDA IBUF,X
        STA IBUF,Y
        BEQ CRDX
        INX
        INY
        JMP CRREST
CRDONE: STA IBUF,Y
CRDX:   RTS

; KWMATCH: is there a keyword at IBUF+X? If so: carry set, A = token, X past it.
; If not: carry clear, X unchanged.
KWMATCH: STX T2
        TXA
        CLC
        ADC #<IBUF
        STA T3
        LDA #>IBUF
        ADC #0
        STA T3+1
        LDA #<KWTAB
        STA TMP
        LDA #>KWTAB
        STA TMP+1
        LDA #$80
        STA T1
KWNEXT: LDY #0
        LDA (TMP),Y
        BEQ KWFAIL      ; end of the table
KWCMP:  LDA (TMP),Y
        AND #$7F
        CMP (T3),Y
        BNE KWSKIP
        LDA (TMP),Y
        BMI KWHIT       ; the last letter matched too
        INY
        JMP KWCMP
KWHIT:  INY
        TYA
        CLC
        ADC T2
        TAX
        LDA T1
        SEC
        RTS
KWSKIP: LDA (TMP),Y     ; step to the next keyword
        BMI KWSK2
        INY
        JMP KWSKIP
KWSK2:  INY
        TYA
        CLC
        ADC TMP
        STA TMP
        LDA TMP+1
        ADC #0
        STA TMP+1
        INC T1
        JMP KWNEXT
KWFAIL: LDX T2
        CLC
        RTS

; KWPRINT: print the keyword for token A. Changes A, X, Y.
KWPRINT: SEC
        SBC #$80
        TAX
        LDA #<KWTAB
        STA TMP
        LDA #>KWTAB
        STA TMP+1
        LDY #0
KWP1:   CPX #0
        BEQ KWPOUT
KWP2:   LDA (TMP),Y
        INY
        CMP #$80
        BCC KWP2
        DEX
        JMP KWP1
KWPOUT: LDA (TMP),Y
        PHA
        AND #$7F
        JSR CHROUT
        INY
        PLA
        BPL KWPOUT
        RTS

; ---------------------------------------------------------------- the line editor

; EDITLN: TXT points at a line number. Store, replace or delete that line.
EDITLN: JSR PARSEDEC
        JSR PEEKC
        LDY #0
EL1:    LDA (TXT),Y
        BEQ EL2
        INY
        JMP EL1
EL2:    STY T2          ; length of the text
        JSR FINDLN
        BCC EL3
        JSR DELLN
EL3:    LDA T2
        BEQ ELX         ; just a number: the line is gone
        JSR INSLN
ELX:    RTS

; FINDLN: LP = the first line numbered ACC or more. Carry set if it's exactly ACC.
FINDLN: LDA #<PROG
        STA LP
        LDA #>PROG
        STA LP+1
FL1:    LDY #0
        LDA (LP),Y
        BEQ FLNO
        LDY #2
        LDA (LP),Y
        CMP ACC+1
        BCC FLNXT
        BNE FLNO
        DEY
        LDA (LP),Y
        CMP ACC
        BCC FLNXT
        BNE FLNO
        SEC
        RTS
FLNXT:  LDY #0
        LDA (LP),Y
        CLC
        ADC LP
        STA LP
        BCC FL1
        INC LP+1
        JMP FL1
FLNO:   CLC
        RTS

; DELLN: remove the line at LP.
DELLN:  LDY #0
        LDA (LP),Y
        STA T1
        CLC
        ADC LP
        STA SRC
        LDA LP+1
        ADC #0
        STA SRC+1
        LDA LP
        STA DST
        LDA LP+1
        STA DST+1
        SEC
        LDA PRGEND
        SBC SRC
        STA CNT
        LDA PRGEND+1
        SBC SRC+1
        STA CNT+1
        INC CNT
        BNE DL1
        INC CNT+1
DL1:    JSR MOVEDN
        SEC
        LDA PRGEND
        SBC T1
        STA PRGEND
        LDA PRGEND+1
        SBC #0
        STA PRGEND+1
        RTS

; INSLN: insert line number ACC with the T2 bytes of text at TXT, at LP.
; A line is: length, number low, number high, text, 0.
INSLN:  LDA T2
        CLC
        ADC #4
        STA T1
        CLC
        LDA PRGEND
        ADC T1
        STA DST
        LDA PRGEND+1
        ADC #0
        STA DST+1
        JSR MEMTOP
        STA T3
        LDA DST+1
        CMP T3
        BCC IL1
        LDX #E_OOM
        JMP ERROR
IL1:    LDA PRGEND
        STA SRC
        LDA PRGEND+1
        STA SRC+1
        SEC
        LDA PRGEND
        SBC LP
        STA CNT
        LDA PRGEND+1
        SBC LP+1
        STA CNT+1
        INC CNT
        BNE IL2
        INC CNT+1
IL2:    JSR MOVEUP
        CLC
        LDA PRGEND
        ADC T1
        STA PRGEND
        LDA PRGEND+1
        ADC #0
        STA PRGEND+1
        LDY #0
        LDA T1
        STA (LP),Y
        INY
        LDA ACC
        STA (LP),Y
        INY
        LDA ACC+1
        STA (LP),Y
        CLC
        LDA LP
        ADC #3
        STA DST
        LDA LP+1
        ADC #0
        STA DST+1
        LDY #0
IL3:    LDA (TXT),Y
        STA (DST),Y
        BEQ IL4
        INY
        JMP IL3
IL4:    RTS

; MOVEDN: copy CNT bytes from SRC to DST, first byte first (for DST < SRC).
MOVEDN: LDY #0
MD1:    LDA CNT
        ORA CNT+1
        BEQ MDX
        LDA (SRC),Y
        STA (DST),Y
        INC SRC
        BNE MD2
        INC SRC+1
MD2:    INC DST
        BNE MD3
        INC DST+1
MD3:    LDA CNT
        BNE MD4
        DEC CNT+1
MD4:    DEC CNT
        JMP MD1
MDX:    RTS

; MOVEUP: copy CNT bytes ending at SRC to end at DST, last byte first (for DST > SRC).
MOVEUP: LDY #0
MV1:    LDA CNT
        ORA CNT+1
        BEQ MVX
        LDA (SRC),Y
        STA (DST),Y
        LDA SRC
        BNE MV2
        DEC SRC+1
MV2:    DEC SRC
        LDA DST
        BNE MV3
        DEC DST+1
MV3:    DEC DST
        LDA CNT
        BNE MV4
        DEC CNT+1
MV4:    DEC CNT
        JMP MV1
MVX:    RTS

; MEMTOP: A = the page BASIC's program must stay below: the bitmap's while it's on,
; otherwise the top of memory.
MEMTOP: LDA VMODE
        AND #2
        BEQ MT1
        LDA BMREG
        RTS
MT1:    LDA RAMTOP
        RTS

; NEW0: forget the program and the variables.
NEW0:   LDA #<PROG
        STA PRGEND
        LDA #>PROG
        STA PRGEND+1
        LDA #0
        STA PROG
CLRVARS: LDA #0
        LDX #51
CV1:    STA VARS,X
        DEX
        BPL CV1
        STA FORSP
        STA GOSP
        STA DATAP+1     ; and READ starts again from the first DATA
        LDA #1
        STA INKM
        RTS

; FREE: ACC = bytes free between the program and the top of memory.
FREE:   SEC
        LDA #0
        SBC PRGEND
        STA ACC
        LDA RAMTOP
        SBC PRGEND+1
        STA ACC+1
        LDA ACC         ; less one for the end marker
        BNE FR0
        DEC ACC+1
FR0:    DEC ACC
        RTS

; ---------------------------------------------------------------- the interpreter

; PEEKC: skip spaces; A = the next character (not taken). Z set at the end of the line.
PEEKC:  LDY #0
PK1:    LDA (TXT),Y
        CMP #' '
        BNE PK2
        INC TXT
        BNE PK1
        INC TXT+1
        JMP PK1
PK2:    CMP #0
        RTS

; NEXTC: step past the current character. Keeps A.
NEXTC:  INC TXT
        BNE NCX
        INC TXT+1
NCX:    RTS

; GETC: take the next character after spaces. Z set (and nothing taken) at the end.
GETC:   JSR PEEKC
        BEQ GCX
        INC TXT
        BNE GCX0
        INC TXT+1
GCX0:   CMP #0
GCX:    RTS

; UNGETC: step back one character.
UNGETC: LDA TXT
        BNE UG1
        DEC TXT+1
UG1:    DEC TXT
        RTS

; EXPECT: the next character must be A, or it's a syntax error.
EXPECT: STA T2
        JSR GETC
        CMP T2
        BNE SYNERR
        RTS

SYNERR: LDX #E_SYN
ERROR:  STX T1
        JSR CRIF
        LDA #'?'
        JSR CHROUT
        LDX T1
        LDA ERRHI,X
        TAY
        LDA ERRLO,X
        TAX
        JSR PRSTR
ERRTAIL: LDA RUNFLG
        BEQ ERT1
        LDX #<MIN
        LDY #>MIN
        JSR PRSTR
        LDA CURLIN
        STA ACC
        LDA CURLIN+1
        STA ACC+1
        JSR PRUNS
ERT1:   LDA #13
        JSR CHROUT
        LDA #0
        STA RUNFLG
        JMP READY

; CHKSTOP: if the STOP key (3) is waiting, take it and break.
CHKSTOP: LDA KBD
        BPL CSX
        AND #$7F
        CMP #3
        BNE CSX
        BIT KBDSTRB
DOBREAK: JSR CRIF
        LDX #<MBREAK
        LDY #>MBREAK
        JSR PRSTR
        JMP ERRTAIL
CSX:    RTS

; STMT: run statements from TXT. The CPU stack is reset before each one.
STMT:   LDX #$FF
        TXS
        JSR CHKSTOP
        JSR GETC
        BEQ ENDLINE
        CMP #':'
        BEQ STMT
        CMP #$80
        BCS STTOK
        JSR UNGETC      ; no keyword: an assignment, as if LET were there
        JSR DOLET
        JMP STMTEND
STTOK:  CMP #T_LOAD
        BNE STTOK1
        JMP DOLOAD      ; never comes back: the program it loads starts instead
STTOK1: CMP #$80+NSTMT
        BCS STSAVE      ; past the statements: SAVE, or not a statement
        ASL A
        TAX
        LDA STTAB,X
        STA DISP
        LDA STTAB+1,X
        STA DISP+1
        JSR CALLD
STMTEND: JSR PEEKC
        BEQ ENDLINE
        CMP #':'
        BEQ STMT
STSYN:  JMP SYNERR
CALLD:  JMP (DISP)
STSAVE: CMP #T_SAVE     ; out of the way, so the statements before it run as fast as ever
        BNE STDATA
        JSR DOSAVE
        JMP STMTEND
STDATA: CMP #T_DATA
        BNE STREAD
        JMP ENDLINE     ; DATA is the rest of its line: READ takes it, running skips it
STREAD: CMP #T_READ
        BNE STREST
        JSR DOREAD
        JMP STMTEND
STREST: CMP #T_RESTORE
        BNE STSYN
        JSR DORESTORE
        JMP STMTEND

; ENDLINE: the line is done. In a program, go on to the next line.
ENDLINE: LDA RUNFLG
        BNE NXTLN
        JMP READY
NXTLN:  LDY #0
        LDA (LINP),Y
        CLC
        ADC LINP
        STA LINP
        BCC GOLINE
        INC LINP+1
; GOLINE: run the program from the line at LINP.
GOLINE: LDY #0
        LDA (LINP),Y
        BEQ PRGDONE
        INY
        LDA (LINP),Y
        STA CURLIN
        INY
        LDA (LINP),Y
        STA CURLIN+1
        CLC
        LDA LINP
        ADC #3
        STA TXT
        LDA LINP+1
        ADC #0
        STA TXT+1
        LDA #1
        STA RUNFLG
        JMP STMT
PRGDONE: LDA #0
        STA RUNFLG
        JMP READY

; GETVAR: take a variable letter. X = its offset in VARS.
GETVAR: JSR GETC
        CMP #'A'
        BCC GVE
        CMP #'Z'+1
        BCS GVE
        SEC
        SBC #'A'
        ASL A
        TAX
        RTS
GVE:    JMP SYNERR

; ---------------------------------------------------------------- statements

DOLET:  JSR GETVAR
        TXA
        PHA
        LDA #'='
        JSR EXPECT
        JSR EXPR
        PLA
        TAX
        LDA ACC
        STA VARS,X
        LDA ACC+1
        STA VARS+1,X
        RTS

DOPRINT:
PRLOOP: JSR PEEKC
        BEQ PRCR
        CMP #':'
        BEQ PRCR
        CMP #';'
        BNE PRL1
        JSR NEXTC
        JSR PEEKC
        BEQ PRX         ; a ; at the end: no new line
        CMP #':'
        BEQ PRX
        JMP PRLOOP
PRL1:   CMP #','
        BNE PRL2
        JSR NEXTC
        LDA #' '
        JSR CHROUT
PRTB1:  LDA CURX        ; pad to the next tenth column
PRTB2:  CMP #10
        BCC PRTB3
        SBC #10
        JMP PRTB2
PRTB3:  CMP #0
        BEQ PRTBX
        LDA #' '
        JSR CHROUT
        JMP PRTB1
PRTBX:  JSR PEEKC
        BEQ PRX
        CMP #':'
        BEQ PRX
        JMP PRLOOP
PRL2:   CMP #'"'
        BNE PRL3
        JSR NEXTC
PRS1:   LDY #0
        LDA (TXT),Y
        BEQ PRCR
        CMP #'"'
        BEQ PRS2
        JSR CHROUT
        JSR NEXTC
        JMP PRS1
PRS2:   JSR NEXTC
        JMP PRLOOP
PRL3:   JSR EXPR
        JSR PRSIGNED
        JMP PRLOOP
PRCR:   LDA #13
        JSR CHROUT
PRX:    RTS

DOINPUT: JSR PEEKC
        CMP #'"'
        BNE IN1
        JSR NEXTC
IN0:    LDY #0          ; print the prompt
        LDA (TXT),Y
        BEQ INSYN
        CMP #'"'
        BEQ IN0X
        JSR CHROUT
        JSR NEXTC
        JMP IN0
INSYN:  JMP SYNERR
IN0X:   JSR NEXTC
        LDA #';'
        JSR EXPECT
IN1:    JSR GETVAR
        TXA
        PHA
INAGAIN: LDA #'?'
        JSR CHROUT
        LDA #' '
        JSR CHROUT
        LDA #<INBUF
        STA BUFP
        LDA #>INBUF
        STA BUFP+1
        LDA #40
        STA BUFN
        JSR READLN
        BCC IN2
        JMP DOBREAK
IN2:    LDA TXT
        PHA
        LDA TXT+1
        PHA
        LDA #<INBUF
        STA TXT
        LDA #>INBUF
        STA TXT+1
        JSR PARSEINT
        PLA
        STA TXT+1
        PLA
        STA TXT
        BCC IN3
        LDX #<MREDO
        LDY #>MREDO
        JSR PRSTR
        JMP INAGAIN
IN3:    PLA
        TAX
        LDA ACC
        STA VARS,X
        LDA ACC+1
        STA VARS+1,X
        RTS

DOIF:   JSR EXPR
        LDA #T_THEN
        JSR EXPECT
        LDA ACC
        ORA ACC+1
        BNE IFTRUE
        JMP ENDLINE     ; false: skip the rest of the line
IFTRUE: JSR PEEKC
        CMP #'0'
        BCC IFST
        CMP #'9'+1
        BCS IFST
        JMP DOGOTO
IFST:   JMP STMT

DOGOTO: JSR EXPR
GOTOACC: JSR FINDLN
        BCS GT1
        LDX #E_UNDEF
        JMP ERROR
GT1:    LDA LP
        STA LINP
        LDA LP+1
        STA LINP+1
        JMP GOLINE

DOGOSUB: JSR EXPR
        LDX GOSP
        CPX #80
        BCC GS1
        LDX #E_OOM
        JMP ERROR
GS1:    LDA LINP
        STA GOSTK,X
        LDA LINP+1
        STA GOSTK+1,X
        LDA TXT
        STA GOSTK+2,X
        LDA TXT+1
        STA GOSTK+3,X
        LDA RUNFLG
        STA GOSTK+4,X
        TXA
        CLC
        ADC #5
        STA GOSP
        JMP GOTOACC

DORET:  LDA GOSP
        BNE RT1
        LDX #E_RET
        JMP ERROR
RT1:    SEC
        SBC #5
        STA GOSP
        TAX
        LDA GOSTK,X
        STA LINP
        LDA GOSTK+1,X
        STA LINP+1
        LDA GOSTK+2,X
        STA TXT
        LDA GOSTK+3,X
        STA TXT+1
        LDA GOSTK+4,X
        STA RUNFLG
        BEQ RT2
        JSR SETCUR
RT2:    RTS

; SETCUR: CURLIN = the number of the line at LINP.
SETCUR: LDY #1
        LDA (LINP),Y
        STA CURLIN
        INY
        LDA (LINP),Y
        STA CURLIN+1
        RTS

; A FOR frame: variable, limit (2), step (2), LINP (2), TXT (2), RUNFLG.
DOFOR:  JSR GETVAR
        TXA
        PHA
        LDA #'='
        JSR EXPECT
        JSR EXPR
        PLA
        TAX
        PHA
        LDA ACC
        STA VARS,X
        LDA ACC+1
        STA VARS+1,X
        LDA #T_TO
        JSR EXPECT
        JSR EXPR
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        LDA #1
        STA ACC
        LDA #0
        STA ACC+1
        JSR PEEKC
        CMP #T_STEP
        BNE FR1
        JSR NEXTC
        JSR EXPR
FR1:    PLA
        STA ARG+1
        PLA
        STA ARG
        PLA
        STA T1
        JSR FINDFOR     ; a loop on the same variable starts over
        BCC FR2
        STX FORSP
FR2:    LDX FORSP
        CPX #80
        BCC FR3
        LDX #E_OOM
        JMP ERROR
FR3:    LDA T1
        STA FORSTK,X
        LDA ARG
        STA FORSTK+1,X
        LDA ARG+1
        STA FORSTK+2,X
        LDA ACC
        STA FORSTK+3,X
        LDA ACC+1
        STA FORSTK+4,X
        LDA LINP
        STA FORSTK+5,X
        LDA LINP+1
        STA FORSTK+6,X
        LDA TXT
        STA FORSTK+7,X
        LDA TXT+1
        STA FORSTK+8,X
        LDA RUNFLG
        STA FORSTK+9,X
        TXA
        CLC
        ADC #10
        STA FORSP
        RTS

; FINDFOR: find the FOR frame for variable T1. Carry set and X = its offset if found.
FINDFOR: LDX #0
FF1:    CPX FORSP
        BCS FFNO
        LDA FORSTK,X
        CMP T1
        BEQ FFYES
        TXA
        CLC
        ADC #10
        TAX
        JMP FF1
FFYES:  SEC
        RTS
FFNO:   CLC
        RTS

DONEXT: JSR PEEKC
        CMP #'A'
        BCC NX0
        CMP #'Z'+1
        BCS NX0
        JSR GETVAR
        STX T1
        JSR FINDFOR
        BCC NXERR
        JMP NX1
NX0:    LDX FORSP
        BEQ NXERR
        TXA
        SEC
        SBC #10
        TAX
NX1:    TXA             ; drop any loops inside this one
        CLC
        ADC #10
        STA FORSP
        LDY FORSTK,X    ; the variable
        CLC
        LDA VARS,Y
        ADC FORSTK+3,X
        STA VARS,Y
        STA ARG
        LDA VARS+1,Y
        ADC FORSTK+4,X
        STA VARS+1,Y
        STA ARG+1
        LDA FORSTK+1,X
        STA ACC
        LDA FORSTK+2,X
        STA ACC+1
        JSR CMPS        ; 1: below the limit, 2: at it, 4: past it
        LDY FORSTK+4,X
        BMI NXNEG
        AND #3          ; counting up: go on while var <= limit
        JMP NXTEST
NXNEG:  AND #6          ; counting down: go on while var >= limit
NXTEST: BEQ NXDONE
        LDA FORSTK+5,X
        STA LINP
        LDA FORSTK+6,X
        STA LINP+1
        LDA FORSTK+7,X
        STA TXT
        LDA FORSTK+8,X
        STA TXT+1
        LDA FORSTK+9,X
        STA RUNFLG
        BEQ NXR
        JSR SETCUR
NXR:    RTS
NXDONE: STX FORSP       ; the loop is finished: pop it
        RTS
NXERR:  LDX #E_NEXT
        JMP ERROR

DOPOKE: JSR EXPR
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        LDA #','
        JSR EXPECT
        JSR EXPR
        PLA
        STA ARG+1
        PLA
        STA ARG
        LDY #0
        LDA ACC
        STA (ARG),Y
        RTS

DOREM:  JMP ENDLINE

DOEND:  LDA #0
        STA RUNFLG
        JMP READY

; CLS: clears the screen. With GR on (mixed mode, the bitmap where GR put it), that's
; the picture too, and the cursor goes to the first text row, as after GR. A bitmap a
; program pointed somewhere else is left alone: it might be on top of anything.
DOCLS:  LDA VMODE
        AND #6
        CMP #6
        BNE CLS0
        LDA RAMTOP
        SEC
        SBC #$20
        CMP BMREG
        BNE CLS0
        JMP GRCLR
CLS0:   LDA #12
        JMP CHROUT

DOSYS:  JSR EXPR
        LDA ACC
        STA DISP
        LDA ACC+1
        STA DISP+1
        JMP CALLD       ; the machine code's RTS returns to STMTEND

DORUN:  JSR CLRVARS
        LDA #<PROG
        STA LINP
        LDA #>PROG
        STA LINP+1
        JMP GOLINE

DONEW:  JSR NEW0
        JMP READY

; HELP: one screen of help. It needs the whole screen, so graphics go off first, as TEXT.
DOHELP: LDA VMODE
        AND #$F9
        STA VMODE
        LDA #12
        JSR CHROUT
        LDA #<MHELP
        STA LP
        LDA #>MHELP
        STA LP+1
HLP1:   LDY #0
        LDA (LP),Y
        BEQ HLPX
        CMP #9
        BNE HLP2
        LDA #17         ; 9: on to column 17
        STA CURX
        BNE HLP4
HLP2:   CMP #$81        ; a keyword's token
        BCC HLP3
        CMP #T_GET+1
        BCS HLP3
        JSR KWPRINT
        JMP HLP4
HLP3:   JSR CHROUT
HLP4:   INC LP
        BNE HLP1
        INC LP+1
        BNE HLP1
HLPX:   RTS

; LIST [from][-[to]]
DOLIST: LDA #0
        STA LFROM
        STA LFROM+1
        LDA #$FF
        STA LTO
        STA LTO+1
        JSR PEEKC
        BEQ LS2
        CMP #':'
        BEQ LS2
        CMP #'-'
        BEQ LS1
        JSR PARSEDEC
        LDA ACC
        STA LFROM
        STA LTO
        LDA ACC+1
        STA LFROM+1
        STA LTO+1
        JSR PEEKC
        CMP #'-'
        BNE LS2
        LDA #$FF
        STA LTO
        STA LTO+1
LS1:    JSR NEXTC
        JSR PEEKC
        CMP #'0'
        BCC LS2
        CMP #'9'+1
        BCS LS2
        JSR PARSEDEC
        LDA ACC
        STA LTO
        LDA ACC+1
        STA LTO+1
LS2:    LDA #<PROG
        STA LP
        LDA #>PROG
        STA LP+1
LS3:    JSR CHKSTOP
        LDY #0
        LDA (LP),Y
        BEQ LSX
        INY
        LDA (LP),Y
        STA ACC
        INY
        LDA (LP),Y
        STA ACC+1
        CMP LFROM+1     ; below the range: skip
        BCC LSNEXT
        BNE LS4
        LDA ACC
        CMP LFROM
        BCC LSNEXT
LS4:    LDA LTO+1       ; past the range: done
        CMP ACC+1
        BCC LSX
        BNE LS5
        LDA LTO
        CMP ACC
        BCC LSX
LS5:    JSR PRUNS
        LDA #' '
        JSR CHROUT
        LDY #3
LS6:    LDA (LP),Y
        BEQ LS8
        BMI LSTOK
        JSR CHROUT
        INY
        JMP LS6
LSTOK:  STY T3
        JSR KWPRINT
        LDY T3
        INY
        JMP LS6
LS8:    LDA #13
        JSR CHROUT
LSNEXT: LDY #0
        LDA (LP),Y
        CLC
        ADC LP
        STA LP
        BCC LS3
        INC LP+1
        JMP LS3
LSX:    RTS

; ---------------------------------------------------------------- graphics

; XYARGS: read "x,y" into PX, PY.
XYARGS: JSR EXPR
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        LDA #','
        JSR EXPECT
        JSR EXPR
        LDA ACC
        STA PY
        LDA ACC+1
        STA PY+1
        PLA
        STA PX+1
        PLA
        STA PX
        RTS

; GR [n]: bitmap on, below it n rows of text (four if n isn't given, none for 0); both
; cleared.
DOGR:   LDA RAMTOP      ; the bitmap goes in the top 8K of memory
        SEC
        SBC #$20
        STA T1
        LDA PRGEND+1    ; the program has to end below it
        CMP T1
        BCC GR1
        LDX #E_OOM
        JMP ERROR
GR1:    LDA T1
        STA BMREG
        LDA #4          ; GR alone: four rows of text under the picture
        STA ACC
        JSR PEEKC       ; GR n: n rows, 0 for none (TEXTROWS takes 25 at most)
        BEQ GR2
        CMP #':'
        BEQ GR2
        JSR EXPR
GR2:    LDA ACC
        STA TROWS
        JSR GRCLR
        LDA #1
        STA INKM
        LDA VMODE
        ORA #6
        STA VMODE
        RTS

; GRCLR: clears the bitmap at BMREG and the text, cursor on the first text row. GR and CLS.
GRCLR:  LDA BMREG
        STA GP+1
        LDA #0
        STA GP
        TAY
        LDX #31         ; clear 31 pages and 64 bytes: 8,000 bytes
GRC1:   STA (GP),Y
        INY
        BNE GRC1
        INC GP+1
        DEX
        BNE GRC1
GRC2:   STA (GP),Y
        INY
        CPY #64
        BNE GRC2
        LDA #12
        JSR CHROUT
        JSR GRTOP       ; the cursor moves to the first text row you can see
        STA CURY
        JMP CURPTR

; GRTOP: A = the first text row that shows under the bitmap, 25 - TEXTROWS: 21 for GR's
; four. With no rows, 24: the cursor has to be somewhere.
GRTOP:  LDA #25
        SEC
        SBC TROWS
        CMP #25
        BCC GRTOPX
        LDA #24
GRTOPX: RTS

; TEXT: back to the text screen, cleared.
DOTEXT: LDA VMODE
        AND #$F9
        STA VMODE
        LDA #12
        JMP CHROUT

DOINK:  JSR EXPR
        LDA ACC
        STA INKM
        RTS

DOPLOT: JSR XYARGS
        JMP PSET

; PADDR: the byte and bit for pixel PX, PY. Carry set if it's off the screen; otherwise
; GP = the byte's address, A = the bit's mask, Y = 0.
PADDR:  LDA PY+1
        BNE PAOFF
        LDA PY
        CMP #200
        BCS PAOFF
        LDA PX+1
        BEQ PA1
        CMP #1
        BNE PAOFF
        LDA PX
        CMP #64
        BCS PAOFF
PA1:    LDA PY          ; y * 40 is (y / 8) * 320 + (y AND 7) * 40: the second from the
        LSR A           ; text rows' table, whose SCREEN Q320HI takes off again
        LSR A
        LSR A
        TAX
        LDA PY
        AND #7
        TAY
        CLC
        LDA Q320LO,X
        ADC ROWLO,Y
        STA GP
        LDA Q320HI,X
        ADC ROWHI,Y
        CLC
        ADC BMREG       ; + the bitmap's address
        STA GP+1
        LDA PX+1        ; + x / 8
        LSR A
        LDA PX
        ROR A
        LSR A
        LSR A
        CLC
        ADC GP
        STA GP
        BCC PA2
        INC GP+1
PA2:
        LDA PX
        AND #7
        TAX
        LDA BITMSK,X
        LDY #0
        CLC
        RTS
PAOFF:  SEC
        RTS

; PSET: draw pixel PX, PY with the current INK. Off the screen, nothing happens.
PSET:   JSR PADDR
        BCS PSX
        LDX INKM
        BEQ PSCLR
        CPX #2
        BEQ PSXOR
        ORA (GP),Y
        STA (GP),Y
PSX:    RTS
PSCLR:  EOR #$FF
        AND (GP),Y
        STA (GP),Y
        RTS
PSXOR:  EOR (GP),Y
        STA (GP),Y
        RTS

; LINE x1,y1,x2,y2: Bresenham. PX, PY walk from the start to LFROM, LTO.
; dx (positive) in TMP, dy (negative) in CNT, steps in T1 and T2, error in GE.
DOLINE: JSR XYARGS
        LDA PX
        PHA
        LDA PX+1
        PHA
        LDA PY
        PHA
        LDA PY+1
        PHA
        LDA #','
        JSR EXPECT
        JSR XYARGS
        LDA PX
        STA LFROM
        LDA PX+1
        STA LFROM+1
        LDA PY
        STA LTO
        LDA PY+1
        STA LTO+1
        PLA
        STA PY+1
        PLA
        STA PY
        PLA
        STA PX+1
        PLA
        STA PX
        LDX #1          ; (before the subtraction: LDX sets N too)
        SEC             ; dx = |x2 - x1|; x steps toward x2
        LDA LFROM
        SBC PX
        STA TMP
        LDA LFROM+1
        SBC PX+1
        STA TMP+1
        BPL LN1
        LDX #$FF
        SEC
        LDA #0
        SBC TMP
        STA TMP
        LDA #0
        SBC TMP+1
        STA TMP+1
LN1:    STX T1
        LDX #1
        SEC             ; dy = -|y2 - y1|; y steps toward y2
        LDA PY
        SBC LTO
        STA CNT
        LDA PY+1
        SBC LTO+1
        STA CNT+1
        BMI LN2         ; y1 < y2: already negative, and y goes up
        LDX #$FF
        SEC
        LDA #0
        SBC CNT
        STA CNT
        LDA #0
        SBC CNT+1
        STA CNT+1
LN2:    STX T2
        CLC             ; error = dx + dy
        LDA TMP
        ADC CNT
        STA GE
        LDA TMP+1
        ADC CNT+1
        STA GE+1
LNLOOP: JSR PSET
        LDA PX
        CMP LFROM
        BNE LN3
        LDA PX+1
        CMP LFROM+1
        BNE LN3
        LDA PY
        CMP LTO
        BNE LN3
        LDA PY+1
        CMP LTO+1
        BNE LN3
        RTS
LN3:    LDA GE          ; e2 = 2 * error
        ASL A
        STA ARG
        LDA GE+1
        ROL A
        STA ARG+1
        SEC             ; e2 >= dy (signed): step x
        LDA ARG
        SBC CNT
        LDA ARG+1
        SBC CNT+1
        BVC LN3A
        EOR #$80
LN3A:   BMI LN5
        CLC
        LDA GE
        ADC CNT
        STA GE
        LDA GE+1
        ADC CNT+1
        STA GE+1
        LDA T1
        BMI LN4
        INC PX
        BNE LN5
        INC PX+1
        JMP LN5
LN4:    LDA PX
        BNE LN4B
        DEC PX+1
LN4B:   DEC PX
LN5:    SEC             ; e2 <= dx (signed), that is dx - e2 >= 0: step y
        LDA TMP
        SBC ARG
        LDA TMP+1
        SBC ARG+1
        BVC LN5A
        EOR #$80
LN5A:   BPL LN5B
        JMP LNLOOP
LN5B:
        CLC
        LDA GE
        ADC TMP
        STA GE
        LDA GE+1
        ADC TMP+1
        STA GE+1
        LDA T2
        BMI LN6
        INC PY
        BNE LN7
        INC PY+1
LN7:    JMP LNLOOP
LN6:    LDA PY
        BNE LN6B
        DEC PY+1
LN6B:   DEC PY
        JMP LNLOOP

; CIRCLE x,y,r: the midpoint circle. x and y of the octant in LFROM and LTO, error in GE.
DOCIRC: JSR XYARGS
        LDA PX
        STA GX0
        LDA PX+1
        STA GX0+1
        LDA PY
        STA GY0
        LDA PY+1
        STA GY0+1
        LDA #','
        JSR EXPECT
        JSR EXPR
        LDA ACC+1
        BPL CI0
        RTS             ; a negative radius draws nothing
CI0:    LDA ACC
        STA LFROM
        LDA ACC+1
        STA LFROM+1
        LDA #0
        STA LTO
        STA LTO+1
        SEC             ; error = 1 - r
        LDA #1
        SBC LFROM
        STA GE
        LDA #0
        SBC LFROM+1
        STA GE+1
CILOOP: LDA LFROM       ; while x >= y
        STA ARG
        LDA LFROM+1
        STA ARG+1
        LDA LTO
        STA ACC
        LDA LTO+1
        STA ACC+1
        JSR CMPS
        CMP #1
        BNE CI1
        RTS
CI1:    PHA             ; 2 if x = y
        LDA LFROM
        STA ACC
        LDA LFROM+1
        STA ACC+1
        LDA LTO
        STA ARG
        LDA LTO+1
        STA ARG+1
        JSR PLOT4       ; (+-x, +-y)
        PLA
        CMP #2
        BEQ CI2
        LDA LTO
        STA ACC
        LDA LTO+1
        STA ACC+1
        LDA LFROM
        STA ARG
        LDA LFROM+1
        STA ARG+1
        JSR PLOT4       ; (+-y, +-x)
CI2:    INC LTO         ; y = y + 1
        BNE CI3
        INC LTO+1
CI3:    LDA GE+1
        BPL CI4
        LDA LTO         ; error < 0: error += 2y + 1
        ASL A
        STA T3
        LDA LTO+1
        ROL A
        STA T3+1
        JMP CI5
CI4:    LDA LFROM       ; else x = x - 1, error += 2(y - x) + 1
        BNE CI4B
        DEC LFROM+1
CI4B:   DEC LFROM
        SEC
        LDA LTO
        SBC LFROM
        STA T3
        LDA LTO+1
        SBC LFROM+1
        STA T3+1
        ASL T3
        ROL T3+1
CI5:    SEC             ; + 1, via the carry
        LDA GE
        ADC T3
        STA GE
        LDA GE+1
        ADC T3+1
        STA GE+1
        JMP CILOOP

; PLOT4: plot (GX0 +- ACC, GY0 +- ARG), each distinct point once (so INK 2 works).
; The four corners go in TMP (x+), CNT (x-), LP (y+) and DISP (y-), free while drawing.
PLOT4:  CLC
        LDA GX0
        ADC ACC
        STA TMP
        LDA GX0+1
        ADC ACC+1
        STA TMP+1
        SEC
        LDA GX0
        SBC ACC
        STA CNT
        LDA GX0+1
        SBC ACC+1
        STA CNT+1
        CLC
        LDA GY0
        ADC ARG
        STA LP
        LDA GY0+1
        ADC ARG+1
        STA LP+1
        SEC
        LDA GY0
        SBC ARG
        STA DISP
        LDA GY0+1
        SBC ARG+1
        STA DISP+1
        LDA TMP         ; (x+, y+)
        STA PX
        LDA TMP+1
        STA PX+1
        LDA LP
        STA PY
        LDA LP+1
        STA PY+1
        JSR PSET
        LDA ACC         ; -0 is +0: only draw the mirror if a isn't 0
        ORA ACC+1
        STA GFXS
        BEQ P4B
        LDA CNT         ; (x-, y+)
        STA PX
        LDA CNT+1
        STA PX+1
        JSR PSET
P4B:    LDA ARG
        ORA ARG+1
        BEQ P4X
        LDA TMP         ; (x+, y-)
        STA PX
        LDA TMP+1
        STA PX+1
        LDA DISP
        STA PY
        LDA DISP+1
        STA PY+1
        JSR PSET
        LDA GFXS
        BEQ P4X
        LDA CNT         ; (x-, y-)
        STA PX
        LDA CNT+1
        STA PX+1
        JSR PSET
P4X:    RTS

; ---------------------------------------------------------------- expressions
; Every level leaves its value in ACC. Values waiting for an operator go on the CPU stack.

EXPR:   JSR ANDEX
EX1:    JSR PEEKC
        CMP #T_OR
        BNE EXX
        JSR NEXTC
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        JSR ANDEX
        PLA
        ORA ACC+1
        STA ACC+1
        PLA
        ORA ACC
        STA ACC
        JMP EX1
EXX:    RTS

ANDEX:  JSR RELEX
AN1:    JSR PEEKC
        CMP #T_AND
        BNE ANX
        JSR NEXTC
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        JSR RELEX
        PLA
        AND ACC+1
        STA ACC+1
        PLA
        AND ACC
        STA ACC
        JMP AN1
ANX:    RTS

; RELEX: a comparison gives -1 (true) or 0 (false).
RELEX:  JSR ADDEX
        LDA #0
        STA T1
RL0:    JSR PEEKC
        CMP #'<'
        BNE RL1
        LDA #1
        BNE RLADD
RL1:    CMP #'='
        BNE RL2
        LDA #2
        BNE RLADD
RL2:    CMP #'>'
        BNE RL3
        LDA #4
RLADD:  ORA T1
        STA T1
        JSR NEXTC
        JMP RL0
RL3:    LDA T1
        BNE RL4
        RTS
RL4:    PHA
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        JSR ADDEX
        PLA
        STA ARG+1
        PLA
        STA ARG
        PLA
        STA T1
        JSR CMPS
        AND T1
        BEQ RLF
        LDA #$FF
        STA ACC
        STA ACC+1
        RTS
RLF:    LDA #0
        STA ACC
        STA ACC+1
        RTS

; CMPS: compare ARG with ACC, signed. A = 1 if ARG < ACC, 2 if equal, 4 if greater.
CMPS:   LDA ARG
        CMP ACC
        BNE CS1
        LDA ARG+1
        CMP ACC+1
        BNE CS1
        LDA #2
        RTS
CS1:    SEC
        LDA ARG
        SBC ACC
        LDA ARG+1
        SBC ACC+1
        BVC CS2
        EOR #$80
CS2:    BMI CSLT
        LDA #4
        RTS
CSLT:   LDA #1
        RTS

ADDEX:  JSR MULEX
AD1:    JSR PEEKC
        CMP #'+'
        BEQ AD2
        CMP #'-'
        BEQ AD2
        RTS
AD2:    PHA
        JSR NEXTC
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        JSR MULEX
        PLA
        STA ARG+1
        PLA
        STA ARG
        PLA
        CMP #'+'
        BNE AD3
        CLC
        LDA ARG
        ADC ACC
        STA ACC
        LDA ARG+1
        ADC ACC+1
        STA ACC+1
        JMP AD1
AD3:    SEC
        LDA ARG
        SBC ACC
        STA ACC
        LDA ARG+1
        SBC ACC+1
        STA ACC+1
        JMP AD1

MULEX:  JSR UNARY
ML1:    JSR PEEKC
        CMP #'*'
        BEQ ML2
        CMP #'/'
        BEQ ML2
        RTS
ML2:    PHA
        JSR NEXTC
        LDA ACC
        PHA
        LDA ACC+1
        PHA
        JSR UNARY
        PLA
        STA ARG+1
        PLA
        STA ARG
        PLA
        CMP #'*'
        BNE ML3
        JSR MUL16
        JMP ML1
ML3:    JSR DIV16
        JMP ML1

UNARY:  JSR PEEKC
        CMP #'-'
        BNE PRIMARY
        JSR NEXTC
        JSR UNARY
NEGACC: SEC
        LDA #0
        SBC ACC
        STA ACC
        LDA #0
        SBC ACC+1
        STA ACC+1
        RTS

PRIMARY: JSR PEEKC
        BNE PR0
        JMP SYNERR
PR0:    CMP #'0'
        BCC PR1
        CMP #'9'+1
        BCS PR1
        JMP PARSEDEC
PR1:    CMP #'$'
        BNE PR2
        JSR NEXTC
        JMP PARSEHEX
PR2:    CMP #'('
        BNE PR3
        JSR NEXTC
        JSR EXPR
        LDA #')'
        JMP EXPECT
PR3:    CMP #'A'
        BCC PR4
        CMP #'Z'+1
        BCS PR4
        JSR NEXTC
        SEC
        SBC #'A'
        ASL A
        TAX
        LDA VARS,X
        STA ACC
        LDA VARS+1,X
        STA ACC+1
        RTS
PR4:    CMP #T_PEEK
        BNE PR5
        JSR NEXTC
        JSR PAREN
        LDY #0
        LDA (ACC),Y
        STA ACC
        STY ACC+1
        RTS
PR5:    CMP #T_RND
        BNE PR6
        JSR NEXTC
        JSR PAREN
        JMP RND
PR6:    CMP #T_ABS
        BNE PR8
        JSR NEXTC
        JSR PAREN
        LDA ACC+1
        BPL PR7
        JMP NEGACC
PR7:    RTS
PR8:    CMP #T_POINT
        BNE PRSYN
        JSR NEXTC
        LDA #'('
        JSR EXPECT
        JSR XYARGS
        LDA #')'
        JSR EXPECT
        LDA #0
        STA ACC+1
        JSR PADDR
        BCS PTZ         ; off the screen: 0
        AND (GP),Y
        BEQ PTZ
        LDA #1
        STA ACC
        RTS
PTZ:    LDA #0
        STA ACC
        RTS
PRSYN:  CMP #T_GET
        BNE PRSYN1
        JSR NEXTC
        JMP DOGET
PRSYN1: JMP SYNERR

PAREN:  LDA #'('
        JSR EXPECT
        JSR EXPR
        LDA #')'
        JMP EXPECT

; PARSEDEC: read decimal digits into ACC (wrapping at 65536).
PARSEDEC: LDA #0
        STA ACC
        STA ACC+1
PD1:    LDY #0
        LDA (TXT),Y
        CMP #'0'
        BCC PDX
        CMP #'9'+1
        BCS PDX
        AND #$0F
        STA T1
        ASL ACC         ; ACC * 2
        ROL ACC+1
        LDA ACC
        STA TMP
        LDA ACC+1
        STA TMP+1
        ASL ACC         ; ACC * 8
        ROL ACC+1
        ASL ACC
        ROL ACC+1
        CLC             ; * 10, + the digit
        LDA ACC
        ADC TMP
        STA ACC
        LDA ACC+1
        ADC TMP+1
        STA ACC+1
        CLC
        LDA ACC
        ADC T1
        STA ACC
        LDA ACC+1
        ADC #0
        STA ACC+1
        JSR NEXTC
        JMP PD1
PDX:    RTS

; PARSEHEX: read hex digits into ACC.
PARSEHEX: LDA #0
        STA ACC
        STA ACC+1
PH1:    LDY #0
        LDA (TXT),Y
        CMP #'0'
        BCC PHX
        CMP #'9'+1
        BCC PH2
        CMP #'A'
        BCC PHX
        CMP #'F'+1
        BCS PHX
        SBC #6          ; carry is clear: 'A' becomes $3A
PH2:    AND #$0F
        STA T1
        LDX #4
PH3:    ASL ACC
        ROL ACC+1
        DEX
        BNE PH3
        LDA ACC
        ORA T1
        STA ACC
        JSR NEXTC
        JMP PH1
PHX:    RTS

; PARSEINT: an optional minus sign and decimal digits, and nothing else. Carry set if not.
PARSEINT: JSR PEEKC
        LDX #0
        CMP #'-'
        BNE PI1
        LDX #1
        JSR NEXTC
        JSR PEEKC
PI1:    CMP #'0'
        BCC PIBAD
        CMP #'9'+1
        BCS PIBAD
        TXA
        PHA
        JSR PARSEDEC
        PLA
        BEQ PI2
        JSR NEGACC
PI2:    JSR PEEKC
        BNE PIBAD
        CLC
        RTS
PIBAD:  SEC
        RTS

; MUL16: ACC = ARG * ACC (the low 16 bits, so signs take care of themselves).
MUL16:  LDA #0
        STA TMP
        STA TMP+1
        LDX #16
MU1:    LSR ACC+1
        ROR ACC
        BCC MU2
        CLC
        LDA TMP
        ADC ARG
        STA TMP
        LDA TMP+1
        ADC ARG+1
        STA TMP+1
MU2:    ASL ARG
        ROL ARG+1
        DEX
        BNE MU1
        LDA TMP
        STA ACC
        LDA TMP+1
        STA ACC+1
        RTS

; DIV16: ACC = ARG / ACC, signed, rounding toward zero.
DIV16:  LDA ACC
        ORA ACC+1
        BNE DV0
        LDX #E_DIV
        JMP ERROR
DV0:    LDA ARG+1
        EOR ACC+1
        STA T3          ; bit 7: the sign of the result
        LDA ARG+1
        BPL DV1
        SEC
        LDA #0
        SBC ARG
        STA ARG
        LDA #0
        SBC ARG+1
        STA ARG+1
DV1:    LDA ACC+1
        BPL DV2
        JSR NEGACC
DV2:    JSR UDIV
        LDA ARG
        STA ACC
        LDA ARG+1
        STA ACC+1
        LDA T3
        BPL DV3
        JMP NEGACC
DV3:    RTS

; UDIV: ARG / ACC, unsigned. Quotient in ARG, remainder in TMP.
UDIV:   LDA #0
        STA TMP
        STA TMP+1
        LDX #16
UD1:    ASL ARG
        ROL ARG+1
        ROL TMP
        ROL TMP+1
        SEC
        LDA TMP
        SBC ACC
        TAY
        LDA TMP+1
        SBC ACC+1
        BCC UD2
        STA TMP+1
        STY TMP
        INC ARG
UD2:    DEX
        BNE UD1
        RTS

; RND: ACC = a pseudo-random number from 0 to ACC-1. 0 if ACC is 0 or negative.
RND:    LDA ACC+1
        BMI RNZ
        ORA ACC
        BEQ RNZ
        JSR RNDSTEP
        LDA SEED
        STA ARG
        LDA SEED+1
        STA ARG+1
        JSR UDIV
        LDA TMP
        STA ACC
        LDA TMP+1
        STA ACC+1
        RTS
RNZ:    LDA #0
        STA ACC
        STA ACC+1
        RTS

; RNDSTEP: advance SEED, a 16-bit xorshift (7, 9, 8), and return its high byte in A.
; The jump table's RND. Changes A.
RNDSTEP: LDA SEED+1
        LSR A
        LDA SEED
        ROR A
        EOR SEED+1
        STA SEED+1
        ROR A
        EOR SEED
        STA SEED
        EOR SEED+1
        STA SEED+1
        RTS

; PRDEC: print X (low) / Y (high) as an unsigned decimal number, no leading zeros.
; The jump table's PRDEC. Changes A and X, and BASIC's ACC, T1 and T2.
PRDEC:  STX ACC
        STY ACC+1
        JMP PRUNS

; PRSIGNED: print ACC as a signed number.
PRSIGNED: LDA ACC+1
        BPL PRUNS
        LDA #'-'
        JSR CHROUT
        JSR NEGACC
; PRUNS: print ACC as an unsigned number, no leading zeros.
PRUNS:  LDA #0
        STA T1          ; nonzero once a digit has been printed
        LDX #0
PU1:    LDA #'0'
        STA T2
PU2:    LDA ACC+1
        CMP POWHI,X
        BCC PU3
        BNE PU2S
        LDA ACC
        CMP POWLO,X
        BCC PU3
PU2S:   SEC
        LDA ACC
        SBC POWLO,X
        STA ACC
        LDA ACC+1
        SBC POWHI,X
        STA ACC+1
        INC T2
        JMP PU2
PU3:    LDA T2
        CMP #'0'
        BNE PU4
        LDA T1
        BEQ PU5         ; a leading zero
        LDA T2
PU4:    JSR CHROUT
        LDA #1
        STA T1
PU5:    INX
        CPX #4
        BNE PU1
        LDA ACC
        ORA #'0'
        JMP CHROUT

; NUMSTR: X (low) / Y (middle) / A (high), a 24-bit unsigned number, as eight ASCII
; digits with leading zeros in NUMBUF. Doesn't print. Returns A = the index in NUMBUF of
; the first digit that isn't a leading zero (7 for 0). Changes A, X, Y and BASIC's ACC
; and ARG ($E4-$E6). Each digit is how many times its power of ten can be taken away:
; take it once too often, then put the last one back.
NUMSTR: STX NUML
        STY NUMM
        STA NUMH
        LDX #0
NS24:   LDY #'0'        ; the four digits whose powers need all 24 bits
        LDA NUMH
        CMP POW24H,X
        BCC NS24D       ; the high byte alone is less: this digit is 0
        DEY
NS24S:  INY
        SEC
        LDA NUML
        SBC POW24L,X
        STA NUML
        LDA NUMM
        SBC POW24M,X
        STA NUMM
        LDA NUMH
        SBC POW24H,X
        STA NUMH
        BCS NS24S
        LDA NUML        ; once too often: put it back (the carry is clear)
        ADC POW24L,X
        STA NUML
        LDA NUMM
        ADC POW24M,X
        STA NUMM
        LDA NUMH
        ADC POW24H,X
        STA NUMH
NS24D:  TYA
        STA NUMBUF,X
        INX
        CPX #4
        BNE NS24
NS16:   LDY #'0'-1      ; under 10,000 now: the next three need only 16 bits
NS16S:  INY
        SEC
        LDA NUML
        SBC POW24L,X
        STA NUML
        LDA NUMM
        SBC POW24M,X
        STA NUMM
        BCS NS16S
        LDA NUML
        ADC POW24L,X
        STA NUML
        LDA NUMM
        ADC POW24M,X
        STA NUMM
        TYA
        STA NUMBUF,X
        INX
        CPX #7
        BNE NS16
        LDA NUML        ; what's left is the ones
        ORA #'0'
        STA NUMBUF+7
        LDX #0          ; A = the first digit that isn't a leading zero
NSZ:    LDA NUMBUF,X
        CMP #'0'
        BNE NSX
        INX
        CPX #7
        BNE NSZ
NSX:    TXA
        RTS

; PRNUM: print NUMSTR's digits at the cursor. A = the width: 0 prints the number without
; leading zeros; 1-8 print exactly that many digits, padded with leading zeros, or all of
; them if the number needs more. Over 8 counts as 8. Changes A and X, and BASIC's T1 ($F0).
PRNUM:  CMP #9
        BCC PN0
        LDA #8
PN0:    EOR #$FF        ; T1 = 8 - width: the first digit the width asks for
        SEC
        ADC #8
        STA T1
        LDX #0          ; start there, or at the first digit that isn't 0, if sooner
PN1:    CPX T1
        BEQ PN2
        CPX #7
        BEQ PN2
        LDA NUMBUF,X
        CMP #'0'
        BNE PN2
        INX
        JMP PN1
PN2:    LDA NUMBUF,X
        JSR CHROUT
        INX
        CPX #8
        BNE PN2
        RTS

; RNDSEED: SEED = X (low) / Y (high). A seed of 0, which the xorshift can't leave, becomes
; the start-up seed. Changes A.
RNDSEED: STX SEED
        STY SEED+1
        TXA
        ORA SEED+1
        BNE RSX
SEED0:  LDA #<SEEDBOOT  ; the start-up seed
        STA SEED
        LDA #>SEEDBOOT
        STA SEED+1
RSX:    RTS

; ---------------------------------------------------------------- tape

; WARM: READY through the jump table. Back to BASIC from machine code, keeping the BASIC
; program and its variables: a fresh stack, interrupts on (as BASIC runs), then READY. on a
; new line. For a program LOAD started that has done its job.
WARM:   LDX #$FF
        TXS
        CLI
        JMP READY

; LOAD or LOAD "NAME": the first program on the tape, or the first one called NAME.
DOLOAD: JSR GETNAME
        JMP LDGO

; GETNAME: LOAD's and SAVE's name, if there is one: "NAME" into TNAME, TNAMED nonzero.
GETNAME: JSR CLRNAME
        JSR PEEKC
        BEQ LDNX0
        CMP #':'
        BEQ LDNX0
        CMP #'"'
        BNE LDSYN
        JSR NEXTC
        LDX #0
LDN1:   LDY #0
        LDA (TXT),Y
        BEQ LDNX0       ; no closing quote: the name ends with the line
        JSR NEXTC
        CMP #'"'
        BEQ LDNX0
        CPX #16
        BCS LDN1        ; past 16 characters: ignored
        STA TNAME,X
        INX
        STX TNAMED
        JMP LDN1
LDNX0:  RTS
LDSYN:  JMP SYNERR

; JLOAD: LOAD for machine code, through the jump table. X/Y = a name ending in 0; an
; empty name takes the first program. Doesn't come back.
JLOAD:  STX STRP
        STY STRP+1
        JSR CLRNAME
        LDY #0
JL1:    LDA (STRP),Y
        BEQ LDGO
        STA TNAME,Y
        INY
        STY TNAMED
        CPY #16
        BNE JL1
; LDGO: the tape part of LOAD, with the name in TNAME.
LDGO:   LDA BORDER
        STA TBORD
        LDA #MOTOR
        STA TAPE
        LDA TAPE
        AND #1
        BNE LDPLAY      ; PLAY is down already
        LDX #<MPLAY
        LDY #>MPLAY
        JSR LDMSG
LDWAIT: JSR LDSTOP
        LDA TAPE
        AND #1
        BEQ LDWAIT
LDPLAY: LDX #<MSRCH
        LDY #>MSRCH
        JSR LDMSG
        LDA TNAMED
        BEQ LDS0
        LDX #<MFOR
        LDY #>MFOR
        JSR LDMSG
        LDX #<TNAME
        LDY #>TNAME
        JSR LDNAME
LDS0:   LDX #<MCR
        LDY #>MCR
        JSR LDMSG
; Search for the magic, UNCT, then read the header after it.
LDS1:   LDX #0          ; how many letters of the magic have matched
LDS2:   JSR LDBYTE
        CMP MAGIC,X
        BEQ LDS3
        LDX #0
        CMP MAGIC       ; a U may start it again
        BNE LDS2
        INX
        JMP LDS2
LDS3:   INX
        CPX #4
        BNE LDS2
        LDA #0          ; the checksum covers everything after the magic
        STA TCHK
        STA TOVR
        LDX #0
LDH1:   JSR LDBYTE
        STA THDR,X
        INX
        CPX #24
        BNE LDH1
        LDA THDR
        CMP #1
        BEQ LDV1
        JMP LDERR       ; a format version this ROM can't read
LDV1:   LDX #<MFOUND
        LDY #>MFOUND
        JSR LDMSG
        LDX #<[THDR+2]
        LDY #>[THDR+2]
        JSR LDNAME
        LDX #<MCR
        LDY #>MCR
        JSR LDMSG
        LDA TNAMED
        BEQ LDTAKE      ; no name: the first program will do
        LDX #15
LDC1:   LDA TNAME,X
        CMP THDR+2,X
        BNE LDSKIP
        DEX
        BPL LDC1
; This is the one.
LDTAKE: LDA THDR+1
        CMP #1
        BEQ LDT1
        JMP LDTYPE      ; not machine code (type 1): BASIC (type 2)?
LDT1:   LDA TLOAD+1     ; it must fit between $0400 and the top of memory
        CMP #$04
        BCS LDT2
        JMP LDOOM
LDT2:   CLC
        LDA TLOAD
        ADC TLEN
        STA TPTR
        LDA TLOAD+1
        ADC TLEN+1
        BCS LDOOM
        CMP RAMTOP
        BCC LDFIT
        BNE LDOOM
        LDA TPTR
        BNE LDOOM
LDFIT:  JSR LDNEW       ; it replaces the resident program, if it lands on it
LDGET:  LDX #<MLDING
        LDY #>MLDING
        JSR LDMSG
        LDA TLOAD
        STA TPTR
        LDA TLOAD+1
        STA TPTR+1
        JSR LDCNT
LDD1:   LDA TCNT
        ORA TCNT+1
        BEQ LDD3
        JSR LDBYTE
        LDY #0
        STA (TPTR),Y
        INC TPTR
        BNE LDD2
        INC TPTR+1
LDD2:   JSR LDDEC
        JMP LDD1
LDD3:   JSR LDBYTE      ; the checksum: now everything XORs to 0
        LDA TCHK
        ORA TOVR
        BNE LDERR
        JSR LDDONE      ; motor off; a BASIC program goes back to READY. from there
        LDX #$FF        ; start it as every program starts (chapter 6)
        TXS
        LDA #0
        TAX
        TAY
        PHA
        PLP
        SEI
        JMP (TENTRY)
; Not this one: let its data and checksum go by, and search again.
LDSKIP: JSR LDCNT
LDK1:   LDA TCNT
        ORA TCNT+1
        BEQ LDK2
        JSR LDBYTE
        JSR LDDEC
        JMP LDK1
LDK2:   JSR LDBYTE
        JMP LDS1
LDERR:  JSR LDFAIL
        LDX #E_LOAD
        JMP ERROR
LDOOM:  JSR LDFAIL
        LDX #E_OOM
        JMP ERROR

; LDNEW: a program that lands on the BASIC program (PROG up to its end marker at PRGEND)
; replaces it, as NEW does. One that loads clear of it leaves the BASIC program and its
; variables alone, so a routine or data loaded above it can go back to them through READY
; ($FF21). TPTR holds the low byte of the address just past the new program.
LDNEW:  CLC             ; the high byte of the end again
        LDA TLOAD
        ADC TLEN
        LDA TLOAD+1
        ADC TLEN+1
        CMP #>PROG      ; ends at or below PROG: clear of it
        BCC LDNX
        BNE LDNAB
        LDA TPTR
        CMP #<PROG+1
        BCC LDNX
LDNAB:  LDA PRGEND      ; starts past PRGEND: clear of it (PRGEND - TLOAD borrows)
        CMP TLOAD
        LDA PRGEND+1
        SBC TLOAD+1
        BCC LDNX
        JMP NEW0
LDNX:   RTS

; LDBYTE: wait for the next byte off the tape and take it. Keeps X and Y. Each byte's low
; four bits go to the border as it arrives: the loading stripes.
LDBYTE: BIT TAPE
        BMI LDB1
        LDA TAPE
        AND #1
        BEQ LDBRK       ; PLAY came up: STOP on the deck, or the end of the tape
        JSR LDSTOP
        JMP LDBYTE
LDB1:   BVC LDB2
        LDA #$40        ; a byte went by unread
        STA TOVR
LDB2:   LDA TDATA
        STA BORDER
        PHA
        EOR TCHK
        STA TCHK
        PLA
        RTS

; LDSTOP: the STOP key ends LOAD with BREAK.
LDSTOP: LDA KBD
        BPL LDSX
        AND #$7F
        CMP #3
        BNE LDSX
        BIT KBDSTRB
LDBRK:  JSR LDFAIL
        JMP DOBREAK
LDSX:   RTS

; LDEND: motor off, the border as it was.
LDEND:  LDA #0
        STA TAPE
        LDA TBORD
        STA BORDER
        RTS

; LDFAIL: LDEND, for a LOAD or SAVE that stops part way. If BASIC has no program (NEW0,
; or a program that landed on it), its end marker goes back at PROG over whatever came in.
LDFAIL: JSR LDEND
        LDA PRGEND
        CMP #<PROG
        BNE LDFX
        LDA PRGEND+1
        CMP #>PROG
        BNE LDFX
        LDA #0
        STA PROG
LDFX:   RTS

; SAVE or SAVE "NAME": the BASIC program on tape as a type-2 program, named NAME or with no
; name. Waits for RECORD and PLAY, then records the magic, the header, the program from
; PROG to its end marker, and the checksum, a byte each time the deck has room for one.
DOSAVE: JSR GETNAME
        LDA BORDER
        STA TBORD
        LDA TAPE
        AND #RECPLAY
        CMP #RECPLAY
        BEQ SVGO        ; RECORD and PLAY are down already
        LDX #<MREC
        LDY #>MREC
        JSR PRSTR
SVW1:   JSR LDSTOP
        LDA TAPE
        AND #RECPLAY
        CMP #RECPLAY
        BNE SVW1
SVGO:   LDA #1          ; the header: format version 1, type 2, the name,
        STA THDR
        LDA #2
        STA THDR+1
        LDX #15
SV1:    LDA TNAME,X
        STA THDR+2,X
        DEX
        BPL SV1
        LDA #<PROG      ; load address PROG,
        STA TLOAD
        LDA #>PROG
        STA TLOAD+1
        SEC             ; length PRGEND - PROG + 1, the end marker included,
        LDA PRGEND
        SBC #<PROG
        STA TLEN
        LDA PRGEND+1
        SBC #>PROG
        STA TLEN+1
        INC TLEN
        BNE SV2
        INC TLEN+1
SV2:    LDA #0          ; entry 0
        STA TENTRY
        STA TENTRY+1
        LDX #<MSAVE
        LDY #>MSAVE
        JSR LDMSG
        LDX #<TNAME
        LDY #>TNAME
        JSR LDNAME
        LDX #<MCR
        LDY #>MCR
        JSR LDMSG
        LDX #0          ; the magic
SV3:    LDA MAGIC,X
        JSR SVBYTE
        INX
        CPX #4
        BNE SV3
        LDA #0          ; the checksum covers everything after it
        STA TCHK
        LDX #0
SV4:    LDA THDR,X
        JSR SVBYTE
        INX
        CPX #24
        BNE SV4
        LDA #<PROG
        STA TPTR
        LDA #>PROG
        STA TPTR+1
        JSR LDCNT
SV5:    LDA TCNT
        ORA TCNT+1
        BEQ SV6
        LDY #0
        LDA (TPTR),Y
        JSR SVBYTE
        INC TPTR
        BNE SV5A
        INC TPTR+1
SV5A:   JSR LDDEC
        JMP SV5
SV6:    LDA TCHK        ; the checksum: everything after the magic XORs to 0
        JSR SVBYTE
        JSR SVROOM      ; the deck has recorded the last byte
        JMP LDEND

; SVBYTE: record A, once the deck has room for it. Keeps X and Y. The border shows each
; byte, as LOAD's stripes do.
SVBYTE: PHA
        JSR SVROOM
        PLA
        STA TDATA
        STA BORDER
        EOR TCHK
        STA TCHK
        RTS

; SVROOM: wait until the deck has room for a byte. RECORD or PLAY coming up (STOP on the
; deck), or the STOP key, ends SAVE with BREAK.
SVROOM: LDA TAPE
        AND #ROOM
        BNE SVRX
        LDA TAPE
        AND #RECPLAY
        CMP #RECPLAY
        BNE SVBRK
        JSR LDSTOP
        JMP SVROOM
SVBRK:  JMP LDBRK
SVRX:   RTS

; LDTYPE: the program LOAD wants isn't machine code; A = its type. Type 2 is a BASIC
; program; anything else is a ?LOAD ERROR.
LDTYPE: CMP #2
        BEQ LDBAS
LDBE:   JMP LDERR
LDBO:   JMP LDOOM

; LDDONE: the program is in and checked: motor off and the border back. Machine code goes
; back to be started; a BASIC program goes to LDBGO.
LDDONE: JSR LDEND
        LDA THDR+1
        CMP #2
        BEQ LDBGO
        RTS

; LDBAS: a BASIC program (type 2). Its lines and end marker go to PROG in place of the
; resident program, as NEW would forget it: it must say $0800, hold at least the end
; marker, and fit below MEMTOP.
LDBAS:  LDA TLOAD
        CMP #<PROG
        BNE LDBE
        LDA TLOAD+1
        CMP #>PROG
        BNE LDBE
        LDA TLEN
        ORA TLEN+1
        BEQ LDBE
        CLC             ; TPTR = just past it
        LDA TLEN
        ADC #<PROG
        STA TPTR
        LDA TLEN+1
        ADC #>PROG
        BCS LDBO
        STA TPTR+1
        JSR MEMTOP
        CMP TPTR+1
        BCC LDBO       ; ends in a page past the top
        BNE LDB0
        LDA TPTR
        BNE LDBO       ; ends inside the top page
LDB0:   JSR NEW0
        JMP LDGET

; LDBGO: the BASIC program is in, from PROG to TPTR-1. Walk its lines: each at least 4
; bytes and ending in 0, then the end marker as its last byte. PRGEND points at it, the
; variables and the FOR and GOSUB stacks are clear (NEW0 did that), and BASIC is READY.
LDBGO:  LDA #<PROG
        STA LP
        LDA #>PROG
        STA LP+1
LDW1:   LDY #0
        LDA (LP),Y
        BEQ LDW3        ; an end marker: is it the last byte?
        CMP #4
        BCC LDBAD
        TAY             ; the line's last byte must be 0
        DEY
        LDA (LP),Y
        BNE LDBAD
        INY
        TYA
        CLC
        ADC LP
        STA LP
        BCC LDW2
        INC LP+1
LDW2:   LDA LP+1        ; still before TPTR?
        CMP TPTR+1
        BCC LDW1
        BNE LDBAD
        LDA LP
        CMP TPTR
        BCC LDW1
        BCS LDBAD
LDW3:   CLC             ; LP + 1 = TPTR
        LDA LP
        ADC #1
        TAX
        LDA LP+1
        ADC #0
        CMP TPTR+1
        BNE LDBAD
        CPX TPTR
        BNE LDBAD
        LDA LP
        STA PRGEND
        LDA LP+1
        STA PRGEND+1
        JMP WARM
LDBAD:  JSR NEW0
        JMP LDBE

; LDMSG: print the string at X/Y with the motor stopped, so no byte goes by unread.
LDMSG:  LDA #0
        STA TAPE
        JSR PRSTR
        LDA #MOTOR
        STA TAPE
        RTS

; LDNAME: print the 16-character name at X/Y without its trailing spaces, motor stopped.
LDNAME: LDA #0
        STA TAPE
        STX STRP
        STY STRP+1
        LDY #16
LDNM1:  DEY
        BMI LDNM3       ; all spaces
        LDA (STRP),Y
        CMP #' '
        BEQ LDNM1
        INY
        STY T1
        LDY #0
LDNM2:  LDA (STRP),Y
        JSR CHROUT
        INY
        CPY T1
        BNE LDNM2
LDNM3:  LDA #MOTOR
        STA TAPE
        RTS

; LDCNT: TCNT = the program's length. LDDEC: one less.
LDCNT:  LDA TLEN
        STA TCNT
        LDA TLEN+1
        STA TCNT+1
        RTS
LDDEC:  LDA TCNT
        BNE LDDC1
        DEC TCNT+1
LDDC1:  DEC TCNT
        RTS

; CLRNAME: no name yet: TNAME all spaces.
CLRNAME: LDA #' '
        LDX #15
CN1:    STA TNAME,X
        DEX
        BPL CN1
        LDA #0
        STA TNAMED
        RTS

; ---------------------------------------------------------------- tables

POWLO:  .byte <10000, <1000, <100, <10
POWHI:  .byte >10000, >1000, >100, >10

; NUMSTR's powers of ten, 10,000,000 down to 10, three bytes each.
POW24L: .byte <10000000, <1000000, <100000, <10000, <1000, <100, <10
POW24M: .byte >10000000, >1000000, >100000, >10000, >1000, >100, >10
POW24H: .byte 10000000 >> 16, 1000000 >> 16, 100000 >> 16, 0, 0, 0, 0

ROWLO:  .byte <[SCREEN+0*40], <[SCREEN+1*40], <[SCREEN+2*40], <[SCREEN+3*40], <[SCREEN+4*40]
        .byte <[SCREEN+5*40], <[SCREEN+6*40], <[SCREEN+7*40], <[SCREEN+8*40], <[SCREEN+9*40]
        .byte <[SCREEN+10*40], <[SCREEN+11*40], <[SCREEN+12*40], <[SCREEN+13*40], <[SCREEN+14*40]
        .byte <[SCREEN+15*40], <[SCREEN+16*40], <[SCREEN+17*40], <[SCREEN+18*40], <[SCREEN+19*40]
        .byte <[SCREEN+20*40], <[SCREEN+21*40], <[SCREEN+22*40], <[SCREEN+23*40], <[SCREEN+24*40]
ROWHI:  .byte >[SCREEN+0*40], >[SCREEN+1*40], >[SCREEN+2*40], >[SCREEN+3*40], >[SCREEN+4*40]
        .byte >[SCREEN+5*40], >[SCREEN+6*40], >[SCREEN+7*40], >[SCREEN+8*40], >[SCREEN+9*40]
        .byte >[SCREEN+10*40], >[SCREEN+11*40], >[SCREEN+12*40], >[SCREEN+13*40], >[SCREEN+14*40]
        .byte >[SCREEN+15*40], >[SCREEN+16*40], >[SCREEN+17*40], >[SCREEN+18*40], >[SCREEN+19*40]
        .byte >[SCREEN+20*40], >[SCREEN+21*40], >[SCREEN+22*40], >[SCREEN+23*40], >[SCREEN+24*40]

; Bitmap rows of eight lines: q * 320, the high bytes less SCREEN's (PADDR adds a text
; row's address, which has it in), and the mask for each pixel of a byte.
Q320LO: .rept 25, q
        .byte <[q*320]
        .endr
Q320HI: .rept 25, q
        .byte <[[>[q*320]]-[>SCREEN]]
        .endr
BITMSK: .byte $80, $40, $20, $10, $08, $04, $02, $01

; Statement handlers, in token order.
STTAB:  .word DOPRINT, DOLET, DOINPUT, DOIF, DOGOTO, DOGOSUB, DORET, DOFOR, DONEXT
        .word DOPOKE, DOREM, DOEND, DOCLS, DOSYS, DORUN, DOLIST, DONEW, DOHELP
        .word DOGR, DOTEXT, DOPLOT, DOLINE, DOCIRC, DOINK

; Keywords, in token order. The last letter of each has bit 7 set.
KWTAB:  .text "PRIN"
        .byte 'T'+$80
        .text "LE"
        .byte 'T'+$80
        .text "INPU"
        .byte 'T'+$80
        .text "I"
        .byte 'F'+$80
        .text "GOT"
        .byte 'O'+$80
        .text "GOSU"
        .byte 'B'+$80
        .text "RETUR"
        .byte 'N'+$80
        .text "FO"
        .byte 'R'+$80
        .text "NEX"
        .byte 'T'+$80
        .text "POK"
        .byte 'E'+$80
        .text "RE"
        .byte 'M'+$80
        .text "EN"
        .byte 'D'+$80
        .text "CL"
        .byte 'S'+$80
        .text "SY"
        .byte 'S'+$80
        .text "RU"
        .byte 'N'+$80
        .text "LIS"
        .byte 'T'+$80
        .text "NE"
        .byte 'W'+$80
        .text "HEL"
        .byte 'P'+$80
        .text "G"
        .byte 'R'+$80
        .text "TEX"
        .byte 'T'+$80
        .text "PLO"
        .byte 'T'+$80
        .text "LIN"
        .byte 'E'+$80
        .text "CIRCL"
        .byte 'E'+$80
        .text "IN"
        .byte 'K'+$80
        .text "THE"
        .byte 'N'+$80
        .text "T"
        .byte 'O'+$80
        .text "STE"
        .byte 'P'+$80
        .text "PEE"
        .byte 'K'+$80
        .text "RN"
        .byte 'D'+$80
        .text "AB"
        .byte 'S'+$80
        .text "AN"
        .byte 'D'+$80
        .text "O"
        .byte 'R'+$80
        .text "POIN"
        .byte 'T'+$80
        .text "LOA"
        .byte 'D'+$80
        .text "SAV"
        .byte 'E'+$80
        .text "DAT"
        .byte 'A'+$80
        .text "REA"
        .byte 'D'+$80
        .text "RESTOR"
        .byte 'E'+$80
        .text "GE"
        .byte 'T'+$80
        .byte 0

ERRLO:  .byte <ESYN, <EUNDEF, <ENEXT, <ERET, <EDIV, <EOOM, <ELOAD, <EDATA
ERRHI:  .byte >ESYN, >EUNDEF, >ENEXT, >ERET, >EDIV, >EOOM, >ELOAD, >EDATA
ESYN:   .asciiz "SYNTAX ERROR"
EUNDEF: .asciiz "UNDEFINED LINE"
ENEXT:  .asciiz "NEXT WITHOUT FOR"
ERET:   .asciiz "RETURN WITHOUT GOSUB"
EDIV:   .asciiz "DIVISION BY ZERO"
EOOM:   .asciiz "OUT OF MEMORY"
ELOAD:  .asciiz "LOAD ERROR"
EDATA:  .asciiz "OUT OF DATA"
MIN:    .asciiz " IN "
MBREAK: .asciiz "BREAK"
MREDO:  .text "?REDO"
        .byte 13, 0
MREADY: .text "READY."
        .byte 13, 0
MPLAY:  .text "PRESS PLAY ON TAPE"
        .byte 13, 0
MSRCH:  .asciiz "SEARCHING"
MFOR:   .asciiz " FOR "
MFOUND: .asciiz "FOUND "
MLDING: .text "LOADING"
MCR:    .byte 13, 0
MREC:   .text "PRESS RECORD AND PLAY ON TAPE"
        .byte 13, 0
MSAVE:  .asciiz "SAVING " 
MAGIC:  .text "UNCT"

BANNER: .text "    UNC 64 SYSTEM ROM  REV A"
        .byte 13, 13, 0
MSG16:  .text " 16K RAM SYSTEM"
        .byte 13, 0
MSG64:  .text " 64K RAM SYSTEM"
        .byte 13, 0
MSGVB:  .text " VIDEO BOARD REV A"
        .byte 13, 0
MSGBAS: .asciiz " UNC BASIC REV A  "
MSGFREE: .text " BYTES FREE"
        .byte 13
        .text " TYPE HELP FOR HELP"
        .byte 13, 13, 0

; The HELP screen: rows 0-22, READY. under it. [Brackets] in the comments are reverse video
; (the bar on top and the headings): bit 7 set. The bar's gaps are $80, glyph $00 (blank)
; reversed, not $A0, which the video chip blinks as the cursor. A full row wraps by itself.
; To fit, DOHELP reads two codes of its own: $81-$A6 is a keyword's token, spelled out as
; LIST does (PRINT, $80, is the bar's gap, so it's written in letters), and 9 moves to
; column 17, where each row's second half begins.
MHELP:
        ; [ UNC BASIC REV A      MORE: MANUAL CH. 2]
        .byte $80, $D5, $CE, $C3, $80, $C2, $C1, $D3, $C9, $C3, $80, $D2, $C5, $D6, $80, $C1
        .byte $80, $80, $80, $80, $80, $80, $CD, $CF, $D2, $C5, $BA, $80, $CD, $C1, $CE, $D5
        .byte $C1, $CC, $80, $C3, $C8, $AE, $80, $B2, $0D
        ; [PROGRAMS]
        .byte $D0, $D2, $CF, $C7, $D2, $C1, $CD, $D3, $0D
        ;  10 PRINT "HI"   STORES THE LINE
        .byte $20, $31, $30, $20, $50, $52, $49, $4E, $54, $20, $22, $48, $49, $22, $09, $53
        .byte $54, $4F, $52, $45, $53, $20, $54, $48, $45, $20, $95, $0D
        ;  RUN LIST NEW    RUN, SHOW, FORGET IT
        .byte $20, $8E, $20, $8F, $20, $90, $09, $8E, $2C, $20, $53, $48, $4F, $57, $2C, $20
        .byte $46, $4F, $52, $47, $45, $54, $20, $49, $54, $0D
        ;  LOAD "NAME"     FROM TAPE
        .byte $20, $A1, $20, $22, $4E, $41, $4D, $45, $22, $09, $46, $52, $4F, $4D, $20, $54
        .byte $41, $50, $45, $0D
        ;  SAVE "NAME"     ONTO TAPE
        .byte $20, $A2, $20, $22, $4E, $41, $4D, $45, $22, $09, $4F, $4E, $54, $4F, $20, $54
        .byte $41, $50, $45, $0D
        ;  STOP (CTRL+C)   STOPS A PROGRAM
        .byte $20, $53, $54, $4F, $50, $20, $28, $43, $54, $52, $4C, $2B, $43, $29, $09, $53
        .byte $54, $4F, $50, $53, $20, $41, $20, $50, $52, $4F, $47, $52, $41, $4D, $0D
        ; 
        .byte $0D
        ; [STATEMENTS]
        .byte $D3, $D4, $C1, $D4, $C5, $CD, $C5, $CE, $D4, $D3, $0D
        ;  PRINT INPUT LET IF..THEN REM END CLS
        .byte $20, $50, $52, $49, $4E, $54, $20, $82, $20, $81, $20, $83, $2E, $2E, $98, $20
        .byte $8A, $20, $8B, $20, $8C, $0D
        ;  GOTO GOSUB RETURN  DATA READ RESTORE
        .byte $20, $84, $20, $85, $20, $86, $20, $20, $A3, $20, $A4, $20, $A5, $0D
        ;  FOR I=1 TO 9 STEP 2 ... NEXT I
        .byte $20, $87, $20, $49, $3D, $31, $20, $99, $20, $39, $20, $9A, $20, $32, $20, $2E
        .byte $2E, $2E, $20, $88, $20, $49, $0D
        ;  POKE A,V        SYS A (MACHINE CODE)
        .byte $20, $89, $20, $41, $2C, $56, $09, $8D, $20, $41, $20, $28, $4D, $41, $43, $48
        .byte $49, $4E, $45, $20, $43, $4F, $44, $45, $29, $0D
        ; 
        .byte $0D
        ; [GRAPHICS]
        .byte $C7, $D2, $C1, $D0, $C8, $C9, $C3, $D3, $0D
        ;  GR / TEXT       PICTURE ON / OFF
        .byte $20, $92, $20, $2F, $20, $93, $09, $50, $49, $43, $54, $55, $52, $45, $20, $4F
        .byte $4E, $20, $2F, $20, $4F, $46, $46, $0D
        ;  PLOT X,Y        LINE X1,Y1,X2,Y2
        .byte $20, $94, $20, $58, $2C, $59, $09, $95, $20, $58, $31, $2C, $59, $31, $2C, $58
        .byte $32, $2C, $59, $32, $0D
        ;  CIRCLE X,Y,R    INK 0 ERASES, 2 FLIPS
        .byte $20, $96, $20, $58, $2C, $59, $2C, $52, $09, $97, $20, $30, $20, $45, $52, $41
        .byte $53, $45, $53, $2C, $20, $32, $20, $46, $4C, $49, $50, $53, $0D
        ; 
        .byte $0D
        ; [NUMBERS]
        .byte $CE, $D5, $CD, $C2, $C5, $D2, $D3, $0D
        ;  A-Z  + - * /  = <> < > <= >=  AND OR
        .byte $20, $41, $2D, $5A, $20, $20, $2B, $20, $2D, $20, $2A, $20, $2F, $20, $20, $3D
        .byte $20, $3C, $3E, $20, $3C, $20, $3E, $20, $3C, $3D, $20, $3E, $3D, $20, $20, $9E
        .byte $20, $9F, $0D
        ;  RND(N) ABS(N) PEEK(A) POINT(X,Y) GET
        .byte $20, $9C, $28, $4E, $29, $20, $9D, $28, $4E, $29, $20, $9B, $28, $41, $29, $20
        .byte $A0, $28, $58, $2C, $59, $29, $20, $A6, $0D
        .byte 0

; ---------------------------------------------------------------- DATA, READ, RESTORE, GET

; READ A[,B...]: each variable takes the next item of the program's DATA lines, in order.
; An item is an expression (a number, or -5, or $FF), so READ runs the expression parser
; on the DATA line's text, then comes back to its own.
DOREAD: JSR GETVAR
        TXA
        PHA
        JSR NXDATA      ; TXT on the item; the READ line's TXT is on the stack
        JSR EXPR
        JSR PEEKC       ; after the item: a comma, or the end of the line
        BEQ RD1
        CMP #','
        BEQ RD0
        CMP #':'
        BEQ RD1
        JMP SYNERR      ; (in the READ line: CURLIN is the line that READ is on)
RD0:    JSR NEXTC
RD1:    LDA TXT         ; where the next item starts
        STA DATAP
        LDA TXT+1
        STA DATAP+1
        PLA             ; back to the READ line
        STA TXT+1
        PLA
        STA TXT
        PLA
        TAX
        LDA ACC
        STA VARS,X
        LDA ACC+1
        STA VARS+1,X
        JSR PEEKC
        CMP #','
        BNE RDX
        JSR NEXTC
        JMP DOREAD
RDX:    RTS

; NXDATA: push TXT (high byte on top, under the return address) and point TXT at the next
; DATA item: where the last one ended, or at the next DATA on a later line. ?OUT OF DATA if
; there is none.
NXDATA: PLA             ; the return address, out of the way
        STA T3
        PLA
        STA T3+1
        LDA TXT
        PHA
        LDA TXT+1
        PHA
        LDA T3+1
        PHA
        LDA T3
        PHA
        LDA DATAP+1
        BEQ ND0         ; before the first READ: search from the first line
        STA TXT+1
        LDA DATAP
        STA TXT
        JSR PEEKC
        BEQ ND1         ; this DATA line is used up: on to the next
        CMP #':'
        BEQ ND1
        RTS
ND0:    LDA #<PROG
        STA LP
        LDA #>PROG
        STA LP+1
        JMP ND2
ND1:    LDA DATAL       ; the line after DATAL
        STA LP
        LDA DATAL+1
        STA LP+1
        LDY #0
        LDA (LP),Y
        CLC
        ADC LP
        STA LP
        BCC ND2
        INC LP+1
ND2:    LDY #0          ; LP on a line: does it have a DATA?
        LDA (LP),Y
        BEQ NDNONE      ; the end marker: no more DATA
        LDY #3
ND3:    LDA (LP),Y
        BEQ ND4
        CMP #T_DATA
        BEQ ND5
        INY
        BNE ND3
ND4:    LDY #0          ; not on this line: the next
        LDA (LP),Y
        CLC
        ADC LP
        STA LP
        BCC ND2
        INC LP+1
        JMP ND2
ND5:    INY             ; found: the item starts after the token
        TYA
        CLC
        ADC LP
        STA TXT
        LDA LP+1
        ADC #0
        STA TXT+1
        LDA LP
        STA DATAL
        LDA LP+1
        STA DATAL+1
        RTS
NDNONE: LDX #E_DATA
        JMP ERROR

; RESTORE: the next READ starts again from the first DATA.
DORESTORE: LDA #0
        STA DATAP+1
        RTS

; GET: the key waiting, taken, or 0 if there's none. It doesn't wait.
DOGET:  LDA #0
        STA ACC+1
        LDA KBD
        BPL GT0
        AND #$7F
        BIT KBDSTRB
        STA ACC
        RTS
GT0:    LDA #0
        STA ACC
        RTS

; Jump table: the stable entry points.
        .org $FF00
        JMP CLS         ; $FF00
        JMP CHROUT      ; $FF03
        JMP GETKEY      ; $FF06
        JMP PRSTR       ; $FF09
        JMP PRHEX       ; $FF0C
        JMP PRDEC       ; $FF0F
        JMP RNDSTEP     ; $FF12: RND
        JMP JLOAD       ; $FF15: LOAD
        JMP NUMSTR      ; $FF18
        JMP PRNUM       ; $FF1B
        JMP RNDSEED     ; $FF1E
        JMP WARM        ; $FF21: READY, back to BASIC

        .org $FFFA
        .word NMI
        .word RESET
        .word IRQ
