// CONSTITUTION 10 license check for the npm tree (spec: specs/tooling/licenses.md).
// Pure functions over already-read data, so the test suite can feed hostile trees. Never
// runs package code.

import { posix } from "node:path";

/** Licenses allowed for anything that can reach a shipped artifact (CONSTITUTION 10). */
export const STRICT = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "Zlib",
  "ISC",
  "Unicode-3.0",
]);

/** Extra licenses for test and build tools that never reach a shipped artifact (CONSTITUTION 10). */
export const TOOL_EXTRA = new Set(["MPL-2.0", "BlueOak-1.0.0", "CC0-1.0", "0BSD", "NCSA"]);

/**
 * CC-BY-4.0 is allowed for data only. Package metadata can't tell data from code, so each
 * CC-BY-4.0 tool package is named here after a reviewer checked it ships data only.
 */
export const CC_BY_DATA = new Set(["caniuse-lite"]);

/**
 * The npm registry tarball URL for `name@version`, or null if either is missing. The CC-BY
 * data allowance holds only for a lockfile entry resolved from exactly this URL, so a git or
 * tarball fork that takes a listed name doesn't inherit it.
 */
export function registryTarball(name, version) {
  if (typeof name !== "string" || typeof version !== "string" || version === "") return null;
  const base = name.startsWith("@") ? name.slice(name.indexOf("/") + 1) : name;
  return `https://registry.npmjs.org/${name}/-/${base}-${version}.tgz`;
}

/**
 * Source files Vite may bundle that the scan reads: lower-case `.js .jsx .mjs .cjs .ts .tsx
 * .mts .cts .css .html`. Every other file under src/ is refused (Vite parses `./a.mod` or
 * `./c.JS` as JavaScript and `.pcss` as CSS, and the scan would never read them).
 */
export function sourceExtensionOk(fileName) {
  return /\.(?:[cm]?[jt]sx?|css|html)$/.test(fileName);
}

/** Package name from a lockfile path such as `node_modules/a/node_modules/@s/b`. */
export function nameOf(path) {
  const i = path.lastIndexOf("node_modules/");
  return i < 0 ? path : path.slice(i + "node_modules/".length);
}

/**
 * A plain SPDX id, or `A OR B` (we may pick any allowed option). AND, WITH, parentheses and
 * anything that isn't a string are rejected.
 */
export function licenseOk(expr, name, production) {
  // The OR split alone would already reject these (an "A AND B" token matches no id); the
  // guard keeps the rule explicit if the split ever changes.
  if (typeof expr !== "string" || /[()]|\b(AND|WITH)\b/.test(expr)) return false;
  return expr.split(" OR ").some((raw) => {
    const id = raw.trim();
    if (STRICT.has(id)) return true;
    if (production) return false;
    return TOOL_EXTRA.has(id) || (id === "CC-BY-4.0" && CC_BY_DATA.has(name));
  });
}

/**
 * Node resolution of `dep` required by the package at lockfile path `from` (`""` is the
 * root): the nearest `node_modules/<dep>` walking up from `from`. Returns the lockfile path
 * or null.
 */
export function resolve(packages, from, dep) {
  let dir = from;
  for (;;) {
    const candidate = dir === "" ? `node_modules/${dep}` : `${dir}/node_modules/${dep}`;
    if (Object.hasOwn(packages, candidate)) return candidate;
    if (dir === "") return null;
    const i = dir.lastIndexOf("/node_modules/");
    dir = i < 0 ? "" : dir.slice(0, i);
  }
}

function keys(obj) {
  return obj !== null && typeof obj === "object" ? Object.keys(obj) : [];
}

/**
 * The reachable part of the production tree: every lockfile path reachable from the root
 * manifest's `dependencies` (and `optionalDependencies`) through each package's
 * `dependencies`, `optionalDependencies` and `peerDependencies`. Edges are the union of the
 * installed manifest's and the lockfile entry's. The lockfile's `dev` flags are not used
 * here, so an edited flag can't hide a reachable package; `check` adds every entry without
 * `dev` on top. Installed optional peers count as production (a package may load them at run
 * time).
 */
export function productionTree(packages, rootManifest, manifests, problems) {
  const edges = (m, field) => (m !== null && typeof m === "object" ? keys(m[field]) : []);
  const seen = new Set();
  const queue = [];
  const visit = (from, deps, required) => {
    for (const dep of deps) {
      const path = resolve(packages, from, dep);
      if (path === null) {
        if (required) problems.push(`${from || "package.json"}: dependency ${dep} not in lockfile`);
      } else if (!seen.has(path)) {
        seen.add(path);
        queue.push(path);
      }
    }
  };
  visit("", keys(rootManifest.dependencies), true);
  visit("", keys(rootManifest.optionalDependencies), false);
  while (queue.length > 0) {
    const path = queue.pop();
    const both = [manifests.get(path), packages[path]];
    const all = (field) => both.flatMap((m) => edges(m, field));
    const optional = new Set([...all("optionalDependencies"), ...all("peerDependencies")]);
    visit(
      path,
      [...new Set(all("dependencies"))].filter((d) => !optional.has(d)),
      true,
    );
    visit(path, [...optional], false);
  }
  return seen;
}

/** Root package name of a bare import specifier, or null for relative/absolute/URL ones. */
export function bareRoot(spec) {
  if (spec.startsWith(".") || spec.startsWith("/") || /^[a-z][a-z0-9+.-]*:/i.test(spec)) {
    return null;
  }
  const parts = spec.split("/");
  return spec.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}

// Whitespace and comments allowed between the keyword and the specifier.
const GAP = String.raw`(?:\s|/\*[\s\S]*?\*/|//[^\n]*\n)*`;
const Q = "[\"'`]";
// Static `import … from "x"`, `export … from "x"`, side-effect `import "x"`, dynamic
// `import("x")` and ``import(`x`)`` (comments allowed in the gaps), CSS `@import "x"` and any
// CSS `url(x)`. In HTML files also every `src=` and `href=`. Over-matching (an import-like
// string in a comment) only adds a false alarm.
// In CSS and HTML files matching is case-insensitive (CSS function names, at-rules and HTML
// attribute names are); in JS it is not, so `new URL(x)` isn't read as CSS `url(x)`.
const importRes = (flags) => [
  new RegExp(String.raw`\b(?:from|import)${GAP}(?:\(${GAP})?${Q}([^"'\`]+)${Q}`, flags),
  new RegExp(String.raw`\burl\(\s*["'\`]?([^"'\`)\s]+)`, flags),
];
const IMPORT_RES = importRes("g");
const MARKUP_RES = importRes("gi");
const HTML_RES = [/\b(?:src|href)\s*=\s*["']?([^"'\s>]+)/gi];
// CSS `image-set("x.png" 1x)`: Vite rewrites its plain strings like url().
const IMAGE_SET_RE = /\bimage-set\(((?:[^()]|\([^()]*\))*)\)/gi;
const QUOTED_RE = /["']([^"']+)["']/g;
// Anywhere in the text, not only in import forms: a string naming a path into a package.
const PACKAGE_PATH_RE = /node_modules|\/@fs\/|\/@id\//i;
// `import.meta` other than `import.meta.env`: Vite turns `import.meta.glob(…)` and
// `new URL(…, import.meta.url)` into imports the scan can't follow.
const IMPORT_META_RE = new RegExp(
  String.raw`\bimport${GAP}\.${GAP}meta\b(?!${GAP}\.${GAP}env\b)`,
  "i",
);

/**
 * One problem string for a specifier in shipped source `file` (a path relative to the app
 * directory, such as `src/a.ts` or `index.html`), or null if it is allowed:
 * - a path into node_modules (relative, absolute, Vite's `/@fs/` or `/@id/`) is refused;
 * - a relative or root-absolute path must stay inside `src/`, because files outside it are
 *   not scanned (`../vendor/x.ts` could import a tool package);
 * - a bare specifier must name a package in `deps`;
 * - scheme URLs (`data:`, `https:`) and, in CSS url() and HTML attributes, `#` fragments are
 *   left to the CSP and E2E checks; in JS import forms `#x` is a subpath import and refused.
 */
export function importProblem(file, spec, deps, fragmentsOk = false) {
  if (PACKAGE_PATH_RE.test(spec)) {
    return `${file}: imports ${spec}, a path into node_modules (use the package name)`;
  }
  // `#x` is a fragment in CSS url() and HTML, but a subpath import in JS.
  if (spec.startsWith("#")) {
    return fragmentsOk ? null : `${file}: imports ${spec}, a #subpath import (not checked)`;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(spec)) return null;
  if (spec.startsWith(".") || spec.startsWith("/")) {
    const target = spec.startsWith("/")
      ? posix.normalize(spec.slice(1))
      : posix.normalize(posix.join(posix.dirname(file), spec));
    return target.startsWith("src/")
      ? null
      : `${file}: imports ${spec}, which is outside src/ and not scanned`;
  }
  const root = bareRoot(spec);
  return deps.has(root)
    ? null
    : `${file}: imports ${spec}, which is not in package.json dependencies`;
}

/**
 * Every import in shipped sources must stay in `src/` or name a package in the root
 * manifest's `dependencies`, so nothing from the test/build-only tree can be bundled.
 */
export function checkImports(sources, rootManifest, problems) {
  const deps = new Set(keys(rootManifest.dependencies));
  for (const { file, text } of sources) {
    const specs = [];
    const html = file.endsWith(".html");
    const res = html
      ? [...MARKUP_RES, ...HTML_RES]
      : file.endsWith(".css")
        ? MARKUP_RES
        : IMPORT_RES;
    for (const [i, re] of res.entries()) {
      for (const m of text.matchAll(re)) specs.push([m[1], i > 0]);
    }
    if (file.endsWith(".css")) {
      for (const m of text.matchAll(IMAGE_SET_RE)) {
        // `type("image/png")` names a MIME type, not a file.
        const args = m[1].replace(/\btype\(\s*["'][^"']*["']\s*\)/gi, "");
        for (const q of args.matchAll(QUOTED_RE)) specs.push([q[1], true]);
      }
    }
    let packagePath = false;
    for (const [spec, fragmentsOk] of specs) {
      const problem = importProblem(file, spec, deps, fragmentsOk);
      if (problem === null) continue;
      if (PACKAGE_PATH_RE.test(spec)) packagePath = true;
      problems.push(problem);
    }
    // Strings outside the import forms (`import.meta.glob("/node_modules/x")`, a computed
    // specifier, a comment): one problem per file unless an import already reported it.
    if (!packagePath && PACKAGE_PATH_RE.test(text)) {
      problems.push(`${file}: names a path into node_modules (/@fs/, /@id/); not allowed in src/`);
    }
    if (IMPORT_META_RE.test(text)) {
      problems.push(
        `${file}: uses import.meta other than import.meta.env (glob, url), which the scan can't follow`,
      );
    }
  }
}

/**
 * Checks the whole tree. `manifests` maps lockfile paths to installed `package.json`
 * contents (missing key: not installed). Returns `{ problems, production, installed }`.
 */
export function check({ lock, rootManifest: rawRoot, manifests, sources }) {
  const problems = [];
  let rootManifest = rawRoot;
  if (rootManifest === null || typeof rootManifest !== "object") {
    problems.push("package.json: not an object");
    rootManifest = {};
  }
  const packages = lock !== null && typeof lock === "object" ? (lock.packages ?? {}) : {};
  if (keys(packages).length === 0) problems.push("package-lock.json: no packages");
  const production = productionTree(packages, rootManifest, manifests, problems);
  let installed = 0;
  let prodCount = 0;
  for (const [path, pkg] of Object.entries(packages)) {
    if (path === "") continue;
    if (pkg === null || typeof pkg !== "object") {
      problems.push(`${path}: lockfile entry is not an object`);
      continue;
    }
    const manifest = manifests.get(path);
    // The CC-BY data list matches names, so an npm alias (`"caniuse-lite": "npm:other@1"`)
    // must not inherit it: the real names have to agree with the path.
    const pathName = nameOf(path);
    const realNames = [pkg.name, manifest?.name].filter((n) => n !== undefined);
    const sameName = realNames.every((n) => n === pathName);
    // The CC-BY allowance also needs the registry tarball for that name and version.
    const fromRegistry =
      typeof pkg.resolved === "string" && pkg.resolved === registryTarball(pathName, pkg.version);
    const name = sameName && fromRegistry ? pathName : null;
    // Reachable from `dependencies` (dev flags ignored), or not marked dev in the lockfile
    // (the owner's rule for #50): either one puts the package under the strict list.
    const prod = production.has(path) || pkg.dev !== true;
    if (prod) prodCount += 1;
    const tree = prod ? "production" : "test/build";
    if (!licenseOk(pkg.license, name, prod)) {
      problems.push(`${path}: lockfile says ${pkg.license ?? "no license"} (${tree} tree)`);
    }
    if (manifest !== undefined) {
      installed += 1;
      const real = manifest?.license;
      if (!licenseOk(real, name, prod)) {
        problems.push(`${path}: package.json says ${real ?? "no license"} (${tree} tree)`);
      }
    } else if (!pkg.optional && !pkg.devOptional && !pkg.peer && !pkg.extraneous) {
      // Optional and unused optional-peer (extraneous) entries are never installed.
      problems.push(`${path}: not installed (run npm ci first)`);
    }
  }
  checkImports(sources, rootManifest, problems);
  return { problems, production: prodCount, installed };
}
