#!/usr/bin/env node
/**
 * Keeps the desktop build's copy of the UI in sync with the shipped one.
 *
 * tauri::generate_context! embeds two files at compile time: dist/index.html
 * (the window's URL) and src-tauri/assets/index.html (the page injected into
 * that window). Same bytes, two locations — a classic silent-divergence trap:
 * edit one and the desktop app quietly runs last week's UI.
 *
 * This copies dist over the embedded copy so there is exactly one source of
 * truth and no hand-sync step to forget. It runs as part of `npm run build`'s
 * Tauri path and inside CI before cargo compiles.
 *
 *   node scripts/embed-web.mjs           # sync
 *   node scripts/embed-web.mjs --check   # verify only (used by tests/CI)
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SOURCE = resolve(ROOT, "dist", "index.html");
const TARGET = resolve(ROOT, "src-tauri", "assets", "index.html");
const checkOnly = process.argv.includes("--check");

if (!existsSync(SOURCE)) {
  console.error(`embed-web: ${SOURCE} is missing — run \`npm run build\` first.`);
  process.exit(1);
}

const source = readFileSync(SOURCE);
const same = existsSync(TARGET) && readFileSync(TARGET).equals(source);

if (checkOnly) {
  if (!same) {
    console.error("embed-web: src-tauri/assets/index.html differs from dist/index.html — run `node scripts/embed-web.mjs`.");
    process.exit(1);
  }
  console.log("embed-web: embedded copy matches dist/index.html");
  process.exit(0);
}

if (same) {
  console.log("embed-web: already in sync");
  process.exit(0);
}

mkdirSync(dirname(TARGET), { recursive: true });
copyFileSync(SOURCE, TARGET);
console.log(`embed-web: copied ${source.length} bytes into src-tauri/assets/index.html`);
