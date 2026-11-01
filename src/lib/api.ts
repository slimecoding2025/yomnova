export interface CliResult {
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
  formatted?: string;
  error?: string;
}

export interface ExampleFile {
  id: string;
  filename: string;
  code: string;
}

export type Action = "run" | "check" | "tokens" | "ast" | "disassemble" | "fmt";

async function post(action: Action, code: string): Promise<CliResult> {
  const res = await fetch(`/api/${action}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code }),
  });
  const data = (await res.json().catch(() => ({}))) as Partial<CliResult>;
  if (!res.ok && data.error) {
    return { ok: false, exitCode: -1, stdout: "", stderr: data.error, durationMs: 0, timedOut: false, error: data.error };
  }
  return data as CliResult;
}

export const api = {
  run: (code: string) => post("run", code),
  check: (code: string) => post("check", code),
  tokens: (code: string) => post("tokens", code),
  ast: (code: string) => post("ast", code),
  disassemble: (code: string) => post("disassemble", code),
  fmt: (code: string) => post("fmt", code),
  async version(): Promise<string | null> {
    try {
      const res = await fetch("/api/version");
      if (!res.ok) return null;
      const data = (await res.json()) as { available?: boolean; version?: string };
      return data.available ? (data.version ?? "yomnova") : null;
    } catch {
      return null;
    }
  },
  async examples(): Promise<ExampleFile[]> {
    try {
      const res = await fetch("/api/examples");
      const data = (await res.json()) as { examples?: ExampleFile[] };
      return data.examples ?? [];
    } catch {
      return [];
    }
  },
};
