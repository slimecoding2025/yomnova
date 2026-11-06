import assert from "node:assert/strict";
import { test } from "node:test";
import { extractJson, jsonSchemaFor, validateResult } from "../../server/ai/schema.ts";

const SRC = new Set(["docs/LANGUAGE_SPECIFICATION.md#3. Types"]);
const good = () => ({
  type: "error_explanation", summary: "s", explanation: "e",
  languageRule: { title: "Types", source: "docs/LANGUAGE_SPECIFICATION.md#3. Types" },
  suggestions: [{ title: "t", description: "d", before: "a", after: "b", confidence: "high", compilerValidated: true }],
  limitations: [], references: ["docs/LANGUAGE_SPECIFICATION.md#3. Types", "docs/MADE_UP.md#x"],
});

test("accepts a valid answer, forces compilerValidated=false and drops unknown references", () => {
  const v = validateResult(good(), "error_explanation", SRC);
  assert.ok(v.ok);
  if (!v.ok) return;
  assert.equal(v.value.suggestions[0].compilerValidated, false);
  assert.deepEqual(v.value.references, [], "public answers carry no documentation references");
  assert.deepEqual(v.value.limitations, []);
});

test("public answers carry no language rule; free-form answers keep their rule handling", () => {
  const g = good();
  g.languageRule.source = "docs/FAKE.md#y";
  const v = validateResult(g, "error_explanation", SRC);
  assert.ok(v.ok && v.value.languageRule === null);
});

test("rejects malformed or unexpected structures", () => {
  assert.equal(validateResult("x", "answer", SRC).ok, false);
  assert.equal(validateResult({ ...good(), type: "nope" }, "error_explanation", SRC).ok, false);
  assert.equal(validateResult({ ...good(), type: "answer" }, "error_explanation", SRC).ok, false);
  assert.equal(validateResult({ ...good(), summary: "" }, "error_explanation", SRC).ok, false);
  assert.equal(validateResult({ ...good(), suggestions: "x" }, "error_explanation", SRC).ok, false);
  assert.equal(validateResult({ ...good(), suggestions: [{ title: 1 }] }, "error_explanation", SRC).ok, false);
  assert.equal(validateResult({ ...good(), references: [1] }, "error_explanation", SRC).ok, false);
});

test("caps lengths, removes control characters, defaults bad confidence to low", () => {
  const g: any = good();
  g.explanation = "a\u0000b‮" + "x".repeat(9000);
  g.suggestions[0].confidence = "certain";
  const v = validateResult(g, "error_explanation", SRC);
  assert.ok(v.ok);
  if (!v.ok) return;
  assert.ok(v.value.explanation.length <= 4000);
  assert.ok(!/[\u0000‮]/.test(v.value.explanation));
  assert.equal(v.value.suggestions[0].confidence, "low");
});

test("extractJson handles plain, fenced and embedded JSON and rejects prose", () => {
  assert.deepEqual(extractJson('{"a":1}'), { a: 1 });
  assert.deepEqual(extractJson('```json\n{"a":2}\n```'), { a: 2 });
  assert.deepEqual(extractJson('Sure! {"a":3} hope that helps'), { a: 3 });
  assert.throws(() => extractJson("no json here"));
});

test("json schema is strict and lists every property as required", () => {
  const s = jsonSchemaFor("fix_suggestions");
  assert.equal(s.strict, true);
  assert.deepEqual([...s.schema.required].sort(), Object.keys(s.schema.properties).sort());
});
