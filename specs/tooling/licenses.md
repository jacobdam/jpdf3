# npm license check (production and test/build trees)

Area: tooling (ui-web build)
ISO 32000-2 clauses: none (supply chain; CONSTITUTION 10)
Journeys: none (a CI step of `cargo xtask ci linux`, specs/ui-web/app-skeleton.md step 3)
Status: implemented

## Behavior
`npm run licenses` in `apps/web` first runs the unit tests (`node --test
scripts/licenses.test.mjs`), then `scripts/check-licenses.mjs`. That script reads
`package-lock.json`, `package.json`, every installed `node_modules/<path>/package.json`,
`index.html`, `vite.config.ts`, the app directory's file names and every `.js/.jsx/.mjs/.cjs/.ts/.tsx/.mts/.cts/.css/.html`
file (or symlink) under `src/`, recursively. An optional argument names another app
directory (used by the integration test). The
logic is in `scripts/licenses-lib.mjs`, pure functions over that data. It never runs package
code and has no dependencies.

1. **Production tree.** It is the union of two sets:
   - **The owner's rule (#50):** every lockfile entry without `dev: true`.
   - **Reachable:** every lockfile path reachable from the root `package.json`
     `dependencies` and `optionalDependencies`, following each package's `dependencies`,
     `optionalDependencies` and `peerDependencies` with Node's resolution rule (the nearest
     `node_modules/<name>`, walking up from the requiring package). Edges are the union of
     the installed `package.json` and the lockfile entry. This set ignores `dev` flags, so an
     edited flag can't move a reachable package out of the production tree. Installed
     optional peers count as production, because a package may load them at run time.

   The union is at least as strict as the owner's rule and stricter than npm's view.
2. **Licenses.** Each lockfile entry's `license` and, if the package is installed, its real
   `package.json` `license` must pass:
   - production tree: the strict list (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Zlib,
     ISC, Unicode-3.0);
   - every other package (test and build tools): the strict list plus MPL-2.0,
     BlueOak-1.0.0, CC0-1.0, 0BSD and NCSA, plus CC-BY-4.0 **only** for package names in
     `CC_BY_DATA` (now `caniuse-lite`). CONSTITUTION 10 allows CC-BY-4.0 for data only, and
     metadata can't tell data from code, so each name is a reviewed decision. The name comes
     from the lockfile path, and the lockfile's and the manifest's `name` (when present)
     must agree with it, so an npm alias (`"caniuse-lite": "npm:other@1"`) doesn't inherit
     the allowance.
   - An expression is a single SPDX id or `A OR B …`, which passes if any option passes for
     that tree. `AND`, `WITH`, parentheses, non-strings and a missing license fail.
     Matching is exact and case-sensitive (`mit` fails).
   - OFL-1.1 (fonts only) is not accepted for npm packages; no font package is used.
3. **Installed.** A lockfile entry that isn't optional, devOptional, peer or extraneous
   must be installed (`run npm ci first`).
4. **Shipped imports.** These forms are scanned in the shipped sources:
   - `import … from "x"`, `export … from "x"`, `import "x"`, `import("x")` and
     ``import(`x`)``, with comments allowed between the keyword and the specifier;
   - CSS `@import "x"` and any CSS `url(x)`;
   - in HTML files, every `src=` and `href=` attribute.
   - Every bare specifier must name a package in the root `dependencies`. Root
     `optionalDependencies` and `peerDependencies` are not enough. Scoped names use two
     segments.
   - Any specifier containing `node_modules`, `/@fs/` or `/@id/` is refused: a path into a
     package bypasses its name.
   - A relative or root-absolute path must resolve inside `src/` (from the importing file's
     directory; `/` is the app directory). Files outside `src/` aren't scanned, so
     `../vendor/x.ts` could import a tool package.
   - Scheme URLs (`data:`, `https:`) are skipped, and so are `#` fragments in CSS `url()`
     and HTML attributes. The CSP and the E2E no-third-party-request test cover them. In
     JS import forms `#x` is a subpath import and is refused. A root `package.json`
     `imports` field is refused too.
5. **Bundle guards.** These change what gets bundled, and the import scan can't follow
   them:
   - Vite copies `public/` verbatim.
   - These config keys: `alias`, `publicDir`, `root`, `input`.
   - Plugin hooks `resolveId`, `load` and `transform`.
   - Another `vite.config.*` file, which Vite loads before `.ts`.
   - Plugins imported from another module, which hide their hooks.

   None is used. The script fails in any of these cases:
   - `public/` exists, or another `vite.config.*` exists;
   - `vite.config.ts` imports anything but `vite` and `./csp`;
   - `vite.config.ts` names one of those keys: plain, quoted, computed (`["alias"]:`) or
     shorthand (`{ alias }`).

   This is a text match, so it is advisory until #99 checks Rollup's real module graph.
   For example, `vite build --config x` bypasses it.
6. **Source files.** Files under `src/` with a source extension are scanned. Symlinks
   anywhere in `src/` are refused, because Vite resolves a symlinked module's imports from
   its real location. Any file whose real path is outside `src/` is refused too. A
   directory with a source-file name is reported.

## Persistence
Not applicable. Output goes to stderr: `licenses: ok (N packages, P production, I installed,
S source files)`, or one line per problem with the path, the license and which tree applied,
and then exit 1.

## Undo
Not applicable.

## Errors
Every problem is a line; the script exits 1 if there is any. Hostile shapes (a root
manifest that isn't an object, no `packages`, a `null` entry, a `null` installed manifest)
become problems, not exceptions.
An unreadable or non-JSON file throws, which also exits non-zero and fails the CI step.

## Limits
The inputs are our own committed lockfile and the tarballs `npm ci` verified by integrity
hash, so there are no size limits. The tree walk visits each lockfile path once (a `seen`
set), so cycles terminate (tested). Resolution walks up at most the depth of the path.

## Security
- **What broken implementation still passes?**
  - A checker that trusts the lockfile `dev` flag fails the test "a lockfile that marks a
    production package dev doesn't move it to the tool tree". One that uses reachability
    alone fails "an entry without dev is production even when nothing reaches it".
  - One that reads only the lockfile fails the installed-manifest tests. One that builds
    edges only from the lockfile fails "edges come from the installed manifest".
  - One that ignores peers or optional dependencies fails the peer/optional test.
  - One that resolves by name only (not nearest `node_modules`) fails the nested tests.
    One that skips intermediate levels (`indexOf` for `lastIndexOf`) fails the deep-path
    test.
  - One that drops root `optionalDependencies` fails the root-optional test.
  - One that allows CC-BY-4.0 by license alone fails the CC-BY tests. These include a
    scoped `@x/caniuse-lite` look-alike and an npm alias.
  - The integration test runs the real script on a fixture app directory. It covers nested
    `src/` directories and `.cts` files. It refuses a symlinked file, a symlinked directory
    (and the files under it), a source-named directory, the `imports` field, foreign
    imports in `vite.config.ts` and a stray `vite.config.mjs`. It covers installed
    manifests and every guard key in all four spellings. It also checks for false
    positives (`transformIndexHtml`, `rooted`).
  - One that skips relative paths fails the outside-`src/` cases. One that drops comment
    gaps fails the `import(/* chunk */ "tool")` case. The alias test checks the lockfile
    `name` and the manifest `name` separately.
  - Dropping `/@fs/` or `/@id/` from the `node_modules` pattern passes the tests, because
    those root-absolute paths are also outside `src/`. They are kept as defense in depth.
  - The `AND`/`WITH`/parentheses guard is redundant with the exact `OR` split. Removing it
    passes the tests and changes nothing.
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
  - **Phantom dependencies:** a production package that `import`s a hoisted package it
    doesn't declare. Node and Vite would resolve it, and the code would be bundled without
    a strict check. The owner's rule has the same gap. A full fix checks Rollup's module
    graph (every bundled module under a production-tree path); that is #99.
  - `vite.config.ts`, `csp.ts` and `e2e/` aren't scanned for imports. They run at build or
    test time, and `src/` can't import them (they're outside `src/`).
  - The import scan is a regular expression. Over-matching (an import-like string in a
    comment) only adds a false alarm. It misses computed specifiers (``import(`${x}`)``,
    `new URL(x, import.meta.url)`) and `require`. Review has to catch those in `src/`.

## Accessibility
Not applicable.

## Platforms
Node ≥ 22.12 (the `engines` field); uses `node:test`, `fs.readdirSync({ recursive })` and
`Dirent.parentPath`. Same on Linux CI and macOS.

## Performance
About 0.1 s for 186 packages plus about 0.1 s of tests.

## Tests
`apps/web/scripts/licenses.test.mjs` (33 tests, `node:test`, one of them an integration
test of `check-licenses.mjs` on a fixture directory under the git-ignored
`node_modules/.cache/`), run by every `npm run
licenses`, so on every `cargo xtask ci linux` run.
