# REOX Programming Language

A modern, declarative UI framework language for building cross-platform user interfaces.

## Overview

REOX is a statically-typed, declarative language inspired by SwiftUI, designed for building user interfaces with predictable behavior and system-level performance.

```reox
fn main() {
    let app = App("My App")
    let window = app.create_window("Hello", 800, 600)
    
    window.set_root(
        VStack(16) {
            Text("Hello, REOX!")
                .font_size(24)
            
            Button("Click Me")
                .background(color_blue())
                .on_click(|| println("Clicked!"))
        }
    )
    
    app.run()
}
```

## Features

- **Declarative UI** - Component-based model inspired by SwiftUI
- **Strong Typing** - Static typing with type inference
- **Zero Dependencies** - Compiles to optimized C code
- **Cross-Platform** - Works on Linux, macOS, Windows, and NeolyxOS
- **Transparent & Secure** - Zero telemetry, no hidden runtimes, and open architecture

## Installation

### From Source

```bash
cd reox
cargo build --release
```

### Usage

```bash
# Compile REOX to C
reoxc main.reox -o main.c

# Compile to executable
reoxc main.reox --emit exe -o myapp
```

## Directory Structure

```
reox/
├── src/           # Compiler (Rust)
│   ├── lexer/     # Tokenizer
│   ├── parser/    # AST generation
│   ├── analyzer/  # Type checking
│   └── codegen/   # C code generation
├── runtime/       # C runtime library
├── stdlib/        # Standard library
├── examples/      # Example programs
└── tests/         # Test suite
```

## License

MIT License - Copyright (c) 2025 KetiveeAI

## Links

- Documentation: https://reox.ketivee.com
- VS Code Extension: https://open-vsx.org/extension/swanaya/reox-language
- Organization: https://ketivee.com
