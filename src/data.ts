/**
 * Content for the site. Every status below is checked against the repository
 * (see docs/PROGRESS.md). "implemented" means it works today in the compiler/VM,
 * "partial" means a limited version works, "planned" is not built, "research" is an open idea.
 */
export type Status = "implemented" | "partial" | "planned" | "research";

export const STATUS_LABEL: Record<Status, string> = {
  implemented: "Implemented",
  partial: "Partial",
  planned: "Planned",
  research: "Research",
};

export const REPO_URL = "https://github.com/slimecoding2025/yomnova";
export const DOCS_URL = `${REPO_URL}/tree/main/docs`;

export interface Domain {
  name: string;
  status: Status;
  today: string;
  next: string;
}

export const DOMAINS: Domain[] = [
  {
    name: "Application programming",
    status: "partial",
    today: "Functions, recursion, let/mut bindings, if/else, while, for-in over arrays, len and push, Text, Int, Float, Bool.",
    next: "User modules and a larger standard library.",
  },
  {
    name: "Systems programming",
    status: "partial",
    today: "Move semantics are checked at compile time; execution runs on a bounded stack VM.",
    next: "Stored references and lifetimes, resource safety, native code generation.",
  },
  {
    name: "Networking",
    status: "partial",
    today: "IPv4, Port and Endpoint are real values with validated constructors (Port is 1–65535).",
    next: "TCP/UDP and socket-level types.",
  },
  {
    name: "Protocol engineering",
    status: "planned",
    today: "NetworkBuffer (bounds-checked 8/16/32-bit big-endian reads and writes, slice, concat) and Packet (header + payload) work. TCP and UDP are reserved names with no behaviour.",
    next: "Protocol-aware parsing, then TCP/UDP.",
  },
  {
    name: "Distributed computing",
    status: "research",
    today: "Nothing is implemented. This is the long-term direction.",
    next: "A runtime model for communicating nodes.",
  },
  {
    name: "Dataflow pipelines",
    status: "partial",
    today: "The |> operator and flow bindings apply functions left to right, type-checked end to end.",
    next: "Parallel stages, back-pressure, streaming sources.",
  },
  {
    name: "Concurrency",
    status: "planned",
    today: "The VM is single-threaded. No tasks, channels or async.",
    next: "A scheduler and message passing.",
  },
  {
    name: "Security tooling",
    status: "research",
    today: "Memory safety of the VM is the only claim; there is no security-specific library.",
    next: "Capability-style permissions.",
  },
  {
    name: "Infrastructure automation",
    status: "research",
    today: "Not started. The language has no file, process or network I/O.",
    next: "A standard library with controlled effects.",
  },
  {
    name: "Embedded & IoT",
    status: "research",
    today: "Not started. The VM is not designed for constrained targets yet.",
    next: "A no-allocation subset and a compact bytecode format.",
  },
];

export interface Stage {
  name: string;
  crate: string;
  status: Status;
  note: string;
  artifact?: { label: string; text: string };
}

const SAMPLE_SRC = `module demo
fn main() -> Int {
    let n: Int = 2 + 3
    emit(n)
    return 0
}`;

export const PIPELINE_SAMPLE = SAMPLE_SRC;

/** Artifacts below are real output of `yomnova tokens|ast|disasm|run` for PIPELINE_SAMPLE. */
export const STAGES: Stage[] = [
  {
    name: "Source",
    crate: "—",
    status: "implemented",
    note: "UTF-8 text in a .yom file or the Playground editor.",
    artifact: { label: "demo.yom", text: SAMPLE_SRC },
  },
  {
    name: "Lexer",
    crate: "yomnova-lexer",
    status: "implemented",
    note: "Turns characters into tokens with line and column spans. Reports unterminated strings and comments.",
    artifact: {
      label: "yomnova tokens",
      text: "1:1   keyword 'module'\n1:8   identifier 'demo'\n2:1   keyword 'fn'\n2:4   identifier 'main'\n2:8   '('\n3:5   keyword 'let'\n3:9   identifier 'n'\n3:10  ':'\n3:12  identifier 'Int'\n3:16  '='",
    },
  },
  {
    name: "Parser",
    crate: "yomnova-parser",
    status: "implemented",
    note: "Recursive descent with precedence climbing. Nesting is capped so hostile input cannot overflow the stack.",
  },
  {
    name: "AST",
    crate: "yomnova-ast",
    status: "implemented",
    note: "Typed syntax tree: functions, statements, expressions, pipe stages.",
    artifact: {
      label: "yomnova ast (excerpt)",
      text: 'Let {\n    name: "n",\n    ty: Some(Int),\n    init: Binary {\n        op: Add,\n        left: Integer(2),\n        right: Integer(3),\n    },\n}',
    },
  },
  {
    name: "Semantic analysis",
    crate: "yomnova-sema",
    status: "partial",
    note: "Scopes, duplicate and undefined names, mutability, return paths, call arity. Methods and lambdas are rejected as unsupported.",
  },
  {
    name: "Type checking",
    crate: "yomnova-sema · yomnova-types",
    status: "partial",
    note: "Int, Float, Bool, Text, Unit, arrays and the network types. Local inference for let; call arguments are checked.",
  },
  {
    name: "Ownership checking",
    crate: "yomnova-ownership",
    status: "partial",
    note: "Tracks moves (use-after-move, moves in branches and loops) and call-scoped borrows of reference parameters. No stored references or lifetimes.",
  },
  {
    name: "IR",
    crate: "yomnova-ir",
    status: "planned",
    note: "The crate defines data types only. The compiler does not lower to it, so this stage does not run today.",
  },
  {
    name: "Bytecode",
    crate: "yomnova-bytecode · yomnova-compiler",
    status: "implemented",
    note: "AST compiled straight to a stack bytecode with 16-bit jump offsets and constants.",
    artifact: {
      label: "yomnova disasm",
      text: "0000  OP_CONSTANT 0 ('2')\n0003  OP_CONSTANT 1 ('3')\n0006  OP_ADD\n0007  OP_SET_LOCAL 0\n0010  OP_GET_LOCAL 0\n0013  OP_EMIT\n0014  OP_POP\n0015  OP_CONSTANT 2 ('0')\n0018  OP_RETURN",
    },
  },
  {
    name: "Virtual machine",
    crate: "yomnova-vm",
    status: "implemented",
    note: "Stack VM with a step limit, call-depth limit, checked integer arithmetic and bounded memory use.",
    artifact: { label: "yomnova run", text: "5\n=> 0" },
  },
];

export interface OwnershipConcept {
  name: string;
  status: Status;
  note: string;
}

export const OWNERSHIP_CONCEPTS: OwnershipConcept[] = [
  { name: "Move semantics", status: "implemented", note: "Passing, aliasing or returning a non-copy value moves it." },
  { name: "Immutable borrow", status: "implemented", note: "`&T` parameters, `&x` arguments; read-only, call-scoped." },
  { name: "Mutable borrow", status: "implemented", note: "`&mut T` parameters, `&mut x` arguments; exclusive for the call, implemented as copy-in/copy-out." },
  { name: "Lifetimes", status: "planned", note: "Not needed yet: references cannot be stored or returned, so they never outlive a call." },
  { name: "Resource safety", status: "planned", note: "No resources (files, sockets) exist yet." },
  { name: "Typestate", status: "research", note: "An idea for protocol state machines." },
];

export const OWNERSHIP_SRC = [
  "fn consume(payload: Text) -> Unit {",
  '    emit("Consumed payload successfully:")',
  "    emit(payload)",
  "}",
  "",
  "fn main() -> Int {",
  '    let msg: Text = "SYN_PACKET_HEADER_DATA"',
  "    consume(msg)",
  "    emit(msg)",
  "    return 0",
  "}",
];

export const OWNERSHIP_DIAGNOSTIC = "error: Use of moved value 'msg'\n  = hint: Value was previously moved at line 8";

export interface NetType {
  name: string;
  status: Status;
  note: string;
}

export const NET_TYPES: NetType[] = [
  { name: "IPv4", status: "implemented", note: 'IPv4("192.168.0.1") validates and stores four octets.' },
  { name: "Port", status: "implemented", note: "Port(n) accepts 1–65535." },
  { name: "Endpoint", status: "implemented", note: "Endpoint(ip, port) pairs an IPv4 with a port." },
  { name: "IPv6", status: "implemented", note: "IPv6(\"::1\") parses and validates an address; it displays in canonical form and compares with ==." },
  { name: "IPAddress", status: "planned", note: "An IPv4-or-IPv6 sum type; enums and IPv6 exist now, the combined type does not." },
  { name: "Protocol library (std.net)", status: "implemented", note: "Ethernet II, IPv4, IPv6, UDP and TCP: readers, builders, IPv4 header and UDP/TCP checksums, on NetworkBuffer. Parsing and building only: no sockets." },
  { name: "File access", status: "implemented", note: "file_read/write/append/exists, CLI only with --allow-fs DIR; confined to that directory; off in the playground." },
  { name: "Text library", status: "implemented", note: "std.text and str_* natives: upper, lower, trim, contains, find, slice, replace, split, parse_int. Characters, not bytes; no regex." },
  { name: "NetworkBuffer", status: "implemented", note: "NetworkBuffer(n), bytes_of(text), buf_get/get16/get32, buf_set/set16/set32, buf_slice, buf_concat; every access is bounds-checked and writes return a new buffer (max 1 MiB)." },
  { name: "Packet", status: "implemented", note: "Packet(header, payload) with packet_header and packet_payload; a plain container, no protocol parsing." },
  { name: "TCP · UDP", status: "planned", note: "Reserved type names with no behaviour." },
];

export interface RoadmapItem {
  title: string;
  note?: string;
}

export const ROADMAP: Record<Status, RoadmapItem[]> = {
  implemented: [
    { title: "Lexer, parser, AST" },
    { title: "Type checking with call-argument checks" },
    { title: "Move-based ownership checking" },
    { title: "Stack bytecode VM with resource limits" },
    { title: "Pipelines and flow bindings" },
    { title: "CLI: run, check, build, disasm, tokens, ast, fmt" },
    { title: "Regression suite of end-to-end programs" },
  ],
  partial: [
    { title: "Network types", note: "IPv4, Port, Endpoint" },
    { title: "Type inference", note: "let only, no generics" },
    { title: "Ownership", note: "moves and call-scoped borrows" },
    { title: "Formatter", note: "re-indents only" },
  ],
  planned: [
    { title: "Nested or-patterns, per-field coverage analysis in match" },
    { title: "Per-library bytecode and caching, more standard-library modules" },
    { title: "A richer IR (SSA) and inlining" },
    { title: "Stored references and lifetimes" },
    { title: "TCP, UDP" },
    { title: "Concurrency and channels" },
    { title: "Stable, verified bytecode format" },
  ],
  research: [
    { title: "Typestate for protocols" },
    { title: "Effect system for I/O" },
    { title: "Distributed runtime" },
    { title: "Native code generation" },
  ],
};
