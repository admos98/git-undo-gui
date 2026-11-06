/**
 * Tests for the scope features: action-aware command generation, strategy
 * selection, multi-entry recovery ranges, undo-push sequence and the incident
 * report. These encode "what the app is for", so regressions here matter most.
 */
import { boot, createSuite } from "./harness.mjs";

const SEEN = { "git-undo-seen": "1", "git-undo-tour-seen": "1" };
const suite = createSuite("recovery features");

const REFLOG_WITH_PUSH = [
  "f55a66b HEAD@{0}: push: forced-update origin/main",
  "e44f55f HEAD@{1}: commit: feat: billing retry",
  "d33e44f HEAD@{2}: branch: Created from HEAD",
  "c22d33e HEAD@{3}: checkout: moving from main to feat/x",
  "b11b22c HEAD@{4}: stash: WIP on main: 1234abc fix",
].join("\n");

const cmdFor = async (strategy, index = 0) => {
  const { app, d } = await boot({ ...SEEN, "git-undo-imported-reflog": REFLOG_WITH_PUSH, "git-undo-import-mode": "reflog" });
  app(`selectEntry(${index}); updateStrategy('${strategy}')`);
  return d.getElementById("generated-command").textContent.trim();
};

suite.test("commands are action-aware, not a blind reset --hard", async () => {
  // index 4 -> "stash:" entry, index 3 -> "checkout:", index 2 -> "branch:"
  const stash = await cmdFor("hard", 4);
  const checkout = await cmdFor("hard", 3);
  const branch = await cmdFor("hard", 2);
  const problems = [];
  if (!stash.startsWith("git stash apply")) problems.push("stash -> " + stash);
  if (!checkout.startsWith("git switch --detach")) problems.push("checkout -> " + checkout);
  if (!branch.startsWith("git switch -c recovered-")) problems.push("branch -> " + branch);
  return problems.length === 0 || problems.join(" | ");
});

suite.test("commit entries still reset, and rescue branch always branches", async () => {
  const commit = await cmdFor("hard", 1);
  const rescue = await cmdFor("branch", 1);
  return commit === "git reset --hard e44f55f" && rescue === "git branch rescue-e44f55f e44f55f" || `${commit} / ${rescue}`;
});

suite.test("restore strategy targets file contents only", async () => {
  const restore = await cmdFor("restore", 1);
  return restore === "git restore --source=e44f55f -- ." || restore;
});

suite.test("the safest strategy is the default", async () => {
  const { app } = await boot(SEEN);
  app("loadDemoData(true)");
  return app("selectedStrategy") === "branch" || "default is " + app("selectedStrategy");
});

suite.test("strategy cards reflect the active choice", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true); updateStrategy('hard')");
  const hardActive = d.getElementById("strategy-card-hard").className.includes("f7768e");
  const softIdle = !d.getElementById("strategy-card-soft").className.includes("7dcfff");
  app("updateStrategy('restore')");
  const restoreActive = d.getElementById("strategy-card-restore").className.includes("73daca");
  const hint = d.getElementById("strategy-hint").textContent;
  return hardActive && softIdle && restoreActive && hint.length > 20 || "cards not painted";
});

suite.test("1-4 keys switch strategy", async () => {
  const { app, d, w } = await boot(SEEN);
  app("loadDemoData(true)");
  const press = (key) => d.body.dispatchEvent(new w.KeyboardEvent("keydown", { key, bubbles: true }));
  press("1"); const soft = app("selectedStrategy");
  press("3"); const restore = app("selectedStrategy");
  press("4"); const hard = app("selectedStrategy");
  press("2"); const branch = app("selectedStrategy");
  return [soft, restore, hard, branch].join(",") === "soft,restore,hard,branch" || `${soft},${restore},${hard},${branch}`;
});

suite.test("a range targets the state *before* it, not the range itself", async () => {
  // Reflog indexes run newest (HEAD@{0}) to oldest. Undoing entries 0..3 must
  // return to entry 4 — targeting entry 3 (or 0) would be a no-op or a mis-reset.
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0); setRangeStart(0); selectEntry(3);");
  const bounds = JSON.parse(app("JSON.stringify(rangeBounds())"));
  const expected = app("reflogEntries[4].hash");
  const cmd = d.getElementById("generated-command").textContent;
  const chip = d.getElementById("range-count").textContent;
  const rows = d.querySelectorAll('#reflog-list [aria-selected="true"]').length;
  return bounds.count === 4 &&
    bounds.targetIndex === 4 &&
    cmd.endsWith(expected) &&
    chip === "4 entries" &&
    rows === 4 || `count=${bounds.count} target=${bounds.targetIndex} cmd=${cmd} chip=${chip} rows=${rows}`;
});

suite.test("a range that reaches the oldest entry is reported as exhausted", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0); setRangeStart(0); selectEntry(5);");
  const bounds = JSON.parse(app("JSON.stringify(rangeBounds())"));
  const hint = d.getElementById("range-hint").textContent;
  return bounds.exhausted === true && hint.includes("older than this paste") || `exhausted=${bounds.exhausted} hint=${hint}`;
});

suite.test("range resets cleanly and Esc clears it", async () => {
  const { app, d, w } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0); setRangeStart(0); selectEntry(2);");
  d.body.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  return app("rangeStartIndex") === null || "range survived Esc";
});

suite.test("shift-click in the list starts a range", async () => {
  const { app, d, w } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0);");
  const rows = Array.from(d.querySelectorAll("#reflog-list [role=option]"));
  rows[2].dispatchEvent(new w.MouseEvent("click", { bubbles: true, shiftKey: true }));
  return JSON.parse(app("JSON.stringify(rangeBounds())")).count === 3 || "shift-click did not extend";
});

suite.test("undo-push panel appears only when the reflog has a push", async () => {
  const withPush = await boot({ ...SEEN, "git-undo-imported-reflog": REFLOG_WITH_PUSH, "git-undo-import-mode": "reflog" });
  const without = await boot({ ...SEEN, "git-undo-imported-reflog": "a43c821 HEAD@{0}: commit: fix", "git-undo-import-mode": "reflog" });
  const shown = !withPush.d.getElementById("push-panel").classList.contains("hidden");
  const hidden = without.d.getElementById("push-panel").classList.contains("hidden");
  return shown && hidden || `shown=${shown} hidden=${hidden}`;
});

suite.test("undo-push sequence is safe: backup branch + force-with-lease", async () => {
  const { app, d } = await boot({ ...SEEN, "git-undo-imported-reflog": REFLOG_WITH_PUSH, "git-undo-import-mode": "reflog" });
  const seq = d.getElementById("push-sequence").textContent;
  const checks = [
    seq.includes("git branch backup-before-recovery"),
    seq.includes("--force-with-lease"),
    !/git push --force origin/.test(seq),
    seq.includes("git reflog"),
  ];
  return checks.every(Boolean) || "unsafe sequence: " + seq;
});

suite.test("incident report is markdown, escaped, and states the data source", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0); updateStrategy('branch');");
  const report = app("buildIncidentReport()");
  const checks = [
    report.startsWith("## git-undo-gui"),
    report.includes("demo sample data"),
    report.includes("```bash"),
    report.includes("git branch rescue-"),
    report.includes("Safety checklist"),
    report.includes("No repository was read"),
  ];
  return checks.every(Boolean) || "report missing sections";
});

suite.test("incident report for an imported reflog says so, not 'demo'", async () => {
  const { app } = await boot({ ...SEEN, "git-undo-imported-reflog": "a43c821 HEAD@{0}: commit: fix", "git-undo-import-mode": "reflog" });
  const report = app("buildIncidentReport()");
  return report.includes("pasted git reflog") && !report.includes("demo sample data") || "source mislabelled";
});

suite.test("oversized paste is refused with an explanation, not a hang", async () => {
  const { app, d, text } = await boot(SEEN);
  app("openImportModal()");
  d.getElementById("import-textarea").value = "a43c821 HEAD@{0}: commit: " + "x".repeat(2_100_000);
  app("parseAndApplyReflog()");
  const msg = text("import-error");
  return msg.includes("2 MB") && app("reflogEntries.length") === 0 || "oversized paste not refused: " + msg.slice(0, 80);
});

suite.test("empty state keeps every panel coherent", async () => {
  const { d, app } = await boot(SEEN);
  app("resetMockData()");
  const problems = [];
  if (d.getElementById("generated-command").textContent !== "# nothing selected") problems.push("command");
  if (d.getElementById("deck-hash").textContent !== "—") problems.push("deck hash");
  if (!d.getElementById("onboarding-hero").hidden === false) problems.push("hero");
  if (!d.getElementById("push-panel").classList.contains("hidden")) problems.push("push panel");
  app("openConfirmModal()");
  if (app("!!getOpenModal()")) problems.push("undo modal opened with nothing selected");
  return problems.length === 0 || problems.join(", ");
});

export default await suite.run();
