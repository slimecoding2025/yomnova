# YOMNOVA — Final Audit

Compares the state recorded in [INITIAL_AUDIT.md](INITIAL_AUDIT.md) (commit `254f12f`) with the final state of this
work. Every number below was produced by running something in the verification environment described next;
anything not run is listed under **Unverifiable**. Status words: PASS · FAIL · PARTIAL · NOT TESTED · BLOCKED.

## 1. Verification environment (read this first)

* Both package registries (`crates.io`, `npmjs.org`) were policy-blocked in the sandbox. I did **not** bypass this.
* **Rust:** the *real* workspace sources (all 14 crates, including the CLI) were compiled in a scratch workspace
  (`mkverify.sh`, kept outside the repo) whose only change is replacing the external `thiserror` crate with a ~130-line local
  proc-macro stand-in supporting exactly the derive patterns used here. Therefore `cargo build/test/clippy/fmt` results below
  are real, but **not** run against the repository's own `Cargo.toml` with the real `thiserror`. The CLI no longer
  depends on `clap`/`colored`; the library crates now depend only on `thiserror`.
* **Web:** `npm install`, `vite build` and `npm run lint` could **not** run. As a stand-in I bundled the same `src/`
  with `esbuild` (React 19.2.8 from the sandbox's global modules, GSAP **3.15.0 cloned from `greensock/GSAP`**,
  used only for testing) and ran `server.ts` with `tsx`. `package.json` declares `gsap ^3.13.0` (the first release with
  free SplitText), `vite ^7`, `@vitejs/plugin-react ^5`, `typescript ^5.8`; those ranges were **not** resolved by a
  package manager and **no lockfile exists** — run `npm install` once and commit it.
* **Types:** `tsc` (7.0.2) type-checked `src/` and `server.ts` against real GSAP typings and `@types/node`, but with a loose
  local shim for React (no `@types/react` available) and without the `vite` types; React-specific typing is therefore unverified.
* **Browser tests:** Playwright/Chromium only (desktop 1440×900, laptop 1280×720, tablet 820×1180, mobile 390×844, plus
  `prefers-reduced-motion`). Firefox and Safari were not tested. The test scripts were ad-hoc and are not in the repository.
* Toolchain: rustc/cargo 1.97.0, Node 22.23.4.

## 2. Scoreboard: initial → final

| Area | Initial | Final | Evidence |
|---|---|---|---|
| Rust unit/integration tests | 10 pass | **32 pass, 0 fail** | `cargo test --workspace` (scratch workspace) |
| `cargo clippy --workspace --all-targets -D warnings` | clean on 13 lib crates | **clean on all 14 crates incl. tests** | run |
| `cargo fmt --check` | FAIL | **PASS** | run |
| Behaviour battery 1 (46 cases with a stated expectation) | 42/46 | **46/46** | `battery.sh` |
| Process crashes in 97 battery programs (exit 101/134/139) | 6 | **0** | battery summaries |
| `fib(15)` / recursion | FAIL (C1 stack leak) | PASS | `fib` in `examples/functions.yom` |
| Pipelines `|>`, `flow` | FAIL (stages discarded) | PASS (sequential) | `pipelines_apply_every_stage` |
| Integer overflow | panic | `IntegerOverflow` error | `integer_overflow_is_an_error_not_a_panic` |
| Deep nesting | abort | `Nesting too deep` error | `deep_nesting_is_rejected_cleanly` |
| Block scoping, `const`, short-circuit | broken | PASS | regression tests |
| Call-argument type checking | none (H8) | static | `static_checks` |
| Ownership: alias, loop, branch, `emit` | holes / false positives | fixed (moves only) | `ownership_rules` |
| `unsafe` code | 2 transmutes | **0** | `grep` |
| CLI third-party deps | clap, colored | **none** | `Cargo.toml` |
| Legacy scaffolding artifacts in tracked files | many | **0** except the description of them in `INITIAL_AUDIT.md` | `git grep` |
| Build output in git | 3,025 files | **0** | `git ls-files` |
| Website | one page, not responsive, no brand | 10 chapters, brand set, responsive | screenshots |
| Playground | partial, no tokens/AST | Run/Check/Format + Output/Diagnostics/Tokens/AST/Bytecode, all real | Playwright e2e |
| Documentation accuracy | FAIL | rewritten against behaviour | docs/ |

The "46 cases" are the battery cases that carry an expectation; 22 further exploratory cases (marked `any`) only
check that the process does not crash. The 6 initial crashes were: 4 integer-overflow/negation panics (exit 101) and the deep-parenthesis and
deep-block aborts (exit 134).

## 3. What was changed

**Compiler/VM** — see [CHANGELOG.md](../CHANGELOG.md). Highest impact: `JumpIfFalse` pops (C1); pipelines compiled
and checked (C2); checked arithmetic (C3); parser nesting cap (C4); scoping, `const`, short-circuit (H1, H2, H6); port
range (H5); argument typing (H8); `emit` borrows (H3); ownership holes (H4); missing returns (H7); no catch-all
`Nil` codegen (M1); `build` no longer executes (M2); honest `version` (M3); negative index (M4); formatter (M8);
transmutes removed (M10). Memory growth (M-mem) is now bounded by the 16 MiB text cap, 65,536 stack slots and
10,000 frames. The battery never reproduced an out-of-memory crash (`r_huge_output` exited cleanly before and after), so the
caps are defensive and their effect on real memory pressure is not demonstrated.

**Web** — original logo system (`public/brand`), GSAP + ScrollTrigger scroll story, real-compiler Playground,
`node:http` backend, removal of Tailwind/lucide/motion/express/dotenv/`@google/genai`.

**GSAP guidance.** I cloned `greensock/gsap-skills`, read its skills and applied them: plugins registered once at module
scope; ScrollTrigger only on top-level tweens/timelines (never on children); containers pinned while children animate;
`scrub` without `toggleActions`; `ease: "none"` on scrubbed tweens; triggers created top-to-bottom; `gsap.matchMedia()`
for reduced-motion and desktop/mobile split with automatic revert; transform/opacity only; SplitText reverted on cleanup;
no scroll hijacking or smooth-scroll library. The skill's horizontal-scroll example contains a typo that was not copied.

## 4. Website verification (what was actually observed)

* No console errors or page errors at any viewport; no horizontal overflow at 1440, 1280, 820, 390 px.
* Pinned chapters step correctly: Domains 1→10, Architecture Source→Virtual machine, Ownership shows the real
  use-after-move diagnostic at step 3, hero scrub converges nodes onto the symbol (checked by screenshots).
* Keyboard order: skip link → logo → header links → chapter rail → …; the Playground editor leaves on `Esc`.
* Playground e2e (real binary): default Run output, Tokens/AST/Bytecode tabs populated from the compiler, type error and
  ownership error land in Diagnostics, infinite loop stops at the 1,000,000-step limit, Format re-indents, Check passes.
* Production-style bundle: `app.js` 356 KB minified (122 KB gzip), `app.css` 22 KB.

## 5. Known limitations (final state)

1. (Updated after this audit: several errors per run are now reported; lexical errors are still one at a time; stages are sequential.) No warnings.
2. Ownership is move-only; no borrows/lifetimes/resources/typestate. Values whose type cannot be inferred are treated as
   copy (conservative against false positives, so it can miss a move).
3. (Updated after this audit: `for … in` over arrays, `len` and `push` now exist.) `struct`, `enum`, methods, lambdas, `match`, `import` are not supported.
4. `yomnova-ir`, `yomnova-network-core`, `yomnova-stdlib` are not wired in; the VM's `And`/`Or` opcodes and
   `IPv6`/`Packet` values are unused by generated code.
5. `build` produces a text listing, not a loadable program. Function effects `![…]` parse but are ignored.
6. The Playground runs untrusted code in a child process **without OS sandboxing** (limits: 5 s, 1 MiB output, 4 jobs,
   64 KiB request). Deploy only behind container isolation.
7. Editor is a textarea with a small highlighter (not Monaco: it could not be installed or tested here).
8. Fonts are system stacks (no web fonts shipped). The OG image uses a relative URL; set an absolute origin when deploying.
9. Mobile and reduced-motion layouts drop pinning entirely: all chapter content is shown as plain scrolling sections.

## 6. Unverifiable here

* `cargo` against the real manifest with the real `thiserror` 2.x (the stand-in could diverge in message formatting for
  derive features this code does not use).
* `npm install`, `vite build`, `npm run lint`, and any behaviour that depends on the exact installed versions of
  React/Vite/plugins.
* Firefox, Safari, real touch devices, screen readers, automated contrast checks and Lighthouse/performance metrics.
* Git push to GitHub (not permitted from this environment): the work is delivered as a git bundle/patch.

## 7. Recommended next steps
Run `npm install` and commit the lockfile; run the full gate in a networked environment
(`cargo test`, `cargo clippy -D warnings`, `npm run lint`, `npm run build`); add a CI workflow; then continue with
`docs/ROADMAP.md` (error recovery, structs/enums, wiring the IR, borrowing).
