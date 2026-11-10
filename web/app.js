// The Machine Room: a browser IDE with the UNC 64 built in. The project (its files, which
// one is main), the machine (screen, sound, keyboard, debugger), the tape deck, the library
// (Browse) and cassettes (Save, Export, Open, drop). The code editor is editor.js, which
// takes over from the textarea once this is up (window.UNCRoom.ready). Everything runs in
// the browser; nothing is fetched but unc.wasm in development (the built single file has it
// inlined as window.UNC_WASM_B64).
(async () => {
  const $ = (id) => document.getElementById(id);
  let roomReady;
  window.UNCRoom = { ready: new Promise((r) => { roomReady = r; }) };
  const hex = (v, n) => v.toString(16).toUpperCase().padStart(n, '0');
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const C = window.UNCCassette;
  const LIB = window.UNCLibrary;
  // Small preferences (screen colour, mute, keys, fast load, your name): localStorage when
  // the browser allows it. Nothing that matters lives there.
  const pref = {
    get(k, d) { try { const v = localStorage.getItem('unc64.' + k); return v === null ? d : v; } catch (e) { return d; } },
    set(k, v) { try { localStorage.setItem('unc64.' + k, v); } catch (e) { /* no storage */ } },
  };

  const PALETTE = ['#000000', '#e8e4da', '#9a3b2e', '#5fb3a8', '#7a3f8a', '#4f9a4f', '#2c3d9a', '#e2c84a',
    '#a8632a', '#5a3e1a', '#d0705f', '#3a3a3a', '#6b6b6b', '#9fe08a', '#6f8fd8', '#a9a9a9'];
  const rgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  // The monochrome monitor in the phosphor you pick (phosphor.js, the same file as the
  // site's). The machine doesn't know.
  const PH = window.UNCPHOSPHOR;
  let phos = PH.current();
  let MONO = PH.mono(PALETTE);
  let colour = pref.get('colour', '0') === '1';

  // ---------- load the machine ----------
  // Three instances of the same emulator: the machine you use; and the bench, which checks
  // your source as you type, builds tapes and takes the pictures for cassette faces without
  // ever touching the machine you're debugging.
  let wasm;
  let bench;
  let module;
  try {
    let bin;
    if (window.UNC_WASM_B64) bin = C.fromBase64(window.UNC_WASM_B64);
    else bin = await (await fetch('unc.wasm')).arrayBuffer();
    module = await WebAssembly.compile(bin);
    wasm = (await WebAssembly.instantiate(module, {})).exports;
    bench = (await WebAssembly.instantiate(module, {})).exports;
  } catch (e) {
    $('status').textContent = 'COULD NOT LOAD THE MACHINE';
    $('status').className = 'bad';
    console.error(e);
    return;
  }
  const mem = (ex) => new Uint8Array(ex.memory.buffer);
  const readOut = (ex, len) => new TextDecoder().decode(mem(ex).subarray(ex.output_ptr(), ex.output_ptr() + len));
  const readBytes = (ex, len) => mem(ex).slice(ex.output_ptr(), ex.output_ptr() + len);
  const writeIn = (ex, data) => {
    const enc = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    const p = ex.input_buffer(enc.length);
    mem(ex).set(enc, p);
  };

  // The deck's RECORD side and BASIC on tape (manual chapter 14): blank tapes, RECORD, the
  // tape's bytes back (with what SAVE recorded), and a type-2 tape of the BASIC program in
  // memory. Feature-detected, so an older unc.wasm still runs: the controls just hide.
  const TAPEX = {
    blank: (ex) => ex.insert_blank_tape, // ()
    record: (ex) => ex.press_record, // () -> 1 if RECORD and PLAY went down
    recording: (ex) => ex.tape_record, // () -> 1 while RECORD is down
    contents: (ex) => ex.tape_bytes, // () -> length into the output buffer, -1 with no tape
    basic: (ex) => ex.basic_tape, // (name in the input buffer) -> length into the output buffer
  };
  const has = (k) => typeof TAPEX[k](wasm) === 'function';
  // The tape in the deck now, recordings and all; null with no tape in.
  function deckBytes() {
    const f = TAPEX.contents(wasm);
    if (!f) return null;
    const n = f();
    return n < 0 ? null : readBytes(wasm, n);
  }

  // ---------- the assembler ----------
  const isBasic = (path) => /\.bas$/i.test(path || '');
  // The project's files for assemble_project_input: path NUL text NUL ..., main first.
  function pack(files, main) {
    const order = [main, ...Object.keys(files).filter((p) => p !== main)];
    return order.map((p) => `${p}\0${files[p] ?? ''}`).join('\0');
  }
  // Assembles a project on an instance (and loads it there). Errors and lines name their
  // file: null is main, as the core says; here it becomes main's path.
  function assemble(ex, files, main) {
    let res;
    if (ex.assemble_project_input) {
      writeIn(ex, pack(files, main));
      res = JSON.parse(readOut(ex, ex.assemble_project_input()));
    } else {
      writeIn(ex, files[main] ?? '');
      res = JSON.parse(readOut(ex, ex.assemble_input()));
    }
    const fix = (f) => (f === null || f === undefined ? main : f.replace(/^\.\//, ''));
    if (res.errors) res.errors = res.errors.map(([line, msg, f]) => [line, msg, fix(f)]);
    if (res.lines) res.lines = res.lines.map(([line, addr, len, f]) => [line, addr, len, fix(f)]);
    return res;
  }
  // What LOAD "NAME" finds: capitals, $20-$5F, 16 at most (unc_asm::tape::tape_name).
  const tapeName = (title) => [...String(title).toUpperCase()].filter((c) => c >= ' ' && c <= '_').join('').slice(0, 16).trim() || 'UNTITLED';

  // ---------- typing into a machine ----------
  // A key at a time, as fast as the ROM takes them: each key goes into KBD and the machine
  // runs until it has been taken (bit 7 of KBD clear). `#` lines are notes in a listing
  // (the magazine's .bas listings), never typed.
  const keysOf = (text) => {
    const out = [];
    for (const ch of text.toUpperCase()) {
      const c = ch === '\n' || ch === '\r' ? 13 : ch === '\t' ? 32 : ch.charCodeAt(0);
      if (c === 13 || (c >= 0x20 && c <= 0x5F)) out.push(c);
    }
    return out;
  };
  const listingLines = (src) => src.split(/\r?\n/).map((l) => l.trimEnd()).filter((l) => l.trim() && !l.trimStart().startsWith('#'));
  function feedKey(ex, code) {
    ex.key(code);
    for (let n = 0; n < 400000 && (ex.peek(0xC010) & 0x80); n++) if (!ex.step()) break;
    ex.key_up(code);
    // let it echo the key and come back to wait for the next before handing over another
    for (let n = 0; n < 200; n++) if (!ex.step()) break;
  }
  const typeAll = (ex, codes) => codes.forEach((c) => feedKey(ex, c));

  // ---------- the bench: tapes and pictures ----------
  function benchPower(ram, board) {
    bench.set_video_board(board ? 1 : 0);
    bench.power_on(ram === 64 ? 64 : 16);
  }
  // A project on tape: { ok, tape } or { ok: false, errors, message }.
  function buildTape(files, main, title, machine) {
    if (isBasic(main)) {
      const fn = TAPEX.basic(bench);
      if (!fn) return { ok: false, errors: [], message: 'This unc.wasm has no BASIC on tape (basic_tape). Run still types it in.' };
      benchPower(machine.ram, machine.board);
      typeAll(bench, keysOf(listingLines(files[main] || '').join('\r') + '\r'));
      for (let i = 0; i < 10; i++) bench.run_frame(); // let BASIC store the last line
      writeIn(bench, tapeName(title));
      const n = fn();
      return n ? { ok: true, tape: readBytes(bench, n) } : { ok: false, errors: [], message: 'The machine made no tape: is there a program?' };
    }
    const res = assemble(bench, files, main);
    if (!res.ok) return { ok: false, errors: res.errors, message: `${res.errors.length} error${res.errors.length > 1 ? 's' : ''}: fix ${res.errors.length > 1 ? 'them' : 'it'} first.` };
    if (!bench.make_tape_input) return { ok: false, errors: [], message: 'This machine can\'t make tapes (make_tape_input is missing).' };
    writeIn(bench, tapeName(title));
    return { ok: true, tape: readBytes(bench, bench.make_tape_input()) };
  }

  // The picture as the monitor shows it, with a little border: 352 x 220, a cassette's label art.
  function picture(ex, blink = true) {
    const cv = document.createElement('canvas');
    cv.width = 352;
    cv.height = 220;
    const g = cv.getContext('2d');
    const bl = mem(ex).subarray(ex.border_lines_ptr(), ex.border_lines_ptr() + 312);
    for (let y = 0; y < 220; y++) {
      g.fillStyle = colour ? PALETTE[bl[y + 41] & 15] : PH.border(MONO, bl[y + 41]);
      g.fillRect(0, y, 352, 1);
    }
    const img = g.createImageData(320, 200);
    paintPixels(ex, img, blink, (y, c) => { g.fillStyle = (colour ? PALETTE : MONO)[c]; g.fillRect(16, 10 + y, 320, 1); });
    const pix = document.createElement('canvas');
    pix.width = 320;
    pix.height = 200;
    pix.getContext('2d').putImageData(img, 0, 0);
    g.drawImage(pix, 16, 10);
    return cv;
  }
  // The core draws the picture as the beam drew it (render, manual chapter 9); this colours
  // it, line by line, in the colours each line had when it was drawn (line_colours_ptr):
  // value 0 is the background, which `background(y, colour)` paints under the line; 1-3 show
  // the line's ink colours. On the monochrome monitor each is a shade of the phosphor, and
  // white is the phosphor itself.
  function paintPixels(ex, img, blink, background) {
    const ptr = ex.render(blink ? 1 : 0);
    const px = mem(ex).subarray(ptr, ptr + 64000);
    const lc = ex.line_colours_ptr ? mem(ex).subarray(ex.line_colours_ptr(), ex.line_colours_ptr() + 800) : null;
    const now = [0, 1, 2, 3].map((v) => (v ? (ex.colour_of ? ex.colour_of(v) & 15 : 1) : ex.peek(0xC021) & 15));
    const shade = (c) => (colour ? rgb(PALETTE[c]) : c === 1 ? phos.ink : MONO[c].match(/\d+/g).slice(0, 3).map(Number));
    const d = img.data;
    for (let y = 0, i = 0, o = 0; y < 200; y++) {
      const cs = lc ? lc.subarray(y * 4, y * 4 + 4) : now;
      background(y, cs[0] & 15);
      const inks = [null, shade(cs[1] & 15), shade(cs[2] & 15), shade(cs[3] & 15)];
      for (let x = 0; x < 320; x++, i++, o += 4) {
        const k = inks[px[i] & 3];
        if (k) { d[o] = k[0]; d[o + 1] = k[1]; d[o + 2] = k[2]; d[o + 3] = 255; } else d[o + 3] = 0;
      }
    }
  }
  // Runs a project on the bench for a few seconds and takes its picture.
  function shoot(files, main, machine, frames = 150) {
    benchPower(machine.ram, machine.board);
    if (isBasic(main)) typeAll(bench, keysOf(listingLines(files[main] || '').join('\r') + '\rRUN\r'));
    else if (!assemble(bench, files, main).ok) return null;
    for (let i = 0; i < frames; i++) if (bench.run_frame() > 1) break;
    return picture(bench);
  }
  async function faceFor(o) {
    await C.fontsReady();
    return C.drawFace(document.createElement('canvas'), o);
  }

  // ---------- the screen ----------
  const cv = $('screen');
  // The keys go to a field over the screen (#typein), not to the canvas: a canvas can't bring
  // up a phone's keyboard, and a field can. A tap on the screen focuses it. Run and the deck
  // focus it too, except on a touch screen, where the keyboard only comes up when asked.
  const kb = $('typein');
  const touch = window.matchMedia?.('(pointer: coarse)').matches;
  const focusScreen = () => { if (!touch) kb.focus({ preventScroll: true }); };
  if (touch) $('kbd-hint').textContent = 'Tap the screen to type.';
  const ctx = cv.getContext('2d');
  const pix = document.createElement('canvas');
  pix.width = 320;
  pix.height = 200;
  const pctx = pix.getContext('2d');
  const pimg = pctx.createImageData(320, 200);
  function render(time) {
    const bl = mem(wasm).subarray(wasm.border_lines_ptr(), wasm.border_lines_ptr() + 312);
    for (let y = 0; y < 296; y++) {
      ctx.fillStyle = colour ? PALETTE[bl[y + 16] & 15] : PH.border(MONO, bl[y + 16]);
      ctx.fillRect(0, y * 2, 768, 2);
    }
    paintPixels(wasm, pimg, Math.floor(time / 400) % 2 === 0, (y, c) => { ctx.fillStyle = (colour ? PALETTE : MONO)[c]; ctx.fillRect(64, 70 + y * 2, 640, 2); });
    pctx.putImageData(pimg, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.shadowColor = colour ? 'transparent' : phos.glow;
    ctx.shadowBlur = colour ? 0 : 6;
    ctx.drawImage(pix, 64, 70, 640, 400);
    ctx.shadowBlur = 0;
    ctx.fillStyle = 'rgba(0,0,0,0.28)';
    for (let y = 1; y < 592; y += 2) ctx.fillRect(0, y, 768, 1); // one dark gap per raster line
  }

  // ---------- sound (manual chapter 12) ----------
  // The core renders each frame's speaker as samples (audio_frame); Web Audio plays them
  // back to back, about 80 ms ahead. Browsers allow sound only after a click or a key.
  const sound = (() => {
    const AC = window.AudioContext || window.webkitAudioContext;
    let ac = null;
    let gain = null;
    let next = 0;
    let muted = pref.get('mute', '0') === '1';
    const stats = { frames: 0, samples: 0, nonzero: 0 };
    const start = () => {
      if (!AC || !wasm.audio_frame) return;
      if (!ac) {
        ac = new AC();
        gain = ac.createGain();
        gain.gain.value = muted ? 0 : 1;
        gain.connect(ac.destination);
      }
      if (ac.state === 'suspended') ac.resume().catch(() => {});
    };
    const frame = () => {
      if (!ac || ac.state !== 'running') return;
      const sr = ac.sampleRate;
      const now = ac.currentTime;
      if (next < now + 0.02) next = now + 0.08;
      const ahead = next - now;
      if (ahead > 0.3) return;
      const base = sr * 0.019968; // one frame: 19,968 cycles at 1 MHz
      const n = Math.round(base * Math.min(1.03, Math.max(0.97, 1 + (0.08 - ahead) * 0.5)));
      const ptr = wasm.audio_frame(sr, n);
      const data = new Float32Array(wasm.memory.buffer, ptr, n);
      const buf = ac.createBuffer(1, n, sr);
      buf.copyToChannel(data, 0);
      const src = ac.createBufferSource();
      src.buffer = buf;
      src.connect(gain);
      src.start(next);
      next += n / sr;
      stats.frames += 1;
      stats.samples += n;
      for (let i = 0; i < n; i++) if (data[i] !== 0) stats.nonzero += 1;
    };
    const setMuted = (on) => {
      muted = on;
      if (gain) gain.gain.value = on ? 0 : 1;
      pref.set('mute', on ? '1' : '0');
    };
    ['pointerdown', 'keydown'].forEach((ev) => document.addEventListener(ev, start, true));
    window.uncSound = { stats, get state() { return ac ? ac.state : 'not started'; } }; // for tests
    return { frame, setMuted, get muted() { return muted; } };
  })();

  // ---------- the project ----------
  // { title, author, about, machine: { ram, board }, files: { path: text }, main, extra,
  //   created, lib: { src, id } when it came from (or went to) the library }
  let project = null;
  let tabs = [];
  let current = null;
  let dirty = false;
  let program = null; // the last assembly: lines, symbols, for the debugger
  const sampleOf = (id) => window.UNC_BUNDLED.find((b) => b.id === id);
  const NEW_ASM = `; UNTITLED: say what it does here.
CHROUT  = $FF03         ; print the character in A
PRSTR   = $FF09         ; print the string at X (low) and Y (high), ending in 0

        .org $0800
start:  LDX #<msg
        LDY #>msg
        JSR PRSTR
        HLT

msg:    .text "HELLO, UNC 64!"
        .byte 13, 0
`;
  const NEW_BAS = `# UNTITLED. Lines starting with # are notes: Run types in the rest.
10 CLS
20 PRINT "HELLO, UNC 64!"
30 FOR I=1 TO 10:PRINT I;" ";:NEXT
40 PRINT
`;
  function newProject(kind) {
    const main = kind === 'basic' ? 'main.bas' : 'main.s';
    return { title: 'UNTITLED', author: pref.get('author', ''), about: '', machine: { ram: 16, board: false }, files: { [main]: kind === 'basic' ? NEW_BAS : NEW_ASM }, main, extra: {}, created: null, lib: null };
  }
  function fromCassette(c, lib) {
    return { title: c.title, author: c.author, about: c.about, machine: { ...c.machine }, files: { ...c.files }, main: c.main, extra: { ...c.extra }, created: c.created, lib };
  }
  function setProject(p, opts = {}) {
    project = p;
    tabs = [p.main];
    current = p.main;
    dirty = !!opts.dirty;
    program = null;
    editor.reset();
    showTitle();
    renderFiles();
    openFile(p.main);
    setMachine(p.machine.ram, p.machine.board, opts.power !== false);
    clearProblems();
    $('output').textContent = 'Nothing assembled yet.';
    saveDraft();
  }
  function markDirty() {
    if (!dirty) { dirty = true; $('dirty').hidden = false; }
    saveDraft();
  }
  function showTitle() {
    $('ptitle').textContent = project.title || 'UNTITLED';
    $('dirty').hidden = !dirty;
    document.title = `${project.title} · UNC 64 Machine Room`;
  }

  // The working project survives a reload: kept as a draft in the library's store.
  let draftTimer = 0;
  function saveDraft() {
    clearTimeout(draftTimer);
    draftTimer = setTimeout(() => {
      LIB.store.setState('draft', { project, tabs, current, dirty }).catch(() => {});
    }, 600);
  }

  // ---------- files and tabs ----------
  const okPath = (p) => /^[\w][\w .-]*(\/[\w][\w .-]*)*$/.test(p) && !p.split('/').includes('..');
  function renderFiles() {
    const paths = Object.keys(project.files).sort((a, b) => (a === project.main ? -1 : b === project.main ? 1 : a.localeCompare(b)));
    $('files').innerHTML = paths.map((p) => `<li data-path="${esc(p)}" class="${p === current ? 'cur' : ''}" tabindex="0" title="${esc(p)}">
      <span class="nm">${esc(p)}</span>${p === project.main ? '<span class="main" title="Run starts here">MAIN</span>' : ''}
      <span class="fa">${p === project.main ? '' : '<button type="button" data-fa="main" title="Run starts from this file">main</button>'}<button type="button" data-fa="rename" title="Rename">rename</button><button type="button" data-fa="delete" title="Delete">×</button></span></li>`).join('');
    $('f-note').textContent = isBasic(project.main)
      ? 'A BASIC project: Run types main into the machine.'
      : paths.length > 1 ? `.include "file" reads another file.` : 'Add files with + File, and .include them.';
    $('tabs').innerHTML = tabs.map((p) => `<button type="button" role="tab" data-tab="${esc(p)}" aria-selected="${p === current}" title="${esc(p)}">${esc(p)}${tabs.length > 1 ? '<span class="x" data-close title="Close">×</span>' : ''}</button>`).join('');
  }
  function openFile(path) {
    if (!(path in project.files)) return;
    if (!tabs.includes(path)) tabs.push(path);
    current = path;
    editor.show(path, project.files[path]);
    editor.setBreakpoints?.(path, bpsOf(path));
    renderFiles();
    saveDraft();
  }
  $('files').addEventListener('click', (e) => {
    const li = e.target.closest('li');
    if (!li) return;
    const p = li.dataset.path;
    const act = e.target.closest('[data-fa]')?.dataset.fa;
    if (act === 'main') { project.main = p; markDirty(); renderFiles(); return; }
    if (act === 'rename') { renameFile(p); return; }
    if (act === 'delete') { deleteFile(p); return; }
    openFile(p);
  });
  $('files').addEventListener('keydown', (e) => { if (e.key === 'Enter' && e.target.matches('li')) openFile(e.target.dataset.path); });
  $('tabs').addEventListener('click', (e) => {
    const b = e.target.closest('[data-tab]');
    if (!b) return;
    const p = b.dataset.tab;
    if (e.target.closest('[data-close]')) {
      tabs = tabs.filter((t) => t !== p);
      if (current === p) openFile(tabs[tabs.length - 1]);
      else renderFiles();
      return;
    }
    openFile(p);
  });
  function addFile(path, text) {
    project.files[path] = text;
    markDirty();
    openFile(path);
  }
  $('f-add').addEventListener('click', () => {
    const name = (prompt('New file name (.s for assembly, .bas for BASIC, .md for notes):', isBasic(project.main) ? 'more.bas' : 'more.s') || '').trim();
    if (!name) return;
    if (!okPath(name)) { toast('Use letters, digits, - _ . and / for folders.'); return; }
    if (name in project.files) { openFile(name); return; }
    addFile(name, /\.s$/i.test(name) ? `; ${name}\n` : '');
  });
  function renameFile(p) {
    const name = (prompt(`Rename ${p} to:`, p) || '').trim();
    if (!name || name === p) return;
    if (!okPath(name)) { toast('Use letters, digits, - _ . and / for folders.'); return; }
    if (name in project.files) { toast(`There's already a ${name}.`); return; }
    project.files[name] = project.files[p];
    delete project.files[p];
    if (project.main === p) project.main = name;
    tabs = tabs.map((t) => (t === p ? name : t));
    if (current === p) current = name;
    editor.rename(p, name);
    markDirty();
    renderFiles();
  }
  function deleteFile(p) {
    if (Object.keys(project.files).length < 2) { toast('A project needs one file. Start a new one with New.'); return; }
    if (!confirm(`Delete ${p} from the project?`)) return;
    delete project.files[p];
    editor.drop(p);
    if (project.main === p) project.main = Object.keys(project.files)[0];
    tabs = tabs.filter((t) => t !== p);
    if (!tabs.length) tabs = [project.main];
    markDirty();
    if (current === p) openFile(tabs[0]);
    else renderFiles();
  }

  // the title: click to rename
  $('ptitle').addEventListener('click', () => {
    const inp = $('ptitle-in');
    inp.value = project.title;
    $('ptitle').hidden = true;
    inp.hidden = false;
    inp.focus();
    inp.select();
  });
  const endRename = (keep) => {
    const inp = $('ptitle-in');
    if (inp.hidden) return;
    const t = inp.value.trim();
    if (keep && t && t !== project.title) { project.title = t; markDirty(); }
    inp.hidden = true;
    $('ptitle').hidden = false;
    showTitle();
  };
  $('ptitle-in').addEventListener('keydown', (e) => { if (e.key === 'Enter') endRename(true); if (e.key === 'Escape') endRename(false); });
  $('ptitle-in').addEventListener('blur', () => endRename(true));

  // ---------- the editor ----------
  // The textarea until editor.js swaps in CodeMirror through UNCRoom.useEditor().
  let editor = (() => {
    let path = null;
    return {
      reset() {},
      show(p, text) { path = p; $('src').value = text; },
      get: () => $('src').value,
      rename(a, b) { if (path === a) path = b; },
      drop() {},
      selectLine(n) {
        const ta = $('src');
        const lines = ta.value.split('\n');
        const start = lines.slice(0, n - 1).reduce((s, l) => s + l.length + 1, 0);
        ta.focus();
        ta.setSelectionRange(start, start + (lines[n - 1]?.length ?? 0));
      },
    };
  })();
  $('src').addEventListener('input', () => { if (current) { project.files[current] = $('src').value; markDirty(); } });
  $('src').addEventListener('keydown', (e) => {
    if (e.key === 'Tab') { e.preventDefault(); document.execCommand('insertText', false, '\t'); }
  });
  function goTo(path, line) {
    if (!(path in project.files)) return;
    openFile(path);
    editor.selectLine(line);
  }

  // ---------- the dock: problems, debugger, output ----------
  function showPane(name) {
    document.querySelectorAll('.dock-tabs [data-pane]').forEach((b) => b.setAttribute('aria-selected', String(b.dataset.pane === name)));
    ['problems', 'debugger', 'output'].forEach((n) => { $(`pane-${n}`).hidden = n !== name; });
    if (name === 'debugger') updateDebugger(true);
  }
  document.querySelectorAll('.dock-tabs [data-pane]').forEach((b) => b.addEventListener('click', () => showPane(b.dataset.pane)));
  function clearProblems() { showProblems([]); }
  function showProblems(errs) {
    const ul = $('errors');
    ul.innerHTML = '';
    errs.forEach(([line, msg, file]) => {
      const li = document.createElement('li');
      li.innerHTML = `<span class="at">${esc(file || project.main)}:${line}</span>${esc(msg)}`;
      li.addEventListener('click', () => goTo(file || project.main, line));
      ul.appendChild(li);
    });
    $('noerr').hidden = errs.length > 0;
    $('nprob').textContent = errs.length ? String(errs.length) : '';
  }
  function showOutput(res, main) {
    if (!res.ok) { $('output').textContent = `${main}: ${res.errors.length} error${res.errors.length > 1 ? 's' : ''}. See Problems.`; return; }
    const segs = res.segments || [[res.start, res.size]];
    const syms = [...res.symbols].sort((a, b) => a[1] - b[1]);
    const files = [...new Set(res.lines.map((l) => l[3]))];
    $('output').textContent = [
      `${main} assembled: ${res.size.toLocaleString('en-US')} bytes${files.length > 1 ? `, from ${files.join(', ')}` : ''}.`,
      ...segs.map(([s, n]) => `  $${hex(s, 4)}-$${hex(s + n - 1, 4)}  ${n.toLocaleString('en-US')} bytes`),
      `Starts at $${hex(res.start, 4)}.`,
      '',
      `${syms.length} symbol${syms.length === 1 ? '' : 's'}:`,
      ...syms.map(([n, v]) => `  $${hex(v & 0xFFFF, 4)}  ${n}`),
    ].join('\n');
  }

  // ---------- the debugger ----------
  let running = true;
  let lastStop = null;
  let memAddr = 0x0800;
  let prevMem = null;
  const regNames = ['A', 'X', 'Y', 'SP', 'PC'];
  $('regs').innerHTML = regNames.map((n) => `<div class="reg"><b>${n}</b><span id="r-${n}">--</span></div>`).join('')
    + '<div class="reg"><b>P</b><span id="r-P">--</span></div>';
  const FLAGS = [['N', 0x80], ['V', 0x40], ['-', 0x20], ['B', 0x10], ['D', 0x08], ['I', 0x04], ['Z', 0x02], ['C', 0x01]];
  $('flags').innerHTML = FLAGS.map(([n]) => `<span title="${n}">${n}</span>`).join('');
  const symbolAt = (addr) => program?.symbols.find(([, v]) => v === addr)?.[0];
  const lineAt = (addr) => program?.lines.find(([, a]) => a === addr);
  function disasmStart(pc) {
    if (program) {
      const addrs = program.lines.map(([, a]) => a);
      let i = -1;
      addrs.forEach((a, k) => { if (a <= pc) i = k; });
      if (i >= 0 && pc - addrs[i] < 64) return addrs[Math.max(0, i - 4)];
    }
    return pc;
  }
  function setStatus() {
    const st = wasm.cpu_state();
    const s = $('status');
    if (st === 2) { s.textContent = `JAMMED ON $${hex(wasm.jam_opcode(), 2)}`; s.className = 'bad'; }
    else if (st === 1) { s.textContent = 'HALTED'; s.className = 'stop'; }
    else if (running) { s.textContent = 'RUNNING'; s.className = 'run'; }
    else { s.textContent = lastStop === 1 ? `BREAK AT $${hex(wasm.reg(5), 4)}` : 'PAUSED'; s.className = 'stop'; }
    $('run').innerHTML = running ? 'Pause <kbd>F8</kbd>' : 'Run <kbd>F8</kbd>';
  }
  function updateDebugger(force) {
    setStatus();
    if ($('pane-debugger').hidden && !force) return;
    const st = wasm.cpu_state();
    const pc = wasm.reg(5);
    ['A', 'X', 'Y', 'SP'].forEach((n, i) => { $(`r-${n}`).textContent = '$' + hex(wasm.reg(i), 2); });
    $('r-PC').textContent = '$' + hex(pc, 4);
    const p = wasm.reg(4);
    $('r-P').textContent = '$' + hex(p, 2);
    [...$('flags').children].forEach((el, i) => el.classList.toggle('on', (p & FLAGS[i][1]) !== 0));
    const line = lineAt(pc);
    $('meta').innerHTML = `<span>Cycles <b>${Math.round(wasm.cycles()).toLocaleString('en-US')}</b></span>`
      + `<span>Raster <b>${wasm.raster_line()}</b></span><span>RAM <b>${wasm.ram_kb()}K</b>${wasm.has_video_board && wasm.has_video_board() ? ' + <b>BOARD</b>' : ''}</span>`
      + (line ? `<span>Source <b>${esc(line[3] || '')}:${line[0]}</b></span>` : '');
    if (running && st === 0 && !force) return;
    const len = wasm.disassemble(disasmStart(pc), 18);
    $('dis').innerHTML = readOut(wasm, len).trim().split('\n').map((l) => {
      const [a, bytes, text] = l.split('\t');
      const addr = Number(a);
      const cls = [addr === pc ? 'pc' : '', wasm.breakpoint(addr) ? 'has-bp' : ''].join(' ');
      return `<tr class="${cls}" data-addr="${addr}"><td class="bp">●</td><td class="addr">${hex(addr, 4)}</td>`
        + `<td class="bytes">${bytes}</td><td class="sym">${esc(symbolAt(addr) ?? '')}</td><td>${esc(text)}</td></tr>`;
    }).join('');
    updateMemory();
    const sp = wasm.reg(3);
    let st8 = '';
    for (let i = 1; i <= 10 && sp + i <= 0xFF; i++) st8 += `$01${hex(sp + i, 2)}  ${hex(wasm.peek(0x100 + sp + i), 2)}\n`;
    $('stack').textContent = st8 || '(empty)';
  }
  function updateMemory() {
    const now = [];
    let html = '';
    for (let r = 0; r < 16; r++) {
      const base = (memAddr + r * 16) & 0xFFFF;
      let hx = '';
      let asc = '';
      for (let c = 0; c < 16; c++) {
        const v = wasm.peek((base + c) & 0xFFFF);
        now.push(v);
        const changed = prevMem && prevMem[r * 16 + c] !== v;
        hx += (changed ? `<span class="chg">${hex(v, 2)}</span>` : hex(v, 2)) + ' ';
        asc += v >= 32 && v < 127 ? esc(String.fromCharCode(v)) : '.';
      }
      html += `<span class="a">${hex(base, 4)}</span>  ${hx} ${asc}\n`;
    }
    prevMem = now;
    $('mem').innerHTML = html;
  }
  function setRunning(on) {
    running = on;
    lastStop = null;
    updateDebugger();
    revealPC();
  }
  function handleStop(code) {
    if (code !== 0) { running = false; lastStop = code; updateDebugger(true); if (code === 1) showPane('debugger'); revealPC(); }
  }
  const doStep = () => { running = false; wasm.step(); lastStop = null; showPane('debugger'); render(performance.now()); revealPC(); };
  const doOver = () => { running = false; lastStop = wasm.step_over(); showPane('debugger'); render(performance.now()); revealPC(); };

  // ---------- breakpoints in the source ----------
  // project.breakpoints: { path: [line, ...] }, the dots in the editor's gutter. After an
  // assembly each one becomes the address of its line's instruction; a dot on a line with no
  // code (a comment, a label alone, a blank) moves to the next line in that file that has
  // some. Breakpoints set in the disassembly on a line of your source are the same dots.
  let srcAddrs = new Set(); // the addresses the dots are set at, in the machine
  const bpsOf = (path) => ((project.breakpoints ||= {})[path] || []);
  function resolveLine(path, line) {
    let best = null;
    for (const [l, a, len, f] of program?.lines || []) if (f === path && len > 0 && l >= line && (!best || l < best[0])) best = [l, a];
    return best;
  }
  function applyBreakpoints(snap) {
    srcAddrs.forEach((a) => wasm.set_breakpoint(a, 0));
    srcAddrs = new Set();
    if (!program) return;
    for (const path of Object.keys(project.breakpoints || {})) {
      const lines = [];
      for (const line of bpsOf(path)) {
        const r = resolveLine(path, line);
        if (!r) { lines.push(line); continue; } // after the last code: nothing to stop at yet
        lines.push(r[0]);
        srcAddrs.add(r[1]);
        wasm.set_breakpoint(r[1], 1);
      }
      const moved = [...new Set(lines)].sort((a, b) => a - b);
      if (snap && JSON.stringify(moved) !== JSON.stringify(bpsOf(path))) {
        project.breakpoints[path] = moved;
        editor.setBreakpoints?.(path, moved);
      }
    }
    if (!$('pane-debugger').hidden) updateDebugger(true);
  }
  function setSourceBreakpoints(path, lines) {
    (project.breakpoints ||= {})[path] = [...lines].sort((a, b) => a - b);
    saveDraft();
    applyBreakpoints(false);
  }
  function toggleSourceBreakpoint(path, line) {
    const now = bpsOf(path);
    const next = now.includes(line) ? now.filter((n) => n !== line) : [...now, line];
    editor.setBreakpoints?.(path, next);
    setSourceBreakpoints(path, next);
  }
  // Where the machine stopped, in the source: the file opened, the line lit.
  function revealPC() {
    if (!editor?.showPC) return;
    const l = !running && program ? lineAt(wasm.reg(5)) : null;
    if (!l) { editor.showPC(null, 0); return; }
    if (l[3] !== current && l[3] in project.files) openFile(l[3]);
    editor.showPC(l[3], l[0]);
  }
  $('run').addEventListener('click', () => setRunning(!running));
  $('step').addEventListener('click', doStep);
  $('over').addEventListener('click', doOver);
  $('dis').addEventListener('click', (e) => {
    const tr = e.target.closest('tr');
    if (!tr) return;
    const a = Number(tr.dataset.addr);
    const l = lineAt(a);
    if (l && l[3] in project.files) toggleSourceBreakpoint(l[3], l[0]); // a line of yours: its dot
    else wasm.set_breakpoint(a, wasm.breakpoint(a) ? 0 : 1);
    updateDebugger(true);
  });
  const setMem = (v) => { memAddr = parseInt(v, 16) & 0xFFFF; $('memaddr').value = hex(memAddr, 4); prevMem = null; updateMemory(); };
  $('memaddr').addEventListener('change', (e) => setMem(e.target.value || '0'));
  document.querySelectorAll('[data-mem]').forEach((b) => b.addEventListener('click', () => setMem(b.dataset.mem)));

  // ---------- the machine: power, RAM, the video board ----------
  function powerOn() {
    // the deck isn't part of the computer: its tape, and anything recorded on it, stays in
    const kept = deck ? deckBytes() : null;
    if (deck && kept) deck.cassette.tape = kept;
    srcAddrs.forEach((a) => wasm.set_breakpoint(a, 0)); // the dots go back in after the next Run
    srcAddrs = new Set();
    wasm.set_video_board($('board').checked ? 1 : 0);
    wasm.power_on(Number($('ram').value));
    wasm.set_tape_turbo?.($('turbo').checked ? 1 : 0);
    typing = [];
    pendingRun = false;
    program = null;
    recording = false;
    setRunning(true);
    reinsertTape();
  }
  function setMachine(ram, board, power = true) {
    const changed = Number($('ram').value) !== ram || $('board').checked !== !!board;
    $('ram').value = String(ram === 64 ? 64 : 16);
    $('board').checked = !!board;
    if (power && changed) powerOn();
  }
  $('reset').addEventListener('click', powerOn);
  // RESET, the button on the monitor: the processor and the I/O start over, memory is kept,
  // and the ROM goes back to READY. with the program. Out of anything, a jammed processor
  // too. Not a power cycle: what's in memory, the breakpoints and the tape all stay.
  $('resetbtn').addEventListener('click', () => {
    if (!wasm.reset_button) return;
    typing = [];
    pendingRun = false;
    wasm.reset_button();
    setRunning(true);
    focusScreen();
  });
  $('ram').addEventListener('change', () => { project.machine.ram = Number($('ram').value); markDirty(); powerOn(); });
  $('board').addEventListener('change', () => { project.machine.board = $('board').checked; markDirty(); powerOn(); });
  $('colour').checked = colour;
  $('colour').addEventListener('change', (e) => { colour = e.target.checked; pref.set('colour', colour ? '1' : '0'); faces.clear(); });
  const phSel = $('phosphor');
  phSel.innerHTML = PH.list.map((p) => `<option value="${p.id}">${p.name} (${p.note})</option>`).join('');
  phSel.value = phos.id;
  phSel.addEventListener('change', () => PH.set(phSel.value));
  PH.onChange((p) => { phos = p; MONO = PH.mono(PALETTE); phSel.value = p.id; faces.clear(); });
  $('mute').checked = sound.muted;
  $('mute').addEventListener('change', (e) => sound.setMuted(e.target.checked));

  // ---------- Run ----------
  let typing = []; // keys still to type into the machine
  let afterTyping = null;
  let pendingRun = false; // RUN once a BASIC program has come off the tape
  function run() {
    endRename(true);
    project.files[current] = editor.get();
    showPane('problems');
    if (isBasic(project.main)) return runBasic();
    powerOn();
    const res = assemble(wasm, project.files, project.main);
    showProblems(res.ok ? [] : res.errors);
    showOutput(res, project.main);
    if (!res.ok) {
      program = null;
      setRunning(false);
      $('status').textContent = `${res.errors.length} ERROR${res.errors.length > 1 ? 'S' : ''}`;
      $('status').className = 'bad';
      const [line, , file] = res.errors[0];
      goTo(file, line);
      return;
    }
    program = res;
    applyBreakpoints(true);
    // a breakpoint on the very first instruction stops it before it starts (running would
    // step off it: a breakpoint stops the machine before an instruction it arrives at)
    if (wasm.breakpoint(wasm.reg(5))) { running = false; lastStop = 1; showPane('debugger'); updateDebugger(true); revealPC(); return; }
    setRunning(true);
    focusScreen();
  }
  // BASIC: switched on, the listing typed in for you (fast), then RUN.
  function runBasic() {
    powerOn();
    const lines = listingLines(project.files[project.main] || '');
    showProblems([]);
    $('output').textContent = `${project.main}: ${lines.length} line${lines.length === 1 ? '' : 's'} typed into the machine, then RUN.\nBASIC finds its own errors: they show on the screen as ?SYNTAX ERROR IN 20, and so on.`;
    typing = keysOf(lines.join('\r') + '\rRUN\r');
    afterTyping = null;
    focusScreen();
  }
  $('b-run').addEventListener('click', run);

  // ---------- the tape deck (manual chapter 14) ----------
  // deck: { cassette, face (url), recorded: whether RECORD went down }
  let deck = null;
  let recording = false;
  function insertTape(cas, face) {
    if (deck) ejectTape(true);
    deck = { cassette: cas, face, recorded: false };
    reinsertTape();
    $('slot-face').src = face || '';
    $('slot-face').hidden = !face;
    $('slot-empty').hidden = !!face;
    const progs = C.programs(cas.tape).filter((p) => p.name !== undefined);
    $('deck-name').innerHTML = `<b class="t">${esc(cas.title)}</b>${progs.length ? ` · ${progs.map((p) => esc(p.name)).join(', ')}` : ''} · ${cas.tape.length.toLocaleString('en-US')} bytes`;
    updateDeck(true);
  }
  // The machine was switched off and on (a new machine, an empty deck): the tape goes back in,
  // rewound. A blank tape with nothing on it yet is a blank tape again.
  function reinsertTape() {
    if (!deck) return;
    if (wasm.tape_length && wasm.tape_length() === deck.cassette.tape.length && wasm.tape_status() & 2) return;
    if (!deck.cassette.tape.length && has('blank')) { TAPEX.blank(wasm)(); return; }
    writeIn(wasm, deck.cassette.tape);
    wasm.insert_tape_input();
  }
  function ejectTape(quiet) {
    if (!deck) return;
    // STOP/EJECT gives the tape back, with whatever SAVE recorded on it
    const n = wasm.eject_tape ? wasm.eject_tape() : -1;
    const bytes = typeof n === 'number' && n > 0 ? readBytes(wasm, n) : null;
    const before = deck.cassette.tape;
    const changed = bytes && (bytes.length !== before.length || bytes.some((v, i) => v !== before[i]));
    if (changed && C.isTape(bytes) && !quiet) offerRecording(bytes);
    deck = null;
    recording = false;
    pendingRun = false;
    $('slot-face').hidden = true;
    $('slot-empty').hidden = false;
    $('deck-name').innerHTML = 'Put a cassette in: Run one from Browse, or press <b>Tape it</b>.';
    if (!quiet) updateDeck(true);
  }
  // A tape with a SAVE on it: keep it as a cassette in the library, with a blank tape's face.
  function offerRecording(bytes) {
    const c = C.read(bytes);
    const names = C.programs(bytes).map((p) => p.name).filter(Boolean);
    c.title = names[names.length - 1] || 'RECORDING';
    lastRecording = c;
    toast(`The tape has ${names.join(', ') || 'a recording'} on it.`, [
      ['Keep it as a cassette', () => keepRecording(c)],
    ]);
  }
  let lastRecording = null;
  async function keepRecording(c) {
    const face = await faceFor({ title: c.title, blank: true });
    const id = await LIB.yours.put(await C.toPNG(c, face));
    toast(`${c.title} is in your library.`, [['Browse', () => showBrowse(true)]]);
    return id;
  }
  $('d-play').addEventListener('click', () => { if (!deck) { toast('No tape in the deck.'); return; } wasm.press_play(); updateDeck(true); });
  $('d-stop').addEventListener('click', () => { wasm.press_stop(); updateDeck(true); });
  $('d-rew').addEventListener('click', () => { wasm.rewind_tape?.(); updateDeck(true); });
  $('d-eject').addEventListener('click', () => ejectTape());
  $('turbo').checked = pref.get('turbo', '1') === '1';
  $('turbo').addEventListener('change', () => { pref.set('turbo', $('turbo').checked ? '1' : '0'); wasm.set_tape_turbo?.($('turbo').checked ? 1 : 0); });
  if (!wasm.set_tape_turbo) $('turbo').closest('label').hidden = true;
  // RECORD and blank tapes come with BASIC on tape (SAVE); shown when the machine has them.
  $('d-rec').hidden = !has('record');
  $('d-blank').hidden = !has('blank');
  $('d-rec').addEventListener('click', () => {
    if (!deck) { toast('Put a tape in first: Blank tape.'); return; }
    if (TAPEX.record(wasm)()) deck.recorded = true;
    updateDeck(true);
  });
  $('d-blank').addEventListener('click', async () => {
    if (deck) ejectTape();
    TAPEX.blank(wasm)();
    const face = await blankFace('BLANK TAPE');
    deck = { cassette: { title: 'BLANK TAPE', tape: new Uint8Array(0) }, face, recorded: false, blank: true };
    $('slot-face').src = face;
    $('slot-face').hidden = false;
    $('slot-empty').hidden = true;
    $('deck-name').innerHTML = '<b class="t">A blank tape</b> · for <code>SAVE</code>';
    updateDeck(true);
  });
  const blankFaces = new Map();
  async function blankFace(title) {
    if (!blankFaces.has(title)) {
      const cvs = await faceFor({ title, blank: true });
      blankFaces.set(title, await new Promise((r) => cvs.toBlob((b) => r(URL.createObjectURL(b)))));
    }
    return blankFaces.get(title);
  }
  // Tape it: this project, built and put on a tape in the deck, to LOAD the real way.
  $('d-tapeit').addEventListener('click', async () => {
    project.files[current] = editor.get();
    const t = buildTape(project.files, project.main, project.title, project.machine);
    if (!t.ok) { showProblems(t.errors || []); toast(t.message); return; }
    const cas = { title: project.title, tape: t.tape };
    insertTape(cas, await blankFace(tapeName(project.title)));
    toast(`${tapeName(project.title)} is on tape, in the deck. Type LOAD at READY. and press Play.`);
  });
  let deckShown = '';
  function updateDeck(force) {
    const st = wasm.tape_status ? wasm.tape_status() : 0;
    const pos = wasm.tape_counter ? wasm.tape_counter() : 0;
    const len = wasm.tape_length ? wasm.tape_length() : 0;
    recording = has('recording') ? TAPEX.recording(wasm)() === 1 : false;
    const key = `${!!deck}|${st}|${pos}|${len}|${recording}`;
    if (key === deckShown && !force) return;
    deckShown = key;
    const play = (st & 1) !== 0;
    const motor = (st & 4) !== 0;
    $('counter').textContent = String(pos % 1000).padStart(3, '0');
    $('counter').title = `${pos.toLocaleString('en-US')} of ${len.toLocaleString('en-US')} bytes`;
    $('motor').classList.toggle('on', motor);
    $('playing').classList.toggle('on', play);
    $('recording').hidden = !has('record');
    $('recording').classList.toggle('on', recording && play);
    $('d-play').classList.toggle('down', play);
    $('d-rec').classList.toggle('down', recording && play);
    ['d-play', 'd-stop', 'd-rew', 'd-eject', 'd-rec'].forEach((id) => { $(id).disabled = !deck; });
    let hint = '';
    if (deck && !deck.blank) {
      if (len && pos >= len) hint = 'At the end of the tape. Rewind (◀◀) to load it again.';
      else if (!play) hint = 'Type LOAD at READY. and press Play.';
      else if (!motor) hint = pos ? 'Stopped by LOAD. It goes on by itself.' : 'PLAY is down. The tape moves when LOAD runs the motor.';
      else hint = $('turbo').checked ? 'Loading, fast.' : 'Loading: about 600 bytes a second.';
    } else if (deck) {
      if (recording && motor) hint = 'Recording.';
      else if (len) hint = 'Recorded. Eject it to keep the recording as a cassette.';
      else if (recording) hint = 'Rec is down. SAVE runs the motor.';
      else hint = 'Type SAVE "NAME" at READY. and press Rec.';
    }
    $('deck-hint').textContent = hint;
  }

  // ---------- the bundled cassettes ----------
  // The games (games/), the Machine Room's samples and the magazine's BASIC starter, built
  // in as source (bundled.js). Their tapes are built and their pictures taken on the bench,
  // the first time they're needed.
  const STYLE = {
    snake: { shell: '#1c1c1e', face: '#0d130d', ink: '#7fe39a' },
    stack: { shell: '#1c1c1e', face: '#2a1f4a', ink: '#e9c04e' },
    orbit: { shell: '#d8d4ca', face: '#090b1c', ink: '#c8e0ff' },
    debug: { shell: '#1c1c1e', face: '#e9c04e', ink: '#1a1410' },
    aurora: { shell: '#1d2440', face: '#0b1030', ink: '#9fe08a' },
    Samples: { shell: '#cfc7b4', face: '#2b2018', ink: '#efe6cf' },
    BASIC: { shell: '#cfc7b4', face: '#2b3a6a', ink: '#f4f1e8' },
  };
  const AUTHOR = { aurora: 'United Northern Computers', design_starter: '8-Bit Monthly' };
  const faces = new Map(); // key -> Promise<url>
  let shots = Promise.resolve();
  const bundledCas = (b) => ({
    title: b.title, author: AUTHOR[b.id] || '', about: b.about, machine: { ram: b.ram, board: b.board },
    files: { [b.file]: b.source }, main: b.file, extra: {}, created: null,
  });
  const bundled = {
    id: 'bundled',
    name: 'Bundled',
    note: 'The games from the uncle\'s stand and the parcel, the Machine Room\'s samples, and BASIC to type in. Open one to read how it\'s done.',
    async list() {
      return window.UNC_BUNDLED.map((b) => ({
        id: b.id, group: b.group, title: b.title, author: AUTHOR[b.id] || '', about: b.about,
        machine: { ram: b.ram, board: b.board }, hasFiles: true, kind: /\.bas$/.test(b.file) ? 'basic' : 'asm',
        face: () => bundledFace(b),
      }));
    },
    async get(id) {
      const b = sampleOf(id);
      if (!b) throw new Error('no such cassette');
      const c = bundledCas(b);
      const t = buildTape(c.files, c.main, c.title, c.machine);
      if (!t.ok) throw new Error(t.message);
      c.tape = t.tape;
      return c;
    },
  };
  function bundledFace(b) {
    const key = `${b.id}|${colour}|${phos.id}`;
    if (!faces.has(key)) {
      // one at a time, each in its own turn of the event loop, so the page stays live
      shots = shots.then(() => new Promise((r) => setTimeout(r, 0))).then(async () => {
        const c = bundledCas(b);
        const screen = shoot(c.files, c.main, c.machine);
        const cvs = await faceFor({ ...c, screen, style: STYLE[b.id] || STYLE[b.group] });
        return new Promise((r) => cvs.toBlob((bl) => r(URL.createObjectURL(bl))));
      });
      faces.set(key, shots);
    }
    return faces.get(key);
  }
  const SOURCES = [bundled, LIB.yours, LIB.online];

  // ---------- Browse ----------
  const host = {
    close: () => showBrowse(false),
    async run(src, id) {
      const c = await src.get(id);
      const face = await faceOf(src, id, c);
      showBrowse(false);
      setMachine(c.machine.ram, c.machine.board, false);
      powerOn();
      insertTape(c, face);
      wasm.set_tape_turbo?.($('turbo').checked ? 1 : 0);
      // LOAD, typed at READY., then PLAY
      typing = keysOf('LOAD\r');
      // LOAD starts a machine-language program; a BASIC one it leaves at READY., so RUN it
      const basic = C.programs(c.tape)[0]?.type === 2;
      afterTyping = () => { wasm.press_play(); pendingRun = basic; updateDeck(true); };
      focusScreen();
      const warn = c.machine.board && !wasm.set_video_board ? ' It needs the video board.' : '';
      toast(`${c.title} is in the deck: LOAD, PLAY.${warn}`);
    },
    async open(src, id) {
      const c = await src.get(id).catch(() => null) || (src === bundled ? bundledCas(sampleOf(id)) : null);
      if (!c || !c.files) { toast('No source on this cassette.'); return; }
      if (dirty && !confirm(`Open ${c.title}? Your changes to ${project.title} haven't been saved.`)) return;
      setProject(fromCassette(c, src.writable ? { src: src.id, id } : null));
      showBrowse(false);
      toast(`${c.title} is open. Run it with Ctrl+Enter.`);
    },
    async download(src, id) {
      let png;
      if (src === LIB.yours) png = (await LIB.store.get(id)).png;
      else {
        const c = await src.get(id);
        const b = sampleOf(id);
        const face = await faceFor({ ...c, screen: shoot(c.files, c.main, c.machine), style: b && (STYLE[b.id] || STYLE[b.group]) });
        png = await C.toPNG(c, face);
      }
      const c = C.read(png);
      downloadBytes(png, C.fileName(c.title, '.unc.png'), 'image/png');
      toast(`Downloaded. ${GAME_HINT}`);
    },
    async duplicate(src, id) {
      const r = await LIB.store.get(id);
      await LIB.yours.put(r.png);
      toast('Duplicated.');
    },
    confirm: (t) => confirm(t),
    toast: (t) => toast(t),
  };
  const browse = LIB.view($('browse'), SOURCES, host);
  async function faceOf(src, id, c) {
    if (src === bundled) return bundledFace(sampleOf(id));
    if (src === LIB.yours) return (await LIB.yours.list()).find((e) => e.id === id)?.face();
    if (c.png) return URL.createObjectURL(new Blob([c.png], { type: 'image/png' }));
    return blankFace(c.title);
  }
  let browsing = false;
  function showBrowse(on) {
    browsing = on;
    $('browse').hidden = !on;
    $('ide').hidden = on;
    $('b-browse').classList.toggle('go', on);
    if (on) { browse.render(); $('br-q')?.focus(); }
  }
  $('b-browse').addEventListener('click', () => showBrowse(!browsing));
  const GAME_HINT = 'To play it in the game: turn on My tapes in the game\'s pause menu (Settings, Assembler), then put the cassette in Documents/UNC 64/Cassettes, or drop it on the game\'s window.';

  // ---------- Save and Export: the cassette dialog ----------
  const dlg = $('dlg-save');
  let shot = null; // the screen when the dialog opened
  function cassetteFromDialog() {
    return {
      title: $('s-title').value.trim() || 'UNTITLED',
      author: $('s-author').value.trim(),
      about: $('s-about').value.trim(),
      machine: { ram: Number(dlg.querySelector('[name=s-ram]:checked').value), board: $('s-board').checked },
      files: $('s-src').checked ? { ...project.files } : null,
      main: $('s-src').checked ? project.main : null,
      extra: { ...project.extra },
      created: null,
    };
  }
  async function drawPreview() {
    const c = cassetteFromDialog();
    $('s-tapename').textContent = tapeName(c.title);
    await C.fontsReady();
    C.drawFace($('save-face'), { ...c, screen: shot });
  }
  function openSave(mode) {
    endRename(true);
    project.files[current] = editor.get();
    shot = picture(wasm);
    $('s-title').value = project.title === 'UNTITLED' ? '' : project.title;
    $('s-author').value = project.author || pref.get('author', '');
    $('s-about').value = project.about || '';
    dlg.querySelector(`[name=s-ram][value="${project.machine.ram === 64 ? 64 : 16}"]`).checked = true;
    $('s-board').checked = !!project.machine.board;
    $('s-err').textContent = '';
    $('s-lib').classList.toggle('go', mode === 'save');
    $('s-png').classList.toggle('go', mode === 'export');
    $('h-save').textContent = mode === 'export' ? 'Export a cassette' : 'Save a cassette';
    drawPreview();
    dlg.showModal();
    (mode === 'export' ? $('s-png') : $('s-title')).focus();
    if (!$('s-title').value) $('s-title').focus();
  }
  ['s-title', 's-author', 's-about', 's-board', 's-src'].forEach((id) => $(id).addEventListener('input', drawPreview));
  dlg.querySelectorAll('[name=s-ram]').forEach((r) => r.addEventListener('change', drawPreview));
  // The dialog's cassette, with its tape: null (and the reason shown) if it doesn't build.
  async function makeCassette() {
    const c = cassetteFromDialog();
    if (!$('s-title').value.trim()) { $('s-err').textContent = 'Give it a title.'; $('s-title').focus(); return null; }
    const t = buildTape(project.files, project.main, c.title, c.machine);
    if (!t.ok) { $('s-err').textContent = t.message; showProblems(t.errors || []); return null; }
    c.tape = t.tape;
    project.title = c.title;
    project.author = c.author;
    project.about = c.about;
    project.machine = { ...c.machine };
    if (c.author) pref.set('author', c.author);
    showTitle();
    return c;
  }
  async function cassettePNG(c) {
    await C.fontsReady();
    return C.toPNG(c, C.drawFace(document.createElement('canvas'), { ...c, screen: shot }));
  }
  $('s-lib').addEventListener('click', async () => {
    const c = await makeCassette();
    if (!c) return;
    const png = await cassettePNG(c);
    const id = await LIB.yours.put(png, project.lib?.src === 'yours' ? project.lib.id : undefined);
    project.lib = { src: 'yours', id };
    dirty = false;
    showTitle();
    saveDraft();
    dlg.close();
    toast(`Saved to your library. Download it to keep it as a file. ${GAME_HINT}`, [['Download', () => downloadBytes(png, C.fileName(c.title, '.unc.png'), 'image/png')]]);
  });
  $('s-png').addEventListener('click', async () => {
    const c = await makeCassette();
    if (!c) return;
    downloadBytes(await cassettePNG(c), C.fileName(c.title, '.unc.png'), 'image/png');
    dlg.close();
    toast(`Downloaded ${C.fileName(c.title, '.unc.png')}. ${GAME_HINT}`);
  });
  $('s-tap').addEventListener('click', async () => {
    const c = await makeCassette();
    if (!c) return;
    downloadBytes(c.tape, C.fileName(c.title, '.tap'), 'application/octet-stream');
    dlg.close();
    toast(`Downloaded ${C.fileName(c.title, '.tap')}. ${GAME_HINT}`);
  });
  $('b-save').addEventListener('click', () => openSave('save'));
  $('b-export').addEventListener('click', () => openSave('export'));
  function downloadBytes(bytes, name, type) {
    const url = URL.createObjectURL(new Blob([bytes], { type }));
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
  }

  // ---------- New, Open, drop ----------
  $('b-new').addEventListener('click', () => {
    toast('A new project:', [
      ['Assembly', () => startNew('asm')],
      ['BASIC', () => startNew('basic')],
    ]);
  });
  function startNew(kind) {
    if (dirty && !confirm(`Start a new project? Your changes to ${project.title} haven't been saved.`)) return;
    setProject(newProject(kind));
    hideToast();
  }
  $('b-open').addEventListener('click', () => $('file-open').click());
  $('file-open').addEventListener('change', async (e) => {
    const files = [...e.target.files];
    e.target.value = '';
    if (files.length === 1) await openLocal(files[0]);
    else for (const f of files) await bringIn(f);
  });
  const isText = (name) => /\.(s|asm|inc|bas|txt|md)$/i.test(name);
  // Open: a source file becomes a new project; a cassette opens (its project, and its
  // tape in the deck).
  async function openLocal(file) {
    try {
      if (isText(file.name)) {
        if (dirty && !confirm(`Open ${file.name}? Your changes to ${project.title} haven't been saved.`)) return;
        const name = file.name.replace(/[^\w.-]/g, '_');
        const p = newProject(isBasic(name) ? 'basic' : 'asm');
        p.files = { [name]: await file.text() };
        p.main = name;
        p.title = name.replace(/\.\w+$/, '').toUpperCase();
        setProject(p);
        return;
      }
      const c = C.read(new Uint8Array(await file.arrayBuffer()), file.name);
      const face = c.png ? URL.createObjectURL(new Blob([c.png], { type: 'image/png' })) : await blankFace(c.title);
      if (c.files) {
        if (dirty && !confirm(`Open ${c.title}? Your changes to ${project.title} haven't been saved.`)) return;
        setProject(fromCassette(c, null));
      } else setMachine(c.machine.ram, c.machine.board);
      insertTape(c, face);
      toast(`${c.title}${c.files ? ' is open, and' : ' has no source:'} its tape is in the deck. Type LOAD and press Play.`);
    } catch (err) {
      toast(err.message);
    }
  }
  // Drop (or several files from Open): cassettes go into your library; sources join the project.
  async function bringIn(file) {
    try {
      if (isText(file.name)) {
        const name = file.name.replace(/[^\w.-]/g, '_');
        if (name in project.files && !confirm(`Replace ${name} in the project?`)) return;
        addFile(name, await file.text());
        toast(`${name} is in the project.`);
        return;
      }
      const c = C.read(new Uint8Array(await file.arrayBuffer()), file.name);
      let png = c.png;
      if (!png) png = await C.toPNG(c, await faceFor({ title: c.title, blank: true }));
      const id = await LIB.yours.put(png);
      toast(`${c.title} is in your library.`, [
        ['Run', () => host.run(LIB.yours, id).catch((err) => toast(err.message))],
        ...(c.files ? [['Open', () => host.open(LIB.yours, id).catch((err) => toast(err.message))]] : []),
      ]);
      if (browsing) browse.render();
    } catch (err) {
      toast(`${file.name}: ${err.message}`);
    }
  }
  let dragDepth = 0;
  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes('Files');
  document.addEventListener('dragenter', (e) => { if (!hasFiles(e)) return; e.preventDefault(); dragDepth++; $('drop').hidden = false; });
  document.addEventListener('dragover', (e) => { if (hasFiles(e)) e.preventDefault(); });
  document.addEventListener('dragleave', (e) => { if (!hasFiles(e)) return; dragDepth = Math.max(0, dragDepth - 1); if (!dragDepth) $('drop').hidden = true; });
  document.addEventListener('drop', async (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dragDepth = 0;
    $('drop').hidden = true;
    for (const f of e.dataTransfer.files) await bringIn(f);
  });

  // ---------- toast ----------
  let toastTimer = 0;
  function toast(text, actions = []) {
    const t = $('toast');
    t.innerHTML = `<span>${esc(text)}</span>`;
    actions.forEach(([label, fn]) => {
      const b = document.createElement('button');
      b.className = 'btn';
      b.type = 'button';
      b.textContent = label;
      b.addEventListener('click', () => { hideToast(); fn(); });
      t.appendChild(b);
    });
    t.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(hideToast, actions.length ? 12000 : 6000);
  }
  function hideToast() { $('toast').hidden = true; }

  // ---------- help, keys ----------
  $('b-help').addEventListener('click', () => $('dlg-help').showModal());
  cv.addEventListener('click', () => kb.focus({ preventScroll: true }));
  cv.addEventListener('focus', () => kb.focus({ preventScroll: true }));
  // The cursor keys' codes (manual chapter 11), and the keys that are down: e.code -> code.
  const CURSOR = { ArrowLeft: 28, ArrowRight: 29, ArrowUp: 30, ArrowDown: 31 };
  const held = new Map();
  kb.addEventListener('keydown', (e) => {
    if (e.ctrlKey && (e.key === 'c' || e.key === 'C')) { e.preventDefault(); held.set(e.code, 3); wasm.key(3); return; } // the STOP key
    if (e.metaKey || e.altKey || /^F\d+$/.test(e.key)) return;
    // RESTORE is Page Up: the processor's NMI. With STOP held (Ctrl+C) it's the way back to
    // READY. from anything; Shift+Page Up presses both.
    if (e.key === 'PageUp' && wasm.restore) {
      e.preventDefault();
      if (e.shiftKey) { wasm.key(3); setTimeout(() => wasm.key_up(3), 150); }
      wasm.restore();
      return;
    }
    if (e.ctrlKey) return;
    let code = CURSOR[e.key] ?? null;
    if (e.key === 'Enter') code = 13;
    else if (e.key === 'Backspace' && e.shiftKey) code = 20; // INSERT: shift and DELETE (not the host's Insert key: not every keyboard has one)
    else if (e.key === 'Backspace') code = 8;
    else if (e.key.length === 1) code = e.key.toUpperCase().charCodeAt(0);
    if (code === null || code >= 96) return;
    e.preventDefault();
    // behind anything still being typed (a phone's keyboard sends its text one way and its
    // RETURN another): the keys arrive in the order they were pressed
    if (typing.length) typing.push(code);
    else { held.set(e.code, code); wasm.key(code); }
  });
  // Key-up lets the key go, so HELD ($C012) sees it come up. Matched by the physical key.
  const release = (physical) => {
    const code = held.get(physical);
    if (code === undefined) return;
    held.delete(physical);
    if (![...held.values()].includes(code)) wasm.key_up(code);
  };
  kb.addEventListener('keyup', (e) => release(e.code));
  kb.addEventListener('blur', () => [...held.keys()].forEach(release));
  // A phone's keyboard often sends no usable keydown: what it types is taken from the field.
  // The field always holds one space, so there's something for the keyboard's delete key to
  // take: the space gone is DELETE.
  const KEEP = ' ';
  const refill = () => { kb.value = KEEP; try { kb.setSelectionRange(1, 1); } catch { /* not focused */ } };
  kb.addEventListener('input', () => {
    const v = kb.value;
    if (!v.startsWith(KEEP)) typing.push(8, ...keysOf(v));
    else typing.push(...keysOf(v.slice(1)));
    refill();
  });
  kb.addEventListener('focus', refill);
  // A paste goes in as typing, line breaks and all (a one-line field would drop them).
  kb.addEventListener('paste', (e) => {
    e.preventDefault();
    typing.push(...keysOf((e.clipboardData ? e.clipboardData.getData('text') : '').replace(/\r\n/g, '\n')));
  });
  // The keys a phone's keyboard doesn't have, under the monitor on a touch screen: the cursor
  // keys and STOP are down for as long as they're held, RESTORE is the NMI. They don't take
  // the focus, so the keyboard stays up.
  document.querySelectorAll('#softkeys [data-key]').forEach((b) => {
    const code = Number(b.dataset.key);
    const id = `soft${code}`;
    b.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      if (code === 0) { wasm.restore?.(); return; }
      held.set(id, code);
      wasm.key(code);
    });
    ['pointerup', 'pointercancel', 'pointerleave'].forEach((t) => b.addEventListener(t, () => release(id)));
    b.addEventListener('contextmenu', (e) => e.preventDefault());
  });
  // With the keyboard up there's less of the page: keep the screen in what's left.
  window.visualViewport?.addEventListener('resize', () => {
    if (document.activeElement === kb) cv.scrollIntoView({ block: 'nearest' });
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'F8') { e.preventDefault(); setRunning(!running); }
    else if (e.key === 'F7') { e.preventDefault(); doStep(); }
    else if (e.key === 'F6') { e.preventDefault(); doOver(); }
    else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.target.closest('.cm-editor') && !e.target.closest('dialog')) { e.preventDefault(); run(); }
    else if (e.key === 'Escape' && browsing && !document.querySelector('dialog[open]')) showBrowse(false);
  });

  // ---------- start ----------
  await document.fonts.load('16px UNC').catch(() => {});
  let draft = null;
  try { draft = await Promise.race([LIB.store.getState('draft'), new Promise((r) => setTimeout(r, 1500))]); } catch (e) { /* none */ }
  wasm.set_tape_turbo?.($('turbo').checked ? 1 : 0);
  if (draft && draft.project && draft.project.files && draft.project.main in draft.project.files) {
    setProject(draft.project, { dirty: draft.dirty, power: false });
    (draft.tabs || []).filter((p) => p in project.files).forEach((p) => { if (!tabs.includes(p)) tabs.push(p); });
    if (draft.current in project.files) openFile(draft.current);
  } else {
    const hello = sampleOf('hello');
    setProject({ ...newProject('asm'), title: 'HELLO', files: { 'hello.s': hello ? hello.source : NEW_ASM }, main: 'hello.s' }, { power: false });
  }
  powerOn(); // boot to UNC BASIC; your program waits in the editor until you Run it

  roomReady({
    module,
    bench, // the editor checks your source on the bench, never on the machine
    run,
    // the editor's view of the project
    useEditor(ed) { editor = ed; ed.show(current, project.files[current]); },
    changed(path, text) { if (project.files[path] !== text) { project.files[path] = text; markDirty(); } },
    files: () => project.files,
    main: () => project.main,
    check: (main) => assemble(bench, project.files, main || project.main),
    breakpoints: (path, lines) => setSourceBreakpoints(path, lines),
    isBasic,
  });
  // for tests: the pieces a script needs to drive it
  window.uncRoom = {
    get project() { return project; }, wasm, bench, C, LIB, buildTape, run, insertTape, picture, showBrowse, setProject,
    get typing() { return typing.length; },
    get deck() { return deck; },
    get lastRecording() { return lastRecording; },
    keepRecording, deckBytes, screenText: () => { let t = ''; for (let r = 0; r < 25; r++) { for (let c = 0; c < 40; c++) { const v = wasm.peek(0x400 + r * 40 + c) & 0x7F; t += v >= 0x20 && v <= 0x5F ? String.fromCharCode(v) : ' '; } t += '\n'; } return t; },
  };

  // ---------- main loop: 50 frames a second of machine time ----------
  // Each refresh runs the frames it owes, at most 2 (after a stall the machine drops time
  // rather than racing). Typing for you (BASIC's Run, LOAD) goes first, as fast as the
  // ROM takes the keys.
  const MAX_FRAMES = 2;
  let last = performance.now();
  let owed = 0;
  function loop(now) {
    if (typing.length && running) {
      const t0 = performance.now();
      while (typing.length && performance.now() - t0 < 12) feedKey(wasm, typing.shift());
      if (!typing.length && afterTyping) { const f = afterTyping; afterTyping = null; f(); }
      owed = 0;
    } else if (running) {
      owed = Math.min(owed + (now - last) / 20, MAX_FRAMES);
      for (let n = 0; n < MAX_FRAMES && owed >= 1 && running; n++) {
        owed -= 1;
        const stop = wasm.run_frame();
        if (stop === 0) sound.frame();
        handleStop(stop);
      }
    } else {
      owed = 0;
    }
    // the tape has run out and the motor is off: give LOAD a moment to say READY., then RUN
    if (pendingRun && deck && !typing.length && wasm.tape_counter() >= wasm.tape_length() && !(wasm.tape_status() & 5)) {
      pendingRun = pendingRun === true ? 1 : pendingRun + 1;
      if (pendingRun > 20) { pendingRun = false; typing = keysOf('RUN\r'); }
    }
    last = now;
    render(now);
    if (running) updateDebugger();
    updateDeck();
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);
})();
