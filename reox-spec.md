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
| Imports (`import a.b.c;`)         | Done            | Done      | Done    | Done        |
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
| `variant` (tagged union)          | Done            | Done      | Done    | Done        |
| Match on variant payloads         | Done            | Done      | Done    | Done        |
| Match exhaustiveness check        | -               | Done      | -       | -           |
| Methods and `self` (`mut fn`)     | Done            | Done      | Done    | Done        |
| Generics (monomorphised)          | Done            | Done      | Done    | Done        |
| `protocol` (static dispatch)      | Done            | Done      | Done    | Done        |
| `extension` (type ext)            | Done            | Done      | Done    | Done        |
| `layer` (UI component)            | Done            | Done      | Done    | Done        |
| `panel` (window)                  | Done            | Done      | Done    | Done        |
| `@Bind` (reactive state)          | Done            | As `let`  | As `let`| As `let`    |
| `signal`/`emit`                   | Done            | -         | Partial | -           |
| `const`/`static`                  | Done            | Done      | Done    | Done        |
| `typealias`                       | Done            | Done      | Done    | Done        |
| `pub` visibility (across files)   | Done            | Done      | -       | -           |
| Gesture handlers                  | Done            | -         | Done    | -           |

Rows marked `-` have nothing to do at that stage. Every row above is covered
by tests under `reox-lang/src` and `reox-lang/tests`; the three examples
below marked with a test file are compiled to C, built against `runtime/`
with gcc and run by `reox-lang/tests/spec_examples.rs`.

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

### Variant Types

Test: `reox-lang/tests/spec/variant_result.rx`.

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

fn describe(result: Result) {
    match result {
        Ok(v) => print_int(v),
        Err(msg) => println(msg)
    }
}
```

A `match` must be exhaustive: the compiler lists the missing cases
(`missing: Some(Err(_)), Nothing`). Patterns nest (`Some(Ok(v))`), may be
qualified (`Result.Ok(v)`), and `_` or a binding catches the rest. Cases are
constructed as `Ok(42)`, `Result.Ok(42)` or the legacy `Result_Ok(42)`;
payloads may be positional (`Some(T)`) or named (`Err(message: string)`).

### Methods and `self`

```reox
struct Counter {
    count: int

    fn value() -> int { return count; }       // implicit self.count
    mut fn bump(by: int) { self.count += by; } // mutation needs `mut fn`
    static fn make() -> Counter { return Counter { count: 0 }; }
}

extension Counter {
    mut fn twice() { self.bump(1); self.bump(1); }
}

let mut c = Counter.make();   // a mut method needs a `let mut` receiver
c.twice();
```

Methods declared inside `struct`, `extension`, `layer` and `panel` bodies
take an implicit `self`; `obj.method(args)` dispatches statically and
compiles to `TypeName_method(TypeName* self, ...)`.

### Generics

```reox
fn swap<T>(a: T, b: T) -> T { return b; }
struct Box<T> { value: T  fn get() -> T { return self.value; } }
variant Option<T> { Some(T), None }          // in the prelude
variant Result<T, E> { Ok(T), Err(E) }       // in the prelude

let b = Box { value: 41 };                   // Box<int>, mangled Box__int
let o: Option<int> = None;                   // type arguments from context
fn show<T: Drawable>(x: T) { x.draw(); }     // bounds are protocols
```

Generic declarations are instantiated per concrete type set at their use
sites (monomorphisation); type arguments are inferred from the arguments
and from the expected type.

### Layer — UI Component

Test: `reox-lang/tests/spec/layer_button.rx` (links against the runtime's
`button_view`, `button_set_style` and theme colors from `import prelude;`).

```reox
import prelude;

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

let button = MyButton { pressed: false, label: "Press me" };
let view: View = button.body();
```

### Panel — Window

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

### Protocol — Interface

Test: `reox-lang/tests/spec/protocol_drawable.rx`.

```reox
protocol Drawable {
    fn draw(canvas: View);
    fn bounds() -> Rect;
}

struct Circle { radius: float }

extension Circle : Drawable {
    fn draw(canvas: View) {
        println("drawing circle");
    }
    fn bounds() -> Rect {
        return rect(0.0, 0.0, self.radius * 2.0, self.radius * 2.0);
    }
}

fn render<T: Drawable>(item: T, canvas: View) {
    item.draw(canvas);      // static dispatch, no vtable
}
```

`extension Type : Protocol` must implement every protocol method with the
same signature; missing or mismatched methods are listed in one diagnostic.
Protocols are not types: they are used as generic bounds.

### Modules

```reox
import geometry;        // geometry.rx next to the entry file, or in stdlib/
import util.math;       // util/math.rx

let r = rect(0.0, 0.0, 2.0, 3.0);   // pub items are available unqualified
let a = geometry.area(r);           // or qualified by module name
```

Every file is a module named after its stem. `pub` controls what other
files may use; using a private item, or an item of a module the file does
not import, is a diagnostic with a hint. Import cycles are reported.

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
