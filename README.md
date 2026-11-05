<p align="center">
  <img src="public/brand/logo.svg" alt="YOMNOVA" height="64" />
</p>

<h3 align="center">ONE LANGUAGE. EVERY SYSTEM.</h3>

<p align="center">
  An experimental hybrid programming language exploring the intersection of general-purpose programming, systems
  engineering, networking, and distributed computing.
</p>

---

## 1. Status

**Early-stage research project (v0.1).** A small but real language works end to end: lexer → parser → type
checker → move checker → bytecode → VM, with a CLI and a web Playground that runs the real compiler. Most of the
larger vision (borrowing, protocol types beyond IPv4/Port/Endpoint, concurrency, distribution) is **not built**.
Every feature below carries a label — *Implemented*, *Partial*, *Planned* or *Research* — and
[docs/PROGRESS.md](docs/PROGRESS.md) lists the evidence.

## 2. Vision
Software for networks is usually written in several languages and toolkits. YOMNOVA is an experiment in one
coherent language where network types, ownership and dataflow are part of the language itself.

## 3. What works today
* **Implemented:** functions and recursion, `let`/`mut`/`const`, `if`/`else`, `while`, short-circuit logic,
  `Int` (checked 64-bit), `Float`, `Bool`, `Text`, arrays (literal + index), `IPv4`/`Port`/`Endpoint` values,
  static type checking including call arguments, move-based ownership checking, a bytecode VM with resource
  limits, and the `|>` pipeline operator / `flow` bindings.
* **Partial:** arrays (`len`, `push` returning a new array, `for … in`; no element assignment or nested-array typing beyond inference), ownership (moves only), type inference (local), formatter
  (re-indent only).
* **Parsed but rejected:** methods, lambdas. `struct` (literals, field reads; no field assignment) and `enum` with an exhaustive `match` statement work; patterns are one level deep.
* **Planned / Research:** borrowing and lifetimes, `IPv6`/`Packet`/`TCP`/`UDP`, concurrency, distributed
  runtime, IR optimisation, native code generation. See [docs/ROADMAP.md](docs/ROADMAP.md).

## 4. Example
Verified output below is from `yomnova run yomnova/examples/showcase.yom`.

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

    let message: Text = "owned once"
    emit(message)
    return 0
}
```

```text
YOMNOVA
127.0.0.1:8080
60
owned once
=> 0
```

Using a moved value is rejected before the program runs (`yomnova/examples/ownership_fail.yom`):

```text
error: Use of moved value 'msg'
  = hint: Value was previously moved at line 9
```

## 5. Architecture
`source → lexer → parser → AST → semantic analysis / types → ownership → bytecode → VM`. `yomnova-ir` exists but is
**not** part of the pipeline yet. Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## 6. Requirements
Rust (stable, edition 2021) and Node.js ≥ 20 for the web app. No other tools are required.

## 7. Build the compiler
```bash
cargo build --release --manifest-path yomnova/Cargo.toml     # or: npm run build:compiler
```

## 8. Command-line usage
```text
yomnova run [-d|--disassemble] <file>   compile and run
yomnova check <file>                    parse, type-check, ownership-check only
yomnova build [-o out] <file>           compile to a .yomc bytecode file (does not run the program)
yomnova exec <file.yomc>                run a .yomc file produced by `build`
yomnova disasm <file>                   print the bytecode listing
yomnova tokens <file> | ast <file>      show tokens / syntax tree
yomnova fmt [--check] <file>            re-indent source
yomnova version
```

## 9. Web site and Playground
```bash
npm install
npm run dev        # http://localhost:3000 (Vite middleware + API)
npm run build && npm start   # production: serves dist/
```
The Playground sends your code to `server.ts`, which runs the real `yomnova` binary (5 s timeout, 1 MiB output cap,
4 concurrent jobs). Tabs show the program's output, diagnostics, tokens, AST and bytecode — all produced by the
compiler. If the server or binary is unavailable the site says so; nothing is simulated. Set `YOMNOVA_BIN` to use a
specific binary. **There is no OS-level sandbox**; run behind container isolation if you expose it publicly.

### Deploying
A `Dockerfile` and a Render Blueprint (`render.yaml`) are included; see [docs/DEPLOY.md](docs/DEPLOY.md) (image build and
Render deployment not yet verified).

### AI Assistant (optional)
The Playground has a suggest-only AI panel (explain compiler errors, explain selected code, syntax tutoring, fix
suggestions you can validate with the compiler). It is **off** unless the server has `OPENROUTER_API_KEY`, sends your
code to the chosen model provider through OpenRouter, and has only been tested against a mock provider. See
[docs/AI_ASSISTANT.md](docs/AI_ASSISTANT.md) for setup, privacy and limitations.

## 10. Examples
`yomnova/examples/`: `hello`, `variables`, `functions`, `pipeline`, `dataflow`, `ownership`, `ownership_fail`
(rejected by the ownership checker), `type_error` (rejected by the type checker), `tcp-client`, `showcase`.

## 11. Testing
```bash
cd yomnova
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # unit tests + end-to-end regression suite
```
Honest verification notes and what could not be run are in [docs/FINAL_AUDIT.md](docs/FINAL_AUDIT.md).

## 12. Project structure
```text
yomnova/            Rust workspace (14 crates) + examples
src/  server.ts     React/GSAP site and node:http API
public/brand/       logo set (generated by scripts/generate-brand.py)
docs/               specification, grammar, architecture, roadmap, progress, audits
```

## 13. Language reference
[docs/LANGUAGE_SPECIFICATION.md](docs/LANGUAGE_SPECIFICATION.md) · [docs/GRAMMAR.md](docs/GRAMMAR.md)

## 14. Safety and limits
Integer arithmetic is checked; the VM caps steps (1,000,000), stack (65,536), call depth (10,000) and text (16 MiB);
the parser caps nesting (150). The language has no file, process or network access. The workspace contains no
`unsafe` code.

## 15. Known limitations
Several errors per run, but lexical errors are reported one at a time and later stages wait for earlier ones · only two warnings (unused binding, unreachable code) · no borrowing/lifetimes · the `.yomc` format is version 1 only (no compatibility across opcode changes, no signatures) ·
the formatter only re-indents · the Playground editor is a plain text area with a small highlighter (not Monaco) ·
IR, `yomnova-network-core` and `yomnova-stdlib` are not wired into the pipeline.

## 16. Roadmap
[docs/ROADMAP.md](docs/ROADMAP.md)

## 17. Changelog
[CHANGELOG.md](CHANGELOG.md)

## 18. Contributing
[CONTRIBUTING.md](CONTRIBUTING.md)

## 19. License
MIT OR Apache-2.0.

## 20. Author
Created by **Mouhamed Salim Bousmina** · <https://github.com/slimecoding2025/yomnova>
