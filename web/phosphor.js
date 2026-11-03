// The monitor's look: monochrome phosphors as on period monitors, green (P1), amber (P3) or
// paper white (P4), and Blue: a blue screen with white text, the most familiar home-computer
// screen of the era. Only the picture changes; the machine doesn't know. The choice is
// remembered across pages (localStorage, when the browser allows it).
//
//   UNCPHOSPHOR.current()      -> { id, name, ink: [r,g,b], glow: 'rgba(...)' }
//   UNCPHOSPHOR.mono(palette)  -> the 16 colours as shades of the phosphor (CSS strings)
//   UNCPHOSPHOR.border(mono, i) -> the colour to paint border colour i in (mono from above)
//   UNCPHOSPHOR.picker(el)     -> fills el with the three buttons
//   UNCPHOSPHOR.onChange(fn)
window.UNCPHOSPHOR = (() => {
  const LIST = [
    { id: 'green', name: 'Green', note: 'P1', ink: [141, 255, 160] },
    { id: 'amber', name: 'Amber', note: 'P3', ink: [255, 182, 64] },
    { id: 'white', name: 'Paper white', note: 'P4', ink: [232, 238, 246] },
    // dark: what black becomes. Without it, black is the glass faintly tinted by the phosphor.
    // edge: the border's colour while the border is black, as the classic blue screens had
    // a lighter blue frame around the dark blue picture.
    { id: 'blue', name: 'Blue', note: 'blue screen', ink: [238, 241, 255], dark: [44, 50, 148], edge: [112, 120, 232] },
  ];
  const KEY = 'unc64-phosphor';
  let id = 'green';
  try { id = localStorage.getItem(KEY) || 'green'; } catch { /* storage unavailable */ }
  const listeners = [];
  const current = () => {
    const p = LIST.find((x) => x.id === id) || LIST[0];
    return { ...p, glow: `rgba(${p.ink.join(',')},0.7)` };
  };
  const mono = (palette) => {
    const lit = current().ink;
    const dark = current().dark || lit.map((v) => 4 + v * 0.045);
    return palette.map((h) => {
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
      const l = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
      const c = [0, 1, 2].map((i) => Math.round(dark[i] + (lit[i] - dark[i]) * l));
      return `rgb(${c.join(',')})`;
    });
  };
  const set = (next) => {
    id = next;
    try { localStorage.setItem(KEY, next); } catch { /* storage unavailable */ }
    document.querySelectorAll('[data-phosphor]').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.phosphor === next)));
    listeners.forEach((fn) => fn(current()));
  };
  const picker = (el) => {
    el.innerHTML = '<span class="ph-label">Screen</span>' + LIST.map((p) =>
      `<button type="button" data-phosphor="${p.id}" aria-pressed="${p.id === id}" title="${p.dark ? p.name + ' screen' : p.name + ' phosphor (' + p.note + ')'}">` +
      `<span class="ph-dot" style="background:rgb(${(p.dark || p.ink).join(',')})${p.dark ? `;box-shadow:inset 0 0 0 3px rgb(${p.dark.join(',')}),inset 0 0 0 6px rgb(${p.ink.join(',')})` : ''}"></span>${p.name}</button>`).join('');
    el.querySelectorAll('[data-phosphor]').forEach((b) => b.addEventListener('click', () => set(b.dataset.phosphor)));
  };
  const border = (mono, i) => {
    const e = current().edge;
    return e && (i & 15) === 0 ? `rgb(${e.join(',')})` : mono[i & 15];
  };
  return { current, mono, border, set, picker, onChange: (fn) => listeners.push(fn), list: LIST };
})();
