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
   `prefers-color-scheme`), system font stack, 14 px base, sentence-case buttons, radius 4;
   the two-tone focus ring (2 px `#0b0b0b` inside 2 px `#ffffff`) on every `:focus-visible`
   element; under `prefers-reduced-motion: reduce` (read at startup) no transitions and no
   ripples.
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
5. **`cargo xtask ci linux --out DIR`** (CONTRACT §2), run from the repository root, runs four
   parts in this order, each even if an earlier one failed (so one run shows every problem),
   and exits 0 only if all four passed: the web part below, `check-specs`, a short fuzz run
   (specs/tooling/fuzz.md) and the offline Arlington check (specs/tooling/arlington.md). The
   web part runs these steps in order and stops at the first failing one:
   1. checks that `apps/web/package.json`, `node` and `npm` exist (a clear message otherwise);
   2. `npm ci --ignore-scripts` (lockfile only; no dependency script runs). `.npmrc` sets
      `ignore-scripts` too, but an inherited `npm_config_ignore_scripts=false` overrides the
      file, and only the command-line flag overrides that (#83);
   3. `npm run licenses` (specs/tooling/licenses.md): the production tree (reached from
      `dependencies`) uses the strict CONSTITUTION 10 list, and test/build tools may also use
      the tool-only licenses. Both the lockfile and each installed `package.json` are checked,
      and shipped sources may import only `dependencies` (ADR 0002 item 5);
   4. `npm run typecheck` (`tsc --noEmit`, strict; `src/` without Node types, tests and
      config with them), `npm run lint` (`biome ci --error-on-warnings`, including
      `noDangerouslySetInnerHtml` and `noGlobalEval`);
   5. `npm run build`, then copies `apps/web/dist/` to `DIR/site/` (replacing it);
   6. installs Playwright's Chromium (with system libraries on Linux CI); every `npm exec`
      uses `--no`, so nothing outside the lockfile is ever downloaded and run;
   7. runs Playwright against `vite preview` of the build; the journey reporter writes
      `DIR/web-e2e.tsv`. If the run as a whole didn't pass (Playwright's final status is
      not `passed`: a spec file that failed to load, a throwing hook or global setup, a
      timed-out or interrupted run) or Playwright reported any error (`onError`), the
      reporter writes every `pass` as `fail`; an interrupted test is `fail` (ADR 0002), not
      the `skip` Playwright's own outcome gives it (#76). A tag that starts with `@e2e` (any
      case) but isn't a valid `@e2e.web.<id>` (`@e2e.web.Bad`, `@e2e.web.a.b`,
      `@e2e.macos.a`) is reported on stderr and has the same effect (#76 item 4).
   Before step 1 it deletes `DIR/e2e.json`, `DIR/web-e2e.tsv` and `DIR/site/` from any
   earlier run; if one can't be removed, no step runs. Once `DIR` exists it always writes
   `DIR/e2e.json` from the reporter's file (empty results if no tests ran or the stale
   output couldn't be removed). Every reported ID must be a journey's test ID (a `- test:`
   line in `specs/journeys/*.md`) or a harness ID (ADR 0002 item 2: `e2e.web.app_start_empty`,
   listed in `xtask/src/web.rs` `HARNESS_IDS`). If any step, including the Playwright run,
   failed, an ID is unknown or the journeys can't be read, every `pass` is written as
   `fail`. The web part passes only if every step passed, every ID is known, at least one
   test ID was reported and every reported ID is `pass`. Every error is printed: the later
   ones first, then the first error, the summary, as the last `xtask: web:` line.

## Persistence
None. The skeleton stores nothing (no localStorage, IndexedDB or cookies).

## Undo
Not applicable (no document, no editing).

## Errors
- Missing `#root` element: `main.tsx` throws (a build defect, caught by the E2E test).
- xtask: a missing tool or failing step prints `xtask: web: <reason>` and exits 1; a damaged
  results file (bad line, unknown status, invalid or duplicate ID, larger than 1 MiB), an ID
  that no journey lists and that isn't a harness ID, journeys that can't be read, or stale
  output that can't be removed is an error, never silently read as a pass.

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
  are checked on every CI run: the strict list for the production tree, and the tool-only
  licenses too for test/build tools (CONSTITUTION 10, ADR 0002 item 5).
- `<meta name="referrer" content="no-referrer">`.
- Not possible with a meta CSP on GitHub Pages: `frame-ancestors` (clickjacking). Nothing in
  the skeleton acts on a click without a document; revisit when the app can save files.

## Accessibility
`lang="en"`, one `main` landmark, `h1`/`h2` headings, the disabled button stays in the tab
order with its reason as accessible description (tested). Contrast follows the wireframe
tokens. An automated WCAG audit (accessibility.md §5 names axe-core) is not in this harness yet.
axe-core (MPL-2.0) is now allowed as a test tool (#50); adding it is its own issue.

## Platforms
Web only. The macOS skeleton is #13. `cargo xtask ci linux` also runs on macOS for local
development (without `--with-deps`).

## Performance
The start screen bundle is about 400 kB (130 kB gzip), dominated by MUI and React. No budget
is enforced yet; the first-page budget (J-RD-01) applies from M4.

## Tests
Playwright (`apps/web/e2e/`), all tagged `@e2e.web.app_start_empty` except the reporter unit
tests:
- the start screen shows every W-02 element in order; `Open file` is `aria-disabled`,
  focusable with Tab, shows the two-tone focus ring (kept on hover), is described by its
  reason, keeps its resting look on hover (no darkening, no elevation; wireframes-web.md §0
  rule 5), and Enter or a click does nothing (same URL, no dialog, no file chooser); the
  theme's primary and secondary text colors are applied (Emotion works under the CSP);
- every browser test (each test that uses `page` from the shared fixture
  `apps/web/e2e/fixtures.ts`, which every test in `start-screen.spec.ts` imports) fails
  after its body on any console error or warning, uncaught error, CSP violation (also on a
  page the test navigated away from) or request outside the app's origin. The CSP test
  allows only what it provokes: the CSP / Trusted Types console errors and the violations
  `script-src-elem inline`, `style-src-elem inline` and
  `require-trusted-types-for trusted-types-sink`. Not checked: that a new spec file using
  `page` imports `test` from the fixture rather than from `@playwright/test` (#205);
- dark appearance uses the dark palette, and an unfocused hover changes neither background
  nor shadow;
- reduced motion leaves no transition on the button;
- the CSP meta tag is the first `<head>` element, has exactly `csp.ts`'s policy, is the only
  one, and no script is inline;
- injected inline script doesn't run, injected inline style doesn't apply, and
  `insertAdjacentHTML` throws (Trusted Types);
- `dragover`/`drop` with a PDF file are cancelled and the URL doesn't change;
- reporter unit tests: outcome mapping (expected failures, flaky passes and interrupted
  results are failures; skipped is skip), worst status wins per ID, foreign and malformed
  tags ignored by the ID mapping, malformed tags found (`@e2e.web.Bad`, `@e2e.web.c.d`,
  `@E2E.web.a`, `@e2e.macos.a`, `@e2e.web.`, `@e2e-web-a`, `@e2e`, `@e2ex`; not `@slow` or
  `@xe2e.web.a`), a failed run downgrades every pass, and the reporter class itself writes
  `fail` for a run status of `failed`, `timedout` or `interrupted`, after `onError` and when
  any test carries a malformed tag (no line at all when its only tag is malformed);
- reporter runs (untagged): `journey-reporter.spec.ts` runs a child Playwright on the probe
  project `apps/web/e2e/harness-probe/` (files `*.probe.ts`, which the app's config never
  matches) with the journey reporter: the passing probe alone reports
  `e2e.web.harness_probe` as `pass` and exits 0 (control); with a file whose `beforeAll`
  throws it exits non-zero and reports `fail`; with an unloadable file it exits non-zero
  and reports no ID (Playwright 1.65 runs nothing then; a tripwire for that behavior, which
  any reporter passes today). The fixture probes (`fixture-*.probe.ts`, pages served by
  routing from the probe config's `origin`): a clean page exits 0 and reports `pass`
  (control); a CSP violation on the last page, a CSP violation on a page the test left
  (both with every console message allowed, so the violation itself is what fails), a
  request to another host, a console warning, and an uncaught error in the page (with every
  console message allowed) each exit non-zero and report `fail`.
Rust unit tests in `xtask/src/web.rs`: ID validation, results parsing (good and damaged
lines), a failed run downgrades every pass (and damaged results are an error and an empty set), reading the results file (missing, non-UTF-8, over 1 MiB), `e2e.json` rendering,
tree copy; known IDs (journey and harness IDs stand; one unknown ID, a near miss of a harness
ID or unreadable journeys is an error that downgrades every pass); stale-output removal (files
and trees, the first failure reported, later paths still removed) and `ci` with an
undeletable `DIR/site/` (skipped as root): it runs nothing, ignores the stale reporter file and
still writes empty results over a stale `e2e.json` that claimed a pass. `xtask/src/specs.rs`:
journey test IDs come from `specs/journeys/*.md` only (not subdirectories or other
extensions), an unreadable file, a directory named `*.md` or a missing directory is an error,
and the repository's journeys list no harness ID. `xtask/tests/check_specs.rs`: `ci linux` with stub `node`/`npm` whose Playwright step
writes a `pass` line writes it as `pass` when that step exits 0 and as `fail` when it exits 1,
and when it also reports an ID no journey lists, both are written as `fail` and the run names it
(also when the step failed too: the unknown ID is printed first, then the failed step as the
summary, the last `xtask: web:` line);
`ci linux` outside a
repository root writes empty results, reports the missing web app and still runs check-specs;
`ci linux` with stub `node`/`npm` on PATH and `npm_config_ignore_scripts=false` must call the
install as `ci --ignore-scripts`.
