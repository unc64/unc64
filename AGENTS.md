# For agents (and people) working in this repository

- **The manual is the spec** (`docs/prg/`). Change the machine and the manual in the same commit.
- **Rev A is in alpha, not frozen yet** (`REVISIONS.md`). The cycle table, the memory map, video timing, the ROM's jump table, BASIC's tokens and the cassette format can still change, but never by accident: a test pins them (`crates/unc-asm/tests/rev_a.rs`). A change that's meant updates `rev_a.txt`, the manual and `REVISIONS.md` in the same commit.
- After editing `rom/system.s`: `cargo run -p unc-asm -- rom/system.s -o rom/system.bin --range E000-FFFF`. A test fails if the binary is stale.
- Appendix C is generated from `crates/unc-core/src/isa.rs`: `UNC_UPDATE_DOCS=1 cargo test -p unc-asm docs`.
- Every program listing in the manual is assembled and run by `cargo test`. A new listing needs a check in `crates/unc-asm/tests/manual.rs`.
- `unc-core` has no dependencies, and stays that way.
- After changing the manual or `docs/cassettes.md`: `python3 scripts/build-llms.py`.
