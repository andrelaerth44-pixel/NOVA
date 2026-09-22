# NOVA

Simple. Native. Fast. Universal. Beautiful.

NOVA is a general-purpose programming language for applications, services, tools, games, graphics, automation, data systems, networking, native integrations and AI.

Current development line: **2.0.0-dev**

The repository keeps implementation claims honest: a capability is considered supported only after executable code and validation exist.

## Implemented core

- variables, assignment, numbers, strings, booleans and arrays
- arithmetic, comparisons, boolean operators and control flow
- functions, return, imports and module graph validation
- structs, enums, pattern matching and generic functions
- type inference, Option / Result and \`?\` propagation
- maps, sets, indexed access, lexical closures and first-class iterators
- deterministic compiler pipeline, legacy stack IR and SSA inspection/verification
- constant folding and branch simplification in the optimization pass
- filesystem, environment, paths, time and JSON runtime services
- process handles, channels and parallel numeric helpers
- HTTP (plain HTTP), process execution and SHA-256 runtime helpers
- dependency-free tensor/matrix operations and a small trainable MLP
- CUDA, Vulkan and Metal kernel-source generation
- SVG, WAV, HTML and ffmpeg media helpers
- package initialization, local/path/git/registry resolution and lockfile generation
- Android-native project generation with `Main.nova` source and Android platform NativeActivity metadata; no Kotlin/Java source is generated

## Development targets

The following are present as explicit implementation layers but are not yet equivalent to mature production backends:

- C backend: executable numeric subset already used by CI
- x86-64 and ARM64: target emitters for a small verified subset
- WebAssembly: WAT emitter for a small verified subset
- Android: native NOVA source/package target is defined; final APK linking still requires the NOVA ARM64 Android runtime/linker implementation
- GPU: kernel/shader emitters; device execution is target-runtime dependent
- self-hosting: bootstrap boundary is defined, but the trusted compiler remains Rust until the NOVA compiler is completely bootstrapped

## CLI examples

\`\`\`text
nova run examples/hello.nova
nova check examples/hello.nova
nova ir examples/hello.nova
nova ssa examples/ssa.nova

nova ai-train-xor 500
nova concurrency-demo
nova gpu-kernels
nova media-demo ./nova-media
nova selfhost-check

nova package-init ./my-app my-app
nova package-install ./my-app
nova build-android-project examples/obra360.nova ./android-project
\`\`\`

NOVA 2.0 is an active native-language implementation line. The project deliberately distinguishes implemented subsets from production-complete compiler targets; no foreign language is part of NOVA application source.
