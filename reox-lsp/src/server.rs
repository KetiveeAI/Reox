// REOX Language Server - request dispatch.
//
// `Server::run` reads framed JSON-RPC messages until `exit`, keeps the open
// documents analysed, and answers the subset of LSP the REOX tooling needs.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use serde_json::{json, Map, Value};

use crate::analysis::{self, Analysis, SymKind, SymbolInfo, KEYWORDS};
use crate::document::{DocumentStore, Position, Range};
use crate::rpc::{self, FrameError, Incoming};

/// Bundled copy of the standard prelude, used for completion when no override
/// is configured. Kept in sync by the build (the file lives in reox-lang).
pub const BUNDLED_PRELUDE: &str = include_str!("../../reox-lang/stdlib/prelude.rx");

/// Server state across one connection.
pub struct Server {
    initialized: bool,
    shutdown_requested: bool,
    docs: DocumentStore,
    analyses: HashMap<String, Analysis>,
    prelude: Vec<SymbolInfo>,
    prelude_source: String,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of handling one message.
enum Step {
    Continue,
    Exit(i32),
}

impl Server {
    pub fn new() -> Self {
        let mut server = Self {
            initialized: false,
            shutdown_requested: false,
            docs: DocumentStore::default(),
            analyses: HashMap::new(),
            prelude: Vec::new(),
            prelude_source: String::new(),
        };
        let source = match std::env::var("REOX_PRELUDE") {
            Ok(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("reox-lsp: cannot read REOX_PRELUDE {}: {}; using bundled prelude", path, e);
                BUNDLED_PRELUDE.to_string()
            }),
            Err(_) => BUNDLED_PRELUDE.to_string(),
        };
        server.load_prelude(source);
        server
    }

    /// Replace the prelude symbols. A prelude that fails to parse yields no
    /// symbols (and a log line) rather than a failing server.
    pub fn load_prelude(&mut self, source: String) {
        let a = analysis::analyze(&source);
        if a.ast.is_none() {
            eprintln!(
                "reox-lsp: prelude did not parse ({}); prelude completion disabled",
                a.diagnostics.first().map(|d| d.message.clone()).unwrap_or_default()
            );
        }
        self.prelude = a.symbols.into_iter().filter(|s| s.is_global()).collect();
        self.prelude_source = source;
    }

    pub fn prelude_symbols(&self) -> &[SymbolInfo] {
        &self.prelude
    }

    /// Serve the connection. Returns the process exit code.
    pub fn run<R: BufRead, W: Write>(&mut self, reader: &mut R, writer: &mut W) -> i32 {
        loop {
            let message = match rpc::read_message(reader) {
                Ok(m) => m,
                Err(FrameError::Eof) => {
                    eprintln!("reox-lsp: client closed the connection");
                    return if self.shutdown_requested { 0 } else { 1 };
                }
                Err(FrameError::Truncated(m)) => {
                    eprintln!("reox-lsp: {}", m);
                    return 1;
                }
                Err(FrameError::Io(e)) => {
                    eprintln!("reox-lsp: i/o error: {}", e);
                    return 1;
                }
                Err(FrameError::BadHeader(m)) => {
                    eprintln!("reox-lsp: skipping malformed header: {}", m);
                    continue;
                }
                Err(FrameError::BadJson(m)) => {
                    let reply = rpc::error_response(&Value::Null, rpc::PARSE_ERROR, m);
                    if let Err(e) = rpc::write_message(writer, &reply) {
                        eprintln!("reox-lsp: cannot write: {}", e);
                        return 1;
                    }
                    continue;
                }
            };

            let mut outgoing = Vec::new();
            let step = self.handle(&message, &mut outgoing);
            for reply in &outgoing {
                if let Err(e) = rpc::write_message(writer, reply) {
                    eprintln!("reox-lsp: cannot write: {}", e);
                    return 1;
                }
            }
            if let Step::Exit(code) = step {
                return code;
            }
        }
    }

    /// Handle one message; replies and notifications are appended to `out`.
    /// Exposed for tests that drive the server without a transport.
    pub fn handle_message(&mut self, message: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        self.handle(message, &mut out);
        out
    }

    fn handle(&mut self, message: &Value, out: &mut Vec<Value>) -> Step {
        match rpc::classify(message) {
            Err((id, why)) => {
                out.push(rpc::error_response(&id, rpc::INVALID_REQUEST, why));
                Step::Continue
            }
            Ok(Incoming::Response { .. }) => Step::Continue,
            Ok(Incoming::Notification { method, params }) => self.notification(&method, &params, out),
            Ok(Incoming::Request { id, method, params }) => {
                let reply = self.request(&method, &params, out);
                out.push(match reply {
                    Ok(result) => rpc::response(&id, result),
                    Err((code, msg)) => rpc::error_response(&id, code, msg),
                });
                Step::Continue
            }
        }
    }

    fn notification(&mut self, method: &str, params: &Value, out: &mut Vec<Value>) -> Step {
        match method {
            "exit" => return Step::Exit(if self.shutdown_requested { 0 } else { 1 }),
            "initialized" => {}
            "textDocument/didOpen" => {
                if let Some(doc) = params.get("textDocument") {
                    let uri = doc.get("uri").and_then(Value::as_str);
                    let text = doc.get("text").and_then(Value::as_str);
                    let version = doc.get("version").and_then(Value::as_i64).unwrap_or(0);
                    if let (Some(uri), Some(text)) = (uri, text) {
                        self.docs.open(uri.to_string(), version, text.to_string());
                        self.reanalyze(uri, out);
                    }
                }
            }
            "textDocument/didChange" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let version = params
                    .get("textDocument")
                    .and_then(|d| d.get("version"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let changes = params.get("contentChanges").and_then(Value::as_array);
                if let (Some(uri), Some(changes)) = (uri, changes) {
                    let applied = match self.docs.get_mut(&uri) {
                        Some(doc) => doc.apply_changes(version, changes),
                        None => Err(format!("didChange for unopened document {}", uri)),
                    };
                    match applied {
                        Ok(()) => self.reanalyze(&uri, out),
                        Err(e) => eprintln!("reox-lsp: {}", e),
                    }
                }
            }
            "textDocument/didSave" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(uri) = uri {
                    if let Some(text) = params.get("text").and_then(Value::as_str) {
                        if let Some(doc) = self.docs.get_mut(&uri) {
                            let version = doc.version;
                            doc.set_text(version, text.to_string());
                        }
                    }
                    if self.docs.get(&uri).is_some() {
                        self.reanalyze(&uri, out);
                    }
                }
            }
            "textDocument/didClose" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(uri) = uri {
                    self.docs.close(&uri);
                    self.analyses.remove(&uri);
                    out.push(rpc::notification(
                        "textDocument/publishDiagnostics",
                        json!({ "uri": uri, "diagnostics": [] }),
                    ));
                }
            }
            // `$/` notifications and anything else are ignored by design.
            _ => {}
        }
        Step::Continue
    }

    fn request(
        &mut self,
        method: &str,
        params: &Value,
        out: &mut Vec<Value>,
    ) -> Result<Value, (i64, String)> {
        if method == "initialize" {
            return self.initialize(params);
        }
        if !self.initialized {
            return Err((rpc::SERVER_NOT_INITIALIZED, "server not initialized".to_string()));
        }
        match method {
            "shutdown" => {
                self.shutdown_requested = true;
                Ok(Value::Null)
            }
            "textDocument/hover" => self.hover(params),
            "textDocument/definition" => self.definition(params),
            "textDocument/documentSymbol" => self.document_symbol(params),
            "textDocument/completion" => self.completion(params),
            _ => {
                let _ = out;
                Err((rpc::METHOD_NOT_FOUND, format!("method not found: {}", method)))
            }
        }
    }

    fn initialize(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        if let Some(path) = params
            .get("initializationOptions")
            .and_then(|o| o.get("preludePath"))
            .and_then(Value::as_str)
        {
            match std::fs::read_to_string(path) {
                Ok(source) => self.load_prelude(source),
                Err(e) => eprintln!("reox-lsp: cannot read preludePath {}: {}", path, e),
            }
        }
        self.initialized = true;
        Ok(json!({
            "capabilities": {
                "textDocumentSync": {
                    "openClose": true,
                    "change": 2,
                    "save": { "includeText": false }
                },
                "hoverProvider": true,
                "definitionProvider": true,
                "documentSymbolProvider": true,
                "completionProvider": {
                    "triggerCharacters": ["."],
                    "resolveProvider": false
                }
            },
            "serverInfo": {
                "name": "reox-lsp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    // === Document lifecycle ===

    fn reanalyze(&mut self, uri: &str, out: &mut Vec<Value>) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let a = analysis::analyze(&doc.text);
        let diagnostics: Vec<Value> = a
            .diagnostics
            .iter()
            .map(|d| {
                let mut message = d.message.clone();
                if let Some(hint) = &d.hint {
                    message.push_str("\nhint: ");
                    message.push_str(hint);
                }
                json!({
                    "range": doc.span_range(&d.span).to_json(),
                    "severity": analysis::lsp_severity(d.severity),
                    "source": "reoxc",
                    "message": message,
                })
            })
            .collect();
        out.push(rpc::notification(
            "textDocument/publishDiagnostics",
            json!({ "uri": uri, "version": doc.version, "diagnostics": diagnostics }),
        ));
        self.analyses.insert(uri.to_string(), a);
    }

    /// Parse `{ textDocument: { uri }, position }` into (uri, position).
    fn text_position(params: &Value) -> Result<(String, Position), (i64, String)> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str)
            .ok_or_else(|| (rpc::INVALID_PARAMS, "missing textDocument.uri".to_string()))?;
        let position = params
            .get("position")
            .and_then(Position::from_json)
            .ok_or_else(|| (rpc::INVALID_PARAMS, "missing or malformed position".to_string()))?;
        Ok((uri.to_string(), position))
    }

    /// The word under the cursor and the symbol it resolves to.
    fn symbol_at(&self, uri: &str, pos: Position) -> Option<(Range, &SymbolInfo, bool)> {
        let doc = self.docs.get(uri)?;
        let a = self.analyses.get(uri)?;
        let (word, start, end) = doc.word_at(pos)?;
        let range = Range { start: doc.position(start), end: doc.position(end) };
        if let Some(sym) = a.resolve(&word, start) {
            return Some((range, sym, true));
        }
        let sym = self.prelude.iter().find(|s| s.name == word)?;
        Some((range, sym, false))
    }

    fn hover(&self, params: &Value) -> Result<Value, (i64, String)> {
        let (uri, pos) = Self::text_position(params)?;
        let Some((range, sym, _)) = self.symbol_at(&uri, pos) else {
            return Ok(Value::Null);
        };
        let mut value = format!("```reox\n{}\n```\n\n*{}*", sym.detail, sym.kind.label());
        if let Some(doc) = &sym.doc {
            value.push_str("\n\n");
            value.push_str(doc);
        }
        Ok(json!({
            "contents": { "kind": "markdown", "value": value },
            "range": range.to_json(),
        }))
    }

    fn definition(&self, params: &Value) -> Result<Value, (i64, String)> {
        let (uri, pos) = Self::text_position(params)?;
        let Some((_, sym, in_file)) = self.symbol_at(&uri, pos) else {
            return Ok(Value::Null);
        };
        if !in_file {
            // Prelude symbols have no document the client can open.
            return Ok(Value::Null);
        }
        let Some(doc) = self.docs.get(&uri) else {
            return Ok(Value::Null);
        };
        let range = Range { start: doc.position(sym.name_range.0), end: doc.position(sym.name_range.1) };
        Ok(json!({ "uri": uri, "range": range.to_json() }))
    }

    fn document_symbol(&self, params: &Value) -> Result<Value, (i64, String)> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str)
            .ok_or_else(|| (rpc::INVALID_PARAMS, "missing textDocument.uri".to_string()))?;
        let (Some(doc), Some(a)) = (self.docs.get(uri), self.analyses.get(uri)) else {
            return Ok(Value::Array(Vec::new()));
        };

        let to_json = |sym: &SymbolInfo, children: Vec<Value>| -> Value {
            let range = Range { start: doc.position(sym.range.0), end: doc.position(sym.range.1) };
            let selection = Range { start: doc.position(sym.name_range.0), end: doc.position(sym.name_range.1) };
            let mut obj = Map::new();
            obj.insert("name".into(), json!(sym.name));
            obj.insert("kind".into(), json!(sym.kind.lsp_symbol_kind()));
            obj.insert("detail".into(), json!(sym.detail));
            obj.insert("range".into(), range.to_json());
            obj.insert("selectionRange".into(), selection.to_json());
            if !children.is_empty() {
                obj.insert("children".into(), Value::Array(children));
            }
            Value::Object(obj)
        };

        // Locals and parameters stay out of the outline; members nest under
        // their declaring symbol.
        let outline_kind = |k: SymKind| !matches!(k, SymKind::Local | SymKind::Param);
        let mut result = Vec::new();
        for (i, sym) in a.symbols.iter().enumerate() {
            if sym.parent.is_some() || !outline_kind(sym.kind) {
                continue;
            }
            let children: Vec<Value> = a
                .symbols
                .iter()
                .filter(|c| c.parent == Some(i) && outline_kind(c.kind))
                .map(|c| to_json(c, Vec::new()))
                .collect();
            result.push(to_json(sym, children));
        }
        Ok(Value::Array(result))
    }

    fn completion(&self, params: &Value) -> Result<Value, (i64, String)> {
        let (uri, pos) = Self::text_position(params)?;
        let mut items: Vec<Value> = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        let mut push_symbol = |sym: &SymbolInfo, seen: &mut Vec<String>| {
            if seen.iter().any(|s| s == &sym.name) {
                return;
            }
            seen.push(sym.name.clone());
            let mut obj = Map::new();
            obj.insert("label".into(), json!(sym.name));
            obj.insert("kind".into(), json!(sym.kind.lsp_completion_kind()));
            obj.insert("detail".into(), json!(sym.detail));
            if let Some(doc) = &sym.doc {
                obj.insert("documentation".into(), json!({ "kind": "markdown", "value": doc }));
            }
            items.push(Value::Object(obj));
        };

        if let (Some(doc), Some(a)) = (self.docs.get(&uri), self.analyses.get(&uri)) {
            let offset = doc.offset(pos);
            for sym in a.visible_at(offset) {
                push_symbol(sym, &mut seen);
            }
        }
        for sym in &self.prelude {
            push_symbol(sym, &mut seen);
        }
        for kw in KEYWORDS {
            if seen.iter().any(|s| s == kw) {
                continue;
            }
            items.push(json!({ "label": kw, "kind": 14, "detail": "keyword" }));
        }
        Ok(json!({ "isIncomplete": false, "items": items }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(server: &mut Server) {
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
        assert_eq!(out.len(), 1);
        assert!(out[0]["result"]["capabilities"]["hoverProvider"].as_bool().unwrap_or(false));
    }

    fn open(server: &mut Server, uri: &str, text: &str) -> Vec<Value> {
        server.handle_message(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "languageId": "reox", "version": 1, "text": text } }
        }))
    }

    #[test]
    fn requests_before_initialize_are_rejected() {
        let mut server = Server::new();
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 5, "method": "textDocument/hover", "params": {}}));
        assert_eq!(out[0]["error"]["code"], rpc::SERVER_NOT_INITIALIZED);
    }

    #[test]
    fn unknown_method_is_method_not_found_and_unknown_notification_is_ignored() {
        let mut server = Server::new();
        init(&mut server);
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/rename", "params": {}}));
        assert_eq!(out[0]["error"]["code"], rpc::METHOD_NOT_FOUND);
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "method": "$/setTrace", "params": {}}));
        assert!(out.is_empty());
    }

    #[test]
    fn open_publishes_diagnostics_with_utf16_columns() {
        let mut server = Server::new();
        init(&mut server);
        let out = open(&mut server, "file:///t.rx", "fn main() {\n    let s: int = \"\u{1F600}x\";\n    let t: bool = 1;\n}\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], "textDocument/publishDiagnostics");
        let diags = out[0]["params"]["diagnostics"].as_array().expect("array");
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0]["severity"], 1);
        assert_eq!(diags[0]["source"], "reoxc");
        assert_eq!(diags[0]["range"]["start"]["line"], 1);
    }

    #[test]
    fn change_then_close_clears_diagnostics() {
        let mut server = Server::new();
        init(&mut server);
        open(&mut server, "file:///t.rx", "fn main() {\n");
        let out = server.handle_message(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///t.rx", "version": 2 },
                "contentChanges": [ { "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0} }, "text": "}\n" } ]
            }
        }));
        assert_eq!(out[0]["params"]["diagnostics"].as_array().map(Vec::len), Some(0));
        assert_eq!(out[0]["params"]["version"], 2);

        let out = server.handle_message(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///t.rx" } }
        }));
        assert_eq!(out[0]["params"]["diagnostics"].as_array().map(Vec::len), Some(0));
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 9, "method": "textDocument/documentSymbol", "params": {"textDocument": {"uri": "file:///t.rx"}}}));
        assert_eq!(out[0]["result"], json!([]));
    }

    #[test]
    fn hover_definition_and_symbols() {
        let mut server = Server::new();
        init(&mut server);
        let text = "/// Doubles.\nfn twice(n: int) -> int {\n    return n * 2;\n}\n\nfn main() {\n    let v = twice(4);\n}\n";
        open(&mut server, "file:///h.rx", text);

        let hover = server.handle_message(&json!({"jsonrpc": "2.0", "id": 3, "method": "textDocument/hover",
            "params": {"textDocument": {"uri": "file:///h.rx"}, "position": {"line": 6, "character": 13}}}));
        let value = hover[0]["result"]["contents"]["value"].as_str().expect("markdown");
        assert!(value.contains("fn twice(n: int) -> int"), "{}", value);
        assert!(value.contains("Doubles."), "{}", value);

        let def = server.handle_message(&json!({"jsonrpc": "2.0", "id": 4, "method": "textDocument/definition",
            "params": {"textDocument": {"uri": "file:///h.rx"}, "position": {"line": 6, "character": 13}}}));
        assert_eq!(def[0]["result"]["uri"], "file:///h.rx");
        assert_eq!(def[0]["result"]["range"]["start"], json!({"line": 1, "character": 3}));
        assert_eq!(def[0]["result"]["range"]["end"], json!({"line": 1, "character": 8}));

        // Hovering the local shows its inferred type.
        let hover = server.handle_message(&json!({"jsonrpc": "2.0", "id": 5, "method": "textDocument/hover",
            "params": {"textDocument": {"uri": "file:///h.rx"}, "position": {"line": 6, "character": 9}}}));
        assert!(hover[0]["result"]["contents"]["value"].as_str().unwrap_or("").contains("let v: int"));

        // Whitespace has no symbol.
        let hover = server.handle_message(&json!({"jsonrpc": "2.0", "id": 6, "method": "textDocument/hover",
            "params": {"textDocument": {"uri": "file:///h.rx"}, "position": {"line": 4, "character": 0}}}));
        assert_eq!(hover[0]["result"], Value::Null);

        let syms = server.handle_message(&json!({"jsonrpc": "2.0", "id": 7, "method": "textDocument/documentSymbol",
            "params": {"textDocument": {"uri": "file:///h.rx"}}}));
        let list = syms[0]["result"].as_array().expect("array");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["name"], "twice");
        assert_eq!(list[0]["kind"], 12);
        assert_eq!(list[0]["range"]["end"]["line"], 3);
    }

    #[test]
    fn prelude_symbols_hover_and_complete_but_have_no_definition() {
        let mut server = Server::new();
        init(&mut server);
        assert!(server.prelude_symbols().iter().any(|s| s.name == "println"));
        open(&mut server, "file:///p.rx", "fn main() {\n    println(\"hi\");\n}\n");
        let hover = server.handle_message(&json!({"jsonrpc": "2.0", "id": 3, "method": "textDocument/hover",
            "params": {"textDocument": {"uri": "file:///p.rx"}, "position": {"line": 1, "character": 6}}}));
        assert!(hover[0]["result"]["contents"]["value"].as_str().unwrap_or("").contains("extern fn println"));
        let def = server.handle_message(&json!({"jsonrpc": "2.0", "id": 4, "method": "textDocument/definition",
            "params": {"textDocument": {"uri": "file:///p.rx"}, "position": {"line": 1, "character": 6}}}));
        assert_eq!(def[0]["result"], Value::Null);

        let comp = server.handle_message(&json!({"jsonrpc": "2.0", "id": 5, "method": "textDocument/completion",
            "params": {"textDocument": {"uri": "file:///p.rx"}, "position": {"line": 1, "character": 4}}}));
        let items = comp[0]["result"]["items"].as_array().expect("items");
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"main"));
        assert!(labels.contains(&"println"));
        assert!(labels.contains(&"fn"));
        assert_eq!(labels.iter().filter(|l| **l == "println").count(), 1);
    }

    #[test]
    fn malformed_params_are_invalid_params_not_crashes() {
        let mut server = Server::new();
        init(&mut server);
        for method in ["textDocument/hover", "textDocument/definition", "textDocument/completion", "textDocument/documentSymbol"] {
            let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": {"textDocument": 5}}));
            assert_eq!(out[0]["error"]["code"], rpc::INVALID_PARAMS, "{}", method);
            let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 1, "method": method}));
            assert_eq!(out[0]["error"]["code"], rpc::INVALID_PARAMS, "{}", method);
        }
        // Positions on unopened documents are simply empty results.
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 1, "method": "textDocument/hover",
            "params": {"textDocument": {"uri": "file:///nope.rx"}, "position": {"line": 0, "character": 0}}}));
        assert_eq!(out[0]["result"], Value::Null);
        // Notifications with junk bodies are ignored.
        for method in ["textDocument/didOpen", "textDocument/didChange", "textDocument/didClose", "textDocument/didSave"] {
            let out = server.handle_message(&json!({"jsonrpc": "2.0", "method": method, "params": [1, 2]}));
            assert!(out.is_empty(), "{}", method);
        }
    }

    #[test]
    fn shutdown_then_exit_returns_zero() {
        let mut server = Server::new();
        init(&mut server);
        let out = server.handle_message(&json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}));
        assert_eq!(out[0]["result"], Value::Null);
        let mut sink = Vec::new();
        let body = r#"{"jsonrpc":"2.0","method":"exit"}"#;
        let mut input = std::io::Cursor::new(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes());
        assert_eq!(server.run(&mut input, &mut sink), 0);
    }

    #[test]
    fn run_survives_bad_json_and_bad_headers() {
        let mut server = Server::new();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let raw = format!(
            "Content-Length: 5\r\n\r\n{{abc}}garbage\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut input = std::io::Cursor::new(raw.into_bytes());
        let mut sink = Vec::new();
        let code = server.run(&mut input, &mut sink);
        assert_eq!(code, 1, "EOF without shutdown exits 1");
        let text = String::from_utf8_lossy(&sink);
        assert!(text.contains("-32700"), "{}", text);
        assert!(text.contains("\"capabilities\""), "{}", text);
    }
}
