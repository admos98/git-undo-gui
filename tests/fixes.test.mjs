/**
 * Regression tests for the five confirmed bugs.
 *
 * Bugs 1, 2 and 5 are enforced by the Rust guard, so their scenarios live in
 * `src-tauri/logic/src/guard.rs` (pure policy) and `src-tauri/src/e2e.rs`
 * (real repositories). This suite covers the parts that ship in index.html:
 * the patch export (bug 3), the backup field the UI reads (bug 4), and the
 * safety copy the web UI shows for bugs 1 and 2.
 */
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { boot, createSuite, ROOT } from "./harness.mjs";

const SEEN = { "git-undo-seen": "1", "git-undo-tour-seen": "1" };
const suite = createSuite("confirmed-bug regressions");

const GIT_ENV = {
  ...process.env,
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_SYSTEM: "/dev/null",
  GIT_AUTHOR_NAME: "Tester",
  GIT_AUTHOR_EMAIL: "t@t.io",
  GIT_COMMITTER_NAME: "Tester",
  GIT_COMMITTER_EMAIL: "t@t.io",
};

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", env: GIT_ENV });
}

/** Capture whatever downloadPatchFile() hands to the browser. */
function captureDownload(w) {
  const box = { text: null };
  const RealBlob = w.Blob;
  w.Blob = function (parts, opts) { box.text = parts.join(""); return new RealBlob(parts, opts); };
  w.URL.createObjectURL = () => "blob:test";
  w.URL.revokeObjectURL = () => {};
  w.HTMLAnchorElement.prototype.click = () => {};
  return box;
}

// Bug 3: the export used to be rebuilt from displayed diff rows, dropping
// `new file mode`, the /dev/null side, rename metadata and binary hunks, so
// `git apply` rejected it. The exported file must now apply cleanly against
// the parent commit — checked with the real git binary, for a commit that
// adds, deletes, renames, modifies and changes a binary file at once.
suite.test("exported patch passes real `git apply --check` (add/delete/rename/binary)", async () => {
  const dir = mkdtempSync(join(tmpdir(), "git-undo-export-"));
  try {
    git(dir, "init", "-b", "main");
    writeFileSync(join(dir, "mod.txt"), "one\ntwo\nthree\n");
    writeFileSync(join(dir, "del.txt"), "to be deleted\n");
    writeFileSync(join(dir, "old-name.txt"), "renamed content\n");
    writeFileSync(join(dir, "bin.dat"), Buffer.from([0, 1, 2, 3, 255, 0, 9]));
    git(dir, "add", "-A");
    git(dir, "commit", "-m", "base");
    const base = git(dir, "rev-parse", "HEAD").trim();

    writeFileSync(join(dir, "mod.txt"), "one\nTWO changed\nthree\n");
    rmSync(join(dir, "del.txt"));
    git(dir, "mv", "old-name.txt", "new-name.txt");
    writeFileSync(join(dir, "added.txt"), "brand new file\n");
    const newBin = Buffer.from([9, 8, 7, 0, 0, 1, 42, 13]);
    writeFileSync(join(dir, "bin.dat"), newBin);
    git(dir, "add", "-A");
    git(dir, "commit", "-m", "add, delete, rename, modify, binary");

    // What a user pastes from a terminal.
    const shown = git(dir, "show", "--binary", "-M", "HEAD");

    const { app, d, w } = await boot(SEEN);
    d.getElementById("import-textarea").value = shown;
    app("parseAndApplyReflog()");
    const box = captureDownload(w);
    app("downloadPatchFile()");
    if (!box.text) return "downloadPatchFile produced no file";
    for (const needle of ["new file mode", "deleted file mode", "rename from", "rename to", "--- /dev/null", "+++ /dev/null", "GIT binary patch"]) {
      if (!box.text.includes(needle)) return `export lost ${JSON.stringify(needle)}`;
    }

    git(dir, "checkout", "--quiet", base);
    writeFileSync(join(dir, "export.patch"), box.text);
    git(dir, "apply", "--check", "export.patch"); // throws unless it applies
    git(dir, "apply", "export.patch");

    if (readFileSync(join(dir, "added.txt"), "utf8") !== "brand new file\n") return "added.txt wrong after apply";
    if (existsSync(join(dir, "del.txt"))) return "del.txt survived the applied patch";
    if (!existsSync(join(dir, "new-name.txt")) || existsSync(join(dir, "old-name.txt"))) return "rename not applied";
    if (readFileSync(join(dir, "mod.txt"), "utf8") !== "one\nTWO changed\nthree\n") return "mod.txt wrong after apply";
    if (!readFileSync(join(dir, "bin.dat")).equals(newBin)) return "binary content wrong after apply";
    return true;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// Bug 3, other half: a real import that contains no patch text (git log
// --oneline, a reflog) has nothing applyable. Exporting the old reconstruction
// handed users a file that failed `git apply`; now it must refuse instead.
suite.test("export refuses when a real import has no patch text", async () => {
  const { app, d, w } = await boot(SEEN);
  d.getElementById("import-textarea").value = "51ee48b feat: add c.txt\nfa86670 fix(a): change line two\n";
  app("parseAndApplyReflog()");
  const box = captureDownload(w);
  app("downloadPatchFile()");
  if (box.text !== null) return "a patch file was offered for an import with no diff text";
  return d.body.textContent.includes("No patch to export") || "refusal was not explained to the user";
});

// Bug 4: Rust serialises BackupReport with `covers_head`; the UI read a
// nonexistent `safe_undo`, so every correct backup looked stale. The response
// used here is built from the fields the Rust struct actually declares, so
// renaming a field on either side fails this test.
suite.test("reset result reads the backup field Rust actually serialises", async () => {
  const service = readFileSync(resolve(ROOT, "src-tauri/src/service.rs"), "utf8");
  const structMatch = service.match(/pub struct BackupReport \{([\s\S]*?)\n\}/);
  if (!structMatch) return "BackupReport not found in service.rs";
  const fields = [...structMatch[1].matchAll(/pub (\w+): (\w+)/g)].map((m) => ({ name: m[1], type: m[2] }));
  if (!fields.some((f) => f.name === "covers_head")) return "Rust no longer sends covers_head; update the UI and this test together";
  if (fields.some((f) => f.name === "safe_undo")) return "safe_undo appeared in Rust; the field-name mismatch is back";

  const valueFor = (field, covers) => {
    if (field.name === "covers_head") return covers;
    if (field.name === "branch") return "backup-before-recovery";
    if (field.name === "commit") return "a43c821aa11bb22cc33dd44ee55ff66aa77bb88cc";
    if (field.type === "bool") return true;
    return "2026-11-06";
  };
  const outcomeFor = (covers) => ({
    ok: true,
    command: "git reset --hard a43c821",
    message: "Branch and working tree are now at a43c821.",
    now_at: "main",
    undo: "git reset --hard f82a91c",
    risks: [],
    previous_head: "f82a91c",
    backup_branch: "backup-before-recovery",
    backup: Object.fromEntries(fields.map((f) => [f.name, valueFor(f, covers)])),
  });

  const { app, d, w } = await boot(SEEN);
  let covers = true;
  w.__TAURI__ = {
    core: {
      invoke: async (cmd) => {
        if (cmd === "git_reset") return outcomeFor(covers);
        if (cmd === "git_read_reflog") return "a43c821 HEAD@{0}: reset: moving to a43c821\n";
        throw new Error("unexpected command " + cmd);
      },
    },
  };
  app("repoPath = '/tmp/repo'");

  await app("performDesktopReset({ hash: 'a43c821' }, 'hard', 'RESET')");
  const fresh = d.getElementById("modal-result").textContent;
  if (!fresh.includes("holds the state you just left")) return "correct backup not reported as covering HEAD: " + fresh;
  if (fresh.includes("does not contain")) return "correct backup reported as stale: " + fresh;

  covers = false;
  await app("performDesktopReset({ hash: 'a43c821' }, 'hard', 'RESET')");
  const stale = d.getElementById("modal-result").textContent;
  return stale.includes("does not contain what you just left") || "stale backup not flagged: " + stale;
});

// Bug 2 (UI copy): restore-files must not claim to keep uncommitted work.
suite.test("restore-files copy warns that uncommitted edits are overwritten", async () => {
  const { app } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0);");
  const text = app("explainCommand(reflogEntries[selectedIndex], 'restore', { count: 1 })");
  const hint = app("STRATEGY_META.restore.hint");
  if (!String(text).includes("overwritten")) return "explainCommand is silent about overwritten edits: " + text;
  if (!String(hint).includes("overwritten")) return "strategy hint is silent about overwritten edits: " + hint;
  return true;
});

// Bug 1 (UI copy): untracked files are only safe when the target snapshot does
// not track their paths; the state box must not promise otherwise.
suite.test("repo-state box never calls untracked files safe", async () => {
  const { app, d, w } = await boot(SEEN);
  w.__TAURI__ = { core: { invoke: async () => { throw new Error("unused"); } } };
  app("repoPath = '/tmp/repo'");
  app(`renderRepoStateBox({
    branch: "main", head: "a43c821", in_progress: null, dirty_count: 1,
    dirty: [{ status: "untracked", path: "notes.md" }], unpushed: [], stash_count: 0, backup: null
  })`);
  const text = d.getElementById("modal-repo-state").textContent;
  if (text.includes("(safe)")) return "untracked files promised safe: " + text;
  return text.includes("checks them against the snapshot") || "no guard check mentioned: " + text;
});

export default await suite.run();
