#!/usr/bin/env sh
# The Machine Room as one self-contained page: build/machine-room/index.html + README.txt.
# Double-click index.html to run it (file://). Needs Node (no npm install). Regenerates
# web/bundled.js first. Rebuild web/unc.wasm with scripts/build-web.sh when the core changes.
#
#   scripts/build-machine-room.sh [OUT_DIR]
set -e
cd "$(dirname "$0")/.."
node scripts/build-machine-room.mjs "$@"
