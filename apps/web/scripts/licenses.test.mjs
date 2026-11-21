// Tests for licenses-lib.mjs (spec: specs/tooling/licenses.md). Run: node --test scripts/
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { bareRoot, check, licenseOk, nameOf, resolve } from "./licenses-lib.mjs";

// A small tree: app depends on `ui`, which depends on `util` and has an optional peer `peer`.
// `tool` is a devDependency with its own nested `util`.
function tree(overrides = {}) {
  const packages = {
    "": { name: "app" },
    "node_modules/ui": { license: "MIT", dependencies: { util: "1" } },
    "node_modules/util": { license: "MIT" },
    "node_modules/tool": { license: "MIT", dev: true, dependencies: { util: "2" } },
    "node_modules/tool/node_modules/util": { license: "MIT", dev: true },
    ...overrides.packages,
  };
  const rootManifest = overrides.rootManifest ?? {
    dependencies: { ui: "1" },
    devDependencies: { tool: "1" },
  };
  const manifests = new Map();
  for (const [path, pkg] of Object.entries(packages)) {
    if (path !== "" && pkg !== null && !pkg.optional) manifests.set(path, { ...pkg });
  }
  for (const [path, m] of Object.entries(overrides.manifests ?? {})) {
    if (m === undefined) manifests.delete(path);
    else manifests.set(path, m);
  }
  const sources = overrides.sources ?? [{ file: "src/main.tsx", text: 'import "ui";' }];
  return { lock: { packages }, rootManifest, manifests, sources };
}

const problems = (overrides) => check(tree(overrides)).problems;

test("the clean tree passes and counts production packages", () => {
  const r = check(tree());
  assert.deepEqual(r.problems, []);
  assert.equal(r.production, 2);
  assert.equal(r.installed, 4);
});

test("every strict license passes in production", () => {
  for (const id of [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Zlib",
    "ISC",
    "Unicode-3.0",
  ]) {
    assert.equal(licenseOk(id, "x", true), true, id);
  }
});

test("every tool-only license passes in the test/build tree and fails in production", () => {
  for (const id of ["MPL-2.0", "BlueOak-1.0.0", "CC0-1.0", "0BSD", "NCSA"]) {
    assert.equal(licenseOk(id, "x", false), true, id);
    assert.equal(licenseOk(id, "x", true), false, id);
  }
});

test("CC-BY-4.0 passes only for listed data packages, and never in production", () => {
  assert.equal(licenseOk("CC-BY-4.0", "caniuse-lite", false), true);
  assert.equal(licenseOk("CC-BY-4.0", "caniuse-lite", true), false);
  assert.equal(licenseOk("CC-BY-4.0", "some-code", false), false);
});

test("unlisted licenses, compound expressions and non-strings fail everywhere", () => {
  for (const expr of [
    "GPL-3.0",
    "LGPL-2.1",
    "MPL-1.1",
    "mit",
    "",
    "MIT AND GPL-3.0",
    "(MIT OR GPL-3.0)",
    "Apache-2.0 WITH LLVM-exception",
    undefined,
    null,
    42,
    ["MIT"],
    { type: "MIT" },
  ]) {
    assert.equal(licenseOk(expr, "x", false), false, String(expr));
  }
});

test("OR passes if one option is allowed for that tree", () => {
  assert.equal(licenseOk("GPL-3.0 OR MIT", "x", true), true);
  assert.equal(licenseOk("GPL-3.0 OR MPL-2.0", "x", false), true);
  assert.equal(licenseOk("GPL-3.0 OR MPL-2.0", "x", true), false);
});

test("an MPL-2.0 production package fails (lockfile and installed manifest)", () => {
  const p = problems({ packages: { "node_modules/util": { license: "MPL-2.0" } } });
  assert.deepEqual(p, [
    "node_modules/util: lockfile says MPL-2.0 (production tree)",
    "node_modules/util: package.json says MPL-2.0 (production tree)",
  ]);
});

test("an MPL-2.0 test/build package passes; GPL there fails", () => {
  assert.deepEqual(
    problems({ packages: { "node_modules/tool": { license: "MPL-2.0", dev: true } } }),
    [],
  );
  assert.equal(
    problems({ packages: { "node_modules/tool": { license: "GPL-3.0", dev: true } } }).length,
    2,
  );
});

test("the installed manifest is checked even when the lockfile is clean", () => {
  const p = problems({ manifests: { "node_modules/util": { license: "BlueOak-1.0.0" } } });
  assert.deepEqual(p, ["node_modules/util: package.json says BlueOak-1.0.0 (production tree)"]);
});

test("a lockfile that marks a production package dev doesn't move it to the tool tree", () => {
  const p = problems({ packages: { "node_modules/util": { license: "MPL-2.0", dev: true } } });
  assert.equal(p.length, 2);
  assert.match(p[0], /production tree/);
});

test("edges come from the installed manifest, not just the lockfile", () => {
  // The lockfile hides ui → extra, but the installed ui/package.json declares it.
  const p = problems({
    packages: { "node_modules/extra": { license: "MPL-2.0", dev: true } },
    manifests: { "node_modules/ui": { license: "MIT", dependencies: { util: "1", extra: "1" } } },
  });
  assert.equal(p.length, 2);
  assert.match(p[0], /^node_modules\/extra: .*production tree/);
});

test("lockfile edges are used for packages that aren't installed", () => {
  const p = problems({
    packages: {
      "node_modules/ui": {
        license: "MIT",
        dependencies: { util: "1" },
        optionalDependencies: { plat: "1" },
      },
      "node_modules/plat": { license: "MIT", optional: true, dependencies: { deep: "1" } },
      "node_modules/deep": { license: "MPL-2.0", optional: true },
    },
  });
  assert.deepEqual(p, ["node_modules/deep: lockfile says MPL-2.0 (production tree)"]);
});

test("optional and peer dependencies that are present count as production", () => {
  for (const kind of ["optionalDependencies", "peerDependencies"]) {
    const ui = { license: "MIT", dependencies: { util: "1" }, [kind]: { peer: "1" } };
    const p = problems({
      packages: { "node_modules/ui": ui, "node_modules/peer": { license: "0BSD", dev: true } },
    });
    assert.equal(p.length, 2, kind);
    assert.match(p[0], /^node_modules\/peer: .*production tree/, kind);
  }
});

test("missing optional and peer dependencies are fine; a missing required one is reported", () => {
  const ui = {
    license: "MIT",
    dependencies: { util: "1", gone: "1" },
    optionalDependencies: { maybe: "1" },
    peerDependencies: { host: "1" },
  };
  assert.deepEqual(problems({ packages: { "node_modules/ui": ui } }), [
    "node_modules/ui: dependency gone not in lockfile",
  ]);
  // A dependency listed in both dependencies and optionalDependencies is optional (npm rule).
  const both = { ...ui, dependencies: { util: "1" }, optionalDependencies: { maybe: "1" } };
  both.dependencies.maybe = "1";
  assert.deepEqual(problems({ packages: { "node_modules/ui": both } }), []);
});

test("nested copies resolve to the nearest node_modules", () => {
  const packages = tree().lock.packages;
  assert.equal(
    resolve(packages, "node_modules/tool", "util"),
    "node_modules/tool/node_modules/util",
  );
  assert.equal(resolve(packages, "node_modules/ui", "util"), "node_modules/util");
  assert.equal(resolve(packages, "", "util"), "node_modules/util");
  assert.equal(resolve(packages, "node_modules/tool/node_modules/util", "ui"), "node_modules/ui");
  assert.equal(resolve(packages, "", "nope"), null);
  // The root copy of util is production, the nested one under the dev tool is not.
  const p = problems({
    packages: { "node_modules/tool/node_modules/util": { license: "MPL-2.0", dev: true } },
  });
  assert.deepEqual(p, []);
});

test("a production package nested under another resolves to its own copy", () => {
  const p = problems({
    packages: {
      "node_modules/ui": { license: "MIT", dependencies: { util: "1" } },
      "node_modules/ui/node_modules/util": { license: "CC0-1.0" },
    },
  });
  assert.equal(p.length, 2);
  assert.match(p[0], /^node_modules\/ui\/node_modules\/util: .*production tree/);
});

test("scoped names resolve and CC-BY data is matched by package name", () => {
  assert.equal(nameOf("node_modules/a/node_modules/@s/b"), "@s/b");
  assert.equal(nameOf("node_modules/caniuse-lite"), "caniuse-lite");
  const ok = problems({
    packages: {
      "node_modules/tool/node_modules/caniuse-lite": { license: "CC-BY-4.0", dev: true },
    },
  });
  assert.deepEqual(ok, []);
  const bad = problems({
    packages: { "node_modules/@x/caniuse-lite": { license: "CC-BY-4.0", dev: true } },
  });
  assert.equal(bad.length, 2);
});

test("a package that should be installed but isn't is reported; optional ones aren't", () => {
  assert.deepEqual(problems({ manifests: { "node_modules/util": undefined } }), [
    "node_modules/util: not installed (run npm ci first)",
  ]);
  for (const flag of ["optional", "devOptional", "peer", "extraneous"]) {
    const p = problems({
      packages: { "node_modules/x": { license: "MIT", dev: true, [flag]: true } },
      manifests: { "node_modules/x": undefined },
    });
    assert.deepEqual(p, [], flag);
  }
});

test("hostile lockfile shapes are reported, not thrown", () => {
  assert.deepEqual(check({ ...tree(), lock: null }).problems.slice(0, 1), [
    "package-lock.json: no packages",
  ]);
  const p = problems({ packages: { "node_modules/odd": null } });
  assert.ok(p.includes("node_modules/odd: lockfile entry is not an object"), p.join("\n"));
  const q = problems({ manifests: { "node_modules/ui": null } });
  assert.deepEqual(q, ["node_modules/ui: package.json says no license (production tree)"]);
});

test("a dependency cycle terminates", () => {
  const p = problems({
    packages: {
      "node_modules/ui": { license: "MIT", dependencies: { util: "1" } },
      "node_modules/util": { license: "MIT", dependencies: { ui: "1" } },
    },
  });
  assert.deepEqual(p, []);
});

test("bare import roots", () => {
  assert.equal(bareRoot("react"), "react");
  assert.equal(bareRoot("react-dom/client"), "react-dom");
  assert.equal(bareRoot("@mui/material/Box"), "@mui/material");
  assert.equal(bareRoot("./App"), null);
  assert.equal(bareRoot("../x"), null);
  assert.equal(bareRoot("/src/main.tsx"), null);
  assert.equal(bareRoot("node:fs"), null);
  assert.equal(bareRoot("https://cdn.example/x.js"), null);
});

test("shipped sources may import only package.json dependencies", () => {
  const src = (text) => [{ file: "src/a.ts", text }];
  for (const text of [
    'import x from "tool";',
    "import 'tool/sub';",
    'export { y } from "tool";',
    'const m = await import("tool");',
    'import {\n  a,\n} from "tool"',
  ]) {
    assert.equal(problems({ sources: src(text) }).length, 1, text);
  }
  assert.deepEqual(problems({ sources: src('import a from "ui/x"; import "./b";') }), []);
});

test("imports from devDependencies of the root are reported with the file name", () => {
  assert.deepEqual(problems({ sources: [{ file: "src/t.tsx", text: 'import "tool"' }] }), [
    "src/t.tsx: imports tool, which is not in package.json dependencies",
  ]);
});

test("an entry without dev is production even when nothing reaches it (owner rule, #50)", () => {
  const p = problems({
    packages: { "node_modules/stray": { license: "MPL-2.0", extraneous: true } },
  });
  assert.deepEqual(p, [
    "node_modules/stray: lockfile says MPL-2.0 (production tree)",
    "node_modules/stray: package.json says MPL-2.0 (production tree)",
  ]);
  assert.equal(check(tree()).production, 2);
  assert.equal(
    check(tree({ packages: { "node_modules/stray": { license: "MIT", extraneous: true } } }))
      .production,
    3,
  );
});

test("resolution walks every level of a deep path", () => {
  const packages = {
    "": {},
    "node_modules/a": {},
    "node_modules/a/node_modules/b": {},
    "node_modules/a/node_modules/b/node_modules/c": {},
    "node_modules/a/node_modules/b/node_modules/e": {},
    "node_modules/a/node_modules/d": {},
    "node_modules/d": {},
  };
  const from = "node_modules/a/node_modules/b/node_modules/c";
  assert.equal(resolve(packages, from, "d"), "node_modules/a/node_modules/d");
  assert.equal(resolve(packages, from, "e"), "node_modules/a/node_modules/b/node_modules/e");
  assert.equal(resolve(packages, from, "b"), "node_modules/a/node_modules/b");
  assert.equal(resolve(packages, "node_modules/a/node_modules/b", "c"), from);
});

test("an optional dependency three levels deep stays in the production tree", () => {
  const p = problems({
    packages: {
      "node_modules/ui": { license: "MIT", dependencies: { util: "1", mid: "1" } },
      "node_modules/ui/node_modules/mid": {
        license: "MIT",
        dev: true,
        dependencies: { leaf: "1" },
      },
      "node_modules/ui/node_modules/mid/node_modules/leaf": {
        license: "MIT",
        dev: true,
        optionalDependencies: { deep: "1" },
      },
      "node_modules/ui/node_modules/deep": { license: "NCSA", dev: true },
    },
  });
  assert.equal(p.length, 2, p.join("\n"));
  assert.match(p[0], /^node_modules\/ui\/node_modules\/deep: .*production tree/);
});

test("root optionalDependencies are production; they may not be imported", () => {
  const rootManifest = { dependencies: { ui: "1" }, optionalDependencies: { opt: "1" } };
  const p = problems({
    rootManifest,
    packages: { "node_modules/opt": { license: "0BSD", dev: true } },
  });
  assert.equal(p.length, 2);
  assert.match(p[0], /^node_modules\/opt: .*production tree/);
  const q = problems({
    rootManifest: { ...rootManifest, peerDependencies: { peer: "1" } },
    packages: { "node_modules/opt": { license: "MIT" } },
    sources: [{ file: "src/a.ts", text: 'import "opt"; import "peer";' }],
  });
  assert.equal(q.length, 2, q.join("\n"));
});

test("edges are the union of the lockfile entry and the installed manifest", () => {
  // The installed manifest drops ui → extra; the lockfile still declares it.
  const p = problems({
    packages: {
      "node_modules/ui": { license: "MIT", dependencies: { util: "1", extra: "1" } },
      "node_modules/extra": { license: "MPL-2.0", dev: true },
    },
    manifests: { "node_modules/ui": { license: "MIT", dependencies: { util: "1" } } },
  });
  assert.equal(p.length, 2);
  assert.match(p[0], /^node_modules\/extra: .*production tree/);
});

test("an npm alias doesn't inherit the CC-BY data allowance", () => {
  const evil = { license: "CC-BY-4.0", dev: true, name: "evil-code" };
  assert.equal(problems({ packages: { "node_modules/caniuse-lite": evil } }).length, 2);
  const real = { license: "CC-BY-4.0", dev: true };
  const p = problems({
    packages: { "node_modules/caniuse-lite": real },
    manifests: { "node_modules/caniuse-lite": { ...real, name: "evil-code" } },
  });
  assert.equal(p.length, 2);
  const ok = problems({
    packages: { "node_modules/caniuse-lite": real },
    manifests: { "node_modules/caniuse-lite": { ...real, name: "caniuse-lite" } },
  });
  assert.deepEqual(ok, []);
});

test("paths into node_modules, template literals and CSS imports are scanned", () => {
  const src = (text) => [{ file: "src/a.css", text }];
  for (const text of [
    'import x from "../node_modules/tool/index.js";',
    'import x from "/node_modules/tool/index.js";',
    'import x from "/@fs/home/u/app/node_modules/ui/x.js";',
    'import x from "../node_modules/ui/x.js";',
    "const m = await import(`tool`);",
    '@import "tool/x.css";',
    "@import url(tool/x.css);",
    '@import url("tool/x.css");',
  ]) {
    assert.equal(problems({ sources: src(text) }).length, 1, text);
  }
  assert.deepEqual(problems({ sources: src('@import url("./local.css");') }), []);
});

test("a hostile root manifest is reported, not thrown", () => {
  for (const rootManifest of [null, 7, "x"]) {
    const p = check({ ...tree(), rootManifest }).problems;
    assert.equal(p[0], "package.json: not an object", String(rootManifest));
  }
});

// Integration: the real script over a fixture app directory.
test("check-licenses.mjs reads nested sources, manifests and the bundle guards", () => {
  const scripts = fileURLToPath(new URL(".", import.meta.url));
  const base = join(scripts, "..", "node_modules", ".cache");
  mkdirSync(base, { recursive: true });
  const dir = mkdtempSync(join(base, "licenses-test-"));
  const run = () =>
    spawnSync(process.execPath, [join(scripts, "check-licenses.mjs"), dir], { encoding: "utf8" });
  const write = (rel, text) => {
    mkdirSync(join(dir, rel, ".."), { recursive: true });
    writeFileSync(join(dir, rel), text);
  };
  try {
    const lock = {
      packages: {
        "": {},
        "node_modules/ui": { license: "MIT" },
        "node_modules/tool": { license: "MPL-2.0", dev: true },
      },
    };
    write("package-lock.json", JSON.stringify(lock));
    write(
      "package.json",
      JSON.stringify({ dependencies: { ui: "1" }, devDependencies: { tool: "1" } }),
    );
    write("node_modules/ui/package.json", JSON.stringify({ license: "MIT" }));
    write("node_modules/tool/package.json", JSON.stringify({ license: "MPL-2.0" }));
    write("index.html", '<script type="module" src="/src/main.tsx"></script>');
    write("vite.config.ts", "export default {};");
    write("src/main.tsx", 'import "ui";');
    write("src/deep/er/x.cts", 'import "ui";');
    let r = run();
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stderr, /2 packages, 1 production, 2 installed, 3 source files/);

    write("src/deep/er/x.cts", 'import "tool";');
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/deep\/er\/x\.cts: imports tool/);
    write("src/deep/er/x.cts", "");

    write("node_modules/ui/package.json", JSON.stringify({ license: "MPL-2.0" }));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /node_modules\/ui: package\.json says MPL-2\.0 \(production tree\)/);
    write("node_modules/ui/package.json", JSON.stringify({ license: "MIT" }));

    write("public/x.js", "");
    write("vite.config.ts", "export default { resolve: { alias: {} }, publicDir: 'p' };");
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /public\/: Vite copies it/);
    assert.match(r.stderr, /uses alias/);
    assert.match(r.stderr, /uses publicDir/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
