# NOVA language specification

NOVA source files use .nova and UTF-8.

Core surface syntax is intentionally small. Variables, functions, conditionals, loops, arrays, arithmetic, comparisons, boolean operators and function calls form the base language.

Example:

fn add(a, b) {
    return a + b
}

name = "NOVA"
print "Hello " + name
print add(20, 22)

The type system is being extended toward explicit integer widths, floating-point types, booleans, strings, arrays, maps, structs, enums, references, pointers, buffers and user-defined types.

Native model:
safe application code -> standard library -> NOVA IR -> native runtime / FFI -> OS + CPU + GPU

Planned native targets: x86-64, ARM64, WebAssembly and RISC-V.
Planned acceleration targets: CUDA, Vulkan compute and Metal.

The compiler must report unsupported targets honestly; no backend is advertised before it actually compiles for that target.

Standard library layers are planned around core, collections, filesystem, paths, processes, time, networking, HTTP, JSON, crypto, databases, UI, graphics, audio, video, AI and native integration.


## Explicit types

NOVA supports optional explicit annotations while retaining concise syntax:

```nova
let age: i64 = 18
let name: string = "NOVA"
let values: i64[] = [1, 2, 3]

fn add(a: i64, b: i64) -> i64 {
    return a + b
}
```

Supported primitive names currently include `i32`, `i64`, `f32`, `f64`, `bool`, `string`, `void`, and `any`. Array types use `T[]`.


## Structs

Structs declare named fields and can be constructed with field names:

```nova
struct User {
    name: string
    age: i64
}

user = User { name: "Laerth", age: 18 }
print user.name
```

## Enums and pattern matching

Enums can carry an optional payload. Option and Result are built-in enum families for nullable and fallible values.

```nova
enum State {
    Ready
    Failed(string)
}

state = Failed("network")

match state {
    Ready {
        print "ready"
    },
    Failed(message) {
        print message
    },
    _ {
        print "unknown"
    }
}
```

Match patterns currently support literal values, `_` wildcard patterns, and enum variants with one binding. The checker validates enum variants and reports missing variants when a match has neither a wildcard nor an `else` arm.


## Option and Result helpers

NOVA provides built-in Option/Result constructors:

```nova
value = Some(42)
missing = None
success = Ok("done")
failure = Err("network")
```

The runtime also provides `is_some`, `is_none`, `is_ok`, `is_err`, `unwrap`, and `unwrap_or`. These form the current safe value-inspection layer while the language-level propagation operator is still being implemented.


## Closures

Function expressions can capture the surrounding lexical environment:

```nova
fn make_adder(x) {
    return fn(y) {
        return x + y
    }
}

add10 = make_adder(10)
print add10(32)
```

Closures are values and can be assigned to variables and called like functions. The runtime captures the environment at closure creation time.


## Generics

Generic functions use type parameters declared after the function name. Type arguments are inferred from the call, so callers normally do not need to write them explicitly:

    fn identity<T>(value: T) -> T {
        return value
    }

    print identity(42)
    print identity("NOVA")

Generic types use angle brackets:

    choice: Option<i64> = Some(42)
    result: Result<string, string> = Ok("done")

The current compiler represents Option<T> and Result<T, E> as parameterized types and propagates their concrete payload types through type inference and pattern bindings.
