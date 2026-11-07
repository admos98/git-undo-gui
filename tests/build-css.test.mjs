/** Regression coverage for the cross-platform CSS build command. */
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { ROOT, createSuite } from "./harness.mjs";
import { matchesInlineCss } from "./css-equivalence.mjs";

const suite = createSuite("CSS build portability");
const page = (css) => `<html><style>/* TAILWIND_INLINE_START */${css}/* TAILWIND_INLINE_END */</style><script>window.ready = true;</script></html>`;

suite.test("inline CSS comparison tolerates native minifier color rounding", () => {
  const before = page(".a{color:oklab(71.8977% -.0133505 -.131483/.3)}");
  const after = page(".a{color:oklab(71.8977% -.0133506 -.131483/.3)}");
  return matchesInlineCss(before, after) || "sub-millionth color rounding was treated as stale CSS";
});

suite.test("inline CSS comparison rejects meaningful color changes", () => {
  return !matchesInlineCss(
    page(".a{color:oklab(71.8977% -.0133505 -.131483/.3)}"),
    page(".a{color:oklab(71.8977% -.0233505 -.131483/.3)}"),
  ) || "a real color change was ignored";
});

suite.test("inline CSS comparison rejects layout changes", () => {
  return !matchesInlineCss(page(".hidden{display:none}"), page(".hidden{display:block}")) || "a layout change was ignored";
});

suite.test("inline CSS comparison rejects changes outside inline CSS", () => {
  const before = page(".hidden{display:none}");
  return !matchesInlineCss(before, before.replace("window.ready = true", "window.ready = false")) || "an application change was ignored";
});

suite.test("CSS builds without PATH tools from a directory containing spaces", () => {
  const fixture = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "git-undo-css "));
  try {
    mkdirSync(resolve(fixture, "src"));
    copyFileSync(resolve(ROOT, "src/build-css.mjs"), resolve(fixture, "src/build-css.mjs"));
    symlinkSync(resolve(ROOT, "node_modules"), resolve(fixture, "node_modules"),
      process.platform === "win32" ? "junction" : "dir");
    writeFileSync(resolve(fixture, "src/tailwind.css"), '@import "tailwindcss" source("../");\n');
    const htmlPath = resolve(fixture, "index.html");
    writeFileSync(htmlPath, `<!doctype html>
<html><head><style>
/* TAILWIND_INLINE_START */
/* TAILWIND_INLINE_END */
</style></head><body><main class="hidden">fixture</main></body></html>
`);
    const env = Object.fromEntries(Object.entries(process.env)
      .filter(([key]) => key.toLowerCase() !== "path"));
    env.PATH = "";
    execFileSync(process.execPath, [resolve(fixture, "src/build-css.mjs")], {
      cwd: fixture, env, timeout: 30000,
    });
    const compiled = readFileSync(htmlPath, "utf8");
    return compiled.includes(".hidden{display:none}") || "compiled utility missing from inline CSS";
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
});

export default await suite.run();
