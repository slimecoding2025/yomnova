<p align="center">
  <img src="public/brand/logo.svg" alt="YOMNOVA" height="64" />
</p>

<h3 align="center">ONE LANGUAGE. EVERY SYSTEM.</h3>

<p align="center">
  An experimental programming language for networking and defensive security, with a Rust compiler,
  a bytecode VM, a security-analysis standard library and a web Playground.
</p>

<p align="center">
  <a href="https://yomnova.onrender.com">Live demo</a> ·
  <a href="docs/LANGUAGE_SPECIFICATION.md">Language reference</a> ·
  <a href="docs/SECURITY_TOOLING.md">Security toolkit</a> ·
  <a href="docs/ROADMAP.md">Roadmap</a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

---

## Overview

YOMNOVA explores one idea: network and security code is usually spread over several languages and tools, and a single
small language with network types, ownership rules and analysis helpers built in can make it safer to write.
The compiler is written in Rust and runs a complete pipeline:

```text
source → lexer → parser → AST → type checker → ownership checker → bytecode → IR optimiser → verifier → VM
```

> **Status: early-stage research project.** The language and its libraries work end to end and are covered by
> automated tests, but they are young. Every claim in this README is backed by a test or an example; what is not built
> is listed under [Limitations](#limitations). [docs/PROGRESS.md](docs/PROGRESS.md) holds the evidence table.

## Highlights

| Area | What you get |
|---|---|
| **Language** | Functions, `let`/`mut`/`const`, `if`/`while`/`for … in`, `struct`, `enum`, pattern `match` (guards, or-patterns, ranges, struct patterns, exhaustiveness checking), pipelines (`\|>`, `flow`), checked 64-bit arithmetic |
| **Ownership** | Move checking, plus shared (`&T`) and mutable (`&mut T`) borrows for function arguments with conflict detection |
| **Network types** | `IPv4`, `IPv6`, `Port`, `Endpoint`, `NetworkBuffer`, `Packet` |
| **Modules** | Built-in `std.*` libraries written in YOMNOVA itself, and file imports with `pub` visibility |
| **Tooling** | CLI (`run`, `check`, `build`, `exec`, `disasm`, `tokens`, `ast`, `fmt`), `.yomc` bytecode files with a verifier, an optimiser you can turn off, a web Playground |
| **Safety** | Step, stack, depth and text limits in the VM; no file access unless you pass `--allow-fs DIR`; no network access; no `unsafe` code in the workspace |

## Security toolkit

All of it is **defensive analysis of data you give it**. Nothing here sends packets, opens sockets, runs the data it
inspects or contains exploit or malware code (the only test signature is the harmless EICAR string).

| Library | Purpose |
|---|---|
| `std.net` | Ethernet, VLAN, ARP, IPv4, IPv6 (with extension headers), ICMP, ICMPv6, UDP, TCP and DNS readers and builders, with checksums |
| `std.pcap` | Read and write classic libpcap captures |
| `std.sec` | Signature and heuristic file scan (PE and ELF checks, entropy, EICAR), indicator lists, SHA-256 and HMAC, and traffic checks: port scans, SYN floods, ARP spoofing, ICMP tunnelling, odd TCP flags, DNS tunnelling, beaconing, log brute force |
| `std.rules` | A small rule language (`severity \| name \| kind \| pattern`) with text, hex (with `??` wildcards), SHA-256 and `N of` / `all of` / `any of` / `none of` groups |
| `std.scan` | nmap-style helpers: CIDR and port-list parsing, service and risk tables, `nmap -oG` output reading, scan-to-scan diffs, banner checks |

```yomnova
import std.sec

fn main() -> Int {
    let sample = bytes_of("X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*")
    let findings = sec.scan(&sample)
    emit(sec.verdict(&findings))
    return 0
}
```

```text
malicious indicators
=> 0
```

These are simple indicators, **not an antivirus or an IDS**: signatures miss new threats and heuristics raise false
alarms. In a survey of 80 real ELF binaries from `/usr/bin`, 77 were reported clean and 3 were false positives; no real
malware was used, so real-world detection has not been measured. Details and honest limits:
[docs/SECURITY_TOOLING.md](docs/SECURITY_TOOLING.md). Runnable programs are in
[`yomnova/examples/`](yomnova/examples) (`security_*.yom`).

## Quick start

Requirements: Rust (stable) for the compiler, Node.js 20 or newer for the web app.

```bash
git clone https://github.com/slimecoding2025/yomnova
cd yomnova
cargo build --release --manifest-path yomnova/Cargo.toml     # or: npm run build:compiler
./yomnova/target/release/yomnova run yomnova/examples/showcase.yom
```

### Example

```yomnova
module demo

const GREETING: Text = "YOMNOVA"

fn inc(n: Int) -> Int {
    return n + 1
}

fn scale(n: Int, k: Int) -> Int {
    return n * k
}

fn main() -> Int {
    emit(GREETING)

    let server: Endpoint = Endpoint(IPv4("127.0.0.1"), Port(8080))
    emit(server)

    flow total = 5 |> inc |> scale(10)
    emit(total)
    return 0
}
```

```text
YOMNOVA
127.0.0.1:8080
60
=> 0
```

Using a moved value is rejected before the program runs (`yomnova/examples/ownership_fail.yom`):

```text
error: Use of moved value 'msg'
  = hint: Value was previously moved at line 9
```

## Command line

```text
yomnova run [-d] [--no-opt] [--allow-fs DIR] <file>   compile and run
yomnova check <file>                                  parse, type-check and ownership-check only
yomnova build [-o out] <file>                         compile to a .yomc bytecode file
yomnova exec [--allow-fs DIR] <file.yomc>             run a .yomc file
yomnova disasm <file>                                 print the bytecode listing
yomnova tokens <file> | ast <file>                    show tokens / syntax tree
yomnova fmt [--check] <file>                          re-indent source
yomnova version
```

## Web site and Playground

```bash
npm install
npm run dev                  # http://localhost:3000
npm run build && npm start   # production build served from dist/
```

The Playground sends your code to `server.ts`, which runs the real `yomnova` binary (5 s timeout, 1 MiB output cap,
4 concurrent jobs) and shows output, diagnostics, tokens, AST and bytecode exactly as the compiler produced them.
Set `YOMNOVA_BIN` to use a specific binary. **There is no OS-level sandbox**: put it behind container isolation if you
expose it publicly. A `Dockerfile` and a Render Blueprint (`render.yaml`) are included, see
[docs/DEPLOY.md](docs/DEPLOY.md).

**AI Assistant (optional).** A suggest-only panel explains compiler errors and code and proposes fixes you can check
with the compiler. It is off unless the server has `OPENROUTER_API_KEY`, and it sends your code to the model provider.
It has been tested with a mock provider; see [docs/AI_ASSISTANT.md](docs/AI_ASSISTANT.md).

## Testing

```bash
cd yomnova
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # unit tests + end-to-end regression suite
cd .. && npm run test:ai          # AI assistant tests (mock provider)
```

The Rust suite includes fuzz-style tests that feed random, truncated and corrupted PE, ELF, pcap and DNS data to the
analysis libraries and corrupted `.yomc` files to the loader and VM. What was and was not verified:
[docs/FINAL_AUDIT.md](docs/FINAL_AUDIT.md).

## Project layout

```text
yomnova/            Rust workspace (14 crates), standard library sources and examples
src/  server.ts     React/GSAP site and the Node API behind the Playground
public/brand/       logo set
docs/               specification, grammar, architecture, security toolkit, roadmap, progress, audits
tests/              AI assistant and end-to-end tests
```

## Limitations

* Borrows exist only as function arguments; there are no stored references or lifetimes.
* Methods and lambdas are parsed but rejected with a clear error; there is no concurrency, no `TCP`/`UDP` sockets and
  no distributed runtime.
* Nested or-patterns and per-field coverage analysis in struct patterns are not implemented.
* Libraries are expanded into the program's source, not compiled separately; `.yomc` is version 2 only, with no
  signatures.
* Security helpers: pcapng limited to packet bytes, no IPv6 fragment reassembly, no Mach-O, no regular expressions in rules, no
  active scanning.
* Not verified: the live OpenRouter provider and iOS Safari.

## Roadmap, changelog, contributing

[docs/ROADMAP.md](docs/ROADMAP.md) · [CHANGELOG.md](CHANGELOG.md) · [CONTRIBUTING.md](CONTRIBUTING.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Unless you explicitly state
otherwise, any contribution you submit for inclusion is dual-licensed as above, without additional terms.

## Author

Created by **Mouhamed Salim Bousmina** · <https://github.com/slimecoding2025/yomnova>
