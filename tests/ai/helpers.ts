import { execFile } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KnowledgeBase } from "../../server/ai/knowledge.ts";
import { configFromEnv, OpenRouter, type AiConfig } from "../../server/ai/openrouter.ts";
import { createAiHandler, type CompileResult } from "../../server/ai/routes.ts";

export const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

export function binary(): string | null {
  const exe = process.platform === "win32" ? "yomnova.exe" : "yomnova";
  return [process.env.YOMNOVA_BIN, path.join(ROOT, "yomnova/target/release", exe), path.join(ROOT, "yomnova/target/debug", exe)]
    .filter((p): p is string => Boolean(p))
    .find((p) => fs.existsSync(p)) ?? null;
}

/** Runs the REAL compiler on `code` (same approach as server.ts, minus the HTTP layer). */
export async function realCompile(cmd: "check" | "run", code: string): Promise<CompileResult | null> {
  const bin = binary();
  if (!bin) return null;
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "yomnova-test-"));
  const file = path.join(dir, "main.yom");
  await fs.promises.writeFile(file, code);
  const started = Date.now();
  return new Promise((resolve) => {
    execFile(bin, [cmd, file], { timeout: 5000, env: { ...process.env, NO_COLOR: "1" } }, (err, stdout, stderr) => {
      fs.promises.rm(dir, { recursive: true, force: true }).catch(() => {});
      const e = err as (NodeJS.ErrnoException & { killed?: boolean; code?: number | string }) | null;
      resolve({
        ok: !e,
        exitCode: e ? (typeof e.code === "number" ? e.code : 1) : 0,
        stdout: String(stdout).split(file).join("main.yom"),
        stderr: String(stderr).split(file).join("main.yom"),
        durationMs: Date.now() - started,
        timedOut: Boolean(e?.killed),
      });
    });
  });
}

export const MODELS = [
  { id: "acme/alpha", name: "Alpha", description: "d", context_length: 32000, supported_parameters: ["structured_outputs", "response_format", "max_tokens"], architecture: { input_modalities: ["text"], output_modalities: ["text"] }, top_provider: { max_completion_tokens: 8000 } },
  { id: "acme/beta-json", name: "Beta JSON", context_length: 8000, supported_parameters: ["response_format"], architecture: { input_modalities: ["text"], output_modalities: ["text"] } },
  { id: "other/plain", name: "Plain", context_length: 4096, supported_parameters: [], architecture: { input_modalities: ["text"], output_modalities: ["text"] } },
  { id: "img/gen", name: "Image only", architecture: { input_modalities: ["text"], output_modalities: ["image"] } },
];

export interface Upstream {
  fetch: typeof fetch;
  calls: Array<{ url: string; method: string; headers: Record<string, string>; body: any }>;
  /** Next /chat/completions behaviour. */
  next: (r: (body: any) => Response | Promise<Response>) => void;
}

export function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

export function sse(chunks: string[]): Response {
  const enc = new TextEncoder();
  const stream = new ReadableStream({
    start(c) {
      for (const ch of chunks) c.enqueue(enc.encode(ch));
      c.close();
    },
  });
  return new Response(stream, { status: 200, headers: { "Content-Type": "text/event-stream" } });
}

export function completion(content: string, usage = { prompt_tokens: 100, completion_tokens: 50 }) {
  return json(200, { choices: [{ message: { role: "assistant", content }, finish_reason: "stop" }], usage });
}

/** A mock OpenRouter: records every call; behaviour of /chat/completions is set per test. */
export function mockUpstream(): Upstream {
  let handler: (body: any) => Response | Promise<Response> = () => completion("{}");
  const calls: Upstream["calls"] = [];
  const f = (async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input);
    const headers = Object.fromEntries(Object.entries((init?.headers ?? {}) as Record<string, string>));
    const body = init?.body ? JSON.parse(String(init.body)) : undefined;
    calls.push({ url, method: init?.method ?? "GET", headers, body });
    if (url.endsWith("/models")) return json(200, { data: MODELS });
    if (url.endsWith("/chat/completions")) {
      const sig = init?.signal;
      const r = handler(body);
      if (sig) {
        return new Promise<Response>((resolve, reject) => {
          sig.addEventListener("abort", () => reject(sig.reason ?? new Error("aborted")), { once: true });
          Promise.resolve(r).then(resolve, reject);
        });
      }
      return r;
    }
    return json(404, { error: { message: "no route" } });
  }) as typeof fetch;
  return { fetch: f, calls, next: (r) => { handler = r; } };
}

export const KEY = "sk-or-test-SECRET-1234567890";

export function makeClient(up: Upstream, env: Record<string, string> = {}): OpenRouter {
  const cfg: AiConfig = configFromEnv({ OPENROUTER_API_KEY: KEY, AI_MODEL_ALLOWLIST: "", AI_FREE_MODELS: "acme/alpha,acme/beta-json", ...env } as NodeJS.ProcessEnv, up.fetch);
  return new OpenRouter(cfg);
}

export async function startServer(client: OpenRouter, extra: Partial<Parameters<typeof createAiHandler>[0]> = {}) {
  const handler = createAiHandler({
    client,
    kb: KnowledgeBase.load(ROOT),
    compile: realCompile,
    version: async () => "yomnova 0.1.0 (test)",
    readJson: (req) =>
      new Promise((resolve, reject) => {
        const chunks: Buffer[] = [];
        req.on("data", (c: Buffer) => chunks.push(c));
        req.on("end", () => {
          try { resolve(JSON.parse(Buffer.concat(chunks).toString() || "{}")); } catch { reject(Object.assign(new Error("Invalid JSON body"), { status: 400 })); }
        });
      }),
    sendJson: (res, status, body) => {
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(JSON.stringify(body));
    },
    ...extra,
  });
  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://x");
    handler(req, res, url).then((h) => { if (!h) res.writeHead(404).end(); });
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  server.unref(); // a failed assertion must not keep the test process alive
  const port = (server.address() as { port: number }).port;
  const call = async (p: string, body?: unknown, method = body === undefined ? "GET" : "POST") => {
    const r = await fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "Content-Type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
    return { status: r.status, text: await r.text(), headers: r.headers };
  };
  return { call, close: () => new Promise<void>((r) => { server.closeAllConnections(); server.close(() => r()); }), port };
}
