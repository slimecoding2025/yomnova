# YOMNOVA — Initial Technical Audit

| | |
|---|---|
| **Audited commit** | `254f12f` ("Delete README.md") on the default branch of `slimecoding2025/yomnova` |
| **Audit date** | 2026-10-31 → 2026-11-01 |
| **Scope** | Rust workspace (14 crates), React/Vite frontend, Express backend, documentation, legacy scaffolding artifacts |
| **Rule followed** | Nothing was modified before this report was written. Every finding below was observed by running something or reading the code; each carries its evidence. |

Status legend: **PASS** verified working · **FAIL** verified broken · **PARTIAL** works in part · **NOT TESTED** not exercised · **BLOCKED** could not be run, exact reason given.

---

## 1. Executive summary

YOMNOVA is a **real, working, small language implementation**, not a mock-up. A lexer, a precedence-climbing parser, a semantic/type checker, a move-tracking ownership checker, a bytecode generator and a stack VM exist, are wired together, and execute real programs. The six shipped examples all behave as described, including a use-after-move being rejected with a precise, source-annotated diagnostic.

It is also **much less complete than its documentation and UI claim**, and it contains several correctness bugs that break ordinary programs:

* **Recursion is broken** (`fib(15)` fails) because every `if`/`while` leaks one VM stack slot (**C1**).
* **The `|>` pipeline operator and `flow` stages are silently discarded** (`5 |> inc |> dbl` prints `5`) (**C2**). "Dataflow" is not implemented.
* **Integer overflow crashes the Rust process** (exit 101), and deeply nested input or memory-hungry programs **abort the process** (exit 134) (**C3, C4, M-mem**).
* Ownership checking is **move tracking only**: no borrowing, lifetimes or typestate exist, and there are holes (`let b = a` does not move; moves inside loops are not caught).
* Three of the 14 crates (`ir`, `network-core`, `stdlib`) are **not used by the compiler pipeline**.
* The README and UI describe crates that do not exist (`yomnova-effects`, `yomnova-optimizer`) and features that do not exist (lambdas, `TCP.connect`, backpressure, borrow checking).
* The website is a single workbench page, with no landing experience, no Monaco/highlighting/tokens/AST views, and it is unusable on phones/tablets (examples are hidden below the `lg` breakpoint).
* **3,025 build-output files (`yomnova/target/`) are committed to git**, including an 18.9 MB debug binary that the backend serves by default. That binary was built at `/app/applet/…` (a path from the original build environment) and from a slightly older revision than the current source.
* A 29 KB scaffolding prompt file sits in a legacy folder under `public/assets/` and is **publicly downloadable** from any deployment.

**Verification constraints (important).** In this audit environment both package registries are policy-blocked (`index.crates.io:443` and `registry.npmjs.org:443` return 403 at the proxy), so `cargo check/test/clippy` and `npm install/build/lint` **could not be run against the real manifests** (BLOCKED, §9). To still get genuine evidence I (a) ran the **committed binary** against 100+ programs and (b) compiled the **unmodified Rust sources** in a scratch copy using a tiny local stand-in for `thiserror` (the only non-optional external crate the library crates use), and confirmed the source-built driver behaves **identically on all 100 cases**. Details and limits are in §9–10.

**Honest scoreboard of the initial state**

| Area | Verdict |
|---|---|
| Lexer / parser / diagnostics | **PASS** for the supported subset; clear errors with line/column |
| Type checker | **PASS** for 11/11 basic type-error cases; inference limited |
| Ownership checker | **PARTIAL** — move-after-call works; aliasing/loop holes; no borrows |
| Bytecode + VM | **PARTIAL** — straight-line code, functions, simple loops OK; stack-leak, overflow panics, no memory cap |
| Network types (`IPv4`, `Port`, `Endpoint`) | **PARTIAL** — constructible & validated (one bypass); `IPv6`, `TCP`, `UDP`, `Packet` **not usable** |
| Dataflow (`flow`, `|>`) | **FAIL** — parsed, then ignored |
| Concurrency / async / channels | **NOT IMPLEMENTED** |
| CLI | **PARTIAL** — `run`, `check` work; `build` runs the program and writes text; `fmt` is naive; `version` hard-codes a fake rustc version |
| Frontend / Playground | **PARTIAL** — real backend calls, no fake output; thin, not responsive |
| Backend | **PARTIAL** — works by design, several security/robustness concerns |
| Documentation | **FAIL** — substantially inaccurate (§7) |

---

## 2. Repository structure (as found)

```
.                         Web app root
├── index.html            generic metadata, no favicon / social image
├── package.json          name "react-example", unused deps, no lockfile (bun.lock is 0 bytes)
├── server.ts             Express API: spawns the yomnova binary
├── vite.config.ts        contains platform-specific comments / DISABLE_HMR logic
├── metadata.json         legacy platform applet manifest
├── .env.example          legacy platform environment instructions
├── src/{main.tsx,App.tsx,index.css}   696-line single component, no routing
├── public/assets/<legacy dir>/…   original scaffolding prompt
└── yomnova/              Rust workspace (14 crates)
    ├── Cargo.toml / Cargo.lock / LICENSE-MIT / LICENSE-APACHE / README.md
    ├── crates/{ast,bytecode,cli,compiler,diagnostics,ir,lexer,network-core,ownership,parser,sema,stdlib,types,vm}
    ├── docs/{GRAMMAR,LANGUAGE_SPECIFICATION,PROGRESS}.md
    ├── examples/{hello,variables,dataflow,ownership,ownership_fail,tcp-client}.yom, hello.yomc
    └── target/           3,025 committed build files (incl. 18.9 MB debug binary)
```

`.gitignore` does not list `target/`. The root README was deleted by the last commit; the only README is `yomnova/README.md`.

Source size: ~5,800 lines total — lexer 629, parser 929, sema 606, ownership 279, compiler 447, vm 481, bytecode 238, ast 237, network-core 243, ir 111, types 113, stdlib 37, diagnostics 111, cli 200.

---

## 3. Rust compiler findings

### 3.1 Pipeline reality check — **PARTIAL**

The compiler driver (`yomnova-compiler`) runs: **Lexer → Parser → Sema/Type-check → Ownership check → Bytecode gen → VM**.

| Component | Exists | Used in pipeline | Evidence |
|---|---|---|---|
| Lexer (`yomnova-lexer`) | yes | yes | 4 unit tests pass |
| Parser (`yomnova-parser`) | yes (recursive descent + precedence climbing) | yes | 2 unit tests pass |
| AST | yes | yes | |
| Semantic analysis / type check | yes | yes | §3.4 |
| Ownership checker | yes (**moves only**) | yes | §3.5 |
| **IR (`yomnova-ir`)** | data types only (6 types, **no functions**) | **no** | `grep yomnova_ir` outside its crate → 0 hits |
| Bytecode + VM | yes | yes | §3.6 |
| **`yomnova-network-core`** | yes (3 tests pass) | **no** — the VM re-implements `IPv4`/`Port`/`Endpoint` inline | 0 imports |
| **`yomnova-stdlib`** | 37 lines, 1 test | **no** | 0 imports |
| `yomnova-effects`, `yomnova-optimizer` | **do not exist** | — | `ls crates` |

### 3.2 Language features actually observed (100 programs, §10)

| Feature | Status | Evidence |
|---|---|---|
| `module`, `fn`, `return`, `let`, `mut`, comments, `emit` | **PASS** | examples + `f_*` cases |
| `Int`, `Float`, `Bool`, `Text`, `Unit`; arithmetic, comparison, `&& \|\| !`, `-x` | **PASS** | `f_arith`, `f_float`, `f_bool_not` |
| `if / else`, `while` (small counts) | **PARTIAL** | work for simple code; leak stack (C1) |
| Functions with params and return | **PARTIAL** | `add(2,40)` OK; **recursion fails** (C1) |
| Arrays + indexing (`xs[1]`), `Array<T>` | **PARTIAL** | indexing works; **no assignment** (`xs[0]=9` parse error), **no `.len`** (compiles to `()` silently) |
| Text concatenation `+` | **PASS** | `f_text_concat` |
| Unicode / escapes / CRLF | **PASS** | `f_unicode`, `f_escape`, `s_crlf` |
| `IPv4(..)`, `Port(..)`, `Endpoint(..)` | **PARTIAL** | construct + validate; `Endpoint(ip, 70000)` bypasses range check (H5) |
| `IPv6`, `TCP`, `UDP`, `Packet`, `NetworkBuffer` | **FAIL / absent** | `IPv6` → runtime "Undefined global"; `TCP(..)` → "Undefined identifier" |
| `flow` + `\|>` | **FAIL** | stages discarded (C2) |
| `const` | **FAIL** | top-level: passes sema, fails at runtime; inside fn: parse error (H2) |
| `struct`, `enum` | **PARTIAL (parse only)** | parsed into AST, then ignored; accepted silently |
| `for`, `match`, `import` | **FAIL** | lexed as keywords, no parse rule: "expected expression" |
| tuples `(Int, Int)` | **FAIL** | "expected type annotation" |
| Effect annotations `![io]` | **PARTIAL (parse only)** | accepted and ignored |
| Lambdas (`x => x*2`), methods (`socket.send()`), `filter`/`map` | **FAIL / absent** | no parse support; used in the old README |
| Async, channels, structured concurrency, cancellation, backpressure, distributed execution | **NOT IMPLEMENTED** | no code |

### 3.3 Lexer & parser diagnostics — **PASS**

Missing brace, unterminated string, illegal character, `for`/`match`/`import` misuse, tuple types: all rejected with `file:line:col: error: …`, a source excerpt, and a caret. Only the **first** error is reported per run (single-diagnostic design, M7).
**Defect C4:** parser recursion is unbounded. `emit(((…(1)…)))` with 20,000 parens and 5,000 nested `if` blocks both **abort the process** (`has overflowed its stack`, exit 134).

### 3.4 Semantic analysis / type checking — **PASS (basic)**

Rejected correctly (all exit 1, with positions): `let x: Int = "hello"`, undefined variable, undefined function, wrong arity, wrong argument type, wrong return type, assignment to immutable, non-Bool condition, `Int + Text`, duplicate binding, duplicate function, mixed `==`, mixed array element, `&&` on Int, unannotated-int vs Text mismatch, oversized integer literal.
**Gaps:** (H7) non-`Unit` function with no `return` is accepted and yields `()`; `fn main(x: Int)` accepted then fails at runtime; missing `main` and empty files are accepted silently; `5 |> nothere` (undefined stage) is accepted (C2).

### 3.5 Ownership checker — **PARTIAL**

Works: moving a `Text`/`Array`/`Endpoint` into a function then reusing it is rejected (`ownership_fail.yom`, `o_move_into_fn`, `o_move_array`, `o_move_net`, `o_move_in_branch`); re-assigning a moved `mut` variable re-initialises it (`o_reassign_after_move` passes).

| Hole / defect | Evidence |
|---|---|
| **H4a** `let b: Text = a` does **not** move `a`; `emit(a)` afterwards is accepted | `o_move_alias` exit 0 |
| **H4b** A move inside a `while` body is not flagged on the second iteration | `o_move_in_loop` exit 0 (prints `x` twice) |
| **H4c** Copy-ness is read from the **type annotation only**; `let k = 9; show(k); show(k)` is wrongly rejected | `i_unannotated_int_reuse` exit 1 |
| **H3** `emit(x)` is an ordinary call and therefore **moves** its argument: `emit(a); emit(a)` is rejected; `emit(ip); Endpoint(ip,p)` is rejected | `o_move_emit`, `f_net_types` exit 1 |
| **Not implemented:** immutable/mutable borrowing, lifetimes, typestate, resource-close tracking. `CannotMoveOut` error variant is declared but never constructed. | source review |

### 3.6 Bytecode & VM — **PARTIAL**

Works: constants, locals, globals, arithmetic, comparison, jumps, calls/returns, arrays, indexing, native `IPv4/Port/Endpoint`; `1,000,000`-step limit; `65,536`-slot stack limit; division/modulo by zero and out-of-bounds index are **safe runtime errors**, not crashes.

| ID | Sev. | Defect | Evidence |
|---|---|---|---|
| **C1** | Critical | `JumpIfFalse` only peeks the condition and codegen emits no `Pop` on either path → **every `if`/`while` iteration leaks one stack slot**. Locals share one stack, so a callee's leak corrupts the caller. | `fib(15)` → `Type error: Cannot add Bool(true) and Int(0)`; loop of 200,000 iterations → `Stack overflow` at i≈65,534; `while true {}` reports *Stack overflow* instead of the step limit |
| **C2** | Critical | `ExprKind::Pipe { stages: _ }` compiles only the source; `Stmt::Flow { stages: _ }` likewise. **No pipeline stage ever runs.** | `5 \|> inc \|> dbl` prints `5`; `flow xs = [1,2,3] \|> dbl` prints `[1, 2, 3]` |
| **C3** | Critical | Integer `+ - * unary-` and `i64::MIN / -1` use unchecked Rust ops → **panic** (exit 101) in debug, silent wrap in release | `r_int_overflow`, `r_mul_overflow`, `r_min_div`, `s_negate_min` all exit 101 |
| **C4** | Critical | Unbounded parser recursion → process abort (exit 134) | §3.3 |
| **M-mem** | High | No memory limit: `s = s + s` loop → `memory allocation of 536870912 bytes failed`, abort (exit 134, under a 2 GB cap) | `probe/mem.yom` |
| **H1** | High | Block scoping miscompiled: an inner `let a` overwrites the outer `a` (prints `2, 2`, expected `2, 1`) | `s_nested_shadow` |
| **H2** | High | `const`: top-level is accepted by sema, then `Undefined global variable` at runtime; inside a function it is a parse error | `f_const`, `c_global_use_in_fn`, `c_const_in_main` |
| **H5** | High | `Endpoint(ip, 70000)` truncates with `n as u16` → `1.2.3.4:4464`; `Port(70000)` itself is correctly rejected | `n_endpoint_big_port` |
| **H6** | Medium | `&&` / `\|\|` are not short-circuit (`false && boom()` evaluates `boom()`) | `s_shortcircuit` |
| **M1** | Medium | Catch-all `_ => Nil` in codegen: any unsupported expression (e.g. `xs.len`, struct literals) silently becomes `()` | `u_array_len` prints `()` |
| **M4** | Low | Negative indices wrap (`xs[-1]` returns the last element) — undocumented behaviour | `r_index_neg` exit 0 |
| **M6** | Low | No call-depth limit (the step limit eventually fires: `r_deep_recursion`) | |
| **M10** | Low | Two `unsafe { transmute(u8→OpCode) }` (bytecode disassembler, VM). Both are range-guarded and `Halt` is the last variant, so they are sound **today** but fragile; contradicts the UI's "safe Rust" wording | `vm/lib.rs`, `bytecode/lib.rs` |

### 3.7 CLI — **PARTIAL**

| Command | Status | Evidence |
|---|---|---|
| `run <file> [--disassemble]` | **PASS** | examples; with `-d` the program output and disassembly share stdout |
| `check <file>` | **PASS** | 0 on success, 1 + diagnostic on failure |
| `build <file> [-o]` | **PARTIAL / misleading** | calls `compile_and_run` → **executes the program**; the ".yomc artifact" is a text disassembly, not loadable bytecode (**M2**) |
| `fmt <file> [--check]` | **FAIL (naive)** | line-based re-indenter: a `{` inside a string or comment shifts indentation of the rest of the file; rewrites in place with no backup (**M8**) |
| `version` | **PARTIAL** | prints hard-coded `rustc 1.99.1, linux-x86_64`, not the real build info (**M3**) |

### 3.8 Tests — **FAIL (insufficient)**

10 unit tests exist in the whole workspace (lexer 4, network-core 3, parser 2, stdlib 1). **None** for sema, ownership, bytecode, vm, compiler, CLI, or end-to-end programs. All 10 pass (§10).

---

## 4. Frontend findings (static review; build BLOCKED — §9)

| ID | Finding | Status |
|---|---|---|
| F1 | One 696-line `App.tsx`, no routing, **no landing page / narrative** — the site is only the workbench | FAIL vs brief |
| F2 | Stack: React 19, Vite, Tailwind 4, lucide-react. `motion` is declared but **never imported**; `@google/genai`, `dotenv`, `autoprefixer`, `esbuild` are **unused** | PARTIAL |
| F3 | `package.json`: name `react-example`, version `0.0.0`; `bun.lock` is a **0-byte file**; no other lockfile → non-reproducible installs; `lint` = `tsc --noEmit` only; **no tests** | FAIL |
| F4 | Layout is `h-screen w-screen overflow-hidden`; the examples sidebar is `hidden lg:flex` → **below 1024 px the examples are unreachable**; the output panel is a fixed `w-1/2`, squeezing the editor on phones | FAIL (static) |
| F5 | Editor is a plain `<textarea>` with a separate, non-scroll-synchronised line-number gutter (misaligns once the code scrolls); **no syntax highlighting** | FAIL |
| F6 | Missing from the brief: syntax highlighting, **tokens**, **AST**, structured **diagnostics** panel, example selector on small screens | FAIL |
| F7 | Accessibility: no landmark roles (`main`, `nav`), tabs without `role=tab/tablist`, textarea without a label, `text-[10px]` labels, status text `VM Active` is a static string | FAIL (static) |
| F8 | "Ownership", "Protocol" and "Spec & EBNF" tabs are **hard-coded prose**. They overstate: list `NetworkBuffer::with_capacity(1500)` as a language feature (it is a Rust API in an unused crate), say "Bounds-checked MTU payload", "implemented in safe Rust" (two `unsafe` blocks) | FAIL (accuracy) |
| F9 | Examples are duplicated in three places (`examples/*.yom`, `App.tsx`, README). `/api/examples` exists but the UI never calls it → drift risk | PARTIAL |
| F10 | `index.html`: generic title/description, **no favicon, no OG image** despite `twitter:card=summary_large_image` | FAIL |
| F11 | Playground output is **genuine**: every button calls the backend, no browser-side interpreter, no fabricated output. The "Bytecode" tab calls `/api/disassemble` which **runs the program a second time** (via `build`). | PASS (honest) / PARTIAL (double execution) |

Examples requested in the brief but **not present in the repo**: *Functions & Control Flow*, *Arrays*, *Type error demo*. Only six examples exist (§5).

---

## 5. Playground / example results (committed binary, real execution)

| # | Example | Compiles | Runs | Actual output | Notes |
|---|---|---|---|---|---|
| 1 | `hello.yom` | PASS | PASS | `Hello from YOMNOVA — One Language. Every System.` / `=> 0` | |
| 2 | `variables.yom` | PASS | PASS | `YOMNOVA`, `0.1.0`, `15` | |
| 3 | `dataflow.yom` | PASS | PASS (**misleading**) | `Streaming network packet buffer sizes:` / `[64, 128, 256, 512, 1024, 1500]` | `flow` is just a binding; **no pipeline occurs** (C2) |
| 4 | `ownership.yom` | PASS | PASS | `Preparing…` / `Consumed payload successfully:` / `SYN_PACKET_HEADER_DATA` | |
| 5 | `ownership_fail.yom` | **correctly rejected** | — | `10:10: error: Use of moved value 'msg'` + hint `moved at line 9` | exit 1 — **PASS** (negative test) |
| 6 | `tcp-client.yom` | PASS | PASS | `Configured YOMNOVA protocol endpoint:` / `127.0.0.1:8080` | no sockets are opened; only a value is built |
| — | Functions & control flow / Arrays / Type error | **not in repo** | | | covered by my battery instead (§10) |

---

## 6. Backend findings (`server.ts`; **BLOCKED** from running — `express` not installable)

Endpoints: `POST /api/run`, `/api/check`, `/api/disassemble`, `/api/fmt`, `/api/test-suite`; `GET /api/version`, `/api/examples`. Static review only:

| ID | Concern | Severity |
|---|---|---|
| B1 | `/api/test-suite` is **unauthenticated** and runs `cargo test` on the server (30 s) — resource abuse; also requires a Rust toolchain in production | High |
| B2 | Arbitrary user programs run with the server's privileges: no rlimits, no memory cap, no container; combined with **M-mem / C4** a single request can abort a worker process | High |
| B3 | No rate limiting, no concurrency cap; each request spawns a process | Medium |
| B4 | `exec(\`${BIN} run "${tmp}"\`)` string interpolation. Safe **today** (path is server-generated) but fragile; `execFile` is the correct pattern | Low |
| B5 | Temp files in shared `/tmp` with `Date.now()+Math.random()` names instead of `mkdtemp` | Low |
| B6 | Binary path is hard-coded to the **debug** build, which in a fresh clone is the **stale committed binary** (built at `/app/applet/…`) — fixes in source are not reflected until someone rebuilds | Medium |
| B7 | `/api/disassemble` re-executes the program (`build`) — duplicate side effects, double latency | Medium |
| B8 | `/api/version` returns a hard-coded `"0.1.0-dev"` and swallows binary errors | Low |
| B9 | Binds `0.0.0.0`; no security headers; `express.json({limit:'5mb'})`; `dotenv` loaded nowhere | Low |
| B10 | CORS: none configured → browsers block cross-origin reads; same-origin deployment works | PASS |
| B11 | Input validation: only `typeof code === 'string'` (empty string accepted) | PARTIAL |

---

## 7. Documentation findings — **FAIL**

`yomnova/README.md` (164 lines) and the three docs were compared with the code:

| Claim in docs | Reality |
|---|---|
| Pipeline includes `yomnova-effects` and `yomnova-optimizer` | **Crates do not exist** |
| "Borrow checking", "typestate tracking", "Connections cannot be used after close, buffer aliasing is statically prevented" | **Not implemented** — move tracking only |
| `yomnova-ir` "Control Flow Graph, Basic Blocks, SSA-like TAC" in the pipeline | Data types only; **not used** |
| "rejecting malformed protocol operations **at compile time**" | Port/IPv4 validation happens at **runtime** in the VM |
| `flow filtered = [..] \|> filter(x => x >= 20) \|> map(x => x * 2)` | **Does not parse** (no lambdas); even simple `\|>` does nothing |
| `TCP.connect(target)`, `socket.send()`, `socket.close()` | **Not implemented** (no method calls, no TCP) |
| `const PORT_DEFAULT: Int = 8080` shown as working | Fails at runtime (H2) |
| "guaranteed backpressure semantics", "zero-copy buffer views", "native concurrency" | **Not implemented** |
| `EthernetFrame` type | Does not exist |
| `docs/PROGRESS.md`: parser "Next", sema/ownership/VM/CLI "Planned", crate `yomnova-effects` | **Badly outdated** — these exist and run |
| `LANGUAGE_SPECIFICATION.md`: `for`, `match`, `import`, tuples, `Struct`/`Enum` types | Keywords exist; **not implemented** (§3.2); no implemented/planned distinction |
| `GRAMMAR.md`: `Assignment ::= … "\|>" PipeStage`, `Postfix … "." Identifier`, `"()"` | Partially inaccurate; no statement of what is supported |
| Commands `cargo build --release`, `run`, `check`, `fmt`, `cargo test --workspace` | Correct as commands (`yomnova` binary name verified from the committed build); **not re-verified here** because crates.io is blocked |
| Wrong repository URL in `Cargo.toml` (`github.com/yomnova/yomnova`) | Actual repo is `slimecoding2025/yomnova` |

Also: examples duplicated (§F9); no CHANGELOG, CONTRIBUTING, ARCHITECTURE, ROADMAP.

---

## 8. Legacy scaffolding artifact report

Searched all tracked files outside `yomnova/target/` (content and paths), plus binary strings.

| # | Location | Trace | Action planned |
|---|---|---|---|
| 1 | `.env.example` lines 1–7 | an unused API-key variable, platform-specific injection/secrets instructions, `APP_URL … Cloud Run` | Replace with accurate `PORT`/`YOMNOVA_BIN` documentation |
| 2 | `metadata.json` | legacy platform applet manifest declaring a hosted-API capability and frame permissions | Delete (no consumer in this repo) |
| 3 | `package.json` | dependency `@google/genai` (**never imported**); name `react-example` | Remove dependency; rename package |
| 4 | `vite.config.ts` lines 15–19 | platform comments about disabling HMR / file watching via `DISABLE_HMR` + the `DISABLE_HMR` logic | Remove comments and logic |
| 5 | `public/assets/<legacy dir>/…` | A 29 KB scaffolding prompt file, **served publicly** | Remove from the served tree and from git history |
| 6 | `yomnova/target/debug/yomnova` (committed binary) | Debug info path `/app/applet/yomnova/crates/…` (observed in panic backtrace) | Remove committed build output; add `target/` to `.gitignore` |
| 7 | `yomnova/target/**` | 3,025 committed files from the original build environment | Untrack |
| 8 | `Cargo.toml` `[workspace.package] repository` | Wrong URL (not a platform trace, but a provenance error) | Fix |
| 9 | `yomnova/Cargo.lock` line 1 | "automatically @generated by Cargo" | **Keep** (standard Cargo header, not a trace) |

Not removed, by design: `LICENSE-MIT`, `LICENSE-APACHE`, the SPDX/copyright header in `App.tsx` (Mouhamed Salim Bousmina), and the `authors` field. Note for the owner: the author e-mail `[removed]` is embedded in `Cargo.toml`, the CLI `--help`/`version` output and the committed binary; remove it if you do not want it public.

---

## 9. Test commands executed and results

| # | Command | Result | Status |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` (real repo) | exit 1; **118 diff hunks in 15 files** (no file is rustfmt-clean) | **FAIL** |
| 2 | `cargo check --workspace` (real repo) | exit 101 — `failed to get serde … download of config.json failed … Could not resolve host: index.crates.io` | **BLOCKED** |
| 3 | `cargo test --workspace` (real repo) | same | **BLOCKED** |
| 4 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` (real repo) | same | **BLOCKED** |
| 5 | Retry of #2 with `NO_PROXY` cleared for the command only | `CONNECT tunnel failed, response 403` — proxy status: `connect_rejected … index.crates.io:443` (**policy denial**; per proxy guidance not retried or bypassed) | **BLOCKED** |
| 6 | `cargo test --offline -p yomnova-diagnostics …` | `no matching package named serde` (even optional deps must resolve) | **BLOCKED** |
| 7 | `npm install` | `ENOTFOUND registry.npmjs.org`; with `NO_PROXY` cleared: `E403` policy denial (`registry.npmjs.org:443`) | **BLOCKED** |
| 8 | `npm run build`, `npm run lint` (`vite build`, `tsc --noEmit`) | cannot run: no `vite`, `@types/react`, `lucide-react`, `express`, `gsap` installable | **BLOCKED** |
| 9 | Backend start / API calls | `express` not installable | **BLOCKED** |
| 10 | Scratch copy: unmodified crates + stand-in `thiserror` + driver (CLI crate excluded) → `cargo build --offline` | success, 14 crates compiled | **PASS** |
| 11 | Same → `cargo test --workspace` | **10 passed, 0 failed** (lexer 4, network-core 3, parser 2, stdlib 1) | **PASS** |
| 12 | Same → `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 — clean on the 13 library crates (the CLI crate is **not covered**) | **PASS (partial scope)** |
| 13 | Committed binary: 6 examples × (`check`,`run`) | all as expected (§5) | **PASS** |
| 14 | Committed binary: behaviour battery 1 (68 programs) + battery 2 (29 programs + 1 CRLF file + 2 stress files) | results in §3 and §10 | mixed |
| 15 | Source-built driver vs committed binary on the 100 cases | **identical exit codes and output**; only a panic line number differs (`vm/lib.rs:203` vs `:212`) → the binary was built from a slightly older revision | **PASS** |

Limits of the substitute evidence: the stand-in `thiserror` generates plain `Display`/`Error` impls and is **not** the real crate; the CLI crate (`clap`, `colored`) and the serde features were not compiled, so CLI code was validated through the committed binary only.

---

## 10. Behaviour battery — summary (100 real executions)

Totals: battery 1 = 68 programs, battery 2 = 29 programs + 1 real-CRLF file + 2 stress files = **100 executions**.

* Valid programs expected to run (battery 1, labelled): **16 / 20** pass. Failures: `f_recursion` (C1), `f_const` (H2), `f_net_types` and `o_move_emit` (H3).
* Invalid programs expected to be rejected (battery 1, labelled): **23 / 26** rejected with a clean diagnostic. Wrongly accepted: `o_move_alias` and `o_move_in_loop` (H4), `r_index_neg` (M4). In battery 2, `s_missing_return` is also wrongly accepted (H7).
* The 11 core type/scope-error programs (`e_type_mismatch`, `e_undefined_var`, `e_undefined_fn`, `e_wrong_arity`, `e_wrong_arg_type`, `e_return_type`, `e_assign_immutable`, `e_assign_const`, `e_cond_not_bool`, `e_add_int_text`, `e_dup_binding`): **11 / 11** rejected.
* Runtime-safety cases: division/modulo by zero **safe**, index out of range **safe**, step limit **safe**; **unsafe**: overflow ×4 (panic), 20,000-deep nesting ×2 (abort), memory doubling (abort), 200k-iteration loop (stack overflow).
* Spec-listed but unimplemented, each probed: `for`, `match`, `import`, tuples, array assignment, `IPv6`, `TCP` → rejected/absent; `struct`, `enum`, effects, top-level statements, `.len` → accepted and **silently ignored**.

The full program set is preserved as a regression suite after fixes (see FINAL_AUDIT).

---

## 11. Critical bugs (summary)

C1 stack leak on `if`/`while` · C2 pipeline stages discarded · C3 overflow panics · C4 unbounded parser recursion · M-mem no memory cap · H1 block scoping · H2 `const` · H3 `emit` moves · H4 ownership holes · H5 `Endpoint` truncation · H6 no short-circuit · H7 missing-return accepted.

## 12. Security concerns (summary)

Public scaffolding prompt (§8-5) · unauthenticated `cargo test` endpoint (B1) · unsandboxed execution of user programs with crash-capable inputs (B2, C4, M-mem) · no rate limit (B3) · stale committed binary served by default (B6) · personal e-mail embedded in artifacts · `unsafe` transmutes (M10).

## 13. Missing / incomplete functionality (vs. the brief)

Landing experience and all 10 chapters · logo, favicon, social image · GSAP/ScrollTrigger · syntax highlighting · Tokens / AST / Diagnostics panels · responsive layout · README + 8 docs · CHANGELOG/CONTRIBUTING · dataflow semantics · borrowing/lifetimes/typestate · IPv6/TCP/UDP/Packet · concurrency · real bytecode artifact format.

## 14. Recommended fixes

1. **VM:** make `JumpIfFalse` pop its condition; checked integer arithmetic returning a runtime error; reject `Endpoint` ports outside 1–65535; add a call-depth limit and a memory/Text-size cap.
2. **Codegen:** per-block scoping of locals; implement `flow`/`|>` stage application for named functions (honest, minimal) and make unsupported expressions a **compile error** instead of `()`; short-circuit `&&`/`||`; top-level `const`.
3. **Sema:** require a `return` on every path of non-`Unit` functions; validate pipe stages; require `main`.
4. **Ownership:** make `let b = a` move; treat `emit` as borrowing; infer copy-ness from the checked type; detect moves in loops.
5. **Parser:** nesting-depth limit with a clean diagnostic.
6. **CLI:** `build` must not execute; honest `version`; token-aware `fmt`; add `tokens`/`ast` output commands so the Playground can show real compiler data.
7. **Repo hygiene:** untrack `yomnova/target/`, add to `.gitignore`; remove legacy artifacts (§8); fix `Cargo.toml` repository URL; real lockfile; run `cargo fmt`.
8. **Backend:** rewrite on Node's built-in `http` (drops `express`, `dotenv`, `@google/genai`), `execFile` + `mkdtemp`, concurrency and rate limits, remove `/api/test-suite`, resolve the binary from `target/release` with a clear error, add `/api/tokens` and `/api/ast`.
9. **Frontend:** rebuild as a cinematic GSAP story with a responsive, accessible Playground wired to the real endpoints; disclose limits in the UI.
10. **Docs:** rewrite README and docs to match §3, with explicit *Implemented / Partial / Planned / Unsupported* labels.

## 15. Prioritised implementation plan

| Priority | Work | Why |
|---|---|---|
| P0 | Git checkpoint; untrack `target/`; remove traces | safety + hygiene |
| P0 | Fix C1, C3, H5 (VM), parser depth (C4) | programs must run correctly and not crash |
| P1 | Scoping (H1), `const` (H2), `emit` borrow (H3), ownership aliasing/loops (H4), short-circuit (H6), missing-return (H7) | semantic correctness |
| P1 | Honest `flow`/`\|>` (C2) or explicit "unsupported" diagnostics | no silent no-ops |
| P1 | CLI: `tokens`/`ast`, real `build`, `version`, `fmt` | Playground needs real data |
| P2 | Regression suite from this audit's 100 programs + unit tests per crate | keep it fixed |
| P2 | Backend rewrite (security) | public exposure |
| P2 | Logo set, design system, GSAP site, Playground | brief |
| P3 | README + docs set, CHANGELOG, CONTRIBUTING | brief |
| P3 | `FINAL_AUDIT.md` comparing every row above | brief |
