// The parts of CodeMirror 6 that web/editor.js uses, re-exported from one file.
// Built into codemirror.js: see README.md.
export { EditorState, Compartment, StateField, StateEffect, RangeSet, Prec } from '@codemirror/state';
export {
  EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection,
  highlightSpecialChars, gutter, GutterMarker, hoverTooltip, Decoration,
} from '@codemirror/view';
export { StreamLanguage, HighlightStyle, syntaxHighlighting, bracketMatching } from '@codemirror/language';
export { tags } from '@lezer/highlight';
export { defaultKeymap, history, historyKeymap, insertTab, indentLess } from '@codemirror/commands';
export { autocompletion, completionKeymap, pickedCompletion } from '@codemirror/autocomplete';
export { linter, lintGutter, lintKeymap } from '@codemirror/lint';
export { search, searchKeymap, highlightSelectionMatches } from '@codemirror/search';
export { vim, Vim, getCM } from '@replit/codemirror-vim';
