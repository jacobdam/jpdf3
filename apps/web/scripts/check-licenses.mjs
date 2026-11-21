// CONSTITUTION 10: npm license check (spec: specs/tooling/licenses.md).
// The production tree (reached from package.json `dependencies`) must use the strict list;
// the rest (test and build tools) may also use the tool-only licenses. Licenses are read from
// the lockfile and from each installed node_modules/<pkg>/package.json (from the
// integrity-checked tarball, so an edited lockfile can't hide a package). Shipped sources may
// import only `dependencies`. Never runs package code. Third-party code bundled *inside* a
// package is not visible here; see docs/adr/0002 item 5.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { check } from "./licenses-lib.mjs";

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
const sources = [{ file: "index.html", text: readFileSync(new URL("index.html", root), "utf8") }];
for (const entry of readdirSync(new URL("src/", root), { recursive: true, withFileTypes: true })) {
  if (!(entry.isFile() || entry.isSymbolicLink())) continue;
  if (!/\.([cm]?[jt]sx?|css|html)$/.test(entry.name)) continue;
  const full = join(entry.parentPath, entry.name);
  sources.push({ file: relative(fileURLToPath(root), full), text: readFileSync(full, "utf8") });
}

const { problems, production, installed } = check({ lock, rootManifest, manifests, sources });

// Ways into the bundle that the import scan can't see: Vite copies `public/` verbatim, and
// aliases or another public directory remap imports. None is used; refuse them until the
// check covers them (specs/tooling/licenses.md).
if (existsSync(new URL("public", root))) {
  problems.push(
    "public/: Vite copies it into the bundle unchecked; extend the license check first",
  );
}
const viteConfig = readFileSync(new URL("vite.config.ts", root), "utf8");
for (const key of ["alias", "publicDir"]) {
  if (viteConfig.includes(key)) {
    problems.push(
      `vite.config.ts: uses ${key}, which the import scan can't follow; extend the check first`,
    );
  }
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
