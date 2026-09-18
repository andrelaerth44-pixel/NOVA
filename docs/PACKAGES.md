# NOVA packages

NOVA uses a project manifest named `nova.toml`.

Basic project:

```toml
[package]
name = "meu_app"
version = "0.1.0"

[dependencies]
util = { path = "../util" }
```

The current resolver performs real local-path dependency resolution, validates that the dependency alias matches the package name, builds the dependency graph, detects cycles, and can generate a deterministic `nova.lock`.

Current CLI:

```text
nova package-check <manifest-or-dir>
nova package-lock <manifest-or-dir>
```

Registry version constraints and Git dependencies are parsed but deliberately rejected until their resolvers exist. This keeps the toolchain from pretending that a remote package registry is already implemented.

The intended future model is:

```text
nova.toml
  ↓
resolver
  ↓
nova.lock
  ↓
NOVA compiler
  ↓
native / Android / web / desktop backend
```

The dependency model is designed so external native libraries, Maven artifacts, AAR/JAR files, Git packages and registry packages can later plug into the same resolver without changing NOVA source syntax.
