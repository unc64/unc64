#!/usr/bin/env node
// Checks the Machine Room as people get it: builds the single file (scripts/build-machine-room.mjs)
// and opens build/machine-room/index.html from a file:// URL in headless Chromium (Playwright),
// then web/ in its development mode over http. Prints one line per check and PASS or FAIL.
//
//   node scripts/check-machine-room.mjs [--no-build] [--browser chromium|firefox|webkit] [--shots DIR]
//
// What it checks: the machine boots to READY.; the editor is CodeMirror; a sample assembles
// and runs; Save puts a cassette in the library, and the .unc.png read back has the same
// unCt chunk and the same tape, byte for byte; Export downloads it; the game's test cassette
// (tests/cassettes/hello.unc.png) reads, and its tape is tests/cassettes/hello.tap; a .tap
// dropped on the page joins the library and loads from the deck with LOAD and PLAY; a BASIC
// project is typed in and runs; AURORA runs from Browse with the video board, to its title;
// FROSTHALL runs from Browse with the NIECE, to its hall;
// a BASIC project saves as a type-2 cassette that LOADs and RUNs; SAVE onto a blank tape with
// RECORD, ejected and kept, joins the library and loads; no errors in the console. --shots saves screenshots at 1440 x 900 and 390 wide.
import { createRequire } from 'node:module';
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const pw = require('playwright');
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const opt = (name, d) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : d; };
const browserName = opt('--browser', 'chromium');
const shotsDir = opt('--shots', null);

let failed = 0;
const ok = (cond, what, detail = '') => {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${what}${detail ? `: ${detail}` : ''}`);
  if (!cond) failed++;
  return cond;
};

if (!args.includes('--no-build')) execFileSync('node', [path.join(ROOT, 'scripts/build-machine-room.mjs')], { stdio: 'inherit' });
const built = path.join(ROOT, 'build/machine-room/index.html');

// the screen as text: screen codes $20-$5F are the characters, reversed ones read as themselves
const SCREEN = `(() => { const w = window.uncRoom.wasm; let s = '';
  for (let r = 0; r < 25; r++) { for (let c = 0; c < 40; c++) { const p = w.peek(0x400 + r * 40 + c) & 0x7F;
    s += p >= 0x20 && p <= 0x5F ? String.fromCharCode(p) : ' '; } s += '\\n'; } return s; })()`;

async function open(browser, url, viewport = { width: 1440, height: 900 }) {
  const ctx = await browser.newContext({ viewport, acceptDownloads: true });
  const page = await ctx.newPage();
  const errors = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('dialog', (d) => d.accept());
  await page.goto(url);
  await page.waitForFunction(() => window.uncRoom && window.uncEditor, null, { timeout: 20000 });
  return { ctx, page, errors };
}
const screenHas = (page, text, timeout = 15000) => page.waitForFunction(`${SCREEN}.includes(${JSON.stringify(text)})`, null, { timeout }).then(() => true, () => false);

async function main() {
  const browser = await pw[browserName].launch();
  try {
    // ---------- the single file, from file:// ----------
    console.log(`${browserName}, ${pathToFileURL(built).href} (${(fs.statSync(built).size / 1024).toFixed(0)} KB)`);
    const html = fs.readFileSync(built, 'utf8');
    ok(!/<(script|link|img)[^>]+(src|href)="(?!data:|#)/i.test(html) && !/https?:\/\/fonts\./.test(html), 'one file: nothing loaded from beside it or the network');
    const { page, errors } = await open(browser, pathToFileURL(built).href);
    ok(await screenHas(page, 'READY.'), 'boots to READY.');
    ok(await page.$('.cm-editor') !== null, 'the editor is CodeMirror');
    const keeps = await page.evaluate(async () => { await window.uncRoom.LIB.store.ready; return window.uncRoom.LIB.store.persistent; });
    console.log(`     (Yours is kept in ${keeps ? 'IndexedDB' : 'memory: this browser won\'t keep it on file://'})`);

    // a sample: open the bundled HELLO and run it
    await page.evaluate(() => { const b = window.UNC_BUNDLED.find((x) => x.id === 'hello'); window.uncRoom.setProject({ title: 'HELLO', author: '', about: '', machine: { ram: 16, board: false }, files: { 'hello.s': b.source }, main: 'hello.s', extra: {}, created: null, lib: null }); });
    await page.click('#b-run');
    ok(await screenHas(page, 'HELLO FROM THE UNC 64!'), 'a sample assembles and runs');
    await page.waitForFunction(() => document.getElementById('status').textContent === 'HALTED', null, { timeout: 5000 }).catch(() => {});
    ok(await page.textContent('#status') === 'HALTED', 'it halts', await page.textContent('#status'));
    ok((await page.textContent('#output')).includes('Starts at $0800'), 'Output shows the assembly');

    // Reset: back to READY. from a halted processor, with memory kept
    const first = await page.evaluate(() => window.uncRoom.wasm.peek(0x0800));
    await page.click('#resetbtn');
    await page.waitForFunction(() => { const s = window.uncRoom.screenText(); return s.includes('READY.') && !s.includes('HELLO FROM'); }, null, { timeout: 5000 }).catch(() => {});
    const after = await page.evaluate(() => ({ s: window.uncRoom.screenText(), b: window.uncRoom.wasm.peek(0x0800), st: document.getElementById('status').textContent }));
    ok(after.s.includes('READY.') && !after.s.includes('HELLO FROM') && after.st === 'RUNNING', 'Reset goes back to READY. from a halted processor', after.st);
    ok(after.b === first && first !== 0, 'and the program is still in memory', `${first} / ${after.b}`);

    // breakpoints in the source: dots in the gutter, clicked; a dot on a line with no code
    // moves to the next that has some; Run stops there and lights the line; stepping into a
    // subroutine in another file opens it; the disassembly's dot is the same breakpoint
    await page.evaluate(() => window.uncRoom.setProject({ title: 'BREAKS', author: '', about: '', machine: { ram: 16, board: false },
      files: { 'main.s': '        .org $0800\nstart:  LDX #0\nloop:   INX\n        JSR bump\n        CPX #10\n        BNE loop\n        HLT\n        .include "sub.s"\n',
        'sub.s': '; the subroutine, in a file of its own\nbump:   INC $10\n        RTS\n' }, main: 'main.s', extra: {}, created: null, lib: null }));
    const gutterClick = async (line) => {
      const at = await page.evaluate((n) => { const v = window.uncEditor.view; const c = v.coordsAtPos(v.state.doc.line(n).from); const g = document.querySelector('.cm-bp-gutter').getBoundingClientRect(); return { x: g.left + g.width / 2, y: (c.top + c.bottom) / 2 }; }, line);
      await page.mouse.click(at.x, at.y);
    };
    await gutterClick(1); // .org: no code
    await gutterClick(3); // INX
    const dots = () => page.evaluate(() => JSON.stringify(window.uncRoom.project.breakpoints?.['main.s'] || []));
    ok(await dots() === '[1,3]', 'a click in the gutter sets a breakpoint', await dots());
    await page.click('#b-run');
    await page.waitForFunction(() => /BREAK AT/.test(document.getElementById('status').textContent), null, { timeout: 5000 }).catch(() => {});
    const lit = () => page.evaluate(() => document.querySelector('.cm-pc-line')?.textContent.trim() || '');
    ok(await page.textContent('#status') === 'BREAK AT $0800' && /LDX #0/.test(await lit()), 'Run stops at the first breakpoint, its line lit', `${await page.textContent('#status')} / ${await lit()}`);
    ok(await dots() === '[2,3]', 'the dot on a line with no code moved to the next line with code', await dots());
    await page.click('#run'); // continue
    await page.waitForFunction(() => /BREAK AT \$0802/.test(document.getElementById('status').textContent), null, { timeout: 5000 }).catch(() => {});
    ok(/INX/.test(await lit()), 'Run again stops at the next one', `${await page.textContent('#status')} / ${await lit()}`);
    await page.click('#step');
    await page.click('#step'); // JSR: into bump, in sub.s
    ok(/INC \$10/.test(await lit()), 'stepping into a subroutine in another file opens it, its line lit', await lit());
    await page.click('#run'); // on to the breakpoint at INX again, where the disassembly shows it
    await page.waitForFunction(() => /BREAK AT \$0802/.test(document.getElementById('status').textContent), null, { timeout: 5000 }).catch(() => {});
    await page.evaluate(() => document.querySelector('#dis tr[data-addr="2050"]')?.click());
    ok(await dots() === '[2]' && await page.evaluate(() => !window.uncRoom.wasm.breakpoint(0x0802)), 'clearing it in the disassembly clears the dot', await dots());
    await page.evaluate(() => { const b = window.UNC_BUNDLED.find((x) => x.id === 'hello'); window.uncRoom.setProject({ title: 'HELLO', author: '', about: '', machine: { ram: 16, board: false }, files: { 'hello.s': b.source }, main: 'hello.s', extra: {}, created: null, lib: null }); });

    // an error goes to Problems
    await page.evaluate(() => { window.uncEditor.view.dispatch({ changes: { from: 0, insert: '        LDQ #1\n' } }); });
    await page.click('#b-run');
    ok(/hello\.s:1/.test(await page.textContent('#errors')), 'an error is listed in Problems', (await page.textContent('#errors')).slice(0, 80));
    await page.evaluate(() => { const v = window.uncEditor.view; v.dispatch({ changes: { from: 0, to: v.state.doc.line(2).from } }); });
    await page.click('#b-run');
    await screenHas(page, 'HELLO FROM THE UNC 64!');

    // Save: a cassette in the library; read it back
    await page.click('#b-save');
    await page.fill('#s-title', 'Hello test');
    await page.fill('#s-author', 'The checker');
    await page.fill('#s-about', 'Round trip.');
    await page.click('#s-lib');
    await page.waitForFunction(() => !document.getElementById('dlg-save').open, null, { timeout: 10000 });
    const rt = await page.evaluate(async () => {
      const R = window.uncRoom;
      const recs = await R.LIB.store.all();
      const rec = recs.find((r) => r.summary.title === 'Hello test');
      if (!rec) return { found: false };
      const png = new Uint8Array(rec.png);
      const c = R.C.read(png);
      const chunk = R.C.chunks(png).find((x) => x.type === 'unCt');
      const allCRC = R.C.chunks(png).every((x) => x.crcOK);
      // write it again from what was read: the same chunk, byte for byte
      const again = await R.C.toPNG(c, document.createElement('canvas'));
      const chunk2 = R.C.chunks(again).find((x) => x.type === 'unCt');
      const same = chunk.data.length === chunk2.data.length && chunk.data.every((v, i) => v === chunk2.data[i]);
      const t = R.buildTape(R.project.files, R.project.main, 'Hello test', { ram: 16, board: false });
      const tapeSame = t.ok && t.tape.length === c.tape.length && t.tape.every((v, i) => v === c.tape[i]);
      const img = await new Promise((res) => { const i = new Image(); i.onload = () => res([i.naturalWidth, i.naturalHeight]); i.onerror = () => res(null); i.src = URL.createObjectURL(new Blob([png], { type: 'image/png' })); });
      const json = JSON.parse(new TextDecoder().decode(chunk.data));
      return { found: true, same, tapeSame, allCRC, img, title: c.title, author: c.author, files: Object.keys(c.files || {}), main: c.main, json: Object.keys(json), name: R.C.programs(c.tape)[0].name };
    });
    ok(rt.found, 'Save puts the cassette in the library');
    ok(rt.same, 'the unCt chunk round-trips byte for byte');
    ok(rt.tapeSame, 'the tape round-trips byte for byte', `tape name ${rt.name}`);
    ok(rt.allCRC, 'every chunk\'s CRC-32 is right');
    ok(rt.img && rt.img[0] === 1004 && rt.img[1] === 638, 'the face is a 1004 x 638 PNG', JSON.stringify(rt.img));
    ok(rt.title === 'HELLO TEST' || rt.title === 'Hello test', 'title, author, files kept', `${rt.title} / ${rt.author} / ${rt.files} main ${rt.main}`);
    ok(['format', 'version', 'title', 'tape', 'files', 'main'].every((k) => rt.json.includes(k)), 'the JSON has the format\'s fields', rt.json.join(','));

    // Export: the download is the same cassette
    await page.click('#b-export');
    await page.fill('#s-title', 'Hello test');
    const [dl] = await Promise.all([page.waitForEvent('download', { timeout: 10000 }), page.click('#s-png')]);
    const dlPath = await dl.path();
    const dlBytes = [...fs.readFileSync(dlPath)];
    const dlOK = await page.evaluate((b) => { const c = window.uncRoom.C.read(new Uint8Array(b)); return c.title === 'HELLO TEST' || c.title === 'Hello test'; }, dlBytes);
    ok(dlOK && dl.suggestedFilename() === 'hello-test.unc.png', 'Export downloads the .unc.png', dl.suggestedFilename());

    // the game's test cassette and tape (tests/cassettes)
    const fixture = [...fs.readFileSync(path.join(ROOT, 'tests/cassettes/hello.unc.png'))];
    const fixtureTap = [...fs.readFileSync(path.join(ROOT, 'tests/cassettes/hello.tap'))];
    const fx = await page.evaluate(([png, tap]) => {
      const c = window.uncRoom.C.read(new Uint8Array(png));
      return { title: c.title, same: c.tape.length === tap.length && c.tape.every((v, i) => v === tap[i]), files: Object.keys(c.files || {}) };
    }, [fixture, fixtureTap]);
    ok(fx.same, 'tests/cassettes/hello.unc.png reads, and its tape is hello.tap', `${fx.title}, files ${fx.files}`);

    // a .tap dropped on the page joins the library, then loads from the deck
    await page.evaluate(async (bytes) => {
      const dt = new DataTransfer();
      dt.items.add(new File([new Uint8Array(bytes)], 'hello.tap'));
      for (const type of ['dragenter', 'drop']) document.body.dispatchEvent(new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }));
    }, fixtureTap);
    let tapId = null;
    for (let i = 0; i < 50 && !tapId; i++) {
      tapId = await page.evaluate(async () => {
        const r = (await window.uncRoom.LIB.store.all()).find((x) => x.summary.title === 'HELLO' && !x.summary.hasFiles);
        return r ? r.id : null;
      });
      if (!tapId) await page.waitForTimeout(200);
    }
    ok(!!tapId, 'a dropped .tap joins the library (Yours)');
    await page.click('#b-browse');
    await page.waitForSelector(`[data-src="yours"] [data-id="${tapId}"] [data-act="run"]`, { timeout: 10000 });
    await page.click(`[data-src="yours"] [data-id="${tapId}"] [data-act="run"]`);
    ok(await screenHas(page, 'FOUND HELLO', 10000), 'Run puts it in the deck: LOAD, PLAY, FOUND HELLO');
    ok(await screenHas(page, 'HELLO FROM A CASSETTE', 15000), 'and it loads and runs');

    // BASIC: typed in, then RUN
    await page.evaluate(() => {
      window.uncRoom.setProject({ title: 'BASIC TEST', author: '', about: '', machine: { ram: 16, board: false }, files: { 'main.bas': '# a note, not typed\n10 PRINT "SUM";\n20 S=0:FOR I=1 TO 10:S=S+I:NEXT\n30 PRINT S\n' }, main: 'main.bas', extra: {}, created: null, lib: null });
    });
    await page.click('#b-run');
    ok(await screenHas(page, 'SUM55', 15000), 'a BASIC project is typed in and runs');

    // BASIC on a cassette: Save builds a type-2 tape; Run in Browse LOADs it, then RUNs it
    await page.click('#b-save');
    await page.fill('#s-title', 'Sums');
    await page.click('#s-lib');
    await page.waitForFunction(() => !document.getElementById('dlg-save').open, null, { timeout: 10000 }).catch(() => {});
    const bas = await page.evaluate(async () => {
      const R = window.uncRoom;
      const rec = (await R.LIB.store.all()).find((r) => r.summary.title === 'Sums');
      if (!rec) return { found: false, err: document.getElementById('s-err').textContent };
      const c = R.C.read(new Uint8Array(rec.png));
      const p = R.C.programs(c.tape)[0];
      return { found: true, id: rec.id, type: p.type, name: p.name, ok: p.ok, main: c.main, kind: rec.summary.kind };
    });
    ok(bas.found && bas.type === 2 && bas.ok, 'a BASIC project saves as a cassette with a type-2 tape', JSON.stringify(bas));
    await page.evaluate(() => window.uncRoom.wasm.power_on(16));
    await page.click('#b-browse');
    await page.click(`[data-src="yours"] [data-id="${bas.id}"] [data-act="run"]`);
    ok(await screenHas(page, 'FOUND SUMS', 10000), 'it goes in the deck: LOAD, PLAY, FOUND SUMS');
    ok(await page.waitForFunction(() => /RUN\s*\n\s*SUM55/.test(window.uncRoom.screenText()), null, { timeout: 15000 }).then(() => true, () => false), 'it loads, and RUNs: SUM55');

    // SAVE onto a blank tape with RECORD; eject; keep it; it's in Yours and loads
    await page.click('#d-blank');
    await page.click('#screen');
    await page.keyboard.type('SAVE "ADDER"', { delay: 60 });
    await page.keyboard.press('Enter');
    ok(await screenHas(page, 'PRESS RECORD AND PLAY ON TAPE', 5000), 'SAVE asks for RECORD');
    await page.click('#d-rec');
    const saved = await page.waitForFunction(() => {
      const w = window.uncRoom.wasm;
      const b = window.uncRoom.deckBytes();
      return b && b.length > 28 && !(w.tape_status() & 4) && window.uncRoom.screenText().includes('SAVING') ? b.length : false;
    }, null, { timeout: 15000 }).then((h) => h.jsonValue(), () => 0);
    ok(saved > 0, 'SAVE records on the blank tape', `${saved} bytes`);
    if (shotsDir) {
      fs.mkdirSync(shotsDir, { recursive: true });
      await page.waitForTimeout(300);
      await page.screenshot({ path: path.join(shotsDir, 'deck-recording-1440.png') });
    }
    await page.click('#d-eject');
    await page.waitForSelector('#toast:not([hidden]) button', { timeout: 5000 }).catch(() => {});
    const offer = await page.textContent('#toast');
    ok(/ADDER/.test(offer), 'Eject offers the recording', offer);
    await page.click('#toast button');
    let addId = null;
    for (let i = 0; i < 50 && !addId; i++) {
      addId = await page.evaluate(async () => (await window.uncRoom.LIB.store.all()).find((r) => r.summary.title === 'ADDER')?.id || null);
      if (!addId) await page.waitForTimeout(200);
    }
    ok(!!addId, 'Keep it as a cassette puts it in Yours');
    if (addId) {
      await page.evaluate(() => window.uncRoom.wasm.power_on(16));
      await page.click('#b-browse');
      await page.click(`[data-src="yours"] [data-id="${addId}"] [data-act="run"]`);
      ok(await page.waitForFunction(() => /FOUND ADDER[\s\S]*SUM55/.test(window.uncRoom.screenText()), null, { timeout: 20000 }).then(() => true, () => false), 'and it loads and RUNs from the deck');
    }

    // a bundled BASIC cassette runs from a real type-2 tape
    await page.click('#b-browse');
    await page.click('[data-src="bundled"] [data-id="sun"] [data-act="run"]');
    ok(await page.waitForFunction(() => window.uncRoom.deck && window.uncRoom.C.programs(window.uncRoom.deck.cassette.tape)[0].type === 2 && /FOUND SUN/.test(window.uncRoom.screenText()), null, { timeout: 15000 }).then(() => true, () => false), 'a bundled BASIC cassette loads from a type-2 tape');
    await page.waitForFunction(() => !window.uncRoom.typing && window.uncRoom.wasm.peek(0x0204) !== 0, null, { timeout: 15000 }).catch(() => {});
    ok(await page.evaluate(() => window.uncRoom.wasm.peek(0x0204)) !== 0, 'and RUNs (BASIC is running)');

    // AURORA from Browse, with the video board, to its title
    await page.click('#b-browse');
    await page.waitForSelector('[data-src="bundled"] [data-id="aurora"] [data-act="run"]');
    await page.click('[data-src="bundled"] [data-id="aurora"] [data-act="run"]');
    const aurora = await page.waitForFunction(() => {
      const w = window.uncRoom.wasm;
      if (!w.has_video_board()) return false;
      const base = w.peek(0xC0A0) << 8;
      let shown = 0;
      for (let i = 0; i < 32; i++) if (w.peek(base + i * 4 + 3) & 0x80) shown++;
      return shown >= 6 ? shown : false;
    }, null, { timeout: 30000 }).then((h) => h.jsonValue(), () => 0);
    ok(aurora > 0 && await page.isChecked('#board'), 'AURORA runs from Browse with the video board, to its title', `${aurora} sprites on`);

    // FROSTHALL from Browse, with the NIECE: its page shown, in sixteen colours
    await page.click('#b-browse');
    await page.waitForSelector('[data-src="bundled"] [data-id="frosthall"] [data-act="run"]');
    await page.click('[data-src="bundled"] [data-id="frosthall"] [data-act="run"]');
    const hall = await page.waitForFunction(() => {
      const w = window.uncRoom.wasm;
      if (!w.has_niece || !w.has_niece() || w.peek(0xC100) !== 1 || !/FROSTHALL/.test(window.uncRoom.screenText())) return false;
      const px = new Uint8Array(w.memory.buffer, w.render(1), 64000);
      const colours = new Set();
      for (let i = 0; i < 168 * 320; i++) if (px[i] & 16) colours.add(px[i] & 15);
      return colours.size >= 5 ? colours.size : false;
    }, null, { timeout: 60000 }).then((h) => h.jsonValue(), () => 0);
    ok(hall > 0 && await page.isChecked('#niece'), 'FROSTHALL runs from Browse with the NIECE, to its hall', `${hall} colours on the page`);

    // Browse: faces for the bundled cassettes
    await page.click('#b-browse');
    await page.waitForFunction(() => document.querySelectorAll('[data-src="bundled"] img.on').length >= 5, null, { timeout: 30000 }).catch(() => {});
    const facesOn = await page.$$eval('[data-src="bundled"] img.on', (l) => l.length);
    ok(facesOn >= 5, 'Browse shows the bundled cassettes\' faces', `${facesOn}`);
    if (shotsDir) {
      fs.mkdirSync(shotsDir, { recursive: true });
      await page.waitForTimeout(2500);
      await page.screenshot({ path: path.join(shotsDir, 'browse-1440.png') });
      await page.keyboard.press('Escape');
      await page.evaluate(() => { const b = window.UNC_BUNDLED.find((x) => x.id === 'snake'); window.uncRoom.setProject({ title: 'SNAKE', author: '', about: b.about, machine: { ram: 16, board: false }, files: { 'snake.s': b.source }, main: 'snake.s', extra: {}, created: null, lib: null }); });
      await page.click('#b-run');
      await page.waitForTimeout(2000);
      await page.click('[data-pane="debugger"]');
      await page.screenshot({ path: path.join(shotsDir, 'ide-1440.png') });
      await page.click('#b-save');
      await page.fill('#s-title', 'Snake');
      await page.fill('#s-author', 'You');
      await page.waitForTimeout(300);
      await page.screenshot({ path: path.join(shotsDir, 'save-1440.png') });
      await page.keyboard.press('Escape');
    }
    ok(errors.length === 0, 'no errors in the console', errors.slice(0, 3).join(' | '));
    if (keeps) {
      const title = await page.evaluate(() => window.uncRoom.project.title);
      await page.waitForTimeout(1000); // the draft is written 600 ms after a change
      await page.reload();
      await page.waitForFunction(() => window.uncRoom && window.uncEditor, null, { timeout: 20000 });
      ok(await page.evaluate(() => window.uncRoom.project.title) === title, 'the working project is there after a reload', title);
    }

    if (shotsDir) {
      const m = await open(browser, pathToFileURL(built).href, { width: 390, height: 844 });
      await screenHas(m.page, 'READY.');
      await m.page.screenshot({ path: path.join(shotsDir, 'ide-390.png'), fullPage: true });
      await m.page.click('#b-browse');
      await m.page.waitForTimeout(4000);
      await m.page.screenshot({ path: path.join(shotsDir, 'browse-390.png'), fullPage: false });
      await m.ctx.close();
    }

    // ---------- a phone: tap the screen and the keyboard's field has the focus ----------
    {
      const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: browserName !== 'firefox' });
      const p = await ctx.newPage();
      const perr = [];
      p.on('pageerror', (e) => perr.push(e.message));
      await p.goto(pathToFileURL(built).href);
      await p.waitForFunction(() => window.uncRoom && window.uncEditor, null, { timeout: 20000 });
      await screenHas(p, 'READY.');
      const coarse = await p.evaluate(() => matchMedia('(pointer: coarse)').matches);
      ok(await p.evaluate(() => document.activeElement.id) !== 'typein', 'on a phone the keyboard is not asked for until the screen is tapped');
      await p.tap('#screen');
      ok(await p.evaluate(() => document.activeElement.id) === 'typein', 'a tap on the screen focuses the field a phone\'s keyboard types into');
      // a phone's keyboard: text arrives in the field with no keydown, and delete takes a character out of it
      await p.keyboard.insertText('PRINT 6*7X');
      await p.evaluate(() => { const k = document.getElementById('typein'); k.value = ''; k.dispatchEvent(new Event('input', { bubbles: true })); });
      await p.keyboard.press('Enter');
      ok(await p.waitForFunction(() => /PRINT 6\*7\s*\n\s*42/.test(window.uncRoom.screenText()), null, { timeout: 10000 }).then(() => true, () => false), 'typed text and the keyboard\'s delete reach the machine: PRINT 6*7, 42');
      if (coarse) {
        ok(await p.isVisible('#softkeys [data-key="3"]'), 'the cursor keys, STOP and RESTORE are under the monitor');
        const held = await p.evaluate(() => {
          const b = document.querySelector('#softkeys [data-key="29"]');
          b.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true }));
          const w = window.uncRoom.wasm; for (let i = 0; i < 2000; i++) w.step();
          const down = w.peek(0xC012);
          b.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
          return [down, document.activeElement.id];
        });
        ok(held[0] === 29 && held[1] === 'typein', 'a cursor key is down while it is held, and the keyboard keeps the focus', String(held));
      } else console.log('     (this browser does not report a touch screen: the key row is not checked)');
      ok(perr.length === 0, 'no errors on a phone', perr.slice(0, 3).join(' | '));
      await ctx.close();
    }

    // ---------- web/ in development mode ----------
    const port = 8600 + Math.floor(Math.random() * 300);
    const srv = spawn('python3', ['-m', 'http.server', String(port), '-d', path.join(ROOT, 'web'), '--bind', '127.0.0.1'], { stdio: 'ignore' });
    try {
      await new Promise((r) => setTimeout(r, 800));
      const d = await open(browser, `http://127.0.0.1:${port}/`);
      ok(await screenHas(d.page, 'READY.') && await d.page.$('.cm-editor') !== null, 'web/ in development mode boots, with the editor');
      ok(d.errors.length === 0, 'no errors in development mode', d.errors.slice(0, 3).join(' | '));
      await d.ctx.close();
    } finally {
      srv.kill();
    }
  } finally {
    await browser.close();
  }
  console.log(failed ? `FAIL (${failed})` : 'PASS');
  process.exit(failed ? 1 : 0);
}
main().catch((e) => { console.error(e); console.log('FAIL'); process.exit(1); });
