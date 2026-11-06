# Desktop shell (Tauri v2)

This directory holds an **optional** native wrapper around the same `index.html`
that ships to the browser. The browser build is the primary artifact; the shell
exists because people asked for a dock icon, not because the app needs native code.

## What it does

- Hosts the built `dist/index.html` in a native window (Windows/macOS/Linux).
- Applies the same `default-src 'none'` CSP as the web build.
- Exposes **no** IPC commands: the desktop build still cannot read your
  repository or execute git. It parses what you paste, exactly like the web build.

## What it does not do (yet)

- No repository picker, no `git` subprocess, no file watcher. Those would need a
  deliberate security review — see the roadmap in the top-level README.

## Building locally

```bash
npm install
npm run build              # produces dist/index.html
npx @tauri-apps/cli@^2 build   # needs Rust + platform toolchain
npx @tauri-apps/cli@^2 dev     # dev window against the built dist
```

Icons are not committed (they are platform binaries); generate them with
`npx @tauri-apps/cli@^2 icon path/to/icon.png` before bundling.
