//! The page shown in the native window.
//!
//! Tauri can only load an `http(s)` URL into a webview, so the desktop build
//! serves the *same* built `index.html` from memory through a custom protocol.
//! There is no server, no port, and nothing to firewall: the bytes come from
//! the binary.
//!
//! `assets/index.html` is a copy of `dist/index.html`, kept in sync by
//! `scripts/embed-web.mjs` (run automatically by `npm run build` and by CI).
//! The point of the copy is that `include_str!` needs the file to exist *before*
//! the frontend is built, which cannot be guaranteed on a clean checkout.

/// The frontend, embedded at compile time.
pub const INDEX_HTML: &str = include_str!("../assets/index.html");

/// The IPC bridge. Without it the page cannot reach a single command: the app
/// would look alive and do nothing, which is worse than not starting.
const BRIDGE_JS: &str = r#"
Object.defineProperty(window, '__TAURI__', {
  value: Object.freeze(Object.assign({}, window.__TAURI__ || {}, {
    core: Object.freeze({
      invoke: (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args)
    }),
    dialog: Object.freeze({
      open: (options) => window.__TAURI_INTERNALS__.invoke('plugin:dialog|open', { options })
    })
  })),
  writable: false,
  configurable: false
});
"#;

/// `index.html` with the bridge injected before any app code runs.
pub fn page() -> String {
    let bridge = format!("<script>{BRIDGE_JS}</script>");
    match INDEX_HTML.find("</head>") {
        Some(at) => {
            let mut out = String::with_capacity(INDEX_HTML.len() + bridge.len());
            out.push_str(&INDEX_HTML[..at]);
            out.push_str(&bridge);
            out.push_str(&INDEX_HTML[at..]);
            out
        }
        // No <head>: still safe to prepend, the script only needs to exist.
        None => format!("{bridge}{INDEX_HTML}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bridge_lands_before_the_app_code() {
        let page = page();
        let bridge = page.find("__TAURI_INTERNALS__").expect("bridge injected");
        // Anchor on the app's own code, not on "<script>" — the bridge is itself
        // a script tag, which makes the naive search tautological.
        let app = page.find("function isDesktop").expect("app code present");
        assert!(bridge < app, "bridge must be defined before the app runs");
        // Count the injected block itself; the token appears twice inside it.
        assert_eq!(
            page.matches("Object.defineProperty(window, '__TAURI__'").count(),
            1,
            "the bridge must be injected exactly once"
        );
    }

    #[test]
    fn the_page_is_the_shipped_file() {
        // Guards against an empty or half-copied asset.
        assert!(INDEX_HTML.contains("UNDO TO THIS SNAPSHOT"));
        assert!(INDEX_HTML.len() > 100_000, "unexpectedly small: {}", INDEX_HTML.len());
    }
}
