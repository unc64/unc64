#!/usr/bin/env sh
# Rebuilds web/vendor/codemirror.js from the exact versions in package.json (and
# package-lock.json), in a scratch directory, so nothing from npm lands in the repo.
# Usage: sh web/vendor/build/build.sh   (needs Node and network access to npm)
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
TMP="$(mktemp -d)"
cp "$HERE/package.json" "$HERE/package-lock.json" "$HERE/codemirror-entry.js" "$TMP/"
cd "$TMP"
npm ci --no-audit --no-fund
npx esbuild codemirror-entry.js --bundle --format=esm --minify --legal-comments=none --target=es2020 \
  --banner:js='/* CodeMirror 6 and @replit/codemirror-vim for the UNC 64 Machine Room. MIT licensed: see LICENSES.txt. Versions and build: README.md. */' \
  --outfile="$HERE/../codemirror.js"
echo "web/vendor/codemirror.js: $(wc -c < "$HERE/../codemirror.js") bytes"
rm -rf "$TMP"
