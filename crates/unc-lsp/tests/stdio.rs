//! Drives the real `unc-lsp` binary over stdio with LSP messages, as an editor does, on a real
//! game source (games/snake.s): initialize, didOpen, diagnostics, completion, hover, inlay
//! hints, definition, references, symbols, rename, code actions, shutdown and exit.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: i64,
    /// Notifications that arrived while waiting for a response.
    notes: Vec<Value>,
}

impl Client {
    fn start(options: Value) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_unc-lsp")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("unc-lsp starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, stdin, stdout, next: 0, notes: Vec::new() };
        let caps = json!({
            "textDocument": { "completion": { "completionItem": { "snippetSupport": true } } },
            "workspace": { "inlayHint": { "refreshSupport": true } }
        });
        let init = c.request("initialize", json!({ "processId": null, "rootUri": null, "capabilities": caps, "initializationOptions": options }));
        assert_eq!(init["serverInfo"]["name"], "unc-lsp");
        assert_eq!(init["capabilities"]["inlayHintProvider"], true);
        c.notify("initialized", json!({}));
        c
    }

    fn send(&mut self, v: &Value) {
        let body = serde_json::to_vec(v).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut len = 0;
        loop {
            let mut h = String::new();
            assert!(self.stdout.read_line(&mut h).unwrap() > 0, "the server closed its output");
            let h = h.trim();
            if h.is_empty() {
                break;
            }
            if let Some(v) = h.strip_prefix("Content-Length:") {
                len = v.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; len];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let m = self.recv();
            if m["id"] == id {
                assert!(m.get("error").is_none(), "{method}: {m}");
                return m["result"].clone();
            }
            self.notes.push(m);
        }
    }

    /// The next diagnostics published for `uri`.
    fn diagnostics(&mut self, uri: &str) -> Vec<Value> {
        loop {
            if let Some(i) = self.notes.iter().position(|n| n["method"] == "textDocument/publishDiagnostics" && n["params"]["uri"] == uri) {
                return self.notes.remove(i)["params"]["diagnostics"].as_array().unwrap().clone();
            }
            let m = self.recv();
            self.notes.push(m);
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify("textDocument/didOpen", json!({ "textDocument": { "uri": uri, "languageId": "unc", "version": 1, "text": text } }));
    }

    fn at(&mut self, method: &str, uri: &str, line: usize, character: usize) -> Value {
        self.request(method, json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": character } }))
    }

    fn finish(mut self) {
        assert_eq!(self.request("shutdown", Value::Null), Value::Null);
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success(), "exit after shutdown is 0");
    }
}

fn uri(p: &Path) -> String {
    format!("file://{}", p.display())
}

fn snake() -> (PathBuf, String) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/snake.s").canonicalize().unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    (p, text)
}

/// (line, column) of the `n`th line containing `needle`, column at its start.
fn find(text: &str, needle: &str) -> (usize, usize) {
    text.lines().enumerate().find_map(|(i, l)| l.find(needle).map(|c| (i, c))).unwrap_or_else(|| panic!("no '{needle}'"))
}

#[test]
fn snake_over_stdio() {
    let (path, text) = snake();
    let u = uri(&path);
    let mut c = Client::start(json!({}));
    c.open(&u, &text);
    assert_eq!(c.diagnostics(&u), Vec::<Value>::new(), "snake.s assembles cleanly");

    // Hover on a mnemonic: the mode it assembled to, its bytes and its cost.
    let (l, col) = find(&text, "main:   LDA FRAME");
    let h = c.at("textDocument/hover", &u, l, col + 9);
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("**LDA** absolute: `LDA $nnnn`"), "{md}");
    assert!(md.contains("**4 cycles** · 3 bytes · opcode `$2B`"), "{md}");
    // On a constant: its value and the manual's register.
    let h = c.at("textDocument/hover", &u, l, col + 13);
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("**FRAME** constant = `$C006` = `49158`"), "{md}");
    assert!(md.contains("Frame counter"), "{md}");
    // On a number that is an I/O address.
    let (l2, col2) = find(&text, "BORDER  = $C020");
    let h = c.at("textDocument/hover", &u, l2, col2 + 11);
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("`$C020` = `49184`") && md.contains("**BORDER** `$C020`: I/O register"), "{md}");

    // Inlay hints: each instruction's cost, branches both ways, a .rept block's total.
    let hints = c.request("textDocument/inlayHint", json!({ "textDocument": { "uri": u }, "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 400, "character": 0 } } }));
    let label = |line: usize| hints.as_array().unwrap().iter().find(|h| h["position"]["line"] == line).map(|h| h["label"].as_str().unwrap().to_string());
    assert_eq!(label(l).as_deref(), Some("4c"));
    let (wait, _) = find(&text, "BEQ @wait");
    assert_eq!(label(wait).as_deref(), Some("2/3c"));
    let (rept, _) = find(&text, ".rept 4, page");
    assert_eq!(label(rept).as_deref(), Some("4 × 10 = 40 cycles"));
    assert_eq!(label(rept + 1).as_deref(), Some("5c ×4"));
    assert_eq!(label(find(&text, "SCREEN  = $0400").0), None, "a constant costs nothing");

    // Completion: mnemonics where a statement starts, symbols and the manual's names in an operand.
    let (l3, col3) = find(&text, "        JSR initchars");
    let items = c.at("textDocument/completion", &u, l3, col3 + 8)["items"].as_array().unwrap().clone();
    let has = |items: &[Value], label: &str| items.iter().any(|i| i["label"] == label);
    assert!(has(&items, "LDA") && has(&items, "LDA $nnnn,X") && has(&items, ".byte") && has(&items, ".rept"));
    let snippet = items.iter().find(|i| i["label"] == "LDA ($nn),Y").unwrap();
    assert_eq!(snippet["textEdit"]["newText"], "LDA (${1:fp}),Y");
    let items = c.at("textDocument/completion", &u, l3, col3 + 12)["items"].as_array().unwrap().clone();
    assert!(has(&items, "initchars") && has(&items, "FRAME") && has(&items, "sin"));
    let chrout = items.iter().find(|i| i["label"] == "CHROUT").expect("CHROUT, which snake.s doesn't define");
    assert_eq!(chrout["additionalTextEdits"][0]["newText"], "CHROUT  = $FF03\n");
    // Locals in scope only.
    let (l4, _) = find(&text, "BEQ @wait");
    let items = c.at("textDocument/completion", &u, l4, 13)["items"].as_array().unwrap().clone();
    // main has @wait; @title and @play belong to work, the label after it.
    assert!(has(&items, "@wait") && !has(&items, "@title") && !has(&items, "@copy"));

    // Definition and references of a label.
    let (l5, col5) = find(&text, "JSR pulse");
    let d = c.at("textDocument/definition", &u, l5, col5 + 5);
    assert_eq!(d["uri"], u);
    assert_eq!(d["range"]["start"]["line"], find(&text, "pulse:  LDA FRAME").0);
    let refs = c.request("textDocument/references", json!({ "textDocument": { "uri": u }, "position": { "line": l5, "character": col5 + 5 }, "context": { "includeDeclaration": true } }));
    assert_eq!(refs.as_array().unwrap().len(), text.matches("pulse").count());

    // The outline: constants, then labels with their locals inside.
    let syms = c.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": u } }));
    let main = syms.as_array().unwrap().iter().find(|s| s["name"] == "main").unwrap();
    assert_eq!(main["kind"], 12);
    assert!(main["children"].as_array().unwrap().iter().any(|s| s["name"] == "@wait"));

    // Rename a local: its definition and uses, nothing else.
    let edit = c.at("textDocument/prepareRename", &u, l4, 13);
    assert_eq!(edit["placeholder"], "@wait");
    let r = c.request("textDocument/rename", json!({ "textDocument": { "uri": u }, "position": { "line": l4, "character": 13 }, "newName": "@tick" }));
    assert_eq!(r["changes"][&u].as_array().unwrap().len(), 2);

    // Semantic tokens: five numbers per token.
    let st = c.request("textDocument/semanticTokens/full", json!({ "textDocument": { "uri": u } }));
    assert_eq!(st["data"].as_array().unwrap().len() % 5, 0);
    c.finish();
}

#[test]
fn errors_land_on_their_columns_and_a_quick_fix_defines_the_register() {
    let u = "file:///nowhere/test.s";
    let mut c = Client::start(json!({ "cumulativeCycles": true }));
    let text = "SCREEN  = $0400\n        .org $0800\nstart:  LDX #0\n@loop:  STA SCREEN,X\n        INX\n        BNE @loop\n        STA BORDER   ; no definition\n        LDQ #1\n        HLT\n";
    c.open(u, text);
    let d = c.diagnostics(u);
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(d[0]["message"], "undefined symbol 'BORDER'");
    assert_eq!(d[0]["range"], json!({ "start": { "line": 6, "character": 12 }, "end": { "line": 6, "character": 18 } }));
    assert_eq!(d[1]["message"], "unknown instruction 'LDQ'");
    assert_eq!(d[1]["range"]["start"]["character"], 8);
    assert!(d[1]["relatedInformation"][0]["message"].as_str().unwrap().starts_with("Appendix E: Not a UNC-1 mnemonic"));

    let fixes = c.request("textDocument/codeAction", json!({ "textDocument": { "uri": u }, "range": { "start": { "line": 6, "character": 0 }, "end": { "line": 6, "character": 0 } }, "context": { "diagnostics": d } }));
    let fix = &fixes[0];
    assert_eq!(fix["title"], "Define BORDER = $C020 (I/O register $C020)");
    assert_eq!(fix["edit"]["changes"][u][0], json!({ "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 0 } }, "newText": "BORDER  = $C020\n" }));

    // Cumulative cycles since the last label, and the cost of a line pass 2 couldn't encode.
    let hints = c.request("textDocument/inlayHint", json!({ "textDocument": { "uri": u }, "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 9, "character": 0 } } }));
    let labels: Vec<(i64, &str)> = hints.as_array().unwrap().iter().map(|h| (h["position"]["line"].as_i64().unwrap(), h["label"].as_str().unwrap())).collect();
    assert_eq!(labels, vec![(2, "2c  Σ2"), (3, "5c  Σ5"), (4, "2c  Σ7"), (5, "2/3c  Σ9+"), (6, "4c  Σ13+"), (8, "2c  Σ15+")]);

    // Fixed: no diagnostics.
    let fixed = text.replace("        LDQ #1\n", "").replace("SCREEN  = $0400\n", "SCREEN  = $0400\nBORDER  = $C020\n");
    c.notify("textDocument/didChange", json!({ "textDocument": { "uri": u, "version": 2 }, "contentChanges": [{ "text": fixed }] }));
    assert_eq!(c.diagnostics(u), Vec::<Value>::new());
    c.finish();
}

#[test]
fn includes_resolve_next_to_the_file() {
    let dir = std::env::temp_dir().join(format!("unc-lsp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.s"), "clear:  LDX #0\n@loop:  STA $0400,X\n        INX\n        BNE @loop\n        RTS\n").unwrap();
    let main = dir.join("main.s");
    let text = "        .org $0800\nstart:  JSR clear\n        HLT\n        .include \"lib.s\"\n";
    std::fs::write(&main, text).unwrap();
    let u = uri(&main);
    let mut c = Client::start(json!({}));
    c.open(&u, text);
    assert_eq!(c.diagnostics(&u), Vec::<Value>::new());
    let d = c.at("textDocument/definition", &u, 1, 12);
    assert_eq!(d["uri"], uri(&dir.join("lib.s")));
    assert_eq!(d["range"]["start"]["line"], 0);
    let d = c.at("textDocument/definition", &u, 3, 20);
    assert_eq!(d["uri"], uri(&dir.join("lib.s")));
    let h = c.at("textDocument/hover", &u, 1, 12);
    assert!(h["contents"]["value"].as_str().unwrap().contains("Defined at lib.s, line 1"));

    // An error in the included file shows on the .include line, pointing at the file.
    std::fs::write(dir.join("lib.s"), "clear:  LDQ\n").unwrap();
    c.notify("textDocument/didChange", json!({ "textDocument": { "uri": u, "version": 2 }, "contentChanges": [{ "text": text }] }));
    let d = c.diagnostics(&u);
    assert_eq!(d[0]["message"], "lib.s:1: unknown instruction 'LDQ'");
    assert_eq!(d[0]["range"]["start"]["line"], 3);
    assert_eq!(d[0]["relatedInformation"][0]["location"]["uri"], uri(&dir.join("lib.s")));
    c.finish();
    std::fs::remove_dir_all(&dir).ok();
}
