/**
 * Regression tests: the confirmation dialog must describe what the button does.
 *
 * The bug: the explanation under the input was chosen from the confirmation
 * *phrase*. That worked while the phrase and the mode happened to agree — the
 * browser copies and asks for `UNDO`, a hard reset executes and asks for
 * `RESET` — and broke for everything else. A desktop soft reset, rescue branch
 * or file restore also ask for `UNDO`, so all three told the user:
 *
 *     "This copies a command for you to run. Nothing is changed on disk."
 *
 * while running git against their repository, and the rescue note went further
 * and promised a backup branch that the rescue strategy does not create.
 *
 * The rule these tests pin: the sentence is a function of (mode, strategy), and
 * the phrase is only the acknowledgement. Every row of that matrix is checked
 * through the real dialog, not by calling the helper directly.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { boot, createSuite, ROOT, wait } from "./harness.mjs";

const SEEN = { "git-undo-seen": "1", "git-undo-tour-seen": "1" };
const REFLOG = [
  "a43c821 HEAD@{0}: reset: moving to HEAD~3",
  "9f2c114 HEAD@{1}: commit: feat: billing retry",
  "7b1e003 HEAD@{2}: commit: chore: tidy",
].join("\n");
// A pasted reflog: in a browser this is copied, in the desktop shell it runs.
const IMPORTED = { ...SEEN, "git-undo-imported-reflog": REFLOG, "git-undo-import-mode": "reflog" };

const suite = createSuite("confirm dialog describes what runs");
const STRATEGIES = ["soft", "branch", "restore", "hard"];

/** Open the dialog on a strategy and read every surface that explains it. */
async function dialog(strategy, h) {
  const harness = h || await boot(IMPORTED);
  harness.app(`selectEntry(1); updateStrategy('${strategy}'); openConfirmModal()`);
  const read = () => ({
    phrase: harness.text("confirm-phrase"),
    placeholder: harness.d.getElementById("confirm-input").placeholder,
    why: harness.text("confirm-why"),
    note: harness.d.getElementById("demo-mode-note").textContent,
    desc: harness.text("confirm-modal-desc"),
  });
  return { h: harness, seen: read(), read };
}

async function desktop(strategy) {
  const h = await boot(IMPORTED);
  h.w.__TAURI__ = {
    core: {
      invoke: async (cmd) => {
        if (cmd === "git_repo_state") {
          return {
            branch: "main", head: "a43c821", dirty_count: 0, dirty: [],
            unpushed: [], in_progress: null, stash_count: 0, backup: null,
          };
        }
        if (cmd === "git_read_reflog") return REFLOG;
        throw new Error(`unexpected command ${cmd}`);
      },
    },
    dialog: { open: async () => "/home/dev/project" },
  };
  await h.app("loadRepository('/home/dev/project')");
  return dialog(strategy, h);
}

// ------------------------------------------------------------ browser build

suite.test("the browser explains copying, for all four strategies", async () => {
  for (const strategy of STRATEGIES) {
    const { seen, h } = await dialog(strategy);
    if (h.app("willExecute()") !== false) return `browser claims it executes (${strategy})`;
    if (!/copies/i.test(seen.why)) return `${strategy}: browser does not say it copies: ${seen.why}`;
    if (!/nothing is changed on disk/i.test(seen.why)) return `${strategy}: browser does not say nothing changes: ${seen.why}`;
    if (/\bruns\b|\bcreates a rescue branch\b/i.test(seen.why)) return `${strategy}: browser claims to act: ${seen.why}`;
    if (!/nothing is executed from the page/i.test(seen.note)) return `${strategy}: note claims execution: ${seen.note}`;
    if (seen.phrase !== "UNDO") return `${strategy}: browser asks for "${seen.phrase}"`;
  }
  return true;
});

suite.test("the browser never promises a backup branch", async () => {
  for (const strategy of STRATEGIES) {
    const { seen } = await dialog(strategy);
    if (/backup/i.test(seen.why) || /backup/i.test(seen.note)) {
      return `${strategy}: the browser builds no backup but mentions one: ${seen.why} | ${seen.note}`;
    }
  }
  return true;
});

// ------------------------------------------------------------- rescue branch

suite.test("a native rescue branch says a branch is created and HEAD stays put", async () => {
  const { seen } = await desktop("branch");
  if (seen.phrase !== "UNDO") return `rescue asks for "${seen.phrase}", expected the everyday phrase`;
  if (!/creates a rescue branch/i.test(seen.why)) return `rescue does not say it creates a branch: ${seen.why}`;
  if (!/HEAD, your branch and your files stay/i.test(seen.why)) return `rescue does not say HEAD/files are unchanged: ${seen.why}`;
  // The lie: the rescue strategy is itself the backup and takes none.
  if (/backup/i.test(seen.why)) return `rescue promises a backup branch: ${seen.why}`;
  if (/backup/i.test(seen.note)) return `rescue note promises a backup branch: ${seen.note}`;
  if (/copies a command/i.test(seen.why)) return `rescue still claims to copy: ${seen.why}`;
  if (/nothing is changed on disk/i.test(seen.why)) return `rescue claims the disk is untouched: ${seen.why}`;
  return true;
});

// --------------------------------------------------------------- soft reset

suite.test("a native soft reset says HEAD moves and changes stay staged", async () => {
  const { seen } = await desktop("soft");
  if (seen.phrase !== "UNDO") return `soft asks for "${seen.phrase}"`;
  if (!/reset --soft/.test(seen.why)) return `soft does not name the command: ${seen.why}`;
  if (!/HEAD moves/i.test(seen.why)) return `soft does not say HEAD moves: ${seen.why}`;
  if (!/staged/i.test(seen.why)) return `soft does not say the changes stay staged: ${seen.why}`;
  if (/copies a command/i.test(seen.why) || /nothing is changed on disk/i.test(seen.why)) {
    return `soft still claims to only copy: ${seen.why}`;
  }
  if (!/HEAD moves/i.test(seen.note)) return `soft note does not describe the HEAD move: ${seen.note}`;
  // The sentence above the field must not describe a copy on a desktop import.
  if (/copies the command|Nothing runs until you paste it/i.test(seen.desc)) {
    return `desktop dialog intro still says it copies: ${seen.desc}`;
  }
  return true;
});

// ------------------------------------------------------------- file restore

suite.test("a native restore says file contents change and dirty paths are refused", async () => {
  const { seen } = await desktop("restore");
  if (seen.phrase !== "UNDO") return `restore asks for "${seen.phrase}"`;
  if (!/restore --source/.test(seen.why)) return `restore does not name the command: ${seen.why}`;
  if (!/file contents change/i.test(seen.why)) return `restore does not say file contents change: ${seen.why}`;
  // The guard refuses dirty paths instead of overwriting them; the dialog has to
  // say "refused", not merely "overwritten", or the refusal reads as a bug.
  if (!/refused/i.test(seen.why)) return `restore does not mention the dirty-path refusal: ${seen.why}`;
  if (/copies a command/i.test(seen.why) || /nothing is changed on disk/i.test(seen.why)) {
    return `restore still claims to only copy: ${seen.why}`;
  }
  if (!/refused/i.test(seen.note)) return `restore note does not mention the refusal: ${seen.note}`;
  return true;
});

// --------------------------------------------------------------- hard reset

suite.test("a native hard reset keeps the destructive warning and the RESET gate", async () => {
  const { seen, h } = await desktop("hard");
  if (seen.phrase !== "RESET") return `hard reset asks for "${seen.phrase}"`;
  if (seen.placeholder !== "Type RESET") return `hard reset placeholder: ${seen.placeholder}`;
  if (!/reset --hard/.test(seen.why)) return `hard reset does not name the command: ${seen.why}`;
  if (!/uncommitted work is discarded/i.test(seen.why)) return `hard reset lost its warning: ${seen.why}`;
  if (!/RESET/.test(seen.why)) return `hard reset does not explain the RESET requirement: ${seen.why}`;
  // "UNDO alone only copies a command" was the sentence that made the other
  // strategies lie; the hard-reset copy must not hand it back.
  if (/UNDO.{0,20}only copies/i.test(seen.why)) return `hard reset repeats the copy-only claim: ${seen.why}`;
  if (!/Retyping RESET is required/i.test(seen.note)) return `hard reset note is not strict: ${seen.note}`;
  // And the gate itself still holds: UNDO must not unlock it.
  h.app("handleConfirmInput('UNDO')");
  if (!h.d.getElementById("btn-confirm-undo").disabled) return "UNDO unlocked a hard reset";
  h.app("handleConfirmInput('RESET')");
  if (h.d.getElementById("btn-confirm-undo").disabled) return "RESET did not unlock the button";
  return true;
});

// ------------------------------------------------------------- mode changes

suite.test("switching strategy in an open dialog repaints the explanation", async () => {
  const { h, read } = await desktop("branch");
  const rescue = read();
  h.app("updateStrategy('soft')");
  const soft = read();
  h.app("updateStrategy('restore')");
  const restore = read();
  h.app("updateStrategy('hard')");
  const hard = read();
  h.app("updateStrategy('branch')");
  const back = read();

  if (new Set([rescue.why, soft.why, restore.why, hard.why]).size !== 4) {
    return `explanations are not strategy-specific:\n${[rescue, soft, restore, hard].map((s) => `  ${s.why}`).join("\n")}`;
  }
  if (back.why !== rescue.why) return `switching back did not restore the rescue wording: ${back.why}`;
  if (back.note !== rescue.note) return `switching back did not restore the rescue note: ${back.note}`;
  return true;
});

suite.test("opening a repository after a browser import repaints the copy claim", async () => {
  // The realistic desktop sequence: the page loads from storage (no repo yet, so
  // it says "copies") and only then does the shell attach the repository.
  const h = await boot(IMPORTED);
  h.app("selectEntry(1); updateStrategy('soft'); openConfirmModal()");
  const before = h.text("confirm-why");
  if (!/copies/i.test(before)) return `browser state does not say it copies: ${before}`;

  h.w.__TAURI__ = {
    core: {
      invoke: async (cmd) => {
        if (cmd === "git_repo_state") {
          return {
            branch: "main", head: "a43c821", dirty_count: 0, dirty: [],
            unpushed: [], in_progress: null, stash_count: 0, backup: null,
          };
        }
        if (cmd === "git_read_reflog") return REFLOG;
        throw new Error(`unexpected command ${cmd}`);
      },
    },
    dialog: { open: async () => "/home/dev/project" },
  };
  await h.app("loadRepository('/home/dev/project')");
  await wait(h.w, 20);
  if (h.app("willExecute()") !== true) return "the repository did not attach";

  // loadRepository -> loadImportedData -> renderAll repaints the deck, and
  // paintExecutionFraming() repaints the note, so neither surface may still be
  // advertising a copy.
  const note = h.d.getElementById("demo-mode-note").textContent;
  if (!/Runs on your repository/i.test(note)) return `note still says it copies: ${note}`;
  h.app("openConfirmModal()");
  const why = h.text("confirm-why");
  return !/copies/i.test(why) || `dialog still says it copies with a repo open: ${why}`;
});

// -------------------------------------------------------------- the source

suite.test("the explanation is not derived from the phrase", async () => {
  const html = readFileSync(resolve(ROOT, "index.html"), "utf8");
  if (/needed === PHRASE\.destructive\s*\n?\s*\?\s*`This runs/.test(html)) {
    return "the confirm explanation is still branched on the phrase instead of the mode";
  }
  if (/needed === PHRASE\.destructive[\s\S]{0,400}backup branch is taken first/.test(html)) {
    return "the backup promise is still tied to the phrase";
  }
  // The word may appear on several surfaces; the *decision* must read strategy.
  return /function explainOutcome\(executing[\s\S]{0,200}strategy/.test(html)
    || "no explainOutcome(executing, strategy) helper: the mode/strategy matrix is not centralised";
});

suite.test("the README states the strategy matrix instead of 'UNDO copies'", async () => {
  const readme = readFileSync(resolve(ROOT, "README.md"), "utf8");
  if (/`UNDO` copies a command; `RESET` runs a destructive one/.test(readme)) {
    return "the README still claims UNDO always copies a command";
  }
  if (!/rescue/i.test(readme) || !/no backup|takes no backup|is the backup/i.test(readme)) {
    return "the README does not say the rescue branch takes no backup";
  }
  for (const needle of ["HEAD moves", "File contents change", "uncommitted work is discarded"]) {
    if (!readme.includes(needle)) return `the README matrix is missing "${needle}"`;
  }
  return true;
});

export default await suite.run();
