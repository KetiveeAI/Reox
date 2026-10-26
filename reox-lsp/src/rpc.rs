// REOX Language Server - JSON-RPC 2.0 transport with LSP base-protocol framing.
//
// Messages are `Content-Length: N\r\n\r\n<N bytes of JSON>`. Everything here is
// hand written: no tower-lsp, no lsp-types.

use std::fmt;
use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

/// JSON-RPC error codes used by the server.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// LSP: request received before `initialize`.
pub const SERVER_NOT_INITIALIZED: i64 = -32002;

/// Hard cap on a single message body, so a corrupt header cannot make the
/// server allocate without bound.
pub const MAX_CONTENT_LENGTH: usize = 64 * 1024 * 1024;

/// Why a frame could not be read.
#[derive(Debug)]
pub enum FrameError {
    /// The peer closed the stream cleanly between messages.
    Eof,
    /// An I/O failure on the underlying stream.
    Io(io::Error),
    /// The stream ended in the middle of a message.
    Truncated(String),
    /// A header line was malformed; the reader skips it and keeps going.
    BadHeader(String),
    /// The body was not valid JSON (the connection is still in sync).
    BadJson(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Eof => f.write_str("end of input"),
            FrameError::Truncated(m) => write!(f, "truncated message: {}", m),
            FrameError::Io(e) => write!(f, "i/o error: {}", e),
            FrameError::BadHeader(m) => write!(f, "malformed header: {}", m),
            FrameError::BadJson(m) => write!(f, "malformed json: {}", m),
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e)
    }
}

/// Read one framed message body from `reader`.
///
/// Headers are read line by line until the blank line; `Content-Length` is
/// required and other headers (`Content-Type`) are ignored.
pub fn read_frame<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut content_length: Option<usize> = None;
    let mut saw_any_line = false;

    loop {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            if saw_any_line {
                return Err(FrameError::Truncated("stream ended inside headers".to_string()));
            }
            return Err(FrameError::Eof);
        }
        saw_any_line = true;

        // Strip the line terminator (accept both CRLF and bare LF).
        while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
            line.pop();
        }
        if line.is_empty() {
            break;
        }

        let text = String::from_utf8_lossy(&line);
        let Some((name, value)) = text.split_once(':') else {
            return Err(FrameError::BadHeader(format!("no ':' in {:?}", text)));
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            let parsed: usize = value
                .trim()
                .parse()
                .map_err(|_| FrameError::BadHeader(format!("bad Content-Length {:?}", value.trim())))?;
            if parsed > MAX_CONTENT_LENGTH {
                return Err(FrameError::BadHeader(format!(
                    "Content-Length {} exceeds the {} byte limit",
                    parsed, MAX_CONTENT_LENGTH
                )));
            }
            content_length = Some(parsed);
        }
    }

    let Some(len) = content_length else {
        return Err(FrameError::BadHeader("missing Content-Length".to_string()));
    };

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            FrameError::Truncated("stream ended inside message body".to_string())
        } else {
            FrameError::Io(e)
        }
    })?;
    Ok(body)
}

/// Read one frame and parse it as JSON.
pub fn read_message<R: BufRead>(reader: &mut R) -> Result<Value, FrameError> {
    let body = read_frame(reader)?;
    serde_json::from_slice(&body).map_err(|e| FrameError::BadJson(e.to_string()))
}

/// Write one JSON value as a framed message and flush.
pub fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// A decoded incoming message.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Request { id: Value, method: String, params: Value },
    Notification { method: String, params: Value },
    /// A response to a request the server sent (the server sends none today,
    /// but a client may still answer `window/showMessageRequest` style calls).
    Response { id: Value },
}

/// Classify a parsed JSON-RPC message. Returns the error to send back when the
/// message is not a valid JSON-RPC 2.0 object.
pub fn classify(message: &Value) -> Result<Incoming, (Value, String)> {
    let Some(obj) = message.as_object() else {
        return Err((Value::Null, "message is not a JSON object".to_string()));
    };
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let params = obj.get("params").cloned().unwrap_or(Value::Null);

    match obj.get("method").and_then(Value::as_str) {
        Some(method) => {
            if id.is_null() {
                Ok(Incoming::Notification { method: method.to_string(), params })
            } else if id.is_number() || id.is_string() {
                Ok(Incoming::Request { id, method: method.to_string(), params })
            } else {
                Err((Value::Null, "request id must be a number or string".to_string()))
            }
        }
        None => {
            if obj.contains_key("result") || obj.contains_key("error") {
                Ok(Incoming::Response { id })
            } else {
                Err((id, "message has neither method nor result".to_string()))
            }
        }
    }
}

/// Build a success response.
pub fn response(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Build an error response.
pub fn error_response(id: &Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() }
    })
}

/// Build a notification.
pub fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn framed(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    #[test]
    fn reads_a_well_formed_frame() {
        let mut cur = Cursor::new(framed(r#"{"jsonrpc":"2.0","id":1,"method":"x"}"#));
        let msg = read_message(&mut cur).expect("frame");
        assert_eq!(msg["method"], "x");
        assert!(matches!(read_message(&mut cur), Err(FrameError::Eof)));
    }

    #[test]
    fn accepts_extra_headers_and_bare_lf() {
        let body = r#"{"jsonrpc":"2.0","method":"n"}"#;
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\nContent-Length: {}\n\n{}",
            body.len(),
            body
        );
        let mut cur = Cursor::new(raw.into_bytes());
        let msg = read_message(&mut cur).expect("frame");
        assert_eq!(msg["method"], "n");
    }

    #[test]
    fn rejects_missing_or_bad_content_length() {
        let mut cur = Cursor::new(b"Foo: bar\r\n\r\n{}".to_vec());
        assert!(matches!(read_message(&mut cur), Err(FrameError::BadHeader(_))));

        let mut cur = Cursor::new(b"Content-Length: abc\r\n\r\n{}".to_vec());
        assert!(matches!(read_message(&mut cur), Err(FrameError::BadHeader(_))));

        let mut cur = Cursor::new(b"garbage line\r\n\r\n{}".to_vec());
        assert!(matches!(read_message(&mut cur), Err(FrameError::BadHeader(_))));

        let mut cur = Cursor::new(b"Content-Length: 99999999999\r\n\r\n{}".to_vec());
        assert!(matches!(read_message(&mut cur), Err(FrameError::BadHeader(_))));
    }

    #[test]
    fn rejects_truncated_body_and_bad_json() {
        let mut cur = Cursor::new(b"Content-Length: 10\r\n\r\n{}".to_vec());
        assert!(matches!(read_message(&mut cur), Err(FrameError::Truncated(_))));

        let mut cur = Cursor::new(framed("{not json"));
        assert!(matches!(read_message(&mut cur), Err(FrameError::BadJson(_))));
    }

    #[test]
    fn write_then_read_round_trips() {
        let mut out = Vec::new();
        write_message(&mut out, &json!({"jsonrpc": "2.0", "id": 7, "result": null})).expect("write");
        let mut cur = Cursor::new(out);
        let back = read_message(&mut cur).expect("read");
        assert_eq!(back["id"], 7);
    }

    #[test]
    fn classifies_messages() {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "a", "params": {"x": 1}});
        assert!(matches!(classify(&req), Ok(Incoming::Request { .. })));

        let note = json!({"jsonrpc": "2.0", "method": "b"});
        assert_eq!(
            classify(&note),
            Ok(Incoming::Notification { method: "b".to_string(), params: Value::Null })
        );

        let resp = json!({"jsonrpc": "2.0", "id": 3, "result": 1});
        assert_eq!(classify(&resp), Ok(Incoming::Response { id: json!(3) }));

        assert!(classify(&json!([1, 2])).is_err());
        assert!(classify(&json!({"jsonrpc": "2.0"})).is_err());
        assert!(classify(&json!({"id": {"a": 1}, "method": "x"})).is_err());
    }
}
