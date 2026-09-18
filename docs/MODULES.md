# NOVA modules

NOVA modules are source files loaded by the compiler pipeline. The language keeps the import syntax deliberately small:

```nova
import "math.nova"
```

## Resolution

- Relative imports resolve from the directory containing the importing module.
- Absolute filesystem paths are accepted.
- Each resolved module is canonicalized before it is cached.
- A module is expanded once per compilation/load graph.
- The loader preserves source order: imported declarations/statements are inserted at the import site.
- Missing files, lexer errors and parser errors identify the module path.

## Cycles

Circular imports are diagnosed before execution instead of recursing forever:

```text
module import cycle: /path/a.nova -> /path/b.nova -> /path/a.nova
```

The active import stack is tracked independently from the cache, so a legal shared dependency can be imported by multiple modules without being expanded repeatedly.

## Typed compilation

For `nova check`, `nova ir`, and native compilation, the same expanded module graph is passed through semantic analysis and the compiler pipeline. This means declarations from imported NOVA modules participate in the same type checking and IR generation as local declarations.

## Runtime

The VM receives the same expanded program for the entry module. Module loading is therefore deterministic and does not depend on a second, runtime-only parser path.

## Direction

The next layer is a real package system built on top of this module graph:

```text
nova.toml
  -> dependency resolver
  -> package cache
  -> module graph
  -> semantic analysis
  -> NOVA IR
  -> backend
```

That package system will provide version constraints, lockfiles, local path dependencies and eventually external native/JAR/AAR/Maven bindings without changing NOVA source syntax.
