#!/usr/bin/env node
/**
 * Rewrite the README size badge from the artifact itself.
 *
 * A number typed by hand goes stale the moment someone adds a feature — this
 * repo did exactly that twice (a v1 badge survived two prose corrections). So
 * the badge is no longer written, it is derived: run this after a build, and
 * tests/integrity.test.mjs fails if the committed README disagrees with the file.
 *
 *   node scripts/sync-badge.mjs          # update
 *   node scripts/sync-badge.mjs --check  # verify only
 */
import { readFileSync, writeFileSync, statSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const INDEX = resolve(ROOT, "index.html");
const README = resolve(ROOT, "README.md");

const raw = readFileSync(INDEX);
const kib = Math.round(raw.length / 1024);
const gzipKib = Math.round(zlib.gzipSync(raw, { level: 9 }).length / 1024);

const badge = `badge/index.html-${kib}%20KB%20raw%20%C2%B7%20${gzipKib}%20KB%20gzip`;
const readme = readFileSync(README, "utf8");
const pattern = /badge\/index\.html-[0-9]+%20KB%20raw%20%C2%B7%20[0-9]+%20KB%20gzip/;

if (!pattern.test(readme)) {
  console.error("sync-badge: no size badge found in README.md");
  process.exit(1);
}

const updated = readme.replace(pattern, badge);

if (process.argv.includes("--check")) {
  if (updated !== readme) {
    console.error(`sync-badge: README badge is stale; expected ${kib} KB raw / ${gzipKib} KB gzip. Run \`npm run badge\`.`);
    process.exit(1);
  }
  console.log(`sync-badge: ok (${kib} KB raw / ${gzipKib} KB gzip)`);
  process.exit(0);
}

if (updated !== readme) {
  writeFileSync(README, updated);
  console.log(`sync-badge: ${kib} KB raw / ${gzipKib} KB gzip`);
} else {
  console.log("sync-badge: already current");
}
