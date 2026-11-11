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
2. *(Done: `struct`, `enum` with exhaustive `match`, `for … in` over arrays, `len` and `push`. Field and element assignment are done. `match` is an expression too, with nested, literal and binding patterns. Guards and top-level or-patterns are done. Integer range patterns (`a..=b`) are done. Struct patterns are done too. Still missing: nested or-patterns.)*
3. *(Done, in a different shape than first planned: bytecode → IR → optimiser → bytecode, with a differential test that compares optimised and unoptimised runs. Not done: lowering straight from the AST, SSA/3-address form, register allocation, inlining, optimising floats or string builders.)*
4. *(Done in a deliberately small form: `&T` / `&mut T` parameters with call-scoped borrow checking, so no lifetimes are needed. Still open: stored/returned references, borrowing fields and elements, real lifetime parameters.)*
5. *(Done: `IPv6`, `NetworkBuffer` with typed bounds-checked access, `Packet` as a container. `std.net` parses and builds IPv4/UDP/TCP headers. Ethernet II, IPv6 and UDP/TCP checksums are done. VLAN, ARP and ICMPv4 are done. Defensive tooling: hashing natives and `std.sec` are done. Still open: extension headers, ICMPv6, a PCAP reader, a detection-rule language, real sockets (`TCP`/`UDP` types).)*
6. *(Done: `.yomc` binary format v2, `build` and `exec`. Done: a verifier for jump targets, constants, instruction boundaries and operand-stack depth. Operand types are verified where statically known (constants, operator results, local slots). Still open: typing of call results, fields and elements, call-arity checks, format stability across versions, signing.)*
7. *(Done for built-in modules only: `import std.math`, `import std.arrays`, expanded from YOMNOVA source before lexing. Done in the CLI: `import "file.yom" as name` with `pub` visibility and namespaced functions/structs/enums (source renaming). Each library is also checked on its own. Still open: separate compilation (per-library bytecode, caching), glob imports, a separate `yomnova-stdlib` crate, generic array functions, text/IO modules.)*

## Research (no commitment)
Typestate for protocol state machines · effect system (effects are parsed in `![…]` but ignored) · tasks, channels
and a scheduler · distributed runtime · native code generation · embedded subset.
