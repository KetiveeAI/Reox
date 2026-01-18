# Reox

**A modern, easy-to-use UI and general purpose programming language that compiles to C for fast performance.**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Open VSX](https://img.shields.io/badge/Open%20VSX-v1.0.1-green)](https://open-vsx.org/extension/ketiveeai/reox-language)

## Overview

Reox is a declarative UI framework language designed for building native applications. It combines Swift-like syntax with C-level performance through ahead-of-time compilation.

## Features

- **Easy syntax** — clean, expressive, and familiar
- **Compiles to C** — fast compile times, native performance
- **Declarative UI** — React/SwiftUI-style component model
- **Pattern matching** — `match` expressions with exhaustive checking
- **Bitwise ops** — full support for `&`, `|`, `^`, `<<`, `>>`
- **Memory safe** — no manual memory management in UI code

## Quick Start

```reox
fn main() {
    let app = app_new("Hello");
    let win = app_create_window(app, "Welcome", 800, 600);

    let btn = button_primary("Click Me");
    window_set_root(win, btn);

    app_run(app);
}
```

## Installation

```bash
cargo install --path reox-lang
```

## Documentation

- [Language Guide](https://reox.ketivee.com)
- [License](https://reox.ketivee.com/license)

## License

Apache 2.0 — see [LICENSE](LICENSE)
