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
    today: "Functions, recursion, let/mut bindings, if/else, while, arrays, Text, Int, Float, Bool.",
    next: "Structs, enums, match, for-in loops, imports.",
  },
  {
    name: "Systems programming",
    status: "partial",
    today: "Move semantics are checked at compile time; execution runs on a bounded stack VM.",
    next: "Borrowing, lifetimes, resource safety, native code generation.",
  },
  {
    name: "Networking",
    status: "partial",
    today: "IPv4, Port and Endpoint are real values with validated constructors (Port is 1–65535).",
    next: "IPv6, Packet and socket-level types.",
  },
  {
    name: "Protocol engineering",
    status: "planned",
    today: "Type names for TCP, UDP and Packet are reserved by the lexer; they have no behaviour yet.",
    next: "Typed packet buffers and protocol-aware parsing.",
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
    note: "Scopes, duplicate and undefined names, mutability, return paths, call arity. Structs, enums, methods and lambdas are rejected as unsupported.",
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
    note: "Tracks moves: use-after-move, moves in branches and loops. No borrows or lifetimes yet.",
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
  { name: "Immutable borrow", status: "planned", note: "Not in the language yet." },
  { name: "Mutable borrow", status: "planned", note: "Not in the language yet." },
  { name: "Lifetimes", status: "planned", note: "Depends on borrowing." },
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
  { name: "IPv6", status: "planned", note: "The type name is reserved; there is no constructor or value." },
  { name: "IPAddress", status: "planned", note: "An IPv4-or-IPv6 sum type; needs enums first." },
  { name: "TCP · UDP", status: "planned", note: "Reserved type names with no behaviour." },
  { name: "Packet · NetworkBuffer", status: "planned", note: "Concept only. The VM has an unused packet value." },
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
    { title: "Ownership", note: "moves, no borrows" },
    { title: "Formatter", note: "re-indents only" },
  ],
  planned: [
    { title: "Structs, enums, match" },
    { title: "For-in loops and imports" },
    { title: "IR lowering and optimisation" },
    { title: "Borrow checker and lifetimes" },
    { title: "IPv6, Packet, TCP, UDP" },
    { title: "Concurrency and channels" },
    { title: "Loadable bytecode format" },
  ],
  research: [
    { title: "Typestate for protocols" },
    { title: "Effect system for I/O" },
    { title: "Distributed runtime" },
    { title: "Native code generation" },
  ],
};
