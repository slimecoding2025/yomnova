import { useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState, type Ref } from "react";
import {
  ai,
  AiRequestError,
  applyToCopy,
  lineDiff,
  type AssistantResult,
  type ChatTurn,
  type Diagnostic,
  type ModelInfo,
  type StructuredResponse,
  type Suggestion,
  type ValidationResponse,
} from "../lib/ai";

export interface AssistantHandle {
  explainError(): void;
  suggestFix(): void;
  explainSelection(): void;
}

interface Props {
  /** Current editor text. The assistant only READS it; it never writes to the editor. */
  code: string;
  selection: string;
  onClose(): void;
  ref?: Ref<AssistantHandle>;
}

type Item =
  | { id: number; kind: "user"; text: string; ctx: string[] }
  | { id: number; kind: "text"; text: string; streaming: boolean; sources: string[]; model: string; diagnostics: Diagnostic[] }
  | { id: number; kind: "structured"; data: StructuredResponse }
  | { id: number; kind: "note"; text: string; tone: "info" | "error" };

type Mode = "chat" | "tutor" | "debug";
const MODEL_KEY = "yomnova.ai.model";

function readSaved(): string {
  try {
    return localStorage.getItem(MODEL_KEY) ?? "";
  } catch {
    return "";
  }
}
function writeSaved(v: string) {
  try {
    localStorage.setItem(MODEL_KEY, v);
  } catch {
    /* storage unavailable: preference simply is not remembered */
  }
}

/** Renders untrusted model text as plain React text. Fenced blocks become <pre>; nothing is interpreted as HTML. */
function Rich({ text }: { text: string }) {
  const parts = text.split(/```[^\n]*\n?([\s\S]*?)```/g);
  return (
    <>
      {parts.map((p, i) =>
        i % 2 === 1 ? (
          <pre key={i} className="ai-code"><code>{p.replace(/\n$/, "")}</code></pre>
        ) : p.trim() ? (
          <p key={i} className="ai-p">
            {p.split(/`([^`\n]+)`/g).map((q, j) => (j % 2 === 1 ? <code key={j}>{q}</code> : q))}
          </p>
        ) : null,
      )}
    </>
  );
}

function Diff({ before, after }: { before: string; after: string }) {
  const rows = useMemo(() => lineDiff(before, after), [before, after]);
  return (
    <pre className="ai-code ai-diff" aria-label="Diff of the proposed change">
      {rows.map((r, i) => (
        <span key={i} className={r.op === "+" ? "add" : r.op === "-" ? "del" : "ctx"}>{`${r.op} ${r.text}\n`}</span>
      ))}
    </pre>
  );
}

function SuggestionCard({ s, editorCode }: { s: Suggestion; editorCode: string }) {
  const [view, setView] = useState<"none" | "code" | "diff">("none");
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [val, setVal] = useState<ValidationResponse | { error: string } | null>(null);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(s.after);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setView("code"); // clipboard blocked: show the text so it can be selected manually
    }
  };

  const validate = async () => {
    const cand = applyToCopy(editorCode, s.before, s.after);
    if (!cand.ok) return setVal({ error: cand.reason });
    setBusy(true);
    setVal(null);
    try {
      setVal(await ai.validate(cand.code));
    } catch (e) {
      setVal({ error: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="ai-sug">
      <div className="ai-sug-head">
        <strong>{s.title}</strong>
        <span className="ai-badge" title="The model's own rating; it is not a calibrated probability.">model confidence: {s.confidence}</span>
      </div>
      <p className="ai-p">{s.description}</p>
      <div className="ai-actions">
        <button className="btn small" onClick={() => setView(view === "code" ? "none" : "code")} aria-expanded={view === "code"}>View suggestion</button>
        <button className="btn small" onClick={() => setView(view === "diff" ? "none" : "diff")} aria-expanded={view === "diff"}>Review diff</button>
        <button className="btn small" onClick={() => void copy()}>{copied ? "Copied" : "Copy"}</button>
        <button className="btn small" onClick={() => void validate()} disabled={busy}>{busy ? "Checking…" : "Validate with compiler"}</button>
      </div>
      {view === "code" && <pre className="ai-code"><code>{s.after}</code></pre>}
      {view === "diff" && <Diff before={s.before} after={s.after} />}
      <p className="ai-status" role="status">
        {val === null && <span className="warn">Not compiler-validated. Your editor has not been changed.</span>}
        {val && "error" in val && <span className="bad">Cannot validate: {val.error}</span>}
        {val && "validated" in val && val.validated && <span className="ok">Compiler check passed on a copy of your code (parse, types, ownership; not executed). Your editor is unchanged.</span>}
        {val && "validated" in val && !val.validated && <span className="bad">Compiler check failed on a copy of your code. Your editor is unchanged.</span>}
      </p>
      {val && "validated" in val && !val.validated && <pre className="ai-code ai-err">{val.output}</pre>}
    </div>
  );
}

function Structured({ data, editorCode }: { data: StructuredResponse; editorCode: string }) {
  const r: AssistantResult = data.result;
  const first = data.diagnostics[0];
  return (
    <div className="ai-msg ai-a">
      <p className="ai-who">{data.model} · AI interpretation</p>
      {first && (
        <pre className="ai-code ai-diag" aria-label="Compiler diagnostic (compiler-confirmed)">
          <span className="ai-tag">compiler output</span>{"\n"}{first.raw}
        </pre>
      )}
      <p className="ai-p"><strong>{r.summary}</strong></p>
      <Rich text={r.explanation} />
      {r.languageRule && (
        <p className="ai-p">Language rule: <strong>{r.languageRule.title}</strong>{r.languageRule.source ? <> — <code>{r.languageRule.source}</code></> : <> — <em>no documentation source supplied</em></>}</p>
      )}
      {r.suggestions.map((s, i) => <SuggestionCard key={i} s={s} editorCode={editorCode} />)}
      {r.limitations.length > 0 && (
        <ul className="ai-lim">{r.limitations.map((l, i) => <li key={i}>{l}</li>)}</ul>
      )}
      {r.references.length > 0 && <p className="ai-src">Sources: {r.references.map((x) => <code key={x}>{x}</code>)}</p>}
      <p className="ai-src">{data.disclaimer}</p>
    </div>
  );
}

export function Assistant({ code, selection, onClose, ref }: Props) {
  const [status, setStatus] = useState<"loading" | "ready" | "unconfigured" | "error">("loading");
  const [statusMsg, setStatusMsg] = useState("");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [model, setModel] = useState("");
  const [filter, setFilter] = useState("");
  const [items, setItems] = useState<Item[]>([]);
  const [input, setInput] = useState("");
  const [mode, setMode] = useState<Mode>("chat");
  const [attachCode, setAttachCode] = useState(false);
  const [busy, setBusy] = useState(false);
  const idRef = useRef(0);
  const ctl = useRef<AbortController | null>(null);
  const log = useRef<HTMLDivElement>(null);
  const nextId = () => ++idRef.current;

  const loadModels = useCallback(async () => {
    setStatus("loading");
    try {
      const r = await ai.models();
      if (!r.configured) {
        setStatus("unconfigured");
        setStatusMsg(r.message ?? "The AI Assistant is not configured on this server.");
        return;
      }
      setModels(r.models);
      const saved = readSaved();
      const wanted = saved || r.defaultModel;
      if (wanted && r.models.some((m) => m.id === wanted)) setModel(wanted);
      else if (saved) {
        setModel("");
        setItems((x) => [...x, { id: nextId(), kind: "note", tone: "error", text: `Your saved model "${saved}" is no longer available. Choose another model; none was selected for you.` }]);
      }
      setStatus("ready");
    } catch (e) {
      setStatus("error");
      setStatusMsg((e as Error).message);
    }
  }, []);

  useEffect(() => {
    void loadModels();
  }, [loadModels]);

  useEffect(() => {
    log.current?.scrollTo({ top: log.current.scrollHeight });
  }, [items]);

  const grouped = useMemo(() => {
    const f = filter.trim().toLowerCase();
    const list = f ? models.filter((m) => `${m.id} ${m.name}`.toLowerCase().includes(f)) : models;
    const by = new Map<string, ModelInfo[]>();
    for (const m of list) by.set(m.provider, [...(by.get(m.provider) ?? []), m]);
    return [...by.entries()];
  }, [models, filter]);
  const current = models.find((m) => m.id === model);

  const pickModel = (id: string) => {
    setModel(id);
    writeSaved(id);
    if (id) setItems((x) => [...x, { id: nextId(), kind: "note", tone: "info", text: `Model changed to ${id}. The conversation is kept.` }]);
  };

  const history = (): ChatTurn[] =>
    items
      .flatMap((i): ChatTurn[] =>
        i.kind === "user" ? [{ role: "user", content: i.text }] : i.kind === "text" && !i.streaming && i.text ? [{ role: "assistant", content: i.text }] : i.kind === "structured" ? [{ role: "assistant", content: `${i.data.result.summary}\n${i.data.result.explanation}` }] : [],
      )
      .slice(-8);

  const fail = (e: unknown) => {
    const err = e as AiRequestError;
    if (err.code === "aborted") return setItems((x) => [...x, { id: nextId(), kind: "note", tone: "info", text: "Cancelled." }]);
    if (err.code === "invalid_model") setModel("");
    setItems((x) => [...x, { id: nextId(), kind: "note", tone: "error", text: err.message || "Request failed." }]);
  };

  const guard = (): boolean => {
    if (busy) return false;
    if (!model) {
      setItems((x) => [...x, { id: nextId(), kind: "note", tone: "error", text: "Choose a model first." }]);
      return false;
    }
    return true;
  };

  const structured = async (kind: "explain" | "suggest", userText: string, ctx: string[], question?: string) => {
    if (!guard()) return;
    const hist = history();
    setItems((x) => [...x, { id: nextId(), kind: "user", text: userText, ctx }]);
    setBusy(true);
    ctl.current = new AbortController();
    try {
      const data =
        kind === "explain"
          ? await ai.explain({ model, code, messages: hist }, ctl.current.signal)
          : await ai.suggest({ model, code, question, selection: selection || undefined, messages: hist }, ctl.current.signal);
      setItems((x) => [...x, { id: nextId(), kind: "structured", data }]);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const stream = async (m: Mode | "explain_code", text: string, ctx: string[], sendCode: boolean, sendSel: boolean) => {
    if (!guard()) return;
    const hist = history();
    const id = nextId();
    setItems((x) => [
      ...x,
      { id: nextId(), kind: "user", text, ctx },
      { id, kind: "text", text: "", streaming: true, sources: [], model, diagnostics: [] },
    ]);
    setBusy(true);
    ctl.current = new AbortController();
    const patch = (fn: (i: Extract<Item, { kind: "text" }>) => Extract<Item, { kind: "text" }>) =>
      setItems((x) => x.map((i) => (i.id === id && i.kind === "text" ? fn(i) : i)));
    try {
      await ai.chat(
        { model, mode: m, messages: [...hist, { role: "user", content: text }], code: sendCode ? code : undefined, selection: sendSel ? selection : undefined },
        { onMeta: (meta) => patch((i) => ({ ...i, sources: meta.sources, diagnostics: meta.diagnostics })), onDelta: (t) => patch((i) => ({ ...i, text: i.text + t })) },
        ctl.current.signal,
      );
    } catch (e) {
      fail(e);
    } finally {
      patch((i) => ({ ...i, streaming: false }));
      setBusy(false);
    }
  };

  const send = () => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    const sendCode = attachCode || mode === "debug";
    const ctx = [sendCode && "editor code", selection && "selected code"].filter(Boolean) as string[];
    void stream(mode, text, ctx, sendCode, Boolean(selection));
  };

  useImperativeHandle(ref, () => ({
    explainError: () => void structured("explain", "Explain the compiler error", ["compiler diagnostic", "editor code"]),
    suggestFix: () => void structured("suggest", "Suggest a fix", ["compiler diagnostic", "editor code"].concat(selection ? ["selected code"] : []), input.trim() || undefined),
    explainSelection: () => {
      if (!selection) return setItems((x) => [...x, { id: nextId(), kind: "note", tone: "error", text: "Select some code in the editor first." }]);
      void stream("explain_code", "Explain the selected code", ["selected code"], false, true);
    },
  }));

  return (
    <aside className="pg-ai" aria-label="YOMNOVA AI Assistant">
      <div className="ai-head">
        <strong>YOMNOVA AI Assistant</strong>
        <span className="spacer" />
        <button className="btn small" onClick={() => { ctl.current?.abort(); setItems([]); }} title="Start a new conversation">New</button>
        <button className="btn small" onClick={onClose} aria-label="Collapse the AI Assistant panel">Close</button>
      </div>

      {status === "loading" && <p className="ai-note" role="status">Loading models…</p>}
      {status === "unconfigured" && (
        <div className="ai-note" role="status">
          <p className="ai-p"><strong>AI Assistant is off.</strong> {statusMsg}</p>
          <p className="ai-p">Add <code>OPENROUTER_API_KEY</code> to the server environment and restart. See <code>docs/AI_ASSISTANT.md</code>.</p>
        </div>
      )}
      {status === "error" && (
        <div className="ai-note bad" role="alert">
          <p className="ai-p">Could not load models: {statusMsg}</p>
          <button className="btn small" onClick={() => void loadModels()}>Retry</button>
        </div>
      )}

      {status === "ready" && (
        <>
          <div className="ai-model">
            <label htmlFor="ai-model-select" className="ai-lbl">Model</label>
            {models.length > 20 && (
              <input type="search" aria-label="Filter models" placeholder="Filter models…" value={filter} onChange={(e) => setFilter(e.target.value)} />
            )}
            <select id="ai-model-select" value={model} onChange={(e) => pickModel(e.target.value)}>
              <option value="">Select a model…</option>
              {grouped.map(([prov, list]) => (
                <optgroup key={prov} label={prov}>
                  {list.map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
                </optgroup>
              ))}
            </select>
            {current && (
              <p className="ai-meta">
                {current.provider}{current.contextLength ? ` · ${Math.round(current.contextLength / 1000)}k context` : ""}
                {current.structuredOutputs ? " · structured output" : current.jsonMode ? " · JSON mode" : " · parsed JSON fallback"}
              </p>
            )}
          </div>

          <div className="ai-log" ref={log} role="log" aria-live="polite" aria-label="Conversation">
            {items.length === 0 && (
              <p className="ai-empty">
                Ask about YOMNOVA syntax, or use <em>Explain with YOMNOVA AI</em> on a compiler error. Answers use this project's documentation and the real compiler's diagnostics. The assistant only suggests; it never edits your code.
              </p>
            )}
            {items.map((i) => {
              if (i.kind === "user") return (
                <div key={i.id} className="ai-msg ai-u">
                  <p className="ai-p">{i.text}</p>
                  {i.ctx.length > 0 && <p className="ai-src">sent: {i.ctx.join(", ")}</p>}
                </div>
              );
              if (i.kind === "note") return <p key={i.id} className={`ai-note ${i.tone === "error" ? "bad" : ""}`} role={i.tone === "error" ? "alert" : undefined}>{i.text}</p>;
              if (i.kind === "structured") return <Structured key={i.id} data={i.data} editorCode={code} />;
              return (
                <div key={i.id} className="ai-msg ai-a">
                  <p className="ai-who">{i.model}{i.streaming ? " · writing…" : ""}</p>
                  {i.diagnostics[0] && <pre className="ai-code ai-diag"><span className="ai-tag">compiler output</span>{"\n"}{i.diagnostics[0].raw}</pre>}
                  <Rich text={i.text} />
                  {!i.streaming && i.text && i.sources.length > 0 && <p className="ai-src">Sources consulted: {i.sources.map((x) => <code key={x}>{x}</code>)}</p>}
                </div>
              );
            })}
          </div>

          <div className="ai-input">
            <div className="ai-ctx" aria-label="Context that will be sent">
              <span className="ai-chip">{selection ? `selection attached (${selection.length} chars)` : "no selection"}</span>
              <label className="ai-chip">
                <input type="checkbox" checked={attachCode || mode === "debug"} disabled={mode === "debug"} onChange={(e) => setAttachCode(e.target.checked)} /> editor code
              </label>
              <select aria-label="Assistant mode" value={mode} onChange={(e) => setMode(e.target.value as Mode)}>
                <option value="chat">Ask</option>
                <option value="tutor">Syntax tutor</option>
                <option value="debug">Debug (sends code + compiler output)</option>
              </select>
            </div>
            <textarea
              aria-label="Message to the AI Assistant"
              placeholder="Ask about YOMNOVA… (Enter to send, Shift+Enter for a new line)"
              value={input}
              rows={2}
              maxLength={2000}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); } }}
            />
            <div className="ai-actions">
              {busy ? (
                <button className="btn small" onClick={() => ctl.current?.abort()}>Cancel</button>
              ) : (
                <button className="btn primary small" onClick={send} disabled={!input.trim()}>Send</button>
              )}
              <span className="ai-src">Your prompt, selected code and compiler output are sent to the selected model provider via OpenRouter.</span>
            </div>
          </div>
        </>
      )}
    </aside>
  );
}
