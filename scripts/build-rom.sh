#!/usr/bin/env sh
# Assembles the system ROM, rom/system.s, into its two images:
#
#   rom/system.bin        the 8K ROM, $E000-$FFFF: BASIC, the screen, tape, the jump table
#   rom/system-d800.bin   the second half of the 4K ROM, $D800-$DFFF: HELP and the monitor
#                         (its first half is the character set, rom/unscii.txt)
#
# Run it after any change to rom/system.s: a test fails while either image is stale.
set -e
cd "$(dirname "$0")/.."
cargo run -q -p unc-asm -- rom/system.s -o rom/system.bin --range E000-FFFF --fill FF
cargo run -q -p unc-asm -- rom/system.s -o rom/system-d800.bin --range D800-DFFF --fill FF
