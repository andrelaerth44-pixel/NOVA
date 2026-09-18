# NOVA 1.4 language core

NOVA now has the first real module mechanism.

## Import

```nova
import "lib/math.nova"
```

The imported file is parsed and executed once per runtime, with a module guard preventing repeated execution of the same path.

## Core surface

- variables and assignment
- numbers, strings, booleans, arrays
- arithmetic and comparisons
- boolean operators
- functions and return
- if / else
- while
- for / in
- range
- len
- str
- import

This is an executable implementation, not a simulated API.

## Direction

The compiler line is being expanded toward:
source -> parser -> semantic/type analysis -> NOVA IR -> optimization -> native backends.

The runtime keeps unsupported native targets explicit rather than pretending they already exist.
