// The Machine Room's source editor: CodeMirror 6 with UNC assembly highlighting, the real
// assembler's errors as you type, completion, hover docs, cycle counts and a Vim mode.
// It takes over from the textarea once app.js has the machine (window.UNCRoom.ready); if
// it can't load, the textarea keeps working. The language reference and tokenizer are
// unc-lang.js, shared with the showcase. CodeMirror is vendored: see vendor/README.md.
import {
  EditorState, Compartment, StateField, StateEffect, RangeSet, Prec,
  EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection,
  highlightSpecialChars, gutter, GutterMarker, hoverTooltip, Decoration,
  StreamLanguage, HighlightStyle, syntaxHighlighting, bracketMatching, tags as t,
  defaultKeymap, history, historyKeymap, insertTab, indentLess,
  autocompletion, completionKeymap, pickedCompletion,
  linter, lintGutter, lintKeymap,
  search, searchKeymap, highlightSelectionMatches,
  vim, Vim,
} from './vendor/codemirror.js';

const L = window.UNCLang;
const room = await window.UNCRoom.ready;
const $ = (id) => document.getElementById(id);
const store = {
  get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* no storage */ } },
};

// ---------- checking, on the bench ----------
// The source is assembled as you type on a machine of the Machine Room's own (the bench),
// never on the one you're debugging. A file is checked as part of the project: from main
// when main includes it, else on its own.
const lang = L.fromWasm(room.bench);
let curPath = null;
function checkFile(path) {
  const files = room.files();
  const main = room.main();
  const pat = path.replace(/[.*+?^$()|[\]\\{}]/g, '\\$&');
  const inc = new RegExp('\\.include\\s+"(\\./)?' + pat + '"', 'i');
  const included = Object.values(files).some((t) => inc.test(t));
  const from = path === main || (included && !room.isBasic(main)) ? main : path;
  const res = room.check(from);
  return {
    ok: res.ok,
    symbols: res.symbols || [],
    lines: (res.lines || []).filter((l) => l[3] === path),
    errors: (res.errors || []).filter((e) => e[2] === path),
    elsewhere: (res.errors || []).filter((e) => e[2] !== path),
  };
}

// ---------- highlighting ----------
const TYPES = {
  mnemonic: t.keyword, directive: t.processingInstruction, labeldef: t.definition(t.labelName),
  localdef: t.definition(t.local(t.labelName)), constdef: t.definition(t.constant(t.variableName)),
  name: t.variableName, local: t.local(t.variableName), hw: t.standard(t.variableName), number: t.number,
  string: t.string, char: t.character, function: t.function(t.variableName), register: t.atom, pc: t.self,
  operator: t.operator, bracket: t.bracket, punct: t.punctuation, paste: t.meta, comment: t.lineComment,
  invalid: t.invalid,
};
const uncAsm = StreamLanguage.define({
  name: 'unc-asm',
  startState: () => ({ toks: [], i: 0 }),
  copyState: (s) => ({ toks: s.toks, i: s.i }),
  token(stream, state) {
    if (stream.sol()) { state.toks = L.tokenizeLine(stream.string, lang); state.i = 0; }
    while (state.i < state.toks.length && state.toks[state.i].to <= stream.pos) state.i++;
    const tok = state.toks[state.i];
    if (!tok) { stream.skipToEnd(); return null; }
    if (tok.from > stream.pos) { stream.pos = tok.from; return null; }
    stream.pos = tok.to;
    state.i++;
    return tok.type;
  },
  languageData: { commentTokens: { line: ';' } },
  tokenTable: TYPES,
});
// Classes, not colours: style.css colours .ul-* the same way for every surface.
const highlight = HighlightStyle.define(Object.entries(TYPES).map(([k, tag]) => ({ tag, class: `ul-${k}` })));

// ---------- the last assembly ----------
const assembled = StateEffect.define();
const asmField = StateField.define({
  create: () => ({ ok: false, symbols: new Map(), lens: new Map() }),
  update(v, tr) {
    for (const e of tr.effects) if (e.is(assembled)) return e.value;
    return v;
  },
});
const toAsm = (res, prev) => (res.ok
  ? { ok: true, symbols: new Map(res.symbols), lens: L.lengths(res.lines) }
  : { ok: false, symbols: prev.symbols, lens: new Map() });

// ---------- cycle counts ----------
// One marker per instruction line, kept in a RangeSet so it moves with edits. An assembly
// sets them all exactly (the assembler's byte counts settle fast page against absolute);
// a line you edit gets a quick estimate until the next one.
class Cycles extends GutterMarker {
  constructor(c) { super(); this.c = c; }
  eq(o) { return o.c.cycles === this.c.cycles && o.c.taken === this.c.taken && o.c.guess === this.c.guess; }
  toDOM() {
    const s = document.createElement('span');
    s.textContent = L.cycleText(this.c);
    s.title = this.c.taken ? `${this.c.cycles} cycles, ${this.c.taken} if the branch is taken` : `${this.c.cycles} cycles`;
    if (this.c.guess) s.className = 'guess';
    return s;
  }
}
function linesCycles(doc, from, to, lens, symbols, b) {
  for (let n = from; n <= to; n++) {
    const line = doc.line(n);
    const c = L.lineCycles(lang, line.text, lens && lens.get(n), symbols);
    if (c) b.push(new Cycles(c).range(line.from));
  }
}
const cyclesField = StateField.define({
  create(state) {
    const b = [];
    linesCycles(state.doc, 1, state.doc.lines, null, null, b);
    return RangeSet.of(b);
  },
  update(set, tr) {
    for (const e of tr.effects) {
      if (e.is(assembled)) {
        const b = [];
        linesCycles(tr.state.doc, 1, tr.state.doc.lines, e.value.ok ? e.value.lens : null, e.value.symbols, b);
        return RangeSet.of(b);
      }
    }
    if (!tr.docChanged) return set;
    set = set.map(tr.changes);
    const doc = tr.state.doc;
    const symbols = tr.startState.field(asmField).symbols;
    const spans = [];
    tr.changes.iterChangedRanges((fa, ta, fb, tb) => spans.push([doc.lineAt(fb).number, doc.lineAt(tb).number]));
    for (const [a, z] of spans) {
      const add = [];
      linesCycles(doc, a, z, null, symbols, add);
      set = set.update({ filterFrom: doc.line(a).from, filterTo: doc.line(z).to, filter: () => false, add });
    }
    return set;
  },
});
const cycleGutter = gutter({
  class: 'cm-cycles',
  markers: (v) => v.state.field(cyclesField),
  initialSpacer: () => new Cycles({ cycles: 2, taken: 3 }),
});
const cyclesOn = new Compartment();
const cycleAt = (state, pos) => {
  let c = null;
  state.field(cyclesField).between(pos, pos, (f, to, m) => { c = m.c; });
  return c;
};

// ---------- defining a known name ----------
// Where `NAME = $nnnn` goes: after the constants at the top, or before the first line of
// code, so it's known before it's used (a name defined later is always absolute).
function defineChange(doc, hw) {
  let lastConst = 0;
  let firstCode = 0;
  for (let n = 1; n <= doc.lines && !firstCode; n++) {
    const p = L.parseLine(doc.line(n).text, lang);
    if (p.constant) lastConst = n;
    else if (p.mnemonic || p.directive || p.label) firstCode = n;
  }
  const text = L.defineLine(hw);
  if (lastConst) return { from: doc.line(lastConst).to, insert: '\n' + text };
  if (firstCode) return { from: doc.line(firstCode).from, insert: text + '\n\n' };
  return { from: 0, insert: text + '\n' };
}
const isDefined = (state, name) => L.scan(state.doc.toString(), lang).defined.has(name);

// ---------- errors ----------
const asmLinter = linter((view) => {
  if (!curPath) return [];
  room.changed(curPath, view.state.doc.toString());
  const res = checkFile(curPath);
  view.dispatch({ effects: assembled.of(toAsm(res, view.state.field(asmField))) });
  if (res.ok) return [];
  const doc = view.state.doc;
  return res.errors.filter(([n]) => n >= 1 && n <= doc.lines).map(([n, message]) => {
    const line = doc.line(n);
    const from = line.from + (/^\s*/.exec(line.text)[0].length);
    const d = { from: Math.min(from, line.to), to: line.to, severity: 'error', message };
    const undef = /^undefined symbol '([A-Za-z_]\w*)'/.exec(message);
    const hw = undef && lang.names[undef[1]];
    if (hw) {
      d.actions = [{
        name: `Define ${L.defineLine(hw).replace(/ +/g, ' ')}`,
        apply(v) { if (!isDefined(v.state, hw.name)) v.dispatch({ changes: defineChange(v.state.doc, hw) }); },
      }];
    }
    return d;
  });
}, { delay: 350 });

// ---------- hover ----------
function context(state, lineNo) {
  const { defined, scopes } = L.scan(state.doc.toString(), lang);
  return { symbols: state.field(asmField).symbols, defined, scope: scopes[lineNo - 1] };
}
const docTip = (html) => () => {
  const dom = document.createElement('div');
  dom.className = 'cm-unc-doc';
  dom.innerHTML = html;
  return { dom };
};
const hover = hoverTooltip((view, pos, side) => {
  const line = view.state.doc.lineAt(pos);
  const off = pos - line.from;
  const tok = L.tokenizeLine(line.text, lang).find((x) => x.from <= off && off <= x.to
    && !(side < 0 && x.to === off && off !== line.length) && !(side > 0 && x.from === off && off !== 0));
  if (!tok || tok.type === 'comment') return null;
  const ctx = context(view.state, line.number);
  if (tok.type === 'mnemonic') ctx.here = cycleAt(view.state, line.from);
  const html = L.docHTML(lang, tok, ctx);
  if (!html) return null;
  return { pos: line.from + tok.from, end: line.from + tok.to, above: true, create: docTip(html) };
}, { hoverTime: 250 });

// ---------- completion ----------
function infoDOM(html) {
  return () => { const d = document.createElement('div'); d.className = 'cm-unc-doc'; d.innerHTML = html; return d; };
}
function complete(cx) {
  const word = cx.matchBefore(/[.@]?[\w@]*$/);
  if (!word || (word.from === word.to && !cx.explicit)) return null;
  const line = cx.state.doc.lineAt(cx.pos);
  const col = word.from - line.from;
  if (L.findComment(line.text) < col) return null;
  const before = line.text.slice(0, col);
  if ((before.match(/"/g) || []).length % 2) return null; // in a string
  const lower = word.text.length > 0 && word.text.replace(/^[.@]/, '') === word.text.replace(/^[.@]/, '').toLowerCase() && /[a-z]/.test(word.text);
  const options = [];
  const atStatement = /^\s+$/.test(before) || /^\s*[A-Za-z_@][\w@]*\s*:\s*$/.test(before) || (before === '' && word.text[0] === '.');
  if (atStatement) {
    if (word.text[0] !== '.') {
      for (const op of Object.values(lang.mnemonics)) {
        const c = op.modes.map((m) => m.cycles);
        options.push({
          label: lower ? op.name.toLowerCase() : op.name, type: 'keyword',
          detail: `${op.description.length > 34 ? op.description.slice(0, 32) + '…' : op.description}  ${Math.min(...c)}${Math.max(...c) > Math.min(...c) ? '-' + Math.max(...c) : ''}${op.branch ? '+' : ''} cyc`,
          info: infoDOM(L.mnemonicDoc(lang, op.name)),
        });
      }
    }
    for (const d of Object.values(lang.directives)) {
      options.push({ label: '.' + d.name, type: 'namespace', detail: d.syntax, info: d.description, boost: -1 });
    }
    return { from: word.from, options, validFor: /^[.@]?[\w@]*$/ };
  }
  if (before.trim() === '' || word.text[0] === '.') return null; // a new name at the start of a line
  const { defined, scopes } = L.scan(cx.state.doc.toString(), lang);
  const scope = scopes[line.number - 1];
  const symbols = cx.state.field(asmField).symbols;
  const hex = (v) => (v === undefined ? '' : L.addr(v & 0xFFFF));
  for (const [name, n] of defined) {
    if (n === line.number && word.from <= line.from + line.text.indexOf(name) + name.length) continue;
    const at = name.indexOf('@');
    if (at > 0) {
      if (name.slice(0, at) === scope) options.push({ label: name.slice(at), type: 'variable', detail: `${hex(symbols.get(name))} line ${n}`, boost: 2 });
      continue;
    }
    if (word.text[0] === '@') continue;
    const hw = lang.names[name];
    options.push({ label: name, type: hw ? 'constant' : 'variable', detail: `${hex(symbols.get(name))} line ${n}`, info: hw && hw.description, boost: 1 });
  }
  if (word.text[0] !== '@') {
    for (const hw of Object.values(lang.names)) {
      if (defined.has(hw.name)) continue;
      const kind = { io: 'I/O', rom: 'ROM', var: 'system' }[hw.kind] || '';
      options.push({
        label: hw.name, type: 'constant', detail: `${L.addr(hw.address)} ${kind} · adds ${hw.name} = ${L.addr(hw.address)}`,
        info: hw.description,
        apply(view, completion, from, to) {
          const def = defineChange(view.state.doc, hw);
          const changes = view.state.changes([def, { from, to, insert: hw.name }]);
          view.dispatch({ changes, selection: { anchor: changes.mapPos(to, 1) }, annotations: pickedCompletion.of(completion), scrollIntoView: true });
        },
      });
    }
    for (const f of Object.values(lang.functions)) {
      options.push({ label: f.name, type: 'function', detail: f.syntax, info: f.description, apply: f.name + '(', boost: -1 });
    }
  }
  return { from: word.from, options, validFor: /^[.@]?[\w@]*$/ };
}

// ---------- keys ----------
const assembleKey = () => { room.run(); return true; };
Vim.defineEx('write', 'w', () => room.run());
const keysMode = new Compartment();
const keyModes = {
  standard: [],
  vim: vim({ status: true }),
};

// ---------- the look: the Machine Room's workbench ----------
const theme = EditorView.theme({
  '&': { color: '#e6dcc4', backgroundColor: '#0b0907', border: '1px solid #3a2e22', borderRadius: '4px', fontSize: '13px' },
  '&.cm-focused': { outline: '2px solid #e9c04e', outlineOffset: '2px' },
  '.cm-scroller': { fontFamily: 'ui-monospace,"SF Mono",Menlo,Consolas,monospace', lineHeight: '1.5' },
  '.cm-content': { caretColor: '#8dffa0', padding: '8px 0' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#8dffa0', borderLeftWidth: '2px' },
  '.cm-fat-cursor': { background: '#8dffa0 !important', color: '#06120a !important' },
  '&:not(.cm-focused) .cm-fat-cursor': { background: 'none !important', outline: '1px solid #8dffa0', color: 'inherit !important' },
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection': { backgroundColor: '#2e4a32 !important' },
  '.cm-activeLine': { backgroundColor: '#17120d' },
  '.cm-gutters': { backgroundColor: '#0b0907', color: '#6b5d4a', border: 'none', borderRight: '1px solid #241c15' },
  '.cm-activeLineGutter': { backgroundColor: '#17120d', color: '#a8977a' },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 6px 0 8px', minWidth: '3ch' },
  '.cm-cycles .cm-gutterElement': { padding: '0 6px', color: '#3f8f52', textAlign: 'right', minWidth: '3ch' },
  '.cm-cycles .guess': { opacity: '.6' },
  '.cm-matchingBracket': { backgroundColor: '#2e4a32', outline: 'none' },
  '.cm-selectionMatch': { backgroundColor: '#2a2119' },
  '.cm-searchMatch': { backgroundColor: '#5a4a1a', outline: '1px solid #e9c04e' },
  '.cm-tooltip': { backgroundColor: '#1b1510', border: '1px solid #3a2e22', color: '#efe6cf', borderRadius: '4px' },
  '.cm-tooltip-autocomplete > ul': { fontFamily: 'ui-monospace,"SF Mono",Menlo,Consolas,monospace', maxHeight: '16em' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: '#3f8f52', color: '#06120a' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected] .cm-completionDetail': { color: '#06120a' },
  '.cm-completionDetail': { color: '#a8977a', fontStyle: 'normal', marginLeft: '1.2em' },
  '.cm-completionMatchedText': { textDecoration: 'none', color: '#e9c04e' },
  '.cm-tooltip.cm-completionInfo': { maxWidth: '380px', padding: '6px 8px' },
  '.cm-diagnostic-error': { borderLeft: '4px solid #d0583f', color: '#ffb4a3' },
  '.cm-lintRange-error': { backgroundImage: 'none', textDecoration: 'underline wavy #d0583f', textUnderlineOffset: '3px' },
  '.cm-diagnosticAction': { backgroundColor: '#3f8f52', color: '#06120a', borderRadius: '3px' },
  '.cm-panels': { backgroundColor: '#1b1510', color: '#efe6cf', borderColor: '#3a2e22' },
  '.cm-panels input, .cm-panels button': { color: 'inherit' },
  '.cm-vim-panel': { fontFamily: 'ui-monospace,"SF Mono",Menlo,Consolas,monospace', padding: '2px 8px', minHeight: '1.6em' },
  '.cm-vim-panel input': { color: '#efe6cf', background: 'transparent', outline: 'none' },
}, { dark: true });

// ---------- UNC BASIC (manual chapter 2) ----------
// Plain highlighting: line numbers, keywords (BASIC reads them anywhere, spaces or not),
// strings, numbers, REM. `#` lines are notes, not typed. No assembler checks.
const BASIC_WORDS = ['PRINT', 'LET', 'INPUT', 'IF', 'GOTO', 'GOSUB', 'RETURN', 'FOR', 'NEXT', 'POKE', 'REM', 'END', 'CLS', 'SYS',
  'RUN', 'LIST', 'NEW', 'HELP', 'GR', 'TEXT', 'PLOT', 'LINE', 'CIRCLE', 'INK', 'THEN', 'TO', 'STEP', 'PEEK', 'RND', 'ABS',
  'AND', 'OR', 'POINT', 'LOAD', 'SAVE', 'DATA', 'READ', 'RESTORE', 'GET', 'MON'].sort((a, b) => b.length - a.length);
const BTYPES = { line: t.labelName, keyword: t.keyword, string: t.string, number: t.number, var: t.variableName, rem: t.lineComment, note: t.comment };
const uncBasic = StreamLanguage.define({
  name: 'unc-basic',
  startState: () => ({ rem: false }),
  token(stream, state) {
    if (stream.sol()) {
      state.rem = false;
      if (stream.match(/^\s*#.*/)) return 'note';
      stream.eatSpace();
      if (stream.match(/^\d+/)) return 'line';
    }
    if (state.rem) { stream.skipToEnd(); return 'rem'; }
    if (stream.eatSpace()) return null;
    if (stream.match(/^"[^"]*"?/)) return 'string';
    if (stream.match(/^\$[0-9A-Fa-f]+/) || stream.match(/^\d+/)) return 'number';
    if (stream.peek() === '?') { stream.next(); return 'keyword'; }
    const rest = stream.string.slice(stream.pos).toUpperCase();
    const w = BASIC_WORDS.find((k) => rest.startsWith(k));
    if (w) { stream.pos += w.length; if (w === 'REM') state.rem = true; return 'keyword'; }
    if (stream.match(/^[A-Za-z]/)) return 'var';
    stream.next();
    return null;
  },
  languageData: { commentTokens: { line: '#' } },
  tokenTable: BTYPES,
});
const basicHighlight = HighlightStyle.define(Object.entries(BTYPES).map(([k, tag]) => ({ tag, class: `ub-${k}` })));
function completeBasic(cx) {
  const word = cx.matchBefore(/[A-Za-z]+$/);
  if (!word || (word.text.length < 2 && !cx.explicit)) return null;
  return { from: word.from, options: BASIC_WORDS.map((k) => ({ label: k, type: 'keyword' })), validFor: /^[A-Za-z]*$/ };
}

// ---------- the editor ----------
// One view; each open file keeps its own state (its undo history, its language).
const ta = $('src');
const keysSel = $('keys');
const cyclesBox = $('cycles');
let keys = store.get('unc64.keys') === 'vim' ? 'vim' : 'standard';
let showCycles = store.get('unc64.cycles') !== '0';
const common = () => [
  keysMode.of(keyModes[keys]), // Vim's keys go first, before every other keymap
  Prec.highest(keymap.of([{ key: 'Mod-Enter', run: assembleKey, preventDefault: true }])),
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  history(),
  drawSelection(),
  EditorState.allowMultipleSelections.of(true),
  EditorState.tabSize.of(8),
  bracketMatching(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  search({ top: false }),
  theme,
  EditorView.updateListener.of((u) => {
    if (u.docChanged && curPath) room.changed(curPath, u.state.doc.toString());
    if (curPath && u.state.field(bpField, false) && (u.docChanged || u.transactions.some((tr) => tr.effects.some((e) => e.is(bpToggle))))) {
      const lines = bpLines(u.state);
      if (room.breakpoints && JSON.stringify(lines) !== JSON.stringify(lastBps.get(curPath) || [])) { lastBps.set(curPath, lines); room.breakpoints(curPath, lines); }
    }
    if (u.selectionSet || u.docChanged || u.transactions.some((tr) => tr.effects.length)) showSelection(u.state);
  }),
];
// ---------- breakpoints, and where the machine stopped ----------
// A dot in the gutter beside a line: the debugger stops before that line's instruction
// (app.js turns lines into addresses after each assembly, and moves a dot on a line with no
// code to the next line that has some). The dots move with the text as you edit. When the
// machine stops, the line it stopped on is lit, with an arrow in the same gutter.
const bpToggle = StateEffect.define({ map: (v, m) => ({ pos: m.mapPos(v.pos), on: v.on }) });
const bpSet = StateEffect.define(); // [line numbers]: all of this file's dots at once
const pcSet = StateEffect.define(); // a line number, or 0 for none
const bpMarker = new (class extends GutterMarker {
  toDOM() { const s = document.createElement('span'); s.className = 'cm-bp-dot'; s.textContent = '●'; return s; }
})();
const pcMarker = new (class extends GutterMarker {
  toDOM() { const s = document.createElement('span'); s.className = 'cm-pc-arrow'; s.textContent = '▶'; return s; }
})();
const lineStart = (doc, n) => doc.line(Math.max(1, Math.min(n, doc.lines))).from;
const bpField = StateField.define({
  create: () => RangeSet.empty,
  update(set, tr) {
    set = set.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(bpToggle)) {
        set = e.value.on ? set.update({ add: [bpMarker.range(e.value.pos)] }) : set.update({ filter: (from) => from !== e.value.pos });
      } else if (e.is(bpSet)) {
        const at = [...new Set(e.value.map((n) => lineStart(tr.state.doc, n)))].sort((a, b) => a - b);
        set = RangeSet.of(at.map((pos) => bpMarker.range(pos)));
      }
    }
    return set;
  },
});
const pcField = StateField.define({
  create: () => 0,
  update(n, tr) {
    for (const e of tr.effects) if (e.is(pcSet)) n = e.value;
    return n > tr.state.doc.lines ? 0 : n;
  },
});
const pcLine = Decoration.line({ class: 'cm-pc-line' });
const pcDecorations = EditorView.decorations.compute([pcField], (state) => {
  const n = state.field(pcField);
  return n ? Decoration.set([pcLine.range(lineStart(state.doc, n))]) : Decoration.none;
});
function bpLines(state) {
  const out = [];
  state.field(bpField).between(0, state.doc.length, (from) => { out.push(state.doc.lineAt(from).number); });
  return [...new Set(out)];
}
function toggleBreakpoint(view, pos) {
  const from = view.state.doc.lineAt(pos).from;
  let on = true;
  view.state.field(bpField).between(from, from, () => { on = false; });
  view.dispatch({ effects: bpToggle.of({ pos: from, on }) });
}
const bpGutter = gutter({
  class: 'cm-bp-gutter',
  markers: (v) => {
    const n = v.state.field(pcField);
    const bps = v.state.field(bpField);
    if (!n) return bps;
    const at = lineStart(v.state.doc, n);
    let hasBp = false;
    bps.between(at, at, () => { hasBp = true; });
    return hasBp ? bps : RangeSet.of([...bpRanges(bps, v.state.doc.length), pcMarker.range(at)], true);
  },
  initialSpacer: () => bpMarker,
  domEventHandlers: { mousedown(view, line) { toggleBreakpoint(view, line.from); return true; } },
});
function bpRanges(set, len) { const r = []; set.between(0, len, (from) => { r.push(bpMarker.range(from)); }); return r; }
const breakpoints = () => [bpField, pcField, pcDecorations, bpGutter];

const editKeys = (extra) => keymap.of([
  { key: 'Tab', run: insertTab },
  { key: 'Shift-Tab', run: indentLess },
  ...completionKeymap, ...searchKeymap, ...historyKeymap, ...extra, ...defaultKeymap,
]);
const asmExtensions = () => [
  ...common(),
  breakpoints(),
  cyclesOn.of(showCycles ? cycleGutter : []),
  lintGutter(),
  uncAsm,
  syntaxHighlighting(highlight),
  asmField,
  cyclesField,
  asmLinter,
  hover,
  autocompletion({ override: [complete], icons: false }),
  editKeys(lintKeymap),
  EditorView.contentAttributes.of({ 'aria-label': 'Assembly source', autocapitalize: 'off', spellcheck: 'false' }),
];
const basicExtensions = () => [
  ...common(),
  uncBasic,
  syntaxHighlighting(basicHighlight),
  autocompletion({ override: [completeBasic], icons: false }),
  editKeys([]),
  EditorView.contentAttributes.of({ 'aria-label': 'BASIC listing', autocapitalize: 'characters', spellcheck: 'false' }),
];
const plainExtensions = () => [...common(), editKeys([]), EditorView.lineWrapping];
const kindOf = (path) => (/\.bas$/i.test(path) ? 'basic' : /\.(s|asm|inc)$/i.test(path) ? 'asm' : 'plain');
const states = new Map(); // path -> EditorState
const lastBps = new Map(); // path -> the breakpoint lines last told to app.js
function stateFor(path, text) {
  const kind = kindOf(path);
  const ext = kind === 'asm' ? asmExtensions() : kind === 'basic' ? basicExtensions() : plainExtensions();
  return EditorState.create({ doc: text, extensions: ext });
}
const view = new EditorView({ parent: ta.parentNode, state: stateFor('main.s', ta.value) });
ta.replaceWith(view.dom);
view.dom.id = 'editor';
const NOTE = { asm: 'UNC assembly', basic: 'UNC BASIC: Run types it in', plain: 'Notes' };

const ed = {
  reset() { states.clear(); curPath = null; },
  show(path, text) {
    if (curPath && curPath !== path) states.set(curPath, view.state);
    let st = states.get(path);
    if (!st || st.doc.toString() !== text) st = stateFor(path, text);
    curPath = path;
    view.setState(st);
    // the key mode and the cycle gutter are the page's choice, not the file's
    view.dispatch({ effects: [keysMode.reconfigure(keyModes[keys]), ...(kindOf(path) === 'asm' ? [cyclesOn.reconfigure(showCycles ? cycleGutter : [])] : [])] });
    $('lang-note').textContent = NOTE[kindOf(path)];
    cyclesBox.closest('label').hidden = kindOf(path) !== 'asm';
  },
  get: () => view.state.doc.toString(),
  rename(a, b) {
    if (states.has(a)) { states.delete(a); }
    if (curPath === a) { curPath = null; ed.show(b, view.state.doc.toString()); }
  },
  drop(path) { states.delete(path); },
  // a file's breakpoint lines, from the project (or moved to lines with code after Run)
  setBreakpoints(path, lines) {
    lastBps.set(path, [...lines]);
    if (path === curPath) {
      if (view.state.field(bpField, false)) view.dispatch({ effects: bpSet.of(lines) });
    } else if (states.has(path) && states.get(path).field(bpField, false)) {
      states.set(path, states.get(path).update({ effects: bpSet.of(lines) }).state);
    }
  },
  // the line the machine stopped on (0: none), lit and scrolled to
  showPC(path, line) {
    for (const [p, st] of states) if (p !== curPath && st.field(pcField, false) && st.field(pcField)) states.set(p, st.update({ effects: pcSet.of(0) }).state);
    if (view.state.field(pcField, false) === undefined) return; // not assembly
    const n = path === curPath ? line : 0;
    if (view.state.field(pcField) === n) return;
    view.dispatch({ effects: [pcSet.of(n), ...(n ? [EditorView.scrollIntoView(lineStart(view.state.doc, n), { y: 'center' })] : [])] });
  },
  selectLine(n) {
    const line = view.state.doc.line(Math.max(1, Math.min(n, view.state.doc.lines)));
    view.dispatch({ selection: { anchor: line.from, head: line.to }, scrollIntoView: true });
    view.focus();
  },
};
room.useEditor(ed);

// The cycles in the selection: what a loop body or a raster routine costs.
function showSelection(state) {
  const out = $('selcycles');
  const sel = state.selection.main;
  if (sel.empty || !showCycles || !state.field(cyclesField, false)) { out.textContent = ''; return; }
  const doc = state.doc;
  const a = doc.lineAt(sel.from);
  const z = doc.lineAt(sel.to);
  let n = 0;
  let min = 0;
  let max = 0;
  state.field(cyclesField).between(a.from, z.from, (f, to, m) => { n++; min += m.c.cycles; max += m.c.taken ?? m.c.cycles; });
  if (!n) { out.textContent = ''; return; }
  out.textContent = `Lines ${a.number}-${z.number}: ${n} instruction${n > 1 ? 's' : ''}, ${min}${max > min ? `-${max}` : ''} cycles`;
}

keysSel.value = keys;
keysSel.addEventListener('change', () => {
  keys = keysSel.value === 'vim' ? 'vim' : 'standard';
  store.set('unc64.keys', keys);
  view.dispatch({ effects: keysMode.reconfigure(keyModes[keys]) });
  view.focus();
});
cyclesBox.checked = showCycles;
cyclesBox.addEventListener('change', () => {
  showCycles = cyclesBox.checked;
  store.set('unc64.cycles', showCycles ? '1' : '0');
  if (view.state.field(cyclesField, false)) view.dispatch({ effects: cyclesOn.reconfigure(showCycles ? cycleGutter : []) });
  showSelection(view.state);
});
$('edbar').hidden = false;
window.uncEditor = { view, lang }; // for tests
