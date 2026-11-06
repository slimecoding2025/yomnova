/**
 * Minimal OpenRouter client (https://openrouter.ai/docs). Server-side only: the API key never leaves this process.
 * `fetch` is injectable so tests can mock the provider; no test needs a paid call.
 */
export type ErrorCode =
  | "not_configured"
  | "invalid_model"
  | "provider_auth"
  | "provider_credits"
  | "rate_limited"
  | "timeout"
  | "network"
  | "provider_error"
  | "bad_response"
  | "aborted";

export class AiError extends Error {
  constructor(
    public code: ErrorCode,
    message: string,
    public status = 502,
  ) {
    super(message);
  }
}

export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  description: string;
  contextLength: number | null;
  /** Provider advertises JSON-schema structured outputs. */
  structuredOutputs: boolean;
  /** Provider advertises plain JSON mode (response_format). */
  jsonMode: boolean;
  maxCompletionTokens: number | null;
}

export interface AiConfig {
  apiKey: string;
  baseUrl: string;
  timeoutMs: number;
  maxOutputTokens: number;
  allowlist: string[];
  /** Free models the server picks from, in order. Visitors never choose or see them. */
  pool: string[];
  defaultModel: string;
  appTitle: string;
  siteUrl: string;
  modelCacheMs: number;
  fetchImpl: typeof fetch;
}

/** Free OpenRouter chat models (listed as $0 on 2026-11-05). Override with AI_FREE_MODELS (comma-separated). */
export const DEFAULT_FREE_POOL = ["google/gemma-4-26b-a4b-it:free", "cohere/north-mini-code:free", "nvidia/nemotron-3-ultra-550b-a55b:free"];

export function configFromEnv(env: NodeJS.ProcessEnv = process.env, fetchImpl: typeof fetch = fetch): AiConfig {
  const n = (v: string | undefined, d: number, lo: number, hi: number) => {
    const x = Number.parseInt(v ?? "", 10);
    return Number.isFinite(x) ? Math.min(hi, Math.max(lo, x)) : d;
  };
  const list = (v: string | undefined) => (v ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  const pool = env.AI_FREE_MODELS !== undefined ? list(env.AI_FREE_MODELS).slice(0, 6) : DEFAULT_FREE_POOL;
  return {
    apiKey: (env.OPENROUTER_API_KEY ?? "").trim(),
    baseUrl: (env.OPENROUTER_BASE_URL ?? "https://openrouter.ai/api/v1").replace(/\/+$/, ""),
    timeoutMs: n(env.AI_TIMEOUT_MS, 60_000, 5_000, 180_000),
    maxOutputTokens: n(env.AI_MAX_OUTPUT_TOKENS, 1200, 200, 4000),
    pool,
    allowlist: env.AI_MODEL_ALLOWLIST !== undefined ? list(env.AI_MODEL_ALLOWLIST) : pool,
    defaultModel: (env.OPENROUTER_DEFAULT_MODEL ?? "").trim(),
    appTitle: env.OPENROUTER_APP_TITLE ?? "YOMNOVA Playground",
    siteUrl: env.OPENROUTER_SITE_URL ?? "",
    modelCacheMs: n(env.AI_MODEL_CACHE_MS, 10 * 60_000, 1_000, 24 * 3_600_000),
    fetchImpl,
  };
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface Usage {
  promptTokens: number | null;
  completionTokens: number | null;
}

interface RawModel {
  id?: unknown;
  name?: unknown;
  description?: unknown;
  context_length?: unknown;
  supported_parameters?: unknown;
  architecture?: { input_modalities?: unknown; output_modalities?: unknown } | null;
  top_provider?: { max_completion_tokens?: unknown } | null;
}

export function mapModel(m: RawModel): ModelInfo | null {
  if (typeof m.id !== "string" || !m.id) return null;
  const arr = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
  const out = arr(m.architecture?.output_modalities);
  const inp = arr(m.architecture?.input_modalities);
  if (out.length && !out.includes("text")) return null;
  if (inp.length && !inp.includes("text")) return null;
  const params = arr(m.supported_parameters);
  return {
    id: m.id,
    name: typeof m.name === "string" && m.name ? m.name : m.id,
    provider: m.id.includes("/") ? m.id.split("/")[0] : "unknown",
    description: typeof m.description === "string" ? m.description.slice(0, 300) : "",
    contextLength: typeof m.context_length === "number" ? m.context_length : null,
    structuredOutputs: params.includes("structured_outputs"),
    jsonMode: params.includes("response_format"),
    maxCompletionTokens: typeof m.top_provider?.max_completion_tokens === "number" ? m.top_provider.max_completion_tokens : null,
  };
}

export class OpenRouter {
  private cache: { at: number; models: ModelInfo[] } | null = null;
  constructor(public cfg: AiConfig) {}

  get configured(): boolean {
    return this.cfg.apiKey.length > 0;
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = { Authorization: `Bearer ${this.cfg.apiKey}`, "Content-Type": "application/json" };
    if (this.cfg.siteUrl) h["HTTP-Referer"] = this.cfg.siteUrl;
    if (this.cfg.appTitle) h["X-Title"] = this.cfg.appTitle;
    return h;
  }

  private require() {
    if (!this.configured) throw new AiError("not_configured", "The AI Assistant is not configured: set OPENROUTER_API_KEY on the server.", 503);
  }

  private async call(path: string, init: RequestInit, signal?: AbortSignal): Promise<Response> {
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(new Error("timeout")), this.cfg.timeoutMs);
    const onAbort = () => ctl.abort(new Error("aborted"));
    signal?.addEventListener("abort", onAbort, { once: true });
    try {
      return await this.cfg.fetchImpl(`${this.cfg.baseUrl}${path}`, { ...init, signal: ctl.signal });
    } catch (e) {
      const why = (s?: AbortSignal) => String((s?.reason as Error | undefined)?.message ?? "");
      if (why(ctl.signal) === "timeout" || why(signal) === "timeout") throw this.timeoutError();
      if (signal?.aborted || why(ctl.signal) === "aborted") throw new AiError("aborted", "Request cancelled", 499);
      throw new AiError("network", "Could not reach the model provider", 502);
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
    }
  }

  private timeoutError() {
    return new AiError("timeout", `The model provider did not respond within ${this.cfg.timeoutMs / 1000}s`, 504);
  }

  /** Maps a non-2xx provider response to an AiError without ever echoing request headers or the key. */
  private async failure(res: Response): Promise<AiError> {
    let msg = "";
    try {
      const j = (await res.json()) as { error?: { message?: unknown } };
      if (typeof j?.error?.message === "string") msg = j.error.message.slice(0, 300);
    } catch {
      /* body was not JSON */
    }
    if (res.status === 401 || res.status === 403) return new AiError("provider_auth", "OpenRouter rejected the server's API key.", 502);
    if (res.status === 402) return new AiError("provider_credits", "The OpenRouter account has insufficient credits.", 502);
    if (res.status === 429) return new AiError("rate_limited", "The model provider is rate limiting requests. Try again shortly.", 429);
    if (res.status === 404 || (res.status === 400 && /model/i.test(msg))) return new AiError("invalid_model", msg || "The selected model is not available.", 400);
    if (res.status === 408) return new AiError("timeout", "The model provider timed out.", 504);
    return new AiError("provider_error", `The model provider returned an error (${res.status})${msg ? `: ${msg}` : ""}`, 502);
  }

  async listModels(force = false): Promise<ModelInfo[]> {
    if (!force && this.cache && Date.now() - this.cache.at < this.cfg.modelCacheMs) return this.cache.models;
    const res = await this.call("/models", { method: "GET", headers: this.configured ? this.headers() : { "Content-Type": "application/json" } });
    if (!res.ok) throw await this.failure(res);
    const body = (await res.json().catch(() => null)) as { data?: unknown } | null;
    if (!body || !Array.isArray(body.data)) throw new AiError("bad_response", "The model list from the provider was malformed.");
    let models = (body.data as RawModel[]).map(mapModel).filter((m): m is ModelInfo => m !== null);
    const allow = this.cfg.allowlist;
    if (allow.length) models = models.filter((m) => allow.some((a) => m.id === a || m.id.startsWith(a)));
    models.sort((a, b) => a.provider.localeCompare(b.provider) || a.name.localeCompare(b.name));
    this.cache = { at: Date.now(), models: models.slice(0, 500) };
    return this.cache.models;
  }

  /** The pool models that the provider really lists right now, in pool order. Empty when none is available. */
  async pool(): Promise<ModelInfo[]> {
    const live = await this.listModels();
    return this.cfg.pool.map((id) => live.find((m) => m.id === id)).filter((m): m is ModelInfo => Boolean(m));
  }

  /** Looks the id up in the provider's real model list. Never falls back to an unlisted id. */
  async requireModel(id: string): Promise<ModelInfo> {
    const found = (await this.listModels()).find((m) => m.id === id) ?? (await this.listModels(true)).find((m) => m.id === id);
    if (!found) throw new AiError("invalid_model", `Model '${id.slice(0, 80)}' is not available. Choose another model.`, 400);
    return found;
  }

  /** Builds the request body. Exposed for tests. */
  buildBody(p: { model: ModelInfo; messages: ChatMessage[]; maxTokens?: number; structured?: object | null; stream: boolean }) {
    const body: Record<string, unknown> = {
      model: p.model.id,
      messages: p.messages,
      max_tokens: Math.min(p.maxTokens ?? this.cfg.maxOutputTokens, this.cfg.maxOutputTokens, p.model.maxCompletionTokens ?? Infinity),
      temperature: 0.2,
      stream: p.stream,
    };
    if (p.structured) {
      if (p.model.structuredOutputs) body.response_format = { type: "json_schema", json_schema: p.structured };
      else if (p.model.jsonMode) body.response_format = { type: "json_object" };
    }
    return body;
  }

  async complete(p: { model: ModelInfo; messages: ChatMessage[]; structured?: object | null; signal?: AbortSignal }): Promise<{ text: string; usage: Usage }> {
    this.require();
    const res = await this.call("/chat/completions", { method: "POST", headers: this.headers(), body: JSON.stringify(this.buildBody({ ...p, stream: false })) }, p.signal);
    if (!res.ok) throw await this.failure(res);
    const j = (await res.json().catch(() => null)) as {
      error?: { message?: string };
      choices?: Array<{ message?: { content?: unknown }; error?: { message?: string }; finish_reason?: string }>;
      usage?: { prompt_tokens?: number; completion_tokens?: number };
    } | null;
    if (!j) throw new AiError("bad_response", "The provider returned an unreadable response.");
    if (j.error) throw new AiError("provider_error", `The model provider reported an error: ${String(j.error.message ?? "unknown").slice(0, 200)}`);
    const choice = j.choices?.[0];
    if (choice?.error) throw new AiError("provider_error", `The model provider reported an error: ${String(choice.error.message ?? "unknown").slice(0, 200)}`);
    const text = choice?.message?.content;
    if (typeof text !== "string" || !text.trim()) throw new AiError("bad_response", "The model returned an empty response.");
    return { text, usage: { promptTokens: j.usage?.prompt_tokens ?? null, completionTokens: j.usage?.completion_tokens ?? null } };
  }

  /** Streams server-sent events from the provider, calling `onDelta` for each text fragment. */
  async stream(p: { model: ModelInfo; messages: ChatMessage[]; signal?: AbortSignal }, onDelta: (t: string) => void): Promise<{ finishReason: string | null; usage: Usage }> {
    this.require();
    // One deadline covers the whole stream, not just the response headers.
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(new Error("timeout")), this.cfg.timeoutMs);
    const relay = () => ctl.abort(new Error("aborted"));
    p.signal?.addEventListener("abort", relay, { once: true });
    try {
      return await this.readStream(p, ctl, onDelta);
    } finally {
      clearTimeout(timer);
      p.signal?.removeEventListener("abort", relay);
    }
  }

  private async readStream(p: { model: ModelInfo; messages: ChatMessage[]; signal?: AbortSignal }, ctl: AbortController, onDelta: (t: string) => void): Promise<{ finishReason: string | null; usage: Usage }> {
    const res = await this.call("/chat/completions", { method: "POST", headers: this.headers(), body: JSON.stringify(this.buildBody({ ...p, stream: true })) }, ctl.signal);
    if (!res.ok) throw await this.failure(res);
    if (!res.body) throw new AiError("bad_response", "The provider returned no stream.");
    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    let finish: string | null = null;
    let usage: Usage = { promptTokens: null, completionTokens: null };
    let got = false;
    const handle = (line: string) => {
      if (!line.startsWith("data:")) return; // comments (": OPENROUTER PROCESSING") and blanks
      const data = line.slice(5).trim();
      if (!data || data === "[DONE]") return;
      let j: { error?: { message?: string }; choices?: Array<{ delta?: { content?: unknown }; finish_reason?: string | null; error?: { message?: string } }>; usage?: { prompt_tokens?: number; completion_tokens?: number } };
      try {
        j = JSON.parse(data);
      } catch {
        throw new AiError("bad_response", "The provider sent a malformed stream event.");
      }
      if (j.error) throw new AiError("provider_error", `The model provider reported an error: ${String(j.error.message ?? "unknown").slice(0, 200)}`);
      const c = j.choices?.[0];
      if (c?.error) throw new AiError("provider_error", `The model provider reported an error: ${String(c.error.message ?? "unknown").slice(0, 200)}`);
      if (typeof c?.delta?.content === "string" && c.delta.content) {
        got = true;
        onDelta(c.delta.content);
      }
      if (c?.finish_reason) finish = c.finish_reason;
      if (j.usage) usage = { promptTokens: j.usage.prompt_tokens ?? null, completionTokens: j.usage.completion_tokens ?? null };
    };
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += dec.decode(value, { stream: true });
        let i: number;
        while ((i = buf.indexOf("\n")) >= 0) {
          handle(buf.slice(0, i).replace(/\r$/, ""));
          buf = buf.slice(i + 1);
        }
      }
      if (buf) handle(buf);
    } catch (e) {
      if (e instanceof AiError) throw e;
      if (String((ctl.signal.reason as Error | undefined)?.message ?? "") === "timeout") throw this.timeoutError();
      if (p.signal?.aborted) throw new AiError("aborted", "Request cancelled", 499);
      throw new AiError("network", "The connection to the model provider was interrupted.");
    } finally {
      reader.cancel().catch(() => {});
    }
    if (!got) throw new AiError("bad_response", "The model returned an empty response.");
    return { finishReason: finish, usage };
  }
}
