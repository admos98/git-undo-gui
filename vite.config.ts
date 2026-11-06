import { defineConfig } from "vite";
import path from "node:path";

/**
 * The app is ONE static file: index.html with inlined CSS and JS.
 * There is no React, no runtime Tailwind plugin, no framework entry point —
 * Vite is only here to serve/build that single document for convenience.
 *
 * Styling is compiled ahead of time by `npm run css:build`
 * (src/tailwind.css -> inlined into index.html), so the output stays offline.
 */
export default defineConfig({
  root: __dirname,
  appType: "mpa",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: { index: path.resolve(__dirname, "index.html") },
    },
  },
  server: {
    port: 3000,
    host: "0.0.0.0",
    // The sandbox preview proxies requests under a *.e2b.app host; allow it.
    allowedHosts: true,
    hmr: process.env.DISABLE_HMR !== "true",
    watch: process.env.DISABLE_HMR === "true" ? null : {},
  },
  preview: {
    port: 4173,
    host: "0.0.0.0",
    allowedHosts: true,
  },
});
