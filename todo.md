# REOX Language — Project Status & TODO

> **Last updated**: 2026-03-13  
> **Status**: Active Development — UI Language Features Complete, Stabilizing for Production

---

## What Works Today

| Component             | Status        | Notes                                                                                                                                               |
| --------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Lexer**             | Complete      | All keywords tokenized (including UI keywords), full operator set, string/number literals, hex support, position tracking                           |
| **Parser**            | Complete      | Functions, structs, UI constructs (variant, protocol, layer, panel, const, typealias, action, @Bind, emit, gestures), control flow                  |
| **Type Checker**      | Complete      | Type inference, explicit integer widths (`i8`-`usize`), function signatures, struct fields, binary/unary ops, type coercion                         |
| **Code Generator**    | Complete      | C transpilation for all AST nodes, `<stdint.h>` type mapping, proper `setjmp`/`longjmp` error propagation, C/C++/Rust FFI                           |
| **Interpreter**       | Complete      | Tree-walking evaluator, native built-ins (math, string, array, map, file IO, AI ops), UI struct evaluation, closure and environment capture         |
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
| `variant`          | Enum type with named variants (like Swift enum)         | **P0**   | ✅ Done      |
| `protocol`         | Trait/interface declarations                            | **P0**   | ✅ Done      |
| `extension`        | Add methods to existing types                           | **P0**   | ✅ Done      |
| `layer`            | UI view component (like SwiftUI View)                   | **P0**   | ✅ Done      |
| `panel`            | Top-level window definition                             | **P0**   | ✅ Done      |
| `@Bind`            | Reactive state variable annotation                      | **P1**   | ✅ Done      |
| `signal` / `emit`  | Reactive event system                                   | **P1**   | ✅ Done      |
| `const`            | Compile-time constants                                  | **P1**   | ✅ Done      |
| `static`           | Static members                                          | **P1**   | ✅ Done      |
| `typealias`        | Type aliases                                            | **P2**   | ✅ Done      |
| `pub`              | Visibility modifier                                     | **P2**   | ✅ Done      |
| `action`           | First-class closure/lambda                              | **P2**   | ✅ Done      |
| `gesture` keywords | `on_tap`, `on_pan`, `on_swipe`, `on_pinch`, `on_rotate` | **P2**   | ✅ Done      |

### Type System Additions (v1.2.0)

- ✅ Explicit integer widths: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `usize`
- `Optional` type (`int?`, `string?`) — parsed, needs interpreter expansion
- `Function` type (`fn(int) -> bool`) — parsed, codegen done
- `Tuple` type (`(int, string)`) — parsed, codegen done
- Protocol conformance checking — future
- `variant` constructor type inference — future

---

## Future Tasks & Roadmap

- **Production Stabilization**: Harden the compiler pipeline and interpreter for production readiness. Address edge cases in parsing, type-checking, and code generation.
- **Calculator App Revamp**: The current `calculator.app` feels like a dummy application. Since REOX now has robust math support in the core language (recursion, loops, arithmetic, math built-ins), the calculator app needs to be verified and rewritten in pure REOX to fully validate the language's capabilities.
- **Protocol Conformance Checking**: Implement strict type checking for protocol conformance (`extension Type : Protocol`).
- **Variant Constructor Type Inference**: Improve type inference for variant payloads and pattern matching.
- **Advanced UI State Management**: Further refine `@Bind` and reactive data flow patterns.

## Upcoming Bug Fixes

- **Nested Scope Parsing**: Fix parser failing on `let` variable declarations inside nested block scopes (e.g., inside a `while` loop body, causing `expected identifier, found Let` error).
- **Mutable Assignment in Loops**: Resolve scoping/parsing issues where `mut` variable assignments in nested loops occasionally trigger syntax errors.
- **Interpreter Consistency**: Ensure all native interpreter functions (like `println`) have consistent behavior mirroring the generated C code output.

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
