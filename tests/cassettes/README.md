# Test cassettes

`hello.s` (a line printed, back to READY), as a bare tape (`hello.tap`, from
`unc-asm hello.s --tape hello.tap --name HELLO`) and as a cassette (`hello.unc.png`: a
plain face with the `unCt` chunk of docs/cassettes.md, carrying the tape and the source). The
game's `--cassettes` run (scripts/run-game.sh cassettes) puts them in its cassettes folder,
finds them in the shoebox, loads one and checks the line.

`shelf/` is a shelf of two, laid out as unc64/cassettes is (`index.json`, `cassettes/`), for
the Machine Room's `--shelf` build (scripts/check-machine-room.mjs): SHELF TEST, which that
build takes in, and HELLO, which it leaves out because the Machine Room already has it from
its source.
