// REOX Compiler - Diagnostics
// One diagnostic type shared by every pipeline stage, rendered as
// `file:line:col: severity: message` with the source line and a caret.

use std::fmt;

use crate::lexer::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
            Severity::Note => f.write_str("note"),
        }
    }
}

/// A single compiler diagnostic produced by any stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self::new(Severity::Error, message, span)
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self::new(Severity::Warning, message, span)
    }

    pub fn note(message: impl Into<String>, span: Span) -> Self {
        Self::new(Severity::Note, message, span)
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// The `file:line:col: severity: message` header line.
    pub fn header(&self, file: &str) -> String {
        format!(
            "{}:{}:{}: {}: {}",
            file, self.span.line, self.span.column, self.severity, self.message
        )
    }

    /// Render the diagnostic with the offending source line and a caret under the span.
    /// Lines that cannot be located in `source` (for example line 0) render the header only.
    pub fn render(&self, file: &str, source: &str) -> String {
        let mut out = self.header(file);

        if let Some(line_text) = source_line(source, self.span.line) {
            let line_chars = line_text.chars().count();
            let col = (self.span.column.max(1) as usize - 1).min(line_chars);

            // Keep tabs in the padding so the caret stays aligned with tabbed source.
            let padding: String = line_text
                .chars()
                .take(col)
                .map(|c| if c == '\t' { '\t' } else { ' ' })
                .collect();

            let span_len = self.span.end.saturating_sub(self.span.start).max(1);
            let caret_len = span_len.min(line_chars.saturating_sub(col).max(1));
            let carets = "^".repeat(caret_len);

            out.push('\n');
            out.push_str("    ");
            out.push_str(line_text);
            out.push('\n');
            out.push_str("    ");
            out.push_str(&padding);
            out.push_str(&carets);
        }

        if let Some(hint) = &self.hint {
            out.push('\n');
            out.push_str("    hint: ");
            out.push_str(hint);
        }

        out
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.span.line, self.span.column, self.severity, self.message
        )
    }
}

/// Render every diagnostic, one after another, separated by blank lines.
pub fn render_all(file: &str, source: &str, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.render(file, source))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Fetch a 1-based source line, without its line terminator.
fn source_line(source: &str, line: u32) -> Option<&str> {
    if line == 0 {
        return None;
    }
    source
        .lines()
        .nth(line as usize - 1)
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
}

// === Conversions from the per-stage error types ===

impl From<crate::lexer::LexError> for Diagnostic {
    fn from(e: crate::lexer::LexError) -> Self {
        Diagnostic::error(e.message, e.span)
    }
}

impl From<crate::parser::ParseError> for Diagnostic {
    fn from(e: crate::parser::ParseError) -> Self {
        let mut d = Diagnostic::error(e.message, e.span);
        d.hint = e.hint;
        d
    }
}

impl From<crate::typechecker::TypeError> for Diagnostic {
    fn from(e: crate::typechecker::TypeError) -> Self {
        let diag = Diagnostic::error(e.message, e.span);
        match e.hint {
            Some(hint) => diag.with_hint(hint),
            None => diag,
        }
    }
}

impl From<crate::codegen::CodegenError> for Diagnostic {
    fn from(e: crate::codegen::CodegenError) -> Self {
        Diagnostic::error(e.message, e.span)
    }
}

impl From<crate::interpreter::RuntimeError> for Diagnostic {
    fn from(e: crate::interpreter::RuntimeError) -> Self {
        Diagnostic::error(format!("runtime error: {}", e.message), e.span.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(line: u32, column: u32, start: usize, end: usize) -> Span {
        Span::new(line, column, start, end)
    }

    #[test]
    fn renders_header_source_and_caret() {
        let source = "fn main() {\n    let x = ;\n}\n";
        let d = Diagnostic::error("expected expression", span(2, 13, 24, 25));
        let rendered = d.render("main.rx", source);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "main.rx:2:13: error: expected expression");
        assert_eq!(lines[1], "        let x = ;");
        assert_eq!(lines[2], "                ^");
    }

    #[test]
    fn renders_multi_char_caret_clamped_to_line() {
        let source = "let abc = 1;";
        let d = Diagnostic::error("bad name", span(1, 5, 4, 7));
        let rendered = d.render("f.rx", source);
        assert!(rendered.ends_with("    ^^^"), "{}", rendered);

        // A span that runs past the end of the line is clamped.
        let d = Diagnostic::error("too long", span(1, 11, 10, 100));
        let rendered = d.render("f.rx", source);
        assert!(rendered.ends_with("    ^^"), "{}", rendered);
    }

    #[test]
    fn renders_hint_and_skips_source_for_unknown_line() {
        let d = Diagnostic::error("unexpected end of file", span(0, 0, 5, 5)).with_hint("add '}'");
        let rendered = d.render("f.rx", "fn x() {");
        assert_eq!(rendered, "f.rx:0:0: error: unexpected end of file\n    hint: add '}'");
    }

    #[test]
    fn out_of_range_line_renders_header_only() {
        let d = Diagnostic::warning("late", span(99, 1, 0, 1));
        assert_eq!(d.render("f.rx", "one line"), "f.rx:99:1: warning: late");
    }

    #[test]
    fn column_past_end_of_line_puts_caret_after_last_char() {
        let source = "let x";
        let d = Diagnostic::error("expected ';'", span(1, 6, 5, 5));
        let rendered = d.render("f.rx", source);
        assert!(rendered.ends_with("    let x\n         ^"), "{}", rendered);
    }

    #[test]
    fn tabs_are_preserved_in_caret_padding() {
        let source = "\tlet x = ;";
        let d = Diagnostic::error("expected expression", span(1, 10, 9, 10));
        let rendered = d.render("f.rx", source);
        assert!(rendered.ends_with("    \t        ^"), "{}", rendered);
    }

    #[test]
    fn render_all_joins_with_blank_lines() {
        let diags = vec![
            Diagnostic::error("a", span(1, 1, 0, 1)),
            Diagnostic::note("b", span(1, 1, 0, 1)),
        ];
        let out = render_all("f.rx", "x", &diags);
        assert_eq!(out.matches("f.rx:1:1").count(), 2);
        assert!(out.contains("\n\n"));
    }

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Note.to_string(), "note");
    }
}
