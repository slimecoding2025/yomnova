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
| Arrays: literal and index | Partial | no length/push/iteration |
| Pipelines and `flow` | Partial (was discarding stages) | `pipelines_apply_every_stage` |
| `IPv4`, `Port`, `Endpoint` | Implemented | `endpoint_port_is_range_checked` |
| Static call-argument type checks | Implemented (was runtime-only) | `static_checks` |
| Missing-return detection | Implemented | `static_checks` |
| Ownership: moves, branches, loops, aliasing | Partial (moves only) | `ownership_rules` |
| `struct`, `enum`, methods, lambdas, field access | Parsed-only, rejected clearly | `unsupported_syntax_is_reported_not_miscompiled` |
| `for`, `match`, `import`, `type` | Planned (reserved words only) | — |
| Borrowing, lifetimes, concurrency, `IPv6`/`Packet`/`TCP`/`UDP` | Planned | — |

## Tooling
| Item | Status |
|---|---|
| CLI `run check build disasm tokens ast fmt version` | Implemented; dependency-free |
| `build` output | Textual listing, not an executable format |
| Formatter | Re-indent only (idempotent, string/comment safe) |
| Web site (10 chapters, GSAP) and Playground | Implemented; see limitations in FINAL_AUDIT.md |
| Monaco editor | Not used (text area + small highlighter) |

## Quality gates (last run, sandbox without registry access — see FINAL_AUDIT.md §Verification environment)
`cargo fmt --check` clean · `cargo clippy --workspace --all-targets -D warnings` clean · 32 Rust tests pass.
