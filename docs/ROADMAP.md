# Roadmap

Direction, not a schedule. Items move to **Implemented** only when covered by tests. Current state is tracked in
[PROGRESS.md](PROGRESS.md).

## Implemented
Lexer, parser, AST · scopes and type checking with call-argument checks · move-based ownership checking ·
bytecode compiler and stack VM with resource limits · pipelines and `flow` · `IPv4`/`Port`/`Endpoint` values ·
multiple errors per run · CLI (`run check build disasm tokens ast fmt`) · formatter (re-indent) · end-to-end regression suite · web
Playground backed by the real compiler.

## Next (planned, in rough order)
1. *(Done: several errors per run and two warnings — unused binding, unreachable code. Still open: more warnings, and lexical errors are reported one at a time.)*
2. *(Done: `struct`, `enum` with exhaustive `match`, `for … in` over arrays, `len` and `push`. Field and element assignment are done. Still missing: nested patterns, `match` as an expression.)*
3. *(Done, in a different shape than first planned: bytecode → IR → optimiser → bytecode, with a differential test that compares optimised and unoptimised runs. Not done: lowering straight from the AST, SSA/3-address form, register allocation, inlining, optimising floats or string builders.)*
4. Borrowing (shared and mutable) and lifetimes in the ownership checker.
5. *(Done: `IPv6`, `NetworkBuffer` with typed bounds-checked access, `Packet` as a container. Still open: protocol parsing, `TCP`/`UDP`.)*
6. *(Done: `.yomc` binary format v2, `build` and `exec`. Still open: format stability across versions, signing, operand-type and stack-depth verification. Done: a structural verifier for jump targets, constants and instruction boundaries.)*
7. Standard library (`yomnova-stdlib`) and modules/imports.

## Research (no commitment)
Typestate for protocol state machines · effect system (effects are parsed in `![…]` but ignored) · tasks, channels
and a scheduler · distributed runtime · native code generation · embedded subset.
