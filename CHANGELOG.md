# Changelog

## Unreleased — 2026-11-03

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
