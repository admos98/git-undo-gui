/**
 * Tests for the optional desktop executor.
 *
 * The web build must keep working exactly as before — it cannot run git. These
 * tests pin three things:
 *
 *   1. In a browser (no `window.__TAURI__`) the app copies commands and never
 *      reaches for a bridge. Nothing is executed, nothing is smuggled in.
 *   2. In a desktop shell the app only ever calls the four IPC commands the
 *      Rust side actually registers, with the argument names and enum strings
 *      those commands expect. A typo here is a runtime failure on the user's
 *      machine, which no amount of Rust unit testing would catch.
 *   3. The safety rules line up: `RESET` acknowledges a hard reset, the backup
 *      branch is agreed on both sides, and `--force` never appears.
 */
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { boot, createSuite, ROOT, wait } from "./harness.mjs";

const SEEN = { "git-undo-seen": "1", "git-undo-tour-seen": "1" };
const REFLOG = [
  "a43c821 HEAD@{0}: reset: moving to HEAD~3",
  "9f2c114 HEAD@{1}: commit: feat: billing retry",
  "7b1e003 HEAD@{2}: commit: chore: tidy",
].join("\n");
const IMPORTED = { ...SEEN, "git-undo-imported-reflog": REFLOG, "git-undo-import-mode": "reflog" };

const suite = createSuite("desktop executor");

const rust = (p) => readFileSync(resolve(ROOT, "src-tauri", p), "utf8");

/** Boot with a fake Tauri bridge, recording every invoke. */
async function bootDesktop(handler) {
  const harness = await boot(IMPORTED);
  const calls = [];
  harness.w.__TAURI__ = {
    core: {
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (handler) return handler(cmd, args);
        return defaultResponse(cmd);
      },
    },
    dialog: { open: async () => "/home/dev/project" },
  };
  // Re-run the desktop boot hooks the app performs on DOMContentLoaded.
  harness.app("window.__TAURI__ && document.getElementById('btn-repo').classList.remove('hidden')");
  return { ...harness, calls };
}

function defaultResponse(cmd) {
  if (cmd === "git_repo_state") {
    return {
      branch: "main",
      head: "a43c821",
      dirty_count: 0,
      dirty: [],
      unpushed: [],
      in_progress: null,
      stash_count: 0,
    };
  }
  if (cmd === "git_read_reflog") return REFLOG;
  if (cmd === "git_create_backup") {
    return {
      ok: true,
      command: "git branch backup-before-recovery HEAD",
      message: "Backup branch backup-before-recovery created at the current HEAD.",
      now_at: "backup-before-recovery @ a43c821",
      undo: null,
      risks: [],
    };
  }
  if (cmd === "git_reset") {
    return {
      message: "Moved main to 9f2c114 (soft).",
      now_at: "9f2c114",
      backup_branch: "backup-before-recovery",
      undo: "git reset --soft a43c821",
      risks: [],
    };
  }
  throw new Error(`unknown command ${cmd}`);
}

/** Drive the real UI: select entry, pick a strategy, open modal, confirm. */
async function runReset(h, { index = 1, strategy = "soft", phrase = "UNDO" } = {}) {
  h.app(`selectEntry(${index}); updateStrategy('${strategy}')`);
  h.app("openConfirmModal()");
  h.d.getElementById("confirm-input").value = phrase;
  h.app(`handleConfirmInput(${JSON.stringify(phrase)})`);
  h.app("executeVisualUndo()");
  await wait(h.w, 30);
}

// ---------------------------------------------------------------- web build

suite.test("the browser build has no bridge and executes nothing", async () => {
  const h = await boot(IMPORTED);
  if (h.app("typeof window.__TAURI__") !== "undefined") return "a Tauri global leaked into the web build";
  if (h.app("isDesktop()") !== false) return "isDesktop() is true without a shell";
  h.app("selectEntry(1); updateStrategy('soft')");
  h.app("openConfirmModal(); executeVisualUndo()");
  await wait(h.w, 20);
  // The desktop paths must be inert: no repo, no state, no modal result.
  const problems = [];
  if (h.app("repoPath") !== null) problems.push("repoPath set");
  if (h.open("modal-result")) problems.push("modal-result visible");
  if (h.open("modal-repo-state")) problems.push("repo-state visible");
  return problems.length === 0 || problems.join(", ");
});

suite.test("the repo picker stays hidden until a shell is present", async () => {
  const h = await boot(SEEN);
  const btn = h.d.getElementById("btn-repo");
  return (!btn || btn.classList.contains("hidden")) || "repo button is visible in the web build";
});

suite.test("the UI only claims to execute when it can", async () => {
  const web = await boot(IMPORTED);
  const webStrip = web.d.querySelector("#undo-outcome span:last-child").textContent;
  const webNote = web.d.getElementById("demo-mode-note").textContent;
  if (!/copies the reset command/.test(webStrip)) return `web strip claims: ${webStrip}`;
  if (/runs? the reset|executes? the reset/i.test(webNote)) return `web note claims to execute: ${webNote}`;
  if (!/terminal/.test(webNote)) return `web note does not point at the terminal: ${webNote}`;

  const h = await bootDesktop();
  const before = h.d.querySelector("#undo-outcome span:last-child").textContent;
  if (!/copies the reset command/.test(before)) return `desktop, no repo, claims: ${before}`;

  await h.app("loadRepository('/home/dev/project')");
  const after = h.d.querySelector("#undo-outcome span:last-child").textContent;
  const afterNote = h.d.getElementById("demo-mode-note").textContent;
  if (!/runs the reset/.test(after)) return `desktop, repo open, claims: ${after}`;
  if (!/backup-before-recovery/.test(afterNote)) return `backup not promised: ${afterNote}`;
  return true;
});

// ------------------------------------------------------------ IPC contract

suite.test("the app only calls the four commands Rust registers", async () => {
  const lib = rust("src/lib.rs");
  const registered = [...lib.matchAll(/#\[tauri::command\]\s*\n\s*(?:pub\s+)?fn\s+(\w+)/g)].map((m) => m[1]);
  if (registered.length !== 4) return `expected 4 #[tauri::command] fns, found ${registered.length}: ${registered}`;

  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  await runReset(h, { index: 1, strategy: "soft" });
  await h.app("createBackupBranch()");
  await wait(h.w, 20);
  const used = [...new Set(h.calls.map((c) => c.cmd))];
  const unknown = used.filter((c) => !registered.includes(c));
  if (unknown.length) return `called unknown command(s): ${unknown} — Rust has ${registered}`;
  if (!used.includes("git_reset")) return "the confirm flow never reached git_reset";
  // Every registered command should be reachable from the UI.
  const unused = registered.filter((c) => !used.includes(c));
  if (unused.length) return `registered but never called: ${unused}`;
  return true;
});

suite.test("argument names match the Rust signatures", async () => {
  const lib = rust("src/lib.rs");
  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  await runReset(h, { index: 1, strategy: "hard", phrase: "RESET" });

  const problems = [];
  for (const call of h.calls) {
    // Find the Rust fn whose name matches, then check each JS key exists in it.
    const fn = lib.match(new RegExp(`fn\\s+${call.cmd}\\s*\\(([^)]*)\\)`));
    if (!fn) { problems.push(`${call.cmd} not found in lib.rs`); continue; }
    for (const key of Object.keys(call.args || {})) {
      if (!new RegExp(`\\b${key}\\s*:`).test(fn[1])) problems.push(`${call.cmd}: no Rust param "${key}"`);
    }
  }
  return problems.length === 0 || problems.join("; ");
});

suite.test("strategy strings are exactly branch|soft|files|hard", async () => {
  const guard = rust("logic/src/guard.rs");
  const kinds = [...guard.matchAll(/"([a-z]+)"\s*=>\s*(?:Some\()?ResetKind::/g)].map((m) => m[1]);
  const expected = ["branch", "soft", "files", "hard"];
  const missing = expected.filter((k) => !kinds.includes(k));
  if (missing.length) return `guard.rs does not parse ${missing} (found ${kinds})`;

  // The UI's four cards map onto those four strings. `restore` is a UI label for
  // the `files` operation; every other card keeps its own name.
  const cards = { soft: "soft", branch: "branch", restore: "files", hard: "hard" };
  const problems = [];
  for (const [card, wire] of Object.entries(cards)) {
    const h = await bootDesktop();
    await h.app("loadRepository('/home/dev/project')");
    await runReset(h, { index: 1, strategy: card, phrase: card === "hard" ? "RESET" : "UNDO" });
    const call = h.calls.find((c) => c.cmd === "git_reset");
    if (!call) { problems.push(`${card}: no git_reset call`); continue; }
    if (call.args.strategy !== wire) problems.push(`${card} sent as ${call.args.strategy}, Rust expects ${wire}`);
    if (!kinds.includes(call.args.strategy)) problems.push(`${card}: "${call.args.strategy}" is not a ResetKind`);
  }
  return problems.length === 0 || problems.join("; ");
});

// -------------------------------------------------------------- guardrails

suite.test("a hard reset demands RESET, never the everyday UNDO", async () => {
  const guard = rust("logic/src/guard.rs");
  const phrase = guard.match(/ACK_PHRASE:\s*&str\s*=\s*"([^"]+)"/);
  if (!phrase) return "ACK_PHRASE not found in guard.rs";

  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  h.app("selectEntry(1); updateStrategy('hard')");
  const required = h.app("requiredPhrase()");
  if (required !== phrase[1]) return `UI asks for "${required}", Rust expects "${phrase[1]}"`;

  // Typing the weak phrase must leave the button locked and send nothing.
  h.app("openConfirmModal(); handleConfirmInput('UNDO')");
  const locked = h.d.getElementById("btn-confirm-undo").disabled;
  if (!locked) return "'UNDO' unlocked a hard reset in desktop mode";
  return true;
});

suite.test("the backup branch name is shared by both sides", async () => {
  const service = rust("src/service.rs");
  const declared = service.match(/BACKUP_BRANCH:\s*&str\s*=\s*"([^"]+)"/);
  if (!declared) return "BACKUP_BRANCH not found in service.rs";
  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  const call = h.calls.find((c) => c.cmd === "git_read_reflog");
  if (!call) return "loading a repo never read the reflog";
  return declared[1] === "backup-before-recovery" || `backup branch renamed to ${declared[1]}`;
});

suite.test("no --force anywhere; push recovery uses --force-with-lease", async () => {
  const html = readFileSync(resolve(ROOT, "index.html"), "utf8");
  const bare = html.match(/--force(?!-with-lease)/g) || [];
  if (bare.length) return `${bare.length} bare --force occurrence(s) in index.html`;
  if (!html.includes("--force-with-lease")) return "--force-with-lease missing from push recovery";
  const rs = rust("src/service.rs") + rust("src/git.rs") + rust("logic/src/guard.rs");
  if (/--force(?!-with-lease)/.test(rs)) return "bare --force in the Rust executor";
  // The app never shells out itself.
  return !/execSync|child_process|require\("child"\)/.test(html) || "index.html shells out";
});

suite.test("refusals are surfaced, not swallowed", async () => {
  const h = await bootDesktop((cmd) => {
    if (cmd === "git_reset") {
      throw { kind: "refused", reason: "Uncommitted changes would be lost. Commit or stash them, then reset." };
    }
    return defaultResponse(cmd);
  });
  await h.app("loadRepository('/home/dev/project')");
  await runReset(h, { index: 1, strategy: "hard", phrase: "RESET" });

  const box = h.d.getElementById("modal-result");
  if (!box || box.classList.contains("hidden")) return "the refusal was never shown";
  if (!box.textContent.includes("Uncommitted changes")) return `refusal text missing: ${box.textContent}`;
  const button = h.d.getElementById("btn-confirm-undo");
  if (button.disabled) return "button stayed disabled after a refusal, trapping the user";
  // A refusal must not claim success or pretend the reflog changed.
  return !box.textContent.includes("✓") || "a refusal was rendered as success";
});

suite.test("the backup button branches off HEAD without moving anything", async () => {
  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  h.app("selectEntry(1); updateStrategy('hard'); openConfirmModal()");
  h.app("createBackupBranch()");
  await wait(h.w, 20);
  const call = h.calls.find((c) => c.cmd === "git_create_backup");
  if (!call) return "no git_create_backup call";
  const box = h.d.getElementById("modal-result");
  if (!box || box.classList.contains("hidden")) return "backup result was not shown";
  if (!box.textContent.includes("backup-before-recovery")) return `backup branch not named: ${box.textContent}`;
  // Creating a backup must never be reported as a reset.
  return !h.calls.some((c) => c.cmd === "git_reset") || "the backup button ran a reset";
});

suite.test("a successful reset reports the way back", async () => {
  const h = await bootDesktop();
  await h.app("loadRepository('/home/dev/project')");
  await runReset(h, { index: 1, strategy: "soft" });

  const box = h.d.getElementById("modal-result");
  if (!box || box.classList.contains("hidden")) return "no result shown after a successful reset";
  if (!box.textContent.includes("git reset --soft a43c821")) return `undo hint missing: ${box.textContent}`;
  // And the timeline is re-read from disk rather than faked.
  const reads = h.calls.filter((c) => c.cmd === "git_read_reflog").length;
  return reads >= 2 || `reflog read ${reads} time(s); expected a refresh after the reset`;
});

suite.test("the bundle ships no icons it cannot use", async () => {
  const conf = JSON.parse(rust("tauri.conf.json"));
  const icons = conf.bundle.icon.join(" ");
  // A desktop bundle needs the Windows .ico, the macOS .icns and a PNG set.
  for (const needed of ["32x32.png", "128x128.png", "icon.ico", "icon.icns"]) {
    if (!icons.includes(needed)) return `bundle is missing ${needed}`;
  }
  // iOS/Android/Store artwork is dead weight for a desktop app and quietly
  // reappears when someone re-runs `tauri icon`.
  const dir = resolve(ROOT, "src-tauri", "icons");
  const bloat = readdirSync(dir).filter((f) => /^(Square|StoreLogo|ios|android)/.test(f));
  if (bloat.length) return `unused platform artwork: ${bloat.join(", ")}`;
  if (!Array.isArray(conf.bundle.targets) || conf.bundle.targets.includes("all")) {
    return "bundle.targets is 'all', which pulls Windows Store artwork back in";
  }
  return true;
});

// ------------------------------------------------------- desktop packaging

suite.test("the desktop build ships the same page as the web build", async () => {
  // tauri embeds src-tauri/assets/index.html; if that drifts from dist the
  // desktop app silently runs an older UI. Compare against the committed source.
  const source = readFileSync(resolve(ROOT, "index.html"), "utf8");
  const embedded = readFileSync(resolve(ROOT, "src-tauri", "assets", "index.html"), "utf8");
  if (embedded !== source) {
    return "src-tauri/assets/index.html is stale — run `npm run build && npm run embed:web`";
  }
  // And the frontend must not depend on a bundled JS package to find the bridge.
  if (!/window\.__TAURI__/.test(source)) return "the UI no longer looks for the Tauri global";
  return true;
});

suite.test("the shell serves the page itself and loads nothing remote", async () => {
  const conf = JSON.parse(rust("tauri.conf.json"));
  const url = conf.app.windows[0].url;
  if (!url || /^https?:\/\/(?!app\.localhost)/.test(url)) return `window points somewhere remote: ${url}`;
  const lib = rust("src/lib.rs");
  if (!/register_uri_scheme_protocol\("app"/.test(lib)) return "no custom protocol registered";
  if (/devUrl/.test(rust("tauri.conf.json"))) return "a devUrl is configured; the packaged app must not fetch a page";
  // The bridge is injected by the shell, so the app needs no @tauri-apps/api.
  if (!/__TAURI_INTERNALS__/.test(rust("src/embed.rs"))) return "the shell injects no IPC bridge";
  return true;
});

// --------------------------------------------------------------- lightness

suite.test("the executor adds no runtime dependency to the web build", async () => {
  const pkg = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));
  if (pkg.dependencies && Object.keys(pkg.dependencies).length) {
    return `runtime dependencies appeared: ${Object.keys(pkg.dependencies)}`;
  }
  const html = readFileSync(resolve(ROOT, "index.html"), "utf8");
  if (/@tauri-apps\/api/.test(html)) return "index.html imports the Tauri JS API instead of using the global";
  // Nothing is *loaded* from the network: no scripts, styles, images or fonts.
  // The one permitted outbound URL is a plain <a href> the user must click.
  const loaders = html.match(/<(?:script|img|iframe|link|source)[^>]*(?:src|href)="https?:\/\//g) || [];
  if (loaders.length) return `${loaders.length} remote asset(s) are fetched on load`;
  const external = html.match(/href="(https?:\/\/[^"]+)"/g) || [];
  return external.every((u) => u.includes("github.com/admos98/git-undo-gui")) || `unexpected outbound link: ${external}`;
});

suite.test("the native layer stays thin: no server, no daemon, no telemetry", async () => {
  const files = ["src/lib.rs", "src/service.rs", "src/git.rs", "Cargo.toml", "tauri.conf.json"];
  const problems = [];
  for (const f of files) {
    const text = rust(f);
    if (/reqwest|hyper|axum|warp|actix|tokio::net/.test(text)) problems.push(`${f}: network server/stack`);
    if (/std::thread::spawn|tokio::spawn/.test(text)) problems.push(`${f}: background thread`);
    if (/posthog|sentry\b|mixpanel|analytics::|track_event/.test(text)) problems.push(`${f}: telemetry`);
    if (/open::that|webbrowser::/.test(text)) problems.push(`${f}: opens external URLs`);
  }
  // The dependency list is the weight control: four direct crates, no more.
  const cargo = rust("Cargo.toml");
  const deps = cargo.split(/^\[/m).find((sec) => sec.startsWith("dependencies]")) || "";
  const names = [...deps.matchAll(/^([a-z0-9_-]+)\s*=/gm)].map((m) => m[1]);
  // Four direct crates plus the folder picker. If that list grows, the shell got
  // heavier than promised and this test should fail loudly.
  const allowed = ["git-undo-logic", "tauri", "serde", "serde_json", "tauri-plugin-dialog"];
  const extra = names.filter((n) => !allowed.includes(n));
  if (extra.length) problems.push(`extra crates: ${extra}`);
  return problems.length === 0 || problems.join("; ");
});

export default await suite.run();
