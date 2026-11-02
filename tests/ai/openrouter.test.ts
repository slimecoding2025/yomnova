import assert from "node:assert/strict";
import { test } from "node:test";
import { AiError, mapModel } from "../../server/ai/openrouter.ts";
import { completion, json, KEY, makeClient, mockUpstream, MODELS, sse } from "./helpers.ts";

const msgs = [{ role: "user" as const, content: "hi" }];

test("model registry: maps, filters non-text models, sorts, caches", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const models = await c.listModels();
  assert.deepEqual(models.map((m) => m.id), ["acme/alpha", "acme/beta-json", "other/plain"]);
  assert.equal(models[0].structuredOutputs, true);
  assert.equal(models[1].structuredOutputs, false);
  assert.equal(models[1].jsonMode, true);
  assert.equal(models[0].provider, "acme");
  assert.equal(models[0].contextLength, 32000);
  await c.listModels();
  assert.equal(up.calls.filter((x) => x.url.endsWith("/models")).length, 1, "second call is served from cache");
  assert.equal(mapModel({ id: 5 } as never), null);
  assert.ok(MODELS.length === 4);
});

test("model registry: allowlist restricts the list; unknown model ids are rejected", async () => {
  const c = makeClient(mockUpstream(), { AI_MODEL_ALLOWLIST: "acme/" });
  assert.deepEqual((await c.listModels()).map((m) => m.id), ["acme/alpha", "acme/beta-json"]);
  await assert.rejects(c.requireModel("other/plain"), (e: AiError) => e.code === "invalid_model" && e.status === 400);
  await assert.rejects(c.requireModel("img/gen"), (e: AiError) => e.code === "invalid_model");
  assert.equal((await c.requireModel("acme/alpha")).id, "acme/alpha");
});

test("request construction: auth header, structured vs json vs plain, token cap", async () => {
  const up = mockUpstream();
  const c = makeClient(up, { AI_MAX_OUTPUT_TOKENS: "900" });
  const [alpha, beta, plain] = await c.listModels();
  const schema = { name: "x", strict: true, schema: {} };

  up.next(() => completion("ok"));
  await c.complete({ model: alpha, messages: msgs, structured: schema });
  await c.complete({ model: beta, messages: msgs, structured: schema });
  await c.complete({ model: plain, messages: msgs, structured: schema });
  const posts = up.calls.filter((x) => x.method === "POST");
  assert.equal(posts[0].headers.Authorization, `Bearer ${KEY}`);
  assert.equal(posts[0].body.response_format.type, "json_schema");
  assert.equal(posts[1].body.response_format.type, "json_object");
  assert.equal(posts[2].body.response_format, undefined);
  assert.equal(posts[0].body.stream, false);
  assert.equal(posts[0].body.max_tokens, 900);
  assert.equal(posts[0].body.model, "acme/alpha");
  assert.equal(posts[0].headers["X-Title"], "YOMNOVA Playground");
});

test("missing API key: not configured, and no network call is made", async () => {
  const up = mockUpstream();
  const c = makeClient(up, { OPENROUTER_API_KEY: "" });
  assert.equal(c.configured, false);
  const info = { id: "acme/alpha", name: "", provider: "", description: "", contextLength: null, structuredOutputs: false, jsonMode: false, maxCompletionTokens: null };
  await assert.rejects(c.complete({ model: info, messages: msgs }), (e: AiError) => e.code === "not_configured" && e.status === 503);
  await assert.rejects(c.stream({ model: info, messages: msgs }, () => {}), (e: AiError) => e.code === "not_configured");
  assert.equal(up.calls.length, 0);
});

test("provider errors are mapped and never leak the API key", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  const cases: Array<[number, string]> = [[401, "provider_auth"], [402, "provider_credits"], [429, "rate_limited"], [404, "invalid_model"], [500, "provider_error"]];
  for (const [status, code] of cases) {
    up.next(() => json(status, { error: { message: `boom ${KEY}`.replace(KEY, "[x]") } }));
    await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => {
      assert.equal(e.code, code);
      assert.ok(!e.message.includes(KEY));
      return true;
    });
  }
});

test("200 responses that carry an error body, empty content or garbage are failures", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  up.next(() => json(200, { error: { message: "upstream failed" } }));
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "provider_error");
  up.next(() => json(200, { choices: [{ message: { content: "" } }] }));
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "bad_response");
  up.next(() => new Response("<html>nope</html>", { status: 200 }));
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "bad_response");
  up.next(() => json(200, { choices: [{ error: { message: "mid error" } }] }));
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "provider_error");
});

test("timeout: a provider that never answers is aborted", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  c.cfg.timeoutMs = 80;
  up.next(() => new Promise<Response>(() => {})); // never resolves; abort is wired by the mock
  const t = Date.now();
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "timeout" && e.status === 504);
  assert.ok(Date.now() - t < 2000);
});

test("cancellation: aborting the caller's signal stops the request", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  up.next(() => new Promise<Response>(() => {}));
  const ctl = new AbortController();
  setTimeout(() => ctl.abort(new Error("aborted")), 30);
  await assert.rejects(c.complete({ model: alpha, messages: msgs, signal: ctl.signal }), (e: AiError) => e.code === "aborted");
});

test("network failure is reported as such", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  c.cfg.fetchImpl = (async () => { throw new TypeError("fetch failed"); }) as typeof fetch;
  await assert.rejects(c.complete({ model: alpha, messages: msgs }), (e: AiError) => e.code === "network");
});

test("streaming: assembles deltas, ignores comments, reports usage and finish reason", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  up.next(() => sse([
    ": OPENROUTER PROCESSING\n\n",
    'data: {"choices":[{"delta":{"content":"Hel"}}]}\n\n',
    'data: {"choices":[{"delta":{"content":"lo"}}]}\n\ndata: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":2}}\n\n',
    "data: [DONE]\n\n",
  ]));
  const parts: string[] = [];
  const out = await c.stream({ model: alpha, messages: msgs }, (t) => parts.push(t));
  assert.equal(parts.join(""), "Hello");
  assert.equal(out.finishReason, "stop");
  assert.deepEqual(out.usage, { promptTokens: 7, completionTokens: 2 });
  assert.equal(up.calls.at(-1)!.body.stream, true);
});

test("streaming: split chunks, mid-stream provider error, malformed event, empty stream", async () => {
  const up = mockUpstream();
  const c = makeClient(up);
  const [alpha] = await c.listModels();
  up.next(() => sse(['data: {"choices":[{"delta":{"con', 'tent":"ab"}}]}\n', "\n"]));
  const parts: string[] = [];
  await c.stream({ model: alpha, messages: msgs }, (t) => parts.push(t));
  assert.equal(parts.join(""), "ab");

  up.next(() => sse(['data: {"choices":[{"delta":{"content":"x"}}]}\n\n', 'data: {"error":{"message":"overloaded"}}\n\n']));
  await assert.rejects(c.stream({ model: alpha, messages: msgs }, () => {}), (e: AiError) => e.code === "provider_error");
  up.next(() => sse(["data: {not json}\n\n"]));
  await assert.rejects(c.stream({ model: alpha, messages: msgs }, () => {}), (e: AiError) => e.code === "bad_response");
  up.next(() => sse(["data: [DONE]\n\n"]));
  await assert.rejects(c.stream({ model: alpha, messages: msgs }, () => {}), (e: AiError) => e.code === "bad_response");
});
