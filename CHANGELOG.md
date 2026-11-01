# Changelog

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
