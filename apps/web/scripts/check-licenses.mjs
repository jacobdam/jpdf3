// CONSTITUTION 10: npm license check (spec: specs/tooling/licenses.md).
// The production tree (every lockfile entry without `dev`, plus everything reachable from
// package.json `dependencies`) must use the strict list;
// the rest (test and build tools) may also use the tool-only licenses. Licenses are read from
// the lockfile and from each installed node_modules/<pkg>/package.json (from the
// integrity-checked tarball, so an edited lockfile can't hide a package). Shipped sources may
// import only `dependencies` or files inside src/. Never runs package code. Third-party code bundled *inside* a
// package is not visible here; see docs/adr/0002 item 5.
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { check, sourceExtensionOk } from "./licenses-lib.mjs";

// The app directory: apps/web, or the first argument (used by the integration test).
const root = process.argv[2]
  ? pathToFileURL(`${process.argv[2].replace(/\/$/, "")}/`)
  : new URL("../", import.meta.url);
const readJson = (url) => JSON.parse(readFileSync(url, "utf8"));

const lock = readJson(new URL("package-lock.json", root));
const rootManifest = readJson(new URL("package.json", root));

const manifests = new Map();
for (const path of Object.keys(lock.packages ?? {})) {
  if (path === "") continue;
  const url = new URL(`${path}/package.json`, root);
  if (existsSync(url)) manifests.set(path, readJson(url));
}

// Everything Vite bundles: src/ (recursively) and index.html.
const sourceProblems = [];
const sources = [{ file: "index.html", text: readFileSync(new URL("index.html", root), "utf8") }];
const appDir = fileURLToPath(root);
const realSrc = realpathSync(join(appDir, "src"));
for (const entry of readdirSync(new URL("src/", root), { recursive: true, withFileTypes: true })) {
  const full = join(entry.parentPath, entry.name);
  const rel = relative(appDir, full);
  // Vite resolves a symlinked module's imports from its real location, which this scan
  // would get wrong, so symlinks in src/ are refused (and so is anything a symlinked
  // directory exposes, checked by real path).
  if (entry.isSymbolicLink()) {
    sourceProblems.push(`${rel}: symlinks in src/ are not allowed`);
    continue;
  }
  if (entry.isDirectory()) {
    // A directory named like a source file (`dir.ts`) would confuse an import of it.
    if (sourceExtensionOk(entry.name)) sourceProblems.push(`${rel}: not a regular file`);
    continue;
  }
  if (!entry.isFile()) {
    sourceProblems.push(`${rel}: not a regular file`);
    continue;
  }
  // Fail closed: Vite bundles `./a.mod` or `./c.JS` as JavaScript and `.pcss` as CSS, and
  // the scan would never read them. Extend the scan before adding an extension.
  if (!sourceExtensionOk(entry.name)) {
    sourceProblems.push(
      `${rel}: only lower-case .js/.jsx/.mjs/.cjs/.ts/.tsx/.mts/.cts/.css/.html files are allowed in src/`,
    );
    continue;
  }
  if (!`${realpathSync(full)}${sep}`.startsWith(`${realSrc}${sep}`)) {
    sourceProblems.push(`${rel}: real path is outside src/`);
    continue;
  }
  sources.push({ file: rel, text: readFileSync(full, "utf8") });
}

const { problems, production, installed } = check({ lock, rootManifest, manifests, sources });
problems.push(...sourceProblems);

// Ways into the bundle that the import scan can't see: Vite copies `public/` verbatim;
// aliases, another root, public directory or input, and plugin hooks that resolve, load or
// transform modules remap what is bundled. None is used; refuse them until #99 checks the
// real module graph. This guard is a text match, so it is advisory (specs/tooling/licenses.md).
if (existsSync(new URL("public", root))) {
  problems.push(
    "public/: Vite copies it into the bundle unchecked; extend the license check first",
  );
}
for (const name of readdirSync(root)) {
  if (/^vite\.config\./.test(name) && name !== "vite.config.ts") {
    problems.push(`${name}: Vite would load it instead of the checked vite.config.ts`);
  }
}
const viteConfig = readFileSync(new URL("vite.config.ts", root), "utf8");
// Plain, quoted, computed (`["alias"]:`) and shorthand (`{ alias }`) keys.
// Also library entries, emitted chunks, preprocessor injections, and the hooks that edit
// the config or the options (`config`, `configResolved`, `options`) or inject text.
const GUARDED = [
  "alias",
  "publicDir",
  "root",
  "input",
  "resolveId",
  "resolveDynamicImport",
  "load",
  "transform",
  "lib",
  "entry",
  "emitFile",
  "additionalData",
  "config",
  "configResolved",
  "options",
  "banner",
  "footer",
  "intro",
  "outro",
  "renderChunk",
  "generateBundle",
];
for (const m of viteConfig.matchAll(
  new RegExp(String.raw`\b(${GUARDED.join("|")})\b["'\]]{0,2}\s*[:(,}]`, "g"),
)) {
  problems.push(
    `vite.config.ts: uses ${m[1]}, which the import scan can't follow; extend the check first`,
  );
}
// Plugins defined in another module would hide their hooks from the text match above.
for (const m of viteConfig.matchAll(/\b(?:from|import)\s*\(?\s*["'`]([^"'`]+)["'`]/g)) {
  if (m[1] !== "vite" && m[1] !== "./csp") {
    problems.push(`vite.config.ts: imports ${m[1]}; only vite and ./csp are checked`);
  }
}
// The root `imports` field maps `#x` specifiers anywhere, past the import scan.
if (rootManifest !== null && typeof rootManifest === "object" && "imports" in rootManifest) {
  problems.push("package.json: the imports field (#subpaths) is not checked");
}
if (problems.length > 0) {
  console.error(`licenses: ${problems.length} problem(s) (CONSTITUTION 10):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.error(
  `licenses: ok (${Object.keys(lock.packages).length - 1} packages, ${production} production, ` +
    `${installed} installed, ${sources.length} source files)`,
);
