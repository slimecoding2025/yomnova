/**
 * HTTP routes for the YOMNOVA AI Assistant (/api/ai/*).
 *
 * Design rules (see docs/AI_ASSISTANT.md):
 *  - Diagnostics always come from the real compiler: the server re-runs it on the submitted code.
 *  - Suggest only: nothing here edits code; `compilerValidated` is only ever reported by /validate-suggestion.
 *  - Validation uses `yomnova check` (parse + types + ownership). It never executes the candidate program.
 *  - /explain-diagnostic may run the program once (the same sandboxed `run` the Playground already offers) to obtain a
 *    runtime diagnostic, but only when `check` is clean and the user asked to explain a failure.
 */
import type http from "node:http";
import { parseDiagnostics, sourceWindow, type Diagnostic } from "./diagnostics.ts";
import type { KnowledgeBase } from "./knowledge.ts";
import { AiError, type ChatMessage, type ModelInfo, type OpenRouter, type Usage } from "./openrouter.ts";
import { codeBlock, diagnosticBlock, knowledgeBlock, PROMPT_VERSION, systemPrompt, type Mode } from "./prompts.ts";
import { RateLimiter } from "./ratelimit.ts";
import { extractJson, jsonSchemaFor, validateResult, type AssistantResult } from "./schema.ts";

export interface CompileResult {
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
}

export interface AiDeps {
  client: OpenRouter;
  kb: KnowledgeBase;
  /** Runs the real compiler. Resolves null when the binary is missing. Goes through the server's concurrency limit. */
  compile: (cmd: "check" | "run", code: string) => Promise<CompileResult | null>;
  version: () => Promise<string>;
  readJson: (req: http.IncomingMessage) => Promise<unknown>;
  sendJson: (res: http.ServerResponse, status: number, body: unknown) => void;
  trustProxy?: boolean;
  perMinute?: number;
  perHour?: number;
  log?: (entry: Record<string, unknown>) => void;
}

const MAX_CODE = 20_000;
const MAX_SELECTION = 8_000;
const MAX_MESSAGES = 12;
const MAX_MESSAGE_CHARS = 4_000;
const POST_ROUTES = new Set(["/api/ai/chat", "/api/ai/explain-diagnostic", "/api/ai/suggest-fix", "/api/ai/validate-suggestion"]);
/** Extra retrieval terms per inferred compiler stage, so lexer/parser errors still find the right documentation. */
const STAGE_TERMS: Record<string, string> = {
  lexer: "lexical rules source files comments string literals tokens",
  parser: "grammar statements expressions program structure",
  semantic: "types statements expressions functions diagnostics",
  ownership: "ownership move moved value",
  runtime: "runtime limits diagnostics",
};

class HttpError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

function str(v: unknown, name: string, max: number, required = false): string | undefined {
  if (v === undefined || v === null || v === "") {
    if (required) throw new HttpError(400, `Missing '${name}'`);
    return undefined;
  }
  if (typeof v !== "string") throw new HttpError(400, `'${name}' must be a string`);
  if (v.length > max) throw new HttpError(413, `'${name}' is too long (max ${max} characters)`);
  return v;
}

function history(v: unknown): ChatMessage[] {
  if (v === undefined) return [];
  if (!Array.isArray(v)) throw new HttpError(400, "'messages' must be an array");
  return v.slice(-MAX_MESSAGES).map((m) => {
    const o = m as { role?: unknown; content?: unknown };
    if ((o?.role !== "user" && o?.role !== "assistant") || typeof o.content !== "string") throw new HttpError(400, "Invalid message");
    return { role: o.role, content: o.content.slice(0, MAX_MESSAGE_CHARS) };
  });
}

export function createAiHandler(deps: AiDeps) {
  const minute = new RateLimiter(deps.perMinute ?? 20, 60_000);
  const hour = new RateLimiter(deps.perHour ?? 300, 3_600_000);
  const { client, kb, sendJson } = deps;

  const clientKey = (req: http.IncomingMessage) =>
    (deps.trustProxy && String(req.headers["x-forwarded-for"] ?? "").split(",")[0].trim()) || req.socket.remoteAddress || "unknown";

  function limited(req: http.IncomingMessage, res: http.ServerResponse): boolean {
    const key = clientKey(req);
    const a = minute.take(key);
    const b = a.ok ? hour.take("*global*") : { ok: true, retryAfterMs: 0 };
    if (a.ok && b.ok) return false;
    const wait = Math.ceil(Math.max(a.retryAfterMs, b.retryAfterMs) / 1000);
    res.setHeader("Retry-After", String(wait));
    sendJson(res, 429, { error: `Too many AI requests. Try again in ${wait}s.`, code: "rate_limited" });
    return true;
  }

  /** Real diagnostics for `code`: `check` first; if clean and asked, one `run` for runtime errors. */
  async function diagnose(code: string, includeRuntime: boolean): Promise<{ diagnostics: Diagnostic[]; checkOk: boolean; ran: boolean }> {
    const c = await deps.compile("check", code);
    if (!c) throw new HttpError(503, "The yomnova compiler binary was not found. Build it first.");
    if (!c.ok) return { diagnostics: parseDiagnostics(c.stderr), checkOk: false, ran: false };
    if (!includeRuntime) return { diagnostics: [], checkOk: true, ran: false };
    const r = await deps.compile("run", code);
    if (r && !r.ok) return { diagnostics: parseDiagnostics(r.stderr), checkOk: true, ran: true };
    return { diagnostics: [], checkOk: true, ran: true };
  }

  /** The free models that are available right now, in preference order. */
  async function candidates(): Promise<ModelInfo[]> {
    const pool = await client.pool();
    if (!pool.length) throw new AiError("provider_error", "The AI Assistant is temporarily unavailable. Please try again later.", 503);
    return pool;
  }

  async function structured(
    mode: "explain_diagnostic" | "suggest_fix",
    pool: ModelInfo[],
    userContent: string,
    sources: Set<string>,
    msgs: ChatMessage[],
    signal: AbortSignal,
    ver: string,
  ): Promise<{ result: AssistantResult; usage: Usage; used: string }> {
    const type = mode === "explain_diagnostic" ? "error_explanation" : "fix_suggestions";
    const messages: ChatMessage[] = [{ role: "system", content: systemPrompt(mode, ver) }, ...msgs, { role: "user", content: userContent }];
    let last: unknown = null;
    for (const model of pool) {
      try {
        const { text, usage } = await client.complete({ model, messages, structured: jsonSchemaFor(type), signal });
        let parsed: unknown;
        try {
          parsed = extractJson(text);
        } catch {
          throw new AiError("bad_response", "The AI did not return a usable answer. Please try again.");
        }
        const v = validateResult(parsed, type, sources);
        if (!v.ok) throw new AiError("bad_response", "The AI did not return a usable answer. Please try again.");
        return { result: v.value, usage, used: model.id };
      } catch (e) {
        if (signal.aborted || (e instanceof AiError && e.code === "aborted")) throw e;
        last = e; // try the next free model
      }
    }
    throw last instanceof AiError ? new AiError(last.code, "The AI could not answer right now. Please try again in a moment.", last.status) : last;
  }

  function errorStatus(e: unknown): { status: number; body: { error: string; code: string } } {
    if (e instanceof HttpError) return { status: e.status, body: { error: e.message, code: "bad_request" } };
    if (e instanceof AiError) return { status: e.status, body: { error: e.message, code: e.code } };
    const err = e as Error & { status?: number };
    if (err.status) return { status: err.status, body: { error: err.message, code: "bad_request" } };
    return { status: 500, body: { error: "Internal error", code: "internal" } };
  }

  return async function handle(req: http.IncomingMessage, res: http.ServerResponse, url: URL): Promise<boolean> {
    if (!url.pathname.startsWith("/api/ai/")) return false;
    const started = Date.now();
    const route = url.pathname;
    const ctl = new AbortController();
    res.on("close", () => {
      if (!res.writableEnded) ctl.abort(new Error("aborted"));
    });
    let model = "";
    try {
      if (req.method === "GET" && route === "/api/ai/models") {
        if (!client.configured) {
          sendJson(res, 200, { configured: false, promptVersion: PROMPT_VERSION, defaultModel: "", models: [], message: "The AI Assistant is not available right now." });
          return true;
        }
        // Visitors never see or choose a model: the server picks from its own free pool.
        sendJson(res, 200, { configured: true, promptVersion: PROMPT_VERSION, defaultModel: "", models: [] });
        return true;
      }
      if (req.method !== "POST") throw new HttpError(405, "Method not allowed");
      if (!POST_ROUTES.has(route)) throw new HttpError(404, "Unknown AI route");
      if (!client.configured) throw new AiError("not_configured", "The AI Assistant is not available right now.", 503);
      if (limited(req, res)) return true;
      const body = (await deps.readJson(req)) as Record<string, unknown>;

      if (route === "/api/ai/validate-suggestion") {
        const code = str(body.code, "code", MAX_CODE, true)!;
        const c = await deps.compile("check", code);
        if (!c) throw new HttpError(503, "The yomnova compiler binary was not found. Build it first.");
        sendJson(res, 200, {
          validated: c.ok,
          diagnostics: c.ok ? [] : parseDiagnostics(c.stderr),
          output: c.ok ? c.stdout : c.stderr,
          note: "Compiler `check` only (parse, types, ownership). The program was NOT executed, and your editor was NOT changed.",
        });
        deps.log?.({ route, status: 200, ms: Date.now() - started });
        return true;
      }

      const pool = await candidates();
      const ver = await deps.version();
      const question = str(body.question, "question", 2000);
      const msgs = history(body.messages);

      if (route === "/api/ai/chat") {
        const mode = (str(body.mode, "mode", 20) ?? "chat") as Mode;
        if (!["chat", "tutor", "explain_code", "debug"].includes(mode)) throw new HttpError(400, "Invalid mode");
        const last = msgs[msgs.length - 1];
        if (!last || last.role !== "user") throw new HttpError(400, "The last message must be from the user");
        const code = str(body.code, "code", MAX_CODE);
        const selection = str(body.selection, "selection", MAX_SELECTION);
        if (mode === "explain_code" && !selection) throw new HttpError(400, "Select some code first");

        let diags: Diagnostic[] = [];
        if (mode === "debug" && code) diags = (await diagnose(code, true)).diagnostics;

        const hits = kb.retrieve(`${last.content} ${selection ?? ""} ${diags.map((d) => d.message).join(" ")}`, { limit: 4, maxChars: 6000 });
        const parts = [knowledgeBlock(hits)];
        if (selection) parts.push("The user selected this code:\n" + codeBlock("selection", selection));
        if (code && mode !== "explain_code") parts.push("The user's current program:\n" + codeBlock("code", code));
        if (diags.length) parts.push("Real compiler output for the program:\n" + diagnosticBlock(diags.map((d) => d.raw)));
        else if (mode === "debug" && code) parts.push("The compiler reports no diagnostics for this program (check passed; it also ran without a runtime error).");
        const messages: ChatMessage[] = [
          { role: "system", content: systemPrompt(mode, ver) + "\n\n" + parts.join("\n\n") },
          ...msgs.slice(0, -1),
          last,
        ];
        // From here on the response is a stream.
        res.writeHead(200, { "Content-Type": "text/event-stream; charset=utf-8", "Cache-Control": "no-store", Connection: "keep-alive", "X-Content-Type-Options": "nosniff", "X-Accel-Buffering": "no" });
        const send = (event: string, data: unknown) => res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
        send("meta", { model: "YOMNOVA AI", promptVersion: PROMPT_VERSION, sources: [], diagnostics: diags });
        let sentAny = false;
        let failure: unknown = null;
        for (const info of pool) {
          try {
            model = info.id;
            const out = await client.stream({ model: info, messages, signal: ctl.signal }, (t) => {
              sentAny = true;
              send("delta", { text: t });
            });
            send("done", { finishReason: out.finishReason, usage: out.usage });
            deps.log?.({ route, model, status: 200, ms: Date.now() - started, ...out.usage });
            failure = null;
            break;
          } catch (e) {
            failure = e;
            if (sentAny || ctl.signal.aborted) break; // only fall back before any text reached the visitor
          }
        }
        if (failure) {
          const { body: b, status } = errorStatus(failure);
          send("error", { error: "The AI could not answer right now. Please try again in a moment.", code: b.code });
          deps.log?.({ route, model, status, ms: Date.now() - started, code: b.code });
        }
        res.end();
        return true;
      }

      if (route === "/api/ai/explain-diagnostic" || route === "/api/ai/suggest-fix") {
        const code = str(body.code, "code", MAX_CODE, true)!;
        const selection = str(body.selection, "selection", MAX_SELECTION);
        const explain = route === "/api/ai/explain-diagnostic";
        const { diagnostics, checkOk, ran } = await diagnose(code, explain);
        if (explain && diagnostics.length === 0) {
          throw new HttpError(422, "The compiler reports no diagnostics for this code, so there is nothing to explain. Edit the code or use the Ask action.");
        }
        if (!explain && diagnostics.length === 0 && !question && !selection) {
          throw new HttpError(422, "No compiler diagnostic exists for this code. Ask a question or select code to get improvement suggestions.");
        }
        const index = Number.isInteger(body.diagnosticIndex) ? (body.diagnosticIndex as number) : 0;
        const picked = diagnostics[Math.min(Math.max(index, 0), Math.max(diagnostics.length - 1, 0))] as Diagnostic | undefined;
        const hits = kb.retrieve(`${picked?.message ?? ""} ${picked?.hint ?? ""} ${STAGE_TERMS[picked?.stage ?? ""] ?? ""} ${question ?? ""} ${selection ?? ""}`, { limit: 4, maxChars: 6000 });
        const parts = [knowledgeBlock(hits)];
        if (picked) {
          parts.push("Real compiler diagnostic to " + (explain ? "explain" : "fix") + ":\n" + diagnosticBlock([picked.raw]));
          if (picked.line) parts.push("Source around the reported line:\n" + codeBlock("code", sourceWindow(code, picked.line)));
          const others = diagnostics.filter((d) => d !== picked);
          if (others.length) parts.push("Other diagnostics from the same compile:\n" + diagnosticBlock(others.map((d) => d.raw)));
        } else {
          parts.push("The compiler reports no diagnostics for this program.");
        }
        parts.push("Full program (suggestion 'before' text must match it exactly):\n" + codeBlock("code", code));
        if (selection) parts.push("Selected code:\n" + codeBlock("selection", selection));
        if (question) parts.push("User question:\n" + question);
        const sources = new Set(hits.map((h) => h.id));
        const { result, usage, used } = await structured(explain ? "explain_diagnostic" : "suggest_fix", pool, parts.join("\n\n"), sources, msgs, ctl.signal, ver);
        model = used;
        sendJson(res, 200, {
          result,
          model: "YOMNOVA AI",
          promptVersion: PROMPT_VERSION,
          sources: [],
          diagnostics,
          selectedDiagnostic: picked?.id ?? null,
          compiler: { checkOk, ranProgram: ran },
          disclaimer: "AI suggestion: not applied to your code.",
        });
        deps.log?.({ route, model, status: 200, ms: Date.now() - started, ...usage });
        return true;
      }

      throw new HttpError(404, "Unknown AI route");
    } catch (e) {
      const { status, body: b } = errorStatus(e);
      if (!res.headersSent) sendJson(res, status, b);
      else res.end();
      deps.log?.({ route, model, status, ms: Date.now() - started, code: b.code });
      return true;
    }
  };
}
