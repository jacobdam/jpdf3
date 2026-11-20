# Web app skeleton and E2E harness

Area: ui-web
ISO 32000-2 clauses: none (application shell; no PDF logic)
Journeys: none yet. The skeleton draws the W-02 start screen that J-RD-01…J-RD-03 begin on;
those journeys belong to M4 and get their tests then. The harness ID is
`e2e.web.app_start_empty` (docs/adr/0002-web-e2e-ids-and-site.md).
Status: implemented

## Behavior
`apps/web` is a static single-page app: Vite 7, React 19, MUI 9 (Emotion), TypeScript strict.
It has no backend and makes no network requests other than loading its own files (PLAN R11).

1. **Start screen (W-02, specs/ui/wireframes-web.md).** A `main` landmark with, in order:
   - the heading "jpdf" (`h1`, typography `h6`);
   - an outlined drop target (2 px dashed control boundary, radius 8) saying
     "Drop a PDF here to open it", with an `Open file` contained button;
   - the heading "Recent files" (`h2`, `subtitle2`) and "Files you open appear here."
     (`body2`, `text.secondary`).
   Until the reader exists (M4), `Open file` is a disabled control per wireframes §0 rule 5:
   `aria-disabled="true"`, still focusable, its reason "Opening files is not available yet"
   in a tooltip and as its accessible description. Activating it does nothing.
   The W-01 "Create" column and the menus come with the features that fill them.
2. **Drops.** `dragover` and `drop` are cancelled on the whole window, so a file dropped on
   the app never makes the browser navigate to it.
3. **Theme.** wireframes-web.md §1 tokens: palette (light and dark, following
   `prefers-color-scheme`), system font stack, 14 px base, sentence-case buttons, radius 4.
4. **CSP.** The build adds one `<meta http-equiv="Content-Security-Policy">` as the first
   element of `<head>` (GitHub Pages can't send headers). The policy is in `apps/web/csp.ts`:
   `default-src 'none'`; `script-src 'self'`; `style-src 'self'` plus the hash of the empty
   string; `img-src`, `font-src`, `connect-src`, `manifest-src` `'self'`; `base-uri`,
   `form-action`, `object-src` `'none'`; Trusted Types required with no policies
   (`require-trusted-types-for 'script'; trusted-types 'none'`). There is no inline script,
   no `unsafe-inline`, no `unsafe-eval`; the wasm engine will add only `'wasm-unsafe-eval'`.
   Emotion in production creates empty `<style>` elements and fills them with CSSOM
   `insertRule`, which CSP doesn't govern; the empty-string hash allows only those empty
   elements. Rationale: docs/adr/0002-web-e2e-ids-and-site.md.
5. **`cargo xtask ci linux --out DIR`** (CONTRACT §2), run from the repository root, in order;
   it stops at the first failing step:
   1. checks that `apps/web/package.json`, `node` and `npm` exist (a clear message otherwise);
   2. `npm ci` (lockfile only; `.npmrc` sets `ignore-scripts`, so no dependency script runs);
   3. `npm run licenses`: every package in `package-lock.json`, shipped or build-only, has a
      CONSTITUTION 10 license (an `A OR B` expression passes if one option does);
   4. `npm run typecheck` (`tsc --noEmit`, strict), `npm run lint` (Biome, warnings fail,
      including `noDangerouslySetInnerHtml` and `noGlobalEval`);
   5. `npm run build`, then copies `apps/web/dist/` to `DIR/site/` (replacing it);
   6. installs Playwright's Chromium (with system libraries on Linux CI);
   7. runs Playwright against `vite preview` of the build; the journey reporter writes
      `DIR/web-e2e.tsv`.
   It then always writes `DIR/e2e.json` from that file (empty results if no tests ran),
   runs `check-specs`, and exits 0 only if every step passed, at least one test ID was
   reported and every reported ID is `pass`.

## Persistence
None. The skeleton stores nothing (no localStorage, IndexedDB or cookies).

## Undo
Not applicable (no document, no editing).

## Errors
- Missing `#root` element: `main.tsx` throws (a build defect, caught by the E2E test).
- xtask: a missing tool or failing step prints `xtask: web: <reason>` and exits 1; a damaged
  results file (bad line, unknown status, invalid or duplicate ID, larger than 1 MiB) is an
  error, never silently read as a pass.

## Limits
No user input is processed. xtask reads at most 1 MiB of reporter output and accepts only IDs
matching `e2e.web.[a-z0-9_]+`, so `e2e.json` needs no escaping.

## Security
- Strict CSP with Trusted Types (Behavior 4); the E2E test proves that inline script and
  inline style are blocked and that HTML-string sinks throw.
- No third-party requests (no CDN fonts or icons); the E2E test fails on any request that
  isn't to the app's own origin.
- Dropped files never reach the browser's own viewer (Behavior 2).
- Dependencies are pinned exactly in `package-lock.json`; install scripts never run; licenses
  are checked on every CI run. Build tooling was chosen to keep the whole tree inside the
  allowlist (ADR 0002).
- `<meta name="referrer" content="no-referrer">`.
- Not possible with a meta CSP on GitHub Pages: `frame-ancestors` (clickjacking). Nothing in
  the skeleton acts on a click without a document; revisit when the app can save files.

## Accessibility
`lang="en"`, one `main` landmark, `h1`/`h2` headings, the disabled button stays in the tab
order with its reason as accessible description (tested). Contrast follows the wireframe
tokens. An automated WCAG audit (accessibility.md §5 names axe-core) is not in this harness:
axe-core is MPL-2.0, outside CONSTITUTION 10; see ops/proposals/test-only-licenses.md.

## Platforms
Web only. The macOS skeleton is #13. `cargo xtask ci linux` also runs on macOS for local
development (without `--with-deps`).

## Performance
The start screen bundle is about 400 kB (130 kB gzip), dominated by MUI and React. No budget
is enforced yet; the first-page budget (J-RD-01) applies from M4.

## Tests
Playwright (`apps/web/e2e/`), all tagged `@e2e.web.app_start_empty` except the reporter unit
tests:
- the start screen shows every W-02 element; `Open file` is `aria-disabled`, focusable with
  Tab, and described by its reason; the theme's primary color is applied (Emotion works under
  the CSP); no console errors or warnings, uncaught errors, CSP violations or foreign requests;
- dark appearance uses the dark palette;
- the CSP meta tag is the first `<head>` element, has exactly `csp.ts`'s policy, is the only
  one, and no script is inline;
- injected inline script doesn't run, injected inline style doesn't apply, and
  `insertAdjacentHTML` throws (Trusted Types);
- `dragover`/`drop` with a PDF file are cancelled and the URL doesn't change;
- reporter unit tests: outcome mapping (expected failures and flaky passes are failures;
  skipped is skip), worst status wins per ID, foreign and malformed tags ignored.
Rust unit tests in `xtask/src/web.rs`: ID validation, results parsing (good and damaged
lines), `e2e.json` rendering, tree copy. `xtask/tests/check_specs.rs`: `ci linux` outside a
repository root writes empty results, reports the missing web app and still runs check-specs.
