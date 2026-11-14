// Cassettes: the UNC 64's shareable program file (docs/cassettes.md). A PNG of the
// cassette's face with the program in a private `unCt` chunk: the tape image (what loads)
// and, optionally, the project it was built from. Bare `.tap` files are read too.
//
//   UNCCassette.read(bytes, filename)  -> cassette, or throws an Error with a plain message
//   UNCCassette.toPNG(cassette, face)  -> Promise<Uint8Array>: face is a canvas
//   UNCCassette.drawFace(canvas, opts) -> draws the face (1004 x 638)
//   UNCCassette.programs(tape)         -> what's on a tape: [{ name, type, load, length, entry, ok }]
//
// A cassette in memory: { title, author, about, machine: { ram, board, niece }, tape: Uint8Array,
// files: { path: text } | null, main, created, extra: { unknown fields, kept }, png: the
// file's bytes when it came from one }.
window.UNCCassette = (() => {
  const FORMAT = 'unc64-cassette';
  const VERSION = 1;
  const CHUNK = 'unCt';
  const SIG = [137, 80, 78, 71, 13, 10, 26, 10];
  const KNOWN = ['format', 'version', 'title', 'author', 'about', 'machine', 'tape', 'files', 'main', 'created'];

  // ---------- CRC-32 (the PNG one: polynomial $EDB88320) ----------
  const TABLE = (() => {
    const t = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
      t[n] = c >>> 0;
    }
    return t;
  })();
  function crc32(bytes, from = 0, to = bytes.length) {
    let c = 0xFFFFFFFF;
    for (let i = from; i < to; i++) c = TABLE[(c ^ bytes[i]) & 0xFF] ^ (c >>> 8);
    return (c ^ 0xFFFFFFFF) >>> 0;
  }

  // ---------- base64 for bytes ----------
  function toBase64(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(s);
  }
  const fromBase64 = (b64) => Uint8Array.from(atob(b64.replace(/\s+/g, '')), (c) => c.charCodeAt(0));

  // ---------- PNG chunks ----------
  const isPNG = (b) => b.length >= 8 && SIG.every((v, i) => b[i] === v);
  const u32 = (b, i) => ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
  const ascii = (b, i, n) => String.fromCharCode(...b.subarray(i, i + n));
  // Every chunk: { type, at (its first byte), data (a view), crcOK }.
  function chunks(png) {
    if (!isPNG(png)) throw new Error('not a PNG');
    const out = [];
    let at = 8;
    while (at + 12 <= png.length) {
      const len = u32(png, at);
      const type = ascii(png, at + 4, 4);
      if (at + 12 + len > png.length) throw new Error('the PNG is cut short');
      const crcOK = crc32(png, at + 4, at + 8 + len) === u32(png, at + 8 + len);
      out.push({ type, at, end: at + 12 + len, data: png.subarray(at + 8, at + 8 + len), crcOK });
      at += 12 + len;
      if (type === 'IEND') break;
    }
    return out;
  }
  function chunk(type, data) {
    const out = new Uint8Array(12 + data.length);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, data.length);
    for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
    out.set(data, 8);
    dv.setUint32(8 + data.length, crc32(out, 4, 8 + data.length));
    return out;
  }
  // The PNG with one `type` chunk holding data, just before IEND (any old ones taken out).
  function withChunk(png, type, data) {
    const list = chunks(png);
    const iend = list.find((c) => c.type === 'IEND');
    if (!iend) throw new Error('the PNG has no end');
    const keep = list.filter((c) => c.type !== type && c.type !== 'IEND');
    const parts = [png.subarray(0, 8), ...keep.map((c) => png.subarray(c.at, c.end)), chunk(type, data), png.subarray(iend.at, iend.end)];
    const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
    let o = 0;
    for (const p of parts) { out.set(p, o); o += p.length; }
    return out;
  }

  // ---------- tapes (manual chapter 14) ----------
  // Every program on a tape, in order. A damaged tail is reported, not thrown.
  function programs(tape) {
    const out = [];
    let at = 0;
    while (at < tape.length) {
      if (ascii(tape, at, 4) !== 'UNCT') { out.push({ ok: false, error: `byte ${at}: no UNCT` }); break; }
      if (at + 28 > tape.length) { out.push({ ok: false, error: `byte ${at}: the tape ends inside a header` }); break; }
      const w = (i) => tape[at + i] | (tape[at + i + 1] << 8);
      const length = w(24);
      const end = at + 28 + length;
      if (end >= tape.length) { out.push({ ok: false, error: `byte ${at}: the tape ends inside a program` }); break; }
      let x = 0;
      for (let i = at + 4; i <= end; i++) x ^= tape[i];
      out.push({
        ok: x === 0, version: tape[at + 4], type: tape[at + 5], name: ascii(tape, at + 6, 16).trimEnd(),
        load: w(22), length, entry: w(26),
      });
      at = end + 1;
    }
    return out;
  }
  const isTape = (b) => b.length >= 5 && ascii(b, 0, 4) === 'UNCT';

  // ---------- reading ----------
  function fromJSON(j, png) {
    if (!j || j.format !== FORMAT) throw new Error('not a UNC 64 cassette');
    if (typeof j.version !== 'number' || j.version > VERSION) throw new Error(`cassette version ${j.version}: this Machine Room knows ${VERSION}. Get a newer one.`);
    if (typeof j.tape !== 'string') throw new Error('the cassette has no tape');
    const tape = fromBase64(j.tape);
    if (!isTape(tape)) throw new Error('the cassette\'s tape isn\'t a UNC tape');
    const extra = {};
    for (const k of Object.keys(j)) if (!KNOWN.includes(k)) extra[k] = j[k];
    let files = null;
    if (j.files && typeof j.files === 'object') {
      files = {};
      for (const [p, t] of Object.entries(j.files)) if (typeof t === 'string' && safePath(p)) files[p] = t;
      if (!Object.keys(files).length) files = null;
    }
    const m = j.machine || {};
    return {
      title: String(j.title || programs(tape)[0]?.name || 'UNTITLED'),
      author: String(j.author || ''),
      about: String(j.about || ''),
      machine: { ram: m.ram === 64 ? 64 : 16, board: !!m.board, niece: !!m.niece },
      tape,
      files,
      main: files && files[j.main] !== undefined ? j.main : files ? Object.keys(files)[0] : null,
      created: j.created || null,
      extra,
      png: png || null,
    };
  }
  const safePath = (p) => typeof p === 'string' && p.length > 0 && !p.startsWith('/') && !p.split('/').includes('..');

  // A bare tape: titled after its first program, with a blank tape's face.
  function fromTape(tape) {
    const first = programs(tape)[0];
    if (!first || first.ok === false && !first.name) throw new Error('not a UNC tape');
    return { title: first.name || 'TAPE', author: '', about: '', machine: { ram: 16, board: false }, tape, files: null, main: null, created: null, extra: {}, png: null, blank: true };
  }

  function read(bytes, filename = '') {
    if (isPNG(bytes)) {
      const c = chunks(bytes).find((x) => x.type === CHUNK);
      if (!c) throw new Error(`${filename || 'this picture'} is a PNG but not a cassette: it has no program in it`);
      if (!c.crcOK) throw new Error('the cassette\'s program is damaged (its CRC doesn\'t match)');
      let j;
      try { j = JSON.parse(new TextDecoder().decode(c.data)); } catch (e) { throw new Error('the cassette\'s program is unreadable'); }
      return fromJSON(j, bytes);
    }
    if (isTape(bytes)) return fromTape(bytes);
    throw new Error(`${filename || 'that file'} isn't a cassette (.unc.png) or a tape (.tap)`);
  }

  // ---------- writing ----------
  function toJSON(c) {
    const j = { format: FORMAT, version: VERSION, title: c.title, author: c.author || '', about: c.about || '' };
    j.machine = { ram: c.machine?.ram === 64 ? 64 : 16, board: !!c.machine?.board };
    if (c.machine?.niece) j.machine.niece = true;
    j.tape = toBase64(c.tape);
    if (c.files && Object.keys(c.files).length) { j.files = c.files; j.main = c.main; }
    j.created = c.created || new Date().toISOString().replace(/\.\d+Z$/, 'Z');
    for (const [k, v] of Object.entries(c.extra || {})) if (!(k in j)) j[k] = v;
    return j;
  }
  function canvasPNG(canvas) {
    return new Promise((resolve, reject) => canvas.toBlob((b) => (b ? b.arrayBuffer().then((a) => resolve(new Uint8Array(a)), reject) : reject(new Error('could not draw the face'))), 'image/png'));
  }
  async function toPNG(c, face) {
    const png = await canvasPNG(face);
    return withChunk(png, CHUNK, new TextEncoder().encode(JSON.stringify(toJSON(c))));
  }

  // ---------- the face ----------
  // 0.1 mm to a unit: 100.4 x 63.8 mm, as the game's cassettes (scripts/room-art/art.js):
  // the shell, five screws, the head opening on its trapezoid; the label with a band for the
  // title and a picture of the screen, the window with its two hubs, a line for the author.
  const W = 1004;
  const H = 638;
  const STYLES = [
    { shell: '#1c1c1e', face: '#0d130d', ink: '#7fe39a' },
    { shell: '#1c1c1e', face: '#2a1f4a', ink: '#e9c04e' },
    { shell: '#d8d4ca', face: '#090b1c', ink: '#c8e0ff' },
    { shell: '#1c1c1e', face: '#e9c04e', ink: '#1a1410' },
    { shell: '#1d2440', face: '#0b1030', ink: '#9fe08a' },
    { shell: '#5a2620', face: '#1a1410', ink: '#f0a868' },
    { shell: '#cfc7b4', face: '#2b2018', ink: '#efe6cf' },
  ];
  const styleFor = (title) => STYLES[[...String(title)].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 7) % STYLES.length];
  const FONTS = { display: 'Anton, Impact, "Arial Narrow", sans-serif', body: '"Libre Baskerville", Georgia, serif', rom: 'UNC, ui-monospace, monospace' };
  const fontsReady = () => Promise.all(['100px Anton', '30px "Libre Baskerville"', 'italic 30px "Libre Baskerville"', '32px UNC']
    .map((f) => document.fonts.load(f).catch(() => {})));

  function rr(ctx, x, y, w, h, r) {
    ctx.beginPath();
    ctx.roundRect ? ctx.roundRect(x, y, w, h, r) : ctx.rect(x, y, w, h);
  }
  function fit(ctx, text, max, size, font) {
    let s = size;
    ctx.font = `${s}px ${font}`;
    while (s > 20 && ctx.measureText(text).width > max) { s -= 2; ctx.font = `${s}px ${font}`; }
    return s;
  }
  function clip(ctx, text, max) {
    if (ctx.measureText(text).width <= max) return text;
    let t = text;
    while (t.length > 1 && ctx.measureText(t + '…').width > max) t = t.slice(0, -1);
    return t.trimEnd() + '…';
  }

  // opts: { title, author, about, machine: { ram, board }, screen: a canvas or null,
  //         style: { shell, face, ink } (default: from the title), blank: a blank tape's label }
  function drawFace(canvas, o) {
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext('2d');
    const st = o.blank ? { shell: '#2b2724' } : o.style || styleFor(o.title || '');
    ctx.clearRect(0, 0, W, H);
    // the shell
    rr(ctx, 0, 0, W, H, 28);
    ctx.fillStyle = st.shell;
    ctx.fill();
    const g = ctx.createLinearGradient(0, 0, 0, H);
    g.addColorStop(0, 'rgba(255,255,255,.14)');
    g.addColorStop(1, 'rgba(0,0,0,.18)');
    ctx.fillStyle = g;
    ctx.fill();
    rr(ctx, 10, 10, 984, 618, 22);
    ctx.strokeStyle = 'rgba(0,0,0,.25)';
    ctx.lineWidth = 3;
    ctx.stroke();
    // the trapezoid at the bottom: the head opening, the capstan holes, the pinch rollers
    ctx.fillStyle = 'rgba(0,0,0,.18)';
    ctx.beginPath(); ctx.moveTo(214, 470); ctx.lineTo(790, 470); ctx.lineTo(846, 638); ctx.lineTo(158, 638); ctx.closePath(); ctx.fill();
    ctx.fillStyle = 'rgba(255,255,255,.12)';
    ctx.fillRect(214, 469, 576, 3);
    for (const x of [292, 712]) {
      ctx.fillStyle = '#0a0908'; ctx.beginPath(); ctx.arc(x, 556, 22, 0, 7); ctx.fill();
      ctx.fillStyle = '#5a5650'; ctx.beginPath(); ctx.arc(x, 556, 8, 0, 7); ctx.fill();
    }
    ctx.fillStyle = '#0a0908';
    ctx.fillRect(436, 588, 132, 50);
    for (const x of [236, 704]) ctx.fillRect(x, 604, 64, 34);
    ctx.fillRect(350, 612, 46, 26);
    ctx.fillRect(608, 612, 46, 26);
    for (const [x, y] of [[48, 48], [956, 48], [48, 590], [956, 590], [502, 520]]) {
      ctx.fillStyle = '#8d8a84'; ctx.beginPath(); ctx.arc(x, y, 15, 0, 7); ctx.fill();
      ctx.strokeStyle = '#3a3835'; ctx.lineWidth = 3;
      ctx.beginPath(); ctx.moveTo(x - 9, y); ctx.lineTo(x + 9, y); ctx.moveTo(x, y - 9); ctx.lineTo(x, y + 9); ctx.stroke();
    }
    // the label
    const L = { x: 72, y: 40, w: 860, h: 410 };
    const HUB = 296; // the hubs' centre line, a little lower than a bought tape's to give the band room
    ctx.fillStyle = o.blank ? '#f4f1e8' : '#f2ecdc';
    rr(ctx, L.x, L.y, L.w, L.h, 14);
    ctx.fill();
    const ink = '#2b2018';
    if (o.blank) {
      // a blank tape: the maker's band, ruled lines, the name typed on
      ctx.save();
      rr(ctx, L.x, L.y, L.w, L.h, 14);
      ctx.clip();
      ctx.fillStyle = '#2b3a6a';
      ctx.fillRect(L.x, L.y, L.w, 46);
      ctx.restore();
      ctx.fillStyle = '#f4f1e8';
      ctx.font = `26px ${FONTS.display}`;
      ctx.letterSpacing = '4px';
      ctx.fillText('LOW NOISE  C60', 110, L.y + 34);
      ctx.fillStyle = '#e9c04e';
      ctx.font = `30px ${FONTS.display}`;
      ctx.fillText('A', 880, L.y + 36);
      ctx.letterSpacing = '0px';
      ctx.fillStyle = '#9db0d0';
      for (const y of [150, 204, 410, 440]) ctx.fillRect(110, y, 784, 2);
      ctx.fillStyle = '#2a2a2a';
      const s = fit(ctx, (o.title || '').toUpperCase(), 760, 64, FONTS.rom);
      ctx.font = `${s - (s % 8)}px ${FONTS.rom}`;
      ctx.fillText((o.title || '').toUpperCase(), 124, 196);
      if (o.author) {
        ctx.font = `32px ${FONTS.rom}`;
        ctx.fillText(clip(ctx, o.author.toUpperCase(), 760), 124, 434);
      }
    } else {
      // the band: the screen on the left, the title beside it, a line about it under
      const B = 182;
      ctx.save();
      rr(ctx, L.x, L.y, L.w, L.h, 14);
      ctx.clip();
      ctx.fillStyle = st.face;
      ctx.fillRect(L.x, L.y, L.w, B);
      ctx.restore();
      const sx = 88;
      const sy = 54;
      const sh = 154;
      const sw = Math.round(sh * 1.6);
      ctx.fillStyle = '#000';
      rr(ctx, sx - 6, sy - 6, sw + 12, sh + 12, 10);
      ctx.fill();
      if (o.screen) {
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.drawImage(o.screen, sx, sy, sw, sh);
      } else {
        ctx.fillStyle = '#0b130d';
        ctx.fillRect(sx, sy, sw, sh);
        ctx.fillStyle = '#3f8f52';
        ctx.font = `16px ${FONTS.rom}`;
        ctx.fillText('READY.', sx + 10, sy + 26);
      }
      ctx.fillStyle = 'rgba(255,255,255,.07)';
      ctx.fillRect(sx, sy, sw, sh / 2);
      const tx = sx + sw + 30;
      const tw = L.x + L.w - 24 - tx;
      const title = (o.title || 'UNTITLED').toUpperCase();
      ctx.fillStyle = st.ink;
      ctx.letterSpacing = '4px';
      const ts = fit(ctx, title, tw, 104, FONTS.display);
      ctx.fillText(title, tx, o.about ? L.y + 24 + ts * 0.95 : L.y + B / 2 + ts * 0.42);
      ctx.letterSpacing = '0px';
      if (o.about) {
        ctx.globalAlpha = 0.75;
        ctx.font = `italic 22px ${FONTS.body}`;
        ctx.fillText(clip(ctx, o.about.split('\n')[0], tw), tx, L.y + B - 24);
        ctx.globalAlpha = 1;
      }
      // side A, and what it needs
      ctx.fillStyle = ink;
      ctx.font = `54px ${FONTS.display}`;
      ctx.fillText('A', 100, HUB + 20);
      ctx.textAlign = 'right';
      const ram = `${o.machine?.ram === 64 ? 64 : 16}K`;
      ctx.font = `40px ${FONTS.display}`;
      const cards = [o.machine?.board ? 'VIDEO BOARD' : '', o.machine?.niece ? 'NIECE' : ''].filter(Boolean).join(' + ');
      ctx.fillText(ram, 902, HUB + (cards ? 0 : 16));
      if (cards) {
        ctx.font = `20px ${FONTS.display}`;
        ctx.letterSpacing = '1px';
        ctx.fillText(`+ ${cards}`, 904, HUB + 32);
        ctx.letterSpacing = '0px';
      }
      ctx.textAlign = 'left';
      // the write-on line: who made it
      ctx.font = `28px ${FONTS.display}`;
      ctx.letterSpacing = '2px';
      ctx.fillText('BY', 104, 432);
      ctx.letterSpacing = '0px';
      ctx.fillRect(154, 436, 736, 2.5);
      if (o.author) {
        ctx.fillStyle = '#1d1d2a';
        ctx.font = `italic 34px ${FONTS.body}`;
        ctx.fillText(clip(ctx, o.author, 700), 172, 426);
      }
    }
    // the window: smoked plastic, the tape on its two hubs
    const win = { x: 226, y: HUB - 70, w: 552, h: 140 };
    ctx.save();
    rr(ctx, win.x, win.y, win.w, win.h, 70);
    ctx.clip();
    ctx.fillStyle = '#2a2622';
    ctx.fillRect(win.x, win.y, win.w, win.h);
    ctx.fillStyle = '#4a3020';
    ctx.beginPath(); ctx.arc(292, HUB, 128, 0, 7); ctx.fill();
    ctx.strokeStyle = '#6a4a32'; ctx.lineWidth = 2; ctx.stroke();
    ctx.beginPath(); ctx.arc(712, HUB, 66, 0, 7); ctx.fill();
    for (const x of [292, 712]) {
      ctx.fillStyle = '#ece8dc'; ctx.beginPath(); ctx.arc(x, HUB, 46, 0, 7); ctx.fill();
      ctx.fillStyle = '#1a1816'; ctx.beginPath(); ctx.arc(x, HUB, 24, 0, 7); ctx.fill();
      ctx.fillStyle = '#ece8dc';
      for (let k = 0; k < 6; k++) {
        ctx.save();
        ctx.translate(x, HUB);
        ctx.rotate((k * Math.PI) / 3);
        ctx.fillRect(-5, -24, 10, 12);
        ctx.restore();
      }
    }
    ctx.restore();
    rr(ctx, win.x, win.y, win.w, win.h, 70);
    ctx.fillStyle = 'rgba(255,255,255,.08)';
    ctx.fill();
    ctx.strokeStyle = 'rgba(0,0,0,.4)';
    ctx.lineWidth = 4;
    ctx.stroke();
    ctx.fillStyle = 'rgba(255,255,255,.4)';
    for (let k = 0; k < 7; k++) ctx.fillRect(440 + k * 20, win.y + 122, 2, k % 3 ? 8 : 14);
    return canvas;
  }

  // A file name for a cassette: the title, lower case, safe anywhere.
  const fileName = (title, ext) => (String(title).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'cassette') + ext;

  return { read, toPNG, toJSON, drawFace, fontsReady, programs, isPNG, isTape, chunks, withChunk, crc32, toBase64, fromBase64, styleFor, STYLES, fileName, CHUNK, FORMAT, VERSION };
})();
