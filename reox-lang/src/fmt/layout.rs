// REOX Formatter - layout engine.
//
// Works on the lossless token stream from `tokens.rs`. The output is a pure
// function of the token kinds plus a small amount of information taken from
// the original line breaks (which bracket groups the author spread over
// several lines, where blank lines were). Every decision that reads an
// original line break is one the formatter reproduces, which is what makes
// `format(format(x)) == format(x)`.
//
// Rules, in the order they are applied for the token `t` after `p`:
//   1. `t` closes a multi-line group            -> newline
//   2. `t` is a comment on the same line as `p` -> no newline (stays trailing)
//   3. `p` opens a multi-line group             -> newline
//   4. `p` is `;`                               -> newline
//   5. `p` is a line comment                    -> newline
//   6. `p` is `,` inside a multi-line group     -> newline
//   7. `p` closes a multi-line `{}` block       -> newline unless `t` continues it
//   8. `t` is `,` `;` `.` `?.` `:` `{` ... or closes a single-line group -> no newline
//   9. `p` opens a single-line group or is `.` `?.` `@` -> no newline
//  10. otherwise the author's line break is kept.

use crate::diagnostic::Diagnostic;

use super::tokens::{Class, Tok};

const INDENT: &str = "    ";
/// Longest token run inspected when deciding whether `<` opens a generic
/// argument list.
const GENERIC_LOOKAHEAD: usize = 64;

/// What a bracket group is, which decides trailing commas and continuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupKind {
    /// Statement block: fn body, if/while/for body, closure body, bare block.
    Block,
    /// Body of struct/variant/protocol/layer/panel/extension.
    Decl,
    /// `match x { ... }`.
    Match,
    /// `Name { field: value }`.
    StructLit,
    /// `[a, b]` value.
    ArrayLit,
    /// `a[i]`.
    Index,
    /// `[int]` in type position.
    TypeBracket,
    /// Any `( ... )`.
    Paren,
}

#[derive(Debug, Clone, Copy)]
struct Group {
    close: usize,
    kind: GroupKind,
    multiline: bool,
}

/// Per-token facts computed before emission.
struct Facts {
    /// `partner[i]` is the matching bracket for an Open/Close token.
    partner: Vec<Option<usize>>,
    /// Group description for every Open token.
    group: Vec<Option<Group>>,
    /// `<` / `>` that delimit generic arguments, not comparisons.
    generic: Vec<bool>,
}

/// Format a token stream. Fails only on unbalanced brackets.
pub fn format_tokens(toks: Vec<Tok>) -> Result<String, Diagnostic> {
    let facts = analyze(&toks)?;
    let toks = insert_trailing_commas(toks, &facts);
    let facts = analyze(&toks)?;
    Ok(emit(&toks, &facts))
}

// === Analysis ===

fn analyze(toks: &[Tok]) -> Result<Facts, Diagnostic> {
    let partner = match_brackets(toks)?;
    let generic = find_generics(toks);
    let mut group = vec![None; toks.len()];
    for (i, t) in toks.iter().enumerate() {
        if t.class != Class::Open {
            continue;
        }
        let Some(close) = partner[i] else {
            continue;
        };
        let kind = classify_group(toks, i, &partner, &group);
        let multiline = is_multiline(toks, i, close, kind, &partner);
        group[i] = Some(Group { close, kind, multiline });
    }
    Ok(Facts { partner, group, generic })
}

fn match_brackets(toks: &[Tok]) -> Result<Vec<Option<usize>>, Diagnostic> {
    let mut partner = vec![None; toks.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.class {
            Class::Open => stack.push(i),
            Class::Close => {
                let Some(open) = stack.pop() else {
                    return Err(Diagnostic::error(
                        format!("formatter: unmatched '{}'", t.text),
                        t.span,
                    ));
                };
                let expected = match toks[open].text.as_str() {
                    "(" => ")",
                    "[" => "]",
                    _ => "}",
                };
                if t.text != expected {
                    return Err(Diagnostic::error(
                        format!("formatter: expected '{}' to close '{}', found '{}'", expected, toks[open].text, t.text),
                        t.span,
                    ));
                }
                partner[open] = Some(i);
                partner[i] = Some(open);
            }
            _ => {}
        }
    }
    if let Some(open) = stack.pop() {
        return Err(Diagnostic::error(
            format!("formatter: '{}' is never closed", toks[open].text),
            toks[open].span,
        ));
    }
    Ok(partner)
}

/// Mark `<`/`>` pairs that read as generic argument lists: `Name<...>` where
/// the inside holds only type-like tokens and the list is followed by
/// something that cannot follow a comparison.
fn find_generics(toks: &[Tok]) -> Vec<bool> {
    let mut generic = vec![false; toks.len()];
    for i in 0..toks.len() {
        if !toks[i].is("<") || i == 0 || !toks[i - 1].is_ident() {
            continue;
        }
        let mut depth = 1usize;
        let mut j = i + 1;
        let mut closers: Vec<usize> = Vec::new();
        let mut ok = false;
        while j < toks.len() && j - i <= GENERIC_LOOKAHEAD {
            let t = &toks[j];
            match t.class {
                Class::Word => {}
                Class::Punct if t.is(",") || t.is(".") || t.is("?") => {}
                Class::Punct if t.is("<") => depth += 1,
                Class::Punct if t.is(">") || t.is(">>") => {
                    let n = if t.is(">>") { 2 } else { 1 };
                    closers.push(j);
                    if depth <= n {
                        depth = 0;
                    } else {
                        depth -= n;
                    }
                    if depth == 0 {
                        ok = toks
                            .get(j + 1)
                            .map(|next| {
                                next.class == Class::Open
                                    || next.class == Class::Close
                                    || next.is_comment()
                                    || matches!(
                                        next.text.as_str(),
                                        "," | "=" | ";" | "." | "?" | "->" | "=>" | ":" | "?." | "??"
                                    )
                            })
                            .unwrap_or(true);
                        break;
                    }
                }
                Class::Open if t.is("[") => {}
                Class::Close if t.is("]") => {}
                _ => break,
            }
            j += 1;
        }
        if ok {
            generic[i] = true;
            for c in closers {
                generic[c] = true;
            }
        }
    }
    generic
}

fn is_decl_keyword(text: &str) -> bool {
    matches!(text, "struct" | "variant" | "protocol" | "layer" | "panel" | "extension")
}

fn classify_group(toks: &[Tok], open: usize, partner: &[Option<usize>], groups: &[Option<Group>]) -> GroupKind {
    let prev = prev_significant(toks, open);
    match toks[open].text.as_str() {
        "(" => GroupKind::Paren,
        "[" => {
            let enclosing = enclosing_group(toks, open, partner);
            if enclosing.and_then(|g| groups[g]).is_some_and(|g| g.kind == GroupKind::TypeBracket) {
                return GroupKind::TypeBracket;
            }
            match prev.map(|p| &toks[p]) {
                Some(p) if p.is_ident() || p.class == Class::Str || p.is(")") || p.is("]") => GroupKind::Index,
                Some(p) if p.is(":") || p.is("->") || p.is("<") || p.is_keyword && is_type_word(&p.text) => {
                    GroupKind::TypeBracket
                }
                _ => GroupKind::ArrayLit,
            }
        }
        _ => {
            // Walk back over the statement that owns this brace.
            let mut saw_match = false;
            let mut saw_decl = false;
            let mut j = open;
            while j > 0 {
                j -= 1;
                let t = &toks[j];
                if t.is_comment() {
                    continue;
                }
                if t.class == Class::Close {
                    match partner[j] {
                        Some(p) => {
                            j = p;
                            continue;
                        }
                        None => break,
                    }
                }
                if t.class == Class::Open || t.is(";") || t.is(",") || t.is("=>") {
                    break;
                }
                if t.is("match") {
                    saw_match = true;
                }
                if is_decl_keyword(&t.text) {
                    saw_decl = true;
                }
            }
            if saw_match {
                return GroupKind::Match;
            }
            if saw_decl {
                return GroupKind::Decl;
            }
            let prev_is_ident = prev.is_some_and(|p| toks[p].is_ident());
            if prev_is_ident && looks_like_struct_literal(toks, open) {
                return GroupKind::StructLit;
            }
            GroupKind::Block
        }
    }
}

fn is_type_word(text: &str) -> bool {
    matches!(
        text,
        "int" | "float" | "string" | "bool" | "void" | "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "usize"
    )
}

/// `{` followed by `name :` or directly by `}`.
fn looks_like_struct_literal(toks: &[Tok], open: usize) -> bool {
    let mut j = open + 1;
    while j < toks.len() && toks[j].is_comment() {
        j += 1;
    }
    let Some(first) = toks.get(j) else {
        return false;
    };
    if first.is("}") {
        return true;
    }
    if !first.is_ident() {
        return false;
    }
    let mut k = j + 1;
    while k < toks.len() && toks[k].is_comment() {
        k += 1;
    }
    toks.get(k).is_some_and(|t| t.is(":"))
}

fn prev_significant(toks: &[Tok], i: usize) -> Option<usize> {
    (0..i).rev().find(|&j| !toks[j].is_comment())
}

fn enclosing_group(toks: &[Tok], i: usize, partner: &[Option<usize>]) -> Option<usize> {
    let mut j = i;
    while j > 0 {
        j -= 1;
        match toks[j].class {
            Class::Close => j = partner[j]?,
            Class::Open => return Some(j),
            _ => {}
        }
    }
    None
}

fn is_multiline(toks: &[Tok], open: usize, close: usize, kind: GroupKind, partner: &[Option<usize>]) -> bool {
    if close == open + 1 {
        return false;
    }
    let brace = toks[open].is("{");
    let _ = kind;
    if brace {
        // Any line break, statement terminator or line comment anywhere inside.
        if toks[close].newlines_before > 0 {
            return true;
        }
        return toks[open + 1..close]
            .iter()
            .any(|t| t.newlines_before > 0 || t.is(";") || t.class == Class::LineComment);
    }
    // Parens and brackets: only line breaks between their direct children
    // count, so `foo(action() {` + block + `})` keeps its shape.
    if toks[close].newlines_before > 0 {
        return true;
    }
    let mut j = open + 1;
    while j < close {
        let t = &toks[j];
        if t.class == Class::Open {
            if t.newlines_before > 0 {
                return true;
            }
            match partner[j] {
                Some(p) => {
                    j = p + 1;
                    continue;
                }
                None => return true,
            }
        }
        if t.newlines_before > 0 || t.class == Class::LineComment {
            return true;
        }
        j += 1;
    }
    false
}

/// Add the trailing comma to multi-line array literals, struct literals and
/// match arms (the parser accepts it in exactly those places).
fn insert_trailing_commas(toks: Vec<Tok>, facts: &Facts) -> Vec<Tok> {
    let mut insert_after: Vec<bool> = vec![false; toks.len()];
    for (i, g) in facts.group.iter().enumerate() {
        let Some(g) = g else { continue };
        if !g.multiline || !matches!(g.kind, GroupKind::ArrayLit | GroupKind::StructLit | GroupKind::Match) {
            continue;
        }
        let Some(last) = (i + 1..g.close).rev().find(|&j| !toks[j].is_comment()) else {
            continue;
        };
        if toks[last].is(",") {
            continue;
        }
        // Match arms only get one when the author already separates arms
        // with commas; otherwise the comma-less style is kept as written.
        if g.kind == GroupKind::Match && !has_direct_comma(&toks, i, g.close, &facts.partner) {
            continue;
        }
        insert_after[last] = true;
    }
    let mut out = Vec::with_capacity(toks.len() + 8);
    for (i, t) in toks.into_iter().enumerate() {
        out.push(t);
        if insert_after[i] {
            out.push(Tok::comma());
        }
    }
    out
}

/// Whether a `,` appears directly inside the group (not in a nested one).
fn has_direct_comma(toks: &[Tok], open: usize, close: usize, partner: &[Option<usize>]) -> bool {
    let mut j = open + 1;
    while j < close {
        if toks[j].class == Class::Open {
            match partner[j] {
                Some(p) => {
                    j = p + 1;
                    continue;
                }
                None => return false,
            }
        }
        if toks[j].is(",") {
            return true;
        }
        j += 1;
    }
    false
}

// === Emission ===

struct Emitter<'a> {
    toks: &'a [Tok],
    facts: &'a Facts,
    out: String,
    line: String,
    line_indent: usize,
    /// Open groups: (open index, multiline).
    stack: Vec<(usize, bool)>,
    prev: Option<usize>,
    /// Last emitted non-comment token.
    prev_sig: Option<usize>,
    /// Nesting inside generic `<...>` lists, where commas never break lines.
    generic_depth: usize,
}

fn emit(toks: &[Tok], facts: &Facts) -> String {
    let mut e = Emitter {
        toks,
        facts,
        out: String::new(),
        line: String::new(),
        line_indent: 0,
        stack: Vec::new(),
        prev: None,
        prev_sig: None,
        generic_depth: 0,
    };
    for i in 0..toks.len() {
        e.token(i);
    }
    e.flush_line();
    if !e.out.is_empty() && !e.out.ends_with('\n') {
        e.out.push('\n');
    }
    e.out
}

/// Tokens that may follow a multi-line `}` on the same line.
fn continues_after_block(t: &Tok) -> bool {
    matches!(t.text.as_str(), "else" | "catch" | ")" | "]" | "}" | "," | ";" | "." | "?." | "??")
}

/// Keywords after which `(` is an expression, not a call.
fn keyword_takes_space_before_paren(text: &str) -> bool {
    !matches!(
        text,
        "self" | "fn" | "action" | "on_tap" | "on_pan" | "on_swipe" | "on_pinch" | "on_rotate" | "gesture"
    ) && !is_type_word(text)
}

impl<'a> Emitter<'a> {
    fn flush_line(&mut self) {
        if self.line.is_empty() {
            return;
        }
        for _ in 0..self.line_indent {
            self.out.push_str(INDENT);
        }
        let trimmed = self.line.trim_end();
        self.out.push_str(trimmed);
        self.out.push('\n');
        self.line.clear();
    }

    fn depth(&self) -> usize {
        self.stack.iter().filter(|(_, m)| *m).count()
    }

    fn innermost(&self) -> Option<(usize, bool)> {
        self.stack.last().copied()
    }

    fn innermost_kind(&self) -> Option<GroupKind> {
        self.innermost().and_then(|(open, _)| self.facts.group[open]).map(|g| g.kind)
    }

    fn is_unary(&self, i: usize) -> bool {
        let t = &self.toks[i];
        match t.text.as_str() {
            "!" | "~" => true,
            "-" | "&" | "++" | "--" => match self.prev_sig.map(|p| &self.toks[p]) {
                None => true,
                Some(p) => match p.class {
                    Class::Word => p.is_keyword && !is_type_word(&p.text) && !p.is("self"),
                    Class::Number | Class::Str | Class::Close => false,
                    Class::Punct => !(self.facts.generic[self.prev_sig.unwrap_or(0)] && p.is(">")),
                    Class::Open => true,
                    Class::LineComment | Class::BlockComment => true,
                },
            },
            _ => false,
        }
    }

    fn is_postfix(&self, i: usize) -> bool {
        let t = &self.toks[i];
        if t.is("?") {
            return true;
        }
        if t.is("++") || t.is("--") {
            return !self.is_unary(i);
        }
        false
    }

    /// Should the token at `i` start a new line? Returns (newline, continuation).
    fn wants_newline(&self, i: usize) -> (bool, bool) {
        let t = &self.toks[i];
        let Some(mut pi) = self.prev else {
            return (false, false);
        };
        // A trailing block comment is transparent: `x; /* c */ y` still
        // breaks after the semicolon.
        if self.toks[pi].class == Class::BlockComment && !t.is_comment() {
            if let Some(sig) = self.prev_sig {
                pi = sig;
            }
        }
        let p = &self.toks[pi];

        // 1. closing a multi-line group
        if t.class == Class::Close {
            let multi = self.facts.partner[i].and_then(|o| self.facts.group[o]).is_some_and(|g| g.multiline);
            if multi {
                return (true, false);
            }
        }
        // 2. trailing comment stays on its line
        if t.is_comment() && t.newlines_before == 0 {
            return (false, false);
        }
        // 3. after a multi-line open
        if p.class == Class::Open && self.facts.group[pi].is_some_and(|g| g.multiline) {
            return (true, false);
        }
        // 4. after a semicolon
        if p.is(";") {
            return (true, false);
        }
        // 5. after a line comment
        if p.class == Class::LineComment {
            let cont = self.continuation(i);
            return (true, cont);
        }
        // 6. after a comma inside a multi-line group (not a generic list)
        if p.is(",") && self.generic_depth == 0 && self.innermost().is_some_and(|(_, m)| m) {
            return (true, false);
        }
        // 7. after a multi-line block
        if p.is("}") && self.facts.partner[pi].and_then(|o| self.facts.group[o]).is_some_and(|g| g.multiline) {
            return (!continues_after_block(t), false);
        }
        // 8. tokens that never start a line
        if t.class == Class::Close
            || t.class == Class::Open
            || matches!(t.text.as_str(), "," | ";" | ":" | "?" | "++" | "--")
        {
            return (false, false);
        }
        // 9. tokens nothing starts a line after
        if p.class == Class::Open || p.is(".") || p.is("?.") || p.is("@") {
            return (false, false);
        }
        // 10. the author's choice
        if t.newlines_before > 0 {
            return (true, self.continuation(i));
        }
        (false, false)
    }

    /// A kept line break in the middle of a statement indents one level more.
    fn continuation(&self, i: usize) -> bool {
        let t = &self.toks[i];
        if t.is_comment() || t.class == Class::Close {
            return false;
        }
        if matches!(self.innermost_kind(), Some(GroupKind::Match) | Some(GroupKind::Decl)) {
            return false;
        }
        match self.prev_sig.map(|p| &self.toks[p]) {
            None => false,
            Some(p) => !(p.is(";") || p.is(",") || p.class == Class::Open || p.is("}")),
        }
    }

    fn needs_space(&self, i: usize) -> bool {
        let t = &self.toks[i];
        let Some(pi) = self.prev else {
            return false;
        };
        let p = &self.toks[pi];

        if t.is_comment() || p.class == Class::BlockComment {
            return true;
        }
        if p.class == Class::Open {
            return p.is("{") && !t.is("}");
        }
        if t.class == Class::Close {
            return t.is("}");
        }
        if t.is(",") || t.is(";") || t.is(".") || t.is("?.") || p.is(".") || p.is("?.") {
            return false;
        }
        if t.is("..") || p.is("..") || p.is("@") {
            return false;
        }
        if t.is(":") {
            return false;
        }
        if p.is(":") {
            return true;
        }
        if self.facts.generic[i] || self.facts.generic[pi] && p.is("<") {
            return false;
        }
        if p.class == Class::Punct && self.prev_sig == Some(pi) && self.prev_unary(pi) {
            return false;
        }
        if self.is_postfix(i) {
            return false;
        }
        if t.is("(") {
            return match p.class {
                Class::Word => p.is_keyword && keyword_takes_space_before_paren(&p.text),
                Class::Close | Class::Number | Class::Str => false,
                Class::Punct => !(self.facts.generic[pi] && (p.is(">") || p.is(">>"))),
                _ => true,
            };
        }
        if t.is("[") {
            return match self.facts.group[i].map(|g| g.kind) {
                Some(GroupKind::Index) => false,
                _ => !(self.facts.generic[pi] && (p.is(">") || p.is(">>"))),
            };
        }
        true
    }

    /// Whether the already emitted operator at `pi` was unary.
    fn prev_unary(&self, pi: usize) -> bool {
        let p = &self.toks[pi];
        if !matches!(p.text.as_str(), "!" | "~" | "-" | "&" | "++" | "--") {
            return false;
        }
        // Recompute with the token before it.
        let before = prev_significant(self.toks, pi).map(|b| &self.toks[b]);
        match p.text.as_str() {
            "!" | "~" => true,
            _ => match before {
                None => true,
                Some(b) => match b.class {
                    Class::Word => b.is_keyword && !is_type_word(&b.text) && !b.is("self"),
                    Class::Number | Class::Str | Class::Close => false,
                    Class::Punct => {
                        let bi = prev_significant(self.toks, pi).unwrap_or(0);
                        !(self.facts.generic[bi] && b.is(">"))
                    }
                    Class::Open => true,
                    Class::LineComment | Class::BlockComment => true,
                },
            },
        }
    }

    fn token(&mut self, i: usize) {
        let t = &self.toks[i];
        if t.class == Class::Close {
            self.stack.pop();
        }

        let (newline, continuation) = self.wants_newline(i);
        if newline {
            let blank = t.newlines_before >= 2
                && t.class != Class::Close
                && self.prev.is_some_and(|p| self.toks[p].class != Class::Open);
            self.flush_line();
            if blank {
                self.out.push('\n');
            }
            self.line_indent = self.depth() + usize::from(continuation);
        } else if self.prev.is_none() {
            self.line_indent = 0;
        } else if self.needs_space(i) {
            self.line.push(' ');
        }

        self.line.push_str(&t.text);

        if t.class == Class::Open {
            let multi = self.facts.group[i].is_some_and(|g| g.multiline);
            self.stack.push((i, multi));
        }
        if self.facts.generic[i] {
            if t.is("<") {
                self.generic_depth += 1;
            } else if t.is(">>") {
                self.generic_depth = self.generic_depth.saturating_sub(2);
            } else {
                self.generic_depth = self.generic_depth.saturating_sub(1);
            }
        }
        self.prev = Some(i);
        if !t.is_comment() {
            self.prev_sig = Some(i);
        }
    }
}
