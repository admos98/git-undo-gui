#!/usr/bin/env node
/**
 * Restore the committed (Linux-generated) inline CSS into index.html.
 *
 * Why this exists: Tailwind's native Lightning CSS build rounds OKLab channel
 * values differently on Windows and Linux (`-.0133505` vs `-.0133506`). Running
 * `npm run build` on Windows therefore rewrites the inlined CSS region with
 * Windows bytes, and CI's `git diff --exit-code -- index.html` step — which owns
 * byte-for-byte reproducibility on Linux — then fails on a clean checkout.
 *
 * The suite tolerates the difference (tests/css-equivalence.mjs compares numbers
 * within 1e-6), but the reproducibility gate does not: it is a byte comparison on
 * purpose, because the artifact is the product.
 *
 * Usage:
 *   node scripts/restore-linux-css.mjs            # from HEAD (default)
 *   node scripts/restore-linux-css.mjs <rev>      # from a specific revision
 *
 * Then re-measure the badge, because the byte count changed:
 *   node scripts/sync-badge.mjs
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const INDEX = resolve(ROOT, "index.html");
const START = "/* TAILWIND_INLINE_START */";
const END = "/* TAILWIND_INLINE_END */";

const rev = process.argv[2] || "HEAD";

const commitHtml = execFileSync("git", ["show", `${rev}:index.html`], {
  cwd: ROOT,
  encoding: "utf8",
  maxBuffer: 32 * 1024 * 1024,
});

function cssRegion(html, label) {
  const start = html.indexOf(START);
  const end = html.indexOf(END);
  if (start === -1 || end === -1) throw new Error(`${label}: inline markers not found`);
  return html.slice(start, end);
}

const committed = cssRegion(commitHtml, rev);
const current = readFileSync(INDEX, "utf8");
const currentCss = cssRegion(current, "index.html");

if (committed === currentCss) {
  console.log(`restore-linux-css: already identical to ${rev}`);
  process.exit(0);
}

const next = current.replace(currentCss, committed);
writeFileSync(INDEX, next, "utf8");
console.log(
  `restore-linux-css: replaced ${currentCss.length} bytes of inline CSS with the ` +
  `${committed.length} bytes from ${rev} (delta ${committed.length - currentCss.length})`
);
console.log("Now run: node scripts/sync-badge.mjs");
