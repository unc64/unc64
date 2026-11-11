; MUSIC TRACKER for the UNC 64's sound chip. Text mode, 50 Hz.
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
;   T           the next tune on the tape
;   Q W E       voice 1, 2, 3 silent, or heard again
;   F           the filter out of the circuit, or back in
;   1-9         the instrument the keys play; O and P step through all of them
;   K L         the pattern this voice plays at this position: the one before, the next
;   A R         a new empty position added after this one; this one removed
;   Y U         the tune slower, faster: the next tempo down or up, shown as BPM;
;               these two work while it plays as well
;   I           the instruments' screen, and back: the instrument's wave and its
;               envelope drawn, each thing it is made of on a row of its own (up and
;               down choose, left and right change, or type hex), and its table;
;               SPACE there plays a note
;   -           a pattern no tune uses yet, emptied, for this voice at this position
;   0           the tune emptied (it asks first): one position of three new patterns,
;               and the name TUNE #1 to #4
;   ;           the tune named
;   ?           help: every key, on a screen of its own; any key goes back
;   /           SAVE: the tracker and everything in it, as a program on the tape
;   \           LOAD: a tracker saved that way, in place of this one
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
;   - lets the note go once it is as old as the instrument's GATE says;
;   - steps the instrument's TABLE: a waveform and a pitch for this frame,
;     which is how one voice plays a chord (the arpeggio, a note a frame) and
;     how it plays a drum (a frame of noise, then a triangle falling);
;   - adds the vibrato, once the note is a quarter of a second old;
;   - if the note was slid into, adds what is left of the slide, and halves it;
;   - moves the pulse width between $30 and $D0;
;   - writes FREQ, WIDTH and WAVE.
;
; A new note writes the envelope and TRIM and takes the gate off and on, so
; the attack starts over; an instrument that SLIDES leaves the gate alone when
; the voice is still sounding, and comes to the new note from the old one. An
; instrument that LETS GO has its note let go two frames before the next one
; on its voice, so that one starts clean. An instrument that DRAWS starts the
; wave table being redrawn, a frame of the plucked string at a time: every
; voice whose waveform is the table hears it. The filter's corner sweeps up
; and down by itself, as fast as the tune says (a step a frame is slow; nine is
; there and back in twenty frames, a wobble); an instrument says whether it
; goes through.
;
; Everything a tune on this tape is made of is here, and all of it can be
; changed from the keys: the notes, the order of the patterns, the speed, the
; instruments and their tables. / saves the lot on a tape; \ loads it back. The instruments, the
; string's frames, the note table and the tunes are written by
; scripts/tracker-song.py.

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
WAVETABLE = $C0E0       ; the drawn waveform, 32 steps
SOUND   = $C0DF         ; $16 with the chip in
SOUND_ID = $16

TRI     = $01
SAW     = $02
PULSE   = $04
NOISE   = $08
DRAWN   = $10           ; the wave table

; the ROM (chapter 13)
CLS     = $FF00
CHROUT  = $FF03
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

PATTERNS = $2800        ; up to 96 patterns of 64 bytes, to the top of a UNC 16
MEMTOP  = $4000
NPATS   = [MEMTOP - PATTERNS] / 64
SLOT    = 96            ; bytes of order a tune has: 31 positions and the end
TAPE    = $C040         ; the deck (chapter 14)
TAPEDATA = $C041
BORDER  = $C020
LOAD    = $FF15

; ---------------------------------------------------------------- fast page
; The tests read the tracker's state from here, by name.

        .org $00
PLAYING: .res 1         ; 1 while the tune plays
SONG:   .res 1          ; which tune, 0 to SONGS - 1
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
CUTSTEP: .res 1         ; how far it moves a frame: the tune says
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
VGATE:  .res 3          ; 1 while its gate is on
VSL:    .res 3          ; what is left of its slide, added to FREQ
VSH:    .res 3
VFL:    .res 3          ; the FREQ it last had
VFH:    .res 3
DRAWON: .res 1          ; 1 while the wave table is being redrawn
DRAWAGE: .res 1         ; frames since the note that started it
HBACK:  .res 1          ; the screen ? was pressed on
MODE:   .res 1          ; 0 the tune's screen, 1 the instruments', 2 help
ECUR:   .res 1          ; there: what the cursor is on, 0-11 the instrument's parts, 12-27 its table's bytes
ENIB:   .res 1          ; 1: the next digit is the low one
VARS_END:
SUM:    .res 1          ; the tape's checksum
SKIP:   .res 1          ; a tune `scan` leaves out
IOFF:   .res 1          ; the instrument's place in INSTR
GROW:   .res 1          ; a graph being drawn: its row,
GBASE:  .res 1          ; and the level its cells start from
NPAT:   .res 3          ; the three patterns at the position after POS
TPAT:   .res 1          ; the pattern `cellp` looks in
TIED:   .res 1
WPTR:   .res 2          ; a frame of the string
ORDP:   .res 2          ; this tune's order
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
        JSR title

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

; The tune SONG, from its top: its order, its speed.
setsong:
        LDA SONG
        ASL
        ASL
        TAX
        LDA SONGTAB,X
        STA ORDP
        LDA SONGTAB+1,X
        STA ORDP+1
        LDA SONGTAB+2,X
        STA SPEED
        LDA SONGTAB+3,X
        STA CUTSTEP
        LDA #0
        STA POS
        STA ROW

; PAT = the three patterns at POS. At the order's $FF, POS goes where it says.
loadpos:
        LDA POS
        ASL
        CLC
        ADC POS
        TAY
        LDA (ORDP),Y
        CMP #$FF
        BNE @ok
        INY
        LDA (ORDP),Y
        STA POS
        JMP loadpos
@ok:    STA PAT
        INY
        LDA (ORDP),Y
        STA PAT+1
        INY
        LDA (ORDP),Y
        STA PAT+2
        INY                     ; and the three after them, for looking a row ahead
        LDA (ORDP),Y
        CMP #$FF
        BNE @next
        INY
        LDA (ORDP),Y
        STA TMP
        ASL
        CLC
        ADC TMP
        TAY
        LDA (ORDP),Y
@next:  STA NPAT
        INY
        LDA (ORDP),Y
        STA NPAT+1
        INY
        LDA (ORDP),Y
        STA NPAT+2
        RTS

; PTR = voice X's cell at row A. Keeps X.
cell:   PHA
        LDA PAT,X
        STA TPAT
        PLA

; PTR = the cell at row A of pattern TPAT. Keeps X.
cellp:  ASL
        STA PTR
        LDA TPAT
        AND #3
        TAY
        LDA SIXTYFOUR,Y
        ORA PTR
        STA PTR
        LDA TPAT
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
        BNE @early
        LDA SPEED
        STA TICK
        JSR dorow
        JMP @voices
@early: LDA TICK                ; two frames before the next row
        CMP #2
        BNE @voices
        JSR ahead
@voices:
        LDX #2
@voice: JSR voice
        DEX
        BPL @voice
        JSR redraw
        JMP filter

; Two frames before the next row: a voice whose instrument LETS GO, and which
; has a note coming, lets its note go now, so the new one starts clean.
ahead:  LDX #2
@voice: LDY VINS,X
        LDA INSTR+I_FLAGS,Y
        AND #F_LETGO
        BEQ @next
        LDA ROW
        CMP #ROWS - 1
        BNE @same
        LDA NPAT,X              ; the next row is the next position's first
        STA TPAT
        LDA #0
        BEQ @look
@same:  LDA PAT,X
        STA TPAT
        LDA ROW
        CLC
        ADC #1
@look:  JSR cellp
        LDY #0
        LDA (PTR),Y
        BEQ @next
        JSR noteoff
@next:  DEX
        BPL @voice
        RTS

; The wave table, while a note of the string is young: this frame's drawing of
; it, if it has one (SCHED says which, or $FF for none), copied to the chip.
redraw: LDA DRAWON
        BEQ @r
        LDY DRAWAGE
        LDA SCHED,Y
        BMI @skip
        TAY
        LDA WAVELO,Y
        STA WPTR
        LDA WAVEHI,Y
        STA WPTR+1
        LDY #31
@copy:  LDA (WPTR),Y
        STA WAVETABLE,Y
        DEY
        BPL @copy
@skip:  INC DRAWAGE
        LDA DRAWAGE
        CMP #SCHEDLEN
        BCC @r
        LDA #0
        STA DRAWON
@r:     RTS

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
        STA VSL,X
        STA VSH,X
        STA TIED
        LDA #METER
        STA VMETER,X
        LDA INSTR+I_FLAGS,Y
        STA TMP2
        AND #F_DRAWS            ; the string: the table starts over with the note
        BEQ @drawn
        LDA #0
        STA DRAWAGE
        LDA #1
        STA DRAWON
@drawn: LDA TMP2
        AND #F_SLIDES
        BEQ @struck
        LDA VGATE,X
        BEQ @struck
        INC TIED                ; still sounding: the slide is from where it is to the new note
        STY TMP
        LDY VNOTE,X
        SEC
        LDA VFL,X
        SBC FREQLO-1,Y
        STA VSL,X
        LDA VFH,X
        SBC FREQHI-1,Y
        STA VSH,X
        LDY TMP
@struck:
        STX TMPX
        LDA VOFF,X
        TAX
        LDA INSTR+I_AD,Y
        STA AUNT+ATTACK_DECAY,X
        LDA INSTR+I_SR,Y
        STA AUNT+SUSTAIN_RELEASE,X
        LDA TMP2
        LSR
        LSR
        LSR
        LSR
        STA AUNT+TRIM,X
        LDA TIED
        BNE @on
        STA AUNT+CONTROL,X      ; the gate off and on: the attack starts over
        LDA #1
        STA AUNT+CONTROL,X
@on:    LDX TMPX
        LDA #1
        STA VGATE,X
        RTS

; Voice X lets its note go.
noteoff:
        LDY VOFF,X
        LDA #0
        STA AUNT+CONTROL,Y
        STA VGATE,X
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
@on:    LDY VINS,X              ; old enough to be let go?
        LDA INSTR+I_GATE,Y
        BEQ @held
        CMP VAGE,X
        BNE @held
        JSR noteoff
@held:  LDY VTAB,X              ; the table: this frame's waveform and pitch
        LDA TABLES,Y
        CMP #$FF
        BNE @have
        LDA TABLES+1,Y          ; its end: go on from where it says
        TAY
        LDA TABLES,Y
        CMP #$FF
        BNE @have
        LDA #0                  ; an end that sends you to an end (a table typed in wrong): the note alone
        STA TWAVE
        STA TNOTE
        BEQ @got
@have:  STA TWAVE
        LDA TABLES+1,Y
        STA TNOTE
        INY
        INY
        TYA
        STA VTAB,X
@got:   LDA TWAVE               ; waveform 0: the instrument's own
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
        LDA INSTR+I_PWM,Y
        LSR
        LSR
        LSR
        LSR
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
@novib: LDA VSL,X               ; what is left of a slide: added, and halved
        ORA VSH,X
        BEQ @level2
        CLC
        LDA FLO
        ADC VSL,X
        STA FLO
        LDA FHI
        ADC VSH,X
        STA FHI
        LDA VSH,X
        CMP #$80
        ROR VSH,X
        ROR VSL,X
        LDA VSL,X               ; from below it stops a step short: call that there
        AND VSH,X
        CMP #$FF
        BNE @level2
        LDA #0
        STA VSL,X
        STA VSH,X
@level2:
        LDY VINS,X              ; the pulse width, to and fro
        LDA INSTR+I_PWM,Y
        AND #$0F
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
        STA VFL,X
        LDA FHI
        STA AUNT+FREQ_HI,Y
        STA VFH,X
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
        CLC
        LDA CUT
        ADC CUTSTEP
        STA CUT
        CMP #CUTMAX
        BCC @set
        INC CUTDIR
        BNE @set
@close: SEC
        LDA CUT
        SBC CUTSTEP
        STA CUT
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
        LDX MODE
        CPX #2
        BNE @some
        JMP hback               ; on the help screen any key goes back
@some:  LDA KEY
        CMP #'?'
        BNE @nohelp
        JMP tohelp
@nohelp:
        CPX #0
        BEQ @tune
        JMP ikeys
@tune:  LDA KEY
        CMP #' '
        BNE @k1
        JMP toggle
@k1:    CMP #13
        BNE @k2
        JMP fromtop
@k2:    CMP #'T'
        BNE @k2b
        JMP nexttune
@k2b:   CMP #'F'
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
        BCC @k7b
        CMP #'9' + 1
        BCS @k7b
        SEC
        SBC #'0'
        STA INS
        RTS
@k7b:   CMP #'O'                ; the instrument before
        BNE @k7c
        LDA INS
        CMP #2
        BCC @r8
        DEC INS
@r8:    RTS
@k7c:   CMP #'P'                ; and the one after
        BNE @k7d
        LDA INS
        CMP #NINS
        BCS @r9
        INC INS
@r9:    RTS
@k7d:   CMP #'Y'                ; the tempo, playing or not: Y, the left of the two, the next one
        BNE @k7e                ; down, U the next one up
        JMP slower
@k7e:   CMP #'U'
        BNE @k8
        JMP faster
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
@e6:    CMP #'K'
        BNE @e7
        JMP patdown
@e7:    CMP #'L'
        BNE @e8
        JMP patup
@e8:    CMP #'A'
        BNE @e9
        JMP addpos
@e9:    CMP #'R'
        BNE @e10
        JMP delpos
@e10:   CMP #'I'
        BNE @e13
        JMP toins
@e13:   CMP #'/'
        BNE @e14
        JMP save
@e14:   CMP #$5C
        BNE @e15
        JMP load
@e15:   CMP #'-'
        BNE @e16
        JMP newpat
@e16:   CMP #'0'
        BNE @e17
        JMP empty
@e17:   CMP #';'
        BNE @e18
        JMP rename
@e18:   LDY #11                 ; a note?
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

; ---------------------------------------------------------------- the order

; Y = where POS is in the order.
posy:   LDA POS
        ASL
        CLC
        ADC POS
        TAY
        RTS

; Y = where the order ends: its $FF.
ordend: LDY #0
@look:  LDA (ORDP),Y
        CMP #$FF
        BEQ @r
        INY
        INY
        INY
        BNE @look
@r:     RTS

; K and L: the pattern the cursor's voice plays here, one down, one up, and round.
patdown:
        JSR posy
        TYA
        CLC
        ADC CURV
        TAY
        LDA (ORDP),Y
        BNE @ok
        LDA #NPATS
@ok:    SEC
        SBC #1
        STA (ORDP),Y
        JMP loadpos
patup:  JSR posy
        TYA
        CLC
        ADC CURV
        TAY
        LDA (ORDP),Y
        CLC
        ADC #1
        CMP #NPATS
        BCC @ok
        LDA #0
@ok:    STA (ORDP),Y
        JMP loadpos

; A: a position after this one, empty: three patterns no tune uses yet (if there are none
; left, it plays what this one plays). Everything from here up moves three bytes.
addpos: JSR ordend
        CPY #SLOT - 5
        BCS @r
        STY TMP                 ; the end
        JSR posy
        STY TMP2
        LDY TMP
        INY                     ; from the byte after the $FF (where it goes round from), down to here
@up:    LDA (ORDP),Y
        INY
        INY
        INY
        STA (ORDP),Y
        DEY
        DEY
        DEY
        CPY TMP2
        BEQ @moved
        DEY
        JMP @up
@moved: LDA TMP                 ; a tune that goes round from later than here goes round from one later
        CLC
        ADC #4
        TAY
        LDA (ORDP),Y
        CMP POS
        BCC @keep
        BEQ @keep
        CLC
        ADC #1
        STA (ORDP),Y
@keep:  INC POS
        LDA #$FF                ; and its three voices get patterns nothing uses, emptied,
        STA SKIP                ; while there are any: a new page to write on
        JSR scan
        LDA #0
        STA CNT
@fresh: JSR freepat
        BCS @done
        PHA
        JSR posy
        TYA
        CLC
        ADC CNT
        TAY
        PLA
        STA (ORDP),Y
        INC CNT
        LDA CNT
        CMP #3
        BNE @fresh
@done:  JMP loadpos
@r:     RTS

; R: this position out, unless it's the only one. Everything after it moves down three.
delpos: JSR ordend
        CPY #4
        BCC @r
        DEY
        STY TMP                 ; the end, less one
        JSR posy
@down:  INY
        INY
        INY
        LDA (ORDP),Y
        DEY
        DEY
        DEY
        STA (ORDP),Y
        INY
        CPY TMP
        BNE @down
        DEY                     ; where it goes round from: one sooner, if that was after here
        LDA (ORDP),Y
        CMP POS
        BCC @keep
        BEQ @keep
        SEC
        SBC #1
        STA (ORDP),Y
@keep:  JSR posy                ; the last position went: the one before it
        INY
        INY
        CPY TMP
        BCC @in
        DEC POS
@in:    JMP loadpos
@r:     RTS

; Y and U: the next tempo down, the next up (a frame more to a row, a frame fewer: BPMS).
; The tune keeps it.
slower: LDA SPEED
        CMP #18
        BCS speed@r
        INC SPEED
        BNE speed
faster: LDA SPEED
        CMP #4
        BCC speed@r
        DEC SPEED
speed:  LDA SONG
        ASL
        ASL
        TAX
        LDA SPEED
        STA SONGTAB+2,X
@r:     RTS

; ---------------------------------------------------------------- the instruments' screen

toins:  LDA #0
        STA ECUR
        STA ENIB
insscr: LDA #1
        STA MODE
        JSR CLS
        LDX #ITEXTS - TEXTS
        JMP panelx

; ?: help, every key, from either screen, playing or not (the tune misses a few frames while the
; screen is printed). Any key: back where it was.
tohelp: LDA MODE
        STA HBACK
        LDA #2
        STA MODE
        JSR CLS
        LDA #<MHELP
        STA PTR
        LDA #>MHELP
        STA PTR+1
@next:  LDY #0
        LDA (PTR),Y
        BEQ @r
        CMP #9                  ; on to column 17, where what the key does begins
        BNE @char
        LDA #17
        STA CURX
        BNE @on
@char:  JSR CHROUT
@on:    INC PTR
        BNE @next
        INC PTR+1
        BNE @next
@r:     RTS
hback:  LDA HBACK
        BNE insscr
        STA MODE
        JMP back

NPARAM  = 12            ; an instrument's parts: where each is, and how it sits there
P_BYTE: .byte I_WAVE, I_AD, I_AD, I_SR, I_SR, I_WIDTH, I_PWM, I_PWM, I_GATE, I_TABLE, I_FLAGS, I_FLAGS
P_KIND: .byte 0, 1, 2, 1, 2, 0, 2, 1, 0, 0, 1, 2       ; 0 the whole byte, 1 its high digit, 2 its low
P_STEP: .byte 1, 1, 1, 1, 1, 4, 1, 1, 1, 2, 1, 1       ; how far left and right move it
t_par:  .text "WAV A   D   S   R   WID PWM VIB GAT TAB TRM FLG "
ESTEP:  .byte 32, 16, 11, 8, 7, 6, 5, 4                 ; an attack of 1 to 8 columns climbs this much a column
VALS:   .fill 32                                        ; a graph's 32 levels, 0 to 31

ioff:   LDA INS
        ASL
        ASL
        ASL
        STA IOFF
        RTS

; A = part X of the instrument.
getpar: LDA IOFF
        CLC
        ADC P_BYTE,X
        TAY
        LDA INSTR,Y
        LDY P_KIND,X
        BEQ @r
        DEY
        BNE @low
        LSR
        LSR
        LSR
        LSR
        RTS
@low:   AND #$0F
@r:     RTS

; Part X of the instrument becomes A.
setpar: STA TMP
        LDA IOFF
        CLC
        ADC P_BYTE,X
        TAY
        LDA P_KIND,X
        BNE @part
        LDA TMP
        STA INSTR,Y
        RTS
@part:  CMP #1
        BNE @low
        LDA TMP
        ASL
        ASL
        ASL
        ASL
        STA TMP
        LDA INSTR,Y
        AND #$0F
        ORA TMP
        STA INSTR,Y
        RTS
@low:   LDA INSTR,Y
        AND #$F0
        ORA TMP
        STA INSTR,Y
        RTS

; REV = reversed if A is what the cursor is on.
irev:   CMP ECUR
        BEQ @on
        LDA #0
        BEQ @set
@on:    LDA #$80
@set:   STA REV
        RTS

; VALS as a graph four rows high from row A, columns 1 to 32: a cell is eight levels, drawn
; with the glyphs that fill a cell from the bottom an eighth at a time.
graph:  STA GROW
        LDA #24
        STA GBASE
@row:   LDX GROW
        LDA ROWLO,X
        STA SCR
        LDA ROWHI,X
        STA SCR+1
        LDX #31
@cell:  TXA
        TAY
        INY
        LDA VALS,X
        CLC
        ADC #1
        SEC
        SBC GBASE
        BCC @none
        BEQ @none
        CMP #8
        BCC @part
        LDA #$0F                ; the whole cell
        BNE @put
@part:  ADC #$07                ; $08 to $0E: one to seven eighths
        BNE @put
@none:  LDA #' '
@put:   STA (SCR),Y
        DEX
        BPL @cell
        INC GROW
        LDA GBASE
        SEC
        SBC #8
        STA GBASE
        BCS @row
        RTS

; The instrument's wave, every frame: the table as the chip holds it now (the string, moving
; as its note dies), or the shape its waveform makes, a pulse as wide as its WIDTH.
iwave:  JSR ioff
        LDY IOFF
        LDA INSTR+I_WAVE,Y
        STA TMP
        LDA INSTR+I_WIDTH,Y
        STA TMP2
        LDX #31
@step:  LDA TMP
        AND #DRAWN
        BEQ @no1
        LDA WAVETABLE,X
        LSR
        LSR
        LSR
        JMP @have
@no1:   LDA TMP
        AND #NOISE
        BEQ @no2
        TXA                     ; nothing of noise repeats; this doesn't look as if it does
        ASL
        ASL
        ASL
        STA TMPX
        TXA
        ASL
        ADC TMPX
        STX TMPX
        ADC TMPX
        ADC #5
        AND #31
        JMP @have
@no2:   LDA TMP
        AND #PULSE
        BEQ @no3
        TXA
        ASL
        ASL
        ASL
        CMP TMP2
        LDA #0
        BCS @have
        LDA #31
        BNE @have
@no3:   LDA TMP
        AND #SAW
        BEQ @no4
        TXA
        BPL @have
@no4:   LDA TMP
        AND #TRI
        BEQ @have
        TXA
        CMP #16
        BCC @rise
        EOR #31
@rise:  ASL
@have:  STA VALS,X
        DEX
        BPL @step
        LDA #2
        JMP graph

; A / X, for small numbers.
divide: STX DHI
        LDX #$FF
        SEC
@sub:   INX
        SBC DHI
        BCS @sub
        TXA
        RTS

; The instrument's envelope: up in one to eight columns for the attack, down to the sustain
; level for the decay, eight columns held, and down to nothing for the release.
ienv:   LDY IOFF
        LDA INSTR+I_AD,Y
        STA TMP
        LDA INSTR+I_SR,Y
        STA TMP2
        LDY #0                  ; the column
        LDA TMP                 ; attack
        LSR
        LSR
        LSR
        LSR
        LSR
        TAX
        STX TMPX
        INC TMPX                ; its columns
        LDA #0
        STA CNT
@att:   LDA CNT
        CLC
        ADC ESTEP,X
        CMP #32
        BCC @a1
        LDA #31
@a1:    STA CNT
        STA VALS,Y
        INY
        DEC TMPX
        BNE @att
        LDA TMP2                ; the sustain level, 0 to 31
        LSR
        LSR
        LSR
        LSR
        BEQ @sus
        ASL
        ORA #1
@sus:   STA DLO
        LDA TMP                 ; decay
        AND #$0F
        LSR
        TAX
        INX
        STX TMPX
        LDA #31
        SEC
        SBC DLO
        JSR divide
        STA DHI                 ; down this much a column
        LDA #31
        STA CNT
@dec:   LDA CNT
        SEC
        SBC DHI
        STA CNT
        DEC TMPX
        BNE @d1
        LDA DLO                 ; and the last of them is the level itself
        STA CNT
@d1:    LDA CNT
        STA VALS,Y
        INY
        LDA TMPX
        BNE @dec
        LDX #8                  ; held
        LDA DLO
@hold:  STA VALS,Y
        INY
        DEX
        BNE @hold
        LDA TMP2                ; release
        AND #$0F
        LSR
        TAX
        INX
        STX TMPX
        LDA DLO
        JSR divide
        STA DHI
        LDA DLO
        STA CNT
@rel:   LDA CNT
        SEC
        SBC DHI
        STA CNT
        DEC TMPX
        BNE @r1
        LDA #0
        STA CNT
@r1:    LDA CNT
        STA VALS,Y
        INY
        LDA TMPX
        BNE @rel
        LDA #0
@rest:  CPY #32
        BCS @drawn
        STA VALS,Y
        INY
        BNE @rest
@drawn: LDA #7
        JMP graph

; After a key: the instrument's number, its parts down the right, its envelope, its table.
idraw:  JSR ioff
        LDA #0
        STA REV
        LDA ROWLO
        STA SCR
        LDA ROWHI
        STA SCR+1
        LDY #27
        LDA INS
        JSR putdig
        LDX #0
@par:   TXA
        CLC
        ADC #2
        TAY
        LDA ROWLO,Y
        STA SCR
        LDA ROWHI,Y
        STA SCR+1
        STX TMPX
        TXA
        ASL
        ASL
        TAX
        LDY #33
@label: LDA t_par,X
        STA (SCR),Y
        INX
        INY
        CPY #37
        BNE @label
        LDX TMPX
        TXA
        JSR irev
        JSR getpar
        LDY P_KIND,X
        BEQ @whole
        LDY #38
        JSR putdig
        JMP @next
@whole: LDY #37
        JSR puthex
@next:  INX
        CPX #NPARAM
        BNE @par
        JSR ienv
        LDY IOFF                ; the table: eight rows from where the instrument's starts
        LDA INSTR+I_TABLE,Y
        STA TMP
        LDX #0
@row:   LDA ROWLO+13,X
        STA SCR
        LDA ROWHI+13,X
        STA SCR+1
        LDA #0
        STA REV
        TXA
        ASL
        CLC
        ADC TMP
        STA TMP2                ; this row's place
        LDY #1
        JSR puthex
        TXA
        ASL
        CLC
        ADC #NPARAM
        JSR irev
        LDY TMP2
        LDA TABLES,Y
        LDY #5
        JSR puthex
        TXA
        ASL
        CLC
        ADC #NPARAM + 1
        JSR irev
        LDY TMP2
        LDA TABLES+1,Y
        STA CNT
        LDY #8
        JSR puthex
        LDA #0                  ; the pitch as a bar, a cell a semitone: not for a note of its own, or an end
        STA REV
        LDY TMP2
        LDA TABLES,Y
        CMP #$FF
        BEQ @flat
        LDA CNT
        BMI @flat
        CMP #21
        BCC @long
        LDA #20
        BNE @long
@flat:  LDA #0
@long:  STA CNT
        LDY #11
@bar:   TYA
        SEC
        SBC #11
        CMP CNT
        LDA #' '
        BCS @put
        LDA #$0F
@put:   STA (SCR),Y
        INY
        CPY #31
        BNE @bar
        INX
        CPX #8
        BNE @row
        RTS

; The keys there.
ikeys:  JSR ioff
        LDA KEY
        CMP #'I'
        BNE @i1
        LDA #0
        STA MODE
        JSR CLS
        JSR panel
        JMP title
@i1:    CMP #'O'
        BNE @i2
        LDA INS
        CMP #2
        BCC @r
        DEC INS
@r:     RTS
@i2:    CMP #'P'
        BNE @i3
        LDA INS
        CMP #NINS
        BCS @r
        INC INS
        RTS
@i3:    CMP #30                 ; up: a part, or a row of the table; from its top, to TAB
        BNE @i4
        LDA ECUR
        BEQ @moved
        CMP #NPARAM
        BCC @up1
        CMP #NPARAM + 2
        BCC @totab
        SBC #2
        STA ECUR
        BNE @moved
@totab: LDA #9
        STA ECUR
        BNE @moved
@up1:   DEC ECUR
@moved: LDA #0
        STA ENIB
        RTS
@i4:    CMP #31                 ; down
        BNE @i5
        LDA ECUR
        CMP #NPARAM
        BCS @dn
        INC ECUR
        BNE @moved
@dn:    CMP #NPARAM + 14
        BCS @moved
        ADC #2
        STA ECUR
        BNE @moved
@i5:    CMP #28                 ; left: the part smaller; in the table, to the waveform
        BNE @i6
        LDX ECUR
        CPX #NPARAM
        BCC @less
        TXA
        LSR
        BCC @moved
        DEC ECUR
        BNE @moved
@less:  JSR getpar
        CPX #0
        BNE @l1
        LSR                     ; the waveform: the one before, and round
        BNE @set
        LDA #DRAWN
        BNE @set
@l1:    SEC
        SBC P_STEP,X
        BCS @set
        LDY P_KIND,X
        BEQ @set
        LDA #0
@set:   JMP setpar
@i6:    CMP #29                 ; right: the part larger; in the table, to the pitch
        BNE @i7
        LDX ECUR
        CPX #NPARAM
        BCC @more
        TXA
        LSR
        BCS @moved
        INC ECUR
        BNE @moved
@more:  JSR getpar
        CPX #0
        BNE @m1
        ASL
        CMP #DRAWN + 1
        BCC @set
        LDA #TRI
        BNE @set
@m1:    CLC
        ADC P_STEP,X
        LDY P_KIND,X
        BEQ @set
        CMP #16
        BCC @set
        LDA #15
        BNE @set
@i7:    CMP #' '                ; hear it: the octave's C, on the cursor's voice
        BNE @i8
        LDX CURV
        LDA INS
        JSR setins
        LDA OCT
        ASL
        ASL
        STA TMP2
        ASL
        CLC
        ADC TMP2
        ADC #1
        JSR noteon
        LDA #PREVIEW * 2
        STA VPREV,X
        RTS
@i8:    CMP #','
        BNE @i9
        LDA OCT
        BEQ @r2
        DEC OCT
@r2:    RTS
@i9:    CMP #'.'
        BNE @i10
        LDA OCT
        CMP #6
        BEQ @r2
        INC OCT
        RTS
@i10:   CMP #'0'                ; a hex digit: typed over what the cursor is on
        BCC @r2
        CMP #'9' + 1
        BCS @alpha
        SEC
        SBC #'0'
        BCS @digit
@alpha: CMP #'A'
        BCC @r2
        CMP #'F' + 1
        BCS @r2
        SEC
        SBC #'A' - 10
@digit: STA DLO
        LDX ECUR
        CPX #NPARAM
        BCS @tab
        LDY P_KIND,X
        BNE @one                ; one digit is all of it
        JSR getpar
        STA DHI
        LDA ENIB
        BNE @plo
        LDA DLO
        ASL
        ASL
        ASL
        ASL
        STA DLO
        LDA DHI
        AND #$0F
        ORA DLO
        INC ENIB
        JMP setpar
@plo:   LDA #0
        STA ENIB
        LDA DHI
        AND #$F0
        ORA DLO
        JMP setpar
@one:   LDA DLO
        JMP setpar
@tab:   LDY IOFF
        LDA INSTR+I_TABLE,Y
        CLC
        ADC ECUR
        SEC
        SBC #NPARAM
        TAY
        LDA ENIB
        BNE @lo
        LDA DLO
        ASL
        ASL
        ASL
        ASL
        STA DLO
        LDA TABLES,Y
        AND #$0F
        ORA DLO
        STA TABLES,Y
        INC ENIB
        RTS
@lo:    LDA TABLES,Y
        AND #$F0
        ORA DLO
        STA TABLES,Y
        LDA #0
        STA ENIB
        LDA ECUR
        CMP #NPARAM + 15
        BCS @r3
        INC ECUR
@r3:    RTS

; ---------------------------------------------------------------- the tape

; A name typed on row 2: into NAMEBUF, sixteen characters padded with spaces (the tape's
; header), and NAMEZ, ending in 0 (LOAD's). Carry set: ESC, never mind.
askname:
        LDA ROWLO+2
        STA SCR
        LDA ROWHI+2
        STA SCR+1
        LDY #39
        LDA #' '
@clear: STA (SCR),Y
        DEY
        BPL @clear
        LDY #1
        LDX #0
@ask:   LDA t_ask,X
        BEQ @asked
        STA (SCR),Y
        INY
        INX
        BNE @ask
@asked: LDX #0
@key:   LDA KBD_STROBE
        BPL @key
        AND #$7F
        CMP #13
        BEQ @done
        CMP #27
        BEQ @never
        CMP #8
        BNE @char
        CPX #0
        BEQ @key
        DEX
        DEY
        LDA #' '
        STA (SCR),Y
        BNE @key
@char:  CMP #' '
        BCC @key
        CPX #16
        BCS @key
        STA NAMEBUF,X
        STA NAMEZ,X
        STA (SCR),Y
        INX
        INY
        BNE @key
@done:  LDA #0
        STA NAMEZ,X
        LDA #' '
@pad:   CPX #16
        BCS @ok
        STA NAMEBUF,X
        INX
        BNE @pad
@ok:    CLC
        RTS
@never: SEC
        RTS

; ---------------------------------------------------------------- a tune made new

BITS8:  .byte 1, 2, 4, 8, 16, 32, 64, 128
USED:   .fill 13                ; a bit a pattern: some tune's order has it

; USED, from every tune's order but tune SKIP's.
scan:   LDX #12
        LDA #0
@zero:  STA USED,X
        DEX
        BPL @zero
        LDX #0                  ; the tune, times four
@tune:  TXA
        LSR
        LSR
        CMP SKIP
        BEQ @next
        LDA SONGTAB,X
        STA PTR
        LDA SONGTAB+1,X
        STA PTR+1
        LDY #0
@pos:   LDA (PTR),Y
        CMP #$FF
        BEQ @next
        STX TMPX
        PHA
        AND #7
        TAX
        LDA BITS8,X
        STA TMP
        PLA
        LSR
        LSR
        LSR
        TAX
        LDA USED,X
        ORA TMP
        STA USED,X
        LDX TMPX
        INY
        BNE @pos
@next:  INX
        INX
        INX
        INX
        CPX #SONGS * 4
        BNE @tune
        RTS

; A = a pattern no tune uses, emptied, and counted as used from now. Carry set: there is none.
freepat:
        LDX #0
@look:  STX TMPX
        TXA
        AND #7
        TAY
        TXA
        LSR
        LSR
        LSR
        TAX
        LDA USED,X
        AND BITS8,Y
        BEQ @free
        LDX TMPX
        INX
        CPX #NPATS
        BNE @look
        SEC
        RTS
@free:  LDA USED,X
        ORA BITS8,Y
        STA USED,X
        LDA TMPX
        STA TPAT
        LDA #0
        JSR cellp
        LDY #63
        LDA #0
@wipe:  STA (PTR),Y
        DEY
        BPL @wipe
        LDA TPAT
        CLC
        RTS

; -: a new pattern for the cursor's voice at this position.
newpat: LDA #$FF
        STA SKIP
        JSR scan
        JSR freepat
        BCS @r
        PHA
        JSR posy
        TYA
        CLC
        ADC CURV
        TAY
        PLA
        STA (ORDP),Y
        JMP loadpos
@r:     RTS

; A question on row 2: the string at X, Y.
prompt: STX TMPX
        STY TMPX2
        LDA ROWLO+2
        STA SCR
        LDA ROWHI+2
        STA SCR+1
        LDY #39
        LDA #' '
@clear: STA (SCR),Y
        DEY
        BPL @clear
        LDA #2
        STA CURY
        LDA #1
        STA CURX
        LDX TMPX
        LDY TMPX2
        JMP PRSTR

; PTR = where this tune's name is kept.
nameptr:
        LDX SONG
        LDA NAMELO,X
        STA PTR
        LDA NAMEHI,X
        STA PTR+1
        RTS

; The tune's name becomes the one typed (NAMEZ).
setname:
        JSR nameptr
        LDY #0
@copy:  LDA NAMEZ,Y
        STA (PTR),Y
        BEQ @r
        INY
        BNE @copy
@r:     RTS

; ;: the tune named.
rename: JSR askname
        BCS @back
        LDA NAMEZ
        BEQ @back
        JSR setname
@back:  JMP back

; 0: the tune emptied, if Y is the answer: one position of three patterns nothing uses,
; six frames a row, and the name TUNE #1 to #4. What it played is still in its patterns,
; which nothing plays now: - will hand them out again.
empty:  LDX #<t_empty
        LDY #>t_empty
        JSR prompt
@key:   LDA KBD_STROBE
        BPL @key
        AND #$7F
        CMP #'Y'
        BNE @no
        LDA SONG
        STA SKIP
        JSR scan
        LDA #0
        STA CNT
@three: JSR freepat
        BCS @fail
        PHA
        INC CNT
        LDA CNT
        CMP #3
        BNE @three
        LDY #2
@put:   PLA
        STA (ORDP),Y
        DEY
        BPL @put
        LDY #3
        LDA #$FF
        STA (ORDP),Y
        INY
        LDA #0
        STA (ORDP),Y
        STA POS
        STA ROW
        LDA #6
        STA SPEED
        JSR speed
        JSR nameptr
        LDY #0
@name:  LDA t_tune,Y
        BEQ @num
        STA (PTR),Y
        INY
        BNE @name
@num:   LDA SONG
        CLC
        ADC #'1'
        STA (PTR),Y
        INY
        LDA #0
        STA (PTR),Y
        JSR loadpos
@no:    JMP back
@fail:  LDA CNT                 ; no three to be had: nothing changes
        BEQ @no
        PLA
        DEC CNT
        JMP @fail

t_empty: .asciiz "EMPTY THIS TUNE? Y DOES IT"
t_tune: .asciiz "TUNE #"

; The tune's screen again, after a question.
back:   JSR CLS
        JSR panel
        JSR title
        LDA #1
        STA DIRTY
        RTS

; /: the whole tracker, $0800 to the top of memory, on the tape as a program of machine
; language that starts at `start` (chapter 14's format). What it is loaded as is this
; tracker with these tunes and these instruments. The name typed is the program's name on
; the tape, and the name of the tune that is showing.
save:   JSR quiet
        JSR askname
        BCS back
        LDA NAMEZ               ; a name typed is this tune's name from now on, and on the tape
        BEQ @named
        JSR setname
        JSR title
@named: LDA TAPE
        AND #$08
        BNE @rec
        LDX #<t_rec
        LDY #>t_rec
        JSR prompt
@wait:  LDA KBD_STROBE          ; any key: never mind
        BMI back
        LDA TAPE
        AND #$08
        BEQ @wait
@rec:   LDA BORDER
        PHA
        LDA #4
        STA TAPE                ; the motor
        LDX #0
@mark:  LDA t_unct,X
        JSR tbyte
        INX
        CPX #4
        BNE @mark
        LDA #0
        STA SUM
        LDX #0
@head:  LDA t_head,X            ; version 1, type 1
        JSR tbyte
        INX
        CPX #2
        BNE @head
        LDX #0
@name:  LDA NAMEBUF,X
        JSR tbyte
        INX
        CPX #16
        BNE @name
        LDX #0
@where: LDA t_where,X           ; load address, length, entry
        JSR tbyte
        INX
        CPX #6
        BNE @where
        LDA #<start
        STA PTR
        LDA #>start
        STA PTR+1
        LDY #0
@prog:  LDA (PTR),Y
        JSR tbyte
        INY
        BNE @prog
        INC PTR+1
        LDA PTR+1
        CMP #>MEMTOP
        BNE @prog
        LDA SUM                 ; everything since the mark EORs with this to 0
        JSR tbyte
@last:  LDA TAPE                ; and the last byte is on the tape before the motor stops
        AND #$10
        BEQ @last
        LDA #0
        STA TAPE
        PLA
        STA BORDER
        JMP back

; A to the tape, when the deck has room for it, and into the checksum. Keeps X and Y.
tbyte:  PHA
        EOR SUM
        STA SUM
@room:  LDA TAPE
        AND #$10
        BEQ @room
        PLA
        STA TAPEDATA
        STA BORDER
        RTS

; \: the ROM's LOAD, with a name or none (the first program on the tape). It doesn't
; come back: what it loads starts, and if that's a tracker saved from here, here it is.
load:   JSR quiet
        JSR askname
        BCC @go
        JMP back
@go:    LDA #0
        STA VMODE
        JSR CLS
        LDX #<NAMEZ
        LDY #>NAMEZ
        JMP LOAD

t_unct: .text "UNCT"
t_head: .byte 1, 1
t_where: .word start, MEMTOP - start, start
t_ask:  .asciiz "NAME, THEN RETURN: "
t_rec:  .asciiz "PRESS RECORD AND PLAY ON TAPE"
NAMEBUF: .fill 16
NAMEZ:  .fill 17

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
        STA CUTDIR              ; the filter starts over with the tune, so a fast sweep keeps time with it
        LDA #CUTMIN
        STA CUT
        RTS

; T: the next tune on the tape, stopped at its top.
nexttune:
        LDA #0
        STA PLAYING
        JSR quiet
        INC SONG
        LDA SONG
        CMP #SONGS
        BCC @in
        LDA #0
        STA SONG
@in:    JSR setsong

; The tune's name, at the top right, over the last one.
title:  LDX #23
        LDA #' '
@clear: STA SCREEN+15,X
        DEX
        BPL @clear
        LDA #0
        STA CURY
        LDA #16
        STA CURX
        LDX SONG
        LDY NAMEHI,X
        LDA NAMELO,X
        TAX
        JMP PRSTR

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

draw:   LDA MODE
        BEQ @tune
        CMP #2
        BEQ @r
        LDA DIRTY               ; the instruments' screen: after a key, everything but the wave;
        BNE @all                ; the wave in every other frame, so the string's is seen moving
        JMP iwave
@all:   LDA #0
        STA DIRTY
        JMP idraw
@tune:  JSR meters
        LDA DIRTY
        BEQ @r
        LDA #0
        STA DIRTY
        JSR status
        JSR pattern
@r:     RTS

; What doesn't change: the names and the help.
panel:  LDX #0
panelx: LDA TEXTS,X             ; (or from X: the instruments' screen's texts follow the tune's)
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
        BNE panelx
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

; A as three digits at (SCR),Y, a space for a leading nought.
putdec: LDX #'0'
@h:     CMP #100
        BCC @hd
        SBC #100
        INX
        BNE @h
@hd:    PHA
        TXA
        CMP #'0'
        BNE @h1
        LDA #' '
@h1:    STA (SCR),Y
        INY
        PLA
        LDX #'0'
@t:     CMP #10
        BCC @td
        SBC #10
        INX
        BNE @t
@td:    PHA
        TXA
        STA (SCR),Y
        INY
        PLA
        ORA #'0'
        STA (SCR),Y
        RTS

; Beats a minute at 3 to 18 frames a row, four rows to a beat: 750 / frames. A row is a
; whole number of frames, so these are the tempos there are.
BPMS:   .byte 250, 187, 150, 125, 107, 93, 83, 75, 68, 62, 57, 53, 50, 46, 44, 41

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
        LDX SPEED               ; the speed as beats a minute: four rows to a beat
        LDA BPMS-3,X
        LDY #21
        JSR putdec
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
HEX:    .text "0123456789ABCDEFGHIJKLMNOPQRSTUV"   ; an instrument is one character: 1 to V
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
        .byte 1, 1
        .word t_top
        .byte 2, 9
        .word t_filt
        .byte 3, 6
        .word t_pats
        .byte 24, 0
        .word t_keys
        .byte $FF
ITEXTS: .byte 0, 1
        .word t_ins
        .byte 1, 1
        .word t_wav
        .byte 6, 1
        .word t_env
        .byte 12, 1
        .word t_tab
        .byte 24, 0
        .word t_ikeys
        .byte $FF
        .assert * - TEXTS <= 256, "the screens' texts are more than a page of them"
t_ins:  .asciiz "MUSIC TRACKER  INSTRUMENT"
t_wav:  .asciiz "ITS WAVE"
t_env:  .asciiz "ITS LEVEL"
t_tab:  .asciiz "AT  WAV PIT"
t_keys: .asciiz "SPACE PLAY T TUNE I INS Z-M PIANO ?HELP"   ; 39: a 40th would scroll the screen
t_ikeys: .asciiz "SPACE HEAR  O P NEXT  I BACK  ? HELP"
; The HELP screen, laid out as the ROM's is (rom/system.s, MHELP): a bar across the top and the
; headings in reverse video (bit 7; the gaps are $80, a blank reversed, not $A0, which blinks),
; and under each heading a key and what it does. 9 moves to column 17. A full row wraps by itself.
MHELP:
        ; [ MUSIC TRACKER         ANY KEY GOES BACK]
        .byte $80, $CD, $D5, $D3, $C9, $C3, $80, $D4, $D2, $C1, $C3, $CB, $C5, $D2, $80, $80
        .byte $80, $80, $80, $80, $80, $80, $80, $C1, $CE, $D9, $80, $CB, $C5, $D9, $80, $C7
        .byte $CF, $C5, $D3, $80, $C2, $C1, $C3, $CB
        ; [PLAYING]
        .byte $D0, $CC, $C1, $D9, $C9, $CE, $C7, $0D
        ;  RETURN          FROM THE TOP
        .byte $20, $52, $45, $54, $55, $52, $4E, $09, $46, $52, $4F, $4D, $20, $54, $48, $45
        .byte $20, $54, $4F, $50, $0D
        ;  SPACE           PLAY, STOP
        .byte $20, $53, $50, $41, $43, $45, $09, $50, $4C, $41, $59, $2C, $20, $53, $54, $4F
        .byte $50, $0D
        ;  T               THE NEXT TUNE
        .byte $20, $54, $09, $54, $48, $45, $20, $4E, $45, $58, $54, $20, $54, $55, $4E, $45
        .byte $0D
        ;  Q W E           SILENCE A VOICE
        .byte $20, $51, $20, $57, $20, $45, $09, $53, $49, $4C, $45, $4E, $43, $45, $20, $41
        .byte $20, $56, $4F, $49, $43, $45, $0D
        ;  F               THE FILTER OUT, IN
        .byte $20, $46, $09, $54, $48, $45, $20, $46, $49, $4C, $54, $45, $52, $20, $4F, $55
        .byte $54, $2C, $20, $49, $4E, $0D
        ; [WRITING, WHEN IT'S STOPPED]
        .byte $D7, $D2, $C9, $D4, $C9, $CE, $C7, $AC, $80, $D7, $C8, $C5, $CE, $80, $C9, $D4
        .byte $A7, $D3, $80, $D3, $D4, $CF, $D0, $D0, $C5, $C4, $0D
        ;  ARROWS  [ ]     THE ROW, THE POSITION
        .byte $20, $41, $52, $52, $4F, $57, $53, $20, $20, $5B, $20, $5D, $09, $54, $48, $45
        .byte $20, $52, $4F, $57, $2C, $20, $54, $48, $45, $20, $50, $4F, $53, $49, $54, $49
        .byte $4F, $4E, $0D
        ;  Z X C V B N M   BOTTOM ROW: A PIANO
        .byte $20, $5A, $20, $58, $20, $43, $20, $56, $20, $42, $20, $4E, $20, $4D, $09, $42
        .byte $4F, $54, $54, $4F, $4D, $20, $52, $4F, $57, $3A, $20, $41, $20, $50, $49, $41
        .byte $4E, $4F, $0D
        ;   S D   G H J    ITS BLACK KEYS
        .byte $20, $20, $53, $20, $44, $20, $20, $20, $47, $20, $48, $20, $4A, $09, $49, $54
        .byte $53, $20, $42, $4C, $41, $43, $4B, $20, $4B, $45, $59, $53, $0D
        ;  , .             OCTAVE DOWN, UP
        .byte $20, $2C, $20, $2E, $09, $4F, $43, $54, $41, $56, $45, $20, $44, $4F, $57, $4E
        .byte $2C, $20, $55, $50, $0D
        ;  =  DEL          NOTE OFF, EMPTY
        .byte $20, $3D, $20, $20, $44, $45, $4C, $09, $4E, $4F, $54, $45, $20, $4F, $46, $46
        .byte $2C, $20, $45, $4D, $50, $54, $59, $0D
        ;  1-9  O P        THE INSTRUMENT
        .byte $20, $31, $2D, $39, $20, $20, $4F, $20, $50, $09, $54, $48, $45, $20, $49, $4E
        .byte $53, $54, $52, $55, $4D, $45, $4E, $54, $0D
        ; [THE TUNE]
        .byte $D4, $C8, $C5, $80, $D4, $D5, $CE, $C5, $0D
        ;  K L  -          ITS PATTERN HERE, NEW
        .byte $20, $4B, $20, $4C, $20, $20, $2D, $09, $49, $54, $53, $20, $50, $41, $54, $54
        .byte $45, $52, $4E, $20, $48, $45, $52, $45, $2C, $20, $4E, $45, $57, $0D
        ;  A  R            NEW POSITION, REMOVE
        .byte $20, $41, $20, $20, $52, $09, $4E, $45, $57, $20, $50, $4F, $53, $49, $54, $49
        .byte $4F, $4E, $2C, $20, $52, $45, $4D, $4F, $56, $45, $0D
        ;  Y U             SLOWER, FASTER (BPM)
        .byte $20, $59, $20, $55, $09, $53, $4C, $4F, $57, $45, $52, $2C, $20, $46, $41, $53
        .byte $54, $45, $52, $20, $28, $42, $50, $4D, $29, $0D
        ;  0  ;            EMPTY IT, NAME IT
        .byte $20, $30, $20, $20, $3B, $09, $45, $4D, $50, $54, $59, $20, $49, $54, $2C, $20
        .byte $4E, $41, $4D, $45, $20, $49, $54, $0D
        ;  /  \            SAVE, LOAD ON TAPE
        .byte $20, $2F, $20, $20, $5C, $09, $53, $41, $56, $45, $2C, $20, $4C, $4F, $41, $44
        .byte $20, $4F, $4E, $20, $54, $41, $50, $45, $0D
        ; [INSTRUMENTS: I, AND I BACK]
        .byte $C9, $CE, $D3, $D4, $D2, $D5, $CD, $C5, $CE, $D4, $D3, $BA, $80, $C9, $AC, $80
        .byte $C1, $CE, $C4, $80, $C9, $80, $C2, $C1, $C3, $CB, $0D
        ;  UP DOWN         CHOOSE A PART
        .byte $20, $55, $50, $20, $44, $4F, $57, $4E, $09, $43, $48, $4F, $4F, $53, $45, $20
        .byte $41, $20, $50, $41, $52, $54, $0D
        ;  LEFT RIGHT 0-F  CHANGE IT
        .byte $20, $4C, $45, $46, $54, $20, $52, $49, $47, $48, $54, $20, $30, $2D, $46, $09
        .byte $43, $48, $41, $4E, $47, $45, $20, $49, $54, $0D
        ;  SPACE  O P      HEAR IT, THE NEXT ONE
        .byte $20, $53, $50, $41, $43, $45, $20, $20, $4F, $20, $50, $09, $48, $45, $41, $52
        .byte $20, $49, $54, $2C, $20, $54, $48, $45, $20, $4E, $45, $58, $54, $20, $4F, $4E
        .byte $45, $0D
        ;  FLG 1 FILTER 2 LETGO 4 SLIDE 8 DRAWS
        .byte $20, $46, $4C, $47, $20, $31, $20, $46, $49, $4C, $54, $45, $52, $20, $32, $20
        .byte $4C, $45, $54, $47, $4F, $20, $34, $20, $53, $4C, $49, $44, $45, $20, $38, $20
        .byte $44, $52, $41, $57, $53
        .byte 0
t_name: .asciiz "MUSIC TRACKER"
t_top:  .asciiz "POS     ROW     BPM     OCT    INS"
t_filt: .asciiz "FILTER"
t_pats: .asciiz "PAT        PAT        PAT"
nochip: .asciiz "THE TRACKER NEEDS THE SOUND CHIP."
nochip2: .asciiz "SWITCH OFF, FIT THE AUNT, LOAD AGAIN."

; An instrument is eight bytes; number 0 isn't one. The keys 1 to 9 choose the
; first nine, O and P step through them all.
I_WAVE  = 0             ; the waveform, unless the table gives another
I_AD    = 1             ; attack and decay
I_SR    = 2             ; sustain and release
I_WIDTH = 3             ; the pulse width a note starts with
I_PWM   = 4             ; low four bits: how far the width moves a frame (0: it stays).
                        ; High four: vibrato, a semitone shifted right this many times (0: none)
I_GATE  = 5             ; frames until the note is let go; 0: held until the next, or a note off
I_TABLE = 6             ; its table's place in TABLES
I_FLAGS = 7             ; bits 4-7: TRIM, and:
F_FILTER = 1            ;   through the filter
F_LETGO = 2             ;   let go two frames before the next note on its voice
F_SLIDES = 4            ;   slides to its note from the last, if the voice is still sounding
F_DRAWS = 8             ;   its note starts the wave table being redrawn

; >>> song
; (written by scripts/tracker-song.py: change the instruments and the tunes there)

NINS    = 30

;       wave  AD   SR   width vib|pwm gate table flags
INSTR:  .fill 8
        .byte $10, $00, $F2, $80, $00,   0,   0, $48    ; 1 the plucked string, drawn in the wave table and driven
        .byte $10, $00, $F2, $80, $00,   0,   0, $90    ; 2 a second voice on the string's table: strike it with the string
        .byte $10, $00, $F2, $80, $30,   0,   0, $48    ; 3 the string with a late vibrato, for slow tunes
        .byte $04, $38, $7A, $80, $3C,   0,   0, $02    ; 4 a pulse lead whose width runs
        .byte $04, $08, $58, $80, $00,   0,   4, $12    ; 5 a note and its octave, a frame each
        .byte $01, $0F, $F8, $80, $30,   0,   0, $02    ; 6 a held triangle
        .byte $04, $09, $98, $30, $02,   0,  10, $12    ; 7 a thin pulse bass with a frame of noise for the pick
        .byte $08, $0A, $09, $80, $00,   2,  18, $10    ; 8 a snare of noise and pulse
        .byte $08, $02, $08, $80, $00,   2,   0, $70    ; 9 a hat: play it high
        .byte $04, $06, $4B, $30, $00,   2,  32, $20    ; A a tom of pulse and noise: play it where the chord is
        .byte $01, $00, $F1, $80, $00,   9,  48, $00    ; B a kick that lands on its note: a triangle falling a twelfth
        .byte $01, $00, $F1, $80, $00,   0,   0, $02    ; C a held triangle bass
        .byte $08, $05, $05, $80, $00,   9,   0, $00    ; D a snare of noise alone
        .byte $01, $00, $F1, $80, $00,   6,  58, $00    ; E a tom: a triangle falling
        .byte $04, $06, $06, $40, $00,   7,   0, $80    ; F a plucked pulse, for arpeggios
        .byte $04, $06, $06, $20, $00,   7,   0, $80    ; G the same, thinner
        .byte $04, $18, $C8, $40, $39,   0,   0, $04    ; H a lead that slides from note to note
        .byte $04, $70, $A9, $30, $02,   0,  72, $63    ; I a minor seventh chord, held, through the filter
        .byte $04, $70, $A9, $30, $02,   0,  90, $63    ; J a major seventh chord
        .byte $04, $70, $A9, $30, $02,   0, 108, $63    ; K a seventh chord
        .byte $04, $70, $A9, $30, $02,   0, 126, $63    ; L a major chord
        .byte $04, $08, $98, $46, $00,   0, 140, $12    ; M a bass that clicks, blips an octave up, and settles
        .byte $04, $08, $89, $80, $00,   0, 150, $02    ; N a kick of pulse that lands on its note
        .byte $01, $7B, $9A, $80, $40,   0,   0, $02    ; O a whistle: a triangle that comes in slowly
        .byte $04, $08, $09, $5C, $00,  16,   0, $20    ; P a soft pluck for broken chords
        .byte $04, $08, $98, $80, $00,   0, 164, $32    ; Q a kick that settles into a soft square bass
        .byte $04, $4A, $98, $80, $00,   0,   0, $32    ; R the soft square bass alone
        .byte $08, $07, $06, $80, $00,   4, 176, $20    ; S a small snare: a stick, a skin, a rattle
        .byte $08, $01, $01, $80, $00,   2,   0, $80    ; T a quiet hat
        .byte $08, $06, $05, $80, $00,   8,   0, $80    ; U an open hat

; The tables: a waveform (0: the instrument's) and a pitch for each frame of a note. A pitch
; is semitones above the note, or with bit 7 a note of its own. $FF ends a table, with where
; to go on from.
TABLES: .byte $00, $00, $FF, $00, $00, $00, $00, $0C, $FF, $04, $00, $00, $08, $24, $00, $00
        .byte $FF, $0E, $08, $0C, $08, $00, $04, $0C, $08, $00, $08, $0C, $08, $00, $FF, $1A
        .byte $04, $00, $08, $0C, $08, $00, $04, $0C, $04, $00, $04, $0C, $04, $00, $FF, $2A
        .byte $01, $13, $01, $0C, $01, $05, $01, $00, $FF, $36, $01, $0B, $01, $08, $01, $06
        .byte $01, $04, $01, $02, $01, $00, $FF, $44, $00, $00, $00, $00, $00, $03, $00, $03
        .byte $00, $07, $00, $07, $00, $0A, $00, $0A, $FF, $48, $00, $00, $00, $00, $00, $04
        .byte $00, $04, $00, $07, $00, $07, $00, $0B, $00, $0B, $FF, $5A, $00, $00, $00, $00
        .byte $00, $04, $00, $04, $00, $07, $00, $07, $00, $0A, $00, $0A, $FF, $6C, $00, $00
        .byte $00, $00, $00, $04, $00, $04, $00, $07, $00, $07, $FF, $7E, $00, $00, $08, $24
        .byte $00, $0C, $00, $00, $FF, $92, $01, $18, $08, $28, $04, $0C, $04, $07, $04, $03
        .byte $04, $00, $FF, $A0, $01, $18, $08, $28, $01, $0C, $01, $05, $00, $00, $FF, $AC
        .byte $08, $DB, $01, $AF, $01, $AB, $08, $D7, $08, $D4, $FF, $B8

; The string. SCHED: for each frame of a note's age, which drawing of the table to copy to
; the chip, or $FF for none this frame. WAVES: the drawings, 32 steps each.
SCHEDLEN = 105
SCHED:  .byte $00, $01, $02, $03, $04, $05, $06, $07, $08, $09, $0A, $0B, $0C, $FF, $0D, $FF
        .byte $0E, $FF, $0F, $FF, $10, $FF, $11, $FF, $12, $FF, $FF, $FF, $13, $FF, $FF, $FF
        .byte $14, $FF, $FF, $FF, $15, $FF, $FF, $FF, $16, $FF, $FF, $FF, $17, $FF, $FF, $FF
        .byte $18, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $19, $FF, $FF, $FF, $FF, $FF, $FF, $FF
        .byte $1A, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $1B, $FF, $FF, $FF, $FF, $FF, $FF, $FF
        .byte $1C, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $1D, $FF, $FF, $FF, $FF, $FF, $FF, $FF
        .byte $1E, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $1F
WAVELO: .rept 32, n
        .byte <[WAVES + n * 32]
        .endr
WAVEHI: .rept 32, n
        .byte >[WAVES + n * 32]
        .endr
WAVES:  .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $BB, $D6    ; frame 0
        .byte $80, $2A, $45, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $C1, $D1    ; frame 1
        .byte $80, $2F, $3F, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $C5, $CC    ; frame 2
        .byte $80, $34, $3B, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $C9, $C9    ; frame 3
        .byte $80, $37, $37, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $CD, $C5    ; frame 4
        .byte $80, $3B, $33, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $CF, $C2    ; frame 5
        .byte $80, $3E, $31, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D2, $C0    ; frame 6
        .byte $80, $40, $2E, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D3, $BD    ; frame 7
        .byte $80, $43, $2D, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D5, $BB    ; frame 8
        .byte $80, $45, $2B, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D6, $B9    ; frame 9
        .byte $80, $47, $2A, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D6, $B8    ; frame 10
        .byte $80, $48, $2A, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D7, $B6    ; frame 11
        .byte $80, $4A, $29, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D7, $B5    ; frame 12
        .byte $80, $4B, $29, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D7, $B2    ; frame 14
        .byte $80, $4E, $29, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D7, $B0    ; frame 16
        .byte $80, $50, $29, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D6, $AF    ; frame 18
        .byte $80, $51, $2A, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D5, $AD    ; frame 20
        .byte $80, $53, $2B, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D4, $AC    ; frame 22
        .byte $80, $54, $2C, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $D2, $AA    ; frame 24
        .byte $80, $56, $2E, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FB, $CF, $A8    ; frame 28
        .byte $80, $58, $31, $05, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $F4, $CB, $A6    ; frame 32
        .byte $80, $5A, $35, $0C, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $EE, $C8, $A4    ; frame 36
        .byte $80, $5C, $38, $12, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $E9, $C4, $A2    ; frame 40
        .byte $80, $5E, $3C, $17, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
        .byte $80, $FD, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $E3, $C1, $A0    ; frame 44
        .byte $80, $60, $3F, $1D, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $03
        .byte $80, $F2, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $DE, $BE, $9F    ; frame 48
        .byte $80, $61, $42, $22, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $0E
        .byte $80, $DF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $F2, $D4, $B7, $9B    ; frame 56
        .byte $80, $65, $49, $2C, $0E, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $21
        .byte $80, $CE, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $E5, $CB, $B1, $98    ; frame 64
        .byte $80, $68, $4F, $35, $1B, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $32
        .byte $80, $C0, $FC, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $F1, $D9, $C2, $AC, $96    ; frame 72
        .byte $80, $6A, $54, $3E, $27, $0F, $00, $00, $00, $00, $00, $00, $00, $00, $04, $40
        .byte $80, $B5, $E6, $FF, $FF, $FF, $FF, $FF, $FF, $FF, $F8, $E3, $CF, $BB, $A7, $93    ; frame 80
        .byte $80, $6D, $59, $45, $31, $1D, $08, $00, $00, $00, $00, $00, $00, $00, $1A, $4B
        .byte $80, $AB, $D3, $F5, $FF, $FF, $FF, $FF, $FF, $FA, $E8, $D7, $C5, $B4, $A2, $91    ; frame 88
        .byte $80, $6F, $5E, $4C, $3B, $29, $18, $06, $00, $00, $00, $00, $00, $0B, $2D, $55
        .byte $80, $A4, $C4, $E0, $F4, $FF, $FF, $FE, $F5, $E8, $DA, $CC, $BC, $AD, $9E, $8F    ; frame 96
        .byte $80, $71, $62, $53, $44, $34, $26, $18, $0B, $02, $00, $00, $0C, $20, $3C, $5C
        .byte $80, $9D, $B8, $CF, $E0, $EA, $ED, $EA, $E3, $DA, $CE, $C2, $B5, $A8, $9A, $8D    ; frame 104
        .byte $80, $73, $66, $58, $4B, $3E, $32, $26, $1D, $16, $13, $16, $20, $31, $48, $63

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

SONGS   = 4
SONGPATS = 85

; Each tune: its order's address, its speed in frames a row, and how far the filter's corner moves a frame.
SONGTAB:.word ORDER
        .byte 6, 1             ; ROB'S YOUR UNCLE
        .word order1
        .byte 9, 1             ; SWITCHED ON
        .word order2
        .byte 8, 1             ; THE LONG WAY ROUND
        .word order3
        .byte 6, 1             ; THE GLIDE
NAMELO: .byte <name0, <name1, <name2, <name3
NAMEHI: .byte >name0, >name1, >name2, >name3
name0:  .asciiz "ROB'S YOUR UNCLE"
        .fill 5
name1:  .asciiz "SWITCHED ON"
        .fill 10
name2:  .asciiz "THE LONG WAY ROUND"
        .fill 3
name3:  .asciiz "THE GLIDE"
        .fill 12

; The orders: a pattern for each voice at each position. $FF, then where to go on from.
; A tune has 96 bytes of it, whatever it uses. ORDER is the first tune's.
ORDER:                 ; ROB'S YOUR UNCLE
        .byte   0,   0,   1      ;  0
        .byte   0,   2,   3      ;  1
        .byte   4,   5,   1      ;  2
        .byte   6,   7,   8      ;  3
        .byte   4,   5,   1      ;  4
        .byte   9,   7,   8      ;  5
        .byte  10,   5,   1      ;  6
        .byte  11,   7,   8      ;  7
        .byte  10,   5,   1      ;  8
        .byte  12,   7,   8      ;  9
        .byte  13,   7,   3      ; 10
        .byte  14,   5,  15      ; 11
        .byte  13,   7,   3      ; 12
        .byte  16,  17,  18      ; 13
        .byte   4,   5,   1      ; 14
        .byte   6,   7,   8      ; 15
        .byte   4,   5,   1      ; 16
        .byte   9,   7,   8      ; 17
        .byte  19,   0,   1      ; 18
        .byte  20,   0,   8      ; 19
        .byte  10,   5,   1      ; 20
        .byte  11,   7,   8      ; 21
        .byte  10,   5,   1      ; 22
        .byte  12,   7,   8      ; 23
        .byte $FF, 2
        .fill 22
order1:                 ; SWITCHED ON
        .byte  21,  22,  23      ;  0
        .byte  21,  24,  25      ;  1
        .byte  21,  26,  27      ;  2
        .byte  21,  28,  29      ;  3
        .byte  30,  22,  23      ;  4
        .byte  31,  24,  25      ;  5
        .byte  32,  26,  27      ;  6
        .byte  33,  28,  29      ;  7
        .byte  34,  22,  23      ;  8
        .byte  35,  24,  25      ;  9
        .byte  36,  26,  27      ; 10
        .byte  37,  28,  29      ; 11
        .byte  21,  22,   0      ; 12
        .byte  21,  24,   0      ; 13
        .byte  21,  26,  38      ; 14
        .byte  21,  28,  39      ; 15
        .byte $FF, 0
        .fill 46
order2:                 ; THE LONG WAY ROUND
        .byte  40,  41,  42      ;  0
        .byte  43,  44,  45      ;  1
        .byte  46,  41,  42      ;  2
        .byte  47,  48,  49      ;  3
        .byte  50,  48,  51      ;  4
        .byte  52,  53,  54      ;  5
        .byte  55,  48,  51      ;  6
        .byte  56,  57,  58      ;  7
        .byte  59,  41,  42      ;  8
        .byte  60,  44,  45      ;  9
        .byte  61,  41,  42      ; 10
        .byte  62,  48,  49      ; 11
        .byte  63,  48,  51      ; 12
        .byte  64,  53,  54      ; 13
        .byte  65,  48,  51      ; 14
        .byte  66,  57,  58      ; 15
        .byte $FF, 0
        .fill 46
order3:                 ; THE GLIDE
        .byte  67,  68,  69      ;  0
        .byte  70,  71,  72      ;  1
        .byte  73,  68,  69      ;  2
        .byte  74,  71,  72      ;  3
        .byte  75,  76,  77      ;  4
        .byte  78,  79,  80      ;  5
        .byte  81,  76,  77      ;  6
        .byte  82,  83,  84      ;  7
        .byte  67,  68,  69      ;  8
        .byte  70,  71,  72      ;  9
        .byte  73,  68,  69      ; 10
        .byte  74,  71,  72      ; 11
        .byte  75,  76,  77      ; 12
        .byte  78,  79,  80      ; 13
        .byte  81,  76,  77      ; 14
        .byte  82,  83,  84      ; 15
        .byte  67,  68,  69      ; 16
        .byte  70,  71,  72      ; 17
        .byte  73,  68,  69      ; 18
        .byte  74,  71,  72      ; 19
        .byte $FF, 0
        .fill 34
        .assert * <= PATTERNS, "the orders run into the patterns"

; The patterns: 32 rows of note, instrument. A row a pair, eight rows a line.
        .org PATTERNS
; pattern 0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 1
        .byte $11,7, $00,0, $1D,7, $00,0, $2F,8, $00,0, $11,7, $1D,7
        .byte $11,7, $00,0, $00,0, $18,7, $2F,8, $00,0, $1B,7, $00,0
        .byte $11,7, $00,0, $1D,7, $00,0, $2F,8, $00,0, $11,7, $1D,7
        .byte $11,7, $00,0, $00,0, $18,7, $2F,8, $00,0, $1B,7, $00,0
; pattern 2
        .byte $31,10, $00,0, $5F,9, $00,0, $38,10, $00,0, $5F,9, $00,0
        .byte $31,10, $00,0, $5F,9, $00,0, $38,10, $00,0, $5F,9, $00,0
        .byte $33,10, $00,0, $5F,9, $00,0, $3A,10, $00,0, $5F,9, $00,0
        .byte $33,10, $00,0, $5F,9, $00,0, $3A,10, $00,0, $5F,9, $00,0
; pattern 3
        .byte $19,7, $00,0, $25,7, $00,0, $2F,8, $00,0, $19,7, $25,7
        .byte $19,7, $00,0, $00,0, $20,7, $2F,8, $00,0, $24,7, $00,0
        .byte $1B,7, $00,0, $27,7, $00,0, $2F,8, $00,0, $1B,7, $27,7
        .byte $1B,7, $00,0, $00,0, $22,7, $2F,8, $00,0, $25,7, $00,0
; pattern 4
        .byte $3C,1, $00,0, $00,0, $41,1, $3F,1, $00,0, $41,1, $00,0
        .byte $44,1, $00,0, $00,0, $43,1, $41,1, $00,0, $3F,1, $00,0
        .byte $41,1, $00,0, $00,0, $00,0, $3C,1, $00,0, $3F,1, $00,0
        .byte $41,1, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 5
        .byte $35,10, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $5F,9, $35,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $35,10, $5F,9
        .byte $35,10, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $5F,9, $35,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $35,10, $5F,9
; pattern 6
        .byte $38,1, $00,0, $00,0, $3D,1, $3C,1, $00,0, $3D,1, $00,0
        .byte $41,1, $00,0, $00,0, $3F,1, $3D,1, $00,0, $3C,1, $00,0
        .byte $3F,1, $00,0, $00,0, $00,0, $3A,1, $00,0, $43,1, $00,0
        .byte $46,1, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 7
        .byte $31,10, $00,0, $5F,9, $5F,9, $38,10, $00,0, $5F,9, $31,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $38,10, $00,0, $31,10, $5F,9
        .byte $33,10, $00,0, $5F,9, $5F,9, $3A,10, $00,0, $5F,9, $33,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $3A,10, $00,0, $33,10, $5F,9
; pattern 8
        .byte $19,7, $00,0, $25,7, $00,0, $2F,8, $00,0, $19,7, $25,7
        .byte $19,7, $00,0, $00,0, $20,7, $2F,8, $00,0, $24,7, $00,0
        .byte $1B,7, $00,0, $27,7, $00,0, $2F,8, $00,0, $1B,7, $27,7
        .byte $1B,7, $00,0, $00,0, $22,7, $2F,8, $2F,8, $25,7, $00,0
; pattern 9
        .byte $38,1, $00,0, $00,0, $3D,1, $3C,1, $00,0, $3D,1, $00,0
        .byte $41,1, $00,0, $00,0, $3F,1, $3D,1, $00,0, $3C,1, $00,0
        .byte $46,1, $00,0, $43,1, $00,0, $3F,1, $00,0, $43,1, $00,0
        .byte $3F,1, $00,0, $00,0, $00,0, $43,1, $00,0, $00,0, $00,0
; pattern 10
        .byte $48,4, $00,0, $00,0, $4D,4, $4B,4, $00,0, $4D,4, $00,0
        .byte $50,4, $00,0, $00,0, $4F,4, $4D,4, $00,0, $4B,4, $00,0
        .byte $4D,4, $00,0, $00,0, $00,0, $48,4, $00,0, $4B,4, $00,0
        .byte $4D,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 11
        .byte $44,4, $00,0, $00,0, $49,4, $48,4, $00,0, $49,4, $00,0
        .byte $4D,4, $00,0, $00,0, $4B,4, $49,4, $00,0, $48,4, $00,0
        .byte $4B,4, $00,0, $00,0, $00,0, $46,4, $00,0, $4F,4, $00,0
        .byte $52,4, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 12
        .byte $44,4, $00,0, $00,0, $49,4, $48,4, $00,0, $49,4, $00,0
        .byte $4D,4, $00,0, $00,0, $4B,4, $49,4, $00,0, $48,4, $00,0
        .byte $52,4, $00,0, $4F,4, $00,0, $4B,4, $00,0, $4F,4, $00,0
        .byte $4B,4, $00,0, $00,0, $00,0, $4F,4, $00,0, $00,0, $00,0
; pattern 13
        .byte $41,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $44,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $43,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 14
        .byte $48,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $46,5, $00,0, $44,5, $00,0
        .byte $41,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 15
        .byte $11,7, $00,0, $1D,7, $00,0, $2F,8, $00,0, $11,7, $1D,7
        .byte $11,7, $00,0, $00,0, $18,7, $2F,8, $00,0, $1B,7, $00,0
        .byte $11,7, $00,0, $1D,7, $00,0, $2F,8, $00,0, $11,7, $1D,7
        .byte $11,7, $00,0, $00,0, $18,7, $2F,8, $2F,8, $1B,7, $00,0
; pattern 16
        .byte $48,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $46,5, $00,0, $44,5, $00,0
        .byte $48,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $43,5, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 17
        .byte $35,10, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $5F,9, $35,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $3C,10, $00,0, $35,10, $5F,9
        .byte $30,10, $00,0, $5F,9, $5F,9, $37,10, $00,0, $5F,9, $30,10
        .byte $5F,9, $00,0, $5F,9, $5F,9, $37,10, $00,0, $30,10, $5F,9
; pattern 18
        .byte $11,7, $00,0, $1D,7, $00,0, $2F,8, $00,0, $11,7, $1D,7
        .byte $11,7, $00,0, $00,0, $18,7, $2F,8, $00,0, $1B,7, $00,0
        .byte $18,7, $00,0, $24,7, $00,0, $2F,8, $00,0, $18,7, $24,7
        .byte $18,7, $00,0, $00,0, $1F,7, $2F,8, $2F,8, $22,7, $00,0
; pattern 19
        .byte $48,6, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,6, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $44,6, $00,0, $00,0, $00,0
; pattern 20
        .byte $41,6, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $43,6, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3F,6, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 21
        .byte $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $5F,29, $00,0, $5F,29, $00,0, $5F,30, $00,0
; pattern 22
        .byte $2E,25, $00,0, $35,25, $00,0, $3C,25, $00,0, $3D,25, $00,0
        .byte $41,25, $00,0, $3D,25, $00,0, $3C,25, $00,0, $35,25, $00,0
        .byte $2E,25, $00,0, $35,25, $00,0, $3C,25, $00,0, $3D,25, $00,0
        .byte $41,25, $00,0, $3D,25, $00,0, $3C,25, $00,0, $35,25, $00,0
; pattern 23
        .byte $16,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $16,26, $00,0, $31,28, $00,0, $22,27, $00,0
        .byte $16,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $16,26, $00,0, $31,28, $00,0, $1D,27, $00,0
; pattern 24
        .byte $2A,25, $00,0, $31,25, $00,0, $35,25, $00,0, $3A,25, $00,0
        .byte $3D,25, $00,0, $3A,25, $00,0, $35,25, $00,0, $31,25, $00,0
        .byte $2A,25, $00,0, $31,25, $00,0, $35,25, $00,0, $3A,25, $00,0
        .byte $3D,25, $00,0, $3A,25, $00,0, $35,25, $00,0, $31,25, $00,0
; pattern 25
        .byte $12,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $12,26, $00,0, $31,28, $00,0, $1E,27, $00,0
        .byte $12,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $12,26, $00,0, $31,28, $00,0, $19,27, $00,0
; pattern 26
        .byte $25,25, $00,0, $2C,25, $00,0, $33,25, $00,0, $35,25, $00,0
        .byte $38,25, $00,0, $35,25, $00,0, $33,25, $00,0, $2C,25, $00,0
        .byte $25,25, $00,0, $2C,25, $00,0, $33,25, $00,0, $35,25, $00,0
        .byte $38,25, $00,0, $35,25, $00,0, $33,25, $00,0, $2C,25, $00,0
; pattern 27
        .byte $19,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $19,26, $00,0, $31,28, $00,0, $25,27, $00,0
        .byte $19,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $19,26, $00,0, $31,28, $00,0, $20,27, $00,0
; pattern 28
        .byte $2C,25, $00,0, $33,25, $00,0, $3A,25, $00,0, $3C,25, $00,0
        .byte $3F,25, $00,0, $3C,25, $00,0, $3A,25, $00,0, $33,25, $00,0
        .byte $2C,25, $00,0, $33,25, $00,0, $3A,25, $00,0, $3C,25, $00,0
        .byte $3F,25, $00,0, $3C,25, $00,0, $3A,25, $00,0, $33,25, $00,0
; pattern 29
        .byte $14,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $14,26, $00,0, $31,28, $00,0, $20,27, $00,0
        .byte $14,26, $00,0, $00,0, $00,0, $31,28, $00,0, $5F,29, $00,0
        .byte $5F,29, $00,0, $14,26, $00,0, $31,28, $00,0, $31,28, $31,28
; pattern 30
        .byte $41,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3F,24, $00,0, $00,0, $00,0
        .byte $3D,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3C,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 31
        .byte $3A,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3D,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $41,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 32
        .byte $44,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $41,24, $00,0, $00,0, $00,0
        .byte $3F,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 33
        .byte $3C,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3F,24, $00,0, $00,0, $00,0
        .byte $3F,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 34
        .byte $46,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $44,24, $00,0, $00,0, $00,0, $41,24, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3D,24, $00,0, $00,0, $00,0, $3F,24, $00,0, $00,0, $00,0
; pattern 35
        .byte $41,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $42,24, $00,0, $00,0, $00,0, $41,24, $00,0, $00,0, $00,0
        .byte $3D,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 36
        .byte $41,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $44,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $49,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $48,24, $00,0, $00,0, $00,0
; pattern 37
        .byte $48,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,24, $00,0, $00,0, $00,0, $44,24, $00,0, $00,0, $00,0
        .byte $3F,24, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 38
        .byte $19,27, $00,0, $00,0, $00,0, $00,0, $00,0, $FF,0, $00,0
        .byte $00,0, $00,0, $19,27, $00,0, $25,27, $00,0, $00,0, $00,0
        .byte $19,27, $00,0, $00,0, $00,0, $00,0, $00,0, $FF,0, $00,0
        .byte $00,0, $00,0, $19,27, $00,0, $20,27, $00,0, $00,0, $00,0
; pattern 39
        .byte $14,27, $00,0, $00,0, $00,0, $00,0, $00,0, $FF,0, $00,0
        .byte $00,0, $00,0, $14,27, $00,0, $20,27, $00,0, $00,0, $00,0
        .byte $14,27, $00,0, $00,0, $00,0, $00,0, $00,0, $FF,0, $00,0
        .byte $00,0, $00,0, $14,27, $00,0, $1B,27, $00,0, $00,0, $00,0
; pattern 40
        .byte $37,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3A,3, $00,0, $00,0, $00,0, $3F,3, $00,0, $00,0, $00,0
        .byte $3E,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3A,3, $00,0, $00,0, $00,0
; pattern 41
        .byte $33,15, $37,16, $3A,15, $3F,16, $3A,15, $37,16, $3A,15, $3F,16
        .byte $43,15, $3F,16, $3A,15, $3F,16, $43,15, $46,16, $43,15, $3F,16
        .byte $3A,15, $3E,16, $41,15, $46,16, $41,15, $3E,16, $41,15, $46,16
        .byte $4A,15, $46,16, $41,15, $46,16, $4A,15, $4D,16, $4A,15, $46,16
; pattern 42
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $33,12, $00,0
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $27,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $2E,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $22,12, $00,0
; pattern 43
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3C,3, $00,0, $00,0, $00,0, $37,3, $00,0, $00,0, $00,0
        .byte $38,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3A,3, $00,0, $3C,3, $00,0
; pattern 44
        .byte $3C,15, $3F,16, $43,15, $48,16, $43,15, $3F,16, $43,15, $48,16
        .byte $4B,15, $48,16, $43,15, $48,16, $4B,15, $4F,16, $4B,15, $48,16
        .byte $38,15, $3C,16, $3F,15, $44,16, $3F,15, $3C,16, $3F,15, $44,16
        .byte $48,15, $44,16, $3F,15, $44,16, $48,15, $4B,16, $48,15, $44,16
; pattern 45
        .byte $24,11, $00,0, $24,12, $00,0, $3B,13, $00,0, $30,12, $00,0
        .byte $24,11, $00,0, $24,12, $00,0, $3B,13, $00,0, $24,12, $00,0
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $2C,12, $00,0
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $20,12, $00,0
; pattern 46
        .byte $37,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3A,3, $00,0, $00,0, $00,0, $3F,3, $00,0, $00,0, $00,0
        .byte $41,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3E,3, $00,0, $00,0, $00,0
; pattern 47
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3C,3, $00,0, $00,0, $00,0, $38,3, $00,0, $00,0, $00,0
        .byte $3A,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3C,3, $00,0, $3E,3, $00,0
; pattern 48
        .byte $38,15, $3C,16, $3F,15, $44,16, $3F,15, $3C,16, $3F,15, $44,16
        .byte $48,15, $44,16, $3F,15, $44,16, $48,15, $4B,16, $48,15, $44,16
        .byte $3A,15, $3E,16, $41,15, $46,16, $41,15, $3E,16, $41,15, $46,16
        .byte $4A,15, $46,16, $41,15, $46,16, $4A,15, $4D,16, $4A,15, $46,16
; pattern 49
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $2C,12, $00,0
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $20,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $2E,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $31,14, $2A,14
; pattern 50
        .byte $3C,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3E,3, $00,0
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3E,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3F,3, $00,0
        .byte $41,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 51
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $2C,12, $00,0
        .byte $20,11, $00,0, $20,12, $00,0, $3B,13, $00,0, $20,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $2E,12, $00,0
        .byte $22,11, $00,0, $22,12, $00,0, $3B,13, $00,0, $22,12, $00,0
; pattern 52
        .byte $3E,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3A,3, $00,0
        .byte $37,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3C,3, $00,0
        .byte $37,3, $00,0, $00,0, $00,0, $3C,3, $00,0, $00,0, $00,0
; pattern 53
        .byte $37,15, $3A,16, $3E,15, $43,16, $3E,15, $3A,16, $3E,15, $43,16
        .byte $46,15, $43,16, $3E,15, $43,16, $46,15, $4A,16, $46,15, $43,16
        .byte $3C,15, $3F,16, $43,15, $48,16, $43,15, $3F,16, $43,15, $48,16
        .byte $4B,15, $48,16, $43,15, $48,16, $4B,15, $4F,16, $4B,15, $48,16
; pattern 54
        .byte $2B,11, $00,0, $2B,12, $00,0, $3B,13, $00,0, $37,12, $00,0
        .byte $2B,11, $00,0, $2B,12, $00,0, $3B,13, $00,0, $2B,12, $00,0
        .byte $24,11, $00,0, $24,12, $00,0, $3B,13, $00,0, $30,12, $00,0
        .byte $24,11, $00,0, $24,12, $00,0, $3B,13, $00,0, $24,12, $00,0
; pattern 55
        .byte $3C,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3E,3, $00,0
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $41,3, $00,0, $00,0, $00,0, $00,0, $00,0, $3F,3, $00,0
        .byte $3E,3, $00,0, $00,0, $00,0, $41,3, $00,0, $00,0, $00,0
; pattern 56
        .byte $3F,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $3A,3, $00,0, $00,0, $00,0
        .byte $37,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 57
        .byte $33,15, $37,16, $3A,15, $3F,16, $3A,15, $37,16, $3A,15, $3F,16
        .byte $43,15, $3F,16, $3A,15, $3F,16, $43,15, $46,16, $43,15, $3F,16
        .byte $33,15, $37,16, $3A,15, $3F,16, $3A,15, $37,16, $3A,15, $3F,16
        .byte $43,15, $3F,16, $3A,15, $3F,16, $43,15, $46,16, $43,15, $3F,16
; pattern 58
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $33,12, $00,0
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $27,12, $00,0
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $33,12, $00,0
        .byte $27,11, $00,0, $27,12, $00,0, $3B,13, $00,0, $36,14, $2F,14
; pattern 59
        .byte $43,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,3, $00,0, $00,0, $00,0, $4B,3, $00,0, $00,0, $00,0
        .byte $4A,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $46,3, $00,0, $00,0, $00,0
; pattern 60
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $48,3, $00,0, $00,0, $00,0, $43,3, $00,0, $00,0, $00,0
        .byte $44,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $46,3, $00,0, $48,3, $00,0
; pattern 61
        .byte $43,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $46,3, $00,0, $00,0, $00,0, $4B,3, $00,0, $00,0, $00,0
        .byte $4D,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $4A,3, $00,0, $00,0, $00,0
; pattern 62
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $48,3, $00,0, $00,0, $00,0, $44,3, $00,0, $00,0, $00,0
        .byte $46,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $48,3, $00,0, $4A,3, $00,0
; pattern 63
        .byte $48,3, $00,0, $00,0, $00,0, $00,0, $00,0, $4A,3, $00,0
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $4A,3, $00,0, $00,0, $00,0, $00,0, $00,0, $4B,3, $00,0
        .byte $4D,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 64
        .byte $4A,3, $00,0, $00,0, $00,0, $00,0, $00,0, $46,3, $00,0
        .byte $43,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $48,3, $00,0
        .byte $43,3, $00,0, $00,0, $00,0, $48,3, $00,0, $00,0, $00,0
; pattern 65
        .byte $48,3, $00,0, $00,0, $00,0, $00,0, $00,0, $4A,3, $00,0
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $4D,3, $00,0, $00,0, $00,0, $00,0, $00,0, $4B,3, $00,0
        .byte $4A,3, $00,0, $00,0, $00,0, $4D,3, $00,0, $00,0, $00,0
; pattern 66
        .byte $4B,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $46,3, $00,0, $00,0, $00,0
        .byte $43,3, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 67
        .byte $3F,17, $00,0, $00,0, $00,0, $00,0, $00,0, $42,17, $00,0
        .byte $44,17, $00,0, $00,0, $00,0, $47,17, $00,0, $46,17, $00,0
        .byte $44,17, $00,0, $00,0, $00,0, $00,0, $00,0, $42,17, $00,0
        .byte $3F,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 68
        .byte $2C,18, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $34,19, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 69
        .byte $14,23, $00,0, $00,0, $20,22, $2F,8, $00,0, $14,22, $00,0
        .byte $14,23, $00,0, $1B,22, $20,22, $2F,8, $00,0, $1E,22, $00,0
        .byte $1C,23, $00,0, $00,0, $28,22, $2F,8, $00,0, $1C,22, $00,0
        .byte $1C,23, $00,0, $23,22, $28,22, $2F,8, $00,0, $26,22, $00,0
; pattern 70
        .byte $40,17, $00,0, $00,0, $00,0, $00,0, $00,0, $44,17, $00,0
        .byte $47,17, $00,0, $00,0, $00,0, $49,17, $00,0, $47,17, $00,0
        .byte $46,17, $00,0, $00,0, $00,0, $00,0, $00,0, $43,17, $00,0
        .byte $3F,17, $00,0, $00,0, $00,0, $43,17, $00,0, $00,0, $00,0
; pattern 71
        .byte $31,18, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $33,20, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 72
        .byte $19,23, $00,0, $00,0, $25,22, $2F,8, $00,0, $19,22, $00,0
        .byte $19,23, $00,0, $20,22, $25,22, $2F,8, $00,0, $23,22, $00,0
        .byte $1B,23, $00,0, $00,0, $27,22, $2F,8, $00,0, $1B,22, $00,0
        .byte $1B,23, $00,0, $22,22, $27,22, $2F,8, $00,0, $25,22, $00,0
; pattern 73
        .byte $3F,17, $00,0, $00,0, $00,0, $00,0, $00,0, $42,17, $00,0
        .byte $44,17, $00,0, $00,0, $00,0, $47,17, $00,0, $49,17, $00,0
        .byte $4B,17, $00,0, $00,0, $00,0, $00,0, $00,0, $47,17, $00,0
        .byte $44,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 74
        .byte $40,17, $00,0, $00,0, $44,17, $00,0, $00,0, $47,17, $00,0
        .byte $49,17, $00,0, $00,0, $47,17, $00,0, $00,0, $44,17, $00,0
        .byte $46,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $49,17, $00,0, $00,0, $00,0, $4B,17, $00,0, $00,0, $00,0
; pattern 75
        .byte $47,17, $00,0, $00,0, $00,0, $44,17, $00,0, $00,0, $00,0
        .byte $4B,17, $00,0, $00,0, $00,0, $00,0, $00,0, $49,17, $00,0
        .byte $49,17, $00,0, $00,0, $00,0, $46,17, $00,0, $00,0, $00,0
        .byte $42,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 76
        .byte $34,19, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $36,21, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 77
        .byte $1C,23, $00,0, $00,0, $28,22, $2F,8, $00,0, $1C,22, $00,0
        .byte $1C,23, $00,0, $23,22, $28,22, $2F,8, $00,0, $26,22, $00,0
        .byte $1E,23, $00,0, $00,0, $2A,22, $2F,8, $00,0, $1E,22, $00,0
        .byte $1E,23, $00,0, $25,22, $2A,22, $2F,8, $00,0, $28,22, $00,0
; pattern 78
        .byte $4B,17, $00,0, $00,0, $00,0, $47,17, $00,0, $00,0, $00,0
        .byte $44,17, $00,0, $00,0, $00,0, $00,0, $00,0, $46,17, $00,0
        .byte $47,17, $00,0, $00,0, $46,17, $00,0, $00,0, $44,17, $00,0
        .byte $3F,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 79
        .byte $2C,18, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $2C,18, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 80
        .byte $14,23, $00,0, $00,0, $20,22, $2F,8, $00,0, $14,22, $00,0
        .byte $14,23, $00,0, $1B,22, $20,22, $2F,8, $00,0, $1E,22, $00,0
        .byte $14,23, $00,0, $00,0, $20,22, $2F,8, $00,0, $14,22, $00,0
        .byte $14,23, $00,0, $1B,22, $20,22, $2F,8, $00,0, $1E,22, $00,0
; pattern 81
        .byte $47,17, $00,0, $00,0, $00,0, $44,17, $00,0, $00,0, $00,0
        .byte $4B,17, $00,0, $00,0, $00,0, $00,0, $00,0, $4C,17, $00,0
        .byte $4C,17, $00,0, $00,0, $00,0, $49,17, $00,0, $00,0, $00,0
        .byte $46,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 82
        .byte $43,17, $00,0, $00,0, $00,0, $00,0, $00,0, $46,17, $00,0
        .byte $49,17, $00,0, $00,0, $00,0, $4B,17, $00,0, $00,0, $00,0
        .byte $4B,17, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $49,17, $00,0, $46,17, $00,0
; pattern 83
        .byte $33,20, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $33,20, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
        .byte $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0, $00,0
; pattern 84
        .byte $1B,23, $00,0, $00,0, $27,22, $2F,8, $00,0, $1B,22, $00,0
        .byte $1B,23, $00,0, $22,22, $27,22, $2F,8, $00,0, $25,22, $00,0
        .byte $1B,23, $00,0, $00,0, $27,22, $2F,8, $00,0, $1B,22, $00,0
        .byte $1B,23, $00,0, $22,22, $27,22, $2F,8, $00,0, $25,22, $00,0
        .assert * <= $4000, "the patterns run off the top of memory"
; <<< song
