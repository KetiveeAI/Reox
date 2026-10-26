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
| Try/Catch/Throw (typed errors)    | Done            | Done      | Done    | Partial     |
| ARC (`ref struct`, `weak`)        | Done            | Done      | Done    | -           |
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

## Memory Model: Automatic Reference Counting

REOX manages memory with automatic reference counting (ARC), the same model
Swift uses. There is no garbage collector, no collection pauses and no borrow
checker. The compiler inserts every retain and release; the programmer only
has to know which values are references and how to avoid reference cycles.

### Value types and reference types

| Kind | Types | Stored | Copied on assignment |
|---|---|---|---|
| Value | `int`, `float`, `bool`, fixed-width integers, `struct`, `variant` | inline (C struct by value) | yes, field by field |
| Reference | `string`, arrays `[T]`, maps `[string: V]`, `ref struct`, `layer`, `Error` | on the heap, behind a pointer | no, the reference is shared |

A `ref struct` is declared like a struct and used like one, but every instance
lives on the heap and is shared by reference:

```reox
ref struct Node {
    value: int,
    next: Node?,          // strong reference, optional
    weak parent: Node     // weak reference, reads as Node?
}

let a = Node { value: 1 };
let b = a;                // b and a are the same object
```

A plain `struct` may hold reference-typed fields; copying the struct retains
each such field and releasing the struct releases them.

### What the compiler guarantees

- Every reference is released exactly when its last owner goes away: at the
  end of the scope that declared a binding, on reassignment, when the struct
  or array or map holding it is freed, and when a temporary's statement ends.
- Releases happen on every exit path: fall-through, `return`, `break`,
  `continue`, `throw` and a failed `try`.
- `defer` blocks run before the function's locals are released, in reverse
  order of declaration, on every exit path.
- Returning a reference transfers ownership to the caller; passing one as an
  argument lends it for the duration of the call.
- Deallocation is synchronous and deterministic: an object is freed at the
  moment its count reaches zero, so the order of frees is the same on every
  run.
- Overflowing a reference count or releasing a freed object aborts the
  program instead of corrupting memory.

### Cycles and `weak`

ARC does not collect cycles. Two objects that hold strong references to each
other keep each other alive forever:

```reox
let mut a = Node { value: 1, next: nil };
let b = Node { value: 2, next: a };
a.next = b;               // a -> b -> a: neither is ever freed
```

Break every cycle with a `weak` reference on the "back" edge. A weak slot does
not keep its target alive and reads as an optional (`T?`) that becomes `nil`
once the target is gone:

```reox
ref struct Parent { name: string, child: Child? }
ref struct Child { weak parent: Parent }

let mut p = Parent { name: "root", child: nil };
let c = Child { parent: p };
p.child = c;              // c -> p is weak, so this is not a cycle

if let owner = c.parent { println(owner.name); } else { println("gone"); }
```

`weak` is allowed on fields of a `ref struct` and on local bindings
(`weak let w = node;`), and only for reference types. Use `if let`, `?.` or
`??` to read a weak or optional reference; accessing a field through `T?`
directly is a type error.

### Mutability

A binding declared with `let` cannot be reassigned, and nothing can be mutated
through it, reference types included: `let n = Node { value: 1 }; n.value = 2;`
is an error. Declare the binding `let mut` to mutate the object it refers to.

### Strings at the C boundary

Inside REOX a `string` is a reference counted object. In an `extern fn`
signature, `string` parameters and results are plain C strings (`const char*`):
arguments are passed as borrowed C strings for the duration of the call, and a
returned C string is copied into a new REOX string. Other reference types cross
`extern` unchanged, and an `extern fn` that returns one must return a +1
reference.

## Error Model: Typed Errors

Errors are ordinary values of types that conform to the intrinsic `Error`
protocol (declared in the prelude as `protocol Error {}`). An error type must be
a `ref struct`:

```reox
ref struct ParseError { code: int, text: string }
extension ParseError: Error { }
```

A function that can fail is marked `throws`; it returns either its value or an
error. Call it with `try`, which propagates the error to the caller when the
call fails:

```reox
fn parse(n: int) throws -> int {
    if n < 0 { throw ParseError { code: 42, text: "negative" }; }
    return n;
}

fn forward(n: int) throws -> int {
    let v = try parse(n);     // on error: locals released, error returned
    return v * 2;
}
```

Rules:

- `throw` is allowed in a function declared `throws` or inside a `try { }`
  block. The thrown value must conform to `Error`.
- A call to a `throws` function must be marked `try`; `try` on a call that
  cannot throw is an error.
- `try` on its own is only allowed in a `throws` function or inside a
  `try { } catch` block.
- `main` cannot be declared `throws`; catch errors inside it.
- `catch e` binds the error as `Error`; `catch e as ParseError` binds it as
  that type and lets any other error continue to the next enclosing handler
  (or to the caller of a `throws` function).

```reox
try {
    try {
        let v = try forward(-1);
        println(v);
    } catch e as ParseError {
        println("parse failed: " + str(e.code));
    }
} catch e {
    println("some other error");     // anything ParseError did not catch
}
```

A `try` statement has exactly one `catch` clause; nest `try` blocks to handle
several error types.

`fn f() throws -> T` is sugar for returning `Result<T, Error>`: the generated C
function returns a `Result__T__Error` struct holding either the value or the
error object. No `setjmp`/`longjmp` is involved, so every path is a normal
return and ARC releases apply as usual. The error object is owned by whoever
catches it and is released when the catch block ends.

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
