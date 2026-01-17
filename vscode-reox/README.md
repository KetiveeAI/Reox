# REOX Programming Language

<p align="center">
  <img src="icon.png" width="128" alt="REOX Language Icon" />
</p>

<p align="center">
  <strong>The modern, declarative UI framework language for NeolyxOS</strong><br/>
  <span>Simpler than C++, Safer than C, AI-Native</span>
</p>

<p align="center">
  <a href="https://marketplace.visualstudio.com/items?itemName=ketiveeai.reox-language">
    <img src="https://img.shields.io/visual-studio-marketplace/v/ketiveeai.reox-language?style=flat-square&color=blue" alt="Version" />
  </a>
  <a href="https://marketplace.visualstudio.com/items?itemName=ketiveeai.reox-language">
    <img src="https://img.shields.io/visual-studio-marketplace/i/ketiveeai.reox-language?style=flat-square&color=green" alt="Installs" />
  </a>
  <a href="https://reox.ketivee.com">
    <img src="https://img.shields.io/badge/docs-online-blue?style=flat-square" alt="Documentation" />
  </a>
</p>

---

## Overview

**REOX** is a statically-typed, declarative programming language designed specifically for **NeolyxOS**. It combines the expressive power of modern UI composition (inspired by SwiftUI) with the predictable, system-level performance of C/C++.

Reox is built to be:
- **Native**: Directly integrates with NeolyxOS rendering primitives.
- **Safe**: Strong typing and memory safety features.
- **Fast**: Compiles to optimized native machine code (LLVM/C backend).
- **AI-First**: First-class support for AI model integration.

## Why Reox?

| Feature | Reox | C++ | Python | Swift |
|---------|------|-----|--------|-------|
| **UI Syntax** | Native | Library | Library | Native |
| **Performance** | System | System | Interpreted | System |
| **Simplicity** | High | Low | High | High |
| **AI Native** | Yes | No | Libraries | CoreML |
| **Target OS** | NeolyxOS | Any | Any | macOS/iOS |

## Key Features

- 🎨 **Declarative UI**: Build interfaces with a composable, intuitive syntax.
- 🔒 **Strong Typing**: Catch errors at compile-time with type inference.
- 🚀 **Zero Dependencies**: Standard library includes everything you need.
- 🤖 **AI Integration**: Built-in keywords for defining AI assistants and contexts.
- 🌙 **Dark Theme**: Includes a custom VS Code theme optimized for Reox.

---

## Code Examples

### 1. Hello World
```reox
fn main() {
    print("Hello from NeolyxOS!");
}
```

### 2. Declarative UI
Build beautiful, native UIs with less code.

```reox
window Calculator {
    title: "Calculator"
    size: (400, 300)

    @Bind result: string = "0"

    view {
        vstack(gap: 12) {
            text(result)
                .font_size(32)
                .color(0xFFFFFF)

            hstack(gap: 8) {
                button("7").on_click(action { append("7") })
                button("8").on_click(action { append("8") })
                button("9").on_click(action { append("9") })
            }
        }
    }
}
```

### 3. AI Integration
Reox treats AI as a first-class citizen.

```reox
ai assistant {
    model: "neolyx-gpt"

    fn suggest_action(context: string) -> string {
        return ai.prompt("Given context: {context}, suggest next action");
    }
}
```

---

## Installation

### Via VS Code Marketplace
Search for **"REOX Programming Language"** in the VS Code Extensions view (Ctrl+Shift+X) and verify the publisher is **ketiveeai**.

### Manual Installation
Download the `.vsix` release and run:
```bash
code --install-extension reox-language-1.0.0.vsix
```

---

## Contributing

We welcome contributions to the Reox ecosystem! This extension is open source.

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally.
3. **Create** a feature branch (`git checkout -b feature/my-feature`).
4. **Commit** your changes.
5. **Push** to the branch and submit a **Pull Request**.

### Reporting Issues
Please report bugs and feature requests on our [GitHub Issues](https://github.com/ketiveeai/reox/issues) page.

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

---

## Community & Resources

- **Documentation**: [reox.ketivee.com](https://reox.ketivee.com)
- **Organization**: [ketivee.com](https://ketivee.com)
- **Twitter**: [@KetiveeAI](https://twitter.com/KetiveeAI)

---

<p align="center">
  <strong>Built with ❤️ by KetiveeAI for NeolyxOS</strong>
</p>