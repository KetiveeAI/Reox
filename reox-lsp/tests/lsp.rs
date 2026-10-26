// Integration test: spawn the reox-lsp binary and speak LSP to it over pipes.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    /// Notifications received while waiting for a response.
    pending: Vec<Value>,
}

impl Client {
    fn spawn() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_reox-lsp"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn reox-lsp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Client { child, stdin: Some(stdin), stdout, next_id: 1, pending: Vec::new() }
    }

    fn stdin(&mut self) -> &mut ChildStdin {
        self.stdin.as_mut().expect("stdin still open")
    }

    fn send_raw(&mut self, body: &str) {
        write!(self.stdin(), "Content-Length: {}\r\n\r\n{}", body.len(), body).expect("write");
        self.stdin().flush().expect("flush");
    }

    fn send(&mut self, message: &Value) {
        let body = serde_json::to_string(message).expect("serialize");
        self.send_raw(&body);
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn read_message(&mut self) -> Value {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read header");
            assert!(n > 0, "server closed stdout");
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                content_length = Some(v.trim().parse::<usize>().expect("length"));
            }
        }
        let len = content_length.expect("Content-Length header");
        let mut body = vec![0u8; len];
        self.stdout.read_exact(&mut body).expect("body");
        serde_json::from_slice(&body).expect("json body")
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let msg = self.read_message();
            if msg.get("id") == Some(&json!(id)) {
                return msg;
            }
            self.pending.push(msg);
        }
    }

    /// Next notification with the given method, from the backlog or the pipe.
    fn wait_notification(&mut self, method: &str) -> Value {
        if let Some(i) = self.pending.iter().position(|m| m["method"] == method) {
            return self.pending.remove(i);
        }
        loop {
            let msg = self.read_message();
            if msg["method"] == method {
                return msg;
            }
            self.pending.push(msg);
        }
    }

    fn initialize(&mut self) -> Value {
        let resp = self.request("initialize", json!({ "processId": null, "rootUri": null, "capabilities": {} }));
        self.notify("initialized", json!({}));
        resp
    }

    fn open_fixture(&mut self, name: &str) -> (String, String) {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name].iter().collect();
        let text = std::fs::read_to_string(&path).expect("fixture");
        let uri = format!("file://{}", path.display());
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": uri, "languageId": "reox", "version": 1, "text": text } }),
        );
        (uri, text)
    }

    fn shutdown(mut self) -> i32 {
        let resp = self.request("shutdown", Value::Null);
        assert_eq!(resp["result"], Value::Null);
        self.notify("exit", Value::Null);
        self.stdin = None;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status.code().unwrap_or(-1);
            }
            assert!(Instant::now() < deadline, "server did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// (line, character) of the first occurrence of `needle` in `text`, plus `offset_in_needle`.
fn position_of(text: &str, needle: &str, offset_in_needle: usize) -> Value {
    let idx = text.find(needle).expect("needle in fixture") + offset_in_needle;
    let line = text[..idx].matches('\n').count();
    let line_start = text[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = text[line_start..idx].encode_utf16().count();
    json!({ "line": line, "character": character })
}

#[test]
fn initialize_reports_capabilities_and_exits_cleanly() {
    let mut client = Client::spawn();
    let resp = client.initialize();
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["definitionProvider"], true);
    assert_eq!(caps["documentSymbolProvider"], true);
    assert_eq!(caps["textDocumentSync"]["change"], 2);
    assert!(caps["completionProvider"].is_object());
    assert_eq!(resp["result"]["serverInfo"]["name"], "reox-lsp");
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn diagnostics_are_published_on_open_change_and_close() {
    let mut client = Client::spawn();
    client.initialize();
    let (uri, _) = client.open_fixture("broken.rx");

    let note = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(note["params"]["uri"], uri);
    let diags = note["params"]["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diags.len(), 2, "{:?}", diags);
    assert_eq!(diags[0]["severity"], 1);
    assert_eq!(diags[0]["source"], "reoxc");
    assert_eq!(diags[0]["range"]["start"]["line"], 1);
    assert!(diags[0]["message"].as_str().unwrap_or("").contains("int"), "{:?}", diags[0]);
    assert_eq!(diags[1]["range"]["start"]["line"], 2);

    // Replace the whole document with a valid program.
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [ { "text": "fn main() {\n    let s: int = 1;\n}\n" } ]
        }),
    );
    let note = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(note["params"]["version"], 2);
    assert_eq!(note["params"]["diagnostics"].as_array().map(Vec::len), Some(0));

    // Incremental edit that breaks it again: `int` -> `bool` on line 1.
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 },
            "contentChanges": [ {
                "range": { "start": { "line": 1, "character": 11 }, "end": { "line": 1, "character": 14 } },
                "text": "bool"
            } ]
        }),
    );
    let note = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(note["params"]["diagnostics"].as_array().map(Vec::len), Some(1));

    client.notify("textDocument/didClose", json!({ "textDocument": { "uri": uri } }));
    let note = client.wait_notification("textDocument/publishDiagnostics");
    assert_eq!(note["params"]["diagnostics"].as_array().map(Vec::len), Some(0));
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn parse_error_carries_hint_and_points_at_end_of_file() {
    let mut client = Client::spawn();
    client.initialize();
    let (_, text) = client.open_fixture("unclosed.rx.broken");
    let note = client.wait_notification("textDocument/publishDiagnostics");
    let diags = note["params"]["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diags.len(), 1);
    let message = diags[0]["message"].as_str().unwrap_or("");
    assert!(message.contains("expected '}'"), "{}", message);
    assert!(message.contains("hint:"), "{}", message);
    let last_line = text.trim_end_matches('\n').matches('\n').count() as u64 + 1;
    assert!(diags[0]["range"]["start"]["line"].as_u64().unwrap_or(0) <= last_line);
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn hover_shows_signature_type_and_doc_comment() {
    let mut client = Client::spawn();
    client.initialize();
    let (uri, text) = client.open_fixture("symbols.rx");

    let resp = client.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "add(p.x", 1) }),
    );
    let value = resp["result"]["contents"]["value"].as_str().expect("hover markdown");
    assert!(value.contains("fn add(a: int, b: int) -> int"), "{}", value);
    assert!(value.contains("Adds two integers."), "{}", value);
    assert_eq!(resp["result"]["contents"]["kind"], "markdown");

    let resp = client.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "let p = Point", 4) }),
    );
    let value = resp["result"]["contents"]["value"].as_str().expect("hover markdown");
    assert!(value.contains("let p: Point"), "{}", value);

    let resp = client.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "Point {", 0) }),
    );
    let value = resp["result"]["contents"]["value"].as_str().expect("hover markdown");
    assert!(value.contains("struct Point"), "{}", value);
    assert!(value.contains("A 2D point."), "{}", value);

    // Prelude externs hover with their signature from stdlib/prelude.rx.
    let resp = client.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "println(", 2) }),
    );
    let value = resp["result"]["contents"]["value"].as_str().expect("hover markdown");
    assert!(value.contains("extern fn println(s: string) -> void"), "{}", value);

    let resp = client.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 0 } }),
    );
    assert_eq!(resp["result"], Value::Null);
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn definition_jumps_to_declaration_names() {
    let mut client = Client::spawn();
    client.initialize();
    let (uri, text) = client.open_fixture("symbols.rx");

    let check = |client: &mut Client, needle: &str, off: usize, decl_needle: &str, decl_off: usize| {
        let resp = client.request(
            "textDocument/definition",
            json!({ "textDocument": { "uri": uri }, "position": position_of(&text, needle, off) }),
        );
        assert_eq!(resp["result"]["uri"], uri, "{}", needle);
        assert_eq!(resp["result"]["range"]["start"], position_of(&text, decl_needle, decl_off), "{}", needle);
    };
    check(&mut client, "add(p.x", 0, "fn add", 3);
    check(&mut client, "Point {", 0, "struct Point", 7);
    check(&mut client, "return total", 7, "let total", 4);
    check(&mut client, "a + b", 4, "fn add(a: int, b", 15);
    check(&mut client, "add(p.x", 4, "let p = ", 4);

    // A prelude extern has no in-file definition.
    let resp = client.request(
        "textDocument/definition",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "println(", 0) }),
    );
    assert_eq!(resp["result"], Value::Null);
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn document_symbols_form_an_outline() {
    let mut client = Client::spawn();
    client.initialize();
    let (uri, _) = client.open_fixture("symbols.rx");
    let resp = client.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": uri } }));
    let list = resp["result"].as_array().expect("symbol list");
    let names: Vec<&str> = list.iter().filter_map(|s| s["name"].as_str()).collect();
    assert_eq!(names, ["Point", "Shape", "Drawable", "add", "Button", "Main"]);

    let point = &list[0];
    assert_eq!(point["kind"], 23);
    let fields: Vec<&str> = point["children"].as_array().expect("fields").iter().filter_map(|c| c["name"].as_str()).collect();
    assert_eq!(fields, ["x", "y"]);

    let shape = &list[1];
    assert_eq!(shape["kind"], 10);
    assert_eq!(shape["children"][0]["kind"], 22);

    let button = &list[4];
    let members: Vec<&str> = button["children"].as_array().expect("members").iter().filter_map(|c| c["name"].as_str()).collect();
    assert_eq!(members, ["label", "clicked", "body"]);

    // Locals never appear in the outline.
    let add = &list[3];
    assert!(add.get("children").is_none(), "{:?}", add);
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn completion_offers_keywords_symbols_locals_and_prelude() {
    let mut client = Client::spawn();
    client.initialize();
    let (uri, text) = client.open_fixture("symbols.rx");
    let resp = client.request(
        "textDocument/completion",
        json!({ "textDocument": { "uri": uri }, "position": position_of(&text, "return total", 0) }),
    );
    let items = resp["result"]["items"].as_array().expect("items");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    for expected in ["fn", "let", "match", "add", "Point", "Shape", "Button", "total", "a", "b", "println", "print_int"] {
        assert!(labels.contains(&expected), "missing {}", expected);
    }
    assert!(!labels.contains(&"p"), "locals of other functions are not in scope");
    assert!(!labels.contains(&"x"), "struct fields are not bare names");
    let add_item = items.iter().find(|i| i["label"] == "add").expect("add item");
    assert_eq!(add_item["kind"], 3);
    assert_eq!(add_item["detail"], "fn add(a: int, b: int) -> int");
    assert_eq!(client.shutdown(), 0);
}

#[test]
fn malformed_input_gets_error_responses_without_killing_the_server() {
    let mut client = Client::spawn();
    client.initialize();

    client.send_raw("{this is not json");
    let msg = client.read_message();
    assert_eq!(msg["error"]["code"], -32700);

    client.send(&json!([1, 2, 3]));
    let msg = client.read_message();
    assert_eq!(msg["error"]["code"], -32600);

    client.send(&json!({ "jsonrpc": "2.0", "id": 1 }));
    let msg = client.read_message();
    assert_eq!(msg["error"]["code"], -32600);

    let resp = client.request("textDocument/references", json!({}));
    assert_eq!(resp["error"]["code"], -32601);

    let resp = client.request("textDocument/hover", json!({ "textDocument": { "uri": 7 } }));
    assert_eq!(resp["error"]["code"], -32602);

    // A header line without a colon is skipped; the following frame still works.
    client.stdin().write_all(b"garbage header line\r\n").expect("write");
    let resp = client.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": "file:///none.rx" } }));
    assert_eq!(resp["result"], json!([]));

    assert_eq!(client.shutdown(), 0);
}

#[test]
fn exit_without_shutdown_returns_nonzero() {
    let mut client = Client::spawn();
    client.initialize();
    client.notify("exit", Value::Null);
    let status = client.child.wait().expect("wait");
    assert_eq!(status.code(), Some(1));
}
