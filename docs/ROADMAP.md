# Roadmap

Direction, not a schedule. Items move to **Implemented** only when covered by tests. Current state is tracked in
[PROGRESS.md](PROGRESS.md).

## Implemented
Lexer, parser, AST · scopes and type checking with call-argument checks · move-based ownership checking ·
bytecode compiler and stack VM with resource limits · pipelines and `flow` · `IPv4`/`Port`/`Endpoint` values ·
multiple errors per run · CLI (`run check build disasm tokens ast fmt`) · formatter (re-indent) · end-to-end regression suite · web
Playground backed by the real compiler.

## Next (planned, in rough order)
1. Warnings (e.g. unused variable, unreachable code). *(Multiple errors per run — parser, semantic and ownership recovery — is implemented; lexical errors are still reported one at a time.)*
2. `struct` and `enum` with `match`; `for … in` loops; array length/push.
3. Wire `yomnova-ir`: lower AST → IR → bytecode, then simple optimisations.
4. Borrowing (shared and mutable) and lifetimes in the ownership checker.
5. `IPv6`, `Packet` and a `NetworkBuffer` with typed, bounds-checked access.
6. A loadable bytecode file format (so `build` produces something `run` can execute).
7. Standard library (`yomnova-stdlib`) and modules/imports.

## Research (no commitment)
Typestate for protocol state machines · effect system (effects are parsed in `![…]` but ignored) · tasks, channels
and a scheduler · distributed runtime · native code generation · embedded subset.
