# REOX for NeolyxOS — Roadmap to a Swift-class Platform Language

> **Status:** Approved direction, 2026-10-25. This is the single brief every workstream
> works from. Update it when a decision changes; do not fork it.

## Goal

REOX is to NeolyxOS what Swift is to macOS: the one language for applications,
system services and the desktop shell. It must reach the usability of Swift while
keeping the determinism and auditability NeolyxOS needs for medical and scientific
deployments.

## Baseline (2026-10-25)

| Layer | State |
|---|---|
| Compiler `reox-lang/` (~11k LOC Rust) | Lexer, parser, type checker, C codegen, interpreter. 202 tests pass. No generics, no `self`/methods, no variant destructuring in `match`, `import` has no semantics, protocol conformance unchecked, 105 `unwrap()` and 60 `panic!` (mostly tests). |
| Runtime `reox-lang/runtime/` (~15.8k LOC C) | UI widgets, animation, themes, FFI. Depends on libc (`stdlib.h`, `stdio.h`, `dirent.h`, `unistd.h`) and SDL2. Does **not** link against NeolyxOS userspace (`-ffreestanding -nostdlib`, `desktop/lib/libc_stubs.c`). |
| NeolyxOS side | Seven `.reox` UI files under `desktop/apps/` with no build path; `gui/nxrender_c/src/reox_ffi.c` (284 lines) bridge; `desktop/include/nxsyscall.h` userspace syscall layer. |
| Tooling | `reoxc compile/run/init/new/pkg`, VS Code highlighter. No LSP, formatter, debugger or cross target. |

## Architecture decisions (fixed)

1. **C backend stays.** REOX compiles to C11 and is built by the same GCC toolchain
   and `user.ld` as every other NeolyxOS userspace binary. No LLVM in this phase.
2. **Memory model: ARC with `weak`, Swift-style.** Deterministic, no GC pauses,
   no borrow checker. Codegen emits `rx_retain` / `rx_release`; cycles are the
   programmer's responsibility via `weak`.
3. **Runtime platform seam.** `librx` = `rx_core` (zero OS deps) +
   `rx_platform_linux.c` (dev host, SDL allowed) + `rx_platform_neolyx.c`
   (built on `nxsyscall.h` + `nx_malloc`, nxrender is the only UI backend).
4. **`layer` / `panel` / `@Bind` compile to nxrender widget trees** through the
   existing `reox_ffi.c` bridge, which is finished, not replaced.
5. **Typed errors.** `Result<T, E>` and `variant`-based errors replace stringly
   `throw`. `try/catch` stays as sugar over `Result`.
6. **Two repositories.** REOX stays at its own repo; NeolyxOS consumes it through
   `neolyx-os/sdk/` (toolchain binary, headers, std). App Makefiles gain one rule:
   `%.c: %.rx` → `reoxc --target neolyx`.
7. **First real app: Calculator.app** rewritten in REOX, booted in QEMU.

## The five layers

```
reolab.app + reoxc pkg + reox-lsp        (Xcode / SPM)
NxUI    layer/panel/@Bind -> nxrender    (SwiftUI)
NxKit   fs, proc, ipc, time, config      (Foundation / AppKit)
reox-std Option/Result/String/Array/Map  (Swift stdlib)
reoxc + librx (C backend, ARC, platform) (swiftc + runtime)
```

## Workstreams

| # | Name | Repo | Done means |
|---|---|---|---|
| A | Compiler hardening | REOX | Zero non-test `unwrap`/`panic` in `src/`; every stage returns `Result` with spanned diagnostics; a corpus run over every `.rx`/`.reox` in both repos never crashes; `cargo clippy` clean; tests stay green. |
| B | Language core | REOX | `self` + methods on struct/extension; variant payload destructuring in `match` with exhaustiveness; monomorphised generics on fn/struct; protocol conformance checked; `import` resolves files and namespaces. Spec "Planned" examples compile and run. |
| C | ARC + typed errors | REOX | `rx_retain/release` emitted for heap types; `weak`; `Result<T,E>`; examples run leak-free under valgrind on the Linux host. |
| D | Runtime platform seam | REOX + NeolyxOS | `rx_core` has no libc/SDL includes; `rx_platform_neolyx.c` builds with `desktop/Makefile` flags into `librx_neolyx.a`; SDL isolated to `rx_platform_linux`. |
| E | NeolyxOS SDK integration | NeolyxOS | `sdk/` layout; `reoxc --target neolyx`; Makefile rule; Calculator.app in REOX renders in the booted desktop (QEMU screenshot). |
| F | Tooling | REOX | `reox-lsp` (diagnostics, hover, goto-def), `reoxc fmt`; wired into VS Code and reolab.app. |

**Ordering:** A first (it touches every compiler module). B and C start after A lands,
on separate modules. D, E, F are independent of A and of each other and start at once.
E depends on D only for the final boot step.

## Non-negotiables for every workstream

- No `unwrap()`, no unchecked calls, no silently swallowed errors.
- Validate every pointer and size in C; free every allocation.
- No Linux/POSIX assumptions in anything that builds for NeolyxOS.
- Keep existing tests green; add tests for every new rule.
- No GUI code in the kernel. REOX runs in userspace only.
