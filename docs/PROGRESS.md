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
| `enum` and `match` (statement and expression, nested/literal/binding patterns, exhaustiveness incl. nested cases) | Implemented | `enum_*`, `match_*`, `nested_pattern_checks`, `examples/match_expr.yom` |
| Match guards (`pattern if cond => …`) | Implemented | `match_guards_*`, `match_guard_rules_*`, `examples/match_guards.yom` |
| Or-patterns (`A | B`, top level of an arm) | Implemented | `or_pattern*`, `examples/match_or.yom` |
| Integer range patterns (`1..=5`) | Implemented | `range_pattern*`, `examples/match_range.yom` |
| Struct patterns (`Point { x, y: 0, .. }`) | Implemented (no per-field coverage analysis) | `struct_pattern*`, `examples/match_struct.yom` |
| Nested or-patterns | Planned | — |
| `for … in` over arrays, `len`, `push` | Implemented | `for_*`, `len_*`, `push_*` regression tests |
| methods, lambdas | Parsed-only, rejected clearly | `unsupported_syntax_is_reported_not_miscompiled`, `static_checks` |
| `import`, `type` | Planned (reserved words only) | — |
| Borrowing: `&T`/`&mut T` parameters, `&x`/`&mut x` arguments, conflict checks | Implemented (call-scoped only) | `shared_borrows_*`, `mutable_borrows_*`, `borrow_rule_*`, `references_are_parameter_only_*`, `examples/borrowing.yom` |
| `import std.math`, `import std.arrays` (built-in modules written in YOMNOVA) | Implemented (built-in only) | `std_modules_*`, `bad_imports_*`, `examples/stdlib.yom` |
| Bytecode verifier: structure + operand-stack depth consistency | Implemented | `stack_verifier_*`, `every_compiled_example_passes_the_stack_verifier` |
| Operand-type verification of bytecode | Partial (constants, operator results and locals; not call results, fields or elements) | `type_verifier_rejects_statically_wrong_operands` |
| File access (`file_*`), CLI opt-in `--allow-fs DIR`, confined to DIR | Implemented | `file_access_is_off_by_default_*`, `examples/files.yom` |
| Protocol library `std.net`: Ethernet II, IPv4, IPv6 (fixed header), UDP, TCP: readers, builders, header and UDP/TCP checksums | Implemented (parsing/building only) | `std_net_*`, `examples/net.yom`, `examples/net_layers.yom` |
| VLAN 802.1Q, ARP (Ethernet/IPv4), ICMPv4 echo and checksum | Implemented | `icmp_arp_and_vlan_in_std_net`, `examples/security_network.yom` |
| Hashing and encoding natives (`sha256`, `hmac_sha256`, hex, base64, `buf_find`, `buf_entropy`, `file_read_bytes`) | Implemented | `hashing_and_encoding_natives_*`, unit tests with NIST/RFC vectors |
| `std.sec`: signature and heuristic scan, PE section checks, IOC lists, port scan / SYN flood / ARP spoof / ICMP tunnel / log brute-force checks | Implemented (simple indicators, not an antivirus) | `std_sec_*`, `examples/security_*.yom` |
| `std.rules`: line-based detection rules (text, hex, sha256) | Implemented (byte wildcards `??`; no conditions) | `std_rules_*`, `examples/security_rules.yom` |
| `std.pcap`: classic libpcap read/write | Implemented (no pcapng) | `std_pcap_*`, `examples/security_pcap.yom` |
| Extension headers, ICMPv6, fragmentation, pcapng, richer rules (wildcards, conditions), ELF/Mach-O checks | Planned | — |
| Text library: `str_*` natives and `import std.text` | Implemented | `text_library_works_*`, `text_errors_are_reported_*`, `examples/text.yom` |
| File imports `import "f.yom" as name` with `pub` visibility, namespaced functions/structs/enums (CLI) | Implemented (source renaming) | `imports_rename_and_nest`, `bad_imports_are_errors`, `examples/modules/` |
| Each imported library type-checked and ownership-checked on its own, errors reported in its own file | Implemented (CLI) | `libraries_are_checked_on_their_own_*` |
| Separately compiled modules (per-library `.yomc`, caching), glob/`use` imports, nested paths | Planned | — |
| Stored references, lifetimes, concurrency, `TCP`/`UDP` | Planned | — |
| IR optimiser (`yomnova-ir`, `--no-opt`) | Implemented | `optimiser_never_changes_behaviour`, `optimiser_shrinks_constant_code_*`, 6 unit tests |
| `NetworkBuffer`, `Packet` | Implemented | `buffers_*`, `buffer_*`, `packets_*` regression tests, `examples/packet.yom` |
| `IPv6` constructor, display, equality | Implemented | regression tests |

## Tooling
| Item | Status |
|---|---|
| CLI `run check build disasm tokens ast fmt version` | Implemented; dependency-free |
| `build` / `exec` | Implemented: binary `.yomc` (format v2), loader validated and fuzz-tested | `a_saved_program_behaves_exactly_like_the_source`, `corrupted_files_never_panic_the_loader_or_the_vm` |
| Formatter | Re-indent only (idempotent, string/comment safe) |
| Web site (10 chapters, GSAP) and Playground | Implemented; see limitations in FINAL_AUDIT.md |
| Monaco editor | Not used (text area + small highlighter) |
| AI Assistant (OpenRouter, suggest-only) | Implemented; tested with a mock provider only, live provider NOT verified (see [AI_ASSISTANT.md](AI_ASSISTANT.md)) |

## Quality gates (last run, sandbox without registry access — see FINAL_AUDIT.md §Verification environment)
`cargo fmt --check` clean · `cargo clippy --workspace --all-targets -D warnings` clean · 166 Rust tests pass.

AI Assistant (2026-11-02): `npm run test:ai` 46 pass against the real compiler binary with a mocked OpenRouter; Playwright
end-to-end 14 checks pass (Chromium, mock provider). `npm run build` / `npm run lint` were not run with real tooling.
