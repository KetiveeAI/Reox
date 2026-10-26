// REOX Language Server - open document store and position mapping.
//
// LSP positions are zero-based lines and UTF-16 code unit columns. The compiler
// reports one-based lines and columns counted in characters, plus byte offsets.
// `LineIndex` converts between the three.

use std::collections::HashMap;

use serde_json::{json, Value};

/// A zero-based LSP position (line, UTF-16 column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    pub fn to_json(self) -> Value {
        json!({ "line": self.line, "character": self.character })
    }

    /// Parse `{ "line": n, "character": m }`; `None` when the shape is wrong.
    pub fn from_json(value: &Value) -> Option<Self> {
        let line = value.get("line")?.as_u64()?;
        let character = value.get("character")?.as_u64()?;
        Some(Self {
            line: u32::try_from(line).ok()?,
            character: u32::try_from(character).ok()?,
        })
    }
}

/// A half-open LSP range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn to_json(self) -> Value {
        json!({ "start": self.start.to_json(), "end": self.end.to_json() })
    }

    pub fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            start: Position::from_json(value.get("start")?)?,
            end: Position::from_json(value.get("end")?)?,
        })
    }

    pub fn contains(&self, pos: Position) -> bool {
        pos >= self.start && pos <= self.end
    }
}

/// Byte offsets of every line start, for offset <-> position conversion.
#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts, len: text.len() }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Byte range of a zero-based line, excluding the newline.
    fn line_bounds(&self, line: usize) -> Option<(usize, usize)> {
        let start = *self.line_starts.get(line)?;
        let end = self.line_starts.get(line + 1).map(|s| s - 1).unwrap_or(self.len);
        Some((start, end.max(start)))
    }

    /// Convert a byte offset into an LSP position (UTF-16 columns).
    pub fn position(&self, text: &str, offset: usize) -> Position {
        let offset = offset.min(text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l.saturating_sub(1),
        };
        let line_start = self.line_starts.get(line).copied().unwrap_or(0);
        let slice = text.get(line_start..offset).unwrap_or("");
        let character: usize = slice.chars().map(char::len_utf16).sum();
        Position::new(line as u32, character as u32)
    }

    /// Convert an LSP position into a byte offset. Positions past the end of a
    /// line clamp to the line end; lines past the end clamp to the text end.
    pub fn offset(&self, text: &str, pos: Position) -> usize {
        let Some((start, end)) = self.line_bounds(pos.line as usize) else {
            return text.len();
        };
        let line_text = text.get(start..end).unwrap_or("");
        let mut units = 0u32;
        for (i, ch) in line_text.char_indices() {
            if units >= pos.character {
                return start + i;
            }
            units += ch.len_utf16() as u32;
        }
        end
    }

    /// Map a one-based compiler (line, column) in characters to a byte offset.
    pub fn offset_of_line_col(&self, text: &str, line: u32, column: u32) -> usize {
        if line == 0 {
            return text.len();
        }
        let Some((start, end)) = self.line_bounds(line as usize - 1) else {
            return text.len();
        };
        let line_text = text.get(start..end).unwrap_or("");
        let col = column.max(1) as usize - 1;
        line_text
            .char_indices()
            .nth(col)
            .map(|(i, _)| start + i)
            .unwrap_or(end)
    }
}

/// One open text document.
#[derive(Debug, Clone)]
pub struct Document {
    pub uri: String,
    pub version: i64,
    pub text: String,
    pub index: LineIndex,
}

impl Document {
    pub fn new(uri: String, version: i64, text: String) -> Self {
        let index = LineIndex::new(&text);
        Self { uri, version, text, index }
    }

    pub fn set_text(&mut self, version: i64, text: String) {
        self.index = LineIndex::new(&text);
        self.text = text;
        self.version = version;
    }

    /// Apply an incremental `TextDocumentContentChangeEvent` list in order.
    /// An event without a range replaces the whole text.
    pub fn apply_changes(&mut self, version: i64, changes: &[Value]) -> Result<(), String> {
        for change in changes {
            let new_text = change
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| "content change without text".to_string())?;
            match change.get("range") {
                Some(range) => {
                    let range = Range::from_json(range).ok_or_else(|| "malformed range".to_string())?;
                    let start = self.index.offset(&self.text, range.start);
                    let end = self.index.offset(&self.text, range.end).max(start);
                    let mut text = String::with_capacity(self.text.len() + new_text.len());
                    text.push_str(&self.text[..start]);
                    text.push_str(new_text);
                    text.push_str(&self.text[end..]);
                    self.index = LineIndex::new(&text);
                    self.text = text;
                }
                None => {
                    self.index = LineIndex::new(new_text);
                    self.text = new_text.to_string();
                }
            }
        }
        self.version = version;
        Ok(())
    }

    pub fn position(&self, offset: usize) -> Position {
        self.index.position(&self.text, offset)
    }

    pub fn offset(&self, pos: Position) -> usize {
        self.index.offset(&self.text, pos)
    }

    /// LSP range of a compiler span. Spans whose byte range is empty get a
    /// one-character range so editors still draw a squiggle.
    pub fn span_range(&self, span: &reoxc::Span) -> Range {
        let (start, end) = if span.start < span.end && span.end <= self.text.len() {
            (span.start, span.end)
        } else {
            // Fall back to line/column (unterminated string and EOF errors
            // sometimes carry a degenerate byte range).
            let start = if span.line > 0 {
                self.index.offset_of_line_col(&self.text, span.line, span.column)
            } else {
                span.start.min(self.text.len())
            };
            let next = self.text[start..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            (start, start + next)
        };
        Range { start: self.position(start), end: self.position(end) }
    }

    /// The identifier word around a position, with its byte range.
    pub fn word_at(&self, pos: Position) -> Option<(String, usize, usize)> {
        let offset = self.offset(pos);
        let bytes = self.text.as_bytes();
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let mut start = offset;
        while start > 0 && is_word(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = offset;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some((self.text[start..end].to_string(), start, end))
    }
}

/// All open documents keyed by URI.
#[derive(Debug, Default)]
pub struct DocumentStore {
    docs: HashMap<String, Document>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: String, version: i64, text: String) {
        self.docs.insert(uri.clone(), Document::new(uri, version, text));
    }

    pub fn close(&mut self, uri: &str) -> Option<Document> {
        self.docs.remove(uri)
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn get_mut(&mut self, uri: &str) -> Option<&mut Document> {
        self.docs.get_mut(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_and_offset_round_trip_ascii() {
        let text = "ab\ncd\n";
        let idx = LineIndex::new(text);
        assert_eq!(idx.position(text, 0), Position::new(0, 0));
        assert_eq!(idx.position(text, 2), Position::new(0, 2));
        assert_eq!(idx.position(text, 3), Position::new(1, 0));
        assert_eq!(idx.position(text, 6), Position::new(2, 0));
        assert_eq!(idx.offset(text, Position::new(1, 1)), 4);
        assert_eq!(idx.offset(text, Position::new(1, 99)), 5);
        assert_eq!(idx.offset(text, Position::new(99, 0)), 6);
    }

    #[test]
    fn columns_count_utf16_units() {
        // U+1F600 is 4 bytes in UTF-8 and 2 UTF-16 units; 'e' with accent is 2 bytes, 1 unit.
        let text = "x\u{1F600}y\u{e9}z";
        let idx = LineIndex::new(text);
        assert_eq!(idx.position(text, 5), Position::new(0, 3)); // before 'y'
        assert_eq!(idx.position(text, 8), Position::new(0, 5)); // before 'z'
        assert_eq!(idx.offset(text, Position::new(0, 3)), 5);
        assert_eq!(idx.offset(text, Position::new(0, 5)), 8);
    }

    #[test]
    fn compiler_line_col_maps_to_offset() {
        let text = "fn main() {\n    let x = 1;\n}\n";
        let idx = LineIndex::new(text);
        assert_eq!(idx.offset_of_line_col(text, 2, 9), 20);
        assert_eq!(&text[20..21], "x");
        assert_eq!(idx.offset_of_line_col(text, 0, 0), text.len());
    }

    #[test]
    fn incremental_changes_apply_in_order() {
        let mut doc = Document::new("file:///a.rx".into(), 1, "let a = 1;\n".into());
        let changes = vec![
            json!({"range": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 5}}, "text": "bb"}),
            json!({"range": {"start": {"line": 0, "character": 9}, "end": {"line": 0, "character": 10}}, "text": "22"}),
        ];
        doc.apply_changes(2, &changes).expect("apply");
        assert_eq!(doc.text, "let bb = 22;\n");
        assert_eq!(doc.version, 2);

        doc.apply_changes(3, &[json!({"text": "x"})]).expect("full");
        assert_eq!(doc.text, "x");
        assert!(doc.apply_changes(4, &[json!({"range": 5, "text": ""})]).is_err());
        assert!(doc.apply_changes(4, &[json!({})]).is_err());
    }

    #[test]
    fn word_at_finds_identifier_boundaries() {
        let doc = Document::new("u".into(), 1, "let count_1 = x;".into());
        assert_eq!(doc.word_at(Position::new(0, 6)), Some(("count_1".into(), 4, 11)));
        assert_eq!(doc.word_at(Position::new(0, 11)), Some(("count_1".into(), 4, 11)));
        assert_eq!(doc.word_at(Position::new(0, 12)), None);
    }

    #[test]
    fn span_range_falls_back_to_line_col_for_empty_spans() {
        let doc = Document::new("u".into(), 1, "fn main() {\n".into());
        let r = doc.span_range(&reoxc::Span::new(1, 11, 0, 0));
        assert_eq!(r.start, Position::new(0, 10));
        assert_eq!(r.end, Position::new(0, 11));
    }
}
