/**
 * Phone-layout e2e (iPhone 13 emulation): code is never clipped and the scroll story advances with touch scrolling.
 * Needs: `npm run build` (dist/), the compiler (or YOMNOVA_BIN) and Playwright.
 *   node tests/e2e/mobile.e2e.mjs
 */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium, devices } = require("playwright");

const PORT = 3122;
const server = spawn("npx", ["tsx", "server.ts", "--prod"], { env: { ...process.env, PORT: String(PORT) }, stdio: "ignore", detached: true });
const stop = () => { try { process.kill(-server.pid); } catch {} };
process.on("exit", stop);
for (let i = 0; i < 60; i++) { try { if ((await fetch(`http://localhost:${PORT}/api/version`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 500)); }

const browser = await chromium.launch();
const page = await (await browser.newContext({ ...devices["iPhone 13"] })).newPage();
await page.goto(`http://localhost:${PORT}/`, { waitUntil: "networkidle" });
const top = (id) => page.evaluate((i) => document.querySelector("#" + i).getBoundingClientRect().top + scrollY, id);
const walk = async (id, read, span = 2400) => {
  const t = await top(id);
  const seen = new Set();
  for (let y = t; y < t + span; y += 100) { await page.evaluate((v) => scrollTo(0, v), y); await page.waitForTimeout(100); seen.add(await page.evaluate(read)); }
  return seen;
};
let n = 0;
const ok = (m) => console.log(`ok ${++n} - ${m}`);

await page.evaluate(() => document.querySelector("#ownership").scrollIntoView());
await page.waitForTimeout(500);
const clipped = await page.evaluate(() => [...document.querySelectorAll(".codeblock, .own-code pre")].filter((e) => e.scrollWidth > e.clientWidth + 1 || e.getBoundingClientRect().right > innerWidth));
assert.equal(clipped.length, 0, "code blocks must fit the phone width");
ok("no code block is wider than the screen");
assert.equal(await page.evaluate(() => document.documentElement.scrollWidth), await page.evaluate(() => innerWidth));
ok("no horizontal page overflow");

const own = await walk("ownership", () => document.querySelector(".own-step")?.textContent?.slice(0, 20));
assert.ok(own.size >= 3, `ownership should show 3 steps, saw ${[...own]}`);
ok("ownership steps advance while scrolling");
const proto = await walk("protocol", () => document.querySelector(".hdr-caption")?.textContent?.slice(0, 25));
assert.ok(proto.size >= 3, `protocol steps: ${[...proto]}`);
ok("protocol steps advance while scrolling");
const flow = await walk("dataflow", () => document.querySelectorAll(".lane .node.on").length);
assert.ok(flow.size >= 3, `dataflow steps: ${[...flow]}`);
ok("dataflow steps advance while scrolling");
const dom = await walk("domains", () => document.querySelector(".dom-row[data-spot] .t")?.textContent ?? "");
assert.ok(dom.size >= 5, `domains spotlight: ${[...dom]}`);
ok("domains spotlight follows the scroll");

await browser.close();
stop();
console.log(`${n} mobile checks passed`);
process.exit(0);
