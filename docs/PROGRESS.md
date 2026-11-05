# Progress

Last updated: 2026-11-01. Evidence for every claim: the regression suite, unit tests and the two behaviour
batteries described in [FINAL_AUDIT.md](FINAL_AUDIT.md). Baseline: [INITIAL_AUDIT.md](INITIAL_AUDIT.md).

## Language features
| Feature | Status | Evidence |
|---|---|---|
| Functions, recursion, `let`/`mut`/`const`, `if`/`else`, `while` | Implemented | `regression.rs`, `examples/functions.yom` |
| Block scoping and shadowing | Implemented (was miscompiled before) | `block_scoping_and_shadowing` |
| Top-level and local `const` | Implemented (was broken) | `constants_work_globally_and_locally` |
| Short-circuit `&&` / `||` | Implemented (was eager) | `short_circuit_does_not_evaluate_right_side` |
| Checked integer arithmetic | Implemented (was panicking) | `integer_overflow_is_an_error_not_a_panic` |
| Arrays: literal, index, `len`, `push`, `for … in` | Partial | `push` returns a new array (O(n) copy); no `xs[i] = v`; no `pop`/slices |
| Pipelines and `flow` | Partial (was discarding stages) | `pipelines_apply_every_stage` |
| `IPv4`, `Port`, `Endpoint` | Implemented | `endpoint_port_is_range_checked` |
| Static call-argument type checks | Implemented (was runtime-only) | `static_checks` |
| Missing-return detection | Implemented | `static_checks` |
| Ownership: moves, branches, loops, aliasing | Partial (moves only) | `ownership_rules` |
| `struct` (declaration, literal, field read, nesting) | Implemented | `struct_*` and `field_access_errors` regression tests |
| Warnings: unused binding, unreachable code | Implemented | `*_warning*`, `unused_*`, `code_after_return_*` regression tests |
| `enum` and `match` (statement, exhaustive, one-level patterns) | Implemented | `enum_*` and `match_*` regression tests |
| `for … in` over arrays, `len`, `push` | Implemented | `for_*`, `len_*`, `push_*` regression tests |
| methods, lambdas | Parsed-only, rejected clearly | `unsupported_syntax_is_reported_not_miscompiled`, `static_checks` |
| `import`, `type` | Planned (reserved words only) | — |
| Borrowing, lifetimes, concurrency, `IPv6`/`Packet`/`TCP`/`UDP` | Planned | — |

## Tooling
| Item | Status |
|---|---|
| CLI `run check build disasm tokens ast fmt version` | Implemented; dependency-free |
| `build` / `exec` | Implemented: binary `.yomc` (format v1), loader validated and fuzz-tested | `a_saved_program_behaves_exactly_like_the_source`, `corrupted_files_never_panic_the_loader_or_the_vm` |
| Formatter | Re-indent only (idempotent, string/comment safe) |
| Web site (10 chapters, GSAP) and Playground | Implemented; see limitations in FINAL_AUDIT.md |
| Monaco editor | Not used (text area + small highlighter) |
| AI Assistant (OpenRouter, suggest-only) | Implemented; tested with a mock provider only, live provider NOT verified (see [AI_ASSISTANT.md](AI_ASSISTANT.md)) |

## Quality gates (last run, sandbox without registry access — see FINAL_AUDIT.md §Verification environment)
`cargo fmt --check` clean · `cargo clippy --workspace --all-targets -D warnings` clean · 87 Rust tests pass.

AI Assistant (2026-11-02): `npm run test:ai` 46 pass against the real compiler binary with a mocked OpenRouter; Playwright
end-to-end 14 checks pass (Chromium, mock provider). `npm run build` / `npm run lint` were not run with real tooling.
