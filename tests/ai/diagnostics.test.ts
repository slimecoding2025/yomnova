import assert from "node:assert/strict";
import { test } from "node:test";
import { parseDiagnostics, sourceWindow } from "../../server/ai/diagnostics.ts";
import { binary, realCompile, ROOT } from "./helpers.ts";
import fs from "node:fs";
import path from "node:path";

// Strings below are copied from real `yomnova check` output (see the real-compiler tests further down).
const TYPE = `main.yom:9:17: error: Type mismatch at 9:17: expected Int, found Text
    9 |     emit(double("twenty-one"))
      |                 ^^^^^^^^^^^^
`;
const OWN = `main.yom:10:10: error: Use of moved value 'msg'
   10 |     emit(msg)
      |          ^^^
  = hint: Value was previously moved at line 9
`;

test("parses a diagnostic with excerpt and no hint", () => {
  const [d] = parseDiagnostics(TYPE);
  assert.equal(d.severity, "error");
  assert.equal(d.line, 9);
  assert.equal(d.column, 17);
  assert.equal(d.hint, null);
  assert.match(d.message, /Type mismatch/);
  assert.equal(d.stage, "semantic");
  assert.match(d.excerpt, /twenty-one/);
  assert.equal(d.raw, TYPE.trim());
});

test("parses hints, and several diagnostics in one output", () => {
  const ds = parseDiagnostics(OWN + TYPE);
  assert.equal(ds.length, 2);
  assert.equal(ds[0].hint, "Value was previously moved at line 9");
  assert.equal(ds[0].stage, "ownership");
  assert.deepEqual(ds.map((d) => d.id), ["d1", "d2"]);
});

test("output without a location is preserved verbatim and nothing is invented", () => {
  const [d] = parseDiagnostics("error: step limit exceeded\n");
  assert.equal(d.line, null);
  assert.equal(d.column, null);
  assert.equal(d.raw, "error: step limit exceeded");
  assert.equal(d.stage, "runtime");
  assert.deepEqual(parseDiagnostics(""), []);
});

test("source window is bounded and numbered", () => {
  const code = Array.from({ length: 30 }, (_, i) => `l${i + 1}`).join("\n");
  const w = sourceWindow(code, 15, 2).split("\n");
  assert.equal(w.length, 5);
  assert.match(w[2], /15 \| l15/);
  assert.equal(sourceWindow(code, null), "");
});

test("real compiler: lexer, parser, semantic, type and ownership diagnostics parse", { skip: !binary() && "yomnova binary not built" }, async () => {
  const cases: Array<[string, RegExp, number]> = [
    ["fn main() -> Int {\n  let x = @\n  return 0\n}\n", /Unexpected character/, 2],
    ["fn main() -> Int {\n  let x = (1 +\n  return 0\n}\n", /Unexpected token/, 3],
    ["fn main() -> Int {\n  return y\n}\n", /Undefined identifier/, 2],
    [fs.readFileSync(path.join(ROOT, "yomnova/examples/type_error.yom"), "utf-8"), /Type mismatch/, 9],
    [fs.readFileSync(path.join(ROOT, "yomnova/examples/ownership_fail.yom"), "utf-8"), /moved value/, 10],
  ];
  for (const [code, re, line] of cases) {
    const r = await realCompile("check", code);
    assert.ok(r && !r.ok, "compiler should reject: " + code.slice(0, 30));
    const ds = parseDiagnostics(r!.stderr);
    assert.ok(ds.length >= 1);
    assert.match(ds[0].message, re);
    assert.equal(ds[0].line, line);
    assert.equal(ds[0].file, "main.yom");
  }
});

test("real compiler: valid code has no diagnostics", { skip: !binary() && "yomnova binary not built" }, async () => {
  const r = await realCompile("check", fs.readFileSync(path.join(ROOT, "yomnova/examples/hello.yom"), "utf-8"));
  assert.ok(r?.ok);
  assert.deepEqual(parseDiagnostics(r!.stderr), []);
});
