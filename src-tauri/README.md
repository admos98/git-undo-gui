# Desktop shell (Tauri v2)

This directory holds an **optional** native wrapper around the same `index.html` that ships
to the browser. The browser build is the primary artifact and is unchanged by anything here:
0 runtime dependencies, 0 network calls, no repository access.

## Status: compiled, tested against real repositories, never yet clicked

On every push CI runs `cargo clippy --all-targets -- -D warnings`, builds the release binary
(4 MiB, enforced under a 20 MiB budget), runs the guard crate's 19 unit tests, and runs
`src/e2e.rs` — which creates throwaway repositories with real `git` and checks that a refusal
leaves the tree untouched and that the backup branch exists before the reset it protects.

What has **not** happened: a human opening the window. There was no desktop session anywhere this
was authored, so the webview and the widget flow are unverified. Treat it as a beta.

The JavaScript side of the contract is tested too (`tests/executor.test.mjs`): command names,
argument names, strategy strings (`restore` → `files`), the `RESET` acknowledgement, and the
refusal path.

First build, on a machine with Rust + your platform's webview libraries:

```bash
rustc --version                 # 1.77 or newer
npm install && npm run build    # dist/index.html
cargo test --no-default-features --offline --manifest-path src-tauri/logic/Cargo.toml
npm run tauri:build
```

## Layout

| Path | Purpose |
| :--- | :--- |
| `logic/` | Pure, dependency-free crate: hash validation, porcelain parsing, the reset guard. Testable without Tauri, a display, or a repository. |
| `src/git.rs` | Thin wrapper around the `git` binary. One short-lived process per call. |
| `src/service.rs` | The four operations, in order: gather → guard → backup → act. |
| `src/lib.rs` | Exactly four `#[tauri::command]` functions. If you add a fifth, `tests/executor.test.mjs` fails on purpose. |
| `capabilities/default.json` | Grants `core:default` + `dialog:allow-open`. Nothing network-facing. |
| `src/embed.rs` | Serves the built page from inside the binary over a custom protocol, and injects the `__TAURI__` bridge the UI expects. |
| `assets/index.html` | **Generated, gitignored.** A copy of `dist/index.html` that `include_str!` can see before the frontend is rebuilt. `npm run build` writes it; CI regenerates it before compiling. It is a build artifact, not a source file — edit `index.html` at the repo root. |
| `tauri.conf.json` | Window loads `http://app.localhost/` (served locally, no port); CSP `default-src 'none'; connect-src 'none'`. |

## What it will not do

- No daemon, no local HTTP server, no background thread, no telemetry, no `reqwest`/`tokio`.
- No `--force`, ever. Push recovery uses `--force-with-lease`.
- No hard reset without (a) a typed `RESET`, (b) a clean tracked working tree, and (c) a
  `backup-before-recovery` branch written first.
- No stale backup offered as an undo: if the branch predates this reset, the reported undo is the
  pre-reset commit, and the branch is named with its date as something to inspect.

## Bundling notes

Icons are not committed (platform binaries); generate them with
`npx @tauri-apps/cli@^2 icon path/to/icon.png`. A full bundle needs the matching host OS —
cross-compiling installers is not supported by Tauri. Release builds use
`opt-level="s"`, LTO, `panic="abort"` and `strip`, which is what keeps the binary in the
single-digit-megabyte range instead of Electron's ~150 MB.
