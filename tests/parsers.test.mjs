/**
 * Parser tests: the import path is the product, so it gets the deepest coverage.
 * Fixtures are real `git` output generated from a throwaway repository.
 */
import { boot, createSuite, wait } from "./harness.mjs";

const REFLOG = [
  "a43c821 HEAD@{1}: commit: fix",
  "a43c821 (HEAD -> main) HEAD@{0}: reset: moving to HEAD~3",
].join("\n");

const GIT_SHOW = `commit fa866702efa6a6b179da5a48039eb3baf0fa9e50
Author: Tester <t@t.io>
Date:   Fri Nov 6 15:52:47 2026 +0000

    fix(a): change line two and add a line

diff --git a/a.txt b/a.txt
index 0c2aa38..1952fd1 100644
--- a/a.txt
+++ b/a.txt
@@ -1,3 +1,4 @@
 line one
-line two
+line TWO changed
 line three
+line four added
diff --git a/b.txt b/b.txt
index ce01362..f31f52d 100644
--- a/b.txt
+++ b/b.txt
@@ -1 +1,2 @@
 hello
+world
`;

const GIT_LOG_P = `commit 51ee48b111111111111111111111111111111aaa (HEAD -> main)
Author: Tester <t@t.io>
Date:   Fri Nov 6 15:52:47 2026 +0000

    feat: add c.txt

diff --git a/c.txt b/c.txt
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/c.txt
@@ -0,0 +1 @@
+x

commit d50e9f3222222222222222222222222222222bb
Author: Tester <t@t.io>
Date:   Fri Nov 6 15:00:00 2026 +0000

    feat: initial files
`;

const suite = createSuite("import parsers");
const { w, app } = await boot({ "git-undo-seen": "1", "git-undo-tour-seen": "1" });
const parse = (text) => JSON.parse(app(`JSON.stringify(parseAnyPaste(${JSON.stringify(text)}))`));

suite.test("reflog: exact message is preserved", () => {
  const r = parse(REFLOG);
  if (r.kind !== "reflog") return "wrong kind: " + r.kind;
  const e = r.entries.find((x) => x.hash === "a43c821" && x.action === "commit: fix");
  if (!e) return "entry missing";
  if (e.files[0].name !== "reflog.txt") return "expected reflog.txt file";
  const raw = e.files[0].diff.some((l) => l.text === "Original reflog line: a43c821 HEAD@{1}: commit: fix");
  return raw || "raw line not preserved";
});

suite.test("reflog: decoration and HEAD@{n} survive", () => {
  const r = parse("a43c821 (HEAD -> main, origin/main) HEAD@{7}: reset: moving to HEAD~3");
  const e = r.entries[0];
  return e.ref === "HEAD@{7}" && e.branch.includes("origin/main") || `ref=${e.ref} branch=${e.branch}`;
});

suite.test("reflog: --date=iso variant parses", () => {
  const r = parse("a43c821 HEAD@{2026-11-06 10:22:13 +0100}: commit: fix");
  return r.matched === 1 && r.entries[0].action === "commit: fix" || "failed";
});

suite.test("reflog: an ISO stamp is never shown as the reflog id", () => {
  // `git reflog --date=iso` puts the timestamp inside the braces. Rendering it as
  // the id made every desktop row read `HEAD@{2026-11-06 10:22:13 +0100}`, which
  // is not a selector anyone can paste back into git.
  const r = parse("a43c821 HEAD@{2026-11-06 10:22:13 +0100}: commit: fix");
  const e = r.entries[0];
  if (e.ref.includes("20")) return `ref still holds the timestamp: ${e.ref}`;
  if (!e.ref.startsWith("HEAD@{")) return `unexpected ref ${e.ref}`;
  if (e.timeAgo === "imported" || !e.timeAgo) return `the age was dropped: ${e.timeAgo}`;
  return true;
});

suite.test("reflog: --date=relative keeps the selector and the age", () => {
  // git writes the age *inside* the braces for --date=relative, and that string
  // is resolvable (`git rev-parse 'HEAD@{1 second ago}'` works), so it is kept
  // verbatim rather than being replaced with an invented number.
  const r = parse("a43c821 HEAD@{0 seconds ago}: commit: c2");
  const e = r.entries[0];
  return e.ref === "HEAD@{0 seconds ago}" && e.timeAgo === "0 seconds ago"
    || `ref=${e.ref} timeAgo=${e.timeAgo}`;
});

suite.test("reflog: the index+age form still splits correctly", () => {
  const r = parse("a43c821 HEAD@{3, 2 days ago}: commit: c1");
  const e = r.entries[0];
  return e.ref === "HEAD@{3}" && e.timeAgo === "2 days ago"
    || `ref=${e.ref} timeAgo=${e.timeAgo}`;
});

suite.test("reflog: a plain index keeps the legacy 'imported' age", () => {
  const r = parse("a43c821 HEAD@{5}: commit: c1");
  const e = r.entries[0];
  return e.ref === "HEAD@{5}" && e.timeAgo === "imported"
    || `ref=${e.ref} timeAgo=${e.timeAgo}`;
});

suite.test("reflog: 6-8 char abbreviated hashes parse", () => {
  const r = parse("a43c82 HEAD@{1}: commit: short hash");
  return r.matched === 1 || "6-char hash rejected";
});

suite.test("reflog: branch reflog (refs/heads/main@{0}) parses", () => {
  const r = parse("a43c821 refs/heads/main@{0}: commit: fix");
  return r.matched === 1 && r.entries[0].ref === "HEAD@{0}" || "failed";
});

suite.test("show: single commit becomes per-file real diffs", () => {
  const r = parse(GIT_SHOW);
  if (r.kind !== "diff") return "wrong kind: " + r.kind;
  if (r.matched !== 1) return `matched ${r.matched}`;
  const e = r.entries[0];
  if (e.files.length !== 2) return `expected 2 files, got ${e.files.length}`;
  const a = e.files.find((f) => f.name === "a.txt");
  const types = a.diff.map((l) => l.type).join(",");
  if (types !== "hunk,context,del,add,context,add") return "unexpected line types: " + types;
  return a.additions === 2 && a.deletions === 1 || `stats +${a.additions}/-${a.deletions}`;
});

suite.test("show: author, subject and full hash are captured", () => {
  const e = parse(GIT_SHOW).entries[0];
  return e.hash === "fa866702efa6a6b179da5a48039eb3baf0fa9e50" &&
    e.author === "Tester <t@t.io>" &&
    e.action === "fix(a): change line two and add a line" || JSON.stringify({ h: e.hash, a: e.author, s: e.action });
});

suite.test("log -p: multiple commits, newest first, files kept apart", () => {
  const r = parse(GIT_LOG_P);
  if (r.matched !== 2) return `matched ${r.matched}`;
  const [first, second] = r.entries;
  if (first.action !== "feat: add c.txt") return "wrong order: " + first.action;
  if (first.files[0].name !== "c.txt") return "wrong file name: " + first.files[0].name;
  if (first.files[0].additions !== 1) return "wrong additions: " + first.files[0].additions;
  // The second commit carries no patch, so it must fall back to commit metadata
  // rather than inventing diff content.
  if (second.files.length !== 1 || second.files[0].name !== "commit.txt") {
    return "second commit shape: " + second.files.map((f) => f.name).join(",");
  }
  const meta = second.files[0].diff.map((l) => l.text).join("\n");
  return meta.includes("No patch was included") || "metadata fallback missing its explanation";
});

suite.test("log --oneline: hashes + subjects parse without a patch", () => {
  const r = parse("51ee48b feat: add c.txt\nfa86670 fix(a): change line two\n");
  return r.matched === 2 && r.entries[0].action === "feat: add c.txt" || JSON.stringify(r.entries.map((e) => e.action));
});

suite.test("unparseable input reports zero matches (no invented data)", () => {
  const r = parse("lorem ipsum dolor sit amet\nnot a reflog");
  return r.matched === 0 && r.kind === "reflog" || `matched=${r.matched} kind=${r.kind}`;
});

suite.test("reflog lines are never mistaken for one-line commits", () => {
  const r = parse(REFLOG);
  return r.kind === "reflog" || "reflog text was routed to the log parser";
});

suite.test("diff counts are derived from the parsed lines", () => {
  const e = parse(GIT_SHOW).entries[0];
  const bad = e.files.filter((f) => {
    const a = f.diff.filter((l) => l.type === "add").length;
    const del = f.diff.filter((l) => l.type === "del").length;
    return f.additions !== a || f.deletions !== del;
  });
  return bad.length === 0 || `${bad.length} files disagree with their own diff`;
});

suite.test("CRLF line endings parse the same as LF", () => {
  const r = parse(GIT_SHOW.replace(/\n/g, "\r\n"));
  return r.matched === 1 && r.entries[0].files.length === 2 || "CRLF broke parsing";
});

await wait(w, 10);
export default await suite.run();
