# NOVA

Simple. Native. Fast. Universal. Beautiful.

NOVA is a general-purpose programming language designed to make applications, services, tools, games, graphics, automation, data systems, networking, native integrations and AI systems easier to build without removing access to the machine.

Current development line: **1.7.0**

The implementation is deliberately honest: a capability is documented as implemented only after executable code and validation exist.

## Current executable core

- variables and assignment
- numbers, strings, booleans and arrays
- arithmetic, comparisons and boolean operators
- functions and return
- if / else
- while
- for / in
- range
- len
- str
- imports and module graph validation
- structs and enums
- generic functions and type inference
- Option / Result and `?` propagation in the VM
- maps, sets and indexed collection access
- lexical closures
- first-class iterators (`iter`, `next`, `has_next`, `collect`)
- filesystem, environment, path, time and JSON runtime builtins
- CLI run/check/version

The current 1.x line is still being hardened. Static typing, diagnostics, IR, native code generation, standard-library breadth, concurrency, package management, UI, graphics, GPU and platform backends remain separate implementation stages.

NOVA does not claim to be universally native or faster than every other language until those backends are actually implemented and tested.

```nova
fn add(a, b) {
    return a + b
}

values = [1, 2, 3, 4]
total = 0

for x in values {
    total = total + x
}

print "sum=" + str(total)
print add(20, 22)

match total {
    10 {
        print "ten"
    }
    else {
        print "other"
    }
}
```
