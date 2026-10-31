/**
 * @license
 * Copyright 2026 Mouhamed Salim Bousmina.
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

import React, { useState, useEffect, useRef } from "react";
import {
  Play,
  CheckCircle2,
  AlertTriangle,
  Code2,
  Cpu,
  ShieldCheck,
  Network,
  BookOpen,
  Terminal as TerminalIcon,
  RotateCcw,
  Sparkles,
  Layers,
  Copy,
  ExternalLink,
  Zap,
  Activity,
  Check,
  FileCode,
  Bug,
} from "lucide-react";

interface ExampleItem {
  id: string;
  filename: string;
  title: string;
  description: string;
  code: string;
}

const DEFAULT_EXAMPLES: ExampleItem[] = [
  {
    id: "hello",
    filename: "hello.yom",
    title: "1. Hello Genesis",
    description: "Basic module declaration, entrypoint function, and output emit",
    code: `module genesis

fn main() -> Int {
    emit("Hello from YOMNOVA — One Language. Every System.")
    return 0
}`,
  },
  {
    id: "variables",
    filename: "variables.yom",
    title: "2. Variables & Mutability",
    description: "Immutable bindings, mutable counters, and arithmetic",
    code: `module variables_demo

fn main() -> Int {
    let language: Text = "YOMNOVA"
    let version: Text = "0.1.0"
    mut counter: Int = 10

    counter = counter + 5

    emit(language)
    emit(version)
    emit(counter)

    return 0
}`,
  },
  {
    id: "ownership_pass",
    filename: "ownership.yom",
    title: "3. Ownership Move Semantics",
    description: "Passing resource by value moves ownership to callee",
    code: `module ownership_demo

fn consume(payload: Text) -> Unit {
    emit("Consumed payload successfully:")
    emit(payload)
}

fn main() -> Int {
    let packet_data: Text = "SYN_PACKET_HEADER_DATA"
    
    emit("Preparing to consume packet:")
    consume(packet_data)

    // packet_data was moved safely to consume()
    return 0
}`,
  },
  {
    id: "ownership_fail",
    filename: "ownership_fail.yom",
    title: "4. Ownership Violation (Negative Test)",
    description: "Compiler statically catches use-after-move error with source caret diagnostics",
    code: `module test_negative

fn consume(payload: Text) -> Unit {
    emit(payload)
}

fn main() -> Int {
    let msg: Text = "Important network packet"
    consume(msg)
    // Ownership error! msg was moved at previous line:
    emit(msg)
    return 0
}`,
  },
  {
    id: "dataflow",
    filename: "dataflow.yom",
    title: "5. Network Buffer Dataflow",
    description: "Dataflow pipelines across buffer collections",
    code: `module dataflow_demo

fn main() -> Int {
    flow packet_sizes = [64, 128, 256, 512, 1024, 1500]

    emit("Streaming network packet buffer sizes:")
    emit(packet_sizes)

    return 0
}`,
  },
  {
    id: "tcp_client",
    filename: "tcp-client.yom",
    title: "6. Protocol-Aware Types",
    description: "First-class IPv4, Port, and Endpoint protocol construction",
    code: `module tcp_client_demo

fn main() -> Int {
    let server_ip: IPv4 = IPv4("127.0.0.1")
    let server_port: Port = Port(8080)
    let remote: Endpoint = Endpoint(server_ip, server_port)

    emit("Configured YOMNOVA protocol endpoint:")
    emit(remote)

    return 0
}`,
  },
];

export default function App() {
  const [selectedExample, setSelectedExample] = useState<string>("hello");
  const [code, setCode] = useState<string>(DEFAULT_EXAMPLES[0].code);
  const [activeTab, setActiveTab] = useState<"terminal" | "disassembler" | "ownership" | "network" | "docs" | "testSuite">("terminal");
  const [output, setOutput] = useState<string>("");
  const [disassembly, setDisassembly] = useState<string>("");
  const [isRunning, setIsRunning] = useState<boolean>(false);
  const [isChecking, setIsChecking] = useState<boolean>(false);
  const [isTesting, setIsTesting] = useState<boolean>(false);
  const [testResults, setTestResults] = useState<string>("");
  const [statusMessage, setStatusMessage] = useState<string>("Compiler Ready (v0.1.0-dev)");
  const [copied, setCopied] = useState<boolean>(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-run on example switch
  const handleSelectExample = (exId: string) => {
    setSelectedExample(exId);
    const ex = DEFAULT_EXAMPLES.find((e) => e.id === exId);
    if (ex) {
      setCode(ex.code);
      setOutput(`// Selected example: ${ex.filename}\n// Click 'Run Program' or press Ctrl+Enter to execute.\n`);
      setDisassembly("");
    }
  };

  const handleRun = async () => {
    setIsRunning(true);
    setStatusMessage("Compiling and executing in VM...");
    setActiveTab("terminal");
    try {
      const res = await fetch("/api/run", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      });
      const data = await res.json();
      if (data.success) {
        setOutput(data.stdout || "// Process finished with exit code 0 (no output)");
        setStatusMessage("Execution Succeeded (Exit code 0)");
      } else {
        setOutput(data.stderr || data.stdout || "// Execution failed with non-zero exit code");
        setStatusMessage(`Compiler Rejected (Exit code ${data.exitCode})`);
      }

      // Automatically fetch disassembly for inspection
      fetchDisassembly();
    } catch (e: any) {
      setOutput(`Error connecting to YOMNOVA compiler daemon: ${e.message}`);
      setStatusMessage("Connection error");
    } finally {
      setIsRunning(false);
    }
  };

  const handleCheck = async () => {
    setIsChecking(true);
    setStatusMessage("Running static analysis & ownership check...");
    setActiveTab("terminal");
    try {
      const res = await fetch("/api/check", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      });
      const data = await res.json();
      if (data.success) {
        setOutput(data.stdout || "✓ Static Verification Passed: 0 syntax, type, or ownership errors found.");
        setStatusMessage("Verification Passed (0 errors)");
      } else {
        setOutput(data.stderr || data.stdout || "Verification failed");
        setStatusMessage("Verification Failed");
      }
    } catch (e: any) {
      setOutput(`Verification connection error: ${e.message}`);
    } finally {
      setIsChecking(false);
    }
  };

  const handleFormat = async () => {
    try {
      const res = await fetch("/api/fmt", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      });
      const data = await res.json();
      if (data.success && data.formatted) {
        setCode(data.formatted);
        setStatusMessage("Code formatted with yomnova fmt");
      }
    } catch (_) {}
  };

  const fetchDisassembly = async () => {
    try {
      const res = await fetch("/api/disassemble", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      });
      const data = await res.json();
      if (data.bytecode) {
        setDisassembly(data.bytecode);
      }
    } catch (_) {}
  };

  const handleRunTests = async () => {
    setIsTesting(true);
    setActiveTab("testSuite");
    setStatusMessage("Running workspace test suites across crates...");
    try {
      const res = await fetch("/api/test-suite", { method: "POST" });
      const data = await res.json();
      setTestResults(data.stdout || data.stderr || "No test output");
      setStatusMessage(data.success ? "All Workspace Tests Passed!" : "Tests finished with failures");
    } catch (e: any) {
      setTestResults(`Error running tests: ${e.message}`);
    } finally {
      setIsTesting(false);
    }
  };

  const copyCode = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Keyboard shortcut Ctrl+Enter to run
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        handleRun();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [code]);

  return (
    <div className="flex flex-col h-screen w-screen bg-[#0d1117] text-[#c9d1d9] font-sans overflow-hidden">
      {/* Top Navigation Bar */}
      <header className="flex items-center justify-between px-5 py-3 border-b border-[#30363d] bg-[#161b22]">
        <div className="flex items-center space-x-3">
          <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-gradient-to-tr from-cyan-600 via-teal-500 to-indigo-600 shadow-md shadow-cyan-900/30">
            <Cpu className="w-5 h-5 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <span className="font-extrabold text-base tracking-wider text-white">YOMNOVA</span>
              <span className="text-[10px] uppercase font-bold tracking-widest px-1.5 py-0.5 rounded bg-cyan-950 text-cyan-400 border border-cyan-800/60">
                0.1.0-dev
              </span>
              <span className="text-xs text-[#8b949e] hidden sm:inline">| ONE LANGUAGE. EVERY SYSTEM.</span>
            </div>
            <div className="text-[11px] text-[#8b949e]">
              Original Hybrid Systems &amp; Networking Language • By Mouhamed Salim Bousmina
            </div>
          </div>
        </div>

        {/* Global Toolbar */}
        <div className="flex items-center space-x-2">
          <div className="hidden md:flex items-center space-x-2 text-xs px-3 py-1.5 rounded bg-[#21262d] border border-[#30363d] text-[#8b949e]">
            <Activity className="w-3.5 h-3.5 text-emerald-400 animate-pulse" />
            <span>{statusMessage}</span>
          </div>

          <button
            onClick={handleCheck}
            disabled={isChecking || isRunning}
            className="flex items-center space-x-1.5 px-3 py-1.5 text-xs font-medium rounded-md bg-[#21262d] hover:bg-[#30363d] text-zinc-200 border border-[#30363d] transition cursor-pointer"
            title="Check syntax and ownership without running"
          >
            <ShieldCheck className="w-3.5 h-3.5 text-cyan-400" />
            <span className="hidden sm:inline">Check</span>
          </button>

          <button
            onClick={handleFormat}
            className="flex items-center space-x-1.5 px-3 py-1.5 text-xs font-medium rounded-md bg-[#21262d] hover:bg-[#30363d] text-zinc-200 border border-[#30363d] transition cursor-pointer"
            title="Format source code"
          >
            <RotateCcw className="w-3.5 h-3.5 text-amber-400" />
            <span className="hidden sm:inline">Fmt</span>
          </button>

          <button
            onClick={handleRun}
            disabled={isRunning || isChecking}
            className="flex items-center space-x-2 px-4 py-1.5 text-xs font-bold rounded-md bg-cyan-600 hover:bg-cyan-500 text-white shadow-sm shadow-cyan-800/40 transition cursor-pointer"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>{isRunning ? "Executing..." : "Run Program"}</span>
            <span className="text-[10px] opacity-75 font-mono hidden md:inline">⌘↵</span>
          </button>
        </div>
      </header>

      {/* Main Workspace Layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Sidebar: Examples & Presets */}
        <aside className="w-64 border-r border-[#30363d] bg-[#0d1117] flex flex-col hidden lg:flex">
          <div className="p-3 border-b border-[#30363d] text-xs font-semibold text-[#8b949e] uppercase tracking-wider flex items-center justify-between">
            <span>Examples &amp; Features</span>
            <FileCode className="w-3.5 h-3.5 text-zinc-400" />
          </div>
          <div className="flex-1 overflow-y-auto p-2 space-y-1">
            {DEFAULT_EXAMPLES.map((ex) => (
              <button
                key={ex.id}
                onClick={() => handleSelectExample(ex.id)}
                className={`w-full text-left p-2.5 rounded-md transition text-xs flex flex-col space-y-1 cursor-pointer ${
                  selectedExample === ex.id
                    ? "bg-[#1f2937] text-cyan-300 border border-cyan-800/50"
                    : "text-[#c9d1d9] hover:bg-[#161b22] border border-transparent"
                }`}
              >
                <div className="font-semibold flex items-center justify-between">
                  <span>{ex.title}</span>
                  {selectedExample === ex.id && <div className="w-1.5 h-1.5 rounded-full bg-cyan-400" />}
                </div>
                <div className="text-[11px] text-[#8b949e] line-clamp-2">{ex.description}</div>
              </button>
            ))}
          </div>

          <div className="p-3 border-t border-[#30363d] bg-[#161b22]">
            <button
              onClick={handleRunTests}
              disabled={isTesting}
              className="w-full flex items-center justify-center space-x-2 p-2 rounded-md bg-emerald-950/60 hover:bg-emerald-900/60 text-emerald-300 border border-emerald-800/60 text-xs font-semibold transition cursor-pointer"
            >
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>{isTesting ? "Running Tests..." : "Run Test Suite"}</span>
            </button>
          </div>
        </aside>

        {/* Center: Code Editor Area */}
        <section className="flex-1 flex flex-col border-r border-[#30363d] bg-[#0d1117]">
          {/* Editor Header */}
          <div className="flex items-center justify-between px-4 py-2 border-b border-[#30363d] bg-[#161b22]">
            <div className="flex items-center space-x-2">
              <span className="text-xs font-mono text-cyan-400 font-semibold">
                {DEFAULT_EXAMPLES.find((e) => e.id === selectedExample)?.filename || "buffer.yom"}
              </span>
              <span className="text-[10px] text-[#8b949e]">• YOMNOVA 0.1.0</span>
            </div>

            <div className="flex items-center space-x-2">
              <button
                onClick={copyCode}
                className="p-1 rounded hover:bg-[#21262d] text-[#8b949e] hover:text-zinc-200 transition"
                title="Copy code"
              >
                {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
              </button>
            </div>
          </div>

          {/* Code Textarea Editor */}
          <div className="flex-1 relative flex">
            {/* Line numbers gutter */}
            <div className="w-12 bg-[#090d13] text-[#484f58] py-4 text-right pr-3 font-mono text-xs select-none border-r border-[#21262d]">
              {code.split("\n").map((_, i) => (
                <div key={i} className="leading-6">
                  {i + 1}
                </div>
              ))}
            </div>

            <textarea
              ref={textareaRef}
              value={code}
              onChange={(e) => setCode(e.target.value)}
              spellCheck={false}
              className="flex-1 p-4 bg-[#0d1117] text-[#e6edf3] font-mono text-xs leading-6 resize-none outline-none focus:ring-0 focus:outline-none"
            />
          </div>

          {/* Editor Status Bar */}
          <div className="px-4 py-1.5 border-t border-[#30363d] bg-[#161b22] text-[11px] text-[#8b949e] flex items-center justify-between">
            <div className="flex items-center space-x-4">
              <span>Lines: {code.split("\n").length}</span>
              <span>Characters: {code.length}</span>
              <span>Backend: Stack-Based Bytecode VM</span>
            </div>
            <div className="text-cyan-400 font-mono text-[10px]">yomnova v0.1.0-dev (x86_64)</div>
          </div>
        </section>

        {/* Right: Output & Inspection Tabs */}
        <section className="w-1/2 flex flex-col bg-[#0d1117]">
          {/* Tab Navigation */}
          <div className="flex items-center border-b border-[#30363d] bg-[#161b22] px-2 overflow-x-auto">
            <button
              onClick={() => setActiveTab("terminal")}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "terminal"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <TerminalIcon className="w-3.5 h-3.5 text-cyan-400" />
              <span>Execution Terminal</span>
            </button>

            <button
              onClick={() => {
                setActiveTab("disassembler");
                if (!disassembly) fetchDisassembly();
              }}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "disassembler"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <Cpu className="w-3.5 h-3.5 text-indigo-400" />
              <span>Bytecode</span>
            </button>

            <button
              onClick={() => setActiveTab("ownership")}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "ownership"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <ShieldCheck className="w-3.5 h-3.5 text-teal-400" />
              <span>Ownership</span>
            </button>

            <button
              onClick={() => setActiveTab("network")}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "network"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <Network className="w-3.5 h-3.5 text-amber-400" />
              <span>Protocol</span>
            </button>

            <button
              onClick={() => setActiveTab("docs")}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "docs"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <BookOpen className="w-3.5 h-3.5 text-blue-400" />
              <span>Spec &amp; EBNF</span>
            </button>

            <button
              onClick={() => {
                setActiveTab("testSuite");
                if (!testResults) handleRunTests();
              }}
              className={`flex items-center space-x-1.5 px-3 py-2 text-xs font-semibold border-b-2 transition cursor-pointer ${
                activeTab === "testSuite"
                  ? "border-cyan-400 text-white bg-[#0d1117]/50"
                  : "border-transparent text-[#8b949e] hover:text-zinc-200"
              }`}
            >
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
              <span>Tests</span>
            </button>
          </div>

          {/* Tab Content Display */}
          <div className="flex-1 overflow-auto p-4 bg-[#090d13]">
            {activeTab === "terminal" && (
              <div className="flex flex-col h-full font-mono text-xs">
                <div className="text-[11px] text-[#8b949e] pb-2 border-b border-[#21262d] flex items-center justify-between">
                  <span>YOMNOVA VM Execution Output</span>
                  <span className="text-emerald-400">VM Active</span>
                </div>
                <pre className="flex-1 pt-3 whitespace-pre-wrap text-[#e6edf3] font-mono leading-relaxed overflow-auto">
                  {output || "// Ready. Click 'Run Program' (or press Ctrl+Enter) to build and run in the Virtual Machine."}
                </pre>
              </div>
            )}

            {activeTab === "disassembler" && (
              <div className="flex flex-col h-full font-mono text-xs">
                <div className="text-[11px] text-[#8b949e] pb-2 border-b border-[#21262d] flex items-center justify-between">
                  <span>Compiled Bytecode Instructions &amp; Constant Pool</span>
                  <span className="text-indigo-400">YOMNOVA Bytecode v1</span>
                </div>
                <pre className="flex-1 pt-3 whitespace-pre-wrap text-cyan-300 font-mono leading-relaxed overflow-auto">
                  {disassembly || "// Click 'Run Program' or select an example to view the disassembled bytecode."}
                </pre>
              </div>
            )}

            {activeTab === "ownership" && (
              <div className="space-y-4 text-xs">
                <div className="p-3 rounded-md bg-[#161b22] border border-[#30363d]">
                  <div className="font-bold text-sm text-cyan-400 mb-1 flex items-center space-x-2">
                    <ShieldCheck className="w-4 h-4 text-cyan-400" />
                    <span>Ownership &amp; Move Semantics Philosophy</span>
                  </div>
                  <p className="text-[#8b949e] leading-relaxed">
                    YOMNOVA distinguishes between <strong>Copy types</strong> (primitives that clone implicitly) and <strong>Move types</strong> (heap and resource types where ownership transitions). Passing a resource into a function transfers ownership. Subsequent attempts to read that variable are statically rejected by the compiler.
                  </p>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="p-3 rounded-md bg-[#161b22] border border-emerald-900/40">
                    <div className="font-semibold text-emerald-400 mb-2 flex items-center space-x-1.5">
                      <Check className="w-3.5 h-3.5" />
                      <span>Copy Types (Implicit Clone)</span>
                    </div>
                    <ul className="space-y-1.5 text-[#c9d1d9] font-mono text-[11px]">
                      <li>• Int (64-bit integer)</li>
                      <li>• Float (64-bit float)</li>
                      <li>• Bool (boolean true/false)</li>
                      <li>• Unit (empty tuple ())</li>
                    </ul>
                  </div>

                  <div className="p-3 rounded-md bg-[#161b22] border border-amber-900/40">
                    <div className="font-semibold text-amber-400 mb-2 flex items-center space-x-1.5">
                      <Zap className="w-3.5 h-3.5" />
                      <span>Move Types (Single Owner)</span>
                    </div>
                    <ul className="space-y-1.5 text-[#c9d1d9] font-mono text-[11px]">
                      <li>• Text (Heap UTF-8 string)</li>
                      <li>• Array&lt;T&gt; (Vector buffer)</li>
                      <li>• Packet (Network envelope)</li>
                      <li>• TCP &amp; UDP (Active socket)</li>
                    </ul>
                  </div>
                </div>

                <div className="p-3 rounded-md bg-[#161b22] border border-[#30363d]">
                  <div className="font-bold text-xs text-white mb-1">Negative Test Demonstration:</div>
                  <p className="text-[#8b949e] mb-2">
                    Select <strong>"4. Ownership Violation"</strong> from the examples on the left, then click <strong>Run</strong>. The compiler will catch the use-after-move and display precise line numbers and caret diagnostics!
                  </p>
                </div>
              </div>
            )}

            {activeTab === "network" && (
              <div className="space-y-4 text-xs">
                <div className="p-3 rounded-md bg-[#161b22] border border-[#30363d]">
                  <div className="font-bold text-sm text-amber-400 mb-1 flex items-center space-x-2">
                    <Network className="w-4 h-4 text-amber-400" />
                    <span>Protocol-Aware Type System</span>
                  </div>
                  <p className="text-[#8b949e] leading-relaxed">
                    Unlike standard general-purpose languages where network addresses are plain strings or integers, YOMNOVA embeds verified networking types directly into the language syntax and compiler type-checker.
                  </p>
                </div>

                <div className="space-y-2">
                  <div className="p-2.5 rounded bg-[#161b22] border border-[#30363d] flex items-center justify-between font-mono">
                    <span className="text-cyan-400">IPv4("127.0.0.1")</span>
                    <span className="text-[#8b949e]">Validated 4-byte IPv4 Address</span>
                  </div>
                  <div className="p-2.5 rounded bg-[#161b22] border border-[#30363d] flex items-center justify-between font-mono">
                    <span className="text-cyan-400">Port(8080)</span>
                    <span className="text-[#8b949e]">Validated 16-bit Port (1..=65535)</span>
                  </div>
                  <div className="p-2.5 rounded bg-[#161b22] border border-[#30363d] flex items-center justify-between font-mono">
                    <span className="text-cyan-400">Endpoint(ip, port)</span>
                    <span className="text-[#8b949e]">Typed Network Target Envelope</span>
                  </div>
                  <div className="p-2.5 rounded bg-[#161b22] border border-[#30363d] flex items-center justify-between font-mono">
                    <span className="text-cyan-400">NetworkBuffer::with_capacity(1500)</span>
                    <span className="text-[#8b949e]">Bounds-checked MTU payload</span>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "docs" && (
              <div className="space-y-4 text-xs text-[#c9d1d9] leading-relaxed">
                <div className="p-3 rounded-md bg-[#161b22] border border-[#30363d]">
                  <div className="font-bold text-sm text-blue-400 mb-1">YOMNOVA Formal Grammar (EBNF Excerpt)</div>
                  <pre className="font-mono text-[11px] text-[#8b949e] mt-2 whitespace-pre-wrap">
{`Program      ::= ( "module" Identifier )? ( TopLevelItem )* EOF ;
FunctionDecl ::= "fn" Identifier "(" ParamList? ")" ( "->" Type )? Block ;
Statement    ::= LetStmt | MutStmt | AssignStmt | IfStmt | WhileStmt | ReturnStmt | FlowStmt ;
FlowStmt     ::= "flow" Identifier "=" Expr ( "|>" Expr )+ ;
Expr         ::= LogicalOr ( "|>" PipeStage )* ;
Type         ::= "Int" | "Float" | "Bool" | "Text" | "Unit" | "IPv4" | "Port" | "Endpoint" ;`}
                  </pre>
                </div>

                <div className="p-3 rounded-md bg-[#161b22] border border-[#30363d]">
                  <div className="font-bold text-sm text-blue-400 mb-1">Compiler Subsystems</div>
                  <p className="text-[#8b949e]">
                    YOMNOVA is implemented in safe Rust as a modular 14-crate workspace:
                    <br />
                    • <code>yomnova-lexer</code>: Tokenization &amp; line/column tracking
                    <br />
                    • <code>yomnova-parser</code>: Recursive descent &amp; Pratt precedence climbing
                    <br />
                    • <code>yomnova-sema</code>: Lexical scopes, symbol tables &amp; type checking
                    <br />
                    • <code>yomnova-ownership</code>: Ownership tracking &amp; use-after-move rejection
                    <br />
                    • <code>yomnova-ir</code>: Intermediate Representation with 3-Address Code
                    <br />
                    • <code>yomnova-bytecode</code>: Instruction set &amp; chunk constant pool
                    <br />
                    • <code>yomnova-vm</code>: Stack-based virtual machine runtime
                    <br />
                    • <code>yomnova-network-core</code>: Protocol validation &amp; safe socket buffers
                  </p>
                </div>
              </div>
            )}

            {activeTab === "testSuite" && (
              <div className="flex flex-col h-full font-mono text-xs">
                <div className="text-[11px] text-[#8b949e] pb-2 border-b border-[#21262d] flex items-center justify-between">
                  <span>Automated Rust Test Runner (`cargo test --workspace`)</span>
                  <button
                    onClick={handleRunTests}
                    disabled={isTesting}
                    className="text-emerald-400 hover:underline cursor-pointer"
                  >
                    {isTesting ? "Testing..." : "Rerun Tests"}
                  </button>
                </div>
                <pre className="flex-1 pt-3 whitespace-pre-wrap text-emerald-400 font-mono leading-relaxed overflow-auto">
                  {testResults || "// Click 'Run Test Suite' to execute automated tests across all 14 crates."}
                </pre>
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
