# Change Log

All notable changes to the REOX Language extension.

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
