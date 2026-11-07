# git-undo-gui — Visual Git Undo Button & Reflog Time Machine ⏪

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/admos98/git-undo-gui?style=flat&logo=github&label=stars)](https://github.com/admos98/git-undo-gui/stargazers)
[![Bundle size](https://img.shields.io/badge/index.html-196%20KB%20raw%20%C2%B7%2048%20KB%20gzip-7dcfff)](index.html)
[![Offline](https://img.shields.io/badge/offline-100%25%2C%20no%20CDN-9ece6a)](#technical-specifications)
[![Dependencies](https://img.shields.io/badge/runtime%20deps-0-brightgreen)](index.html)

> **Turn `git reflog` into a safe, visual "undo" workflow with side-by-side diff preview.**
> Tokyo Night dark mode. Single self-contained `index.html`.

**The point of the app is the reset.** You point at the snapshot you want back, choose how aggressive the reset
should be (`--soft`, a rescue branch, file-only restore, or `--hard`), type `UNDO`, and get the exact command to run.
It is a *reset planner with a diff preview*, not a log viewer: everything else — the timeline, the diff panel, the
push helper, the incident report — exists to make that one decision safe.

### 📌 Demo Status — please read

**The browser build plans a reset; it does not execute one.** This is a reset planner and a safe command generator. In the browser it does **not** run git, does not open a repository, and does not shell out to anything. It parses the reflog / `git log -p` / `git show` text that *you* paste in, shows you what each entry means with real per-file diffs, and writes the exact command you'd need. You stay in control of your terminal.

There is one deliberate exception: an **optional desktop build** (Tauri, ~5–8 MB) can run the
reset for you through a four-command, guard-checked bridge. It is opt-in, it never touches the
web build, and it takes a `backup-before-recovery` branch first. Everything below describes the
static build unless it says "desktop".

- Sample data is 6 hand-written entries, clearly labelled **DEMO MODE** in the titlebar.
- Data you import is labelled **REFLOG** or **GIT LOG** and stays in your browser's `localStorage` only.
- The undo button always says **UNDO TO THIS SNAPSHOT**. In the browser it copies the reset command for
  you to run; only the desktop build (with a repository open) runs it, after a backup and a typed
  confirmation. In demo mode the same flow runs against sample data and is labelled **simulated**.

---

## 📽️ The 15-Second Demo

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│  🔴 🟡 🟢  git-undo-gui — ~/workspace/billing-api (branch: main)      [15s Demo Tour]  │
├────────────────────┬──────────────────────────────────────┬────────────────────────────┤
│ REFLOG TIMELINE    │ DIFF PREVIEW (Word-level Highlight)  │ SAFE UNDO ENGINE           │
│                    │                                      │                            │
│ ⚠️ reset --hard    │  src/auth/jwt-verifier.ts            │ Target: a43c821 (25m ago)  │
│    HEAD@{0} 10m    │  @@ -24,8 +24,12 @@                  │ feat: stripe webhook sigs  │
│                    │  - if (!decoded) throw Error();      │                            │
│ 🔥 force push      │  + if (!decoded?.header?.kid) throw; │ [⚡ UNDO TO THIS SNAPSHOT] │
│    HEAD@{1} 25m    │    (Token-level green/red highlight) │                            │
│                    │                                      │ Safety: "Type UNDO" Modal  │
│ 🗑️ deleted branch │                                      │   [ U N D O ]              │
│    HEAD@{2} 1h     │                                      │   🎉 Victory Confetti!     │
└────────────────────┴──────────────────────────────────────┴────────────────────────────┘
```

The tour auto-plays **once** on your first visit (2s after load). Replay it any time with <kbd>t</kbd> or the **15s Demo Tour** button.

To record your own GIF: press <kbd>t</kbd>, then capture with [Peek](https://github.com/phw/peek), [vhs](https://github.com/charmbracelet/vhs), or [ScreenToGif](https://www.screentogif.com/).

---

## 💡 Why Not the Raw `git reflog`?

Command-line output is great until you press <kbd>Enter</kbd> on a destructive command at 2 AM. Here's how **git-undo-gui** compares:

| Feature | `git-undo-gui` 🚀 | Raw `git reflog` 📟 | `git-time-machine` TUI ⏳ |
| :--- | :---: | :---: | :---: |
| **Word-Level Diff Highlighting** | ✅ Yes | ❌ Raw terminal lines | ⚠️ Line-level only |
| **Accidental Enter Protection** | ✅ **"Type UNDO"** modal | ❌ Instant destructive reset | ❌ None |
| **Non-Destructive Rescue Branch** | ✅ One-click command generator | ❌ Manual typing | ❌ Not available |
| **Installation Overhead** | ✅ **0 MB** (open `index.html`) | ⚙️ Git CLI only | 📦 Node/Rust/Python toolchain |
| **Visual Disaster Timeline** | ✅ Badged: `reset`, `force push`, `branch` | ❌ Monochromatic SHA wall | ⚠️ Basic log list |
| **Import Terminal Reflog** | ✅ Robust regex parser | ❌ N/A | ❌ Direct repository only |
| **Download `.patch` File** | ✅ Yes (all files in the entry) | ❌ Requires piping git commands | ❌ No |
| **Aesthetics** | 🌌 **Tokyo Night**, chimes, confetti | ⬛ Terminal monochrome | 🔲 Curses terminal box |

---

## ✨ Features

### 1. 🕒 Reflog Timeline Inspector
- Filtered view of `git reflog` events (`HEAD@{0}`, `HEAD@{1}`, …) with decorative ref info preserved exactly as git printed it.
- Pre-categorized badges: `reset`, `force push`, `rebase`, `branch`, `checkout`, `stash`, `amend`, `merge`, `cherry-pick`.
- Instant search by hash, action, or message (press <kbd>/</kbd>).
- Empty state hero on first load: **Explore Demo Disaster** or **Import Reflog**.

### 2. 🔍 Word-Level Diff Preview
- Token-level highlighting in Tokyo Night colours (`--tn-*` variables).
- **Unified** and **Split** (side-by-side) modes — both render real content.
- File tabs per commit; imported entries expose their raw reflog line as the diff body.

### 3. 🛡️ Safe Undo Engine
- Generates, never executes. Four strategies (`1`–`4`), defaulting to the one that cannot lose work:
  - **Soft Undo** (`1`) — `git reset --soft <hash>` (keeps files staged)
  - **Rescue Branch** (`2`, default) — `git branch rescue-<hash> <hash>` (zero risk)
  - **Restore Files** (`3`) — `git restore --source=<hash> -- .` (no HEAD move)
  - **Hard Reset** (`4`) — `git reset --hard <hash>` (full restoration)
- **Action-aware commands**: a `stash:` entry becomes `git stash apply`, a `checkout:` entry becomes
  `git switch --detach`, a `branch:` entry becomes `git switch -c recovered-…`. A blind
  `reset --hard` on every row would be wrong for most of them.
- **Safety interlock**: you must type `UNDO` before the confirm button unlocks.
- Web Audio chime + canvas confetti when you complete the flow.

### 3a. 🧰 Multi-entry recovery, undo-push, and reports
- **Recovery range** — Shift-click an entry (or <kbd>Shift</kbd>+<kbd>J</kbd>) to select "everything from here down".
  Indexes run newest→oldest, so undoing N selected actions targets the entry **after** the range — the last state
  before the actions you're undoing. If the range reaches the oldest entry in your paste, the UI says so instead of
  silently pointing at a state you're already in.
- **Undo last push** — when the reflog contains a push (especially a forced update), a dedicated panel hands you
  a reviewed 4-step sequence: inspect the reflog, create `backup-before-recovery`, reset the branch, and only then
  `git push --force-with-lease` — never a bare `--force`.
- **Incident report** (`r`) — copies a Markdown brief (entry, hash, selected strategy, command, change surface,
  safety checklist) ready to paste into a PR comment or team chat.

### 3b. 📋 Copy & export that actually works offline
- **Clipboard**: `navigator.clipboard` doesn't exist on `file://` or in some webviews — so a `textarea` + `execCommand` fallback runs first, and if the browser blocks both, the command is *selected on screen* and the toast says "press ⌘C/Ctrl+C".
- **Copy diff**: copies **every file** in the entry (Shift+click copies only the active tab).
- **Download `.patch`**: all files, per-file `diff --git` headers, `git apply --check` friendly.

### 4. 📋 Import — reflog **or** real patches
- `git reflog` (optionally `--date=iso`) → a timeline; every entry's diff body is the
  raw line you pasted (`Original reflog line: …`). Nothing is invented.
- `git log -p`, `git show <hash>`, `git format-patch`, or `git log --oneline` → parsed
  into commits with **real per-file diffs**: file tabs, hunks, +/- counts, word-level
  highlighting, split view. This is how the app shows real patches without ever
  executing git — you run the command, it reads the output.
- Three one-click samples in the import dialog (reflog / log -p / show) so you can see
  the difference before pasting your own.
- The import is remembered in `localStorage`, so a reload restores *your* history instead of the demo.
- Bad input shows an **inline error** with the line numbers it couldn't read — no `alert()` popups anywhere.

### 4b. 🧭 Honest state handling
- Demo and your import are **separate**: the **Demo** button loads samples without deleting your import; **Forget import** (the reset icon) clears stored data and returns to the empty state.
- If the browser blocks `localStorage` (typical for `file://` in Safari), the UI **says so** after import instead of pretending the data was saved.
- **Esc** is layered: close dialog → cancel a running tour → clear search → reset filter.
- Reflog entries that carry commit URLs (git's stock `log.showSignature`/alias output) get an **"open ↗"** link.
- `+N/-M` counters are derived from the rendered diff lines, so the numbers can't drift from the content.

### 5. 💾 Bonus Utilities
- **Terminal Steps**: a safe, copyable workflow (`git branch rescue-backup…` → reset → `git status`).

---

## ✅ Tests

The suite loads the shipped `index.html` into jsdom — it tests the artifact, not a build.

```bash
npm install
npm test              # 96 checks, ~17s
npm test -- parsers   # filter by suite file
```

| Suite | Covers |
| :--- | :--- |
| `tests/parsers.test.mjs` | reflog (plain, decorated, `--date=iso`, branch reflog, short hashes), `git show`, `git log -p`, `--oneline`, CRLF, unparseable input, count integrity |
| `tests/state.test.mjs` | first-run hero + auto tour, returning visit, import persistence across reload, demo/import switching, banner honesty, demo stats |
| `tests/ui.test.mjs` | j/k/arrows, `u`, `?`, `/`, layered Esc, `Type UNDO`, confetti+chime, clipboard fallbacks, copy/patch exports, row semantics, focus trap, tour cancel/replay |
| `tests/features.test.mjs` | action-aware commands, strategy cards + keys, recovery range, undo-push sequence, incident report, oversized paste, empty-state coherence |
| `tests/integrity.test.mjs` | no CDN/network/`alert`, inlined CSS freshness, CSP present, package.json hygiene, XSS regression, size budget |
| `tests/executor.test.mjs` | the web build executes nothing; the desktop UI calls only the four registered commands with matching argument names, strategy strings and confirm phrase; `RESET` gating; refusals surfaced; no bare `--force`; no server/thread/telemetry; dependency budget |

CI runs the suite on Node 20 + 22 and fails if `index.html` is stale relative to
`src/tailwind.css` (`.github/workflows/ci.yml`).

---

## 🔒 Security model

- **No network surface**: CSP `default-src 'none'` with `connect-src 'none'`, no remote
  script/style/img/font, and no `fetch`/XHR in the code. Your pasted reflog never leaves
  the tab.
- **No shell (web build)**: the browser build cannot read a repository or run git. It parses text
  and emits a reset command for *you* to review. A static file has no access to your repo, and a
  browser tab should never be able to run `reset --hard`. The desktop build can — see
  [the executor](#-desktop-executor-optional) for exactly what stops a mistake, and what does not.
- **No injection**: every user-derived string is escaped before it reaches `innerHTML`;
  a regression test pastes `<img onerror>` / `<script>` payloads and asserts nothing executes.
- **Persisted locally**: `localStorage` only, capped at 200k chars, and the app tells you
  when the browser blocks it (common on `file://`).

---

## 🖥️ Desktop executor (optional)

> **Status, honestly stated.** Everything below is verified by CI on every push, not asserted:
>
> | Check | Result |
> | :--- | :--- |
> | Guard crate: compile + clippy `-D warnings` + unit tests | **19/19 passing** |
> | Shell: clippy `-D warnings`, release build | **clean, 4 MiB binary** |
> | Reset driven against real throwaway repositories | **10/10 end-to-end scenarios** |
> | UI ↔ IPC contract (names, args, strategy strings, `RESET` gate) | **`tests/executor.test.mjs`** |
>
> The end-to-end tests build actual repositories with `git` and assert that a refusal leaves the
> working tree untouched, that `backup-before-recovery` exists before the reset it protects, and
> that a second reset cannot overwrite the first backup.
>
> There is still exactly one gap: **nobody has opened the window.** No human has clicked through
> the webview against a repository, because every environment this was written in has no desktop
> session. The destructive path is exercised; the *widgets* are not. Treat the first desktop
> release as a beta.

The web build stays exactly what it was: one static file, **0 runtime dependencies, 0 network
calls**, no repository access. The desktop build wraps *the same* file and adds the one thing a
static page cannot do — run the command it just planned.

### The bridge is four commands, and that is the whole surface

| Command | Signature | Touches your repo |
|---|---|---|
| `git_read_reflog` | `(path, limit?) -> String` | read only |
| `git_repo_state` | `(path) -> RepoState` | read only |
| `git_create_backup` | `(path) -> OpReport` | creates one branch, moves nothing |
| `git_reset` | `(path, target, strategy, confirm?) -> ResetOutcome` | the only mutating call |

No daemon, no local HTTP server, no background thread, no telemetry, no `reqwest`/`tokio`.
Each call spawns one short-lived `git` process with `GIT_TERMINAL_PROMPT=0` and
`GIT_OPTIONAL_LOCKS=0`, capped at 8 MB of output.

### What stops a mistake (in the order the guard runs)

1. **An operation is half-finished?** merge / rebase / cherry-pick / revert / bisect → refused.
2. **Hard reset that changes nothing?** → refused instead of pretending to work.
3. **Hard reset without acknowledgement?** the UI makes you type `RESET` (not `UNDO`) and the
   Rust side re-checks the same phrase — the frontend cannot skip this.
4. **Hard reset over uncommitted tracked files?** → refused. Commit or stash first. Untracked
   files do not block (they survive a reset), and they are listed so you can see that.
5. **Then** a `backup-before-recovery` branch is created at the pre-reset HEAD, and every
   success response carries an `undo` line that walks you back to it.

`--force` appears nowhere in the project. Push recovery uses `--force-with-lease`.

### What it does **not** protect you from

- **Unpushed commits.** The backup branch keeps them alive, but a `reset --hard` genuinely
  moves them off the branch. The UI shows the count before you confirm; read it.
- **A backup branch from an earlier session.** The name is fixed and idempotent, so a second
  reset does not silently overwrite the first backup — the UI says the backup predates this
  reset and to check `git log backup-before-recovery`. It is a safety net, not a time machine.
- **A bug in uncompiled code.** Treat the first desktop release as a beta: run it in a scratch
  clone once and watch what it does before pointing it at work you care about.

### Building it

```bash
npm install
npm run tauri:build        # builds the UI, syncs the embedded copy, then bundles
```

Under the hood that is `npm run build` → `npm run embed:web` → `tauri build`. The middle step
matters: a Tauri webview can only load an `http(s)` URL, so the shell **serves the page from
inside the binary** through a custom protocol — no server, no port. `scripts/embed-web.mjs`
keeps the compiled-in copy identical to `dist/index.html` (the build has no way to read
`dist/` at compile time on a clean checkout), and a test fails if the two ever drift.

To work on the guard rules alone, no Tauri needed:

```bash
cd src-tauri/logic
cargo test --no-default-features     # 19 tests, zero dependencies
```

CI (`.github/workflows/compile-check.yml`) compiles both crates with `-D warnings`, runs the
guard unit tests and the end-to-end repository tests, and builds + weighs the release binary,
failing above 20 MiB. Failures are re-emitted as check annotations on the offending line, so a
compile error is readable without downloading job logs. `cargo fmt --check` is advisory only:
formatting is not a correctness signal, and this code was authored where rustfmt cannot run.

The icon is committed (`src-tauri/icons/`, generated from a hand-drawn undo glyph — no
third-party art). macOS and Windows **installers** additionally need those hosts; cross-compiling
a Tauri bundle is not supported.

### Weight, measured and estimated

| Artifact | Size | Verified |
|---|---|---|
| `index.html` (web build) | 200 KB / **49 KB gzip** | measured |
| Runtime dependencies | **0** | measured |
| Network calls at runtime | **0** | measured |
| Desktop binary (Tauri v2, `opt-level="s"`, LTO, stripped) | **4 MiB** | measured by CI on every push; hard budget 20 MiB |
| Electron equivalent | ~150 MB | reference point, and it bundles Chromium |

The desktop build ships no browser engine: it uses the OS webview already on your machine.
That is the entire reason it is a few megabytes rather than a few hundred.

## 🚀 Quickstart

### Option A — Zero install (recommended)

```bash
git clone https://github.com/admos98/git-undo-gui.git
cd git-undo-gui
open index.html          # macOS
# xdg-open index.html    # Linux
# start index.html       # Windows
```

Double-click works: there is no build step required to *use* the shipped file.

### Option B — Local dev server

```bash
npm install
npm run dev        # recompiles Tailwind, serves on http://localhost:3000
npm run build      # static build into dist/
```

| Script | What it does |
| :--- | :--- |
| `npm run css:build` | Compiles `src/tailwind.css` and inlines the result into `index.html` |
| `npm run dev` | `css:build` + Vite dev server on `0.0.0.0:3000` |
| `npm run build` | `css:build` + static build to `dist/` |
| `npm test` | Run the jsdom suite against the shipped `index.html` |
| `npm run tauri:build` | Build the native shell from `src-tauri/` |

> Run `npm run css:build` after editing markup or class names — the CSS is **inlined**, not loaded at runtime.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| <kbd>j</kbd> / <kbd>↓</kbd> | Move to next reflog entry |
| <kbd>k</kbd> / <kbd>↑</kbd> | Move to previous reflog entry |
| <kbd>u</kbd> / <kbd>⌘</kbd>+<kbd>Z</kbd> | Open the safe undo confirmation dialog |
| <kbd>c</kbd> | Copy the generated git command |
| <kbd>d</kbd> | Download `.patch` for the current snapshot |
| <kbd>Shift</kbd>+<kbd>j</kbd>/<kbd>k</kbd> | Extend the recovery range |
| <kbd>1</kbd>–<kbd>4</kbd> | Pick undo strategy (soft / branch / restore / hard) |
| <kbd>r</kbd> | Copy the Markdown incident report |
| <kbd>t</kbd> | Replay the 15-second demo tour |
| <kbd>/</kbd> | Focus the reflog search box |
| <kbd>?</kbd> | Toggle keyboard shortcuts |
| <kbd>Esc</kbd> | Close dialog → cancel tour → clear search → reset filter |
| <kbd>Tab</kbd> | Cycle focus inside the reflog list / dialog (rows are reachable) |
| <kbd>Enter</kbd> | Select the focused reflog row |

The three titlebar dots are real browser controls, not decoration: **red** attempts to close the tab (browsers only allow this for script-opened tabs and will tell you so), **amber** toggles zen mode (hide side panels), **green** toggles the Fullscreen API.

---

## 🎯 Technical Specifications

- **Architecture**: one `index.html` with inlined CSS + vanilla JS. No framework, no bundler at runtime.
- **Size**: 196 KB raw / 48 KB gzip — about 30 KB of that is Tailwind CSS compiled ahead of time and
  inlined. (The badge above is checked against the file by `tests/integrity.test.mjs`, so it cannot
  drift from the artifact again.)
- **Network**: **none**. No `<script src>`, no `@import`, no fonts, no analytics. Works fully offline from `file://`.
- **Runtime dependencies**: zero.
- **Theme**: Tokyo Night (`#1a1b26`, `#16161e`, `#7dcfff`, `#bb9af7`, `#9ece6a`, `#f7768e`) via `--tn-*` variables.
- **Import limits**: up to ~2 MB of pasted text is parsed in-tab (larger pastes are refused with an explanation);
  only the first 200 KB is persisted for reloads.
- **Storage**: `localStorage` keys `git-undo-seen`, `git-undo-tour-seen`, `git-undo-imported-reflog`, `git-undo-import-mode` (capped at 200k chars, clearable with the reset button).
- **Accessibility**: text selectable by default (only app chrome opts out), one `h1`, `role="listbox"/"option"` timeline that is keyboard reachable (Tab + Enter), `role="dialog"` + `aria-modal` + `aria-describedby` + focus trap, `aria-live` toasts and counts, `aria-label` on every icon-only button, visible focus rings, `prefers-reduced-motion` respected.
- **Git safety**: the web build is read-only visualisation and command generation — it never
  invokes a subprocess. The optional desktop build runs git through four guarded commands; see
  [Desktop executor](#-desktop-executor-optional).

---

## 🆚 Similar projects (and what's different here)

| Project | Shape | Overlap with this repo | Where this repo differs |
| :--- | :--- | :--- | :--- |
| [lazygit](https://github.com/jesseduffield/lazygit) (~84k★) | Go TUI | `z` undo uses the reflog | Terminal-only; this is a zero-install browser view with word-level diff + a "type UNDO" interlock |
| [git-time-machine](https://github.com/dinakars777/git-time-machine) | Rust TUI | reflog timeline, j/k, one-key restore | **Actually executes** `reset --hard`; this one never runs git and now parses real patches instead |
| [pronzzz/time-machine](https://github.com/pronzzz/time-machine) | Rust TUI | reflog + live diff previews + safe revert | Same idea in the terminal; this needs no toolchain at all |
| [gitkeen](https://github.com/tgomilar/gitkeen) | Tauri desktop client | full git client with undo | Far broader scope (a real client); this is a single-purpose, read-only triage tool |
| GitButler / GitKraken / Sourcetree | Desktop GUI | undo & reflog views | Require install + repo access; this works on a pasted reflog from a machine you can't install on |
| [mrsaynothing/git-undo](https://github.com/mrsaynothing/git-undo) | web page | "one exact command out" | A Q&A command picker; this is a timeline of *your actual* reflog |

**Honest positioning**: the TUI tools above are strictly more capable — they read the repo and execute commands. This project's only defensible niche is *zero-install, zero-network, read-only triage of a reflog you already have in your clipboard*, with a UI that explains what each entry means before you touch a terminal.

---

## 🗺️ Roadmap (honest)

- [x] Real diffs from `git show` / `git log -p` paste (no backend needed).
- [x] Test suite + CI.
- [x] Tauri v2 desktop shell with a four-command, guard-checked bridge (folder picker → read
      reflog → optional real reset).
- [x] First real compile: CI builds the shell, runs the guard crate's 19 tests, and drives the
      reset against throwaway repositories (10 scenarios).
- [x] Release binary measured: **4 MiB**, enforced by a 20 MiB CI budget.
- [ ] **First real click-through.** Open the desktop window against a throwaway repository and
      use the widgets. Machine-verified below that line; this last step needs a desktop.
- [ ] Signed/notarised installers (needs macOS + Windows hosts and certificates).
- [ ] Undo across more than the current branch's reflog (branch/reflog selection).
- [x] Multi-entry selection ("undo everything since HEAD@{4}").
- [x] Copy-as-markdown incident report for PRs / team standups.
- [x] Action-aware command generation (stash / checkout / branch entries).
- [x] Undo-push sequence with `--force-with-lease`.

---

## 📄 License

MIT © 2026 git-undo-gui contributors — see [LICENSE](LICENSE).
