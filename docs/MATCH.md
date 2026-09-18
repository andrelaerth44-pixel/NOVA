# NOVA 1.5 core

NOVA now includes a compact pattern-selection construct:

```nova
match value {
    1 { print "one" }
    2 { print "two" }
    else { print "other" }
}
```

The implementation evaluates the selector once, compares arm values, executes the first matching arm, and supports an `else` arm.

This is part of the executable language core and is intentionally kept small so the future typed compiler can lower it into NOVA IR.
