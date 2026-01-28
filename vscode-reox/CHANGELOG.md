# Change Log

All notable changes to the REOX Language extension.

## [1.1.0] - 2026-01-20 (Pre-release)

### Added

- **Protocol/Extension/Typealias**: Full syntax highlighting for `protocol`, `extension`, and `typealias` declarations
- **Gesture Language**: Syntax highlighting for REOX gesture system (`gesture`, `on_tap`, `on_pan`, `on_swipe`, `on_pinch`, `on_rotate`)
- **Loop Statement**: Added `loop` keyword for infinite loops
- **Bitwise Operators**: Syntax highlighting for `~`, `^`, `<<`, `>>` bitwise operations
- **Null Coalescing**: Highlighting for `??` and `?.` optional chaining operators
- **Increment/Decrement**: Support for `++` and `--` operators
- **Extended IntelliSense**: Completions for 70+ keywords, types, and built-in functions
- **Enhanced Hover Docs**: Rich documentation for 30+ language constructs
- **Animation Functions**: Completions for `animate_*` functions (fade, slide, scale, rotate, pulse, bounce)
- **Color Functions**: Extended support for `color_lighten`, `color_darken`, `color_blend`, `color_hsl`

### Fixed

- Correct scoping for `where` clause (now control flow, not other)
- `maybe` moved to constants (optional type marker)
- `effect` added as a modifier keyword
- Improved operator precedence in syntax grammar

### Compatibility

- Works with VS Code 1.60.0 through 1.108.x (latest stable)

---

## [1.0.5] - 2026-01-18

### Fixed

- Minor syntax grammar fixes

## [1.0.4] - 2026-01-18

## [1.0.3] - 2026-01-18

### Fixed

- Updated license to Apache 2.0 to match language core
- Fixed overview icon visibility on Open VSX Registry

## [1.0.2] - 2026-01-18

### Added

- Async/await syntax highlighting (`async fn`, `await`)
- Error handling keywords (`guard`, `defer`, `try`, `catch`, `throw`)

### Fixed

- README icon now displays correctly on VS Marketplace

## [1.0.1] - 2026-01-15

### Fixed

- Minor syntax highlighting improvements

## [1.0.0] - 2026-01-12

### Added

- Initial release
- Syntax highlighting for `.reox` files
- REOX Dark theme
- Code snippets for common patterns
- Language configuration (brackets, comments, folding)

### Syntax Support

- Keywords: `fn`, `let`, `struct`, `kind`, `layer`, `panel`, `action`, `signal`, `emit`
- Control flow: `if`, `else`, `while`, `for`, `match`, `when`, `return`
- Types: primitives, sized types, custom types
- UI functions: `vstack`, `hstack`, `button`, `text`, `slider`, etc.
- Color functions: `color_rgba`, `color_blend`, `color_lighten`, etc.
- Animation functions: `animate_fade_in`, `animate_slide_in`, etc.
- Attributes: `@Bind`, `@State`, etc.
