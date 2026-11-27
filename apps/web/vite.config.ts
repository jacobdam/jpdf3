import { defineConfig, type Plugin } from "vite";
import { CSP } from "./csp";

// Adds the CSP meta tag to every built page (index.html and dashboard/index.html). The dev
// server injects its own inline scripts (HMR), so it runs without the policy; every E2E test
// runs against the build.
function cspMeta(): Plugin {
  return {
    name: "jpdf-csp-meta",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler: () => [
        {
          tag: "meta",
          attrs: { "http-equiv": "Content-Security-Policy", content: CSP },
          injectTo: "head-prepend",
        },
      ],
    },
  };
}

export default defineConfig({
  // Relative asset URLs: the site is served from a sub-path on GitHub Pages.
  base: "./",
  plugins: [cspMeta()],
  esbuild: { jsx: "automatic" },
  build: {
    target: "es2022",
    // No data: URIs (img-src/font-src are 'self' only).
    assetsInlineLimit: 0,
    sourcemap: false,
    rollupOptions: {
      // The web app and the metrics dashboard (specs/tooling/dashboard.md); both get the CSP.
      input: { app: "index.html", dashboard: "dashboard/index.html" },
      // MUI marks modules "use client" for React Server Components; meaningless in a SPA.
      onwarn(warning, warn) {
        if (warning.code !== "MODULE_LEVEL_DIRECTIVE") warn(warning);
      },
    },
  },
  preview: { host: "127.0.0.1", port: 4173, strictPort: true },
});
