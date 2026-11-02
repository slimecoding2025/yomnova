/** Browser client for the YOMNOVA AI Assistant API. No secrets live here: the OpenRouter key stays on the server. */
export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  description: string;
  contextLength: number | null;
  structuredOutputs: boolean;
  jsonMode: boolean;
}

export interface Diagnostic {
  id: string;
  severity: "error" | "warning" | "info";
  message: string;
  file: string;
  line: number | null;
  column: number | null;
  hint: string | null;
  excerpt: string;
  stage: string | null;
  raw: string;
}

export interface Suggestion {
  title: string;
  description: string;
  before: string;
  after: string;
  confidence: "low" | "medium" | "high";
  compilerValidated: false;
}

export interface AssistantResult {
  type: "error_explanation" | "fix_suggestions" | "answer";
  summary: string;
  explanation: string;
  languageRule: { title: string; source: string } | null;
  suggestions: Suggestion[];
  limitations: string[];
  references: string[];
}

export interface StructuredResponse {
  result: AssistantResult;
  model: string;
  sources: Array<{ id: string; title: string }>;
  diagnostics: Diagnostic[];
  disclaimer: string;
}

export interface ValidationResponse {
  validated: boolean;
  diagnostics: Diagnostic[];
  output: string;
  note: string;
}

export interface ModelsResponse {
  configured: boolean;
  defaultModel: string;
  models: ModelInfo[];
  message?: string;
}

export interface ChatTurn {
  role: "user" | "assistant";
  content: string;
}

export class AiRequestError extends Error {
  constructor(
    message: string,
    public code: string,
    public status: number,
  ) {
    super(message);
  }
}

async function post<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body), signal });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw new AiRequestError("Request cancelled", "aborted", 499);
    throw new AiRequestError("Could not reach the server", "network", 0);
  }
  const data = (await res.json().catch(() => ({}))) as { error?: string; code?: string };
  if (!res.ok) throw new AiRequestError(data.error ?? `Request failed (${res.status})`, data.code ?? "error", res.status);
  return data as T;
}

export interface ChatHandlers {
  onMeta?: (m: { model: string; sources: string[]; diagnostics: Diagnostic[] }) => void;
  onDelta: (text: string) => void;
}

export const ai = {
  async models(): Promise<ModelsResponse> {
    const res = await fetch("/api/ai/models");
    const data = (await res.json().catch(() => ({}))) as Partial<ModelsResponse> & { error?: string };
    if (!res.ok) throw new AiRequestError(data.error ?? `Could not load models (${res.status})`, "models", res.status);
    return { configured: Boolean(data.configured), defaultModel: data.defaultModel ?? "", models: data.models ?? [], message: data.message };
  },
  explain: (body: { model: string; code: string; diagnosticIndex?: number; messages?: ChatTurn[] }, signal?: AbortSignal) =>
    post<StructuredResponse>("/api/ai/explain-diagnostic", body, signal),
  suggest: (body: { model: string; code: string; question?: string; selection?: string; messages?: ChatTurn[] }, signal?: AbortSignal) =>
    post<StructuredResponse>("/api/ai/suggest-fix", body, signal),
  validate: (code: string, signal?: AbortSignal) => post<ValidationResponse>("/api/ai/validate-suggestion", { code }, signal),

  /** Streams a chat answer over server-sent events. Resolves when the stream ends; throws AiRequestError on failure. */
  async chat(
    body: { model: string; mode: "chat" | "tutor" | "explain_code" | "debug"; messages: ChatTurn[]; code?: string; selection?: string },
    h: ChatHandlers,
    signal?: AbortSignal,
  ): Promise<void> {
    let res: Response;
    try {
      res = await fetch("/api/ai/chat", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body), signal });
    } catch (e) {
      if ((e as Error).name === "AbortError") throw new AiRequestError("Request cancelled", "aborted", 499);
      throw new AiRequestError("Could not reach the server", "network", 0);
    }
    if (!res.ok || !res.body) {
      const data = (await res.json().catch(() => ({}))) as { error?: string; code?: string };
      throw new AiRequestError(data.error ?? `Request failed (${res.status})`, data.code ?? "error", res.status);
    }
    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    const dispatch = (block: string) => {
      let event = "message";
      let data = "";
      for (const line of block.split("\n")) {
        if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("data:")) data += line.slice(5).trim();
      }
      if (!data) return;
      const j = JSON.parse(data) as Record<string, unknown>;
      if (event === "meta") h.onMeta?.(j as unknown as Parameters<NonNullable<ChatHandlers["onMeta"]>>[0]);
      else if (event === "delta") h.onDelta(String(j.text ?? ""));
      else if (event === "error") throw new AiRequestError(String(j.error ?? "The model request failed"), String(j.code ?? "error"), 502);
    };
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += dec.decode(value, { stream: true });
        let i: number;
        while ((i = buf.indexOf("\n\n")) >= 0) {
          dispatch(buf.slice(0, i));
          buf = buf.slice(i + 2);
        }
      }
    } catch (e) {
      if (e instanceof AiRequestError) throw e;
      if ((e as Error).name === "AbortError") throw new AiRequestError("Request cancelled", "aborted", 499);
      throw new AiRequestError("The connection was interrupted", "network", 0);
    }
  },
};

/** Line diff (LCS) for small snippets: lines prefixed '-', '+' or ' '. */
export function lineDiff(before: string, after: string): Array<{ op: "-" | "+" | " "; text: string }> {
  const a = before.split("\n");
  const b = after.split("\n");
  if (a.length * b.length > 40_000) return [...a.map((t) => ({ op: "-" as const, text: t })), ...b.map((t) => ({ op: "+" as const, text: t }))];
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => new Array<number>(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) for (let j = b.length - 1; j >= 0; j--) dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out: Array<{ op: "-" | "+" | " "; text: string }> = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ op: " ", text: a[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) out.push({ op: "-", text: a[i++] });
    else out.push({ op: "+", text: b[j++] });
  }
  while (i < a.length) out.push({ op: "-", text: a[i++] });
  while (j < b.length) out.push({ op: "+", text: b[j++] });
  return out;
}

/** Replaces `before` with `after` in `code` only when `before` occurs exactly once. */
export function applyToCopy(code: string, before: string, after: string): { ok: true; code: string } | { ok: false; reason: string } {
  if (!before.trim()) return { ok: false, reason: "This suggestion has no 'before' text, so it cannot be located in your code." };
  const n = code.split(before).length - 1;
  if (n === 0) return { ok: false, reason: "The suggestion's 'before' text was not found in the editor (the code may have changed)." };
  if (n > 1) return { ok: false, reason: `The suggestion's 'before' text appears ${n} times, so the change location is ambiguous.` };
  return { ok: true, code: code.replace(before, () => after) };
}
