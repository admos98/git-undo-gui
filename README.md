# git-undo-gui — Visual Git Undo Button & Reflog Time Machine ⏪

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/admos98/git-undo-gui?style=flat&logo=github&label=stars)](https://github.com/admos98/git-undo-gui/stargazers)
[![Bundle size](https://img.shields.io/badge/index.html-180%20KB%20(30%20KB%20CSS%20inlined)-7dcfff)](index.html)
[![Offline](https://img.shields.io/badge/offline-100%25%2C%20no%20CDN-9ece6a)](#technical-specifications)
[![Dependencies](https://img.shields.io/badge/runtime%20deps-0-brightgreen)](index.html)

> **Turn `git reflog` into a safe, visual "undo" workflow with side-by-side diff preview.**
> Tokyo Night dark mode. Single self-contained `index.html`.

**The point of the app is the reset.** You point at the snapshot you want back, choose how aggressive the reset
should be (`--soft`, a rescue branch, file-only restore, or `--hard`), type `UNDO`, and get the exact command to run.
It is a *reset planner with a diff preview*, not a log viewer: everything else — the timeline, the diff panel, the
push helper, the incident report — exists to make that one decision safe.

### 📌 Demo Status — please read

**It plans a reset; it does not execute one.** This is a reset planner and a safe command generator. It does **not** run git, does not open a repository, and does not shell out to anything. It parses the reflog / `git log -p` / `git show` text that *you* paste in, shows you what each entry means with real per-file diffs, and writes the exact command you'd need. You stay in control of your terminal.

- Sample data is 6 hand-written entries, clearly labelled **DEMO MODE** in the titlebar.
- Data you import is labelled **REFLOG** or **GIT LOG** and stays in your browser's `localStorage` only.
- The undo button always says **UNDO TO THIS SNAPSHOT** and always ends in "copies the reset command — run it in
  your terminal". In demo mode the same flow runs against sample data and is labelled **simulated**.

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
npm test              # 77 checks, ~14s
npm test -- parsers   # filter by suite file
```

| Suite | Covers |
| :--- | :--- |
| `tests/parsers.test.mjs` | reflog (plain, decorated, `--date=iso`, branch reflog, short hashes), `git show`, `git log -p`, `--oneline`, CRLF, unparseable input, count integrity |
| `tests/state.test.mjs` | first-run hero + auto tour, returning visit, import persistence across reload, demo/import switching, banner honesty, demo stats |
| `tests/ui.test.mjs` | j/k/arrows, `u`, `?`, `/`, layered Esc, `Type UNDO`, confetti+chime, clipboard fallbacks, copy/patch exports, row semantics, focus trap, tour cancel/replay |
| `tests/features.test.mjs` | action-aware commands, strategy cards + keys, recovery range, undo-push sequence, incident report, oversized paste, empty-state coherence |
| `tests/integrity.test.mjs` | no CDN/network/`alert`, inlined CSS freshness, CSP present, package.json hygiene, XSS regression, size budget |

CI runs the suite on Node 20 + 22 and fails if `index.html` is stale relative to
`src/tailwind.css` (`.github/workflows/ci.yml`).

---

## 🔒 Security model

- **No network surface**: CSP `default-src 'none'` with `connect-src 'none'`, no remote
  script/style/img/font, and no `fetch`/XHR in the code. Your pasted reflog never leaves
  the tab.
- **No shell**: the app cannot read a repository or run git. It parses text and emits a reset
  command for *you* to review. A static file has no access to your repo, and a browser tab should never be able to run
  `reset --hard`.
- **No injection**: every user-derived string is escaped before it reaches `innerHTML`;
  a regression test pastes `<img onerror>` / `<script>` payloads and asserts nothing executes.
- **Persisted locally**: `localStorage` only, capped at 200k chars, and the app tells you
  when the browser blocks it (common on `file://`).

---

## 🖥️ Desktop (Tauri v2, optional)

`src-tauri/` contains a thin native shell that hosts the same built `dist/index.html`.
It exposes **no IPC commands** — the desktop build has the same read-only, no-git
guarantees as the web build.

```bash
npm install
npm run build
npx @tauri-apps/cli@^2 dev     # needs Rust + platform toolchain
npm run tauri:build            # bundles via src-tauri/tauri.conf.json
```

The shell is scaffolded and configured but **not part of CI by default** (release
workflow gates it behind `vars.TAURI_ENABLED`). Icons are generated with
`npx @tauri-apps/cli@^2 icon <png>`; see `src-tauri/README.md`.

---

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
- **Size**: ≈180 KB total — ≈30 KB of that is Tailwind CSS compiled ahead of time and inlined.
- **Network**: **none**. No `<script src>`, no `@import`, no fonts, no analytics. Works fully offline from `file://`.
- **Runtime dependencies**: zero.
- **Theme**: Tokyo Night (`#1a1b26`, `#16161e`, `#7dcfff`, `#bb9af7`, `#9ece6a`, `#f7768e`) via `--tn-*` variables.
- **Import limits**: up to ~2 MB of pasted text is parsed in-tab (larger pastes are refused with an explanation);
  only the first 200 KB is persisted for reloads.
- **Storage**: `localStorage` keys `git-undo-seen`, `git-undo-tour-seen`, `git-undo-imported-reflog`, `git-undo-import-mode` (capped at 200k chars, clearable with the reset button).
- **Accessibility**: text selectable by default (only app chrome opts out), one `h1`, `role="listbox"/"option"` timeline that is keyboard reachable (Tab + Enter), `role="dialog"` + `aria-modal` + `aria-describedby` + focus trap, `aria-live` toasts and counts, `aria-label` on every icon-only button, visible focus rings, `prefers-reduced-motion` respected.
- **Git safety**: read-only visualisation and command generation. It never invokes a subprocess.

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
- [x] Tauri v2 shell scaffolded (shell only, no IPC).
- [ ] Read `.git/logs/HEAD` directly — needs a local helper or file picker API, and a
      security review, because it turns "text in" into "filesystem access".
- [x] Multi-entry selection ("undo everything since HEAD@{4}").
- [x] Copy-as-markdown incident report for PRs / team standups.
- [x] Action-aware command generation (stash / checkout / branch entries).
- [x] Undo-push sequence with `--force-with-lease`.

---

## 📄 License

MIT © 2026 git-undo-gui contributors — see [LICENSE](LICENSE).
