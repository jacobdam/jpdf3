// CONSTITUTION 10: every npm package (shipped or build-only) must carry an allowlisted
// license. Checks both the lockfile's `license` field and, for installed packages, the
// `license` in node_modules/<pkg>/package.json (from the integrity-checked tarball, so an
// edited lockfile can't hide a package). Never runs package code. Third-party code bundled
// *inside* a package is not visible here; see docs/adr/0002 item 5.
import { existsSync, readFileSync } from "node:fs";

const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "Zlib",
  "ISC",
  "Unicode-3.0",
]);
const root = new URL("../", import.meta.url);
const lock = JSON.parse(readFileSync(new URL("package-lock.json", root), "utf8"));

// A plain id, or "A OR B" (we may pick any allowed option). AND, WITH and parentheses are rejected.
function allowed(expr) {
  if (typeof expr !== "string" || /[()]|\b(AND|WITH)\b/.test(expr)) return false;
  return expr.split(" OR ").some((id) => ALLOWED.has(id.trim()));
}

const bad = [];
let installed = 0;
for (const [path, pkg] of Object.entries(lock.packages ?? {})) {
  if (path === "") continue;
  if (!allowed(pkg.license)) bad.push(`${path}: lockfile says ${pkg.license ?? "no license"}`);
  const manifest = new URL(`${path}/package.json`, root);
  if (existsSync(manifest)) {
    installed += 1;
    const real = JSON.parse(readFileSync(manifest, "utf8")).license;
    if (!allowed(real)) bad.push(`${path}: package.json says ${real ?? "no license"}`);
    // Optional and unused optional-peer (extraneous) entries are never installed.
  } else if (!pkg.optional && !pkg.devOptional && !pkg.peer && !pkg.extraneous) {
    bad.push(`${path}: not installed (run npm ci first)`);
  }
}
if (bad.length > 0) {
  console.error(`licenses: ${bad.length} problem(s) (CONSTITUTION 10):`);
  for (const b of bad) console.error(`  ${b}`);
  process.exit(1);
}
console.error(
  `licenses: ok (${Object.keys(lock.packages).length - 1} packages, ${installed} installed)`,
);
