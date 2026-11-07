/**
 * Static integrity checks on the shipped artifact: offline promises, XSS sinks,
 * package metadata, and the inlined-CSS contract. These are the cheap tests that
 * catch the "senior dev blocks the PR" stuff.
 */
import { readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import zlib from "node:zlib";
import { resolve } from "node:path";
import { ROOT, INDEX_PATH, HTML, boot, createSuite } from "./harness.mjs";

const suite = createSuite("integrity & offline contract");
const pkg = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));

suite.test("no external scripts, styles or fonts", () => {
  const remote = HTML.match(/<(script|link|img)[^>]+(src|href)=["']https?:\/\//gi) || [];
  return remote.length === 0 || "remote assets: " + remote.join(", ");
});

suite.test("no CDN Tailwind runtime", () => HTML.includes("cdn.tailwindcss.com") === false || "CDN Tailwind still referenced");

suite.test("no network calls in the app logic", () => {
  const calls = HTML.match(/\b(fetch|XMLHttpRequest|WebSocket|sendBeacon|EventSource)\s*\(/g) || [];
  return calls.length === 0 || "network calls: " + calls.join(", ");
});

suite.test("no alert/confirm/prompt anywhere", () => {
  const stripped = HTML.replace(/<!--[\s\S]*?-->/g, "");
  const hits = stripped.match(/\b(alert|confirm|prompt)\s*\(/g) || [];
  return hits.length === 0 || "blocking dialogs: " + hits.join(", ");
});

suite.test("Tailwind is inlined and compiled ahead of time", () => {
  if (!HTML.includes("/* TAILWIND_INLINE_START */")) return "inline markers missing";
  const css = HTML.slice(HTML.indexOf("TAILWIND_INLINE_START"), HTML.indexOf("TAILWIND_INLINE_END"));
  if (css.length < 20000) return `inlined CSS suspiciously small (${css.length} bytes)`;
  return css.includes(".hidden{display:none") && css.includes("md\\:w-96") || "expected utilities missing";
});

suite.test("inlined CSS is not stale (rebuild yields the same bytes)", () => {
  const before = HTML;
  execFileSync(process.execPath, [resolve(ROOT, "src/build-css.mjs")], { cwd: ROOT });
  const after = readFileSync(INDEX_PATH, "utf8");
  return before === after || "index.html CSS was out of date — run `npm run css:build`";
});

suite.test("package.json is minimal and correctly named", () => {
  const problems = [];
  if (pkg.name !== "git-undo-gui") problems.push("name " + pkg.name);
  if (pkg.version !== "1.0.0") problems.push("version " + pkg.version);
  for (const banned of ["express", "@google/genai", "react", "react-dom", "motion", "lucide-react", "@tailwindcss/vite", "@vitejs/plugin-react"]) {
    if ((pkg.dependencies || {})[banned] || (pkg.devDependencies || {})[banned]) problems.push("banned dep " + banned);
  }
  for (const script of ["dev", "build", "test", "tauri"]) {
    if (!pkg.scripts[script]) problems.push("missing script " + script);
  }
  return problems.length === 0 || problems.join(", ");
});

suite.test("LICENSE is MIT and README makes no fake claims", () => {
  const license = readFileSync(resolve(ROOT, "LICENSE"), "utf8");
  const readme = readFileSync(resolve(ROOT, "README.md"), "utf8");
  if (!/MIT License/.test(license)) return "LICENSE is not MIT";
  if (/stars-2\.4k|2\.4k/i.test(readme)) return "README still claims 2.4k stars";
  if (!/Demo Status/.test(readme)) return "README is missing the demo-status disclosure";
  return true;
});

suite.test("HTML is a single valid document with one h1", async () => {
  const { d } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  const h1s = d.querySelectorAll("h1").length;
  const lang = d.documentElement.getAttribute("lang");
  return h1s === 1 && lang === "en" || `h1=${h1s} lang=${lang}`;
});

suite.test("CSP meta does not break the shipped file", () => {
  const match = HTML.match(/http-equiv="Content-Security-Policy"\s+content="([^"]+)"/);
  if (!match) return "no CSP meta tag";
  const csp = match[1];
  return csp.includes("default-src 'none'") && csp.includes("script-src 'unsafe-inline'") || "CSP looks wrong: " + csp;
});

suite.test("no user data is ever uploaded or echoed into a script sink", async () => {
  const { d, app } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  const payload = 'a43c821 HEAD@{1}: commit: <img src=x onerror=window.__pwned=1><script>window.__pwned2=1</script>';
  d.getElementById("import-textarea").value = payload;
  app("parseAndApplyReflog()");
  const injected = d.querySelectorAll("img, [onerror]").length;
  return injected === 0 && !app("window.__pwned") && !app("window.__pwned2") || "markup from user input executed";
});

suite.test("artifact stays under the promised size budget", () => {
  const kb = statSync(INDEX_PATH).size / 1024;
  return kb < 200 || `index.html is ${kb.toFixed(0)} KB`;
});

// The v1 bug this repo keeps re-learning: a number in a badge that nobody
// re-measures. These derive the figures from the artifact itself, so the README
// cannot advertise a size the file does not have.
suite.test("the README badge reports the real size", () => {
  const kib = Math.round(statSync(INDEX_PATH).size / 1024);
  const gzipKib = Math.round(zlib.gzipSync(readFileSync(INDEX_PATH), { level: 9 }).length / 1024);
  const readme = readFileSync(resolve(ROOT, "README.md"), "utf8");

  const badge = readme.match(/badge\/index\.html-([0-9]+)%20KB%20raw%20%C2%B7%20([0-9]+)%20KB%20gzip/);
  if (!badge) return "no size badge found in README (expected index.html-<raw>%20KB%20raw%20%C2%B7%20<gzip>%20KB%20gzip)";

  const bad = [];
  if (Number(badge[1]) !== kib) bad.push(`badge says ${badge[1]} KB raw, file is ${kib} KiB`);
  if (Number(badge[2]) !== gzipKib) bad.push(`badge says ${badge[2]} KB gzip, file is ${gzipKib} KiB`);
  return bad.length === 0 || bad.join("; ");
});

suite.test("the README does not claim a size the file contradicts", () => {
  const kib = Math.round(statSync(INDEX_PATH).size / 1024);
  const readme = readFileSync(resolve(ROOT, "README.md"), "utf8");
  // Every "<n> KB" figure presented as the total must match, allowing ±2 KiB.
  const claims = [...readme.matchAll(/(?:≈|about\s)?\**(\d{3})\s*KB\**\s*(?:total|raw)/gi)].map((m) => Number(m[1]));
  const wrong = claims.filter((n) => Math.abs(n - kib) > 2);
  if (wrong.length) return `README claims ${wrong.join(", ")} KB total; file is ${kib} KiB`;
  if (!claims.length) return "no explicit total-size claim found (keep one so it can be checked)";
  return true;
});

export default await suite.run();
