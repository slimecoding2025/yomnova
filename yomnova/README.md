# YOMNOVA

> **ONE LANGUAGE. EVERY SYSTEM.**  
> A high-performance hybrid general-purpose and systems programming language engineered for memory safety, protocol intelligence, native concurrency, and distributed systems.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)
[![Language: Rust](https://img.shields.io/badge/Implementation-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status: Development](https://img.shields.io/badge/Status-0.1.0--dev-green.svg)](docs/PROGRESS.md)

---

## 1. Vision & Architectural Philosophy

Modern systems programming languages frequently treat networking as an external library concern, forcing developers into an awkward compromise: either sacrifice safety for speed in low-level sockets (C/C++), or sacrifice predictability and memory footprint for runtime abstraction (Go/Node/Java).

**YOMNOVA** bridges this divide by embedding **network protocol awareness**, **ownership-driven resource safety**, and **native dataflow pipelines** directly into the type system and compiler architecture.

### The Three Core Differentiators:
1. **Protocol-Aware Type System**: Types like `IPv4`, `IPv6`, `Port`, `Endpoint`, `Packet`, and `EthernetFrame` are first-class language constructs. The compiler distinguishes between unvalidated input bytes and verified network envelopes, rejecting malformed protocol operations at compile time.
2. **Ownership-Driven Resource Safety**: Fine-grained ownership, move semantics, and typestate tracking extend beyond heap memory to network sockets, active TCP streams, open files, and hardware buffers. Connections cannot be used after close, and buffer aliasing is statically prevented.
3. **Network-Native Dataflow Pipelines**: Declarative pipeline operators (`|>`) allow streaming transformations across network packets, event queues, and in-memory collections with zero-copy buffer views and guaranteed backpressure semantics.

---

## 2. Hello World in YOMNOVA

```yom
module genesis

fn main() -> Int {
    emit("Hello from YOMNOVA — One Language. Every System.")
    return 0
}
```

---

## 3. Language Highlights

### Variables & Mutability
```yom
let name: Text = "YOMNOVA"
mut counter: Int = 0
const PORT_DEFAULT: Int = 8080
```

### Functions & Arithmetic
```yom
fn calculate_checksum(base: Int, delta: Int) -> Int {
    return (base * 31) + delta
}
```

### Ownership & Resource Moves
```yom
fn consume(msg: Text) -> Unit {
    emit(msg)
}

fn main() -> Int {
    let message: Text = "Payload data"
    consume(message)
    // message is moved here; subsequent use is rejected by ownership checker!
    return 0
}
```

### Dataflow Pipelines
```yom
flow filtered = 
    [10, 25, 4, 88, 102]
    |> filter(x => x >= 20)
    |> map(x => x * 2)

emit(filtered)
```

### Network Types & Typestate
```yom
let ip: IPv4 = IPv4("127.0.0.1")
let port: Port = Port(8080)
let target: Endpoint = Endpoint(ip, port)

let socket = TCP.connect(target)
socket.send("PING")
let resp = socket.receive()
socket.close()
```

---

## 4. Compiler & Runtime Architecture

```
 SOURCE CODE (.yom)
        │
        ▼
   yomnova-lexer       (Token stream, source spans, diagnostic locations)
        │
        ▼
   yomnova-parser      (Recursive descent declarations + Pratt precedence parser)
        │
        ▼
   yomnova-ast         (Strongly-typed Abstract Syntax Tree)
        │
        ▼
   yomnova-sema        (Symbol tables, nested lexical scopes, type inference)
        │
        ▼
   yomnova-ownership   (Borrow checking, move semantics, use-after-move detection)
        │
        ▼
   yomnova-effects     (Effect tracking: filesystem, network, process)
        │
        ▼
   yomnova-ir          (Control Flow Graph, Basic Blocks, SSA-like TAC)
        │
        ▼
   yomnova-optimizer   (Constant folding, dead code elimination)
        │
        ▼
   yomnova-bytecode    (Opcode set, chunk encoder, constant pool)
        │
        ▼
   yomnova-vm          (Stack-based Virtual Machine, call frames, loopback networking)
```

---

## 5. Quick Start & CLI Usage

### Build the Toolchain
```bash
cd yomnova
cargo build --release
```

### Run a Program
```bash
./target/release/yomnova run examples/hello.yom
```

### Check Syntax & Types without Execution
```bash
./target/release/yomnova check examples/tcp-client.yom
```

### Format Code
```bash
./target/release/yomnova fmt examples/hello.yom
```

### Run Full Test Suite
```bash
cargo test --workspace
```

---

## 6. Project Metadata
- **Creator**: Mouhamed Salim Bousmina
- **Version**: `0.1.0-dev`
- **License**: Dual MIT / Apache-2.0
