// The library: cassettes as pictures on a shelf, PICO-8 "splore" style. Each section is a
// *source* with the same small interface, so the community library can plug in later as
// one more:
//
//   { id, name, note,            // the section's heading and a line under it
//     writable,                  // Yours: true (put, remove)
//     list()  -> Promise<[entry]>,
//     get(id) -> Promise<cassette>  (UNCCassette's shape: title, tape, files, main, ...) }
//
//   entry: { id, title, author, about, machine: { ram, board, niece }, hasFiles, kind,
//            face() -> Promise<url of the face picture> }
//
// Yours lives in the browser's IndexedDB. Where the browser won't keep anything (some
// private windows, some browsers on file://), it keeps them in memory for this visit and
// says so.
window.UNCLibrary = (() => {
  const C = window.UNCCassette;
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  // ---------- Yours: IndexedDB, or memory ----------
  const DB = 'unc64-machine-room';
  function openDB() {
    return new Promise((resolve, reject) => {
      let req;
      try { req = indexedDB.open(DB, 1); } catch (e) { reject(e); return; }
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('cassettes')) db.createObjectStore('cassettes', { keyPath: 'id' });
        if (!db.objectStoreNames.contains('state')) db.createObjectStore('state');
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
      req.onblocked = () => reject(new Error('blocked'));
    });
  }
  const tx = (db, store, mode, fn) => new Promise((resolve, reject) => {
    const t = db.transaction(store, mode);
    const s = t.objectStore(store);
    const r = fn(s);
    t.oncomplete = () => resolve(r && 'result' in r ? r.result : undefined);
    t.onerror = () => reject(t.error);
    t.onabort = () => reject(t.error || new Error('aborted'));
  });

  const store = (() => {
    let db = null;
    const mem = { cassettes: new Map(), state: new Map() };
    const ready = (async () => {
      try {
        db = await Promise.race([openDB(), new Promise((_, rej) => setTimeout(() => rej(new Error('timeout')), 3000))]);
        // a write proves it: some browsers open the database and then refuse to keep anything
        await tx(db, 'state', 'readwrite', (s) => s.put(1, 'probe'));
        return true;
      } catch (e) {
        db = null;
        console.warn('IndexedDB unavailable; keeping cassettes in memory', e && e.message);
        return false;
      }
    })();
    return {
      ready,
      get persistent() { return !!db; },
      async all() {
        await ready;
        if (!db) return [...mem.cassettes.values()];
        return tx(db, 'cassettes', 'readonly', (s) => s.getAll());
      },
      async get(id) {
        await ready;
        if (!db) return mem.cassettes.get(id);
        return tx(db, 'cassettes', 'readonly', (s) => s.get(id));
      },
      async put(rec) {
        await ready;
        if (!db) { mem.cassettes.set(rec.id, rec); return; }
        await tx(db, 'cassettes', 'readwrite', (s) => s.put(rec));
      },
      async remove(id) {
        await ready;
        if (!db) { mem.cassettes.delete(id); return; }
        await tx(db, 'cassettes', 'readwrite', (s) => s.delete(id));
      },
      async getState(k) {
        await ready;
        if (!db) return mem.state.get(k);
        return tx(db, 'state', 'readonly', (s) => s.get(k));
      },
      async setState(k, v) {
        await ready;
        if (!db) { mem.state.set(k, v); return; }
        await tx(db, 'state', 'readwrite', (s) => s.put(v, k));
      },
    };
  })();

  const urls = new Map(); // id -> blob URL of the face, made once
  function faceURL(key, png) {
    if (!urls.has(key)) urls.set(key, URL.createObjectURL(new Blob([png], { type: 'image/png' })));
    return urls.get(key);
  }
  const dropURL = (key) => { if (urls.has(key)) { URL.revokeObjectURL(urls.get(key)); urls.delete(key); } };

  const summary = (c) => ({ title: c.title, author: c.author, about: c.about, machine: c.machine, hasFiles: !!c.files, kind: c.files && c.main && /\.bas$/i.test(c.main) ? 'basic' : 'asm' });
  const newId = () => `c${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;

  const yours = {
    id: 'yours',
    name: 'Yours',
    note: 'Kept in this browser. Export the ones you want to keep for good, or to bring into the game.',
    writable: true,
    async list() {
      const all = await store.all();
      all.sort((a, b) => (b.saved || '').localeCompare(a.saved || ''));
      return all.map((r) => ({ id: r.id, ...r.summary, saved: r.saved, face: async () => faceURL(r.id + r.saved, r.png) }));
    },
    async get(id) {
      const r = await store.get(id);
      if (!r) throw new Error('that cassette is gone');
      const c = C.read(r.png);
      c.libId = id;
      return c;
    },
    // Keeps a cassette (png: the .unc.png's bytes). With id, replaces that one.
    async put(png, id) {
      const c = C.read(png);
      const rec = { id: id || newId(), png, summary: summary(c), saved: new Date().toISOString() };
      if (id) [...urls.keys()].filter((k) => k.startsWith(id)).forEach(dropURL);
      await store.put(rec);
      return rec.id;
    },
    async remove(id) {
      [...urls.keys()].filter((k) => k.startsWith(id)).forEach(dropURL);
      await store.remove(id);
    },
  };

  // The community library goes here: a source like the others, list() and get() over the
  // network. Nothing is fetched today.
  const online = {
    id: 'online',
    name: 'Online',
    note: 'Coming later: cassettes other people have posted, in the same file format.',
    placeholder: true,
    async list() { return []; },
    async get() { throw new Error('not yet'); },
  };

  // ---------- the Browse view ----------
  // host: { run(source, id), open(source, id), download(source, id), duplicate(id), confirm(text) -> bool, toast(text) }
  function view(root, sources, host) {
    let query = '';
    let token = 0;
    root.innerHTML = `
      <div class="br-head">
        <h2>Browse</h2>
        <input type="search" id="br-q" placeholder="Search titles and authors" aria-label="Search cassettes" autocomplete="off">
        <span class="br-note">Drop <b>.unc.png</b>, <b>.tap</b> or <b>.s</b>/<b>.bas</b> files anywhere to bring them in.</span>
        <button class="btn" type="button" id="br-close">Back to the editor <kbd>Esc</kbd></button>
      </div>
      <div id="br-warn" class="br-warn" hidden></div>
      <div id="br-sections"></div>`;
    root.querySelector('#br-q').addEventListener('input', (e) => { query = e.target.value.trim().toLowerCase(); render(); });
    root.querySelector('#br-close').addEventListener('click', () => host.close());

    async function render() {
      const my = ++token;
      const box = root.querySelector('#br-sections');
      const warn = root.querySelector('#br-warn');
      await store.ready;
      warn.hidden = store.persistent;
      warn.textContent = 'This browser won\'t keep your cassettes here, so Yours lasts until you close the page. Export the ones you want (Download), and open them again next time.';
      const parts = [];
      for (const src of sources) {
        let list = [];
        try { list = await src.list(); } catch (e) { console.error(e); }
        if (my !== token) return;
        const shown = list.filter((e) => !query || `${e.title} ${e.author} ${e.about}`.toLowerCase().includes(query));
        parts.push({ src, list: shown, total: list.length });
      }
      box.innerHTML = parts.map(({ src, list, total }) => {
        const groups = [...new Set(list.map((e) => e.group || ''))];
        const cards = (l) => l.map((e) => card(src, e)).join('');
        let body;
        if (src.placeholder) body = `<div class="br-empty">${esc(src.note)}</div>`;
        else if (!list.length) body = `<div class="br-empty">${total ? 'Nothing here matches.' : src.writable ? 'No cassettes yet. Save your program (Save), or drop a .unc.png or .tap on the page.' : 'Nothing here.'}</div>`;
        else body = groups.map((g) => `${g ? `<h4>${esc(g)}</h4>` : ''}<div class="br-grid">${cards(list.filter((e) => (e.group || '') === g))}</div>`).join('');
        return `<section class="br-sec" data-src="${src.id}"><h3>${esc(src.name)} <span>${src.placeholder ? '' : total}</span></h3>${src.placeholder ? '' : `<p>${esc(src.note)}</p>`}${body}</section>`;
      }).join('');
      // the faces, as they come
      for (const { src, list } of parts) {
        for (const e of list) {
          e.face().then((url) => {
            if (my !== token) return;
            const img = box.querySelector(`[data-src="${src.id}"] [data-id="${CSS.escape(e.id)}"] img`);
            if (img && url) { img.src = url; img.classList.add('on'); }
          }).catch((err) => console.warn('face', e.id, err));
        }
      }
    }

    function card(src, e) {
      const badges = [
        `<span class="b">${e.machine?.ram === 64 ? '64K' : '16K'}</span>`,
        e.machine?.board ? '<span class="b board">Video board</span>' : '',
        e.machine?.niece ? '<span class="b board">NIECE</span>' : '',
        e.kind === 'basic' ? '<span class="b">BASIC</span>' : '',
        e.hasFiles ? '<span class="b src">Source</span>' : '<span class="b">Binary only</span>',
      ].join('');
      const acts = [
        `<button class="btn go" data-act="run" type="button" title="Put it in the deck and load it">Run</button>`,
        e.hasFiles ? `<button class="btn" data-act="open" type="button" title="Open the project in the editor">Open</button>` : '',
        `<button class="btn" data-act="download" type="button" title="Download the .unc.png">Download</button>`,
        src.writable ? `<button class="btn" data-act="duplicate" type="button">Duplicate</button><button class="btn warn" data-act="delete" type="button">Delete</button>` : '',
      ].join('');
      return `<article class="br-card" data-id="${esc(e.id)}">
        <div class="br-face"><img alt="${esc(e.title)} cassette" draggable="false"></div>
        <div class="br-meta"><b>${esc(e.title)}</b>${e.author ? `<span class="by">by ${esc(e.author)}</span>` : ''}</div>
        ${e.about ? `<p class="about">${esc(e.about)}</p>` : ''}
        <div class="badges">${badges}</div>
        <div class="acts">${acts}</div>
      </article>`;
    }

    root.addEventListener('click', async (ev) => {
      const b = ev.target.closest('[data-act]');
      if (!b) return;
      const id = b.closest('[data-id]').dataset.id;
      const src = sources.find((s) => s.id === b.closest('[data-src]').dataset.src);
      const act = b.dataset.act;
      try {
        if (act === 'run') await host.run(src, id);
        else if (act === 'open') await host.open(src, id);
        else if (act === 'download') await host.download(src, id);
        else if (act === 'duplicate') { await host.duplicate(src, id); render(); }
        else if (act === 'delete') {
          const e = (await src.list()).find((x) => x.id === id);
          if (host.confirm(`Delete ${e ? e.title : 'this cassette'} from your library? It can't be undone.`)) { await src.remove(id); render(); }
        }
      } catch (err) {
        console.error(err);
        host.toast(err.message || String(err));
      }
    });
    return { render };
  }

  return { store, yours, online, view, newId };
})();
