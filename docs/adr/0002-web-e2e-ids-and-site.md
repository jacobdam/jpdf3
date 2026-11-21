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
5. **Tooling and the license rule** (amended for #50; the original text is in git history).
   The owner amended CONSTITUTION 10 (PR #95): test and build tools whose code never reaches
   a shipped artifact may also use MPL-2.0, BlueOak-1.0.0, CC-BY-4.0 (data only), CC0-1.0,
   0BSD and NCSA. `npm run licenses` (specs/tooling/licenses.md) enforces the rule in two
   trees:
   - The **production tree** is everything reachable from `package.json` `dependencies`. It
     is computed from installed manifests, not from the lockfile's `dev` flags, and must use
     the strict list.
   - The **full tree** may also use the tool-only licenses. CC-BY-4.0 passes only for named
     data packages (`caniuse-lite`).
   - Shipped sources (`src/`, `index.html`) may import only `dependencies`, so no tool-only
     package can be bundled.

   The bundled-code gap recorded here before is now covered by the rule. Playwright bundles
   `caniuse-lite` (CC-BY-4.0, data) and `minimatch` (BlueOak-1.0.0); Vite bundles
   `string-hash` (CC0-1.0); Rollup bundles `tslib` (0BSD). All of these are test/build tools
   whose files never reach `dist/`. The check still reads package metadata only, not code
   bundled inside a package's files. So a **production** dependency that bundles third-party
   code, or a Vite plugin that injects third-party runtime code into the bundle, must be
   checked by hand when it is added. Vite 8, ESLint, Babel-based plugins and axe-core are no
   longer ruled out by license; adopting any of them is a separate issue (#50 item 3).
6. Tests run against `vite preview` of the production build, never the dev server (whose HMR
   scripts would need a weaker policy).

## Consequences
- Adding an npm dependency means checking its whole tree with `npm run licenses` first.
- Each feature's Playwright tests tag the journey IDs from its spec; the judge sees them the
  next run.
- A new production dependency must pass the strict list. A new test/build tool may also use
  the tool-only licenses. A CC-BY-4.0 tool package needs a check that it ships data only,
  and then a `CC_BY_DATA` entry.
