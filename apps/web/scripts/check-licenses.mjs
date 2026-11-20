// CONSTITUTION 10: every npm package in the lockfile (shipped or build-only) must carry an
// allowlisted license. Reads package-lock.json only; never runs package code.
import { readFileSync } from "node:fs";

const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "Zlib",
  "ISC",
  "Unicode-3.0",
]);
const lock = JSON.parse(readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"));

// A plain id, or "A OR B" (we may pick any allowed option). AND, WITH and parentheses are rejected.
function allowed(expr) {
  if (typeof expr !== "string" || /[()]|\b(AND|WITH)\b/.test(expr)) return false;
  return expr.split(" OR ").some((id) => ALLOWED.has(id.trim()));
}

const bad = [];
for (const [path, pkg] of Object.entries(lock.packages ?? {})) {
  if (path === "") continue;
  if (!allowed(pkg.license)) bad.push(`${path}: ${pkg.license ?? "no license field"}`);
}
if (bad.length > 0) {
  console.error(`licenses: ${bad.length} package(s) outside the allowlist (CONSTITUTION 10):`);
  for (const b of bad) console.error(`  ${b}`);
  process.exit(1);
}
console.error(`licenses: ok (${Object.keys(lock.packages).length - 1} packages)`);
