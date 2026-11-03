# web/vendor

`codemirror.js` is CodeMirror 6 and its Vim mode, bundled once into a single ES module for the Machine Room's editor (`web/editor.js`). The site has no build step, so the bundle is committed; nothing is fetched from a CDN at run time.

| Package | Version |
|---|---|
| `@codemirror/state` | 6.7.6 |
| `@codemirror/view` | 6.43.15 |
| `@codemirror/language` | 6.13.1 |
| `@codemirror/commands` | 6.11.1 |
| `@codemirror/autocomplete` | 6.20.4 |
| `@codemirror/lint` | 6.9.7 |
| `@codemirror/search` | 6.7.2 |
| `@lezer/highlight` | 1.2.5 |
| `@replit/codemirror-vim` | 6.4.0 |

The dependencies that come with them (`@codemirror/streamparser`, `@lezer/common`, `@lezer/lr`, `@replit/codemirror-vim-core`, `@marijn/find-cluster-break`, `crelt`, `style-mod`, `w3c-keyname`) are pinned by `build/package-lock.json`.

Size: about 512 KB minified, 165 KB gzipped. Vim mode is about 120 KB of that.

## Rebuilding

```sh
sh web/vendor/build/build.sh
```

It copies `build/package.json`, `build/package-lock.json` and `build/codemirror-entry.js` to a temporary directory, runs `npm ci`, then bundles with esbuild 0.28.2:

```sh
npx esbuild codemirror-entry.js --bundle --format=esm --minify --legal-comments=none --target=es2020 --outfile=web/vendor/codemirror.js
```

`codemirror-entry.js` lists the exports `editor.js` uses. The one-file build (`scripts/build-machine-room.mjs`) relies on the bundle ending in one `export{...}` list: it turns that into `window.UNCCodeMirror`, so the page runs from `file://`. To use something else from CodeMirror, add it there and rebuild. To upgrade, change the versions in `build/package.json`, run `npm install` in a scratch copy to refresh `package-lock.json`, rebuild, and update the table above.

## Licence

Every package in the bundle is MIT licensed. The licence texts, with each package's name and version, are in `LICENSES.txt`.
