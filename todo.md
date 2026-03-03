# REOX Language — Project Status & TODO

> **Last updated**: 2026-03-03  
> **Status**: Active Development — Compiler Core Working, UI Language Features In Progress

---

## What Works Today

| Component             | Status        | Notes                                                                                                                                               |
| --------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Lexer**             | Complete      | All keywords tokenized (including UI keywords), full operator set, string/number literals, hex support, position tracking                           |
| **Parser**            | Core Complete | Functions, structs, imports, extern, if/while/for, match, guard, defer, try/catch, throw, async/await, trailing closures, optional chaining, ranges |
| **Type Checker**      | Core Complete | Type inference, function signatures, struct fields, binary/unary ops, member access, indexing                                                       |
| **Code Generator**    | Core Complete | C transpilation for all parsed constructs, struct generation, extern declarations, control flow                                                     |
| **Interpreter**       | Core Complete | Tree-walking evaluator, built-in functions (print, math, string, array, map), REPL mode                                                             |
| **C Runtime**         | Extensive     | 50+ files: UI widgets, animation system, transitions, themes, color system, SDL backend, FFI, accessibility, display, desktop integration           |
| **Stdlib (.rx)**      | Extensive     | 649-line prelude with extern declarations for UI (views, widgets, forms, shapes, effects, media, state management, timers, platform API)            |
| **CLI**               | Complete      | `reoxc compile`, `reoxc run`, `reoxc init`, `reoxc new`, `reoxc pkg`, project templates                                                             |
| **Package Manager**   | Complete      | `reoxc pkg init/add/remove/install/list/publish`                                                                                                    |
| **Benchmarks**        | Working       | Fibonacci benchmark comparing REOX vs C vs Rust                                                                                                     |
| **VS Code Extension** | Working       | Syntax highlighting for `.rx`/`.reox` files                                                                                                         |

---

## What's In Progress

### Parser — UI-Specific Language Constructs

The lexer already tokenizes these keywords, but the parser does **not yet consume them**. These are the constructs needed to write REOX UI code natively:

| Keyword            | Purpose                                                 | Priority | Status      |
| ------------------ | ------------------------------------------------------- | -------- | ----------- |
| `variant`          | Enum type with named variants (like Swift enum)         | **P0**   | Not started |
| `protocol`         | Trait/interface declarations                            | **P0**   | Not started |
| `extension`        | Add methods to existing types                           | **P0**   | Not started |
| `layer`            | UI view component (like SwiftUI View)                   | **P0**   | Not started |
| `panel`            | Top-level window definition                             | **P0**   | Not started |
| `@Bind`            | Reactive state variable annotation                      | **P1**   | Not started |
| `signal` / `emit`  | Reactive event system                                   | **P1**   | Not started |
| `const`            | Compile-time constants                                  | **P1**   | Not started |
| `static`           | Static members                                          | **P1**   | Not started |
| `typealias`        | Type aliases                                            | **P2**   | Not started |
| `pub`              | Visibility modifier                                     | **P2**   | Not started |
| `action`           | First-class closure/lambda                              | **P2**   | Not started |
| `gesture` keywords | `on_tap`, `on_pan`, `on_swipe`, `on_pinch`, `on_rotate` | **P2**   | Not started |

### Type System Additions Needed

- `Optional` type (`int?`, `string?`)
- `Function` type (`fn(int) -> bool`)
- `Tuple` type (`(int, string)`)
- Protocol conformance checking
- `variant` constructor type inference

---

## How to Contribute

### Prerequisites

- Rust (stable, 2021 edition)
- Understanding of recursive descent parsing
- Familiarity with AST design

### Build & Test

```bash
cd reox-lang

# Build
cargo build

# Run all tests
cargo test

# Compile a .rx file
cargo run -- compile tests/hello.rx

# Run a .rx file (interpreter mode)
cargo run -- run tests/hello.rx
```

### Key Source Files

| File                       | Purpose                                       |
| -------------------------- | --------------------------------------------- |
| `src/lexer/token.rs`       | Token definitions — all keywords live here    |
| `src/lexer/mod.rs`         | Tokenizer implementation                      |
| `src/parser/ast.rs`        | AST node definitions — **add new nodes here** |
| `src/parser/mod.rs`        | Parser rules — **add new parse methods here** |
| `src/typechecker/mod.rs`   | Type checking logic                           |
| `src/typechecker/types.rs` | Resolved type definitions                     |
| `src/codegen/mod.rs`       | C code generation                             |
| `src/interpreter/mod.rs`   | Tree-walking interpreter                      |
| `src/stdlib/`              | Built-in function implementations (Rust)      |
| `stdlib/prelude.rx`        | Standard library declarations (REOX)          |
| `stdlib/ui.rx`             | UI stdlib declarations (REOX)                 |
| `runtime/`                 | C runtime library (widgets, animation, FFI)   |
| `tests/smoke.rs`           | Integration tests                             |

### Contribution Areas (Ordered by Impact)

1. **Parser extensions** — Add `parse_variant_decl()`, `parse_protocol_decl()`, `parse_layer_decl()` etc. to `src/parser/mod.rs` with corresponding AST nodes in `ast.rs`
2. **Codegen for new nodes** — Generate C code for the new AST constructs
3. **Interpreter support** — Add evaluation logic for new constructs
4. **Test cases** — Write `.rx` test files and Rust integration tests for new features
5. **Runtime expansion** — Extend C runtime with missing widget/effect implementations

### Code Standards

- Zero external dependencies in the compiler (exception: AI module uses `reqwest`/`serde`)
- All new parser rules must have unit tests
- No commented-out code, no placeholder logic
- Follow existing naming patterns in the codebase

---

## Architecture Quick Reference

```
Source (.rx)
    |
    v
[Lexer] -> Token stream (src/lexer/)
    |
    v
[Parser] -> AST (src/parser/)
    |
    v
[Type Checker] -> Validated AST (src/typechecker/)
    |
    v
[Code Generator] -> C source (src/codegen/)
    |
    v
[C Compiler + Runtime] -> Native binary (runtime/)
```

Interpreter path (for `reoxc run`):

```
Source -> Lexer -> Parser -> AST -> Interpreter (src/interpreter/)
```
