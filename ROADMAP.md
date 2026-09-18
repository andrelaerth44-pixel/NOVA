# NOVA roadmap

The roadmap is an implementation order, not a claim that unfinished features already exist.

## 1.5 hardening
- source spans and line/column diagnostics
- deterministic parser errors
- static semantic checking
- safe runtime error handling
- module path resolution and cycle diagnostics
- language conformance tests

## 1.6 language system
- explicit primitive types
- arrays, maps and sets
- structs and enums
- Option and Result
- pattern matching beyond literal equality
- closures
- iterators and generators

## 1.7 compiler
- typed NOVA IR
- control-flow lowering
- optimization passes
- C ABI and FFI
- real x86-64 backend
- ARM64 backend
- WebAssembly backend

## 1.8 standard library
- filesystem and paths
- environment and processes
- time
- JSON
- HTTP and sockets
- crypto and compression
- databases

## 1.9 concurrency
- threads
- channels
- atomics
- async/await
- structured concurrency

## 2.x application stack
- UI and platform bindings
- 2D/3D graphics and canvas
- audio/video
- database clients
- mobile and desktop application targets

## 2.x acceleration
- SIMD
- GPU abstraction
- CUDA/Vulkan/Metal
- tensor IR
- kernel fusion
- mixed precision
- memory planning
- distributed execution
