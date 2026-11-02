/**
 * TEST FIXTURE ONLY: a tiny local stand-in for the OpenRouter API, used by ai-assistant.e2e.mjs.
 * It returns canned, clearly labelled "[mock provider]" answers. It is never used by the application.
 */
import http from "node:http";

export function startMock(port) {
  const log = [];
  const server = http.createServer(async (req, res) => {
    const chunks = [];
    for await (const c of req) chunks.push(c);
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : null;
    if (req.url?.endsWith("/models")) {
      res.writeHead(200, { "Content-Type": "application/json" });
      return res.end(JSON.stringify({ data: [
        { id: "mockco/alpha", name: "Mock Alpha", context_length: 32000, supported_parameters: ["structured_outputs", "response_format"], architecture: { input_modalities: ["text"], output_modalities: ["text"] } },
        { id: "mockco/beta", name: "Mock Beta", context_length: 8000, supported_parameters: [], architecture: { input_modalities: ["text"], output_modalities: ["text"] } },
      ] }));
    }
    log.push({ auth: req.headers.authorization, stream: body?.stream, model: body?.model, system: body?.messages?.[0]?.content ?? "", user: body?.messages?.at(-1)?.content ?? "" });
    const user = body.messages.at(-1).content;
    if (body.stream) {
      res.writeHead(200, { "Content-Type": "text/event-stream" });
      const words = (/SLOW/.test(user) ? Array.from({ length: 40 }, (_, i) => `w${i} `) : ["[mock provider] ", "Functions ", "are ", "declared ", "with `fn`."]);
      for (const w of words) {
        if (res.destroyed) return;
        res.write(`data: ${JSON.stringify({ choices: [{ delta: { content: w } }] })}\n\n`);
        await new Promise((r) => setTimeout(r, /SLOW/.test(user) ? 150 : 20));
      }
      res.write('data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}\n\ndata: [DONE]\n\n');
      return res.end();
    }
    const src = /<knowledge id="([^"]+)"/.exec(user)?.[1] ?? "";
    const type = /with type "fix_suggestions"/.test(body.messages[0].content) ? "fix_suggestions" : "error_explanation";
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ choices: [{ message: { role: "assistant", content: JSON.stringify({
      type, summary: "[mock provider] The argument has the wrong type.",
      explanation: "The function expects an Int but the call passes Text.",
      languageRule: { title: "Types", source: src },
      suggestions: [{ title: "Pass an Int", description: "Replace the Text argument with an Int literal.", before: 'double("twenty-one")', after: "double(21)", confidence: "high" }],
      limitations: ["This is canned output from a test fixture."], references: [src],
    }) }, finish_reason: "stop" }], usage: { prompt_tokens: 120, completion_tokens: 60 } }));
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve({ log, close: () => { server.closeAllConnections(); server.close(); } })));
}
