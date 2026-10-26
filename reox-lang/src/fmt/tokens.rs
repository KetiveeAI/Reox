// REOX Formatter - lossless token stream.
//
// The compiler lexer drops comments and whitespace. The formatter re-uses its
// tokens (so the two never disagree about where a token starts or ends) and
// recovers comments from the gaps between them. Each token remembers how many
// newlines preceded it in the original source, which drives line-break
// decisions in `layout.rs`.

use crate::diagnostic::Diagnostic;
use crate::lexer::{tokenize, Span, TokenKind};

/// Coarse token class used by the layout rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Identifier or keyword.
    Word,
    /// Integer or float literal.
    Number,
    /// String literal, interpolated or not.
    Str,
    /// `// ...` up to (not including) the newline.
    LineComment,
    /// `/* ... */`, possibly spanning lines.
    BlockComment,
    /// `(`, `[`, `{`.
    Open,
    /// `)`, `]`, `}`.
    Close,
    /// Any operator or separator.
    Punct,
}

/// One formatter token.
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub class: Class,
    /// Exact source text.
    pub text: String,
    /// Newlines between the previous token and this one in the original.
    pub newlines_before: usize,
    /// Keyword flag for `Word`s (type keywords such as `int` count).
    pub is_keyword: bool,
    /// Where the token came from, for error messages.
    pub span: Span,
}

impl Tok {
    pub fn is(&self, text: &str) -> bool {
        self.text == text
    }

    pub fn is_comment(&self) -> bool {
        matches!(self.class, Class::LineComment | Class::BlockComment)
    }

    pub fn is_ident(&self) -> bool {
        self.class == Class::Word && !self.is_keyword
    }

    fn synthetic(class: Class, text: &str) -> Self {
        Tok {
            class,
            text: text.to_string(),
            newlines_before: 0,
            is_keyword: false,
            span: Span::default(),
        }
    }

    /// A comma the formatter inserts (trailing commas in multi-line literals).
    pub fn comma() -> Self {
        Tok::synthetic(Class::Punct, ",")
    }
}

/// Split `source` into formatter tokens. Lex errors are returned as diagnostics;
/// any non-whitespace, non-comment text between compiler tokens is an error too
/// (it would be silently dropped otherwise).
pub fn scan(source: &str) -> Result<Vec<Tok>, Diagnostic> {
    let tokens = tokenize(source).map_err(Diagnostic::from)?;
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len() * 2);
    let mut cursor = 0usize;

    for token in &tokens {
        let start = token.span.start.min(source.len());
        let gap = source.get(cursor..start).unwrap_or("");
        let newlines = scan_gap(gap, cursor, &mut out)?;

        if token.kind == TokenKind::Eof {
            cursor = start;
            break;
        }

        let end = token.span.end.clamp(start, source.len());
        let text = &source[start..end];
        if text.is_empty() {
            return Err(Diagnostic::error(
                format!("formatter: empty token {:?}", token.kind),
                token.span,
            ));
        }
        let (class, is_keyword) = classify(&token.kind, text);
        out.push(Tok {
            class,
            text: text.to_string(),
            newlines_before: newlines,
            is_keyword,
            span: token.span,
        });
        cursor = end;
    }

    // Comments after the last token (when the lexer's EOF span did not cover them).
    if cursor < source.len() {
        let gap = &source[cursor..];
        scan_gap(gap, cursor, &mut out)?;
    }
    Ok(out)
}

/// Consume whitespace and comments in `gap`, pushing comment tokens. Returns
/// the number of newlines after the last comment (the `newlines_before` of
/// whatever token follows the gap).
fn scan_gap(gap: &str, base: usize, out: &mut Vec<Tok>) -> Result<usize, Diagnostic> {
    let bytes = gap.as_bytes();
    let mut i = 0usize;
    let mut newlines = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                newlines += 1;
                i += 1;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let end = gap[i..].find('\n').map(|n| i + n).unwrap_or(bytes.len());
                let text = gap[i..end].trim_end();
                out.push(Tok {
                    class: Class::LineComment,
                    text: text.to_string(),
                    newlines_before: newlines,
                    is_keyword: false,
                    span: Span::new(0, 0, base + i, base + end),
                });
                newlines = 0;
                i = end;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let mut depth = 0usize;
                let mut j = i;
                let mut end = None;
                while j < bytes.len() {
                    if bytes[j] == b'/' && bytes.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                let Some(end) = end else {
                    return Err(Diagnostic::error(
                        "formatter: unterminated block comment",
                        Span::new(0, 0, base + i, base + i + 2),
                    ));
                };
                out.push(Tok {
                    class: Class::BlockComment,
                    text: gap[i..end].to_string(),
                    newlines_before: newlines,
                    is_keyword: false,
                    span: Span::new(0, 0, base + i, base + end),
                });
                newlines = 0;
                i = end;
            }
            _ => {
                let ch = gap[i..].chars().next().unwrap_or(' ');
                return Err(Diagnostic::error(
                    format!("formatter: unexpected text {:?} between tokens", ch),
                    Span::new(0, 0, base + i, base + i + ch.len_utf8()),
                ));
            }
        }
    }
    Ok(newlines)
}

fn classify(kind: &TokenKind, text: &str) -> (Class, bool) {
    match kind {
        TokenKind::Ident(_) => (Class::Word, false),
        TokenKind::IntLit(_) | TokenKind::FloatLit(_) => (Class::Number, false),
        TokenKind::StringLit(_) | TokenKind::StringInterp(_) => (Class::Str, false),
        TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => (Class::Open, false),
        TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => (Class::Close, false),
        TokenKind::Eof => (Class::Punct, false),
        other => {
            if other.is_keyword() || is_type_keyword(other) {
                (Class::Word, true)
            } else if text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                // Any keyword the lexer adds later still formats as a word.
                (Class::Word, true)
            } else {
                (Class::Punct, false)
            }
        }
    }
}

fn is_type_keyword(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Int
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::Bool
            | TokenKind::Void
            | TokenKind::I8
            | TokenKind::U8
            | TokenKind::I16
            | TokenKind::U16
            | TokenKind::I32
            | TokenKind::U32
            | TokenKind::I64
            | TokenKind::U64
            | TokenKind::USize
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<String> {
        scan(src).expect("scan").into_iter().map(|t| t.text).collect()
    }

    #[test]
    fn recovers_comments_between_tokens() {
        let toks = scan("let x = 1; // one\n/* two */ let y = 2;\n// tail").expect("scan");
        let comments: Vec<&Tok> = toks.iter().filter(|t| t.is_comment()).collect();
        assert_eq!(comments.len(), 3);
        assert_eq!(comments[0].text, "// one");
        assert_eq!(comments[0].newlines_before, 0);
        assert_eq!(comments[1].text, "/* two */");
        assert_eq!(comments[1].newlines_before, 1);
        assert_eq!(comments[2].text, "// tail");
        assert_eq!(comments[2].class, Class::LineComment);
    }

    #[test]
    fn counts_newlines_before_tokens() {
        let toks = scan("a\n\n\nb").expect("scan");
        assert_eq!(toks[1].newlines_before, 3);
        let toks = scan("a // c\n\nb").expect("scan");
        assert_eq!(toks[2].newlines_before, 2);
    }

    #[test]
    fn nested_block_comments_are_one_token() {
        assert_eq!(texts("/* a /* b */ c */ x"), vec!["/* a /* b */ c */", "x"]);
    }

    #[test]
    fn classifies_words_numbers_strings_and_punct() {
        let toks = scan("fn f(x: int) -> string { return \"\\(x)!\"; } 0x1F 2.5").expect("scan");
        let find = |s: &str| toks.iter().find(|t| t.text == s).unwrap_or_else(|| panic!("{}", s));
        assert!(find("fn").is_keyword);
        assert!(find("int").is_keyword);
        assert!(find("f").is_ident());
        assert_eq!(find("->").class, Class::Punct);
        assert_eq!(find("{").class, Class::Open);
        assert_eq!(find("}").class, Class::Close);
        assert_eq!(find("\"\\(x)!\"").class, Class::Str);
        assert_eq!(find("0x1F").class, Class::Number);
        assert_eq!(find("2.5").class, Class::Number);
    }

    #[test]
    fn lex_errors_surface_as_diagnostics() {
        assert!(scan("let s = \"open").is_err());
        assert!(scan("/* never closed").is_err());
        assert!(scan("let x = $;").is_err());
    }
}
