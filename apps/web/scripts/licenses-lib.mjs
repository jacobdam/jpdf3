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
  // A strict semver version, so `x/../../evil/-/evil-1.0.0` can't steer the URL.
  const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
  if (typeof name !== "string" || typeof version !== "string" || !semver.test(version)) {
    return null;
  }
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

// Whitespace and comments allowed between the keyword and the specifier. The comment body
// can't run past `*/` (a lazy `[\s\S]*?` backtracks exponentially over many comments).
const GAP = String.raw`(?:\s|/\*(?:[^*]|\*(?!/))*\*/|//[^\n]*\n)*`;
// A quoted string, each quote kind closed by its own kind (`"a'b"` is one string). Newlines
// are allowed inside, so a line continuation (`"./a\<newline>/../x"`) is still captured
// (and then refused for its `\`).
const QSTR = String.raw`(?:"([^"]*)"|'([^']*)'|` + "`([^`]*)`)";
// url(…): a quoted string, or an unquoted run trimmed in code. One alternative each, so
// whitespace isn't shared between quantifiers (no polynomial backtracking).
const URL_ARG = String.raw`(?:\s*${QSTR}\s*|([^)"'\`]*))\)`;
// Static `import … from "x"`, `export … from "x"`, side-effect `import "x"`, dynamic
// `import("x")` and ``import(`x`)`` (comments allowed in the gaps), CSS `@import "x"` and
// `@import url(x)`: these load code, so they are "import" specifiers. CSS `url(x)`,
// `image-set("x")` strings and HTML attributes are "asset" specifiers. Over-matching (an
// import-like string in a comment) only adds a false alarm.
// In CSS and HTML files matching is case-insensitive (CSS function names, at-rules and HTML
// attribute names are); in JS it is not, so `new URL(x)` isn't read as CSS `url(x)`.
const sourceRes = (flags) => [
  ["import", new RegExp(String.raw`\b(?:from|import)${GAP}(?:\(${GAP})?${QSTR}`, flags)],
  ["import", new RegExp(String.raw`@import${GAP}url\(${URL_ARG}`, flags)],
  ["asset", new RegExp(String.raw`\burl\(${URL_ARG}`, flags)],
];
// Vite turns `import("./" + x + ".ts")` and `import("./".concat(x))` into globs over the
// joined literals, which the scan would check only the first of. So every JS `import(` must
// hold exactly one string literal, followed by `)` or `,` (import attributes).
const DYNAMIC_IMPORT_RE = new RegExp(String.raw`\bimport${GAP}\(`, "g");
const SINGLE_LITERAL_RE = new RegExp(String.raw`^${GAP}${QSTR}${GAP}[),]`);
const JS_RES = sourceRes("g");
const MARKUP_RES = sourceRes("gi");
// HTML attributes Vite rewrites: `src`, `href`, `srcset` (comma-separated `url size`),
// `poster` and `<meta content>` (og:image). `content` is checked only when it looks like a
// path, because it is also plain text (`width=device-width`).
const HTML_ATTR_RE =
  /\b(src|href|srcset|poster|content)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))/gi;
// CSS `image-set("x.png" 1x)`: Vite rewrites its plain strings like url().
const IMAGE_SET_RE = /\bimage-set\(((?:[^()]|\([^()]*\))*)\)/gi;
const QUOTED_RE = /"([^"\n]*)"|'([^'\n]*)'/g;
// Anywhere in the text, not only in import forms: a string naming a path into a package.
const PACKAGE_PATH_RE = /node_modules|\/@fs\/|\/@id\//i;
// `import.meta` other than `import.meta.env`: Vite turns `import.meta.glob(…)` and
// `new URL(…, import.meta.url)` into imports the scan can't follow.
const IMPORT_META_RE = new RegExp(
  String.raw`\bimport${GAP}\.${GAP}meta\b(?!${GAP}\.${GAP}env\b)`,
  "i",
);
// esbuild's `@jsx`, `@jsxImportSource`, `@jsxFrag`, `@jsxRuntime` pragmas: an import source
// makes esbuild inject `import "<source>/jsx-runtime"`, which the scan never sees.
const JSX_PRAGMA_RE = /@jsx/i;
// Characters a checked path specifier may use. Anything else (`\` escapes, `&` entities,
// `%` encodings, quotes, whitespace, `$`) could decode to a different path than the one
// checked, so it is refused rather than decoded.
const SPEC_CHARS_RE = /^[A-Za-z0-9@._/~?=#+-]+$/;
// Scheme URLs left to the CSP and the E2E no-third-party-request test, in asset positions
// only. Vite bundles `data:` and `file:` modules, so import positions refuse every scheme.
const ASSET_SCHEMES_RE = /^(?:data|https?|mailto):/i;

/**
 * One problem string for a specifier in shipped source `file` (a path relative to the app
 * directory, such as `src/a.ts` or `index.html`), or null if it is allowed. `kind` is
 * `"import"` (code: JS import forms, CSS `@import`) or `"asset"` (CSS `url()`, `image-set`,
 * HTML attributes):
 * - a path into node_modules (relative, absolute, Vite's `/@fs/` or `/@id/`) is refused;
 * - a scheme URL is refused in import positions; in asset positions only `data:`, `http:`
 *   and `https:` are allowed (left to the CSP and E2E checks);
 * - `#x` is a fragment in asset positions and a refused subpath import in import positions;
 * - any character outside `A-Z a-z 0-9 @ . _ / ~ ? = # + -` is refused (no decoding);
 * - a relative or root-absolute path must stay inside `src/`, because files outside it are
 *   not scanned (`../vendor/x.ts` could import a tool package);
 * - a bare specifier must name a package in `deps` and have no `.` or `..` segment.
 */
export function importProblem(file, spec, deps, kind = "import") {
  if (kind === "dynamic") {
    return `${file}: has an import(…) that isn't one string literal (Vite globs + and .concat)`;
  }
  const asset = kind === "asset";
  if (PACKAGE_PATH_RE.test(spec)) {
    return `${file}: imports ${spec}, a path into node_modules (use the package name)`;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(spec)) {
    if (asset && ASSET_SCHEMES_RE.test(spec)) return null;
    return `${file}: imports ${spec}, a scheme URL (Vite bundles data: and file: modules)`;
  }
  if (spec.startsWith("#")) {
    return asset ? null : `${file}: imports ${spec}, a #subpath import (not checked)`;
  }
  if (!SPEC_CHARS_RE.test(spec)) {
    return `${file}: imports ${JSON.stringify(spec)}, which has characters the scan doesn't decode`;
  }
  if (spec.startsWith(".") || spec.startsWith("/")) {
    const target = spec.startsWith("/")
      ? posix.normalize(spec.slice(1))
      : posix.normalize(posix.join(posix.dirname(file), spec));
    return target.startsWith("src/")
      ? null
      : `${file}: imports ${spec}, which is outside src/ and not scanned`;
  }
  if (spec.split("/").some((s) => s === "." || s === "..")) {
    return `${file}: imports ${spec}, a package path with . or .. segments`;
  }
  const root = bareRoot(spec);
  return deps.has(root)
    ? null
    : `${file}: imports ${spec}, which is not in package.json dependencies`;
}

const captured = (m, from = 1) => m.slice(from).find((g) => g !== undefined) ?? "";

/**
 * The URLs in a `srcset` value, as the HTML parser reads them: a URL runs to whitespace
 * (trailing commas stripped), then a descriptor runs to the next comma. So
 * `data:image/png;base64,AA== 1x` is one URL and `a.png 1x,b.png 2x` two.
 */
export function srcsetUrls(value) {
  const urls = [];
  let i = 0;
  while (i < value.length) {
    while (i < value.length && /[\s,]/.test(value.charAt(i))) i += 1;
    if (i >= value.length) break;
    let j = i;
    while (j < value.length && !/\s/.test(value.charAt(j))) j += 1;
    let url = value.slice(i, j);
    if (/,$/.test(url)) {
      url = url.replace(/,+$/, "");
    } else {
      while (j < value.length && value.charAt(j) !== ",") j += 1;
    }
    urls.push(url);
    i = j;
  }
  return urls;
}

/** The asset specifiers in one HTML attribute. */
function attrSpecs(name, value) {
  const attr = name.toLowerCase();
  if (attr === "srcset") return srcsetUrls(value);
  // `content` is also plain text (`width=device-width, initial-scale=1`, `light dark`);
  // check it when it looks like a path (`/` or a leading `.`, no whitespace).
  if (attr === "content") return /^\S*$/.test(value) && /\/|^\./.test(value) ? [value] : [];
  return [value];
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
    const css = file.endsWith(".css");
    for (const [kind, re] of html || css ? MARKUP_RES : JS_RES) {
      for (const m of text.matchAll(re)) {
        const spec = captured(m);
        // An unquoted CSS url() may carry surrounding whitespace; inner whitespace stays.
        specs.push([m[4] !== undefined ? spec.trim() : spec, kind]);
      }
    }
    if (!css) {
      for (const m of text.matchAll(DYNAMIC_IMPORT_RE)) {
        const rest = text.slice((m.index ?? 0) + m[0].length);
        if (!SINGLE_LITERAL_RE.test(rest)) specs.push(["", "dynamic"]);
      }
    }
    if (html) {
      for (const m of text.matchAll(HTML_ATTR_RE)) {
        for (const spec of attrSpecs(m[1], captured(m, 2))) specs.push([spec, "asset"]);
      }
    }
    if (css) {
      for (const m of text.matchAll(IMAGE_SET_RE)) {
        // `type("image/png")` names a MIME type, not a file.
        const args = m[1].replace(/\btype\(\s*["'][^"']*["']\s*\)/gi, "");
        for (const q of args.matchAll(QUOTED_RE)) specs.push([captured(q), "asset"]);
      }
    }
    // One line per distinct problem (`@import url(x)` matches two forms).
    const found = new Set();
    let packagePath = false;
    for (const [spec, kind] of specs) {
      const problem = importProblem(file, spec, deps, kind);
      if (problem === null) continue;
      if (PACKAGE_PATH_RE.test(spec)) packagePath = true;
      found.add(problem);
    }
    // Strings outside the import forms (`import.meta.glob("/node_modules/x")`, a computed
    // specifier, a comment): one problem per file unless an import already reported it.
    if (!packagePath && PACKAGE_PATH_RE.test(text)) {
      found.add(`${file}: names a path into node_modules (/@fs/, /@id/); not allowed in src/`);
    }
    if (IMPORT_META_RE.test(text)) {
      found.add(
        `${file}: uses import.meta other than import.meta.env (glob, url), which the scan can't follow`,
      );
    }
    if (JSX_PRAGMA_RE.test(text)) {
      found.add(`${file}: has an @jsx pragma, which can make esbuild inject an import`);
    }
    problems.push(...found);
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
    // The installed manifest (when present) must have the same version.
    const fromRegistry =
      typeof pkg.resolved === "string" &&
      pkg.resolved === registryTarball(pathName, pkg.version) &&
      (manifest === undefined || manifest?.version === pkg.version);
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
