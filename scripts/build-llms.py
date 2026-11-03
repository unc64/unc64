#!/usr/bin/env python3
"""Builds llms-full.txt: the Programmer's Reference Guide and the cassette format as one
Markdown file for AI agents (linked from llms.txt). The manual goes in as the PDF prints
it: the workshop notes (blockquotes starting **Workshop note.**) and the generated-section
markers stripped. `crates/unc-asm/tests/manual.rs` fails when this file is stale.

    python3 scripts/build-llms.py
"""
import os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')


def strip_workshop_notes(md):
    """The manual as the PDF prints it. The same rules are in crates/unc-asm/tests/manual.rs."""
    out, skip = [], False
    for line in md.split('\n'):
        if line.startswith('> **Workshop note.**'):
            skip = True
            continue
        if skip:
            if line.startswith('>'):
                continue
            skip = False
            if line == '':  # and the blank line after it
                continue
        if line.startswith('<!-- BEGIN GENERATED') or line.startswith('<!-- END GENERATED'):
            continue
        out.append(line)
    md = '\n'.join(out)
    # a stripped note leaves two rules with nothing between them: keep one
    while '\n---\n\n---\n' in md:
        md = md.replace('\n---\n\n---\n', '\n---\n')
    return md


def full():
    read = lambda p: open(os.path.join(ROOT, p), encoding='utf-8').read()
    manual = strip_workshop_notes(read('docs/prg/programmers-reference-guide.md'))
    return (read('scripts/llms/full-header.md') + manual.rstrip('\n') + '\n\n---\n\n'
            + read('docs/cassettes.md').rstrip('\n') + '\n')


if __name__ == '__main__':
    out = os.path.join(ROOT, 'llms-full.txt')
    text = full()
    open(out, 'w', encoding='utf-8').write(text)
    print(f'llms-full.txt: {len(text.encode())} bytes')
