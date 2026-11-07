/**
 * Shared test harness.
 *
 * The app is a single self-contained index.html that talks to the DOM directly,
 * so the tests load that exact file into jsdom — no build step, no bundler, no
 * test framework. Anything that passes here exercises the shipped artifact.
 */
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { JSDOM, VirtualConsole } from "jsdom";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const INDEX_PATH = resolve(ROOT, "index.html");
export const HTML = readFileSync(INDEX_PATH, "utf8");

/** Boot the app in jsdom with optional pre-seeded localStorage. */
export async function boot(storage = {}, opts = {}) {
  const errors = [];
  const alerts = [];
  const virtualConsole = new VirtualConsole();
  virtualConsole.on("jsdomError", (e) => errors.push(`jsdom: ${e.message}`));

  const dom = new JSDOM(HTML, {
    url: opts.url || "https://git-undo.test/",
    runScripts: "dangerously",
    pretendToBeVisual: true,
    virtualConsole,
  });

  const w = dom.window;
  const d = w.document;

  // jsdom gaps: rAF timing, canvas 2d, alert
  w.requestAnimationFrame = (cb) => w.setTimeout(() => cb(Date.now()), 0);
  w.cancelAnimationFrame = (id) => w.clearTimeout(id);
  w.HTMLCanvasElement.prototype.getContext = () => new Proxy({}, { get: () => () => {}, set: () => true });
  w.alert = (...args) => alerts.push(args.join(" "));

  for (const [k, v] of Object.entries(storage)) {
    try { w.localStorage.setItem(k, v); } catch (e) { /* blocked storage is a test case in itself */ }
  }

  d.dispatchEvent(new w.Event("DOMContentLoaded", { bubbles: true }));
  await wait(w, opts.settle ?? 40);

  /** Evaluate an expression inside the app's own scope. */
  const app = (expr) => w.eval(expr);
  const $ = (sel) => d.querySelector(sel);
  const $$ = (sel) => Array.from(d.querySelectorAll(sel));
  const press = (key, el = d.body, init = {}) =>
    el.dispatchEvent(new w.KeyboardEvent("keydown", { key, bubbles: true, ...init }));
  const open = (id) => !d.getElementById(id).classList.contains("hidden");
  const text = (id) => d.getElementById(id).textContent.trim();

  return { dom, w, d, app, $, $$, press, open, text, errors, alerts };
}

export function wait(w, ms = 0) {
  return new Promise((resolve) => w.setTimeout(resolve, ms));
}

/** Minimal assertion + reporting harness. */
export function createSuite(name) {
  const results = [];
  const test = (label, fn) => results.push({ label, fn });
  async function run() {
    const rows = [];
    for (const { label, fn } of results) {
      try {
        // Contract: return true (or nothing) to pass; return a string or false
        // to fail — the string is the diagnostic. This keeps assertions honest:
        // `return cond || "why"` can never silently pass.
        const result = await fn();
        if (result === true || result === undefined) {
          rows.push({ ok: true, label, detail: "" });
        } else if (result === false) {
          rows.push({ ok: false, label, detail: "returned false" });
        } else if (typeof result === "string") {
          rows.push({ ok: false, label, detail: result });
        } else {
          rows.push({ ok: true, label, detail: "" });
        }
      } catch (err) {
        rows.push({ ok: false, label, detail: err.message });
      }
    }
    return { name, rows, failed: rows.filter((r) => !r.ok).length };
  }
  return { test, run };
}

export function report(suites) {
  let failed = 0;
  let total = 0;
  for (const s of suites) {
    console.log(`\n\x1b[1m${s.name}\x1b[0m`);
    for (const r of s.rows) {
      total++;
      if (!r.ok) failed++;
      const mark = r.ok ? "\x1b[32m✓\x1b[0m" : "\x1b[31m✗\x1b[0m";
      console.log(`  ${mark} ${r.label}${r.detail ? `\n      ${r.detail}` : ""}`);
    }
  }
  const passed = total - failed;
  const colour = failed ? "\x1b[31m" : "\x1b[32m";
  console.log(`\n${colour}${passed}/${total} checks passed\x1b[0m\n`);
  return failed === 0;
}
