# Reox Language Specification v1.0

## Goals

- Simpler than C++, safer than C
- Built for NeolyxOS
- AI-native syntax
- Fast compilation
- Easy to learn

## Implementation Status

| Feature                           | Parser          | TypeCheck | Codegen | Interpreter |
| --------------------------------- | --------------- | --------- | ------- | ----------- |
| Functions (`fn`)                  | Done            | Done      | Done    | Done        |
| Structs (`struct`)                | Done            | Done      | Done    | Done        |
| Imports (`import`)                | Done            | -         | Done    | Done        |
| Extern (`extern fn`)              | Done            | Done      | Done    | Done        |
| Variables (`let`/`mut`)           | Done            | Done      | Done    | Done        |
| Control flow (`if`/`while`/`for`) | Done            | Done      | Done    | Done        |
| Match expressions                 | Done            | Done      | Done    | Done        |
| Guard/Defer                       | Done            | Done      | Done    | Done        |
| Try/Catch/Throw                   | Done            | Done      | Done    | Done        |
| Async/Await                       | Done            | Done      | Done    | Done        |
| Trailing closures                 | Done            | -         | Done    | Done        |
| Optional chaining (`?.`)          | Done            | -         | Done    | Done        |
| Null coalescing (`??`)            | Done            | -         | Done    | Done        |
| Compound assign (`+=`)            | Done            | Done      | Done    | Done        |
| Increment/Decrement               | Done            | Done      | Done    | Done        |
| Bitwise operators                 | Done            | Done      | Done    | Done        |
| Range expressions (`..`)          | Done            | -         | Done    | Done        |
| `variant` (enum type)             | **Not started** | -         | -       | -           |
| `protocol` (interface)            | **Not started** | -         | -       | -           |
| `extension` (type ext)            | **Not started** | -         | -       | -           |
| `layer` (UI component)            | **Not started** | -         | -       | -           |
| `panel` (window)                  | **Not started** | -         | -       | -           |
| `@Bind` (reactive state)          | **Not started** | -         | -       | -           |
| `signal`/`emit`                   | **Not started** | -         | -       | -           |
| `const`/`static`                  | **Not started** | -         | -       | -           |
| `typealias`                       | **Not started** | -         | -       | -           |
| `pub` visibility                  | **Not started** | -         | -       | -           |
| Gesture handlers                  | **Not started** | -         | -       | -           |

## Syntax Reference

### Hello World

```reox
fn main() {
    print("Hello from Reox!");
}
```

### Variables & Types

```reox
let x: int = 42;              // Immutable by default
let mut y: int = 10;          // Mutable with 'mut'
let name: string = "NeolyxOS";
let active: bool = true;
```

### Functions

```reox
fn add(a: int, b: int) -> int {
    return a + b;
}

// Short form
fn multiply(a: int, b: int) -> int = a * b;
```

### Structures

```reox
struct User {
    id: int,
    name: string,
    email: string
}

let user = User {
    id: 1,
    name: "Swana",
    email: "swana@neolyx.os"
};
```

### Pattern Matching

```reox
match command {
    "save" => save_file(),
    "load" => load_file(),
    "exit" => return,
    _ => print("Unknown command")
}
```

### Variant Types (Planned)

```reox
variant Direction {
    Up,
    Down,
    Left,
    Right
}

variant Result {
    Ok(value: int),
    Err(message: string)
}

match result {
    Ok(v) => print_int(v),
    Err(msg) => print(msg)
}
```

### Layer — UI Component (Planned)

```reox
layer MyButton {
    @Bind let pressed: bool = false;
    let label: string;

    signal on_click: int;

    fn body() -> View {
        let btn = button_view(label);
        button_set_style(btn, color_primary(), color_secondary(), color_accent());
        return btn;
    }
}
```

### Panel — Window (Planned)

```reox
panel MainWindow {
    title: "Calculator",
    size: (400, 300),

    fn root() -> View {
        let stack = vstack(10.0);
        let input = text_field("Enter number");
        let btn = button_view("Calculate");
        let output = text_view("Result: ");

        view_add_child(stack, input);
        view_add_child(stack, btn);
        view_add_child(stack, output);
        return stack;
    }
}
```

### Protocol — Interface (Planned)

```reox
protocol Drawable {
    fn draw(canvas: View);
    fn bounds() -> Rect;
}

extension Circle {
    fn draw(canvas: View) {
        // implementation
    }
    fn bounds() -> Rect {
        return rect(0.0, 0.0, self.radius * 2.0, self.radius * 2.0);
    }
}
```

### AI Integration

```reox
// Uses built-in ai.generate() function
fn main() {
    let response = ai_generate("gemini-2.0-flash", "Explain REOX language");
    print(response);
}
```

## Compilation Pipeline

```
.rx source file
     |
Lexer (tokenize)
     |
Parser (build AST)
     |
Type Checker (validate)
     |
C Code Generator
     |
C Compiler + REOX Runtime
     |
Native Executable (ELF)
```

## File Extensions

- `.rx` — REOX source files
- `.reox` — REOX source files (legacy, both supported)

## C Runtime

The REOX runtime is a C library providing:

- UI widget rendering (via SDL backend)
- Animation system with easing functions
- Theme/color management
- State management with diffing
- Transition system
- FFI bridge for C/C++ interop
- Accessibility support
- Desktop integration

Build: `cd runtime && make`

## Package Manager

```bash
reoxc pkg init               # Initialize project
reoxc pkg add <name>          # Add dependency
reoxc pkg remove <name>       # Remove dependency
reoxc pkg install             # Install all deps
reoxc pkg list                # List installed packages
```

## Contributing

See `todo.md` for full project status, contribution areas, and build instructions.
