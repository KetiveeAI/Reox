# REOX Style Guide

`reoxc fmt` enforces this style. It is the one style used across the REOX
repository and the NeolyxOS apps written in REOX; editors apply it through the
`Reox: Format Document` command (VS Code) or `reoxc fmt --stdin`.

```bash
reoxc fmt path/to/file.rx          # rewrite in place
reoxc fmt src/                     # every .rx / .reox under a directory
reoxc fmt --check .                # exit 1 and list files that would change
reoxc fmt --stdin < file.rx        # formatted text on stdout
```

The formatter is token based: it only moves whitespace, and adds trailing
commas in a few well-defined places. It never reorders code, never rewrites
literals and never drops a comment. Any file that lexes and has balanced
brackets can be formatted, even one that does not yet parse, so it is safe to
run on work in progress. Files that fail to lex are reported with the usual
compiler diagnostic and left untouched.

Formatting is idempotent: running `reoxc fmt` on its own output changes
nothing. The test `reox-lang/tests/fmt_corpus.rs` checks that on every source
file in the repository, and also that the compiler sees the same token stream
before and after.

## Indentation and line structure

- Four spaces per level. Tabs are replaced.
- One statement per line. `a(); b();` becomes two lines.
- K&R braces: the opening `{` stays on the line of the construct that owns it
  (`fn main() {`, `if x > 0 {`, `} else {`, `} catch e {`). A `{` never starts
  a line.
- A block that holds a statement (anything ending in `;`), a line comment, or
  that was written over several lines is laid out one item per line with the
  closing `}` on its own line. `fn f() { return 1; }` becomes three lines.
- Empty blocks are written `{}`.
- Blank lines are kept where the author put them, collapsed to at most one,
  and removed directly after `{` and before `}`. Leading blank lines in a file
  are removed; the file ends with exactly one newline.
- Trailing whitespace is removed. Windows line endings become `\n`.
- There is no line-length limit. The formatter never joins or splits a line
  on its own; see "Continuation lines" for how author line breaks inside a
  statement are kept.

## Brackets, calls and literals

- No space inside `()` and `[]`: `foo(a, b)`, `xs[i + 1]`, `(a + b)`.
- A single space inside single-line `{}`: `Point { x: 1, y: 2 }`,
  `struct Color { r: int, g: int }`.
- No space before the `(` of a call, a declaration, or a function type:
  `add(1, 2)`, `fn add(a: int)`, `fn(int) -> void`, `action(x: int) {`. A space
  after control keywords: `if (a)`, `return (a + b)`.
- No space before an index bracket (`xs[0]`); one space before an array
  literal or type (`let xs = [1, 2]`, `let t: [int]`).
- A bracket group written over several lines is kept multi-line, one element
  per line, with the closing bracket at the indentation of the line that opened
  it. A single-line group stays single-line. The formatter decides from where
  the author put line breaks between the group's own elements, so a call whose
  only multi-line part is a closure body keeps its shape:

  ```reox
  on_click(button, action() {
      count += 1;
  });
  ```

- Trailing commas are added to multi-line array literals, struct literals and
  `match` blocks (when the arms are already comma separated). They are never
  added to parameter or argument lists, which the parser does not accept them
  in.

  ```reox
  let p = Point {
      x: 1,
      y: 2,
  };
  let xs = [
      1,
      2,
  ];
  ```

## Operators

- Binary operators, assignment and compound assignment take one space on each
  side: `a + b`, `x = 1`, `n += 2`, `a == b && c`, `x ?? 0`, `a << 2 | b`.
- `->` and `=>` take one space on each side.
- Unary operators attach to their operand: `-x`, `!done`, `~mask`, `&ref`.
  `a - -b` and `return -1;` are handled.
- `++` and `--` attach to their operand whether prefix or postfix.
- Member access and optional chaining have no spaces: `a.b.c`, `view?.frame`.
- Ranges have no spaces: `0..n`.
- `:` has no space before and one after: `x: int`, `Point { x: 1 }`,
  `extension Color: Drawable`.
- Optional types attach the `?`: `int?`.
- Generic argument lists have no spaces around the angle brackets and one
  space after each comma: `Map<string, int>`, `Array<Array<int>>`.
  Comparisons keep their spaces: `a < b && b > c`.
- `@` attaches to the name that follows: `@Bind let count = 0;`.

## Declarations

- `extern fn`, `typealias`, `const`, `import` and global `let` are one line
  each, ending in `;`.
- Struct, variant, protocol, layer, panel and extension bodies follow the
  block rules: single-line if written single-line without statements,
  otherwise one member per line.
- Match arms go one per line. A block arm closes with `},` on its own line.
  Arms written without commas are left without commas.

## Comments

- Every comment is preserved, in order, next to the code it was next to.
- A comment on its own line keeps its own line and is indented like code.
- A comment at the end of a line stays on that line after one space.
  Column-aligned trailing comments lose their alignment.
- Block comments are emitted verbatim; their inner lines are not re-indented.
- Documentation comments are `///` lines directly above a declaration. The
  language server shows them on hover and in completion.

## Continuation lines

A line break the author placed inside a statement (after a binary operator or
`=`, or before `.` in a call chain) is kept and the continuation is indented
one level deeper than the statement. The formatter does not introduce such
breaks itself.

```reox
let total = a +
    b +
    c;
view
    .padding(1)
    .background(c);
```

## What the formatter refuses

- Input the lexer rejects (unterminated strings or block comments, unknown
  characters). The diagnostic names the position.
- Unbalanced or mismatched brackets.

In both cases nothing is written and `reoxc fmt` exits 1.
