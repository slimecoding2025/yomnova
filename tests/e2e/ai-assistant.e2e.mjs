/**
 * End-to-end test of the AI Assistant: real browser -> real server -> real `yomnova` binary -> MOCK OpenRouter.
 * It does NOT call the real OpenRouter and therefore proves nothing about live provider behaviour.
 *
 * Needs: `npm run build` (dist/), the compiler built (or YOMNOVA_BIN), and Playwright (`npm i -D playwright`).
 *   node tests/e2e/ai-assistant.e2e.mjs
 */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { startMock } from "./mock-openrouter.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SHOTS = process.env.E2E_SHOTS ?? "";
const KEY = "sk-or-e2e-SECRET-KEY";

function startServer(port, env) {
  const p = spawn(process.platform === "win32" ? "npx.cmd" : "npx", ["tsx", "server.ts", "--prod"], { cwd: ROOT, env: { ...process.env, PORT: String(port), ...env }, stdio: ["ignore", "pipe", "pipe"], detached: process.platform !== "win32" });
  let out = "";
  p.stdout.on("data", (d) => (out += d));
  p.stderr.on("data", (d) => (out += d));
  return new Promise((resolve, reject) => {
    const t = setInterval(() => { if (out.includes("http://localhost")) { clearInterval(t); resolve({ stop: () => { try { process.platform === "win32" ? p.kill() : process.kill(-p.pid); } catch { /* already stopped */ } }, out: () => out }); } }, 100);
    setTimeout(() => { clearInterval(t); reject(new Error("server did not start: " + out)); }, 20000);
  });
}

const results = [];
const step = async (name, fn) => { await fn(); results.push(name); console.log("ok -", name); };

const mock = await startMock(4010);
const on = await startServer(3112, { OPENROUTER_API_KEY: KEY, OPENROUTER_BASE_URL: "http://127.0.0.1:4010" });
const off = await startServer(3113, { OPENROUTER_API_KEY: "" });
const browser = await chromium.launch();
const errors = [];
try {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 1000 } })).newPage();
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  await page.goto("http://localhost:3112/#playground", { waitUntil: "networkidle" });
  await page.evaluate(() => document.getElementById("playground").scrollIntoView());
  const editor = page.locator(".pg-code textarea");
  const panel = page.locator(".pg-ai");

  await step("assistant panel opens and lists models from the provider", async () => {
    assert.equal(await panel.count(), 0);
    await page.getByRole("button", { name: "AI Assistant" }).click();
    await panel.waitFor();
    await page.locator("#ai-model-select option", { hasText: "Mock Alpha" }).waitFor({ state: "attached" });
    assert.equal(await page.locator("#ai-model-select").inputValue(), "", "no model is chosen for the user");
  });

  await page.selectOption('select[aria-label="Example program"]', "type_error");
  const original = await editor.inputValue();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("button", { name: "Explain with YOMNOVA AI" }).waitFor();

  await step("explain without a model asks the user to choose one (no silent default)", async () => {
    await page.getByRole("button", { name: "Explain with YOMNOVA AI" }).click();
    await panel.getByText("Choose a model first.").waitFor();
  });

  await step("Explain Error: real diagnostic + validated structured answer + suggestion shown", async () => {
    await page.selectOption("#ai-model-select", "mockco/alpha");
    await page.getByRole("button", { name: "Explain with YOMNOVA AI" }).click();
    await panel.getByText("The argument has the wrong type.").waitFor();
    const diag = await panel.locator(".ai-diag").first().innerText();
    assert.match(diag, /Type mismatch/);
    assert.match(diag, /twenty-one/);
    await panel.locator(".ai-status .warn").getByText("Not compiler-validated").waitFor();
    const call = mock.log.at(-1);
    assert.equal(call.auth, `Bearer ${KEY}`, "the server (not the browser) authenticates to the provider");
    assert.match(call.user, /Type mismatch/);
    assert.match(call.user, /<knowledge id="docs\//);
    assert.equal(await editor.inputValue(), original, "editor untouched");
  });

  await step("suggestion: view, diff, copy, validate with the real compiler; editor never changes", async () => {
    await panel.getByRole("button", { name: "View suggestion" }).click();
    assert.match(await panel.locator(".ai-code").last().innerText(), /double\(21\)/);
    await panel.getByRole("button", { name: "Review diff" }).click();
    const diff = await panel.locator(".ai-diff").innerText();
    assert.match(diff, /^- .*twenty-one/m);
    assert.match(diff, /^\+ .*double\(21\)/m);
    await panel.getByRole("button", { name: "Copy", exact: true }).click();
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().endsWith("/api/ai/validate-suggestion")),
      panel.getByRole("button", { name: "Validate with compiler" }).click(),
    ]);
    const body = await resp.json();
    assert.equal(body.validated, true);
    await panel.getByText("Compiler check passed on a copy of your code").waitFor();
    assert.equal(await editor.inputValue(), original, "editor still untouched after validation");
    assert.match(await editor.inputValue(), /twenty-one/);
  });

  await step("validation reports a real compiler failure when the suggestion is wrong", async () => {
    const bad = mock.log.length;
    await page.locator('select[aria-label="Assistant mode"]').selectOption("chat");
    // Edit the user's code so that 'before' no longer matches: validation must refuse, not guess.
    await editor.fill(original.replace('double("twenty-one")', 'double("other")'));
    await panel.getByRole("button", { name: "Validate with compiler" }).click();
    await panel.getByText(/Cannot validate: .*not found in the editor/).waitFor();
    await editor.fill(original);
    assert.ok(mock.log.length >= bad);
  });

  await step("Suggest a fix returns fix suggestions with compilerValidated=false", async () => {
    await page.getByRole("button", { name: "Run", exact: true }).click();
    await page.getByRole("button", { name: "Suggest a fix" }).click();
    await page.waitForFunction(() => document.querySelectorAll(".ai-sug").length >= 2);
    assert.match(mock.log.at(-1).system, /with type "fix_suggestions"/);
  });

  await step("Explain selected code streams a plain-text answer and sends only the selection", async () => {
    // Select with the keyboard, as a user would: Ctrl+Home, then Shift+Down three times.
    await editor.click();
    await page.keyboard.press("Control+Home");
    for (let i = 0; i < 3; i++) await page.keyboard.press("Shift+ArrowDown");
    const picked = await page.evaluate(() => {
      const t = document.querySelector(".pg-code textarea");
      return t.value.slice(t.selectionStart, t.selectionEnd);
    });
    assert.ok(picked.length > 5);
    await page.getByRole("button", { name: "Explain selection" }).click();
    await panel.getByText("are declared with").waitFor();
    const call = mock.log.at(-1);
    assert.equal(call.stream, true);
    assert.ok(call.system.includes(picked.trim().split("\n")[0]), "selected text was sent");
    assert.match(call.system, /<untrusted-selection>/);
    assert.ok(!call.system.includes("<untrusted-code>"), "whole program was not sent");
    assert.equal(await editor.inputValue(), original);
  });

  await step("changing model keeps the conversation and announces the change", async () => {
    const before = await panel.locator(".ai-msg").count();
    await page.selectOption("#ai-model-select", "mockco/beta");
    await panel.getByText("Model changed to mockco/beta. The conversation is kept.").waitFor();
    assert.equal(await panel.locator(".ai-msg").count(), before);
  });

  await step("cancel stops a streaming answer", async () => {
    await panel.locator("textarea").fill("SLOW please");
    await panel.getByRole("button", { name: "Send" }).click();
    await panel.getByRole("button", { name: "Cancel" }).waitFor();
    await page.waitForTimeout(500);
    await panel.getByRole("button", { name: "Cancel" }).click();
    await panel.getByText("Cancelled.").waitFor();
    const len = (await panel.locator(".ai-a").last().innerText()).length;
    await page.waitForTimeout(600);
    assert.equal((await panel.locator(".ai-a").last().innerText()).length, len, "no more text after cancel");
  });

  await step("model output is rendered as inert text (no HTML injection)", async () => {
    assert.equal(await panel.locator("script, img, iframe").count(), 0);
  });

  await step("assistant panel closes and the editor is still intact", async () => {
    await panel.getByRole("button", { name: "Collapse the AI Assistant panel" }).click();
    assert.equal(await panel.count(), 0);
    assert.equal(await editor.inputValue(), original);
  });

  await step("API key is absent from the page and bundles", async () => {
    const html = await page.content();
    assert.ok(!html.includes(KEY));
    const js = await (await page.request.get("http://localhost:3112/assets/app.js")).text();
    assert.ok(!js.includes(KEY) && !js.includes("OPENROUTER_API_KEY=sk"));
  });

  if (SHOTS) {
    await page.getByRole("button", { name: "AI Assistant" }).click();
    await page.getByRole("button", { name: "Explain with YOMNOVA AI" }).click();
    await page.getByText("The argument has the wrong type.").first().waitFor();
    await page.locator(".pg").screenshot({ path: path.join(SHOTS, "assistant-desktop.png") });
    await page.setViewportSize({ width: 390, height: 900 });
    await page.locator(".pg").screenshot({ path: path.join(SHOTS, "assistant-mobile.png") });
  }

  await step("server without a key: panel explains how to enable it; no model calls possible", async () => {
    const p2 = await (await browser.newContext({ viewport: { width: 1440, height: 1000 } })).newPage();
    await p2.goto("http://localhost:3113/#playground", { waitUntil: "networkidle" });
    await p2.getByRole("button", { name: "AI Assistant" }).click();
    await p2.getByText("AI Assistant is off.").waitFor();
    assert.equal(await p2.locator("#ai-model-select").count(), 0);
    const r = await p2.request.post("http://localhost:3113/api/ai/suggest-fix", { data: { model: "x/y", code: "x" } });
    assert.equal(r.status(), 503);
  });

  await step("no console or page errors", async () => assert.deepEqual(errors, []));
  console.log(`\n${results.length} e2e checks passed (mock provider; live OpenRouter not exercised)`);
} catch (e) {
  console.error("\nE2E FAILED:", e);
  console.error("server log:\n" + on.out().slice(-1500));
  process.exitCode = 1;
} finally {
  await browser.close();
  on.stop(); off.stop(); mock.close();
}
