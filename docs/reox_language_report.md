# REOX Language — State of the Language Report

> **Date:** April 2026 · **Compiler Version:** reoxc v1.1.0 · **Target Platform:** NeolyxOS (Linux)
> **Test Suite:** 102 unit tests + 10 smoke tests · **Result: 100% pass**

---

## 1. What Exists Today (Foundations Are Solid)

The compiler pipeline is fully operational end-to-end. Every major stage is implemented from scratch in Rust with zero external dependencies.

```
.rx source → Lexer → Parser → Type Checker → C Codegen → C Compiler + Runtime → Native ELF
                                             → Interpreter (tree-walking, direct execution)
```

### 1.1 Compiler Stages

| Stage | Lines of Code | Status | Notes |
|---|---|---|---|
| **Lexer** | 806 | ✅ Complete | All tokens, operators, keywords tokenized with span tracking |
| **Parser** | 1,941 | ✅ Complete | Recursive descent + Pratt parsing for expressions |
| **Type Checker** | 1,149 + 342 | ✅ Functional | Two-pass: declaration registration + body checking |
| **Code Generator** | 1,210 | ✅ Complete | Transpiles to C with full `<stdint.h>` type mapping |
| **Interpreter** | 1,109 | ✅ Complete | Tree-walking evaluator, native built-ins |
| **C Runtime** | ~15,874 | ✅ Extensive | UI, animation, themes, SDL backend, FFI |

---

### 1.2 Language Features That Work

#### Core Language
| Feature | Parse | TypeCheck | Codegen | Interpreter |
|---|---|---|---|---|
| `fn` — functions | ✅ | ✅ | ✅ | ✅ |
| `struct` — product types | ✅ | ✅ | ✅ | ✅ |
| `let` / `let mut` — variables | ✅ | ✅ | ✅ | ✅ |
| `if` / `else` | ✅ | ✅ | ✅ | ✅ |
| `while` loops | ✅ | ✅ | ✅ | ✅ |
| `for ... in` loops | ✅ | ✅ | ✅ | ✅ |
| `return` | ✅ | ✅ | ✅ | ✅ |
| `match` expressions | ✅ | ✅ | ✅ | ✅ |
| Arithmetic, bitwise, logical operators | ✅ | ✅ | ✅ | ✅ |
| `+=`, `-=`, `*=`, `/=`, `%=` | ✅ | ✅ | ✅ | ✅ |
| `++`, `--` (pre and post) | ✅ | ✅ | ✅ | ✅ |
| `??` null coalescing | ✅ | — | ✅ | ✅ |
| `?.` optional chaining | ✅ | — | ✅ | ✅ |
| `guard` / `defer` | ✅ | ✅ | ✅ | ✅ |
| `try` / `catch` / `throw` | ✅ | ✅ | ✅ | ✅ |
| `async` / `await` | ✅ | ✅ | ✅ | ✅ |
| `extern fn` — C/C++/Rust FFI | ✅ | ✅ | ✅ | ✅ |
| `import` | ✅ | — | ✅ | ✅ |
| `action` — closures / lambdas | ✅ | ✅ | ✅ | ✅ |
| Trailing closures | ✅ | — | ✅ | ✅ |
| Range expressions `..` | ✅ | ✅ | ✅ | ✅ |
| `typealias` | ✅ | ✅ | ✅ | — |
| `pub` visibility modifier | ✅ | — | ✅ | — |
| `const` / `static` | ✅ | ✅ | ✅ | — |

#### Type System
- **Primitive types:** `int`, `float`, `string`, `bool`, `void`
- **Explicit integer widths:** `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `usize`
- **Container types:** `[T]` arrays, `Map<K,V>`, `Optional<T>` (`T?`), `Tuple`, `fn(T) -> R` function types
- **Named types:** structs, variants, protocols
- **Type inference** from initializers
- **Widening coercions:** `int → float`, all integer widths are mutually coercible

#### UI / NeolyxOS-Specific
| Feature | Parse | Codegen | Notes |
|---|---|---|---|
| `variant` — enum types | ✅ | ✅ | C union + tag struct emitted |
| `protocol` — interfaces | ✅ | ✅ | Method registration, no runtime vtable yet |
| `extension` — type extensions | ✅ | ✅ | Methods added via mangled names |
| `layer` — UI view components | ✅ | ✅ | Struct + methods generated |
| `panel` — top-level windows | ✅ | ✅ | Properties + methods generated |
| `@Bind` reactive state | ✅ | ✅ | Parsed as annotated let |
| `signal` / `emit` | ✅ | ✅ | Signal fields declared in layers |
| Gesture handlers | ✅ | ✅ | `on_tap`, `on_pan`, `on_swipe`, etc. |

#### Standard Library (Interpreter built-ins)
`print`, `println`, `len`, `push`, `pop`, `map_new`, `map_set`, `map_get`, `map_has`, `map_remove`, `map_keys`, `str_split`, `str_join`, `str_trim`, `str_upper`, `str_lower`, `str_contains`, `str_replace`, `abs`, `min`, `max`, `sqrt`, `pow`, `floor`, `ceil`, `round`, `int()`, `float()`, `str()`, `bool()`, `range`, `clamp`, `sign`, `is_file`, `is_dir`, `create_dir`, `file_append`, `ease_linear`, `ease_in`, `ease_out`, `ease_in_out`, `lerp`, `ai_generate`, `ai_complete`, input/output, type_of, animation helpers

---

## 2. What Is Missing — Gaps vs. Swift, C, C++, Python

This is the honest assessment. The gaps are organized by severity.

---

### 🔴 Critical — Blocks Real Programs from Being Written

#### 2.1 No Top-Level Global Variables
```reox
// This fails with "expected declaration, found Let"
let mut counter: int = 0;

fn increment() -> void {
    counter = counter + 1;
}
```
C, C++, Python, Swift all allow module-level state. REOX forces all state through `extern` C accessors, making pure-REOX programs awkward. Every stateful program currently needs a C bridge file.

**Root Cause:** `parse_declaration` only accepts `fn`, `struct`, `extern`, `variant`, `protocol`, `extension`, `layer`, `panel`, `const`, `typealias`, `pub`, `static`. `let` at top level panics.

---

#### 2.2 No Mutable Reassignment Enforcement
The type checker records `mutable: bool` per symbol but **never enforces it**. This means the following silently compiles:
```reox
let x: int = 10;
x = 99; // should be a compile error — x is not mut
```
Every proper language (Rust, Swift, C `const`) catches this at compile time. REOX doesn't.

---

#### 2.3 No Pattern Matching on Variants
Variants are parsed and codegen'd, but `match` only handles literal patterns and identifier wildcards:
```reox
variant Result { Ok(value: int), Err(msg: string) }

// This does NOT work today:
match result {
    Ok(v) => print_int(v),
    Err(msg) => print(msg),
}
```
Without destructuring in match arms, `variant` types are useless as discriminated unions. This is table stakes for a safe alternative to C `union`.

---

#### 2.4 No String Interpolation
Every other modern language has this:
```python
# Python
print(f"Hello {name}!")
```
```swift
// Swift
print("Hello \(name)!")
```
In REOX you must:
```reox
print(string_concat("Hello ", string_concat(name, "!")));
```
This is the single biggest ergonomic regression from any language REOX targets. It makes practical output code unreadable.

---

#### 2.5 Type Checker Does Not Propagate Errors Through Nested Blocks
```reox
fn test() -> int {
    while condition {
        let x: string = 42; // type mismatch
    }
    return 0;
}
```
The nested block scoping was recently fixed in the parser, but the type checker's `check_block` does not push/pop its own scope for nested blocks inside `while` and `for` bodies — it relies on `check_while` / `check_for` to do it. This can cause false-negative type errors inside loops.

---

### 🟡 High Priority — Prevents REOX from Being Self-Hosting or Practical

#### 2.6 No Module System / Namespacing
`import foo::bar;` is parsed but has no semantic effect. There is no resolution, no file loading, no namespace isolation. In Python, Swift, C++, and Rust, the module system is fundamental to organizing code across files.

**Without this:** Every REOX program must declare everything in one file or use C `#include`.

---

#### 2.7 No `self` in Methods / No Method Dispatch
```reox
struct Calculator { value: int }

// This does NOT work:
extension Calculator {
    fn add(n: int) -> void {
        self.value = self.value + n; // 'self' is a keyword but not implemented
    }
}
```
`self` is a recognized keyword (`TokenKind::Self_`) but has no parser rule, no type checker handling, and no codegen. Extensions with `self` cannot be written.

---

#### 2.8 Protocol Conformance Is Not Checked
```reox
protocol Drawable { fn draw(); }
extension MyShape { fn draw() { ... } }
// No check that MyShape actually satisfies Drawable
```
The type checker registers protocols and extensions but never verifies conformance. The protocol system is purely structural/nominal at the syntax level with no runtime or compile-time dispatch guarantee.

---

#### 2.9 No Generics / Parametric Types
```swift
// Swift
func swap<T>(_ a: inout T, _ b: inout T) { ... }
```
REOX has no generic functions or generic structs. The workaround is `Unknown` type, which disables type checking entirely. This is Python-level safety at the C-level performance promise — a contradiction.

---

#### 2.10 No Memory Safety Model
REOX compiles to C via transpilation. It inherits all of C's memory hazards:
- No borrow checker (unlike Rust)
- No automatic reference counting (unlike Swift)
- No garbage collector (unlike Python)
- No `unsafe` boundary (unlike Rust)

For NeolyxOS kernel/driver code this is acceptable. For userland REOX programs, it means heap-allocated strings and arrays are manually managed through C runtime functions, which is not visible to the REOX programmer. **This is the deepest architectural gap versus Swift and Python.**

---

#### 2.11 No `else if` Chain
```reox
// Does NOT parse:
if a {
    ...
} else if b {
    ...
} else {
    ...
}
```
`parse_if_stmt` only handles a single optional `else { }` block. `else if` requires a recursive `if` as the else branch. This is missing.

---

### 🟠 Medium Priority — Important for Developer Experience

#### 2.12 No Multi-Line String Literals / Raw Strings
```python
sql = """
SELECT * FROM users
WHERE active = true
"""
```
REOX strings must fit on one line. No raw strings, no heredoc.

---

#### 2.13 No `break` / `continue` Codegen
`Stmt::Break` and `Stmt::Continue` are in the AST but the codegen does not emit them. They silently disappear in compiled output.

```reox
while true {
    if done { break; } // silently ignored in generated C
}
```

---

#### 2.14 Typechecker Has No Mutability Check on Reassignment in Loops
```reox
for x in my_array {
    x = 42; // x is not mut, should error, doesn't
}
```

---

#### 2.15 No Standard Error Type / Result Type
No built-in `Result<T, E>` or `Error` type. `try/catch/throw` is parsed but `throw` takes any expression and `catch` binds it as `string`. There is no typed error system.

---

#### 2.16 Interpreter and Codegen Are Out of Sync
The interpreter has ~50 built-in native functions. The C runtime has different signatures. Code written to run under `reoxc run` (interpreter mode) may not compile identically to C due to:
- Different `print` behavior (fixed recently, but more exist)
- Missing or renamed functions between the two surfaces
- No automated cross-validation test

---

### 🟢 Low Priority — Polish and Completeness

| Gap | Comparator |
|---|---|
| No char type / single-char literals | C, Rust |
| No hex/binary integer literals (`0xFF`, `0b1010`) | C, Python, Rust |
| No multi-value returns / named return values | Go, Swift |
| No labeled break / continue | Kotlin, Rust |
| No operator overloading | Swift, C++ |
| No variadic functions | C, Python |
| No compile-time constants (`#define` equivalent) | C |
| No package registry (publish/install) | npm, Cargo, PyPI |
| `elif` chain missing | Python users |
| No LLVM backend (only C transpilation) | Zig, Rust, Swift |
| No language server (LSP) implementation | All modern languages |

---

## 3. Summary Score vs. Mature Languages

| Capability | REOX | C | C++ | Swift | Python |
|---|---|---|---|---|---|
| **Core syntax** | 🟡 80% | ✅ | ✅ | ✅ | ✅ |
| **Type system** | 🟡 60% | 🟡 | ✅ | ✅ | 🔴 |
| **Memory safety** | 🔴 None | 🔴 None | 🟡 RAII | ✅ ARC | ✅ GC |
| **Module system** | 🔴 Stub | ✅ | ✅ | ✅ | ✅ |
| **Generics** | 🔴 None | 🔴 None | ✅ Templates | ✅ | ✅ Duck |
| **Error handling** | 🟡 try/catch | 🔴 codes | 🟡 exceptions | ✅ Result | ✅ exceptions |
| **Standard library** | 🟡 Runtime-only | ✅ | ✅ | ✅ | ✅ |
| **Tooling (LSP, debugger)** | 🔴 None | 🟡 | ✅ | ✅ | ✅ |
| **Self-hosting** | 🔴 Not yet | ✅ | ✅ | ✅ | ✅ |
| **Package ecosystem** | 🔴 None | 🟡 | 🟡 | ✅ SPM | ✅ PyPI |

---

## 4. The Three Most Impactful Fixes to Make Next

These are ordered by effort-to-impact ratio.

### Priority 1: `else if` chains (1 day)
Modify `parse_if_stmt` to recursively parse `else if`:
```rust
let else_block = if self.match_token(&[TokenKind::Else]) {
    if self.check(&TokenKind::If) {
        Some(Block { statements: vec![self.parse_if_stmt()?], ... })
    } else {
        Some(self.parse_block()?)
    }
};
```
Every program that has more than two conditions needs this.

### Priority 2: Top-level `let` / global variables (2-3 days)
Add `TokenKind::Let` as a valid top-level declaration. Emit it as a C `static` global. This removes the need for C accessor boilerplate in every stateful REOX program.

### Priority 3: Mutability enforcement in type checker (1-2 days)
In `infer_expr_type` for `Expr::Assign`, look up the target symbol and emit a `TypeError` if `symbol.mutable == false`. This is the difference between REOX being a type-safe language and a type-checked-looking language.

---

## 5. Overall Assessment

REOX has a real, working compiler. The pipeline is architecturally sound. The C transpilation backend is pragmatic and produces correct output. The runtime C library (15,000+ lines) is substantial. The test suite passes at 100%.

However, the language is currently **an OS UI scripting language, not a general-purpose language**. The critical missing pieces — global variables, mutability enforcement, pattern destructuring, string interpolation, and a module system — are what separate a scripting language from a production programming language.

The path to parity with Swift and Python is not fundamentally blocked. The architecture can support all of it. It requires continued, focused implementation work on the compiler rather than the runtime.

> **Verdict:** REOX is a well-structured language at the prototype stage. It can compile real programs today, but requires 6–12 months of focused compiler work to reach the usability threshold of Swift or Python for general-purpose development.
