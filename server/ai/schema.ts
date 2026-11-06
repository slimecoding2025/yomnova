/**
 * Response contract for structured assistant answers, plus strict validation.
 * Model output is untrusted: valid JSON is not enough, every field is type-checked, length-capped and
 * references are checked against the sources that were actually supplied to the model.
 */
export type Confidence = "low" | "medium" | "high";

export interface Suggestion {
  title: string;
  description: string;
  before: string;
  after: string;
  /** The model's self-reported, uncalibrated rating. Not a probability. */
  confidence: Confidence;
  /** Always false when produced here. Only the explicit validate endpoint can show a compiler result. */
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

export type Validation = { ok: true; value: AssistantResult } | { ok: false; error: string };

const TYPES = ["error_explanation", "fix_suggestions", "answer"] as const;
const CONF = ["low", "medium", "high"] as const;

/** JSON Schema sent to providers that support structured outputs. Strict mode needs every key required. */
export function jsonSchemaFor(type: AssistantResult["type"]) {
  const str = { type: "string" };
  return {
    name: "yomnova_assistant_response",
    strict: true,
    schema: {
      type: "object",
      additionalProperties: false,
      required: ["type", "summary", "explanation", "languageRule", "suggestions", "limitations", "references"],
      properties: {
        type: { type: "string", enum: [type] },
        summary: str,
        explanation: str,
        languageRule: {
          type: "object",
          additionalProperties: false,
          required: ["title", "source"],
          properties: { title: str, source: str },
        },
        suggestions: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["title", "description", "before", "after", "confidence"],
            properties: {
              title: str,
              description: str,
              before: str,
              after: str,
              confidence: { type: "string", enum: [...CONF] },
            },
          },
        },
        limitations: { type: "array", items: str },
        references: { type: "array", items: str },
      },
    },
  };
}

/** Removes control characters (except tab/newline) so model text cannot smuggle terminal or bidi tricks. */
export function cleanText(s: string, max: number): string {
  // eslint-disable-next-line no-control-regex
  return s.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f‪-‮⁦-⁩]/g, "").slice(0, max);
}

/** Replaces project file names and doc paths (e.g. `docs/GRAMMAR.md`) that a model may echo from its context. */
export function scrubProjectRefs(s: string): string {
  return s
    .replace(/\(?\s*(?:see|check|inspect|refer to|consult)[^.\n]*?\b[\w\-./]+\.(?:md|ts|tsx|rs|toml|json|ya?ml|html|css)\b[^.\n]*\)?\.?/gi, "")
    .replace(/\b(?:[\w\-]+\/)*[\w\-]+\.(?:md|ts|tsx|rs|toml|json|ya?ml|html|css)\b(?:#[\w\-]+)?/gi, "")
    .replace(/[ \t]{2,}/g, " ")
    .replace(/\s+([.,;:])/g, "$1")
    .trim();
}

/** First `n` sentences of `s`, capped at `max` characters. */
export function brief(s: string, n: number, max: number): string {
  const parts = s.trim().match(/[^.!?]+[.!?]+(\s|$)|[^.!?]+$/g) ?? [s];
  const out = parts.slice(0, n).join("").trim();
  return out.length > max ? out.slice(0, max - 1).trimEnd() + "…" : out;
}

export function extractJson(text: string): unknown {
  const t = text.trim();
  try {
    return JSON.parse(t);
  } catch {
    /* fall through to lenient extraction */
  }
  const fenced = /```(?:json)?\s*([\s\S]*?)```/i.exec(t);
  if (fenced) {
    try {
      return JSON.parse(fenced[1].trim());
    } catch {
      /* continue */
    }
  }
  const a = t.indexOf("{");
  const b = t.lastIndexOf("}");
  if (a >= 0 && b > a) {
    try {
      return JSON.parse(t.slice(a, b + 1));
    } catch {
      /* give up */
    }
  }
  throw new Error("The model did not return JSON");
}

function isObj(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/**
 * Validates a model answer. `allowedSources` is the set of source ids supplied to the model; any reference or
 * language-rule source outside it is dropped (and reported in `limitations`) rather than shown as fact.
 */
export function validateResult(raw: unknown, expected: AssistantResult["type"], allowedSources: Set<string>): Validation {
  if (!isObj(raw)) return { ok: false, error: "response is not an object" };
  const type = raw.type;
  if (typeof type !== "string" || !(TYPES as readonly string[]).includes(type)) return { ok: false, error: "invalid 'type'" };
  if (type !== expected) return { ok: false, error: `unexpected response type '${type}'` };
  if (typeof raw.summary !== "string" || !raw.summary.trim()) return { ok: false, error: "missing 'summary'" };
  if (typeof raw.explanation !== "string") return { ok: false, error: "missing 'explanation'" };

  const limitations: string[] = [];
  if (raw.limitations !== undefined) {
    if (!Array.isArray(raw.limitations) || !raw.limitations.every((x) => typeof x === "string")) {
      return { ok: false, error: "invalid 'limitations'" };
    }
    limitations.push(...raw.limitations.slice(0, 6).map((x) => cleanText(x as string, 400)));
  }

  const suggestions: Suggestion[] = [];
  if (raw.suggestions !== undefined) {
    if (!Array.isArray(raw.suggestions)) return { ok: false, error: "invalid 'suggestions'" };
    for (const s of raw.suggestions.slice(0, 3)) {
      if (!isObj(s)) return { ok: false, error: "invalid suggestion" };
      const { title, description, before, after, confidence } = s;
      if (typeof title !== "string" || typeof description !== "string" || typeof before !== "string" || typeof after !== "string") {
        return { ok: false, error: "invalid suggestion fields" };
      }
      const conf = typeof confidence === "string" && (CONF as readonly string[]).includes(confidence) ? (confidence as Confidence) : "low";
      suggestions.push({
        title: cleanText(title, 160),
        description: cleanText(description, 1200),
        before: cleanText(before, 4000),
        after: cleanText(after, 4000),
        confidence: conf,
        compilerValidated: false,
      });
    }
  }

  const publicMode = expected !== "answer";
  let dropped = 0;
  const references: string[] = [];
  if (raw.references !== undefined) {
    if (!Array.isArray(raw.references) || !raw.references.every((x) => typeof x === "string")) {
      return { ok: false, error: "invalid 'references'" };
    }
    for (const r of raw.references.slice(0, 8) as string[]) {
      if (allowedSources.has(r)) references.push(r);
      else dropped++;
    }
  }

  let languageRule: AssistantResult["languageRule"] = null;
  if (raw.languageRule !== undefined && raw.languageRule !== null) {
    if (!isObj(raw.languageRule) || typeof raw.languageRule.title !== "string" || typeof raw.languageRule.source !== "string") {
      return { ok: false, error: "invalid 'languageRule'" };
    }
    const title = cleanText(raw.languageRule.title, 200).trim();
    const source = raw.languageRule.source;
    if (title) {
      if (allowedSources.has(source)) languageRule = { title, source };
      else {
        languageRule = { title, source: "" };
        if (source.trim()) dropped++;
      }
    }
  }
  if (publicMode) {
    // The public UI shows a short explanation and a fix only: no documentation ids, rules or caveat lists.
    languageRule = null;
    references.length = 0;
    limitations.length = 0;
    dropped = 0;
  }
  if (dropped > 0) limitations.push(`${dropped} source reference(s) from the model were removed because they do not match the documentation that was supplied.`);

  return {
    ok: true,
    value: {
      type: type as AssistantResult["type"],
      summary: publicMode ? brief(scrubProjectRefs(cleanText(raw.summary, 400)), 2, 240) : cleanText(raw.summary, 400),
      explanation: publicMode ? brief(scrubProjectRefs(cleanText(raw.explanation, 4000)), 2, 320) : cleanText(raw.explanation, 4000),
      languageRule,
      suggestions: publicMode
        ? suggestions.slice(0, expected === "error_explanation" ? 1 : 2).map((s) => ({ ...s, title: scrubProjectRefs(s.title), description: brief(scrubProjectRefs(s.description), 2, 240) }))
        : suggestions,
      limitations,
      references,
    },
  };
}
