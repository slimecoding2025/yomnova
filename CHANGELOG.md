# Changelog

## Unreleased — 2026-11-03

### `match` as an expression, nested and literal patterns
* **Added** `match` as an expression (`let a = match s { Shape.Circle(r) => r * 3, … }`, arms are expressions of one common type), patterns
  `_`, bindings, integer literals (`-1`), `true`/`false`, and enum patterns whose payload is any pattern (nested to any depth). Exhaustiveness
  covers nested cases (`some nested cases are not covered`), booleans need both values, integer literals need a `_`/binding arm, and
  unreachable arms are errors. Moves inside arms follow the existing rules; arm bindings do not leak. `examples/match_expr.yom`.
* **Changed:** matching a non-enum value with only `_`/literal arms is now valid (it was "expected an enum value"); an enum pattern against
  a non-enum value is a type error.
* **Not done:** guards, or-patterns, struct patterns, range patterns.

### `std.net`: Ethernet, IPv6, UDP/TCP checksums; `fail`
* **Added** to `std.net`: Ethernet II (`eth_dst eth_src eth_type eth_payload`, `make_ethernet` with `aa:bb:..` MAC text); IPv6 fixed header
  (`ipv6_version traffic_class flow_label payload_length next_header hop_limit src dst payload valid`, `make_ipv6` accepting `::` compression);
  UDP and TCP checksums over IPv4 and IPv6 with the RFC pseudo-headers (`ipv4_udp_checksum`, `ipv4_tcp_checksum`, `ipv6_udp_checksum`,
  `ipv6_tcp_checksum`, matching `*_ok`, and `fill_*_checksum` that return the packet with the field set). The checksums are tested against an
  independent Python implementation of RFC 1071 for IPv4/IPv6 x UDP/TCP and an odd-length payload; corrupted packets are detected.
  `examples/net_layers.yom`. 4 new tests (143 Rust tests total).
* **Added** the native `fail(message)`: stops the program with `Program failed: message` (used for malformed addresses; usable in any program).
* **Limits:** IPv6 extension headers are not walked (the next-header field is read as-is and the checksum assumes none); no VLAN tags, ARP, ICMP,
  fragmentation or options parsing; over IPv4 a stored UDP checksum of 0 means "not computed" and is accepted, over IPv6 it is rejected.

### Match guards
* **Added** `pattern if condition => …` in both `match` forms. The guard is type-checked as a Bool with the arm's bindings in scope; guarded arms
  do not count for exhaustiveness or make later arms unreachable; a guard that would move a value is rejected (`A match guard cannot move a value`);
  names used only in a guard count as read. `examples/match_guards.yom`, 2 tests (139 Rust tests total).
* **Internal:** the match-expression parsing and analysis moved into their own functions so the recursive-descent frames stay small (a debug build
  of the nesting-limit test overflowed its stack otherwise).
* **Not done:** or-patterns (`A | B`), struct patterns, range patterns.

### Network protocol library (`import std.net`)
* **Added** `std.net`, written in YOMNOVA on `NetworkBuffer` (`stdlib/net.yom`): IPv4 readers (`ipv4_version header_len total_length ttl protocol src dst`
  with `src`/`dst` as `IPv4` values), `ipv4_compute_checksum`, `ipv4_checksum_ok`, `ipv4_valid` (version, lengths, checksum), `ipv4_payload`; UDP
  readers (`udp_src_port dst_port length payload`); TCP readers (`tcp_src_port dst_port seq ack header_len syn ack_flag fin rst psh payload`);
  builders `make_udp` and `make_ipv4` (correct checksum); `from_bytes`/`to_bytes`. Tested against the Wikipedia IPv4 checksum vector (0xb861),
  a built UDP-in-IPv4 packet and a TCP SYN+ACK header; short buffers and bad addresses are errors. `examples/net.yom`.
  Library functions that call each other are pulled in together. 6 new tests (137 Rust tests total, with the match tests).
* **Limits:** no Ethernet, IPv6, options, fragmentation or reassembly; TCP/UDP checksums are not verified or computed; the readers only check
  bounds (use `ipv4_valid` on untrusted bytes); no sockets: this parses and builds bytes, it does not send them.

### Per-library checking
* **Added**: the CLI checks every imported library on its own (nested ones too, once each) before checking the combined program, so an error in
  `lib.yom` is reported as `lib.yom:6:18` with that file's source, not as a line past the end of the importing file. 1 new test (131 Rust tests total).
* **Not done:** this is separate *checking*, not separate *compilation*: there are still no per-library `.yomc` files, no caching and no
  signature files; the final program is compiled from the combined source.

### File access
* **Added** `file_read(path)`, `file_write(path, text)`, `file_append(path, text)`, `file_exists(path)`. **Off by default**: they fail with a clear
  error unless the command line passes `--allow-fs <DIR>` (`run` and `exec`); the playground on the website never does. Paths must be plain
  relative paths: absolute paths, `..`, empty paths and anything that resolves outside DIR through a symlink are refused (tested, including
  symlink read/write escapes). Files are UTF-8 text, at most 16 MiB. `examples/files.yom`, 1 test (130 Rust tests total).
* **Limits:** text only (no bytes/NetworkBuffer files, no directory listing, delete or rename, no streaming); the check is made when each call
  runs, so another process changing the directory concurrently is not defended against.

### Bytecode verifier: local variables
* **Added** flow-sensitive typing of local slots to the verifier (argument slots start unknown, other slots start as `Unit`, types are joined at
  merge points), so `let x = "a"` followed by `x - 1` in a hostile `.yomc` is rejected before it runs. `.yomc` decoding passes each function's
  arity (`verify_chunk_arity`). 1 new test (129 Rust tests total).
* **Still open:** call results, fields and array elements are unknown; no call-arity check against the callee.

### Text library
* **Added** 13 pure natives: `str_upper str_lower str_trim str_contains str_starts_with str_ends_with str_find str_slice str_repeat
  str_replace str_split str_from_int str_parse_int`, and `import std.text` (`text.upper(&s)` ...). Positions are in characters (like `len`),
  `str_find` returns -1 when absent, `str_slice` is bounds-checked, empty patterns/separators and invalid `str_parse_int` input are errors,
  results are bounded by the 16 MiB text limit and the array limit. `examples/text.yom`, 2 tests (128 Rust tests total).
* **Limits:** no regex, no format strings, no Unicode normalisation or locale rules (`upper`/`lower` are Rust's), no byte-level access
  (use `bytes_of`). In `std.text` the first argument must be a borrowed variable (`&s`), because only variables can be borrowed.

### Modules with visibility
* **Changed** file imports: in a library only `pub fn` / `pub struct` / `pub enum` are visible from outside; using a private or
  missing item is a compile error. Functions, structs and enums are all namespaced (`geo.area(...)`, `geo.Rect { ... }`), so the
  importer may declare the same names without collision. Imports are not re-exported. A library with no `pub` exposes nothing.
  1 new end-to-end test (126 Rust tests total).
* **Limits:** still source renaming (types print as `Geo_Rect`), no glob/`use` imports, no nested paths, no `pub` on fields,
  CLI only.

### Bytecode verifier: operand types (partial)
* **Added** to the stack verifier: it now tracks what is statically known about each stack slot (constants and operator results)
  and rejects operands that are certainly wrong: arithmetic on text/bool, comparing non-numbers or Int with Float, adding text to
  a number, a non-Int index, `Len` of a number, field access on a scalar. Types are joined at merge points. All compiled examples
  still pass. 1 new test (125 Rust tests total).
* **Limits:** locals, call results and fields are "unknown", so a hostile file can still fail with a type error at run time (the VM
  reports it safely); no call-arity or local-slot checks.

### File imports
* **Added** `import "file.yom" as name` (CLI only: `run`, `check`, `build`, `disasm`). The file is inlined, its functions are renamed
  `name_fn`, callers write `name.fn(...)`. Nested imports (paths relative to the importing file), cycle and missing-file errors,
  a library may not define `main`. `examples/modules/`, 2 tests (124 Rust tests total).
* **Limits:** it is renaming, not a real module system: only functions are namespaced (structs/enums stay global), no `pub`,
  no re-exports. The browser playground has no files, so it supports only `import std.<module>`.

### Bytecode verifier: operand-stack depth
* **Added** to `verify_chunk` (run when a `.yomc` file is decoded): abstract interpretation of the operand stack. It rejects
  stack underflow, an instruction reached with two different depths along different paths, and depth above 65 536. Every
  compiled example (optimised and not) passes it. 2 new tests (122 Rust tests total).
* **Not done:** operand *type* checking (the VM still checks types at run time), local-slot and call-arity checks against the callee.

### Standard library and imports
* **Added** `import std.math` (`abs sign min max clamp pow`) and `import std.arrays` (`sum contains index_of max_of min_of`, taking `&Array<Int>`),
  called as `math.abs(x)`. The library is written in YOMNOVA and expanded before lexing (unused functions are not included,
  diagnostic line numbers do not move). `examples/stdlib.yom`, 6 tests (120 Rust tests total).
* **Not done:** user modules / files, `import foo`, bare names, generic array functions (arrays are `Array<Int>` only).

### Borrowing
* **Added** reference parameters `&T` / `&mut T` with `&x` / `&mut x` at the call site, and a call-scoped borrow checker
  (conflicting borrows in one call, borrowing a moved value, moving out of a reference, writing through `&T`,
  `&mut` of a non-`mut` variable). References cannot be stored or returned, so no lifetime annotations are needed.
  `&mut` is implemented as copy-in/copy-out. 4 test groups (114 Rust tests total), `examples/borrowing.yom`.
* **Not done:** stored/returned references, borrowing fields or elements, lifetime parameters.

### IR optimiser
* **Added** `yomnova-ir` as a real stage: bytecode is lifted into a linear IR with symbolic jump targets, optimised and
  lowered back (constant folding that cannot change run-time errors, dead push/pop, constant-condition jumps, jump
  threading, unreachable code). On by default; `--no-opt` disables it. A differential test runs every example and
  extra programs (including ones that must fail) with and without it and requires identical results. 8 new tests
  (110 Rust tests total). The old, unused 3-address data types were removed.
* **Not done:** no lowering from the AST, no SSA, inlining or float folding; the speed-up was not benchmarked, only the
  size of the code (e.g. a constant-heavy example went from 24 to 9 listing lines).

### NetworkBuffer and Packet
* **Added** `NetworkBuffer` (bounds-checked big-endian 8/16/32-bit reads and writes, `buf_slice`, `buf_concat`, `bytes_of`,
  `len`; max 1 MiB) and `Packet(header, payload)`. Reads borrow, writes consume and return a new buffer. 4 new tests
  (102 Rust tests total), `examples/packet.yom`.
* **Not done:** no protocol parsing, checksums, endianness choice (big-endian only), signed reads, or `TCP`/`UDP`.

### Element and field assignment
* **Added** `xs[i] = v`, `p.f = v` and chains such as `b.vals[2] = v` / `grid[1][0] = v`. The root variable must be `mut`;
  types are checked; moved variables cannot be written to; a variable that is only written is still reported as unread.
  New opcodes `SetIndex` and `SetField` (40 opcodes); the `.yomc` format is now **version 2** (version 1 files are rejected
  cleanly). 7 new tests (98 Rust tests total), `examples/assignment.yom`.
* **Limits:** arrays and structs are values, so each write copies the container (O(n)); in a nested target an index may not
  contain a call; no compound assignment (`+=`).

### AI Assistant
* **Changed** the model is chosen by the server from three free OpenRouter models, with fallback to the next on failure.
  The picker is gone and the browser never sees a model id (it shows "YOMNOVA AI"). A model id sent by a client is ignored.
* **Changed** *Explain error* is now short (one sentence what, at most one why, one fix). Rule/source/caveat lists are
  removed, file names in model text are stripped, and the limits are enforced in code. Not exercised against live OpenRouter.

### Bytecode verifier
* **Added** `verify_chunk`, run on every chunk of a loaded `.yomc`: whole instructions only, known opcodes, constant
  operands in range (names must be text, struct field lists arrays of text), jump targets on instruction starts, and a
  final Halt/Return/Jump so code cannot run off the end. Corrupt files now fail at load with an offset. 2 new tests
  (91 Rust tests total).
* **Not done:** no stack-depth or type verification (the VM still checks those at run time), no local-slot bound check.

### IPv6
* **Added** `IPv6("…")`: validated by `std::net::Ipv6Addr`, displayed in canonical form, comparable with `==`. Bad input is a
  clean runtime error. 2 new tests (89 Rust tests total). `Packet`, `TCP`, `UDP` are still not implemented.

### Bytecode files
* **Added** a binary `.yomc` format (`yomnova-bytecode::module`) and the `exec` command; `build` now writes it (it used to write
  a text listing; `disasm` still prints the listing, for source files and for `.yomc` files). Running a saved program gives
  exactly the output of `run` (tested).
* **Hardened** the VM for untrusted bytecode: constant lookups no longer index out of range, and local-variable slots are
  budgeted (1,000,000 across frames). A corruption test mutates a valid file 4,000 times and requires that loading and running
  never panic. 9 new tests (87 Rust tests total).
* **Not done:** the loader checks structure only; there is no type/stack verifier, no signature, and the format is
  tied to the current opcode set (version 1).

### Warnings
* **Added** `yomnova-compiler::lint` and `Compiler::check_with_warnings`: *variable is never read* (for `let`, `mut`, `flow`,
  `for` and `match` bindings; `_name` and parameters exempt) and *unreachable code after `return`*. The CLI prints them to
  stderr before `run`/`check`/`build` without changing the exit code; the Playground status line counts them. 7 new
  regression tests (78 Rust tests total). None of the shipped examples produce warnings.
* **Not done yet:** other warnings (unused functions, shadowing, unused imports), lexical errors are still one at a time.

### Language
* **Added** `enum` (variants carry at most one value; built as `Shape.Circle(2)` / `Shape.Empty`) and the `match` statement with
  exhaustiveness checking, `_` and `Enum.Variant(x)` patterns, duplicate/unreachable-arm errors, and move rules
  (`match` borrows; a move in one arm makes the value maybe-moved afterwards). New opcodes `EnumNew`, `EnumIs`,
  `EnumPayload` (38 opcodes). 11 new regression tests (71 Rust tests total), `examples/enums.yom`.
  Not supported: nested patterns, guards, `match` as an expression.
* **Added** `struct`: declarations, literals (`P { x: 1, y: 2 }`), field reads, nesting, structs as parameters, return values
  and array elements; by-value equality and display; checked field names, types, duplicates and recursive structs;
  moved like other non-copy values (field reads do not move). 9 new tests (60 Rust tests total), `examples/structs.yom`.
  Not supported: field assignment, methods, generics.
* **Added** `for name in array { … }`, and the built-ins `len(Array|Text) -> Int` and `push(array, value) -> Array`
  (returns a new array; the old one is consumed). New opcode `OP_LEN` (33 opcodes). The VM caps arrays at 1,000,000
  elements. 11 new regression tests (51 Rust tests total) and `examples/arrays.yom`.
* **Not done yet:** `struct`/`enum`/`match`, element assignment (`xs[i] = v`), `pop`, ranges.

### Web (phones)
* **Fixed** code panels cut off on phones: grid children could not shrink and code never wrapped. Code now wraps and
  nothing exceeds the screen width.
* **Fixed** the scroll story on phones: Ownership, Protocol and Dataflow now pin their visual (not the whole chapter) and
  advance step by step with touch scrolling; Domains and Architecture light up the item crossing the middle of the screen.
  Hero and Vision are still plain scrolling sections on phones.
* **Added** `tests/e2e/mobile.e2e.mjs` (6 checks, iPhone 13 emulation in Chromium; not tested on a real device or on iOS Safari).

### Compiler
* **Added** multiple diagnostics per run (roadmap item 1, first part): parser error recovery at statement/declaration
  boundaries, per-statement/per-function recovery in semantic analysis and the ownership check, a cap of 20 errors per
  stage, and suppression of "undefined" cascades from a failed declaration. 8 new regression tests (40 Rust tests total).
* **Not done yet:** warnings; lexical errors are still reported one at a time; a later stage runs only after the
  earlier one is clean.

## Unreleased — 2026-11-02

### Deployment
* **Added** `Dockerfile`, `.dockerignore`, `render.yaml` and `docs/DEPLOY.md` (Render free plan). The container layout was
  verified locally; the image build and the Render deployment have not been run.
* **Removed** the "Run the real compiler." heading from the Playground chapter.

### AI Assistant (optional, suggest-only)
* **Added** a native Playground panel: *Explain with YOMNOVA AI* on compiler errors, *Explain selection*, syntax tutor,
  debug mode, and fix suggestions with copy / diff / *Validate with compiler* (runs `yomnova check` on a copy; the
  editor is never modified). Models are selected from the live OpenRouter model list.
* **Added** `server/ai/` (OpenRouter client, diagnostic adapter, documentation retrieval, versioned prompts, response
  validation, rate limiting) and `/api/ai/*` routes on the existing `node:http` server. No new runtime dependencies.
* **Added** `docs/AI_ASSISTANT.md`, AI settings in `.env.example`, `npm run test:ai` (46 tests) and a Playwright
  end-to-end script. Disabled unless `OPENROUTER_API_KEY` is set. Tested only against a mock provider.

## Unreleased — 2026-11-01

### Compiler / VM fixes
* **Fixed** stack leak: `JumpIfFalse` now pops its condition (long loops no longer grow the stack).
* **Fixed** integer overflow panics (exit 101): all `Int` arithmetic is checked and reports `IntegerOverflow`.
* **Fixed** unbounded parser recursion (abort on deeply nested input): nesting is capped at 150.
* **Fixed** pipelines/`flow`: stages were silently discarded; they are now compiled, validated and type-checked.
* **Fixed** block scoping (inner `let` leaked and shadowing miscompiled) and `const` (global and local).
* **Fixed** `&&`/`||` evaluated both operands; they now short-circuit.
* **Fixed** `Endpoint(ip, 70000)` truncating the port; negative array indices no longer wrap.
* **Fixed** silent miscompilation: unsupported expressions are now compile errors instead of `Nil`.
* **Added** static checks: call-argument types, missing return, `main` signature, indexing non-arrays,
  `Float % Float`, unsupported `struct`/`enum`/method/field/lambda syntax.
* **Added** ownership: `emit` borrows; `let b = a` moves; copy-ness inferred from expression types; branch-aware
  moves; move-in-loop detection.
* **Added** resource limits: call depth 10,000, text 16 MiB; compile errors instead of silent 16-bit truncation.
* **Removed** both `unsafe` transmutes (safe `OpCode::from_u8`).
* **Changed** `build` no longer executes the program; it writes a textual bytecode listing.

### CLI
* Rewritten without `clap`/`colored` (zero third-party dependencies in the CLI). New: `disasm`, `tokens`, `ast`.
* `version` reports the real package version and platform (it used to print a hard-coded compiler version) and
  no longer prints a personal e-mail address.
* `fmt` now ignores braces in strings/comments and never edits string or block-comment contents.

### Web
* New original brand: logo set, favicon, social image (`public/brand/`).
* New ten-chapter, GSAP/ScrollTrigger scroll story with `prefers-reduced-motion` and mobile fallbacks.
* Playground rebuilt on the real compiler: Output, Diagnostics, Tokens, AST, Bytecode, Format, Check, examples.
* Backend rewritten on `node:http` (removed express, dotenv, `@google/genai`, tailwind, lucide, motion).
* Removed legacy scaffolding leftovers and the fabricated `/api/test-suite` endpoint.

### Repository
* Build output (`yomnova/target`, 3,025 files) removed from version control; `.gitignore` rewritten.
* Docs moved to `docs/` and rewritten to match the implementation; new README, CONTRIBUTING, audits.
* New examples: `functions.yom`, `pipeline.yom`, `type_error.yom`.

## 0.1.0 — initial import
Lexer, parser, type checker, move checker, bytecode VM and web workbench as found at the start of this work;
see `docs/INITIAL_AUDIT.md`.
