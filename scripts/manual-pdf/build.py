"""Builds the printable manual: docs/prg/programmers-reference-guide.md as an A4 PDF, set in
the UNC 64's fonts (fonts/: Libre Baskerville, Courier Prime for code,
VT323 for the running head and the cover's small print, Anton for the
cover, Caveat for the one handwritten note), with the workshop notes stripped as the manual asks.

    python3 scripts/manual-pdf/build.py OUT.pdf

Needs the Python packages `markdown` and `pypdf`, and Node with Playwright (render.js). Two
passes: the first finds the page each chapter starts on, the second prints the contents
with those page numbers.
"""
import html, os, re, subprocess, sys, tempfile

import markdown
from pypdf import PdfReader

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FONTS = os.path.join(ROOT, 'fonts')
SRC = os.path.join(ROOT, 'docs', 'prg', 'programmers-reference-guide.md')
HERE = os.path.dirname(os.path.abspath(__file__))


def manual_body():
    md = open(SRC, encoding='utf-8').read()
    # The workshop notes: "Notes like this one are workshop notes, not part of the in-game
    # book: strip them when the player's copy is printed." Each is a blockquote that starts
    # with **Workshop note.**
    out, skip = [], False
    for line in md.split('\n'):
        if line.startswith('> **Workshop note.**'):
            skip = True
            continue
        if skip:
            if line.startswith('>'):
                continue
            skip = False
        out.append(line)
    md = '\n'.join(out)
    md = re.sub(r'<!-- (BEGIN|END) GENERATED[^>]*-->\n?', '', md)
    # The title block and the hand-made contents: the book gets its own cover and contents.
    md = md[md.index('## About this book'):]
    # rules between chapters are page breaks in print
    md = re.sub(r'\n---\n', '\n', md)
    body = markdown.markdown(md, extensions=['tables', 'fenced_code', 'toc', 'sane_lists'],
                             extension_configs={'toc': {'permalink': False}})
    # code in tables stays on one line, unless it's long (the assembler's messages)
    body = re.sub(r'<code>([^<]{25,}?)</code>', r'<code class="w">\1</code>', body)
    # very wide tables (the 16 x 16 opcode grid) get a compact, fixed layout that wraps
    def grid(m):
        t = m.group(0)
        return t.replace('<table>', '<table class="grid">', 1) if t.count('<th') >= 12 else t
    body = re.sub(r'<table>.*?</table>', grid, body, flags=re.S)
    return body


def chapters(body):
    return re.findall(r'<h2 id="([^"]+)">(.*?)</h2>', body)


def page(body, toc_pages):
    font = lambda f: 'file://' + os.path.join(FONTS, f)
    rows = []
    for cid, title in chapters(body):
        t = html.unescape(re.sub('<[^>]+>', '', title))
        m = re.match(r'(\d+|Appendix [A-Z])\.?\s+(.*)', t)
        num, name = (m.group(1), m.group(2)) if m else ('', t)
        num = num.replace('Appendix ', '')
        rows.append(f'<li><a href="#{cid}"><span class="n">{html.escape(num)}</span>'
                    f'<span class="t">{html.escape(name)}</span><span class="dots"></span>'
                    f'<span class="p">{toc_pages.get(cid, "000")}</span></a></li>')
    # the handwritten note beside chapter 1, as in the game
    body = body.replace('<h2 id="1-getting-started">1. Getting started</h2>',
                        '<h2 id="1-getting-started">1. Getting started</h2>'
                        '<p class="uncle">start here.<br><span>—U</span></p>', 1)
    return f'''<!doctype html><html><head><meta charset="utf-8">
<title>UNC 64 Programmer's Reference Guide</title>
<style>
@font-face{{font-family:Baskerville;src:url({font("LibreBaskerville-Regular.ttf")})}}
@font-face{{font-family:Baskerville;font-weight:700;src:url({font("LibreBaskerville-Bold.ttf")})}}
@font-face{{font-family:Baskerville;font-style:italic;src:url({font("LibreBaskerville-Italic.ttf")})}}
@font-face{{font-family:VT323;src:url({font("VT323-Regular.ttf")})}}
@font-face{{font-family:CourierPrime;src:url({font("CourierPrime-Regular.ttf")})}}
@font-face{{font-family:CourierPrime;font-weight:700;src:url({font("CourierPrime-Bold.ttf")})}}
@font-face{{font-family:Anton;src:url({font("Anton-Regular.ttf")})}}
@font-face{{font-family:Caveat;src:url({font("Caveat-SemiBold.ttf")})}}
@page{{size:A4;margin:22mm 20mm 24mm 24mm;
  @bottom-center{{content:counter(page);font:10pt Baskerville;color:#5a4a38}}
  @top-right{{content:"UNC 64 PROGRAMMER'S REFERENCE GUIDE";font:11pt VT323;letter-spacing:1px;color:#8a7a62}}}}
@page cover{{margin:0;@bottom-center{{content:none}}@top-right{{content:none}}}}
@page front{{@bottom-center{{content:none}}@top-right{{content:none}}}}
html{{-webkit-print-color-adjust:exact;print-color-adjust:exact}}
body{{margin:0;font:9.6pt/1.55 Baskerville,serif;color:#241c14;hyphens:auto}}
.cover{{page:cover;height:296.5mm;width:100%;box-sizing:border-box;background:#e7dcc2;position:relative;overflow:hidden;break-after:page}}
.cover .band{{position:absolute;left:0;right:0;top:118mm;height:9mm;background:linear-gradient(90deg,#b23f31 0 50%,#e2b23f 50% 100%)}}
.cover .maker{{position:absolute;left:22mm;top:24mm;font:13pt VT323;letter-spacing:3px;color:#5a4a38}}
.cover h1{{position:absolute;left:20mm;top:40mm;margin:0;font:86pt/0.9 Anton;color:#241c14;letter-spacing:1px}}
.cover h1 span{{color:#b23f31}}
.cover .sub{{position:absolute;left:22mm;top:136mm;font:700 24pt/1.15 Baskerville;color:#241c14;width:150mm}}
.cover .covers{{position:absolute;left:22mm;top:170mm;font:15pt/1.4 VT323;letter-spacing:1px;color:#5a4a38;width:160mm}}
.cover .foot{{position:absolute;left:22mm;bottom:20mm;font:12pt VT323;letter-spacing:2px;color:#5a4a38}}
.front{{page:front}}
.verso{{break-after:page;padding-top:150mm;font-size:8.6pt;color:#5a4a38}}
.verso p{{margin:0 0 8pt}}
.toc{{break-after:page}}
.toc h2{{break-before:auto}}
.toc ol{{list-style:none;padding:0;margin:0}}
.toc li a{{display:flex;align-items:baseline;gap:8pt;text-decoration:none;color:#241c14;padding:2.6pt 0}}
.toc .n{{width:16pt;font:700 10pt Baskerville;color:#b23f31}}
.toc .t{{font-size:10.5pt}}
.toc .dots{{flex:1;border-bottom:1px dotted #a8977a;transform:translateY(-3pt)}}
.toc .p{{font:12pt VT323}}
h2{{break-before:page;font:700 21pt/1.15 Baskerville;margin:0 0 16pt;padding-bottom:8pt;border-bottom:2.5pt solid #241c14}}
h3{{font:700 12.5pt/1.25 Baskerville;margin:16pt 0 5pt;break-after:avoid}}
h4{{font:700 10.5pt Baskerville;margin:12pt 0 4pt;break-after:avoid}}
p{{margin:0 0 7pt;orphans:3;widows:3}}
a{{color:inherit;text-decoration:none}}
code{{font:9.4pt/1 CourierPrime;color:#3a2e22}}
pre{{background:#efe6d0;border-left:3pt solid #c9b88f;padding:6pt 10pt;margin:6pt 0 10pt;break-inside:avoid;white-space:pre-wrap}}
pre code{{font:700 8.8pt/1.4 CourierPrime;color:#241c14}}
table{{border-collapse:collapse;width:100%;margin:6pt 0 11pt;font-size:8.4pt;line-height:1.4}}
th{{text-align:left;font-weight:700;border-bottom:1.5pt solid #241c14;padding:3pt 5pt}}
td{{border-bottom:0.6pt solid #cdbf9f;padding:2.6pt 5pt;vertical-align:top}}
tr{{break-inside:avoid}}
thead{{display:table-header-group}}
td code,th code{{font-size:8.2pt}}
table.grid{{table-layout:fixed;font-size:6.4pt;line-height:1.25}}
table.grid th,table.grid td{{padding:2pt 1.5pt;overflow-wrap:anywhere}}
table.grid td:first-child,table.grid th:first-child{{width:14pt}}
td{{overflow-wrap:break-word}}
td code{{white-space:nowrap}}
td code.w{{white-space:normal}}
table.grid td code{{white-space:normal;font-size:6.2pt}}
ul,ol{{margin:0 0 8pt;padding-left:16pt}}
li{{margin:0 0 2.5pt}}
blockquote{{margin:6pt 0 10pt;padding:4pt 10pt;border-left:3pt solid #b23f31;background:#f6efdc}}
h2#1-getting-started{{position:relative}}
.uncle{{float:right;margin:-4pt 10pt 0 12pt;font:600 22pt/0.95 Caveat;color:#a8322a;transform:rotate(-5deg)}}
.uncle span{{display:block;padding-left:28pt}}
</style></head><body>
<section class="cover">
  <div class="maker">UNITED NORTHERN COMPUTERS</div>
  <h1>UNC <span>64</span></h1>
  <div class="band"></div>
  <div class="sub">Programmer's<br>Reference Guide</div>
  <div class="covers">Covers the UNC 16 and the UNC 64<br>UNC-1 processor · System ROM Rev A<br>UNC BASIC · the assembler · the memory map</div>
  <div class="foot">UNC64.COM</div>
</section>
<section class="front verso">
  <p><b>UNC 64 Programmer's Reference Guide.</b> Rev A.</p>
  <p>United Northern Computers is fictional. The UNC 64 is a fantasy 8-bit computer, emulated for real, and the computer in <i>UNC 64</i>, a game in development for Mac, PC and Linux by Viljami Kuosmanen.</p>
  <p>This book is the machine's specification: every program listing in it is assembled and run by the emulator's tests. It is free to download, print and share.</p>
  <p>unc64.com</p>
</section>
<section class="front toc"><h2>Contents</h2><ol>{''.join(rows)}</ol></section>
<main>{body}</main>
</body></html>'''


def render(html_text, pdf):
    with tempfile.NamedTemporaryFile('w', suffix='.html', delete=False, encoding='utf-8') as f:
        f.write(html_text)
        tmp = f.name
    if os.environ.get('UNC_KEEP_HTML'):
        open(os.environ['UNC_KEEP_HTML'], 'w', encoding='utf-8').write(html_text)
    try:
        subprocess.run(['node', os.path.join(HERE, 'render.js'), tmp, pdf], check=True)
    finally:
        os.unlink(tmp)


def chapter_pages(pdf, body):
    """The printed page number (counter(page)) each chapter starts on."""
    reader = PdfReader(pdf)
    lig = str.maketrans({'\ufb01': 'fi', '\ufb02': 'fl', '\ufb00': 'ff', '\ufb03': 'ffi', '\ufb04': 'ffl'})
    texts = [(p.extract_text() or '').translate(lig) for p in reader.pages]
    found = {}
    start = 0
    for cid, title in chapters(body):
        t = html.unescape(re.sub('<[^>]+>', '', title))
        key = re.sub(r'\s+', ' ', t).strip()
        for i in range(start, len(texts)):
            if key in re.sub(r'\s+', ' ', texts[i]) and i >= 3:
                found[cid] = i + 1
                start = i
                break
    return found


def main():
    out = sys.argv[1]
    body = manual_body()
    with tempfile.TemporaryDirectory() as d:
        first = os.path.join(d, 'pass1.pdf')
        render(page(body, {}), first)
        pages = chapter_pages(first, body)
    missing = [c for c, _ in chapters(body) if c not in pages]
    if missing:
        print('warning: no page found for', missing, file=sys.stderr)
    render(page(body, pages), out)
    n = len(PdfReader(out).pages)
    print(f'{out}: {n} pages, {os.path.getsize(out) // 1024} KB')


if __name__ == '__main__':
    main()
