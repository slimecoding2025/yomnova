/**
 * Language knowledge base for the YOMNOVA AI Assistant.
 *
 * The knowledge is NOT a separate copy: it is built at start-up from the repository's own Markdown
 * documentation (docs/LANGUAGE_SPECIFICATION.md, GRAMMAR.md, ARCHITECTURE.md, ROADMAP.md, README.md) and the
 * programs in yomnova/examples/. Update the docs and the assistant's knowledge updates with them.
 *
 * Retrieval is a small keyword scorer (BM25-style term weighting) over heading-delimited chunks. No embeddings,
 * no vector database.
 */
import fs from "node:fs";
import path from "node:path";

export interface Chunk {
  /** Stable source id shown to users, e.g. `docs/LANGUAGE_SPECIFICATION.md#3. Types`. */
  id: string;
  kind: "doc" | "example";
  title: string;
  text: string;
  tokens: string[];
}

export interface Hit {
  id: string;
  kind: Chunk["kind"];
  title: string;
  text: string;
  score: number;
}

const DOCS = [
  "docs/LANGUAGE_SPECIFICATION.md",
  "docs/GRAMMAR.md",
  "docs/ARCHITECTURE.md",
  "docs/ROADMAP.md",
  "README.md",
];

const STOP = new Set(
  "a an and are as at be by for from has have how in is it its of on or that the this to was what when where which why with i you do does can not no my your me we if then else into".split(
    " ",
  ),
);

export function tokenize(s: string): string[] {
  return (s.toLowerCase().match(/[a-z0-9_]+|[^\sa-z0-9_]/g) ?? []).filter((t) => !STOP.has(t) && t.length > 0);
}

/** Splits Markdown into chunks at `#`/`##`/`###` headings, ignoring headings inside code fences. */
export function chunkMarkdown(file: string, md: string, maxChars = 2400): Chunk[] {
  const out: Chunk[] = [];
  let title = path.basename(file);
  let buf: string[] = [];
  let fence = false;
  const flush = () => {
    const text = buf.join("\n").trim();
    buf = [];
    if (!text) return;
    // Split very long sections on blank lines so a chunk never dominates the prompt.
    const parts: string[] = [];
    let cur = "";
    for (const para of text.split(/\n{2,}/)) {
      if (cur && cur.length + para.length > maxChars) {
        parts.push(cur);
        cur = "";
      }
      cur += (cur ? "\n\n" : "") + para;
    }
    if (cur) parts.push(cur);
    parts.forEach((p, i) => {
      const id = `${file}#${title}${parts.length > 1 ? ` (part ${i + 1})` : ""}`;
      out.push({ id, kind: "doc", title, text: p, tokens: tokenize(`${title} ${p}`) });
    });
  };
  for (const line of md.split("\n")) {
    if (line.trimStart().startsWith("```")) fence = !fence;
    const h = !fence && /^(#{1,3})\s+(.*)$/.exec(line);
    if (h) {
      flush();
      title = h[2].trim();
    }
    buf.push(line);
  }
  flush();
  return out;
}

/** Source priority: the specification outranks other docs, which outrank README prose and the roadmap. */
function weight(id: string): number {
  if (id.startsWith("docs/LANGUAGE_SPECIFICATION.md")) return 1.3;
  if (id.startsWith("docs/GRAMMAR.md")) return 1.15;
  if (id.startsWith("README.md") || id.startsWith("docs/ROADMAP.md")) return 0.8;
  if (id.startsWith("yomnova/examples/")) return 0.85; // the specification explains; examples only illustrate
  return 1;
}

export class KnowledgeBase {
  readonly chunks: Chunk[];
  private df = new Map<string, number>();
  private avgLen = 1;

  constructor(chunks: Chunk[]) {
    this.chunks = chunks;
    for (const c of chunks) for (const t of new Set(c.tokens)) this.df.set(t, (this.df.get(t) ?? 0) + 1);
    this.avgLen = chunks.reduce((a, c) => a + c.tokens.length, 0) / Math.max(1, chunks.length);
  }

  static load(root: string): KnowledgeBase {
    const chunks: Chunk[] = [];
    for (const rel of DOCS) {
      try {
        chunks.push(...chunkMarkdown(rel, fs.readFileSync(path.join(root, rel), "utf-8")));
      } catch {
        /* a missing doc simply contributes nothing */
      }
    }
    const exDir = path.join(root, "yomnova", "examples");
    try {
      for (const name of fs.readdirSync(exDir).filter((n) => n.endsWith(".yom")).sort()) {
        const text = fs.readFileSync(path.join(exDir, name), "utf-8").slice(0, 3000);
        const id = `yomnova/examples/${name}`;
        // Two shipped examples deliberately fail; say so, so the model never presents them as valid code.
        const note = /(_fail|type_error)\.yom$/.test(name)
          ? "// NOTE: this example is intentionally INVALID and is rejected by the compiler.\n"
          : "";
        chunks.push({ id, kind: "example", title: name, text: note + text, tokens: tokenize(`${name} ${text}`) });
      }
    } catch {
      /* no examples directory */
    }
    return new KnowledgeBase(chunks);
  }

  /** Returns the best chunks for `query`, capped by `limit` chunks and `maxChars` total characters. */
  retrieve(query: string, opts: { limit?: number; maxChars?: number; kinds?: Chunk["kind"][] } = {}): Hit[] {
    const { limit = 4, maxChars = 6000, kinds } = opts;
    const q = [...new Set(tokenize(query))];
    if (q.length === 0) return [];
    const N = this.chunks.length;
    const k1 = 1.4;
    const b = 0.75;
    const scored: Hit[] = [];
    for (const c of this.chunks) {
      if (kinds && !kinds.includes(c.kind)) continue;
      const tf = new Map<string, number>();
      for (const t of c.tokens) tf.set(t, (tf.get(t) ?? 0) + 1);
      let score = 0;
      for (const t of q) {
        const f = tf.get(t);
        if (!f) continue;
        const df = this.df.get(t) ?? 0;
        const idf = Math.log(1 + (N - df + 0.5) / (df + 0.5));
        score += (idf * f * (k1 + 1)) / (f + k1 * (1 - b + (b * c.tokens.length) / this.avgLen));
      }
      if (score > 0) scored.push({ id: c.id, kind: c.kind, title: c.title, text: c.text, score: score * weight(c.id) });
    }
    scored.sort((a, b2) => b2.score - a.score);
    const hits: Hit[] = [];
    let used = 0;
    for (const h of scored) {
      if (hits.length >= limit) break;
      if (used + h.text.length > maxChars) {
        if (hits.length === 0) hits.push({ ...h, text: h.text.slice(0, maxChars) });
        break;
      }
      hits.push(h);
      used += h.text.length;
    }
    return hits;
  }
}
