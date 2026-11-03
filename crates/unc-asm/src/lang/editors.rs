//! The editor files that are made from the language data, so they keep up with it: the Vim
//! syntax file (`editors/vim/syntax/unc.vim`) and the VS Code TextMate grammar
//! (`editors/vscode/syntaxes/unc.tmLanguage.json`). `cargo run -p unc-asm -- --vim-syntax`
//! and `-- --textmate` print them; `tests/editors.rs` checks the files in the repository are
//! what they print.

use super::get;
use crate::json::{Json, obj};

struct Words {
    mnemonics: Vec<String>,
    branches: Vec<String>,
    directives: Vec<String>,
    functions: Vec<String>,
    names: Vec<String>,
}

fn words() -> Words {
    let l = get();
    let mut directives: Vec<String> = l.directives.iter().flat_map(|d| std::iter::once(d.name.clone()).chain(d.aliases.iter().cloned())).map(|d| d[1..].to_string()).collect();
    directives.sort();
    let mut names: Vec<String> = l.named().map(|n| n.name().to_string()).collect();
    names.sort();
    names.dedup();
    Words {
        mnemonics: l.instructions.iter().filter(|i| !i.branch).map(|i| i.mnemonic.to_string()).collect(),
        branches: l.instructions.iter().filter(|i| i.branch).map(|i| i.mnemonic.to_string()).collect(),
        directives,
        functions: l.functions.iter().map(|f| f.name.to_string()).collect(),
        names,
    }
}

/// `syn keyword GROUP w1 w2 ...`, wrapped at about 100 columns.
fn keyword_lines(group: &str, words: &[String]) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for w in words {
        if line.len() + w.len() > 90 {
            out.push_str(&format!("syn keyword {group} {}\n", line.trim_end()));
            line.clear();
        }
        line.push_str(w);
        line.push(' ');
    }
    if !line.is_empty() {
        out.push_str(&format!("syn keyword {group} {}\n", line.trim_end()));
    }
    out
}

/// `syn match GROUP "\<\%(w1\|w2...\)\>"`, a line for every few words. A match, not a
/// keyword, so a definition (`BORDER = $C020`) still shows as one.
fn match_lines(group: &str, words: &[String]) -> String {
    let mut out = String::new();
    for chunk in words.chunks(10) {
        out.push_str(&format!("syn match {group} \"\\<\\%({}\\)\\>\"\n", chunk.join("\\|")));
    }
    out
}

/// The Vim syntax file for UNC assembly.
pub fn vim_syntax() -> String {
    let w = words();
    let lower = |v: &[String]| v.iter().map(|s| s.to_ascii_lowercase()).collect::<Vec<_>>();
    format!(
        r#"" Vim syntax file
" Language:    UNC assembly (the UNC 64's assembler: Programmer's Reference Guide, chapter 6)
" Maintainer:  the unc-64 repository, editors/vim
"
" Generated from unc_asm::lang by `cargo run -p unc-asm -- --vim-syntax`. Don't edit it by
" hand: crates/unc-asm/tests/editors.rs fails when it differs from what that prints.

if exists("b:current_syntax")
  finish
endif

let s:cpo_save = &cpo
set cpo&vim

" Mnemonics, directives, functions and registers are in any case; names aren't.
syn case ignore

{mnemonics}{branches}
syn match uncDirectiveBad "^\%(\s*@\=\h\w*:\)\=\s*\zs\.\h\w*"
syn match uncDirective "\.\%({directives}\)\>"
syn match uncFunction "\<\%({functions}\)\ze\s*("

" X and Y after the last comma, A alone after a shift or rotate.
syn match uncRegister "\%(,\s*\)\@<=[xy]\>\ze\s*\%(;\|$\)"
syn match uncRegister "\%(\<\%(asl\|lsr\|rol\|ror\)\s\+\)\@<=a\>\ze\s*\%(;\|$\)"

syn case match

syn match uncOperator "[-+*/%&|^~<>=!#\[\](),:]"
syn match uncPaste "\\\h\w*"

" The names the manual gives addresses (I/O registers, ROM routines, system variables).
" The UNC has no built-in symbol table: a program defines the ones it uses.
{names}
" Definitions: `name:` and `@name:` at the start of a line, `NAME = value` on its own.
syn match uncLabel "^\s*\zs\h\w*\%(\\\h\w*\)*\ze\s*:"
syn match uncLocal "@\h\w*"
syn match uncConstant "^\s*\zs\h\w*\ze\s*=[^=]"

syn match uncNumber "\$\x\+\>"
syn match uncNumber "%[01]\+\>"
syn match uncNumber "\<\d\+\>"
syn match uncChar "'\%(\\.\|[^\\']\)'"
syn region uncString start=+"+ skip=+\\.+ end=+"+ oneline
syn keyword uncTodo contained TODO FIXME XXX NOTE
syn match uncComment ";.*$" contains=uncTodo

hi def link uncMnemonic     Statement
hi def link uncBranch       Conditional
hi def link uncDirective    PreProc
hi def link uncDirectiveBad Error
hi def link uncFunction     Function
hi def link uncRegister     Type
hi def link uncOperator     Operator
hi def link uncPaste        Special
hi def link uncLabel        Label
hi def link uncLocal        Identifier
hi def link uncConstant     Constant
hi def link uncSystem       Special
hi def link uncNumber       Number
hi def link uncChar         Character
hi def link uncString       String
hi def link uncTodo         Todo
hi def link uncComment      Comment

let b:current_syntax = "unc"

let &cpo = s:cpo_save
unlet s:cpo_save
"#,
        mnemonics = keyword_lines("uncMnemonic", &w.mnemonics),
        branches = keyword_lines("uncBranch", &w.branches),
        directives = lower(&w.directives).join("\\|"),
        functions = w.functions.join("\\|"),
        names = match_lines("uncSystem", &w.names),
    )
}

/// The TextMate grammar for VS Code (and anything else that reads one), as JSON.
pub fn textmate_grammar() -> String {
    let w = words();
    let alt = |v: &[String]| v.join("|");
    let pat = |name: &str, re: String| obj! { "name" => name, "match" => re };
    let patterns = vec![
        obj! { "include" => "#comment" },
        obj! { "include" => "#string" },
        pat("constant.character.unc", r"'(?:\\.|[^\\'])'".into()),
        obj! {
            "match" => r"^\s*([A-Za-z_][A-Za-z0-9_]*(?:\\[A-Za-z_][A-Za-z0-9_]*)*)\s*(:)",
            "captures" => obj! { "1" => obj! { "name" => "entity.name.function.label.unc" }, "2" => obj! { "name" => "punctuation.separator.label.unc" } },
        },
        obj! {
            "match" => r"^\s*(@[A-Za-z_][A-Za-z0-9_]*)\s*(:)",
            "captures" => obj! { "1" => obj! { "name" => "entity.name.function.label.local.unc" }, "2" => obj! { "name" => "punctuation.separator.label.unc" } },
        },
        obj! {
            "match" => r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(=)(?!=)",
            "captures" => obj! { "1" => obj! { "name" => "entity.name.constant.unc" }, "2" => obj! { "name" => "keyword.operator.assignment.unc" } },
        },
        pat("variable.other.local.unc", r"@[A-Za-z_][A-Za-z0-9_]*".into()),
        obj! {
            "match" => r"(?i)\b(asl|lsr|rol|ror)\s+(a)\b(?=\s*(?:;|$))",
            "captures" => obj! { "1" => obj! { "name" => "keyword.other.mnemonic.unc" }, "2" => obj! { "name" => "variable.language.register.unc" } },
        },
        pat("keyword.control.directive.unc", format!(r"(?i)\.(?:{})\b", alt(&w.directives))),
        pat("invalid.illegal.directive.unc", r"(?:^|(?<=[\s:]))\.[A-Za-z_]\w*".into()),
        pat("keyword.control.branch.unc", format!(r"(?i)\b(?:{})\b", alt(&w.branches))),
        pat("keyword.other.mnemonic.unc", format!(r"(?i)\b(?:{})\b", alt(&w.mnemonics))),
        pat("support.function.unc", format!(r"(?i)\b(?:{})(?=\s*\()", alt(&w.functions))),
        obj! {
            "match" => r"(?i)(,)\s*([xy])\b(?=\s*(?:;|$))",
            "captures" => obj! { "1" => obj! { "name" => "punctuation.unc" }, "2" => obj! { "name" => "variable.language.register.unc" } },
        },
        pat("support.constant.unc", format!(r"\b(?:{})\b", alt(&w.names))),
        pat("constant.numeric.hex.unc", r"\$[0-9A-Fa-f]+\b".into()),
        pat("constant.numeric.binary.unc", r"(?<![\w)\]])%[01]+\b".into()),
        pat("constant.numeric.decimal.unc", r"\b[0-9]+\b".into()),
        pat("keyword.operator.unc", r"<<|>>|<=|>=|==|!=|&&|\|\||[-+*/%&|^~<>=!#\\]".into()),
        pat("punctuation.unc", r"[\[\](),]".into()),
    ];
    let g = obj! {
        "$schema" => "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
        "name" => "UNC assembly",
        "scopeName" => "source.unc",
        "comment" => "Generated from unc_asm::lang by `cargo run -p unc-asm -- --textmate`. Don't edit by hand.",
        "patterns" => Json::Arr(patterns),
        "repository" => obj! {
            "comment" => obj! { "name" => "comment.line.semicolon.unc", "match" => ";.*$" },
            "string" => obj! {
                "name" => "string.quoted.double.unc",
                "begin" => "\"",
                "end" => "\"|$",
                "patterns" => Json::Arr(vec![obj! { "name" => "constant.character.escape.unc", "match" => r#"\\["'\\]"# }, obj! { "name" => "invalid.illegal.escape.unc", "match" => r"\\." }]),
            },
        },
    };
    g.pretty()
}
