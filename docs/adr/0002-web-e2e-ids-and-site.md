# 2. Web E2E test IDs, site layout and build tooling

Status: accepted (#12)

## Context
`cargo xtask ci linux --out DIR` must run real Playwright tests, write `DIR/e2e.json` with
journey test IDs (CONTRACT §2) and may write `DIR/site/`, which CI deploys to GitHub Pages
on every merge. xtask has no dependencies (a JSON crate would have to pass dependency
admission, and `serde_json` uses `unsafe`). The npm tree is not covered by `cargo deny`, so
CONSTITUTION 10 has to be enforced by us.

## Decision
1. **Test IDs are Playwright tags.** A test declares the journey test IDs it covers as
   `{ tag: "@e2e.web.<id>" }`; several tests may share an ID and one test may carry several.
   A custom reporter (`apps/web/e2e/journey-reporter.ts`) maps each ID to the worst outcome
   of its tests: `fail` (failed, timed out, interrupted, flaky, or an expected failure via
   `test.fail`) > `skip` (skipped or never run) > `pass`. It writes `<id>\t<status>` lines;
   xtask validates every line (`e2e.web.[a-z0-9_]+`, known status, no duplicates, ≤ 1 MiB)
   and renders `e2e.json` itself. A damaged file fails the run.
2. **Harness IDs** that no journey lists (today `e2e.web.app_start_empty`) are allowed. The
   judge grades only journey IDs, so they don't change any grade; they show that the harness
   ran and are kept as smoke checks. A journey that later covers the same screen lists its own
   ID; harness IDs are never renamed to fake a journey pass.
3. **Site layout.** `DIR/site/` is the Vite build (`index.html` + `assets/`), built with
   `base: "./"` because Pages serves the repository under a sub-path. CI adds `site/judge/`;
   the metrics dashboard (#15) will live under `site/metrics/`. Nothing else writes there.
4. **CSP in a meta tag.** Pages can't send headers, so the build prepends the policy from
   `apps/web/csp.ts` as the first `<head>` element. `frame-ancestors` can't be expressed
   this way; accepted until the app can change files. Emotion's empty `<style>` elements are
   allowed by the hash of the empty string instead of `'unsafe-inline'` or a static nonce
   (a nonce in a static file is public, so it would allow any injected style).
5. **Tooling inside the license allowlist.** CONSTITUTION 10 lists licenses without an
   exemption for build-only tools, so the whole lockfile is checked. That rules out Vite 8
   (requires `lightningcss`, MPL-2.0), ESLint (`minimatch`, BlueOak-1.0.0) and Babel-based
   plugins (`caniuse-lite`, CC-BY-4.0). We use Vite 7 with esbuild's JSX transform (no React
   plugin, so no fast refresh in development) and Biome for linting. `axe-core` (MPL-2.0)
   is excluded too, though accessibility.md §5 asks for it; ops/proposals/test-only-licenses.md
   asks the owner to decide.
6. Tests run against `vite preview` of the production build, never the dev server (whose HMR
   scripts would need a weaker policy).

## Consequences
- Adding an npm dependency means checking its whole tree with `npm run licenses` first.
- Each feature's Playwright tests tag the journey IDs from its spec; the judge sees them the
  next run.
- If the owner allows MPL-2.0 test-only tools, the accessibility audit (axe-core) and Vite 8
  become possible; this ADR is then amended.
