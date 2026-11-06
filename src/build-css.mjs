/**
 * Compiles src/tailwind.css and inlines it into index.html.
 *
 *   npm run css:build   # compile + inline
 *
 * The app is a single double-clickable index.html: no CDN <script>, no runtime
 * CSS request. Run this after changing markup/classes.
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const htmlPath = resolve(root, "index.html");
const START = "/* TAILWIND_INLINE_START */";
const END = "/* TAILWIND_INLINE_END */";

const css = execFileSync(
  process.platform === "win32" ? "npx.cmd" : "npx",
  ["@tailwindcss/cli", "-i", "src/tailwind.css", "--minify"],
  { cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }
).trim();

const html = readFileSync(htmlPath, "utf8");
const startIdx = html.indexOf(START);
const endIdx = html.indexOf(END);

if (startIdx === -1 || endIdx === -1) {
  throw new Error(`Missing inline markers in index.html (${START} / ${END})`);
}

const next =
  html.slice(0, startIdx + START.length) +
  "\n" + css + "\n" +
  html.slice(endIdx);

writeFileSync(htmlPath, next, "utf8");
console.log(`Inlined ${(css.length / 1024).toFixed(1)} KB of Tailwind CSS into index.html`);
