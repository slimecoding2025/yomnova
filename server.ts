/**
 * YOMNOVA Playground server.
 *
 * A small dependency-free HTTP server (node:http) that
 *   - exposes the real `yomnova` CLI as a JSON API (/api/*), and
 *   - serves the web app (Vite middleware in development, dist/ in production).
 *
 * SECURITY NOTE: the CLI runs as a child process with a timeout, an output cap and a
 * concurrency limit, but there is NO operating-system sandbox. The language has no file or
 * network I/O, yet you should still run this behind container isolation if you expose it
 * publicly.
 */
import { execFile } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const isProd = process.argv.includes("--prod") || process.env.NODE_ENV === "production";
const PORT = Number.parseInt(process.env.PORT ?? "3000", 10);

const MAX_BODY_BYTES = 64 * 1024;
const RUN_TIMEOUT_MS = 5_000;
const MAX_OUTPUT_BYTES = 1024 * 1024;
const MAX_CONCURRENT = 4;
let running = 0;

const EXAMPLES_DIR = path.join(__dirname, "yomnova", "examples");

function resolveBinary(): string | null {
  const exe = process.platform === "win32" ? "yomnova.exe" : "yomnova";
  const candidates = [
    process.env.YOMNOVA_BIN,
    path.join(__dirname, "yomnova", "target", "release", exe),
    path.join(__dirname, "yomnova", "target", "debug", exe),
  ].filter((p): p is string => Boolean(p));
  return candidates.find((p) => fs.existsSync(p)) ?? null;
}

interface CliResult {
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
}

function runCli(bin: string, args: string[], cwd?: string): Promise<CliResult> {
  const started = Date.now();
  return new Promise((resolve) => {
    execFile(
      bin,
      args,
      { timeout: RUN_TIMEOUT_MS, maxBuffer: MAX_OUTPUT_BYTES, cwd, env: { ...process.env, NO_COLOR: "1" } },
      (err, stdout, stderr) => {
        const e = err as (NodeJS.ErrnoException & { killed?: boolean; code?: number | string }) | null;
        resolve({
          ok: !e,
          exitCode: e ? (typeof e.code === "number" ? e.code : 1) : 0,
          stdout: String(stdout),
          stderr: String(stderr) || (e?.killed ? `Terminated: exceeded ${RUN_TIMEOUT_MS / 1000}s time limit` : ""),
          durationMs: Date.now() - started,
          timedOut: Boolean(e?.killed),
        });
      },
    );
  });
}

/** Write `code` to a private temp dir, run a CLI command on it, and clean up. */
async function withSource(
  bin: string,
  code: string,
  makeArgs: (file: string) => string[],
  readBack = false,
): Promise<CliResult & { file?: string }> {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "yomnova-"));
  const file = path.join(dir, "main.yom");
  try {
    await fs.promises.writeFile(file, code, "utf-8");
    const result = await runCli(bin, makeArgs(file), dir);
    const sanitize = (s: string) => s.split(file).join("main.yom");
    const out = { ...result, stdout: sanitize(result.stdout), stderr: sanitize(result.stderr) };
    if (readBack) return { ...out, file: await fs.promises.readFile(file, "utf-8") };
    return out;
  } finally {
    fs.promises.rm(dir, { recursive: true, force: true }).catch(() => {});
  }
}

function sendJson(res: http.ServerResponse, status: number, body: unknown) {
  const data = JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  res.end(data);
}

function readJsonBody(req: http.IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => {
      size += c.length;
      if (size > MAX_BODY_BYTES) {
        reject(Object.assign(new Error(`Request body exceeds ${MAX_BODY_BYTES} bytes`), { status: 413 }));
        req.destroy();
        return;
      }
      chunks.push(c);
    });
    req.on("end", () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf-8") || "{}"));
      } catch {
        reject(Object.assign(new Error("Invalid JSON body"), { status: 400 }));
      }
    });
    req.on("error", reject);
  });
}

const COMMANDS: Record<string, (file: string) => string[]> = {
  "/api/run": (f) => ["run", f],
  "/api/check": (f) => ["check", f],
  "/api/tokens": (f) => ["tokens", f],
  "/api/ast": (f) => ["ast", f],
  "/api/disassemble": (f) => ["disasm", f],
  "/api/fmt": (f) => ["fmt", f],
};

async function handleApi(req: http.IncomingMessage, res: http.ServerResponse, url: URL) {
  const bin = resolveBinary();

  if (req.method === "GET" && url.pathname === "/api/version") {
    if (!bin) {
      return sendJson(res, 503, {
        available: false,
        error: "The yomnova binary was not found. Build it with `npm run build:compiler` or set YOMNOVA_BIN.",
      });
    }
    const r = await runCli(bin, ["version"]);
    return sendJson(res, r.ok ? 200 : 500, { available: r.ok, version: r.stdout.split("\n")[0]?.trim() ?? "" });
  }

  if (req.method === "GET" && url.pathname === "/api/examples") {
    try {
      const names = (await fs.promises.readdir(EXAMPLES_DIR)).filter((n) => n.endsWith(".yom")).sort();
      const examples = await Promise.all(
        names.map(async (name) => ({
          id: name.replace(/\.yom$/, ""),
          filename: name,
          code: await fs.promises.readFile(path.join(EXAMPLES_DIR, name), "utf-8"),
        })),
      );
      return sendJson(res, 200, { examples });
    } catch {
      return sendJson(res, 200, { examples: [] });
    }
  }

  const makeArgs = COMMANDS[url.pathname];
  if (req.method === "POST" && makeArgs) {
    if (!bin) {
      return sendJson(res, 503, { error: "The yomnova binary was not found. Build the compiler first." });
    }
    let body: unknown;
    try {
      body = await readJsonBody(req);
    } catch (e) {
      const err = e as Error & { status?: number };
      return sendJson(res, err.status ?? 400, { error: err.message });
    }
    const code = (body as { code?: unknown }).code;
    if (typeof code !== "string") {
      return sendJson(res, 400, { error: "Missing or invalid 'code' field" });
    }
    if (running >= MAX_CONCURRENT) {
      return sendJson(res, 429, { error: "Server is busy. Please retry in a moment." });
    }
    running += 1;
    try {
      const isFmt = url.pathname === "/api/fmt";
      const r = await withSource(bin, code, makeArgs, isFmt);
      return sendJson(res, 200, isFmt ? { ...r, formatted: r.file } : r);
    } catch (e) {
      return sendJson(res, 500, { error: (e as Error).message });
    } finally {
      running -= 1;
    }
  }

  return sendJson(res, 404, { error: "Unknown API route" });
}

const MIME: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".json": "application/json; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".woff2": "font/woff2",
};

async function serveStatic(_req: http.IncomingMessage, res: http.ServerResponse, url: URL) {
  const dist = path.join(__dirname, "dist");
  const rel = decodeURIComponent(url.pathname);
  let file = path.normalize(path.join(dist, rel));
  if (!file.startsWith(dist)) {
    res.writeHead(403).end("Forbidden");
    return;
  }
  try {
    const stat = await fs.promises.stat(file);
    if (stat.isDirectory()) file = path.join(file, "index.html");
  } catch {
    file = path.join(dist, "index.html"); // SPA fallback
  }
  try {
    const data = await fs.promises.readFile(file);
    const ext = path.extname(file);
    res.writeHead(200, {
      "Content-Type": MIME[ext] ?? "application/octet-stream",
      "Cache-Control": file.includes(`${path.sep}assets${path.sep}`) ? "public, max-age=31536000, immutable" : "no-cache",
      "X-Content-Type-Options": "nosniff",
    });
    res.end(data);
  } catch {
    res.writeHead(500, { "Content-Type": "text/plain" }).end("dist/ not found. Run `npm run build` first.");
  }
}

async function main() {
  type Middleware = (req: http.IncomingMessage, res: http.ServerResponse, next: (e?: unknown) => void) => void;
  let viteMiddleware: Middleware | null = null;
  if (!isProd) {
    const { createServer } = await import("vite");
    const vite = await createServer({ server: { middlewareMode: true }, appType: "spa" });
    viteMiddleware = vite.middlewares as unknown as Middleware;
  }

  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    if (url.pathname.startsWith("/api/")) {
      handleApi(req, res, url).catch((e) => sendJson(res, 500, { error: (e as Error).message }));
    } else if (viteMiddleware) {
      viteMiddleware(req, res, () => res.writeHead(404).end("Not found"));
    } else {
      serveStatic(req, res, url).catch(() => res.writeHead(500).end("Internal error"));
    }
  });

  server.listen(PORT, () => {
    const bin = resolveBinary();
    console.log(`YOMNOVA ${isProd ? "production" : "development"} server: http://localhost:${PORT}`);
    console.log(bin ? `Compiler binary: ${bin}` : "Compiler binary NOT FOUND — the Playground will be unavailable.");
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
