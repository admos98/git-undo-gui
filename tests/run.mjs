#!/usr/bin/env node
/**
 * Test entry point.
 *
 *   npm test              # every suite
 *   npm test -- parsers   # filter by file name
 *
 * Each suite imports the shipped index.html into jsdom, so the tests describe
 * the artifact the user actually double-clicks.
 */
import { readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { report } from "./harness.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const filter = process.argv[2];
const files = readdirSync(here)
  .filter((f) => f.endsWith(".test.mjs"))
  .filter((f) => !filter || f.includes(filter))
  .sort();

const results = [];
for (const file of files) {
  try {
    const mod = await import(`file://${resolve(here, file)}`);
    if (mod.default) results.push(mod.default);
  } catch (err) {
    results.push({ name: file, rows: [{ ok: false, label: "suite crashed", detail: err.message }], failed: 1 });
  }
}

const ok = report(results);
process.exit(ok ? 0 : 1);
