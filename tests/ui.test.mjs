/**
 * UI contract tests: keyboard, dialogs, focus trap, clipboard fallbacks,
 * accessibility attributes and the export paths.
 */
import { boot, createSuite } from "./harness.mjs";

const SEEN = { "git-undo-seen": "1", "git-undo-tour-seen": "1" };
const suite = createSuite("ui, keyboard, clipboard & export");

suite.test("j / k / arrows navigate the timeline", async () => {
  const { app, press } = await boot(SEEN);
  app("loadDemoData(true)");
  app("selectEntry(0)");
  press("j");
  const after = app("selectedIndex");
  press("k");
  const back = app("selectedIndex");
  press("ArrowDown");
  const arrow = app("selectedIndex");
  return after === 1 && back === 0 && arrow === 1 || `j=${after} k=${back} arrow=${arrow}`;
});

suite.test("u opens the undo dialog, Esc closes it", async () => {
  const { app, press, open } = await boot(SEEN);
  app("loadDemoData(true)");
  press("u");
  const opened = open("confirm-modal");
  press("Escape");
  return opened && !open("confirm-modal") || "dialog open/close failed";
});

suite.test("? opens shortcuts, Esc closes", async () => {
  const { press, open } = await boot(SEEN);
  press("?");
  const opened = open("help-modal");
  press("Escape");
  return opened && !open("help-modal") || "help modal failed";
});

suite.test("/ focuses search", async () => {
  const { d, press } = await boot(SEEN);
  press("/");
  return d.activeElement === d.getElementById("search-input") || "focus moved to " + (d.activeElement && d.activeElement.id);
});

suite.test("Esc is layered: dialog -> search -> filter", async () => {
  const { app, press, d } = await boot(SEEN);
  app("loadDemoData(true)");
  app("openHelpModal()");
  press("Escape");
  if (app("getOpenModal()")) return "dialog did not close first";
  d.getElementById("search-input").value = "stripe";
  app("handleSearch('stripe')");
  press("Escape");
  if (app("searchQuery") !== "") return "search not cleared";
  app("setFilter('danger')");
  press("Escape");
  return app("currentFilter") === "all" || "filter not reset";
});

suite.test("typing UNDO unlocks the confirm button", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true)");
  app("openConfirmModal()");
  app("handleConfirmInput('und')");
  const locked = d.getElementById("btn-confirm-undo").disabled;
  app("handleConfirmInput('UNDO')");
  const unlocked = !d.getElementById("btn-confirm-undo").disabled;
  return locked && unlocked || `locked=${locked} unlocked=${unlocked}`;
});

suite.test("confirming fires confetti + chime and inserts a simulated row", async () => {
  const { app } = await boot(SEEN);
  app("loadDemoData(true)");
  let confetti = false;
  let chime = false;
  app("window.__c = launchConfetti; window.__ch = playChime; launchConfetti = () => { window.__calls = true; }; playChime = (t) => { window.__chime = t; };");
  app("openConfirmModal(); handleConfirmInput('UNDO'); executeVisualUndo();");
  confetti = app("!!window.__calls");
  chime = app("window.__chime");
  const top = JSON.parse(app("JSON.stringify(reflogEntries[0])"));
  return confetti && chime === "success" && top.source === "simulated-undo" || `confetti=${confetti} chime=${chime}`;
});

suite.test("no alert() is reachable from any UI action", async () => {
  const { app, d, alerts } = await boot(SEEN);
  app("openImportModal(); parseAndApplyReflog();");
  d.getElementById("import-textarea").value = "garbage";
  app("parseAndApplyReflog(); closeImportModal(); loadDemoData(true); resetMockData();");
  return alerts.length === 0 || "alert fired: " + alerts.join(" | ");
});

suite.test("clipboard falls back to execCommand when navigator.clipboard is absent", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true)");
  let used = false;
  d.execCommand = () => { used = true; return true; };
  await app("copyCommand()");
  return used || "execCommand fallback not used";
});

suite.test("clipboard failure selects the command on screen and says so", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true)");
  d.execCommand = () => false;
  app("window.__toast = null; showToast = (t, m) => { window.__toast = t + ' | ' + m; };");
  await app("copyCommand()");
  const toast = app("window.__toast") || "";
  const command = d.getElementById("generated-command").textContent.trim();
  const selection = String(app("window.getSelection().toString()") || "");
  const selected = selection.includes(command);
  return toast.includes("manually") && selected || `toast=${toast} selected="${selection}" command="${command}"`;
});

suite.test("copy diff includes every file; Shift+click copies the active one", async () => {
  const { app, d } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0);");
  let copied = "";
  d.execCommand = () => {
    const ta = d.querySelector("textarea[readonly]");
    copied = ta ? ta.value : "";
    return true;
  };
  await app("copyCurrentDiff(false)");
  const full = copied.includes("jwt-verifier.ts") && copied.includes("webhook.ts");
  await app("copyCurrentDiff(true)");
  const single = !copied.includes("webhook.ts");
  return full && single || `full=${full} single=${single}`;
});

suite.test("patch export contains every file of the entry", async () => {
  const { app, w } = await boot(SEEN);
  app("loadDemoData(true); selectEntry(0);");
  let patch = "";
  const RealBlob = w.Blob;
  w.Blob = function (parts, opts) { patch = parts.join(""); return new RealBlob(parts, opts); };
  w.URL.createObjectURL = () => "blob:test";
  w.URL.revokeObjectURL = () => {};
  w.HTMLAnchorElement.prototype.click = () => {};
  app("downloadPatchFile()");
  return patch.includes("src/auth/jwt-verifier.ts") &&
    patch.includes("src/routes/webhook.ts") &&
    patch.includes("2 files changed") || "patch is missing files";
});

suite.test("patch is downloadable for a real git log import too", async () => {
  const { app, d, w } = await boot(SEEN);
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
  let patch = "";
  const RealBlob = w.Blob;
  w.Blob = function (parts, opts) { patch = parts.join(""); return new RealBlob(parts, opts); };
  w.URL.createObjectURL = () => "blob:test";
  w.URL.revokeObjectURL = () => {};
  w.HTMLAnchorElement.prototype.click = () => {};
  app("downloadPatchFile()");
  return patch.includes("diff --git a/c.txt b/c.txt") && patch.includes("+x") || "patch missing real diff";
});

suite.test("reflog rows are keyboard reachable options", async () => {
  const { app, $$, d } = await boot(SEEN);
  app("loadDemoData(true)");
  const rows = $$("#reflog-list [role=option]");
  const focusable = rows.every((r) => r.getAttribute("tabindex") === "0");
  const labelled = rows.every((r) => (r.getAttribute("aria-label") || "").length > 10);
  return rows.length > 0 && focusable && labelled &&
    d.getElementById("reflog-list").getAttribute("role") === "listbox" || "row semantics incomplete";
});

suite.test("Enter on a focused row selects it", async () => {
  const { app, $$, w } = await boot(SEEN);
  app("loadDemoData(true)");
  const rows = $$("#reflog-list [role=option]");
  rows[4].dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  return app("selectedIndex") === 4 || "selectedIndex = " + app("selectedIndex");
});

suite.test("icon-only buttons all have accessible names", async () => {
  const { $$ } = await boot(SEEN);
  const missing = $$("button")
    .filter((b) => !b.textContent.trim() && !b.getAttribute("aria-label") && !b.getAttribute("title"))
    .map((b) => b.outerHTML.slice(0, 60));
  return missing.length === 0 || missing.join(" | ");
});

suite.test("dialogs expose role, aria-modal, labels and descriptions", async () => {
  const { d } = await boot(SEEN);
  const problems = [];
  for (const id of ["confirm-modal", "import-modal", "help-modal"]) {
    const el = d.getElementById(id);
    if (el.getAttribute("role") !== "dialog") problems.push(`${id} role`);
    if (el.getAttribute("aria-modal") !== "true") problems.push(`${id} aria-modal`);
    const labelledBy = el.getAttribute("aria-labelledby");
    if (!labelledBy || !d.getElementById(labelledBy)) problems.push(`${id} label`);
  }
  if (!d.getElementById("confirm-modal").getAttribute("aria-describedby")) problems.push("confirm description");
  return problems.length === 0 || problems.join(", ");
});

suite.test("Tab is trapped inside the open dialog", async () => {
  const { app, d, w } = await boot(SEEN);
  app("loadDemoData(true); openConfirmModal();");
  const modal = d.getElementById("confirm-modal");
  const items = Array.from(modal.querySelectorAll("button:not([disabled]), input, textarea, [href]"));
  const last = items[items.length - 1];
  last.focus();
  d.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Tab", bubbles: true }));
  return modal.contains(d.activeElement) || "focus escaped to " + (d.activeElement && d.activeElement.tagName);
});

suite.test("text is selectable outside app chrome", async () => {
  const { $$, d } = await boot(SEEN);
  const selectsNone = (el) => {
    const cs = d.defaultView.getComputedStyle(el);
    return cs.userSelect === "none" || cs.webkitUserSelect === "none";
  };
  const bodySelectable = !selectsNone(d.body);
  const titlebarLocked = selectsNone(d.querySelector(".app-titlebar"));
  return bodySelectable && titlebarLocked || `body=${bodySelectable} titlebar=${titlebarLocked}`;
});

suite.test("close button explains itself, then asks the browser to close", async () => {
  const { app, w, d } = await boot(SEEN);
  // Real browsers refuse window.close() for tabs they didn't open; stub it so the
  // document survives long enough to read the toast (jsdom's close tears it down).
  let closeCalled = false;
  w.close = () => { closeCalled = true; };
  app("closeApp()");
  const toast = d.getElementById("toast-title").textContent + " | " + d.getElementById("toast-msg").textContent;
  return closeCalled && /close/i.test(toast) || `closeCalled=${closeCalled} toast=${toast}`;
});

suite.test("maximise uses the Fullscreen API, zen mode toggles panels", async () => {
  const { app, d } = await boot(SEEN);
  let requested = false;
  d.documentElement.requestFullscreen = () => { requested = true; return Promise.resolve(); };
  app("toggleMaximize()");
  app("toggleZenMode()");
  const zenOn = d.body.classList.contains("zen-mode");
  app("toggleZenMode()");
  const zenOff = !d.body.classList.contains("zen-mode");
  return requested && zenOn && zenOff || `fullscreen=${requested} zenOn=${zenOn} zenOff=${zenOff}`;
});

suite.test("the 15s tour can be cancelled and replayed", async () => {
  const { app, w } = await boot(SEEN);
  app("start15SecDemoTour()");
  await new Promise((r) => w.setTimeout(r, 200));
  const started = app("isTourRunning");
  app("cancelTour(true)");
  await new Promise((r) => w.setTimeout(r, 2200));
  const released = !app("isTourRunning");
  app("start15SecDemoTour()");
  await new Promise((r) => w.setTimeout(r, 100));
  const replay = app("isTourRunning");
  app("cancelTour(true)");
  return started && released && replay || `started=${started} released=${released} replay=${replay}`;
});

suite.test("closing the confirm dialog mid-tour cancels the tour", async () => {
  const { app, w } = await boot(SEEN);
  app("start15SecDemoTour()");
  await new Promise((r) => w.setTimeout(r, 200));
  app("closeConfirmModal()");
  await new Promise((r) => w.setTimeout(r, 100));
  return app("isTourRunning") === false || "tour kept running after the dialog was closed";
});

export default await suite.run();
