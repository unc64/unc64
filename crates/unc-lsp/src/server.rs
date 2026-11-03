//! The server's state (open documents and their analyses, settings) and the dispatch of LSP
//! messages to the features in `features`.

use crate::features;
use crate::text::{self, path_to_uri, uri_to_path, utf16_col};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use unc_asm::analysis::{Analysis, Diagnostic};

/// An open document and what the assembler made of it.
pub struct Doc {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub text: String,
    /// Where each line starts and ends in `text`, line ends left out.
    lines: Vec<(usize, usize)>,
    pub analysis: Analysis,
}

impl Doc {
    pub fn new(uri: String, text: String) -> Doc {
        let mut d = Doc { path: uri_to_path(&uri), uri, text: String::new(), lines: Vec::new(), analysis: Analysis::default() };
        d.set_text(text);
        d
    }

    pub fn set_text(&mut self, text: String) {
        self.lines = text::lines(&text).iter().map(|l| (l.as_ptr() as usize - text.as_ptr() as usize, l.len())).map(|(s, n)| (s, s + n)).collect();
        self.text = text;
    }

    pub fn line(&self, n: usize) -> &str {
        self.lines.get(n).map_or("", |&(s, e)| &self.text[s..e])
    }

    /// The folder `.include` names are read from: the document's own.
    pub fn dir(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }

    /// The URI of an included file (`Some(name)`) or of this document (`None`).
    pub fn file_uri(&self, file: Option<&str>) -> String {
        match (file, self.dir()) {
            (Some(name), Some(dir)) => path_to_uri(&dir.join(name)),
            _ => self.uri.clone(),
        }
    }
}

/// Settings a client can give in `initializationOptions` or `workspace/didChangeConfiguration`
/// (under `unc`).
#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// Inlay hints also show the cycles since the last label: `5c  Σ23`.
    pub cumulative_cycles: bool,
}

#[derive(Default)]
pub struct Server {
    docs: HashMap<String, Doc>,
    pub settings: Settings,
    snippets: bool,
    hint_refresh: bool,
    shutdown: bool,
    /// Set by `exit`: the process should end with this code.
    pub exit: Option<i32>,
    next_id: i64,
}

fn response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// The semantic token types and modifiers the server uses, in legend order.
pub const TOKEN_TYPES: &[&str] = &["keyword", "macro", "label", "variable", "number", "string", "comment", "operator", "function"];
pub const TOKEN_MODIFIERS: &[&str] = &["declaration", "readonly", "defaultLibrary"];

impl Server {
    pub fn shut_down(&self) -> bool {
        self.shutdown
    }

    pub fn doc(&self, uri: &str) -> Option<&Doc> {
        self.docs.get(uri)
    }

    /// Handles one message and returns what to send back: a response for a request, and any
    /// notifications (diagnostics) or requests (inlay hint refresh) it causes.
    pub fn handle(&mut self, msg: Value) -> Vec<Value> {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match msg.get("id").cloned() {
            Some(id) if !method.is_empty() => {
                if self.shutdown && method != "shutdown" {
                    return vec![error(id, -32600, "the server is shutting down")];
                }
                match self.request(&method, &params) {
                    Ok(result) => vec![response(id, result)],
                    Err((code, message)) => vec![error(id, code, &message)],
                }
            }
            // A response to one of our requests (an inlay hint refresh): nothing to do.
            Some(_) => Vec::new(),
            None => self.notify(&method, &params),
        }
    }

    fn request(&mut self, method: &str, p: &Value) -> Result<Value, (i64, String)> {
        let doc = || {
            let uri = p["textDocument"]["uri"].as_str().unwrap_or("");
            self.docs.get(uri).ok_or((-32602, format!("unknown document {uri}")))
        };
        let pos = || (p["position"]["line"].as_u64().unwrap_or(0) as usize, p["position"]["character"].as_u64().unwrap_or(0) as u32);
        Ok(match method {
            "initialize" => self.initialize(p),
            "shutdown" => {
                self.shutdown = true;
                Value::Null
            }
            "textDocument/completion" => features::completion(doc()?, pos(), self.snippets),
            "textDocument/hover" => features::hover(doc()?, pos()),
            "textDocument/inlayHint" => features::inlay_hints(doc()?, &p["range"], &self.settings),
            "textDocument/definition" => features::definition(self, doc()?, pos()),
            "textDocument/references" => features::references(self, doc()?, pos(), p["context"]["includeDeclaration"].as_bool().unwrap_or(true)),
            "textDocument/documentSymbol" => features::document_symbols(doc()?),
            "textDocument/prepareRename" => features::prepare_rename(doc()?, pos()),
            "textDocument/rename" => features::rename(self, doc()?, pos(), p["newName"].as_str().unwrap_or("")).map_err(|m| (-32602, m))?,
            "textDocument/semanticTokens/full" => features::semantic_tokens(doc()?),
            "textDocument/codeAction" => features::code_actions(doc()?, &p["range"]),
            "textDocument/foldingRange" => features::folding_ranges(doc()?),
            _ => return Err((-32601, format!("unhandled method {method}"))),
        })
    }

    fn initialize(&mut self, p: &Value) -> Value {
        let caps = &p["capabilities"];
        self.snippets = caps["textDocument"]["completion"]["completionItem"]["snippetSupport"].as_bool().unwrap_or(false);
        self.hint_refresh = caps["workspace"]["inlayHint"]["refreshSupport"].as_bool().unwrap_or(false);
        self.configure(&p["initializationOptions"]);
        json!({
            "capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": { "openClose": true, "change": 1 },
                "completionProvider": { "triggerCharacters": [".", "@", "#", "(", ","] },
                "hoverProvider": true,
                "inlayHintProvider": true,
                "definitionProvider": true,
                "referencesProvider": true,
                "documentSymbolProvider": true,
                "renameProvider": { "prepareProvider": true },
                "codeActionProvider": { "codeActionKinds": ["quickfix"] },
                "foldingRangeProvider": true,
                "semanticTokensProvider": {
                    "legend": { "tokenTypes": TOKEN_TYPES, "tokenModifiers": TOKEN_MODIFIERS },
                    "full": true
                }
            },
            "serverInfo": { "name": "unc-lsp", "version": env!("CARGO_PKG_VERSION") }
        })
    }

    /// Reads settings from `{cumulativeCycles}`, `{unc: {cumulativeCycles}}` or
    /// `{unc: {inlayHints: {cumulativeCycles}}}`.
    fn configure(&mut self, v: &Value) {
        for s in [v, &v["unc"], &v["inlayHints"], &v["unc"]["inlayHints"]] {
            if let Some(b) = s["cumulativeCycles"].as_bool() {
                self.settings.cumulative_cycles = b;
            }
        }
    }

    fn notify(&mut self, method: &str, p: &Value) -> Vec<Value> {
        let uri = p["textDocument"]["uri"].as_str().unwrap_or("").to_string();
        match method {
            "exit" => {
                self.exit = Some(if self.shutdown { 0 } else { 1 });
                Vec::new()
            }
            "textDocument/didOpen" => {
                let text = p["textDocument"]["text"].as_str().unwrap_or("").to_string();
                self.docs.insert(uri.clone(), Doc::new(uri, text));
                self.reanalyze()
            }
            "textDocument/didChange" => {
                // Full sync: the last change is the whole text.
                if let (Some(doc), Some(text)) = (self.docs.get_mut(&uri), p["contentChanges"].as_array().and_then(|c| c.last()).and_then(|c| c["text"].as_str())) {
                    doc.set_text(text.to_string());
                }
                self.reanalyze()
            }
            "textDocument/didClose" => {
                self.docs.remove(&uri);
                let mut out = vec![notification("textDocument/publishDiagnostics", json!({ "uri": uri, "diagnostics": [] }))];
                out.extend(self.reanalyze());
                out
            }
            "workspace/didChangeConfiguration" => {
                self.configure(&p["settings"]);
                if self.hint_refresh {
                    self.next_id += 1;
                    vec![json!({ "jsonrpc": "2.0", "id": format!("unc-{}", self.next_id), "method": "workspace/inlayHint/refresh" })]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// The text of a file a document includes: the editor's copy if it's open, else the disk's.
    fn read_include(&self, dir: Option<&Path>, name: &str) -> Option<String> {
        let path = dir?.join(name);
        let uri = path_to_uri(&path);
        match self.docs.get(&uri) {
            Some(d) => Some(d.text.clone()),
            None => std::fs::read_to_string(path).ok(),
        }
    }

    /// The text of the file `file` names, as seen from `doc`.
    pub fn file_text(&self, doc: &Doc, file: Option<&str>) -> Option<String> {
        match file {
            None => Some(doc.text.clone()),
            Some(name) => self.read_include(doc.dir(), name),
        }
    }

    /// Reassembles every open document (one may include another) and publishes diagnostics.
    fn reanalyze(&mut self) -> Vec<Value> {
        let uris: Vec<String> = self.docs.keys().cloned().collect();
        let mut out = Vec::new();
        for uri in uris {
            let doc = &self.docs[&uri];
            let analysis = match doc.path.is_some() {
                true => unc_asm::analyze_with(&doc.text, |name| self.read_include(doc.dir(), name)),
                false => unc_asm::analyze(&doc.text),
            };
            let doc = self.docs.get_mut(&uri).expect("open");
            doc.analysis = analysis;
            out.push(notification("textDocument/publishDiagnostics", json!({ "uri": uri, "diagnostics": diagnostics(doc) })));
        }
        out
    }
}

/// The document's errors and warnings as LSP diagnostics. One in an included file shows on
/// the `.include` line that brings it in, and points at the file's own line.
pub fn diagnostics(doc: &Doc) -> Vec<Value> {
    let a = &doc.analysis;
    let lang = unc_asm::lang::get();
    let lines = text::lines(&doc.text);
    let include_line = |name: &str| -> usize {
        let is_include = |l: &&str| l.trim_start().to_ascii_lowercase().contains(".include");
        lines.iter().position(|l| is_include(l) && l.contains(&format!("\"{name}\""))).or_else(|| lines.iter().position(is_include)).unwrap_or(0)
    };
    let one = |d: &Diagnostic| -> Value {
        let (line, start, end, message) = match &d.file {
            None => {
                let l = lines.get(d.line.wrapping_sub(1)).copied().unwrap_or("");
                (d.line.saturating_sub(1), utf16_col(l, d.start), utf16_col(l, d.end), d.message.clone())
            }
            Some(f) => {
                let n = include_line(f);
                let l = lines.get(n).copied().unwrap_or("");
                let s = l.len() - l.trim_start().len();
                (n, utf16_col(l, s), utf16_col(l, l.trim_end().len()), format!("{f}:{}: {}", d.line, d.message))
            }
        };
        let range = json!({ "start": { "line": line, "character": start }, "end": { "line": line, "character": end } });
        let mut related = Vec::new();
        if let Some(f) = &d.file {
            let r = json!({ "start": { "line": d.line.saturating_sub(1), "character": d.start }, "end": { "line": d.line.saturating_sub(1), "character": d.end } });
            related.push(json!({ "location": { "uri": doc.file_uri(Some(f)), "range": r }, "message": d.message }));
        }
        if let Some(m) = lang.explain(&d.message) {
            related.push(json!({ "location": { "uri": doc.uri, "range": range }, "message": format!("Appendix E: {}", m.meaning) }));
        }
        let mut v = json!({ "range": range, "severity": if d.warning { 2 } else { 1 }, "source": "unc-asm", "message": message });
        if !related.is_empty() {
            v["relatedInformation"] = Value::Array(related);
        }
        v
    };
    a.errors.iter().chain(&a.warnings).map(one).collect()
}
