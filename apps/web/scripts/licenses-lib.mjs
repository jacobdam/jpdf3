// CONSTITUTION 10 license check for the npm tree (spec: specs/tooling/licenses.md).
// Pure functions over already-read data, so the test suite can feed hostile trees. Never
// runs package code.

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
 * The production tree: every lockfile path reachable from the root manifest's
 * `dependencies` (and `optionalDependencies`) through each package's `dependencies`,
 * `optionalDependencies` and `peerDependencies`. Edges come from the installed manifest when
 * there is one, else from the lockfile entry (platform packages that aren't installed here).
 * The lockfile's own `dev` flags are not used. Installed optional peers count as production
 * (a package may load them at run time), which is stricter than npm's view.
 */
export function productionTree(packages, rootManifest, manifests, problems) {
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
    const m = manifests.get(path) ?? packages[path] ?? {};
    const optional = new Set([...keys(m.optionalDependencies), ...keys(m.peerDependencies)]);
    visit(
      path,
      keys(m.dependencies).filter((d) => !optional.has(d)),
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

// Static `import … from "x"`, `export … from "x"`, side-effect `import "x"` and dynamic
// `import("x")`. Over-matching (an import-like string in a comment) only adds a false alarm.
const IMPORT_RE = /\b(?:from|import)\s*\(?\s*["']([^"']+)["']/g;

/**
 * Every bare import in shipped sources must name a package in the root manifest's
 * `dependencies`, so nothing from the test/build-only tree can be bundled.
 */
export function checkImports(sources, rootManifest, problems) {
  const deps = new Set(keys(rootManifest.dependencies));
  for (const { file, text } of sources) {
    for (const m of text.matchAll(IMPORT_RE)) {
      const root = bareRoot(m[1]);
      if (root !== null && !deps.has(root)) {
        problems.push(`${file}: imports ${m[1]}, which is not in package.json dependencies`);
      }
    }
  }
}

/**
 * Checks the whole tree. `manifests` maps lockfile paths to installed `package.json`
 * contents (missing key: not installed). Returns `{ problems, production, installed }`.
 */
export function check({ lock, rootManifest, manifests, sources }) {
  const problems = [];
  const packages = lock !== null && typeof lock === "object" ? (lock.packages ?? {}) : {};
  if (keys(packages).length === 0) problems.push("package-lock.json: no packages");
  const production = productionTree(packages, rootManifest, manifests, problems);
  let installed = 0;
  for (const [path, pkg] of Object.entries(packages)) {
    if (path === "") continue;
    if (pkg === null || typeof pkg !== "object") {
      problems.push(`${path}: lockfile entry is not an object`);
      continue;
    }
    const name = nameOf(path);
    const prod = production.has(path);
    const tree = prod ? "production" : "test/build";
    if (!licenseOk(pkg.license, name, prod)) {
      problems.push(`${path}: lockfile says ${pkg.license ?? "no license"} (${tree} tree)`);
    }
    const manifest = manifests.get(path);
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
  return { problems, production: production.size, installed };
}
