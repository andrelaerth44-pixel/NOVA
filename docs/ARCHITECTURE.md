# NOVA compiler architecture

NOVA is being built as a compiler and runtime ecosystem rather than a single interpreter.

Pipeline:

source -> lexer -> parser -> AST -> semantic analysis -> typed NOVA IR -> optimization -> target backend -> native object or executable

IR layers:
- Core IR: control flow, values, calls, memory, branches and returns.
- Data IR: arrays, maps, structs, strings and ownership metadata.
- Native IR: pointers, ABI calls, alignment, atomics and SIMD.
- Tensor IR: contiguous tensors, shape/stride metadata, dtype, fused operations and memory planning.
- UI IR: application trees, state, events, layout and platform bindings.
- Graphics IR: 2D paths, images, meshes, shaders, render passes and GPU resources.

Optimization targets:
- constant folding
- dead-code elimination
- inlining
- common-subexpression elimination
- escape analysis
- allocation elimination
- vectorization
- loop transformations
- buffer reuse
- kernel fusion
- target-specific lowering

The runtime owns capabilities that cannot or should not be generated directly by the compiler. C ABI and FFI provide a stable bridge to existing native libraries.

NOVA does not promise that every program is faster than every program written in another language. The objective is to make high-level code compile to efficient native operations while preserving explicit access to low-level facilities when required.
