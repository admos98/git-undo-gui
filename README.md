# git-undo-gui — Visual Git Undo Button & Time Machine ⏪

[![GitHub Stars](https://img.shields.io/badge/stars-2.4k-yellow?style=for-the-badge&logo=github)](https://github.com)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
[![Zero Dependencies](https://img.shields.io/badge/dependencies-0-9ece6a?style=for-the-badge)](index.html)
[![Bundle Size](https://img.shields.io/badge/bundle%20size-%3C50KB-7dcfff?style=for-the-badge)](index.html)
[![Tauri Ready](https://img.shields.io/badge/Tauri-Desktop%20Ready-bb9af7?style=for-the-badge&logo=tauri)](https://tauri.app)
[![Offline First](https://img.shields.io/badge/offline-100%25-orange?style=for-the-badge)](#)

> **Turn `git reflog` into a pretty, safe "Undo Last Push" button with side-by-side diff preview.**  
> Crafted in Tokyo Night dark mode. No accidental disasters. Zero real git commands executed without your explicit review.

---

## 📽️ 15-Second Demo Preview

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

> **How to Record Your Own 15-sec Demo GIF:**
> 1. Double click `index.html` in your browser (or run via Tauri).
> 2. Press `t` on your keyboard (or click the top **"15s Demo Tour"** button).
> 3. The built-in automated director will choreograph a 15-second sequence: highlights the disaster `reset --hard`, inspects the restored word-level diff, pulses the Safe Undo button, auto-types `UNDO` into the safety modal, and explodes with celebratory canvas confetti!
> 4. Record using [Peek](https://github.com/phw/peek), [vhs](https://github.com/charmbracelet/vhs), or [ScreenToGif](https://www.screentogif.com/).

---

## 💡 Why Not `git-time-machine` TUI?

Command-line TUIs are great until you accidentally press <kbd>Enter</kbd> on a dangerous command at 2:00 AM. Here is how **git-undo-gui** compares:

| Feature | `git-undo-gui` 🚀 | Standard `git reflog` 📟 | `git-time-machine` TUI ⏳ |
| :--- | :---: | :---: | :---: |
| **Word-Level Diff Highlighting** | ✅ Yes (Diffshot style) | ❌ Raw terminal patch | ⚠️ Line-level only |
| **Accidental Enter Protection** | ✅ **"Type UNDO"** Modal | ❌ Instant destructive reset | ❌ None |
| **Non-Destructive Rescue Branch** | ✅ One-click branch generator | ❌ Manual typing | ❌ Not available |
| **Installation Overhead** | ✅ **0 MB** (Double click `index.html`) | ⚙️ Git CLI only | 📦 Node/Rust/Python toolchain |
| **Visual Disaster Timeline** | ✅ Badged: `reset`, `push -f`, `deleted` | ❌ Monochromatic raw SHA wall | ⚠️ Basic log list |
| **Import Terminal Reflog** | ✅ One-click regex parser | ❌ N/A | ❌ Direct repository only |
| **Download `.patch` File** | ✅ Yes (Instant browser blob) | ❌ Requires piping git commands | ❌ No |
| **Aesthetics** | 🌌 **Tokyo Night** GUI + Chimes + Confetti | ⬛ Terminal monochrome | 🔲 Curses terminal box |

---

## ✨ Features (MVP)

### 1. 🕒 Reflog Timeline Inspector
- Real-time filtered view of `git reflog` events (`HEAD@{0}`, `HEAD@{1}`, etc.).
- Pre-categorized emergency tags:
  - 🔴 **`reset --hard`** (Accidental loss of commits or working tree)
  - 🟠 **`force push`** (Overwritten upstream commits)
  - 🟣 **`deleted branch`** (Dropped feature branches before PR merge)
  - 🟢 **`amend` / `merge` / `cherry-pick`**
- Instant search filter by commit hash, branch name, or message (press `/` to search).

### 2. 🔍 Word-Level Diff Preview (Diffshot style)
- Precise word-level and token-level diff highlighting.
- Syntax styling matching Tokyo Night palette:
  - Ruby red background with bright highlights for deletions.
  - Neon emerald green background with crisp highlights for additions.
- Side-by-side (Split) or Unified diff toggle.
- File tabs for multi-file commit changes.

### 3. 🛡️ SAFE Undo Engine & Safety Guard
- Never runs destructive git commands silently.
- Preview exact generated commands with one-click copy:
  - **Soft Revert**: `git reset --soft <hash>` (keeps all files safely staged)
  - **Rescue Branch**: `git branch rescue-<hash> <hash>` (zero risk)
  - **Hard Reset**: `git reset --hard <hash>` (full restoration)
- **Safety Interlock**: Requires typing **`UNDO`** to confirm any rollback.
- Confetti explosion & Web Audio synthesized chime when a disaster is averted!

### 4. 📋 Import Real Terminal Reflog
- Run `git reflog` in your local terminal.
- Click **"Import Reflog"** in the top bar and paste your output.
- Robust regex parser (`hash HEAD@{n}: message`) instantly parses your actual git history and generates interactive diff previews.

### 5. 💾 Bonus Dev Utilities
- **Download `.patch` file**: Export an RFC-compliant `.patch` file with one click to apply via `git apply`.
- **Terminal Dry-Run Steps**: Step-by-step terminal checklist to verify your repository state safely.

---

## 🚀 Installation & Quickstart

### Option A: Zero Install (Recommended)
No `node_modules`, no Docker, no build step.

```bash
# Clone and open directly
git clone https://github.com/your-username/git-undo-gui.git
cd git-undo-gui
open index.html # On macOS, or xdg-open index.html on Linux / start index.html on Windows
```

Or just download [`index.html`](index.html) (<50KB) and double-click it in any web browser!

---

### Option B: Tauri Desktop App (Native macOS, Linux, Windows)

Run as an ultra-lightweight, native desktop menu bar app with Tauri:

```bash
# 1. Install dependencies
npm install

# 2. Run Tauri in development mode
npm run tauri dev

# 3. Build optimized native desktop binary (<5MB)
npm run tauri build
```

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| <kbd>j</kbd> or <kbd>↓</kbd> | Move to next reflog entry |
| <kbd>k</kbd> or <kbd>↑</kbd> | Move to previous reflog entry |
| <kbd>u</kbd> or <kbd>⌘</kbd>+<kbd>Z</kbd> | Open Safe Undo confirmation dialog |
| <kbd>c</kbd> | Copy generated git undo command |
| <kbd>d</kbd> | Download `.patch` file for current snapshot |
| <kbd>t</kbd> | Start automated 15-second demo tour (GIF ready) |
| <kbd>/</kbd> | Focus reflog search bar |
| <kbd>?</kbd> | Toggle keyboard shortcuts modal |
| <kbd>Esc</kbd> | Close any open modal dialog |

---

## 🎯 Technical Specifications

- **File Size**: < 50KB total (single-file architecture).
- **Network**: 100% offline capable (uses embedded canvas confetti and Web Audio API).
- **Theme**: Tokyo Night (`#1a1b26`, `#16161e`, `#7dcfff`, `#bb9af7`, `#9ece6a`, `#f7768e`).
- **Git Safety**: Read-only visualization and command generator. Does not invoke native subprocesses without user authorization.

---

## 🔍 SEO & Keywords

`git undo`, `reflog visual`, `git time machine gui`, `visual git undo button`, `restore lost commits`, `git recovery tool`, `undo git push`, `git reflog viewer`, `tauri git gui`, `tokyo night git diff`.

---

## 📄 License

MIT © 2026 git-undo-gui contributors. Free for personal and commercial developer use.
