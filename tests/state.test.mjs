/**
 * Behaviour tests for first-run onboarding, import persistence and the demo/import
 * state machine. Each test boots a fresh jsdom from the shipped index.html.
 */
import { boot, createSuite, wait } from "./harness.mjs";

const suite = createSuite("onboarding, import & state");

suite.test("first visit: onboarding hero, empty timeline, zero entries", async () => {
  const { d, text, app } = await boot();
  const hero = d.getElementById("onboarding-hero");
  if (hero.hidden) return "hero is hidden";
  if (!hero.textContent.includes("No reflog yet")) return "hero copy missing";
  if (text("reflog-count-badge") !== "0") return "count badge is not 0";
  return app("dataMode") === "empty" || "mode is " + app("dataMode");
});

suite.test("first visit: hero offers both paths", async () => {
  const { app, $$ } = await boot();
  const labels = $$("#onboarding-hero button").map((b) => b.textContent.trim());
  return labels.includes("Explore Demo Disaster") && labels.includes("Import Reflog") || labels.join(", ");
});

suite.test("first visit: auto tour plays after ~2s and labels itself", async () => {
  const { w, d, app, text } = await boot({}, { settle: 10 });
  if (app("dataMode") !== "empty") return "should start empty";
  await wait(w, 2300);
  const badge = text("mode-badge");
  const running = app("isTourRunning");
  if (badge !== "DEMO MODE") return "tour did not load demo data (badge " + badge + ")";
  return running || "tour finished too fast to observe";
});

suite.test("returning visit: no hero, no auto tour", async () => {
  const { w, d, app } = await boot({
    "git-undo-seen": "1",
    "git-undo-tour-seen": "1",
    "git-undo-imported-reflog": "a43c821 HEAD@{1}: commit: fix",
    "git-undo-import-mode": "reflog",
  }, { settle: 10 });
  const heroHidden = d.getElementById("onboarding-hero").hidden;
  await wait(w, 2300);
  return heroHidden && !app("isTourRunning") && app("dataMode") === "imported" || "hero/tour/state mismatch";
});

suite.test("import persists and survives a reload with its mode", async () => {
  const { d, app, w } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = "a43c821 HEAD@{1}: commit: fix";
  app("parseAndApplyReflog()");
  if (app("reflogEntries.length") !== 1) return "import not applied";
  if (w.localStorage.getItem("git-undo-import-mode") !== "reflog") return "mode not stored";

  const reloaded = await boot(Object.fromEntries(
    Object.keys(w.localStorage).map((k) => [k, w.localStorage.getItem(k)])
  ));
  return reloaded.app("reflogEntries.length") === 1 &&
    reloaded.app("importKind") === "reflog" &&
    reloaded.d.getElementById("onboarding-hero").hidden || "reload lost the import";
});

suite.test("git log paste renders real files (not reflog.txt)", async () => {
  const { d, app, $$ } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = `commit 51ee48b111111111111111111111111111111aaa (HEAD -> main)
Author: T <t@t.io>
Date:   Fri Nov 6 15:52:47 2026 +0000

    feat: add c.txt

diff --git a/c.txt b/c.txt
new file mode 100644
--- /dev/null
+++ b/c.txt
@@ -0,0 +1 @@
+x
`;
  app("parseAndApplyReflog()");
  const tabs = $$("#file-tabs button").map((b) => b.textContent.trim());
  const badge = d.getElementById("mode-badge").textContent;
  return tabs.some((t) => t.startsWith("c.txt")) && badge === "GIT LOG" || `tabs=${tabs} badge=${badge}`;
});

suite.test("empty paste shows an inline error, never an alert", async () => {
  const { d, app, alerts, text } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  app("openImportModal()");
  app("parseAndApplyReflog()");
  const shown = !d.getElementById("import-error").classList.contains("hidden");
  return shown && text("import-error").length > 0 && alerts.length === 0 || "inline error missing";
});

suite.test("unparseable paste explains the expected formats", async () => {
  const { d, app, text } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = "just some text\nand more";
  app("parseAndApplyReflog()");
  const msg = text("import-error");
  return msg.includes("Could not parse") && msg.includes("git reflog") || msg;
});

suite.test("switching to the demo keeps the stored import", async () => {
  const { d, app, w } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = "a43c821 HEAD@{1}: commit: fix";
  app("parseAndApplyReflog()");
  app("loadDemoData(true)");
  const kept = w.localStorage.getItem("git-undo-imported-reflog");
  return kept && app("dataMode") === "demo" || "demo load dropped the import";
});

suite.test("forget-import clears storage and returns to the empty state", async () => {
  const { d, app, w } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = "a43c821 HEAD@{1}: commit: fix";
  app("parseAndApplyReflog()");
  app("resetMockData()");
  return w.localStorage.getItem("git-undo-imported-reflog") === null &&
    app("dataMode") === "empty" &&
    !d.getElementById("onboarding-hero").hidden || "reset did not clear everything";
});

suite.test("demo data is labelled as demo, never as the user's history", async () => {
  const { app, text, d } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  app("loadDemoData(true)");
  if (text("mode-badge") !== "DEMO MODE") return "badge is " + text("mode-badge");
  if (text("current-file-tag") !== "DEMO SAMPLE FILE") return "file tag not marked as demo";
  return !d.getElementById("emergency-banner").hidden || "demo banner missing";
});

suite.test("the emergency banner is never shown over a real import", async () => {
  const { d, app } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  d.getElementById("import-textarea").value = "a43c821 HEAD@{1}: reset: moving to HEAD~3";
  app("parseAndApplyReflog()");
  return d.getElementById("emergency-banner").style.display === "none" || "banner leaked into imported mode";
});

suite.test("demo diff stats match the rendered lines (no invented numbers)", async () => {
  const { app } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
  app("loadDemoData(true)");
  const bad = JSON.parse(app(`JSON.stringify(reflogEntries.flatMap(e => e.files.filter(f =>
    f.additions !== f.diff.filter(l => l.type === "add").length ||
    f.deletions !== f.diff.filter(l => l.type === "del").length
  ).map(f => f.name)))`));
  return bad.length === 0 || "mismatched files: " + bad.join(", ");
});

export default await suite.run();
