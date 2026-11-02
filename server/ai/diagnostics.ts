/**
 * Adapter from the real `yomnova` CLI output to a structured diagnostic.
 *
 * The compiler prints diagnostics as text:
 *   main.yom:9:17: error: Type mismatch at 9:17: expected Int, found Text
 *       9 |     emit(double("twenty-one"))
 *         |                 ^^^^^^^^^^^^
 *     = hint: ...
 * It has NO diagnostic codes and does not report which stage produced an error, so neither field exists here:
 * nothing is fabricated. `stage` is only set when it can be derived from the CLI message wording.
 */
export interface Diagnostic {
  id: string;
  severity: "error" | "warning" | "info";
  message: string;
  file: string;
  line: number | null;
  column: number | null;
  hint: string | null;
  /** The compiler's own source excerpt lines, verbatim. */
  excerpt: string;
  /** Best-effort stage inferred from the message text; null when unknown. */
  stage: "lexer" | "parser" | "semantic" | "ownership" | "runtime" | null;
  /** The compiler text exactly as printed. */
  raw: string;
}

const HEAD = /^(.+?):(\d+):(\d+): (error|warning|info): (.*)$/;

function inferStage(msg: string): Diagnostic["stage"] {
  const m = msg.toLowerCase();
  if (/unexpected character|unterminated|invalid (escape|number)/.test(m)) return "lexer";
  if (/unexpected token|expected (expression|identifier|type|`|')|nesting/.test(m)) return "parser";
  if (/moved|borrow|ownership/.test(m)) return "ownership";
  if (/type mismatch|undefined|not supported|duplicate|missing return|expects \d+ argument|cannot assign|invalid/.test(m)) return "semantic";
  return null;
}

export function parseDiagnostics(stderr: string, file = "main.yom"): Diagnostic[] {
  const lines = stderr.split("\n");
  const out: Diagnostic[] = [];
  let cur: { head: RegExpExecArray; body: string[] } | null = null;
  const close = () => {
    if (!cur) return;
    const [, f, l, c, sev, message] = cur.head;
    const hintLine = cur.body.find((b) => b.trim().startsWith("= hint:"));
    out.push({
      id: `d${out.length + 1}`,
      severity: sev as Diagnostic["severity"],
      message,
      file: f || file,
      line: Number(l),
      column: Number(c),
      hint: hintLine ? hintLine.trim().replace(/^= hint:\s*/, "") : null,
      excerpt: cur.body.filter((b) => /^\s*\d+ \|/.test(b) || /^\s*\|/.test(b)).join("\n"),
      stage: inferStage(message),
      raw: [cur.head[0], ...cur.body].join("\n"),
    });
    cur = null;
  };
  for (const line of lines) {
    const h = HEAD.exec(line);
    if (h) {
      close();
      cur = { head: h, body: [] };
    } else if (cur && line.trim() !== "") {
      cur.body.push(line);
    }
  }
  close();
  if (out.length === 0 && stderr.trim()) {
    // Output that does not carry a location (e.g. a runtime error). Preserve it verbatim.
    out.push({
      id: "d1",
      severity: "error",
      message: stderr.trim().split("\n")[0].replace(/^error:\s*/i, ""),
      file,
      line: null,
      column: null,
      hint: null,
      excerpt: "",
      stage: /runtime|step limit|overflow|division|stack/i.test(stderr) ? "runtime" : null,
      raw: stderr.trim(),
    });
  }
  return out;
}

/** A few source lines around `line` (1-based), for models that were not given the whole file. */
export function sourceWindow(code: string, line: number | null, radius = 4): string {
  if (!line) return "";
  const rows = code.split("\n");
  const from = Math.max(1, line - radius);
  const to = Math.min(rows.length, line + radius);
  const out: string[] = [];
  for (let i = from; i <= to; i++) out.push(`${String(i).padStart(4)} | ${rows[i - 1]}`);
  return out.join("\n");
}
