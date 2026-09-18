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
