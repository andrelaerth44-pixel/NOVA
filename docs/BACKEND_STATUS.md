# NOVA backend status

NOVA is being built as a real multi-backend language. Backend claims stay explicit so a target is not described as implemented before it can compile and execute programs.

## Current

- Front end: lexer, parser, interpreter, semantic checking and NOVA IR are present in the prototype.
- IR verification: jump-target validation is present.
- IR optimization: the repository now contains a constant-folding pass for supported arithmetic IR.
- Native strategy: a portable C backend exists as a bootstrap/host path; direct x86-64 and ARM64 emitters are being expanded independently so application source remains NOVA-native.

## Android target plan

The Android backend is a first-class compiler target, not a mock UI generator. NOVA application code will lower to a real Kotlin/Jetpack Compose Android project, while platform services are exposed through native target/runtime layers. The target will support real networking, persistence, authentication, file/media access and background work rather than embedding fake data.

The OBRA360 application is the first serious end-to-end conformance target for this stack: its authentication, PostgreSQL/RLS data access, Storage uploads, synchronization, Compose UI and Android platform integration must execute against real services before the target is marked supported.

## Not yet claimed

- Complete x86-64 machine-code generation
- Complete ARM64 code generation
- WASM code generation
- Full Android application runtime/codegen (native APK host is implemented; full UI/application lowering remains)
- GPU/CUDA/Vulkan/Metal backends
- Full standard library
- Package manager

These are implementation milestones. A backend becomes supported only after executable conformance tests are added.
