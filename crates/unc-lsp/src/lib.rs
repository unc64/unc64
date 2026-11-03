//! A language server for UNC assembly (the UNC 64's assembler, manual chapter 6), over stdio.
//!
//! Everything it knows comes from the real assembler (`unc_asm::analysis`) and the language
//! data (`unc_asm::lang`): diagnostics, completion, hover, cycle-count inlay hints,
//! definitions, references, the outline, rename, semantic tokens and a quick fix that
//! defines an I/O register or ROM routine the program uses. A tool outside the machine: it
//! changes nothing about the CPU, its timing or the emulation.

pub mod features;
pub mod rpc;
pub mod server;
pub mod text;

pub use server::Server;

use std::io::{BufRead, Write};

/// Serves one client: reads requests from `input` and writes replies to `output` until the
/// client says `exit` or the input ends. Returns the exit code LSP asks for: 0 if the client
/// sent `shutdown` first, else 1.
pub fn run(input: &mut impl BufRead, output: &mut impl Write) -> std::io::Result<i32> {
    let mut server = Server::default();
    while let Some(msg) = rpc::read_message(input)? {
        // A bug in one feature mustn't take the editor's server down: answer with an error.
        let id = msg.get("id").cloned();
        let replies = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.handle(msg))).unwrap_or_else(|_| match id {
            Some(id) => vec![serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": "unc-lsp: internal error" } })],
            None => Vec::new(),
        });
        for reply in replies {
            rpc::write_message(output, &reply)?;
        }
        if let Some(code) = server.exit {
            return Ok(code);
        }
    }
    Ok(if server.shut_down() { 0 } else { 1 })
}
