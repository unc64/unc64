; AUNT TRACKER for the UNC 64's sound chip. Text mode, 50 Hz.
;
; The AUNT's cassette: a three-voice tracker, with a tune on it. Every UNC has
; the chip; if a host has taken it out, SOUND ($C0DF) doesn't read $16, and
; the tracker says so and stops.
;
; A tune is an ORDER of positions; a position is three PATTERNS, one for each
; voice, side by side; a pattern is 32 rows of a note and an instrument. The
; player steps a row every SPEED frames. The row sounding is the bright one in
; the middle of the screen.
;
;   SPACE       play from the bright row, or stop
;   RETURN      play from the top of the tune
;   Q W E       voice 1, 2, 3 silent, or heard again
;   F           the filter out of the circuit, or back in
;   1-9         the instrument the keys play
;   , .         the octave the keys play, down and up
;   left right  the voice the keys write to
;
; and, while it's stopped:
;
;   up down     the row
;   [ ]         the position
;   Z S X D C V G B H N J M   a note, C to B as on a piano: heard, written to
;               the bright row with the instrument, and on to the next row
;   =           a note off, written the same way
;   DELETE      the cell emptied
;
; The player, once a frame, for each voice with a note:
;
;   - steps the instrument's TABLE: a waveform and a pitch for this frame,
;     which is how one voice plays a chord (the arpeggio, a note a frame) and
;     how it plays a drum (a frame of noise, then a triangle falling);
;   - adds the vibrato, once the note is a quarter of a second old;
;   - moves the pulse width between $30 and $D0;
;   - writes FREQ, WIDTH and WAVE.
;
; A new note writes the envelope and TRIM and takes the gate off and on, so
; the attack starts over. The filter's corner sweeps up and down by itself;
; an instrument says whether it goes through.
;
; The tune, the note table and the patterns are written by
; scripts/tracker-song.py. The instruments and their tables are below, by
; hand: change them and assemble again.

SCREEN  = $0400
VMODE   = $C000
FRAME   = $C006
KBD_STROBE = $C011

; the sound chip (manual ch. 12): a voice is 8 bytes from AUNT
AUNT    = $C0C0
FREQ_LO = 0
FREQ_HI = 1
WIDTH   = 2
WAVE    = 3
CONTROL = 4             ; bit 0 the gate
ATTACK_DECAY = 5
SUSTAIN_RELEASE = 6
TRIM    = 7             ; sixteenths off the voice's level
CUTOFF  = $C0D8
FILTER  = $C0D9         ; bits 0-2 the voices through it, bits 4-7 resonance
VOLUME  = $C0DA         ; bits 0-3 volume, bit 4 low-pass
SOUND   = $C0DF         ; $16 with the chip in
SOUND_ID = $16

TRI     = $01
SAW     = $02
PULSE   = $04
NOISE   = $08

; the ROM (chapter 13)
CLS     = $FF00
PRSTR   = $FF09
CURX    = $0200
CURY    = $0201

STEADY  = %00001000     ; VMODE bit 3: $A0 doesn't blink
OFF     = $FF           ; a note off, in a pattern
ROWS    = 32
VIEW    = 17            ; pattern rows on the screen
MIDDLE  = 8             ; the bright one
TOP     = 5             ; screen row of the first
VIBDELAY = 12           ; frames before a note's vibrato
PREVIEW = 14            ; frames a note sounds when you play it by hand
METER   = 8
RES     = 8             ; the filter's resonance
CUTMIN  = 104           ; its corner sweeps between these: 285 Hz
CUTMAX  = 190           ; and 1,840 Hz

PATTERNS = $2000        ; 48 patterns of 64 bytes
ORDER   = $2C00

; ---------------------------------------------------------------- fast page
; The tests read the tracker's state from here, by name.

        .org $00
PLAYING: .res 1         ; 1 while the tune plays
POS:    .res 1          ; position in the order
ROW:    .res 1          ; the bright row, 0-31
TICK:   .res 1          ; frames left on this row
STEP:   .res 1          ; 0 until the first row has played: then the row moves on first
SPEED:  .res 1          ; frames a row
CURV:   .res 1          ; the voice the keys write to, 0-2
OCT:    .res 1          ; the octave the keys play, 0-6
INS:    .res 1          ; the instrument the keys play, 1-9
FILTON: .res 1          ; 0: the filter is out of the circuit
CUT:    .res 1          ; the filter's corner now
CUTDIR: .res 1          ; 0 opening, 1 closing
DIRTY:  .res 1          ; the screen is behind
FRAMEC: .res 1          ; frames, for the vibrato's turn
PAT:    .res 3          ; the three patterns at POS
VNOTE:  .res 3          ; each voice: its note, 1-95, 0 before its first
VINS:   .res 3          ; its instrument's place in INSTR (number x 8)
VTAB:   .res 3          ; where it is in TABLES
VPW:    .res 3          ; its pulse width
VPWD:   .res 3          ; 0 widening, 1 narrowing
VAGE:   .res 3          ; frames since its note began (it stops at 255)
VMUTE:  .res 3          ; 1: silent
VMETER: .res 3          ; its meter, 0-8
VPREV:  .res 3          ; frames left of a note played by hand
VARS_END:
PTR:    .res 2          ; a pattern cell
SCR:    .res 2          ; a screen row
FLO:    .res 1          ; the FREQ being worked out
FHI:    .res 1
DLO:    .res 1          ; the vibrato's step
DHI:    .res 1
TWAVE:  .res 1
TNOTE:  .res 1
TINS:   .res 1
TMP:    .res 1
TMP2:   .res 1
TMPX:   .res 1
TMPX2:  .res 1
CNT:    .res 1
REV:    .res 1          ; $80 while drawing the bright row
LINE:   .res 1
DROW:   .res 1
KEY:    .res 1

; ---------------------------------------------------------------- start

        .org $0800
start:  LDA #STEADY
        STA VMODE
        JSR CLS
        LDA SOUND
        CMP #SOUND_ID
        BEQ @chip
        LDA #11
        STA CURY
        LDA #3
        STA CURX
        LDX #<nochip
        LDY #>nochip
        JSR PRSTR
        LDA #13
        STA CURY
        LDA #3
        STA CURX
        LDX #<nochip2
        LDY #>nochip2
        JSR PRSTR
@stop:  JMP @stop
@chip:  JSR init
        JSR panel

; The main loop: wait for the frame counter to tick, then do one frame's work.
main:   LDA FRAME
@wait:  CMP FRAME
        BEQ @wait
work:   JSR player
        JSR keys
        JSR draw
        JMP main

init:   LDA #0
        TAX
@zero:  STA $00,X
        INX
        CPX #VARS_END
        BNE @zero
        LDX #$1F                ; the chip: every register 0
@chip:  STA AUNT,X
        DEX
        BPL @chip
        LDA #SONGSPEED
        STA SPEED
        LDA #4
        STA OCT
        LDA #1
        STA INS
        STA FILTON
        STA DIRTY
        LDA #CUTMIN
        STA CUT
        LDA #$1C                ; volume 12 of 15, the low-pass
        STA VOLUME
        LDX #2                  ; every voice starts on instrument 1
@ins:   LDA #1
        JSR setins
        DEX
        BPL @ins
        JMP loadpos

; PAT = the three patterns at POS. At the order's $FF, POS goes where it says.
loadpos:
        LDA POS
        ASL
        CLC
        ADC POS
        TAX
        LDA ORDER,X
        CMP #$FF
        BNE @ok
        LDA ORDER+1,X
        STA POS
        JMP loadpos
@ok:    STA PAT
        LDA ORDER+1,X
        STA PAT+1
        LDA ORDER+2,X
        STA PAT+2
        RTS

; PTR = voice X's cell at row A. Keeps X.
cell:   ASL
        STA PTR
        LDA PAT,X
        AND #3
        TAY
        LDA SIXTYFOUR,Y
        ORA PTR
        STA PTR
        LDA PAT,X
        LSR
        LSR
        CLC
        ADC #>PATTERNS
        STA PTR+1
        RTS

; ---------------------------------------------------------------- the player

player: INC FRAMEC
        LDA PLAYING
        BEQ @voices
        DEC TICK
        BNE @voices
        LDA SPEED
        STA TICK
        JSR dorow
@voices:
        LDX #2
@voice: JSR voice
        DEX
        BPL @voice
        JMP filter

; The next row: its notes and instruments, for all three voices.
dorow:  LDA STEP
        BEQ @first
        INC ROW
        LDA ROW
        CMP #ROWS
        BNE @play
        LDA #0
        STA ROW
        INC POS
        JSR loadpos
        JMP @play
@first: INC STEP
@play:  LDX #2
@voice: LDA ROW
        JSR cell
        LDY #1
        LDA (PTR),Y
        BEQ @same
        JSR setins
@same:  LDY #0
        LDA (PTR),Y
        BEQ @next
        CMP #OFF
        BNE @note
        JSR noteoff
        JMP @next
@note:  JSR noteon
@next:  DEX
        BPL @voice
        LDA #1
        STA DIRTY
        RTS

; Voice X takes instrument A (1-9).
setins: ASL
        ASL
        ASL
        STA VINS,X
        TAY
        LDA INSTR+I_WIDTH,Y
        STA VPW,X
        LDA #0
        STA VPWD,X
        RTS

; Voice X starts note A (1-95) on its instrument.
noteon: STA VNOTE,X
        LDY VINS,X
        LDA INSTR+I_TABLE,Y
        STA VTAB,X
        LDA #0
        STA VAGE,X
        STA VPREV,X
        LDA #METER
        STA VMETER,X
        STX TMPX
        LDA VOFF,X
        TAX
        LDA INSTR+I_AD,Y
        STA AUNT+ATTACK_DECAY,X
        LDA INSTR+I_SR,Y
        STA AUNT+SUSTAIN_RELEASE,X
        LDA INSTR+I_FLAGS,Y
        LSR
        LSR
        LSR
        LSR
        STA AUNT+TRIM,X
        LDA #0                  ; the gate off and on: the attack starts over
        STA AUNT+CONTROL,X
        LDA #1
        STA AUNT+CONTROL,X
        LDX TMPX
        RTS

; Voice X lets its note go.
noteoff:
        LDY VOFF,X
        LDA #0
        STA AUNT+CONTROL,Y
        RTS

; All three let go: the tune stopped.
quiet:  LDX #2
@voice: JSR noteoff
        DEX
        BPL @voice
        RTS

; One frame of voice X.
voice:  LDA VNOTE,X
        BNE @on
        RTS
@on:    LDY VTAB,X              ; the table: this frame's waveform and pitch
@step:  LDA TABLES,Y
        CMP #$FF
        BNE @have
        LDA TABLES+1,Y          ; its end: go on from where it says
        TAY
        JMP @step
@have:  STA TWAVE
        LDA TABLES+1,Y
        STA TNOTE
        INY
        INY
        TYA
        STA VTAB,X
        LDA TWAVE               ; waveform 0: the instrument's own
        BNE @wave
        LDY VINS,X
        LDA INSTR+I_WAVE,Y
        STA TWAVE
@wave:  LDA VMUTE,X
        BEQ @heard
        LDA #0                  ; no waveform: silence
        STA TWAVE
@heard: LDA TNOTE               ; bit 7: that note; else so many semitones up
        BMI @fixed
        CLC
        ADC VNOTE,X
        CMP #96
        BCC @pitch
        LDA #95
        BNE @pitch
@fixed: AND #$7F
@pitch: STA TNOTE
        TAY
        LDA FREQLO-1,Y
        STA FLO
        LDA FREQHI-1,Y
        STA FHI

        LDY VINS,X              ; vibrato: a semitone's step, shifted down
        LDA INSTR+I_VIB,Y
        BEQ @novib
        STA TMP
        LDA VAGE,X
        CMP #VIBDELAY
        BCC @novib
        LDY TNOTE
        SEC
        LDA FREQLO,Y
        SBC FREQLO-1,Y
        STA DLO
        LDA FREQHI,Y
        SBC FREQHI-1,Y
        STA DHI
        LDY TMP
@shift: LSR DHI
        ROR DLO
        DEY
        BNE @shift
        LDA FRAMEC              ; up one, up two, up one, level, and the same down
        AND #7
        TAY
        LDA VIBTAB,Y
        BEQ @novib
        BMI @under
        TAY
@up:    CLC
        LDA FLO
        ADC DLO
        STA FLO
        LDA FHI
        ADC DHI
        STA FHI
        DEY
        BNE @up
        BEQ @novib
@under: EOR #$FF
        TAY
        INY
@down:  SEC
        LDA FLO
        SBC DLO
        STA FLO
        LDA FHI
        SBC DHI
        STA FHI
        DEY
        BNE @down
@novib:
        LDY VINS,X              ; the pulse width, to and fro
        LDA INSTR+I_PWM,Y
        BEQ @nopwm
        STA TMP
        LDA VPWD,X
        BNE @narrow
        CLC
        LDA VPW,X
        ADC TMP
        STA VPW,X
        CMP #$D0
        BCC @nopwm
        INC VPWD,X
        BNE @nopwm
@narrow:
        SEC
        LDA VPW,X
        SBC TMP
        STA VPW,X
        CMP #$30
        BCS @nopwm
        LDA #0
        STA VPWD,X
@nopwm:
        LDY VOFF,X              ; and to the chip
        LDA FLO
        STA AUNT+FREQ_LO,Y
        LDA FHI
        STA AUNT+FREQ_HI,Y
        LDA VPW,X
        STA AUNT+WIDTH,Y
        LDA TWAVE
        STA AUNT+WAVE,Y

        LDA VAGE,X
        CMP #$FF
        BEQ @old
        INC VAGE,X
@old:   LDA FRAMEC              ; the meter falls a step every four frames
        AND #3
        BNE @level
        LDA VMETER,X
        BEQ @level
        DEC VMETER,X
@level: LDA VPREV,X             ; a note played by hand lets go by itself
        BEQ @done
        DEC VPREV,X
        BNE @done
        JSR noteoff
@done:  RTS

; The filter: which voices go through it, and its corner on its way up or down.
filter: LDA #0
        STA TMP
        LDX #2
@voice: LDY VINS,X
        LDA INSTR+I_FLAGS,Y
        AND #1
        BEQ @past
        LDA TMP
        ORA BITS,X
        STA TMP
@past:  DEX
        BPL @voice
        LDA FILTON
        BNE @in
        STA TMP
@in:    LDA TMP
        ORA #RES << 4
        STA FILTER
        LDA CUTDIR
        BNE @close
        INC CUT
        LDA CUT
        CMP #CUTMAX
        BCC @set
        INC CUTDIR
        BNE @set
@close: DEC CUT
        LDA CUT
        CMP #CUTMIN + 1
        BCS @set
        LDA #0
        STA CUTDIR
@set:   LDA CUT
        STA CUTOFF
        RTS

; ---------------------------------------------------------------- the keys

keys:   LDA KBD_STROBE
        BMI @key
        RTS
@key:   AND #$7F
        STA KEY
        LDA #1
        STA DIRTY
        LDA KEY
        CMP #' '
        BNE @k1
        JMP toggle
@k1:    CMP #13
        BNE @k2
        JMP fromtop
@k2:    CMP #'F'
        BNE @k3
        LDA FILTON
        EOR #1
        STA FILTON
        RTS
@k3:    LDX #2
@mute:  CMP MUTEKEYS,X
        BNE @heard
        LDA VMUTE,X
        EOR #1
        STA VMUTE,X
        RTS
@heard: DEX
        BPL @mute
        CMP #28                 ; left
        BNE @k4
        LDA CURV
        BEQ @r4
        DEC CURV
@r4:    RTS
@k4:    CMP #29                 ; right
        BNE @k5
        LDA CURV
        CMP #2
        BEQ @r5
        INC CURV
@r5:    RTS
@k5:    CMP #','
        BNE @k6
        LDA OCT
        BEQ @r6
        DEC OCT
@r6:    RTS
@k6:    CMP #'.'
        BNE @k7
        LDA OCT
        CMP #6
        BEQ @r7
        INC OCT
@r7:    RTS
@k7:    CMP #'1'
        BCC @k8
        CMP #'9' + 1
        BCS @k8
        SEC
        SBC #'0'
        STA INS
        RTS
@k8:    LDX PLAYING             ; the rest write to the tune: only while it's stopped
        BEQ edit
        RTS

edit:   CMP #30                 ; up
        BNE @e1
        LDA ROW
        BEQ @r1
        DEC ROW
@r1:    RTS
@e1:    CMP #31                 ; down
        BNE @e2
        JMP rowdown
@e2:    CMP #'['
        BNE @e3
        LDA POS
        BEQ @r3
        DEC POS
        JSR loadpos
@r3:    RTS
@e3:    CMP #']'
        BNE @e4
        INC POS
        JMP loadpos
@e4:    CMP #8                  ; DELETE
        BNE @e5
        LDA #0
        STA TNOTE
        STA TINS
        JMP putcell
@e5:    CMP #'='
        BNE @e6
        LDA #OFF
        STA TNOTE
        LDA #0
        STA TINS
        JMP putcell
@e6:    LDY #11                 ; a note?
@piano: CMP PIANO,Y
        BEQ @hit
        DEY
        BPL @piano
        RTS
@hit:   STY TMP                 ; note = octave x 12 + the key + 1
        LDA OCT
        ASL
        ASL
        STA TMP2
        ASL
        CLC
        ADC TMP2
        ADC TMP
        ADC #1
        STA TNOTE
        LDA INS
        STA TINS
        LDX CURV                ; heard as it's written
        JSR setins
        LDA TNOTE
        JSR noteon
        LDA #PREVIEW
        STA VPREV,X

; The cell under the cursor takes TNOTE and TINS, and the row moves on.
putcell:
        LDX CURV
        LDA ROW
        JSR cell
        LDY #0
        LDA TNOTE
        STA (PTR),Y
        INY
        LDA TINS
        STA (PTR),Y
rowdown:
        LDA ROW
        CMP #ROWS - 1
        BEQ @r
        INC ROW
@r:     RTS

; SPACE: play from the bright row, or stop.
toggle: LDA PLAYING
        EOR #1
        STA PLAYING
        BNE go
        JMP quiet
go:     LDA #1
        STA TICK
        LDA #0
        STA STEP
        RTS

; RETURN: play from the top.
fromtop:
        LDA #0
        STA POS
        STA ROW
        JSR loadpos
        LDA #1
        STA PLAYING
        BNE go

; ---------------------------------------------------------------- the screen

draw:   JSR meters
        LDA DIRTY
        BEQ @r
        LDA #0
        STA DIRTY
        JSR status
        JSR pattern
@r:     RTS

; What doesn't change: the names and the help.
panel:  LDX #0
@next:  LDA TEXTS,X
        BMI @done
        STA CURY
        LDA TEXTS+1,X
        STA CURX
        STX TMPX
        LDA TEXTS+3,X
        TAY
        LDA TEXTS+2,X
        TAX
        JSR PRSTR
        LDX TMPX
        INX
        INX
        INX
        INX
        BNE @next
@done:  RTS

; A as two hex digits at (SCR),Y, then Y past them. Keeps X.
puthex: PHA
        LSR
        LSR
        LSR
        LSR
        JSR putdig
        PLA
        AND #15
putdig: STX TMPX2
        TAX
        LDA HEX,X
        ORA REV
        STA (SCR),Y
        INY
        LDX TMPX2
        RTS

; The numbers along the top, and the three voices' headings.
status: LDA #0
        STA REV
        LDA #<[SCREEN + 40]
        STA SCR
        LDA #>[SCREEN + 40]
        STA SCR+1
        LDY #5
        LDA POS
        JSR puthex
        LDY #13
        LDA ROW
        JSR puthex
        LDY #21
        LDA SPEED
        JSR putdig
        LDY #29
        LDA OCT
        JSR putdig
        LDY #36
        LDA INS
        JSR putdig
        LDA PLAYING             ; row 2: STOP or PLAY, the filter IN or OUT
        ASL
        ASL
        TAX
        LDY #41
@word:  LDA WORDS,X
        STA (SCR),Y
        INX
        INY
        CPY #45
        BNE @word
        LDA FILTON
        ASL
        ASL
        TAX
        LDY #56
@filt:  LDA WORDS+8,X
        STA (SCR),Y
        INX
        INY
        CPY #60
        BNE @filt
        LDA #<[SCREEN + 120]    ; rows 3 and 4: each voice's pattern and its name
        STA SCR
        LDA #>[SCREEN + 120]
        STA SCR+1
        LDX #2
@voice: LDA #0
        STA REV
        LDA CELLCOL,X
        CLC
        ADC #4
        TAY
        LDA PAT,X
        JSR puthex
        CPX CURV                ; the voice the keys write to is reversed
        BNE @plain
        LDA #$80
        STA REV
@plain: STX TMPX
        LDA CELLCOL,X
        CLC
        ADC #40
        TAY
        LDA VMUTE,X
        BEQ @name
        LDA #6
@name:  TAX
        LDA #6
        STA CNT
@char:  LDA HEADS,X
        ORA REV
        STA (SCR),Y
        INX
        INY
        DEC CNT
        BNE @char
        LDA TMPX
        CLC
        ADC #'1'
        ORA REV
        STA (SCR),Y
        LDX TMPX
        DEX
        BPL @voice
        RTS

; The pattern rows round the bright one.
pattern:
        LDA #0
        STA LINE
@line:  LDY LINE
        LDA ROWLO+TOP,Y
        STA SCR
        LDA ROWHI+TOP,Y
        STA SCR+1
        CLC
        LDA ROW
        ADC LINE
        SEC
        SBC #MIDDLE
        STA DROW
        CMP #ROWS
        BCC @show
        LDY #38                 ; before the first row or after the last: nothing
        LDA #' '
@blank: STA (SCR),Y
        DEY
        BNE @blank
        BEQ @next
@show:  LDA #0
        STA REV
        LDA LINE
        CMP #MIDDLE
        BNE @dim
        LDA #$80
        STA REV
@dim:   LDY #1
        LDA DROW
        JSR puthex
        LDX #0
@cell:  LDA DROW
        JSR cell
        LDY #0
        LDA (PTR),Y
        STA TNOTE
        INY
        LDA (PTR),Y
        STA TINS
        LDY CELLCOL,X
        JSR putnote
        LDA #' '
        ORA REV
        STA (SCR),Y
        INY
        LDA TINS
        BNE @ins
        LDA #'.'
        ORA REV
        STA (SCR),Y
        BNE @after
@ins:   JSR putdig
@after: INX
        CPX #3
        BNE @cell
@next:  INC LINE
        LDA LINE
        CMP #VIEW
        BNE @line
        RTS

; TNOTE as three characters at (SCR),Y: C-4, F#5, --- for none, === for off.
putnote:
        LDA TNOTE
        BNE @some
        LDA #'-'
        BNE @three
@some:  CMP #OFF
        BNE @name
        LDA #'='
@three: ORA REV
        STA (SCR),Y
        INY
        STA (SCR),Y
        INY
        STA (SCR),Y
        INY
        RTS
@name:  STX TMPX
        TAX
        LDA OCTAVE-1,X
        PHA
        LDA SEMI-1,X
        ASL
        TAX
        LDA NAMES,X
        ORA REV
        STA (SCR),Y
        INY
        LDA NAMES+1,X
        ORA REV
        STA (SCR),Y
        INY
        PLA
        CLC
        ADC #'0'
        ORA REV
        STA (SCR),Y
        INY
        LDX TMPX
        RTS

; Each voice's meter: full at a new note, falling.
meters: LDA #<[SCREEN + 22 * 40]
        STA SCR
        LDA #>[SCREEN + 22 * 40]
        STA SCR+1
        LDX #2
@voice: LDY CELLCOL,X
        LDA #METER
        STA CNT
        LDA VMETER,X
        STA TMP
@cell:  LDA TMP
        BEQ @dot
        DEC TMP
        LDA #$0F                ; a solid block
        BNE @put
@dot:   LDA #'.'
@put:   STA (SCR),Y
        INY
        DEC CNT
        BNE @cell
        DEX
        BPL @voice
        RTS

; ---------------------------------------------------------------- tables

VOFF:   .byte 0, 8, 16          ; each voice's registers from AUNT
BITS:   .byte 1, 2, 4
SIXTYFOUR: .byte 0, 64, 128, 192
CELLCOL: .byte 6, 17, 28        ; each voice's column
MUTEKEYS: .text "QWE"
PIANO:  .text "ZSXDCVGBHNJM"
HEX:    .text "0123456789ABCDEF"
NAMES:  .text "C-C#D-D#E-F-F#G-G#A-A#B-"
HEADS:  .text "VOICE MUTED "
WORDS:  .text "STOPPLAYOUT IN  "
VIBTAB: .byte 0, 1, 2, 1, 0, $FF, $FE, $FF

SEMI:   .rept 96, n
        .byte n % 12
        .endr
OCTAVE: .rept 96, n
        .byte n / 12
        .endr
ROWLO:  .rept 25, n
        .byte <[SCREEN + n * 40]
        .endr
ROWHI:  .rept 25, n
        .byte >[SCREEN + n * 40]
        .endr

; The panel: row, column, the text's address; a row of $FF ends it.
TEXTS:  .byte 0, 1
        .word t_name
        .byte 0, 16
        .word SONGNAME
        .byte 1, 1
        .word t_top
        .byte 2, 9
        .word t_filt
        .byte 3, 6
        .word t_pats
        .byte 23, 1
        .word t_help1
        .byte 24, 1
        .word t_help2
        .byte $FF
t_name: .asciiz "AUNT TRACKER"
t_top:  .asciiz "POS     ROW     SPD     OCT    INS"
t_filt: .asciiz "FILTER"
t_pats: .asciiz "PAT        PAT        PAT"
t_help1: .asciiz "SPACE PLAY  Q W E MUTE  F FILTER  [ ]"
t_help2: .asciiz "Z-M NOTES  , . OCTAVE  1-9 INS  DEL  ="
nochip: .asciiz "THE TRACKER NEEDS THE SOUND CHIP."
nochip2: .asciiz "SWITCH OFF, FIT THE AUNT, LOAD AGAIN."

; The instruments, eight bytes each; number 0 isn't one.
I_WAVE  = 0             ; the waveform, unless the table gives another
I_AD    = 1             ; attack and decay
I_SR    = 2             ; sustain and release
I_WIDTH = 3             ; the pulse width a note starts with
I_PWM   = 4             ; how far the width moves a frame; 0: it stays
I_VIB   = 5             ; vibrato: a semitone shifted right this many times; 0: none
I_TABLE = 6             ; its table's place in TABLES
I_FLAGS = 7             ; bit 0: through the filter. Bits 4-7: TRIM

INSTR:  .fill 8
        .byte PULSE, $18, $A9, $60, 3, 3, t_plain - TABLES, $00    ; 1 the lead
        .byte PULSE, $06, $46, $50, 2, 0, t_minor - TABLES, $50    ; 2 a minor chord
        .byte PULSE, $06, $46, $50, 2, 0, t_major - TABLES, $50    ; 3 a major chord
        .byte SAW,   $07, $87, $80, 0, 0, t_plain - TABLES, $21    ; 4 the bass
        .byte TRI,   $08, $07, $80, 0, 0, t_kick - TABLES,  $00    ; 5 the kick
        .byte NOISE, $07, $06, $80, 0, 0, t_snare - TABLES, $30    ; 6 the snare
        .byte TRI,   $2A, $8B, $80, 0, 3, t_plain - TABLES, $00    ; 7 glass
        .byte NOISE, $01, $01, $80, 0, 0, t_hat - TABLES,   $80    ; 8 a hat
        .byte PULSE, $9A, $A9, $80, 1, 0, t_drift - TABLES, $41    ; 9 a slow chord

; The tables: a waveform (0: the instrument's) and a pitch for each frame of a
; note. A pitch is semitones above the note, or with bit 7 a note of its own.
; $FF ends a table, with where to go on from.
TABLES:
t_plain: .byte 0, 0
        .byte $FF, t_plain - TABLES
t_minor: .byte 0, 0,  0, 3,  0, 7
        .byte $FF, t_minor - TABLES
t_major: .byte 0, 0,  0, 4,  0, 7
        .byte $FF, t_major - TABLES
t_kick: .byte NOISE, $80 | 95           ; the stick
        .byte TRI, $80 | 39,  TRI, $80 | 34,  TRI, $80 | 29,  TRI, $80 | 25,  TRI, $80 | 22
t_kick2: .byte TRI, $80 | 20            ; the skin, falling to G1 and staying
        .byte $FF, t_kick2 - TABLES
t_snare: .byte NOISE, $80 | 95,  TRI, $80 | 46,  TRI, $80 | 44
t_snare2: .byte NOISE, $80 | 85
        .byte $FF, t_snare2 - TABLES
t_hat:  .byte NOISE, $80 | 95
        .byte $FF, t_hat - TABLES
t_drift: .byte 0, 0,  0, 0,  0, 0,  0, 7,  0, 7,  0, 7,  0, 12,  0, 12,  0, 12,  0, 7,  0, 7,  0, 7
        .byte $FF, t_drift - TABLES
        .assert * - TABLES <= 256, "the tables are more than a page"

; >>> song
; (written by scripts/tracker-song.py: change the tune there)

; FREQ for each note, C0 (1) to A#7 (95), and one more for the vibrato's reach:
; Hz x 16,777,216 / 1,000,000.
FREQLO: .byte $12, $23, $34, $46, $5A, $6E, $84, $9B, $B3, $CD, $E9, $06
        .byte $25, $45, $68, $8C, $B3, $DC, $08, $36, $67, $9B, $D2, $0C
        .byte $49, $8B, $D0, $19, $67, $B9, $10, $6C, $CE, $35, $A3, $17
        .byte $93, $15, $9F, $32, $CD, $72, $20, $D8, $9C, $6B, $46, $2F
        .byte $25, $2A, $3F, $64, $9A, $E3, $3F, $B1, $38, $D6, $8D, $5E
        .byte $4B, $55, $7E, $C8, $34, $C6, $7F, $61, $6F, $AC, $1A, $BC
        .byte $95, $A9, $FC, $8F, $69, $8C, $FE, $C2, $DF, $58, $34, $78
        .byte $2B, $53, $F7, $1F, $D2, $19, $FC, $85, $BD, $B0, $67, $FF
FREQHI: .byte $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $02
        .byte $02, $02, $02, $02, $02, $02, $03, $03, $03, $03, $03, $04
        .byte $04, $04, $04, $05, $05, $05, $06, $06, $06, $07, $07, $08
        .byte $08, $09, $09, $0A, $0A, $0B, $0C, $0C, $0D, $0E, $0F, $10
        .byte $11, $12, $13, $14, $15, $16, $18, $19, $1B, $1C, $1E, $20
        .byte $22, $24, $26, $28, $2B, $2D, $30, $33, $36, $39, $3D, $40
        .byte $44, $48, $4C, $51, $56, $5B, $60, $66, $6C, $73, $7A, $81
        .byte $89, $91, $99, $A3, $AC, $B7, $C1, $CD, $D9, $E6, $F4, $FF

SONGSPEED = 6
SONGPATS = 37
SONGNAME:
        .asciiz "ROB'S YOUR UNCLE"

; The order: a pattern for each voice at each position. $FF, then where to go on from.
        .org ORDER
        .byte  0,  1,  2      ;  0
        .byte  3,  4,  5      ;  1
        .byte  6,  7,  8      ;  2
        .byte  9, 10, 11      ;  3
        .byte 12, 13, 14      ;  4
        .byte 15, 16, 17      ;  5
        .byte 18, 19, 20      ;  6
        .byte 21, 22, 23      ;  7
        .byte 24, 13, 14      ;  8
        .byte 25, 16, 17      ;  9
        .byte 26, 19, 20      ; 10
        .byte 27, 22, 23      ; 11
        .byte 28, 29, 30      ; 12
        .byte 31, 19, 20      ; 13
        .byte 32, 33, 34      ; 14
        .byte 35, 36, 23      ; 15
        .byte 12, 13, 14      ; 16
        .byte 15, 16, 17      ; 17
        .byte 18, 19, 20      ; 18
        .byte 21, 22, 23      ; 19
        .byte $FF, 4

; The patterns: 32 rows of note, instrument. A row a pair, eight rows a line.
        .org PATTERNS
; pattern 0
        .byte $41,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 1
        .byte $2E,9, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 2
        .byte $16,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $16,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 3
        .byte $49,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 4
        .byte $2A,9, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 5
        .byte $12,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $12,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 6
        .byte $48,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $44,7, $00,0, $00,0, $00,0
        .byte $4B,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 7
        .byte $2C,9, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 8
        .byte $14,5, $00,0, $14,4, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $14,5, $00,0, $14,4, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 9
        .byte $4D,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $45,7, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 10
        .byte $29,9, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 11
        .byte $11,5, $00,0, $11,4, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $11,5, $00,0, $11,4, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $11,6, $00,0, $11,6, $00,0, $11,6, $11,6, $11,6, $11,6
; pattern 12
        .byte $3A,1, $00,0, $00,0, $3D,1, $00,0, $00,0, $41,1, $00,0
        .byte $00,0, $00,0, $3F,1, $00,0, $3D,1, $00,0, $3C,1, $00,0
        .byte $3A,1, $00,0, $00,0, $00,0, $00,0, $00,0, $35,1, $00,0
        .byte $3A,1, $00,0, $3C,1, $00,0, $3D,1, $00,0, $3F,1, $00,0
; pattern 13
        .byte $3A,2, $00,0, $00,0, $3A,2, $00,0, $00,0, $3A,2, $00,0
        .byte $00,0, $00,0, $3A,2, $00,0, $00,0, $3A,2, $00,0, $00,0
        .byte $3A,2, $00,0, $00,0, $3A,2, $00,0, $00,0, $3A,2, $00,0
        .byte $00,0, $00,0, $3A,2, $00,0, $00,0, $3A,2, $00,0, $00,0
; pattern 14
        .byte $16,5, $00,0, $16,4, $22,4, $16,6, $00,0, $16,4, $22,4
        .byte $16,5, $00,0, $16,4, $22,4, $16,6, $00,0, $1D,4, $22,4
        .byte $16,5, $00,0, $16,4, $22,4, $16,6, $00,0, $16,4, $22,4
        .byte $16,5, $00,0, $16,4, $22,4, $16,6, $00,0, $1D,4, $22,4
; pattern 15
        .byte $3D,1, $00,0, $00,0, $42,1, $00,0, $00,0, $46,1, $00,0
        .byte $00,0, $00,0, $44,1, $00,0, $42,1, $00,0, $41,1, $00,0
        .byte $42,1, $00,0, $00,0, $00,0, $00,0, $00,0, $3D,1, $00,0
        .byte $3A,1, $00,0, $3D,1, $00,0, $42,1, $00,0, $46,1, $00,0
; pattern 16
        .byte $36,3, $00,0, $00,0, $36,3, $00,0, $00,0, $36,3, $00,0
        .byte $00,0, $00,0, $36,3, $00,0, $00,0, $36,3, $00,0, $00,0
        .byte $36,3, $00,0, $00,0, $36,3, $00,0, $00,0, $36,3, $00,0
        .byte $00,0, $00,0, $36,3, $00,0, $00,0, $36,3, $00,0, $00,0
; pattern 17
        .byte $12,5, $00,0, $12,4, $1E,4, $12,6, $00,0, $12,4, $1E,4
        .byte $12,5, $00,0, $12,4, $1E,4, $12,6, $00,0, $19,4, $1E,4
        .byte $12,5, $00,0, $12,4, $1E,4, $12,6, $00,0, $12,4, $1E,4
        .byte $12,5, $00,0, $12,4, $1E,4, $12,6, $00,0, $19,4, $1E,4
; pattern 18
        .byte $48,1, $00,0, $00,0, $46,1, $00,0, $00,0, $44,1, $00,0
        .byte $00,0, $00,0, $3F,1, $00,0, $44,1, $00,0, $46,1, $00,0
        .byte $48,1, $00,0, $00,0, $00,0, $46,1, $00,0, $44,1, $00,0
        .byte $3F,1, $00,0, $00,0, $00,0, $3C,1, $00,0, $3F,1, $00,0
; pattern 19
        .byte $38,3, $00,0, $00,0, $38,3, $00,0, $00,0, $38,3, $00,0
        .byte $00,0, $00,0, $38,3, $00,0, $00,0, $38,3, $00,0, $00,0
        .byte $38,3, $00,0, $00,0, $38,3, $00,0, $00,0, $38,3, $00,0
        .byte $00,0, $00,0, $38,3, $00,0, $00,0, $38,3, $00,0, $00,0
; pattern 20
        .byte $14,5, $00,0, $14,4, $20,4, $14,6, $00,0, $14,4, $20,4
        .byte $14,5, $00,0, $14,4, $20,4, $14,6, $00,0, $1B,4, $20,4
        .byte $14,5, $00,0, $14,4, $20,4, $14,6, $00,0, $14,4, $20,4
        .byte $14,5, $00,0, $14,4, $20,4, $14,6, $00,0, $1B,4, $20,4
; pattern 21
        .byte $41,1, $00,0, $00,0, $00,0, $00,0, $00,0, $44,1, $00,0
        .byte $3C,1, $00,0, $00,0, $00,0, $41,1, $00,0, $00,0, $00,0
        .byte $41,1, $3F,1, $3D,1, $3C,1, $3A,1, $3C,1, $3D,1, $3F,1
        .byte $41,1, $00,0, $00,0, $00,0, $39,1, $00,0, $00,0, $00,0
; pattern 22
        .byte $35,2, $00,0, $00,0, $35,2, $00,0, $00,0, $35,2, $00,0
        .byte $00,0, $00,0, $35,2, $00,0, $00,0, $35,2, $00,0, $00,0
        .byte $35,3, $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0
        .byte $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0, $00,0
; pattern 23
        .byte $11,5, $00,0, $11,4, $1D,4, $11,6, $00,0, $11,4, $1D,4
        .byte $11,5, $00,0, $11,4, $1D,4, $11,6, $00,0, $18,4, $1D,4
        .byte $11,5, $00,0, $11,4, $1D,4, $11,6, $00,0, $11,4, $1D,4
        .byte $11,5, $00,0, $11,4, $1D,4, $11,6, $00,0, $18,4, $1D,4
; pattern 24
        .byte $46,1, $00,0, $00,0, $49,1, $00,0, $00,0, $4D,1, $00,0
        .byte $00,0, $00,0, $4B,1, $00,0, $49,1, $00,0, $48,1, $00,0
        .byte $46,1, $00,0, $00,0, $00,0, $00,0, $00,0, $41,1, $00,0
        .byte $46,1, $00,0, $48,1, $00,0, $49,1, $00,0, $4B,1, $00,0
; pattern 25
        .byte $49,1, $00,0, $00,0, $4E,1, $00,0, $00,0, $52,1, $00,0
        .byte $00,0, $00,0, $50,1, $00,0, $4E,1, $00,0, $4D,1, $00,0
        .byte $4E,1, $00,0, $00,0, $00,0, $00,0, $00,0, $49,1, $00,0
        .byte $46,1, $00,0, $49,1, $00,0, $4E,1, $00,0, $52,1, $00,0
; pattern 26
        .byte $54,1, $00,0, $00,0, $52,1, $00,0, $00,0, $50,1, $00,0
        .byte $00,0, $00,0, $4B,1, $00,0, $50,1, $00,0, $52,1, $00,0
        .byte $54,1, $00,0, $00,0, $00,0, $52,1, $00,0, $50,1, $00,0
        .byte $4B,1, $00,0, $00,0, $00,0, $48,1, $00,0, $4B,1, $00,0
; pattern 27
        .byte $4D,1, $00,0, $00,0, $00,0, $00,0, $00,0, $50,1, $00,0
        .byte $48,1, $00,0, $00,0, $00,0, $4D,1, $00,0, $00,0, $00,0
        .byte $4D,1, $4B,1, $49,1, $48,1, $46,1, $48,1, $49,1, $4B,1
        .byte $4D,1, $00,0, $00,0, $00,0, $45,1, $00,0, $00,0, $00,0
; pattern 28
        .byte $3F,1, $00,0, $42,1, $00,0, $46,1, $00,0, $00,0, $00,0
        .byte $4B,1, $00,0, $00,0, $00,0, $49,1, $00,0, $46,1, $00,0
        .byte $42,1, $00,0, $00,0, $00,0, $46,1, $00,0, $00,0, $00,0
        .byte $44,1, $00,0, $42,1, $00,0, $41,1, $00,0, $3F,1, $00,0
; pattern 29
        .byte $33,2, $00,0, $00,0, $33,2, $00,0, $00,0, $33,2, $00,0
        .byte $00,0, $00,0, $33,2, $00,0, $00,0, $33,2, $00,0, $00,0
        .byte $33,2, $00,0, $00,0, $33,2, $00,0, $00,0, $33,2, $00,0
        .byte $00,0, $00,0, $33,2, $00,0, $00,0, $33,2, $00,0, $00,0
; pattern 30
        .byte $1B,5, $00,0, $1B,4, $27,4, $1B,6, $00,0, $1B,4, $27,4
        .byte $1B,5, $00,0, $1B,4, $27,4, $1B,6, $00,0, $22,4, $27,4
        .byte $1B,5, $00,0, $1B,4, $27,4, $1B,6, $00,0, $1B,4, $27,4
        .byte $1B,5, $00,0, $1B,4, $27,4, $1B,6, $00,0, $22,4, $27,4
; pattern 31
        .byte $3F,1, $00,0, $44,1, $00,0, $48,1, $00,0, $00,0, $00,0
        .byte $4B,1, $00,0, $00,0, $00,0, $00,0, $00,0, $48,1, $00,0
        .byte $44,1, $00,0, $00,0, $00,0, $48,1, $00,0, $00,0, $00,0
        .byte $46,1, $00,0, $44,1, $00,0, $42,1, $00,0, $3F,1, $00,0
; pattern 32
        .byte $41,1, $00,0, $44,1, $00,0, $49,1, $00,0, $00,0, $00,0
        .byte $4D,1, $00,0, $00,0, $00,0, $4B,1, $00,0, $49,1, $00,0
        .byte $44,1, $00,0, $00,0, $00,0, $00,0, $00,0, $41,1, $00,0
        .byte $3D,1, $00,0, $41,1, $00,0, $44,1, $00,0, $49,1, $00,0
; pattern 33
        .byte $3D,3, $00,0, $00,0, $3D,3, $00,0, $00,0, $3D,3, $00,0
        .byte $00,0, $00,0, $3D,3, $00,0, $00,0, $3D,3, $00,0, $00,0
        .byte $3D,3, $00,0, $00,0, $3D,3, $00,0, $00,0, $3D,3, $00,0
        .byte $00,0, $00,0, $3D,3, $00,0, $00,0, $3D,3, $00,0, $00,0
; pattern 34
        .byte $19,5, $00,0, $19,4, $25,4, $19,6, $00,0, $19,4, $25,4
        .byte $19,5, $00,0, $19,4, $25,4, $19,6, $00,0, $20,4, $25,4
        .byte $19,5, $00,0, $19,4, $25,4, $19,6, $00,0, $19,4, $25,4
        .byte $19,5, $00,0, $19,4, $25,4, $19,6, $00,0, $20,4, $25,4
; pattern 35
        .byte $48,1, $00,0, $00,0, $00,0, $00,0, $00,0, $45,1, $00,0
        .byte $41,1, $00,0, $00,0, $00,0, $45,1, $00,0, $00,0, $00,0
        .byte $48,1, $45,1, $41,1, $45,1, $48,1, $45,1, $41,1, $45,1
        .byte $4D,1, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 36
        .byte $35,3, $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0
        .byte $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0, $00,0
        .byte $35,3, $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0
        .byte $00,0, $00,0, $35,3, $00,0, $00,0, $35,3, $00,0, $00,0
; <<< song
