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
     `https://registry.npmjs.org/<name>/-/<base>-<version>.tgz`, where `version` is the
     entry's own strict semver version (so `x/../../evil/-/evil-1.0.0` can't steer the
     URL) and the installed manifest, when present, has the same `version`. A git or
     tarball fork that takes a listed name doesn't inherit the allowance either (#112).
   - An expression is a single SPDX id or `A OR B …`, which passes if any option passes for
     that tree. `AND`, `WITH`, parentheses, non-strings and a missing license fail.
     Matching is exact and case-sensitive (`mit` fails).
   - OFL-1.1 (fonts only) is not accepted for npm packages; no font package is used.
3. **Installed.** A lockfile entry that isn't optional, devOptional, peer or extraneous
   must be installed (`run npm ci first`).
4. **Shipped imports.** These forms are scanned in the shipped sources. Strings are
   matched with each quote closed by its own kind (`"./'/../x"` is one specifier), and may
   span lines (a line continuation is captured, then refused for its `\`):
   - **import positions** (code): `import … from "x"`, `export … from "x"`, `import "x"`,
     `import("x")` and ``import(`x`)``, with comments allowed between the keyword and the
     specifier; CSS `@import "x"` and `@import url(x)`;
   - **asset positions**: any CSS `url(x)`, every quoted string inside CSS `image-set(…)`
     except `type("…")` MIME types, and in HTML files the `src`, `href`, `poster` and
     `srcset` attributes, plus `content` when its value has no whitespace and contains
     `/` or starts with `.` (`<meta property="og:image">`; otherwise it is plain text such
     as `light dark`); quoted or unquoted. `srcset` is split the way the HTML parser does
     it: a URL runs to whitespace (trailing commas stripped), then a descriptor runs to the
     next comma, so `data:image/png;base64,AA== 1x` is one URL and `a.png 1x,b.png 2x` two.
   - In `.css` and `.html` files the forms match case-insensitively (`URL(x)`, `@IMPORT`,
     `SRC=`), because CSS and HTML are; in JS they match case-sensitively, so `new URL(x)`
     is not read as a CSS `url(x)`. HTML files get the JS and CSS forms too (inline
     `<script>` and `<style>`).

   Each specifier is then checked in this order; one line per distinct problem:
   - Any specifier containing `node_modules`, `/@fs/` or `/@id/` (any case) is refused: a
     path into a package bypasses its name.
   - **Scheme URLs.** In import positions every scheme is refused: Vite's build bundles
     `data:text/javascript,…` modules and resolves `file:` URLs. In asset positions only
     `data:`, `http:`, `https:` and `mailto:` are skipped; the CSP and the E2E
     no-third-party-request test cover them. Any other scheme (`file:`, `javascript:`) is refused.
   - **`#x`** is a fragment in asset positions (skipped) and a subpath import in import
     positions (refused). A root `package.json` `imports` field is refused too.
   - **Characters.** Only `A–Z a–z 0–9 @ . _ / ~ ? = # + -` are allowed. Anything else
     (`\` escapes, `&` entities, `%` encodings, quotes, whitespace, `$`, `&` in queries)
     could decode to a different path than the one checked, so it is refused rather than
     decoded (#112 review). An unquoted CSS `url( x )` is trimmed first.
   - A relative or root-absolute path must resolve inside `src/` (from the importing file's
     directory; `/` is the app directory). Files outside `src/` aren't scanned, so
     `../vendor/x.ts` could import a tool package.
   - A bare specifier must have no `.` or `..` segment (`ui/../tool/x.js` would leave the
     package for one without `exports`) and must name a package in the root
     `dependencies`. Root `optionalDependencies` and `peerDependencies` are not enough.
     Scoped names use two segments.

   And over the **whole text** of each shipped source (comments included; one line each):
   - any `node_modules`, `/@fs/` or `/@id/` (any case), unless an import form already
     reported it, so computed specifiers and Vite-only forms can't name a package path
     (#112);
   - `import.meta` in every form except `import.meta.env` (comments allowed in the gaps).
     Vite turns `import.meta.glob(…)` and `new URL(…, import.meta.url)` into imports the
     scan can't follow (#112). This also refuses the `new Worker(new URL("./w.ts",
     import.meta.url))` pattern; use a `?worker` import, or land #99 first;
   - in JS and HTML, any `import(` that doesn't hold exactly one string literal followed
     by `)` or `,` (import attributes). Vite turns `import("./" + x + ".ts")` and
     `import("./".concat(…))` into globs over every joined literal, and a computed
     specifier can't be checked;
   - any `@jsx` pragma (`@jsxImportSource`, `@jsx`, `@jsxFrag`, `@jsxRuntime`): an import
     source makes esbuild inject `import "<source>/jsx-runtime"`.
5. **Bundle guards.** These change what gets bundled, and the import scan can't follow
   them:
   - Vite copies `public/` verbatim.
   - These config keys: `alias`, `publicDir`, `root`, `input`, library builds (`lib`,
     `entry`), preprocessor injections (`additionalData`), code or import-source injection
     (`jsxInject`, `jsxImportSource`, `tsconfigRaw`, `renderBuiltUrl`) and an inline
     `postcss` config.
   - Plugin hooks that resolve, load or transform modules (`resolveId`,
     `resolveDynamicImport`, `load`, `transform`, `renderChunk`, `generateBundle`), emit
     chunks (`emitFile`), edit the config or Rollup options (`config`, `configResolved`,
     `options`), inject text (`banner`, `footer`, `intro`, `outro`,
     `renderDynamicImport`, `resolveImportMeta`, `resolveFileUrl`) or touch the written
     output (`writeBundle`, `closeBundle`).
   - Another `vite.config.*` file, which Vite loads before `.ts`.
   - Plugins imported from another module, which hide their hooks. `./csp` is the one
     allowed module, so `csp.ts` must import nothing (no `import`, `export … from` or
     `require(`), carries none of the keys above, and names none of `process`,
     `globalThis`, `global`, `getBuiltinModule`, `eval`, `Function`, `Reflect`, `module`,
     `exports` (it runs at config load, so it could otherwise write a file into `src/`
     after the scan). Hyphenated CSP words (`'wasm-unsafe-eval'`) don't count.
   - `tsconfig.json`: esbuild takes `jsxImportSource` from it (or from what it `extends`).
     It must exist (otherwise a parent directory's applies) and must not contain
     `jsxImportSource`, `extends`, `references` (tsconfck would pick a referenced config
     for a file) or a `\u` escape (JSON decodes it, so it could spell a key).
   - A PostCSS config, which Vite loads automatically and whose plugins can inject CSS and
     imports: `.postcssrc*`, `postcss.config.*` or `.config/postcssrc*` in the app
     directory **or any parent directory**, or a `postcss` field in a `package.json` there
     (an unreadable `package.json` there is refused too).

   None is used. The script fails in any of these cases:
   - `public/` exists, or another `vite.config.*` exists;
   - `vite.config.ts` imports anything but `vite` and `./csp`;
   - `vite.config.ts` or `csp.ts` names one of those keys: plain, quoted, computed
     (`["alias"]:`, `` [`alias`]: ``) or shorthand (`{ alias }`);
   - the `csp.ts`, `tsconfig.json` or PostCSS rules above.

   This is a text match, so it is advisory until #99 checks Rollup's real module graph.
   It can't see, for example: `vite build --config x`; a key built at run time
   (`{ [k]: v }` with `k = "al" + "ias"`); `transformIndexHtml` (allowed, our CSP plugin
   uses it) returning a `<script src>` tag; plugin code that reads a file itself
   (`fs.readFileSync` into a string) or **writes one into `src/` after this scan** (any
   code in `vite.config.ts` can; `csp.ts` is held to the identifier list above);
   environment-specific config (`environments.*`) and hooks not listed above.
6. **Source files.** Every regular file under `src/` must have a lower-case
   `.js/.jsx/.mjs/.cjs/.ts/.tsx/.mts/.cts/.css/.html` extension, and all of them are
   scanned. Any other file is refused (fail closed, #112): Vite parses `./a.mod` or
   `./c.JS` as JavaScript and `.pcss`/`.postcss`/`.sss` (and `.scss`/`.less`/`.styl` with a
   preprocessor) as CSS, and the scan would never read them. Images, JSON and other assets
   need a scan extension first, and so does the WASM engine. A Finder `.DS_Store` in
   `src/` fails a local run too (Vite would parse `./.DS_Store` as JavaScript). Symlinks anywhere in `src/` are refused, because Vite
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
    suffix, other name, missing, different or traversal version, a manifest with another
    version). Each new guard key is checked in six spellings (plain, method, quoted,
    shorthand, computed with quotes and with backticks), with false-positive controls
    (`libX`, `entryFileNames`, `optionsX`, `defineConfig`).
  - From the review of this change, each refused in "specifiers that decode or resolve
    past the check": one that skips schemes in import positions (`data:`, `file:`), skips
    any asset scheme (`file:`, `javascript:`), pairs quotes loosely (`"./'/../x"`),
    drops the character allowlist (`\x2e`, `&#46;`, `%2e`, whitespace), drops the
    `.`/`..` segment rule, the `@jsx` pragma rule, or the `srcset`/`poster`/`content`
    attributes. The integration test refuses imports and guarded keys in `csp.ts`, a
    `jsxImportSource` or `extends` in `tsconfig.json` and a missing one, PostCSS configs
    in the app directory, its `.config/` and its parent, and `postcss` fields (app and
    parent `package.json`, and an unreadable one). The comment-gap pattern is checked
    for exponential backtracking with 60 comments, and unquoted `url(` with a 20 000-space
    run (the earlier pattern took minutes). The delta review added: a line continuation
    in a string, `+`/`.concat`/variable/template dynamic imports, `srcset` without spaces
    after commas and with a `data:` URL, `mailto:`, `content` without a leading slash,
    `csp.ts` identifiers, `references` and `\u` in `tsconfig.json`.
  - Each of these mutations was applied by hand and failed the suite.
  - The integration test runs the real script on a fixture app directory. It covers nested
    `src/` directories and `.cts` files. It refuses a symlinked file, a symlinked directory
    (and the files under it), a source-named directory, the `imports` field, foreign
    imports in `vite.config.ts` and a stray `vite.config.mjs`. It covers installed
    manifests and every guard key in four spellings (six for the keys added in #112). It
    also checks for false
    positives (`transformIndexHtml`, `rooted`).
  - One that skips relative paths fails the outside-`src/` cases. One that drops comment
    gaps fails the `import(/* chunk */ "tool")` case. The alias test checks the lockfile
    `name` and the manifest `name` separately.
  - `/@fs/` and `/@id/` in the any-text rule are tested (`fetch("/@fs/…")`,
    `// see /@id/tool`); in import positions they are also caught as paths outside `src/`.
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
  - `e2e/` isn't scanned for imports: it runs at test time, and `src/` can't import it.
    `vite.config.ts` and `csp.ts` get only the text guards of item 5.
  - The import scan is a regular expression. Over-matching (an import-like string in a
    comment) only adds a false alarm. Every `import(` that isn't one string literal is
    refused. It misses `require` (Vite doesn't bundle it in ESM). Review has to catch the
    rest in `src/` until #99.

## Accessibility
Not applicable.

## Platforms
Node ≥ 22.12 (the `engines` field); uses `node:test`, `fs.readdirSync({ recursive })` and
`Dirent.parentPath`. Same on Linux CI and macOS.

## Performance
About 0.1 s for 186 packages plus about 0.1 s of tests.

## Tests
`apps/web/scripts/licenses.test.mjs` (41 tests, `node:test`, one of them an integration
test of `check-licenses.mjs` on a fixture directory under the git-ignored
`node_modules/.cache/`), run by every `npm run
licenses`, so on every `cargo xtask ci linux` run.
