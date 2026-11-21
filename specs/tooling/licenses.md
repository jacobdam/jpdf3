# npm license check (production and test/build trees)

Area: tooling (ui-web build)
ISO 32000-2 clauses: none (supply chain; CONSTITUTION 10)
Journeys: none (a CI step of `cargo xtask ci linux`, specs/ui-web/app-skeleton.md step 3)
Status: implemented

## Behavior
`npm run licenses` in `apps/web` first runs the unit tests (`node --test
scripts/licenses.test.mjs`), then `scripts/check-licenses.mjs`. That script reads
`package-lock.json`, `package.json`, every installed `node_modules/<path>/package.json`,
`index.html` and every `.js/.jsx/.mjs/.ts/.tsx/.mts/.css/.html` file under `src/`. The
logic is in `scripts/licenses-lib.mjs`, pure functions over that data. It never runs package
code and has no dependencies.

1. **Production tree.** These are the lockfile paths reachable from the root `package.json`
   `dependencies` and `optionalDependencies`, following each package's `dependencies`,
   `optionalDependencies` and `peerDependencies` with Node's resolution rule: the nearest
   `node_modules/<name>`, walking up from the requiring package. Edges come from the
   installed `package.json` when there is one, else from the lockfile entry (platform
   packages that aren't installed on this machine). The lockfile's `dev` flags are **not**
   used, so an edited flag can't move a package out of the production tree. Installed
   optional peers count as production, because a package may load them at run time. This is
   stricter than npm's view.
2. **Licenses.** Each lockfile entry's `license` and, if the package is installed, its real
   `package.json` `license` must pass:
   - production tree: the strict list (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Zlib,
     ISC, Unicode-3.0);
   - every other package (test and build tools): the strict list plus MPL-2.0,
     BlueOak-1.0.0, CC0-1.0, 0BSD and NCSA, plus CC-BY-4.0 **only** for package names in
     `CC_BY_DATA` (now `caniuse-lite`). CONSTITUTION 10 allows CC-BY-4.0 for data only, and
     metadata can't tell data from code, so each name is a reviewed decision.
   - An expression is a single SPDX id or `A OR B …`, which passes if any option passes for
     that tree. `AND`, `WITH`, parentheses, non-strings and a missing license fail.
     Matching is exact and case-sensitive (`mit` fails).
   - OFL-1.1 (fonts only) is not accepted for npm packages; no font package is used.
3. **Installed.** A lockfile entry that isn't optional, devOptional, peer or extraneous
   must be installed (`run npm ci first`).
4. **Shipped imports.** Every bare import specifier in the shipped sources (`import … from
   "x"`, `export … from "x"`, `import "x"`, `import("x")`) must name a package in the root
   `dependencies`. Scoped names use two segments. Relative, absolute and URL/scheme
   specifiers are skipped. So no package that is only in the test/build tree can be bundled.

## Persistence
Not applicable. Output goes to stderr: `licenses: ok (N packages, P production, I installed,
S source files)`, or one line per problem with the path, the license and which tree applied,
and then exit 1.

## Undo
Not applicable.

## Errors
Every problem is a line; the script exits 1 if there is any. Hostile lockfile shapes (no
`packages`, a `null` entry, a `null` installed manifest) become problems, not exceptions.
An unreadable or non-JSON file throws, which also exits non-zero and fails the CI step.

## Limits
The inputs are our own committed lockfile and the tarballs `npm ci` verified by integrity
hash, so there are no size limits. The tree walk visits each lockfile path once (a `seen`
set), so cycles terminate (tested). Resolution walks up at most the depth of the path.

## Security
- **What broken implementation still passes?**
  - A checker that trusts the lockfile `dev` flag fails the test "a lockfile that marks a
    production package dev doesn't move it to the tool tree".
  - One that reads only the lockfile fails the installed-manifest tests. One that builds
    edges only from the lockfile fails "edges come from the installed manifest".
  - One that ignores peers or optional dependencies fails the peer/optional test.
  - One that resolves by name only (not nearest `node_modules`) fails both nested tests.
  - One that allows CC-BY-4.0 by license alone fails the CC-BY tests (including a scoped
    `@x/caniuse-lite` look-alike).
  - One that applies the tool list everywhere fails "an MPL-2.0 production package fails";
    one that applies the strict list everywhere fails the tool-tree tests.
  - Each license in both lists has its own pass/fail assertion.
- **Arithmetic:** none.
- **What these tests can't see:**
  - Third-party code that a package bundles inside its own files (ADR 0002 item 5). Under
    the amended rule this is acceptable for test/build tools only.
  - Code injected into the bundle by build tools rather than imported: Vite's own
    module-preload helper and esbuild's helpers (both MIT, in the strict list). A future
    Vite plugin that injects third-party runtime code must be checked by hand when it is
    added. ADR 0002 item 5 says so.
  - `vite.config.ts`, `csp.ts` and `e2e/` aren't scanned: they run at build or test time
    and aren't bundled.
  - The import scan is a regular expression. Over-matching (an import-like string in a
    comment) only adds a false alarm. Under-matching would need syntax Biome and TypeScript
    reject in `src/` (`require`, computed `import()`).

## Accessibility
Not applicable.

## Platforms
Node ≥ 22.12 (the `engines` field); uses `node:test`, `fs.readdirSync({ recursive })` and
`Dirent.parentPath`. Same on Linux CI and macOS.

## Performance
About 0.1 s for 186 packages plus about 0.1 s of tests.

## Tests
`apps/web/scripts/licenses.test.mjs` (23 tests, `node:test`), run by every `npm run
licenses`, so on every `cargo xtask ci linux` run.
