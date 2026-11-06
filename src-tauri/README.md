# Desktop shell (Tauri v2)

This directory holds an **optional** native wrapper around the same `index.html` that ships
to the browser. The browser build is the primary artifact and is unchanged by anything here:
0 runtime dependencies, 0 network calls, no repository access.

## Status: compiled and tested by CI, never yet run by a human

`cargo clippy --all-targets -- -D warnings` and `cargo build --release` run on every push
(`.github/workflows/compile-check.yml`), and the guard crate's 19 unit tests run with clippy
`-D warnings` on top. What has **not** happened: opening the window against a real repository.
The environment this was authored in has neither a Rust toolchain nor a desktop session, so the
first click-through is still owed. Treat it as a beta.

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
| `assets/index.html` | The compiled-in copy of `dist/index.html`, kept in sync by `scripts/embed-web.mjs`. |
| `tauri.conf.json` | Window loads `http://app.localhost/` (served locally, no port); CSP `default-src 'none'; connect-src 'none'`. |

## What it will not do

- No daemon, no local HTTP server, no background thread, no telemetry, no `reqwest`/`tokio`.
- No `--force`, ever. Push recovery uses `--force-with-lease`.
- No hard reset without (a) a typed `RESET`, (b) a clean tracked working tree, and (c) a
  `backup-before-recovery` branch written first.

## Bundling notes

Icons are not committed (platform binaries); generate them with
`npx @tauri-apps/cli@^2 icon path/to/icon.png`. A full bundle needs the matching host OS —
cross-compiling installers is not supported by Tauri. Release builds use
`opt-level="s"`, LTO, `panic="abort"` and `strip`, which is what keeps the binary in the
single-digit-megabyte range instead of Electron's ~150 MB.
