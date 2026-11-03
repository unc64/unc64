# For agents (and people) working in this repository

- **The manual is the spec** (`docs/prg/`). Change the machine and the manual in the same commit.
- **Rev A is frozen** (`REVISIONS.md`). Don't change the cycle table, the memory map, video timing, the ROM's jump table, BASIC's tokens or the cassette format.
- After editing `rom/system.s`: `cargo run -p unc-asm -- rom/system.s -o rom/system.bin --range E000-FFFF`. A test fails if the binary is stale.
- Appendix C is generated from `crates/unc-core/src/isa.rs`: `UNC_UPDATE_DOCS=1 cargo test -p unc-asm docs`.
- Every program listing in the manual is assembled and run by `cargo test`. A new listing needs a check in `crates/unc-asm/tests/manual.rs`.
- `unc-core` has no dependencies, and stays that way.
- After changing the manual or `docs/cassettes.md`: `python3 scripts/build-llms.py`.
