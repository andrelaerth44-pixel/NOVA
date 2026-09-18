# NOVA compiler

The compiler is being built as independent layers:

1. source spans and diagnostics
2. lexer
3. parser / AST
4. semantic and type analysis
5. NOVA IR
6. optimization
7. target lowering
8. native/runtime integration

The initial IR is intentionally small. It will grow only when language constructs have a real lowering path.

No native backend is declared complete until it produces executable output and is validated by CI.
