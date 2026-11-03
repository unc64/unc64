//! `unc-lsp`: the UNC assembly language server. Speaks LSP over stdin and stdout; an editor
//! starts it. `unc-lsp --version` prints the version.

use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args().nth(1).is_some_and(|a| a == "--version" || a == "-V") {
        println!("unc-lsp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    match unc_lsp::run(&mut stdin.lock(), &mut stdout.lock()) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("unc-lsp: {e}");
            ExitCode::FAILURE
        }
    }
}
