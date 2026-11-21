# npm license check (production and test/build trees)

Area: tooling (ui-web build)
ISO 32000-2 clauses: none (supply chain; CONSTITUTION 10)
Journeys: none (a CI step of `cargo xtask ci linux`, specs/ui-web/app-skeleton.md step 3)
Status: implemented

## Behavior
`npm run licenses` in `apps/web` first runs the unit tests (`node --test
scripts/licenses.test.mjs`), then `scripts/check-licenses.mjs`. That script reads
`package-lock.json`, `package.json`, every installed `node_modules/<path>/package.json`,
`index.html`, `vite.config.ts`, the app directory's file names and every file under `src/`,
recursively (only lower-case `.js/.jsx/.mjs/.cjs/.ts/.tsx/.mts/.cts/.css/.html` files are
allowed there; see item 6). An optional argument names another app
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
     the allowance. The lockfile entry must also be `resolved` from exactly
     `https://registry.npmjs.org/<name>/-/<base>-<version>.tgz` (its own `version`), so a
     git or tarball fork that takes a listed name doesn't inherit it either (#112).
   - An expression is a single SPDX id or `A OR B …`, which passes if any option passes for
     that tree. `AND`, `WITH`, parentheses, non-strings and a missing license fail.
     Matching is exact and case-sensitive (`mit` fails).
   - OFL-1.1 (fonts only) is not accepted for npm packages; no font package is used.
3. **Installed.** A lockfile entry that isn't optional, devOptional, peer or extraneous
   must be installed (`run npm ci first`).
4. **Shipped imports.** These forms are scanned in the shipped sources:
   - `import … from "x"`, `export … from "x"`, `import "x"`, `import("x")` and
     ``import(`x`)``, with comments allowed between the keyword and the specifier;
   - CSS `@import "x"`, any CSS `url(x)` and every quoted string inside CSS `image-set(…)`
     except `type("…")` MIME types;
   - in HTML files, every `src=` and `href=` attribute.
   - In `.css` and `.html` files these match case-insensitively (`URL(x)`, `@IMPORT`,
     `SRC=`), because CSS and HTML are; in JS they match case-sensitively, so `new URL(x)`
     is not read as a CSS `url(x)`.
   - Every bare specifier must name a package in the root `dependencies`. Root
     `optionalDependencies` and `peerDependencies` are not enough. Scoped names use two
     segments.
   - Any specifier containing `node_modules`, `/@fs/` or `/@id/` (any case) is refused: a
     path into a package bypasses its name. **Any text** in a shipped source that contains
     one of them is refused too (one problem per file unless an import form already
     reported it), so computed specifiers and Vite-only forms can't name a package path
     (#112).
   - `import.meta` is refused in every form except `import.meta.env` (comments allowed in
     the gaps). Vite turns `import.meta.glob(…)` and `new URL(…, import.meta.url)` into
     imports the scan can't follow (#112).
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
   - These config keys: `alias`, `publicDir`, `root`, `input`, library builds (`lib`,
     `entry`) and preprocessor injections (`additionalData`).
   - Plugin hooks that resolve, load or transform modules (`resolveId`,
     `resolveDynamicImport`, `load`, `transform`, `renderChunk`, `generateBundle`), emit
     chunks (`emitFile`), edit the config or Rollup options (`config`, `configResolved`,
     `options`) or inject text (`banner`, `footer`, `intro`, `outro`).
   - Another `vite.config.*` file, which Vite loads before `.ts`.
   - Plugins imported from another module, which hide their hooks.

   None is used. The script fails in any of these cases:
   - `public/` exists, or another `vite.config.*` exists;
   - `vite.config.ts` imports anything but `vite` and `./csp`;
   - `vite.config.ts` names one of those keys: plain, quoted, computed (`["alias"]:`) or
     shorthand (`{ alias }`).

   This is a text match, so it is advisory until #99 checks Rollup's real module graph.
   It can't see, for example: `vite build --config x`; a key built at run time
   (`{ [k]: v }` with `k = "al" + "ias"`); `transformIndexHtml` (allowed, our CSP plugin
   uses it) returning a `<script src>` tag; plugin code that reads a file itself
   (`fs.readFileSync` into a string); environment-specific config (`environments.*`) and
   hooks not listed above.
6. **Source files.** Every regular file under `src/` must have a lower-case
   `.js/.jsx/.mjs/.cjs/.ts/.tsx/.mts/.cts/.css/.html` extension, and all of them are
   scanned. Any other file is refused (fail closed, #112): Vite parses `./a.mod` or
   `./c.JS` as JavaScript and `.pcss`/`.postcss`/`.sss` (and `.scss`/`.less`/`.styl` with a
   preprocessor) as CSS, and the scan would never read them. Images, JSON and other assets
   need a scan extension first. Symlinks anywhere in `src/` are refused, because Vite
   resolves a symlinked module's imports from its real location. Any file whose real path
   is outside `src/` is refused too. A directory with a source-file name, and anything that
   is neither a regular file nor a directory (a FIFO), is reported.

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
  - One that filters extensions instead of refusing unknown ones (the audit 0002 bypass), or
    matches them case-insensitively, fails the integration test's `a.mod`, `c.JS`,
    `s.pcss` and `README` cases and the extension unit test. One that skips the
    `import.meta` rule, or doesn't exempt `import.meta.env`, fails the `import.meta` test;
    one that checks `node_modules` only in import forms, or case-sensitively, fails the
    any-text cases (`import.meta.glob("/node_modules/…")`, `` `../NODE_MODULES/…` ``). One
    that matches CSS case-sensitively, or JS case-insensitively (`new URL(href)`), or
    skips `image-set` or its `type()` strip, fails the CSS test. One that grants CC-BY
    without the exact registry tarball fails the fork cases (git, other host, query
    suffix, other name, missing or different version). Each new guard key is checked in
    four spellings, with false-positive controls (`libX`, `entryFileNames`, `optionsX`,
    `defineConfig`). Each of these mutations was applied by hand and failed the suite.
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
    comment) only adds a false alarm. It misses computed bare specifiers
    (``import(`${x}`)``, which Vite leaves to run time and so doesn't bundle) and
    `require`. `import.meta.glob` and `new URL(…, import.meta.url)` are refused outright.
    Review has to catch the rest in `src/` until #99.

## Accessibility
Not applicable.

## Platforms
Node ≥ 22.12 (the `engines` field); uses `node:test`, `fs.readdirSync({ recursive })` and
`Dirent.parentPath`. Same on Linux CI and macOS.

## Performance
About 0.1 s for 186 packages plus about 0.1 s of tests.

## Tests
`apps/web/scripts/licenses.test.mjs` (37 tests, `node:test`, one of them an integration
test of `check-licenses.mjs` on a fixture directory under the git-ignored
`node_modules/.cache/`), run by every `npm run
licenses`, so on every `cargo xtask ci linux` run.
