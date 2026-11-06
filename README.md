# git-undo-gui — Visual Git Undo Button & Reflog Time Machine ⏪

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/admos98/git-undo-gui?style=flat&logo=github&label=stars)](https://github.com/admos98/git-undo-gui/stargazers)
[![Bundle size](https://img.shields.io/badge/index.html-142%20KB%20(29%20KB%20CSS%20inlined)-7dcfff)](index.html)
[![Offline](https://img.shields.io/badge/offline-100%25%2C%20no%20CDN-9ece6a)](#technical-specifications)
[![Dependencies](https://img.shields.io/badge/runtime%20deps-0-brightgreen)](index.html)

> **Turn `git reflog` into a safe, visual "undo" workflow with side-by-side diff preview.**
> Tokyo Night dark mode. Single self-contained `index.html`. No git command is ever executed for you — the app generates the command, you review it, you paste it.

### 📌 Demo Status — please read

**This is a visual demo and a safe command generator.** It does **not** run git, does not open a repository, and does not shell out to anything. It parses reflog text that *you* paste in, shows you what each entry means, and writes the exact command you'd need. You stay in control of your terminal.

- Sample data is 6 hand-written entries, clearly labelled **DEMO MODE** in the titlebar.
- Data you import is labelled **IMPORTED** and stays in your browser's `localStorage` only.
- The safe undo flow is labelled **Demo Undo (simulated)** — it copies a command, nothing more.

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
- Generates, never executes:
  - **Soft Undo** — `git reset --soft <hash>` (keeps files staged)
  - **Rescue Branch** — `git branch rescue-<hash> <hash>` (zero risk)
  - **Hard Reset** — `git reset --hard <hash>` (full restoration)
- **Safety interlock**: you must type `UNDO` before the confirm button unlocks.
- Web Audio chime + canvas confetti when you complete the flow.

### 3b. 📋 Copy & export that actually works offline
- **Clipboard**: `navigator.clipboard` doesn't exist on `file://` or in some webviews — so a `textarea` + `execCommand` fallback runs first, and if the browser blocks both, the command is *selected on screen* and the toast says "press ⌘C/Ctrl+C".
- **Copy diff**: copies **every file** in the entry (Shift+click copies only the active tab).
- **Download `.patch`**: all files, per-file `diff --git` headers, `git apply --check` friendly.

### 4. 📋 Import Your Real Reflog
- Run `git reflog` (optionally `--date=iso`), paste it, hit parse.
- Your lines are preserved verbatim: each entry's diff shows `Original reflog line: …`, never invented content.
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
| `npm run tauri` | Placeholder for a future Tauri shell (not wired up yet) |

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
- **Size**: ≈142 KB total — ≈29 KB of that is Tailwind CSS compiled ahead of time and inlined.
- **Network**: **none**. No `<script src>`, no `@import`, no fonts, no analytics. Works fully offline from `file://`.
- **Runtime dependencies**: zero.
- **Theme**: Tokyo Night (`#1a1b26`, `#16161e`, `#7dcfff`, `#bb9af7`, `#9ece6a`, `#f7768e`) via `--tn-*` variables.
- **Storage**: `localStorage` keys `git-undo-seen`, `git-undo-tour-seen`, `git-undo-imported-reflog` (your pasted reflog, capped at 200k chars, clearable with the reset button).
- **Accessibility**: text selectable by default (only app chrome opts out), one `h1`, `role="listbox"/"option"` timeline that is keyboard reachable (Tab + Enter), `role="dialog"` + `aria-modal` + `aria-describedby` + focus trap, `aria-live` toasts and counts, `aria-label` on every icon-only button, visible focus rings, `prefers-reduced-motion` respected.
- **Git safety**: read-only visualisation and command generation. It never invokes a subprocess.

---

## 🆚 Similar projects (and what's different here)

| Project | Shape | Overlap with this repo | Where this repo differs |
| :--- | :--- | :--- | :--- |
| [lazygit](https://github.com/jesseduffield/lazygit) (~84k★) | Go TUI | `z` undo uses the reflog | Terminal-only; this is a zero-install browser view with word-level diff + a "type UNDO" interlock |
| [git-time-machine](https://github.com/dinakars777/git-time-machine) | Rust TUI | reflog timeline, j/k, one-key restore | **Actually executes** `reset --hard`; this one never runs git and never claims to |
| [pronzzz/time-machine](https://github.com/pronzzz/time-machine) | Rust TUI | reflog + live diff previews + safe revert | Same idea in the terminal; this needs no toolchain at all |
| [gitkeen](https://github.com/tgomilar/gitkeen) | Tauri desktop client | full git client with undo | Far broader scope (a real client); this is a single-purpose, read-only triage tool |
| GitButler / GitKraken / Sourcetree | Desktop GUI | undo & reflog views | Require install + repo access; this works on a pasted reflog from a machine you can't install on |
| [mrsaynothing/git-undo](https://github.com/mrsaynothing/git-undo) | web page | "one exact command out" | A Q&A command picker; this is a timeline of *your actual* reflog |

**Honest positioning**: the TUI tools above are strictly more capable — they read the repo and execute commands. This project's only defensible niche is *zero-install, zero-network, read-only triage of a reflog you already have in your clipboard*, with a UI that explains what each entry means before you touch a terminal.

---

## 🗺️ Roadmap (honest)

- [ ] Real Tauri desktop shell (currently only a `tauri` script placeholder).
- [ ] Read `.git/logs/HEAD` via a local helper instead of copy/paste.
- [ ] True `git show` diff import (needs a git-invoking backend, out of scope for a static file).

---

## 📄 License

MIT © 2026 git-undo-gui contributors — see [LICENSE](LICENSE).
