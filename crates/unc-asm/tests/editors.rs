//! The editor files made from the language data must be what it makes now, so they can't
//! fall behind the assembler. After changing the language, regenerate them with
//! `UNC_UPDATE_EDITORS=1 cargo test -p unc-asm --test editors`.

fn check(path: &str, want: String) {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UNC_UPDATE_EDITORS").is_ok() {
        std::fs::write(&full, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&full).unwrap_or_default();
    assert!(have == want, "{path} is stale: run `UNC_UPDATE_EDITORS=1 cargo test -p unc-asm --test editors`");
}

#[test]
fn the_vim_syntax_file_is_current() {
    check("editors/vim/syntax/unc.vim", unc_asm::lang::vim_syntax());
}

#[test]
fn the_vscode_grammar_is_current() {
    check("editors/vscode/syntaxes/unc.tmLanguage.json", unc_asm::lang::textmate_grammar());
}

#[test]
fn the_keyword_lists_cover_the_language() {
    let vim = unc_asm::lang::vim_syntax();
    let tm = unc_asm::lang::textmate_grammar();
    let l = unc_asm::lang::get();
    for i in &l.instructions {
        assert!(vim.contains(&format!(" {}", i.mnemonic)) && tm.contains(i.mnemonic), "{}", i.mnemonic);
    }
    for d in &l.directives {
        assert!(vim.contains(&d.name[1..]) && tm.contains(&d.name[1..]), "{}", d.name);
    }
    for n in l.named() {
        assert!(vim.contains(n.name()) && tm.contains(n.name()), "{}", n.name());
    }
}
