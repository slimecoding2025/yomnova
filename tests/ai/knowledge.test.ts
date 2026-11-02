import assert from "node:assert/strict";
import { test } from "node:test";
import { chunkMarkdown, KnowledgeBase } from "../../server/ai/knowledge.ts";
import { ROOT } from "./helpers.ts";

const kb = KnowledgeBase.load(ROOT);

test("knowledge base is built from the repository docs and examples", () => {
  assert.ok(kb.chunks.some((c) => c.id.startsWith("docs/LANGUAGE_SPECIFICATION.md#")));
  assert.ok(kb.chunks.some((c) => c.id.startsWith("docs/GRAMMAR.md#")));
  assert.ok(kb.chunks.some((c) => c.kind === "example" && c.id === "yomnova/examples/hello.yom"));
});

test("retrieval finds the ownership section for an ownership question", () => {
  const hits = kb.retrieve("why was my value moved ownership", { limit: 3 });
  assert.ok(hits.length > 0);
  assert.ok(hits.some((h) => /Ownership/i.test(h.title)), JSON.stringify(hits.map((h) => h.id)));
});

test("retrieval finds types for a type question and pipelines for a pipeline question", () => {
  assert.ok(kb.retrieve("what types exist Int Float Text", { limit: 3 }).some((h) => /Types/.test(h.title)));
  assert.ok(kb.retrieve("pipeline flow stage", { limit: 3 }).some((h) => /Pipelines/.test(h.title)));
});

test("retrieval honours character and chunk limits and returns nothing for empty queries", () => {
  const hits = kb.retrieve("types statements expressions ownership", { limit: 10, maxChars: 1500 });
  assert.ok(hits.reduce((a, h) => a + h.text.length, 0) <= 1500 || hits.length === 1);
  assert.deepEqual(kb.retrieve("   the a of "), []);
});

test("intentionally invalid examples are labelled as such", () => {
  const bad = kb.chunks.find((c) => c.id === "yomnova/examples/type_error.yom");
  assert.ok(bad?.text.includes("intentionally INVALID"));
});

test("markdown chunking ignores headings inside code fences", () => {
  const chunks = chunkMarkdown("x.md", "# A\ntext\n```\n# not a heading\n```\n## B\nmore");
  assert.deepEqual(chunks.map((c) => c.title), ["A", "B"]);
});

test("a type-mismatch diagnostic retrieves the specification's type rules ahead of README prose", () => {
  const hits = kb.retrieve("Type mismatch at 9:17: expected Int, found Text", { limit: 3 });
  assert.ok(hits.some((h) => h.id.startsWith("docs/LANGUAGE_SPECIFICATION.md#") && /Types|Expressions|Diagnostics/.test(h.title)), JSON.stringify(hits.map((h) => h.id)));
  assert.ok(hits[0].id.startsWith("docs/"), "specification/docs rank first");
});
