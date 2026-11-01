import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type CliResult, type ExampleFile } from "../lib/api";
import { highlight } from "../lib/highlight";

type Tab = "output" | "diagnostics" | "tokens" | "ast" | "bytecode";
const TABS: Array<{ id: Tab; label: string }> = [
  { id: "output", label: "Output" },
  { id: "diagnostics", label: "Diagnostics" },
  { id: "tokens", label: "Tokens" },
  { id: "ast", label: "AST" },
  { id: "bytecode", label: "Bytecode" },
];

const FALLBACK = `module hello

fn main() -> Int {
    emit("Hello from YOMNOVA")
    return 0
}
`;

interface Results {
  run?: CliResult;
  tokens?: CliResult;
  ast?: CliResult;
  bytecode?: CliResult;
}

export function Playground({ version }: { version: string }) {
  const [examples, setExamples] = useState<ExampleFile[]>([]);
  const [exampleId, setExampleId] = useState("hello");
  const [code, setCode] = useState(FALLBACK);
  const [tab, setTab] = useState<Tab>("output");
  const [busy, setBusy] = useState<string | null>(null);
  const [res, setRes] = useState<Results>({});
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const pre = useRef<HTMLPreElement>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  const seq = useRef(0);

  useEffect(() => {
    api.examples().then((list) => {
      setExamples(list);
      const hello = list.find((e) => e.id === "hello") ?? list[0];
      if (hello) {
        setExampleId(hello.id);
        setCode(hello.code);
      }
    });
  }, []);

  const lines = useMemo(() => code.split("\n").length, [code]);
  const pieces = useMemo(() => highlight(code), [code]);

  const syncScroll = () => {
    if (pre.current && area.current) {
      pre.current.scrollTop = area.current.scrollTop;
      pre.current.scrollLeft = area.current.scrollLeft;
    }
  };

  /** Runs the program and, in parallel, fetches the token stream, AST and bytecode listing. */
  const run = useCallback(async () => {
    const id = ++seq.current;
    setBusy("Running…");
    setStatus(null);
    try {
      const [r, t, a, b] = await Promise.all([api.run(code), api.tokens(code), api.ast(code), api.disassemble(code)]);
      if (id !== seq.current) return;
      setRes({ run: r, tokens: t, ast: a, bytecode: b });
      setStatus({ ok: r.ok, text: r.ok ? `Finished in ${r.durationMs} ms` : r.timedOut ? "Timed out" : `Failed (exit ${r.exitCode})` });
      setTab(r.ok ? "output" : "diagnostics");
    } catch {
      setStatus({ ok: false, text: "Could not reach the compiler server" });
    } finally {
      if (id === seq.current) setBusy(null);
    }
  }, [code]);

  const check = useCallback(async () => {
    setBusy("Checking…");
    try {
      const r = await api.check(code);
      setRes((s) => ({ ...s, run: r }));
      setStatus({ ok: r.ok, text: r.ok ? "Check passed: types and ownership are valid" : "Check failed" });
      setTab(r.ok ? "output" : "diagnostics");
    } catch {
      setStatus({ ok: false, text: "Could not reach the compiler server" });
    } finally {
      setBusy(null);
    }
  }, [code]);

  const format = useCallback(async () => {
    setBusy("Formatting…");
    try {
      const r = await api.fmt(code);
      if (r.ok && typeof r.formatted === "string") setCode(r.formatted);
      setStatus({ ok: r.ok, text: r.ok ? "Formatted (re-indent only)" : "Format failed" });
    } finally {
      setBusy(null);
    }
  }, [code]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      void run();
    } else if (e.key === "Tab" && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // Indent with Tab; press Esc first (or Shift+Tab) to move focus out with the keyboard.
      e.preventDefault();
      const el = e.currentTarget;
      const { selectionStart: s, selectionEnd: en } = el;
      setCode(code.slice(0, s) + "    " + code.slice(en));
      requestAnimationFrame(() => el.setSelectionRange(s + 4, s + 4));
    } else if (e.key === "Escape") {
      e.currentTarget.blur();
    }
  };

  const pick = (id: string) => {
    setExampleId(id);
    const ex = examples.find((x) => x.id === id);
    if (ex) {
      setCode(ex.code);
      setRes({});
      setStatus(null);
    }
  };

  const pane = (() => {
    const r = res.run;
    switch (tab) {
      case "output":
        if (!r) return <pre className="pg-pane"><span className="muted">Press Run (Ctrl/⌘ + Enter) to compile and execute your program.</span></pre>;
        return <pre className="pg-pane">{r.stdout || (r.ok ? "(no output)" : "")}</pre>;
      case "diagnostics":
        if (!r) return <pre className="pg-pane"><span className="muted">No diagnostics yet.</span></pre>;
        return <pre className={`pg-pane${r.stderr ? " err" : ""}`}>{r.stderr || "No diagnostics."}</pre>;
      case "tokens":
        return <pre className="pg-pane">{res.tokens ? res.tokens.stdout || res.tokens.stderr : "Run the program to see its tokens."}</pre>;
      case "ast":
        return <pre className="pg-pane">{res.ast ? res.ast.stdout || res.ast.stderr : "Run the program to see its syntax tree."}</pre>;
      case "bytecode":
        return <pre className="pg-pane">{res.bytecode ? res.bytecode.stdout || res.bytecode.stderr : "Run the program to see its bytecode."}</pre>;
    }
  })();

  return (
    <div className="pg">
      <div className="pg-bar" role="toolbar" aria-label="Playground actions">
        <button className="btn primary small" onClick={() => void run()} disabled={busy !== null}>Run</button>
        <button className="btn small" onClick={() => void check()} disabled={busy !== null}>Check</button>
        <button className="btn small" onClick={() => void format()} disabled={busy !== null}>Format</button>
        <label className="mono" style={{ fontSize: 12, color: "var(--ink-faint)" }}>
          <span className="sr-only" style={{ position: "absolute", left: -9999 }}>Example program</span>
          <select aria-label="Example program" value={exampleId} onChange={(e) => pick(e.target.value)}>
            {examples.length === 0 && <option value="hello">hello</option>}
            {examples.map((e) => (
              <option key={e.id} value={e.id}>{e.filename}</option>
            ))}
          </select>
        </label>
        <span className="spacer" />
        <span className="meta">{busy ?? version}</span>
      </div>
      <div className="pg-body">
        <div className="pg-editor">
          <div className="pg-gutter" aria-hidden="true">{Array.from({ length: lines }, (_, i) => i + 1).join("\n")}</div>
          <div className="pg-code">
            <pre ref={pre} aria-hidden="true">
              {pieces.map((p, i) => (p.cls === "ws" || p.cls === "id" ? p.text : <span key={i} className={p.cls}>{p.text}</span>))}
              {"\n"}
            </pre>
            <textarea
              ref={area}
              value={code}
              onChange={(e) => setCode(e.target.value)}
              onScroll={syncScroll}
              onKeyDown={onKeyDown}
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              aria-label="YOMNOVA source code editor. Press Escape to leave the editor."
            />
          </div>
        </div>
        <div className="pg-out">
          <div className="pg-tabs" role="tablist" aria-label="Compiler output">
            {TABS.map((t) => (
              <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>
            ))}
          </div>
          {pane}
          <div className="pg-status" aria-live="polite">
            <span className={status ? (status.ok ? "ok" : "bad") : ""}>{status?.text ?? "Ready"}</span>
            <span>{lines} lines</span>
          </div>
        </div>
      </div>
    </div>
  );
}
