// Tests for licenses-lib.mjs (spec: specs/tooling/licenses.md). Run: node --test scripts/
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  bareRoot,
  check,
  licenseOk,
  nameOf,
  registryTarball,
  resolve,
  sourceExtensionOk,
  srcsetUrls,
} from "./licenses-lib.mjs";

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

// A lockfile entry resolved from the npm registry (the CC-BY allowance requires it).
const fromRegistry = (name, extra = {}) => ({
  version: "1.0.1",
  resolved: registryTarball(name, "1.0.1"),
  ...extra,
});

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
      "node_modules/tool/node_modules/caniuse-lite": fromRegistry("caniuse-lite", {
        license: "CC-BY-4.0",
        dev: true,
      }),
    },
  });
  assert.deepEqual(ok, []);
  const bad = problems({
    packages: {
      "node_modules/@x/caniuse-lite": fromRegistry("@x/caniuse-lite", {
        license: "CC-BY-4.0",
        dev: true,
      }),
    },
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
  // Only the lockfile names the real package; the installed manifest looks right.
  const evil = fromRegistry("caniuse-lite", { license: "CC-BY-4.0", dev: true, name: "evil-code" });
  const lockOnly = problems({
    packages: { "node_modules/caniuse-lite": evil },
    manifests: { "node_modules/caniuse-lite": { license: "CC-BY-4.0", name: "caniuse-lite" } },
  });
  assert.equal(lockOnly.length, 2, lockOnly.join("\n"));
  const real = fromRegistry("caniuse-lite", { license: "CC-BY-4.0", dev: true });
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

test("the CC-BY data allowance needs the registry tarball for that name and version", () => {
  assert.equal(
    registryTarball("caniuse-lite", "1.0.1"),
    "https://registry.npmjs.org/caniuse-lite/-/caniuse-lite-1.0.1.tgz",
  );
  assert.equal(registryTarball("@s/b", "2.0.0"), "https://registry.npmjs.org/@s/b/-/b-2.0.0.tgz");
  assert.equal(registryTarball("x", ""), null);
  assert.equal(registryTarball("x", undefined), null);
  assert.equal(registryTarball("x", "1.0"), null);
  assert.equal(registryTarball("x", "1.0.0/../y"), null);
  assert.equal(
    registryTarball("x", "1.0.0-rc.1+b.2"),
    "https://registry.npmjs.org/x/-/x-1.0.0-rc.1+b.2.tgz",
  );
  // The installed manifest must carry the same version.
  const mismatch = problems({
    packages: {
      "node_modules/caniuse-lite": fromRegistry("caniuse-lite", {
        license: "CC-BY-4.0",
        dev: true,
      }),
    },
    manifests: { "node_modules/caniuse-lite": { license: "CC-BY-4.0", version: "2.0.0" } },
  });
  assert.equal(mismatch.length, 2);
  const real = fromRegistry("caniuse-lite", { license: "CC-BY-4.0", dev: true });
  const reg = "https://registry.npmjs.org/caniuse-lite/-/caniuse-lite-1.0.1.tgz";
  for (const fork of [
    { resolved: "git+ssh://git@github.com/evil/caniuse-lite.git#abc" },
    { resolved: "https://evil.example/caniuse-lite-1.0.1.tgz" },
    { resolved: `${reg}?x` },
    { resolved: "https://registry.npmjs.org/evil/-/evil-1.0.1.tgz" },
    { resolved: undefined },
    { version: "9.9.9" },
    { version: undefined },
    // A traversal version that WHATWG URL parsing would normalize to another package.
    {
      version: "x/../../../evil/-/evil-1.0.0",
      resolved:
        "https://registry.npmjs.org/caniuse-lite/-/caniuse-lite-x/../../../evil/-/evil-1.0.0.tgz",
    },
  ]) {
    const pkg = { ...real, ...fork };
    const p = problems({
      packages: { "node_modules/caniuse-lite": pkg },
      manifests: { "node_modules/caniuse-lite": { license: "CC-BY-4.0" } },
    });
    // The lockfile and the installed manifest both fail without the allowance.
    assert.equal(p.length, 2, JSON.stringify(fork));
  }
  // The registry pin is about the CC-BY allowance only; other licenses don't need it.
  assert.deepEqual(problems({ packages: { "node_modules/x": { license: "MIT", dev: true } } }), []);
});

test("paths into node_modules, template literals and CSS imports are scanned", () => {
  const src = (text) => [{ file: "src/a.css", text }];
  for (const text of [
    'import x from "../node_modules/tool/index.js";',
    'import x from "/node_modules/tool/index.js";',
    'import x from "/@fs/home/u/app/node_modules/ui/x.js";',
    'import x from "/@fs/home/u/vendor/x.js";',
    'import x from "/@id/tool";',
    'import x from "../vendor/x.ts";',
    'import x from "../../x.ts";',
    'import "/csp.ts";',
    'import "./../../e2e/helper.ts";',
    'const m = await import(/* chunk */ "tool");',
    'import x from /* c */ "tool";',
    'import x from // c\n "tool";',
    "a { background: url(tool/x.png); }",
    'import x from "#tool";',
    "const m = await import('#internal/x');",
    'import x from "../node_modules/ui/x.js";',
    "const m = await import(`tool`);",
    '@import "tool/x.css";',
    "@import url(tool/x.css);",
    '@import url("tool/x.css");',
  ]) {
    assert.equal(problems({ sources: src(text) }).length, 1, text);
  }
  assert.deepEqual(problems({ sources: src('@import url("./local.css");') }), []);
  // Relative and root-absolute paths inside src/ are fine; schemes and fragments are left to
  // the CSP and E2E checks.
  const ok = [
    { file: "src/deep/a.ts", text: 'import "../b"; import "./c"; import "/src/d.ts";' },
    { file: "src/a.css", text: "a { background: url(data:image/png;base64,AA); }" },
    { file: "index.html", text: '<script type="module" src="/src/main.tsx"></script>' },
    { file: "index.html", text: '<a href="#top">x</a><script src="./src/m.ts"></script>' },
  ];
  assert.deepEqual(problems({ sources: ok }), []);
});

test("import.meta (other than env) and package paths in any string are refused (#112)", () => {
  const one = (file, text) => {
    const p = problems({ sources: [{ file, text }] });
    assert.equal(p.length, 1, `${text}\n${p.join("\n")}`);
    return p[0];
  };
  // The audit's proofs: both rules fire.
  for (const text of [
    'const m = import.meta.glob("/node_modules/tool/index.js", { eager: true });',
    'const u = new URL("../node_modules/tool/x.js", import.meta.url);',
  ]) {
    const p = problems({ sources: [{ file: "src/main.tsx", text }] });
    assert.equal(p.length, 2, p.join("\n"));
    assert.match(p[0], /src\/main\.tsx: names a path into node_modules/);
    assert.match(p[1], /src\/main\.tsx: uses import\.meta other than/);
  }
  for (const text of [
    'const m = import.meta.glob("./x/*.ts");',
    "const m = import.meta.globEager('./x/*.ts');",
    'const u = new URL("./worker.ts", import.meta.url);',
    'const g = import.meta["glob"];',
    "const g = import . meta . glob;",
    "const g = import /* c */ .meta\n.glob;",
    "const g = import.meta?.glob;",
    "const m = import.meta;",
    "const e = import.meta.environment;",
  ]) {
    assert.match(one("src/a.ts", text), /uses import\.meta other than import\.meta\.env/, text);
  }
  for (const text of [
    "const mode = import.meta.env.MODE;",
    "const mode = import.meta /* c */ .env.DEV;",
    'const e = import.meta.env["BASE_URL"];',
  ]) {
    assert.deepEqual(problems({ sources: [{ file: "src/a.ts", text }] }), [], text);
  }
  // JS `new URL(x)` is not CSS `url(x)`.
  assert.deepEqual(problems({ sources: [{ file: "src/a.ts", text: "new URL(href);" }] }), []);
  // A path into node_modules anywhere in the text, not only in import forms.
  for (const [file, text] of [
    ["src/a.ts", 'const p = "/node_modules/tool/x.js";'],
    ["src/a.ts", "const p = `../NODE_MODULES/tool/x.js`;"],
    ["src/a.ts", 'fetch("/@fs/home/u/x.js");'],
    ["src/a.ts", "// see /@id/tool"],
    ["src/a.css", "a { --x: '/node_modules/tool/x.png'; }"],
    ["index.html", "<script>const p = '/node_modules/tool/x.js';</script>"],
  ]) {
    assert.match(one(file, text), /names a path into node_modules/, text);
  }
  // An import that names node_modules is reported once, by the import form.
  assert.match(
    one("src/a.ts", 'import x from "../node_modules/tool/index.js"; // node_modules'),
    /imports \.\.\/node_modules\/tool\/index\.js, a path into node_modules/,
  );
});

test("CSS forms are matched case-insensitively, and image-set strings are scanned", () => {
  for (const text of [
    "a { background: URL(tool/x.png); }",
    "a { background: Url('tool/x.png'); }",
    '@IMPORT "tool/x.css";',
    '@Import url("tool/x.css");',
    'a { background: image-set("tool/x.png" 1x); }',
    "a { background: -webkit-image-set('tool/x.png' 1x); }",
    'a { background: IMAGE-SET("./ok.png" 1x, "tool/x.png" 2x); }',
    'a { background: image-set("tool/x.png" type("image/png")); }',
    'a { background: image-set("./a.png" TYPE("image/png"), "tool/x.png"); }',
  ]) {
    assert.equal(problems({ sources: [{ file: "src/a.css", text }] }).length, 1, text);
  }
  const ok =
    'a { background: image-set("./a.png" 1x type("image/avif"), url(./b.png) 2x, "#f" 3x); }';
  assert.deepEqual(problems({ sources: [{ file: "src/a.css", text: ok }] }), []);
  // In JS, image-set strings are not imports (only CSS files get that scan).
  const js = "const s = 'image-set(\"x.png\" 1x)';";
  assert.deepEqual(problems({ sources: [{ file: "src/a.ts", text: js }] }), []);
});

test("specifiers that decode or resolve past the check are refused (review of #112)", () => {
  const bad = [
    // Vite bundles data: and file: modules: no scheme in import positions.
    ["src/a.ts", 'import "data:text/javascript;base64,aW1wb3J0ICd0b29sJw==";'],
    ["src/a.ts", `import "data:text/javascript,import 'tool'";`],
    ["src/a.ts", 'import "file:///ci/apps/web/node%5Fmodules/tool/index.js";'],
    ["src/a.ts", 'const m = await import("https://cdn.example/x.js");'],
    ["src/a.css", "@import \"data:text/css,@import 'tool/x.css';\";"],
    ["src/a.css", "@import url(data:text/css,x);"],
    ["src/a.css", "a { background: url(file:///x.png); }"],
    ["index.html", '<script type="module" src="javascript:alert(1)"></script>'],
    // A quote of the other kind doesn't end the string.
    ["src/a.ts", `import "./'/../../e2e/x.ts";`],
    ["src/a.css", `@import "./a.css'/../../../e2e/x.css";`],
    // Escapes, entities, percent-encoding and whitespace aren't decoded: refused.
    ["src/a.ts", String.raw`import "/src/\x2e\x2e/node\x5fmodules/tool/index.js";`],
    ["src/a.ts", String.raw`import "./../../e2e/x.ts";`],
    ["index.html", '<script type="module" src="/src/&#46;&#46;/node&#95;modules/tool/i.js">'],
    ["index.html", '<link rel="stylesheet" href="/src/%2e%2e/e2e/x.css">'],
    ["index.html", '<script type="module" src="/src/a.ts /../../e2e/x.ts"></script>'],
    ["index.html", `<script type="module" src='/src/a.ts"/../../../e2e/x.ts'></script>`],
    ["src/a.css", '@import url("./a.css /../../../e2e/x.css");'],
    ["src/a.css", "a { background: url(./a.png /../../../e2e/x.png); }"],
    // `.` and `..` segments in a package path.
    ["src/a.ts", 'import "ui/../tool/index.js";'],
    ["src/a.ts", 'import "ui/./x.js";'],
    // esbuild JSX pragmas.
    ["src/a.tsx", "/** @jsxImportSource tool */ export const A = () => <div />;"],
    ["src/a.tsx", "// @jsx h\nexport const A = 1;"],
    // HTML asset attributes Vite rewrites.
    ["index.html", '<img srcset="/src/a.png 1x, /e2e/x.png 2x">'],
    ["index.html", '<video poster="/e2e/x.png"></video>'],
    ["index.html", '<meta property="og:image" content="/e2e/x.png">'],
    ["index.html", '<meta property="og:image" content="e2e/x.png">'],
    ["index.html", '<img srcset="/src/a.png 1x,/e2e/x.png 2x">'],
    // Delta review: a line continuation inside the string.
    ["src/a.ts", 'import "./a\\\n/../../e2e/x.ts";'],
    // Vite globs `+` and `.concat` dynamic imports over every literal.
    ["src/a.ts", 'const m = import("./" + "../../e2e/" + x + ".ts");'],
    [
      "src/a.ts",
      String.raw`const m = import("./".concat("../../node\x5fmodules/tool/", x, ".js"));`,
    ],
    ["src/a.ts", "const m = import(name);"],
    ["src/a.ts", `const m = import(\`./\${x}.ts\`);`],
    ["index.html", '<script type="module">import("./" + x + ".js")</script>'],
    // A line comment ends at CR, U+2028 and U+2029 too.
    ["src/a.ts", 'import //\r"../../e2e/x.ts";'],
    ["src/a.ts", 'export * from //\r"../../e2e/x.ts";'],
    ["src/a.ts", 'import //\u2028"../../e2e/x.ts";'],
    ["src/a.ts", 'const m = import //\r(`./` + "../../e2e/" + x + ".ts");'],
    ["src/a.ts", 'const m = import //\u2029("./" + x);'],
    // Vite's srcset splitter reads a second URL where the HTML parser sees a descriptor.
    ["index.html", '<img srcset="/src/a.png /e2e/x.png 2x">'],
  ];
  for (const [file, text] of bad) {
    const p = problems({ sources: [{ file, text }] });
    assert.ok(p.length >= 1, `${file}: ${text}`);
  }
  const deps = { dependencies: { ui: "1", "@s/z": "1" } };
  const ok = [
    ["src/a.css", "a { background: url( ./a.png ); }"],
    ["src/a.css", "a { background: url(data:image/png;base64,AA==); }"],
    ["index.html", '<img src="https://example.org/x.png" srcset="/src/a.png 1x, /src/b.png 2x">'],
    ["index.html", '<meta name="viewport" content="width=device-width, initial-scale=1" />'],
    ["src/a.ts", 'import x from "./b.ts?raw"; import "ui/x.js"; import y from "@s/z";'],
    ["src/a.ts", 'const m = await import("./b.ts"); const n = import(/* c */ "ui", { with: {} });'],
    ["src/a.ts", 'type T = typeof import("./b.ts");'],
    ["index.html", '<img srcset="data:image/png;base64,AA== 1x, /src/a.png 2x">'],
    ["index.html", '<a href="mailto:a@example.org">mail</a>'],
    ["index.html", '<meta name="color-scheme" content="light dark" />'],
  ];
  for (const [file, text] of ok) {
    const r = check({ ...tree({ sources: [{ file, text }] }), rootManifest: deps });
    // @s/z isn't in the fixture lockfile; only the import scan is under test here.
    assert.deepEqual(
      r.problems.filter((x) => !x.includes("not in lockfile")),
      [],
      `${file}: ${text}`,
    );
  }
});

test("srcset URLs are split the way the HTML parser splits them", () => {
  assert.deepEqual(srcsetUrls("a.png 1x, b.png 2x"), ["a.png", "b.png"]);
  assert.deepEqual(srcsetUrls("a.png 1x,b.png 2x"), ["a.png", "b.png"]);
  assert.deepEqual(srcsetUrls("a.png, b.png"), ["a.png", "b.png"]);
  assert.deepEqual(srcsetUrls("data:image/png;base64,AA== 1x"), ["data:image/png;base64,AA=="]);
  assert.deepEqual(srcsetUrls(" ,, a.png  100w ,"), ["a.png"]);
  assert.deepEqual(srcsetUrls(""), []);
  assert.deepEqual(srcsetUrls("a.png 1.5x, b.png 100w 2h"), ["a.png", "b.png"]);
  assert.deepEqual(srcsetUrls("a.png b.png 2x").sort(), ["a.png", "b.png"]);
});

test("unquoted url() with long whitespace runs stays fast", () => {
  const start = performance.now();
  for (const file of ["src/a.css", "src/a.ts"]) {
    problems({ sources: [{ file, text: `url(${" ".repeat(20000)}x` }] });
  }
  assert.ok(performance.now() - start < 2000, "url() scan took too long");
});

test("comment gaps don't backtrack exponentially", () => {
  const start = performance.now();
  for (const [text, n] of [
    [`import${" /**/".repeat(60)} "tool";`, 1],
    [`import${" /**/".repeat(60)} x;`, 0],
    [`import${" /**/".repeat(60)}.meta.glob;`, 1],
  ]) {
    assert.equal(problems({ sources: [{ file: "src/a.ts", text }] }).length, n, text);
  }
  assert.ok(performance.now() - start < 2000, "comment gaps took too long");
});

test("only lower-case scanned source extensions are allowed", () => {
  for (const name of ["a.js", "a.jsx", "a.mjs", "a.cjs", "a.ts", "a.tsx", "a.mts", "a.cts"]) {
    assert.equal(sourceExtensionOk(name), true, name);
  }
  for (const name of ["a.css", "a.html", "a.d.ts", "dir/x.tsx"]) {
    assert.equal(sourceExtensionOk(name), true, name);
  }
  for (const name of [
    "a.mod",
    "c.JS",
    "c.Ts",
    "x.CSS",
    "x.pcss",
    "x.postcss",
    "x.sss",
    "x.scss",
    "x.less",
    "x.styl",
    "x.json",
    "x.svg",
    "x.vue",
    "x.wasm",
    "x.htm",
    "x.ts.bak",
    "README",
    ".DS_Store",
    "ts",
  ]) {
    assert.equal(sourceExtensionOk(name), false, name);
  }
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
  const top = mkdtempSync(join(base, "licenses-test-"));
  // The app sits one level down, so a PostCSS config in its parent can be tested.
  const dir = join(top, "app");
  mkdirSync(dir);
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
    write("csp.ts", 'export const CSP = ["require-trusted-types-for \'script\'"].join("; ");');
    write("tsconfig.json", '{ "compilerOptions": { "jsx": "react-jsx" } }');
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

    // specs/tooling/dashboard.md: the one allowed multi-page input; its second page is scanned.
    const input = 'input: { app: "index.html", dashboard: "dashboard/index.html" },';
    const viteWith = (inputs) =>
      `export default { build: { rollupOptions: { ${inputs} onwarn() {} } } };`;
    write("vite.config.ts", viteWith(input));
    write("dashboard/index.html", '<script type="module" src="/src/main.tsx"></script>');
    r = run();
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stderr, /4 source files/);
    write("dashboard/index.html", '<script type="module" src="../e2e/x.ts"></script>');
    r = run();
    assert.equal(r.status, 1);
    assert.match(
      r.stderr,
      /dashboard\/index\.html: imports \.\.\/e2e\/x\.ts, which is outside src\//,
    );
    write("dashboard/index.html", '<script type="module" src="/src/main.tsx"></script>');
    // Any other input is still refused: another entry, or a second input (even a copy).
    for (const other of [
      'input: { app: "index.html", dashboard: "e2e/x.html" },',
      `${input} input: "e2e/x.html",`,
      `${input} ${input}`,
    ]) {
      write("vite.config.ts", viteWith(other));
      r = run();
      assert.equal(r.status, 1, other);
      assert.match(r.stderr, /uses input,/, other);
    }
    write("vite.config.ts", "export default {};");
    rmSync(join(dir, "dashboard"), { recursive: true });

    write("node_modules/ui/package.json", JSON.stringify({ license: "MPL-2.0" }));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /node_modules\/ui: package\.json says MPL-2\.0 \(production tree\)/);
    write("node_modules/ui/package.json", JSON.stringify({ license: "MIT" }));

    symlinkSync(join(dir, "elsewhere.ts"), join(dir, "src", "linked.ts"));
    write("elsewhere.ts", 'import "tool";');
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/linked\.ts: symlinks in src\/ are not allowed/);
    rmSync(join(dir, "src", "linked.ts"));
    // A symlinked directory is refused too, and nothing under it is scanned as src/.
    write("other/x.ts", 'import "./vendor.ts";');
    symlinkSync(join(dir, "other"), join(dir, "src", "lib"));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/lib: symlinks in src\/ are not allowed/);
    assert.match(r.stderr, /src\/lib\/x\.ts: real path is outside src\//);
    rmSync(join(dir, "src", "lib"));

    mkdirSync(join(dir, "src", "dir.ts"));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/dir\.ts: not a regular file/);
    rmSync(join(dir, "src", "dir.ts"), { recursive: true });

    // #112: Vite bundles `./a.mod` and `./c.JS` as JS and `.pcss` as CSS; refuse them.
    write("src/main.tsx", 'import "ui"; import "./a.mod"; import "./c.JS"; import "./s.pcss";');
    write("src/a.mod", 'import "tool";');
    write("src/c.JS", 'import "tool";');
    write("src/s.pcss", '@import "tool/x.css";');
    write("src/deep/README", "");
    mkdirSync(join(dir, "src", "assets.d"));
    r = run();
    assert.equal(r.status, 1);
    for (const f of ["src/a.mod", "src/c.JS", "src/s.pcss", "src/deep/README"]) {
      assert.match(r.stderr, new RegExp(`${f.replace(/\./g, "\\.")}: only lower-case`), f);
    }
    // A plain directory is fine; it's not a source file.
    assert.doesNotMatch(r.stderr, /assets\.d/);
    assert.match(r.stderr, /4 problem\(s\)/);
    for (const f of ["a.mod", "c.JS", "s.pcss", "deep/README"]) rmSync(join(dir, "src", f));
    rmSync(join(dir, "src", "assets.d"), { recursive: true });
    write("src/main.tsx", 'import "ui";');
    // The audit's other proof, end to end.
    write("src/g.ts", 'import.meta.glob("/node_modules/tool/index.js", { eager: true });');
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/g\.ts: names a path into node_modules/);
    assert.match(r.stderr, /src\/g\.ts: uses import\.meta other than/);
    rmSync(join(dir, "src", "g.ts"));
    // Anything that isn't a regular file or directory (here a FIFO) is refused.
    if (spawnSync("mkfifo", [join(dir, "src", "pipe")]).status === 0) {
      r = run();
      assert.equal(r.status, 1);
      assert.match(r.stderr, /src\/pipe: not a regular file/);
      rmSync(join(dir, "src", "pipe"));
    }

    write("package.json", JSON.stringify({ dependencies: { ui: "1" }, imports: { "#t": "tool" } }));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /package\.json: the imports field/);
    write(
      "package.json",
      JSON.stringify({ dependencies: { ui: "1" }, devDependencies: { tool: "1" } }),
    );

    write(
      "vite.config.ts",
      'import p from "./plugin"; import { defineConfig } from "vite"; import { CSP } from "./csp";',
    );
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /vite\.config\.ts: imports \.\/plugin;/);
    assert.equal(r.stderr.match(/vite\.config\.ts: imports/g).length, 1);
    for (const cfg of ['{ "alias": {} }', '{ ["alias"]: {} }', "{ alias }", "{ alias, x }"]) {
      write("vite.config.ts", `export default ${cfg};`);
      r = run();
      assert.equal(r.status, 1, cfg);
      assert.match(r.stderr, /uses alias,/, cfg);
    }
    write("vite.config.ts", "export default { transformIndexHtml: {}, rooted: 1 };");
    r = run();
    assert.equal(r.status, 0, r.stderr);

    write("vite.config.mjs", "export default {};");
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /vite\.config\.mjs: Vite would load it/);
    rmSync(join(dir, "vite.config.mjs"));

    write("public/x.js", "");
    write(
      "vite.config.ts",
      "export default { root: 'x', resolve: { alias: {} }, publicDir: 'p', build: { rollupOptions: { input: 'y' } }, plugins: [{ resolveId(id) {}, load(id) {}, transform(c) {}, transformIndexHtml: {} }] };",
    );
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /public\/: Vite copies it/);
    for (const key of ["alias", "publicDir", "root", "input", "resolveId", "load", "transform"]) {
      assert.match(r.stderr, new RegExp(`uses ${key},`), key);
    }
    assert.equal(r.stderr.match(/uses transform,/g).length, 1);
    rmSync(join(dir, "public"), { recursive: true });

    // #112: library entries, emitted chunks, preprocessor injections, config and output hooks.
    const more = [
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
      "resolveDynamicImport",
      "renderDynamicImport",
      "resolveImportMeta",
      "resolveFileUrl",
      "writeBundle",
      "closeBundle",
      "jsxInject",
      "jsxImportSource",
      "tsconfigRaw",
      "renderBuiltUrl",
      "postcss",
    ];
    for (const key of more) {
      for (const cfg of [
        `{ ${key}: 1 }`,
        `{ ${key}() {} }`,
        `{ "${key}": 1 }`,
        `{ ${key} }`,
        `{ ["${key}"]: 1 }`,
        `{ [\`${key}\`](c) {} }`,
      ]) {
        write("vite.config.ts", `export default ${cfg};`);
        r = run();
        assert.equal(r.status, 1, cfg);
        assert.match(r.stderr, new RegExp(`uses ${key},`), cfg);
      }
    }
    write(
      "vite.config.ts",
      "export default { build: { lib: { entry: 'x' } }, css: { preprocessorOptions: { scss: { additionalData: '@import \"tool\";' } } }, plugins: [{ buildStart() { this.emitFile({ type: 'chunk', id: 'tool' }); } }] };",
    );
    r = run();
    assert.equal(r.status, 1);
    for (const key of ["lib", "entry", "additionalData", "emitFile"]) {
      assert.match(r.stderr, new RegExp(`uses ${key},`), key);
    }
    // No false alarms on names that merely contain a guarded word.
    write(
      "vite.config.ts",
      "import { defineConfig } from 'vite'; export default defineConfig({ build: { libX: 1, entryFileNames: 'a', rollupOptions: { onwarn() {} } }, optionsX: 1 });",
    );
    r = run();
    assert.equal(r.status, 0, r.stderr);
    write("vite.config.ts", "export default {};");

    // A quote of the other kind doesn't end the vite.config.ts import specifier.
    write("vite.config.ts", `import p from "./csp'/../plugin"; export default {};`);
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /vite\.config\.ts: imports \.\/csp'\/\.\.\/plugin;/);
    write("vite.config.ts", "export default {};");

    // csp.ts has a fixed shape: comment lines and one array of plain strings, joined.
    const cspOk = 'export const CSP = [\n  // a comment\n  "a \'self\'",\n  "b",\n].join("; ");\n';
    for (const text of [
      'import "tool";',
      'export * from "./x";',
      'const t = require("tool");',
      'process.getBuiltinModule("fs").writeFileSync("src/gen.ts", "x");',
      '"".constructor.constructor("return proc" + "ess")();',
      String.raw`\u0070rocess.exit();`,
      "export default { transform(c) { return c; } };",
      `${cspOk}import "tool";`,
      cspOk.replace('"b"', '"b" + x'),
      cspOk.replace('"b"', '"\\u0062"'),
      cspOk.replace('"b"', "`b`"),
      cspOk.replace('"b"', '"b" /* c */'),
      cspOk.replace('join("; ")', "join(x)"),
      cspOk.replace("  // a comment", '  "x", // trailing comment'),
      `// x\rimport "tool";\n${cspOk}`,
      `// x\u2028process.exit();\n${cspOk}`,
      `// x\u2029process.exit();\n${cspOk}`,
      cspOk.replace(/\n/g, "\r\n"),
    ]) {
      write("csp.ts", text);
      r = run();
      assert.equal(r.status, 1, text);
      assert.match(r.stderr, /csp\.ts: must be only/, text);
    }
    for (const text of [
      cspOk,
      "// header\nexport const CSP = [\"script-src 'self' 'wasm-unsafe-eval'\"].join(\"; \");",
      'export const CSP = [].join("; ");',
    ]) {
      write("csp.ts", text);
      r = run();
      assert.equal(r.status, 0, `${text}\n${r.stderr}`);
    }

    // tsconfig.json: no jsxImportSource, no extends, and it must exist.
    for (const text of [
      '{ "compilerOptions": { "jsxImportSource": "tool" } }',
      '{ "extends": "./base.json" }',
      '{ "files": [], "references": [{ "path": "./tsconfig.app.json" }] }',
      '{ "compilerOptions": { "jsx\\u0049mportSource": "tool" } }',
    ]) {
      write("tsconfig.json", text);
      r = run();
      assert.equal(r.status, 1, text);
      assert.match(r.stderr, /tsconfig\.json: uses (jsxImportSource|extends|references|\\u)/, text);
    }
    rmSync(join(dir, "tsconfig.json"));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /tsconfig\.json: missing/);
    write("tsconfig.json", "{}");

    // PostCSS configs in the app directory, its .config/ and a parent; a postcss field.
    for (const where of [
      join(dir, "postcss.config.js"),
      join(dir, ".postcssrc.json"),
      join(dir, ".config", "postcssrc.mjs"),
      join(top, "postcss.config.cjs"),
      join(top, ".postcssrc"),
    ]) {
      mkdirSync(join(where, ".."), { recursive: true });
      writeFileSync(where, "export default {};");
      r = run();
      assert.equal(r.status, 1, where);
      assert.match(r.stderr, /a PostCSS config/, where);
      rmSync(where);
    }
    write("package.json", JSON.stringify({ dependencies: { ui: "1" }, postcss: {} }));
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /package\.json: a postcss field/);
    write("package.json", JSON.stringify({ dependencies: { ui: "1" } }));
    writeFileSync(join(top, "package.json"), JSON.stringify({ postcss: { plugins: {} } }));
    r = run();
    assert.equal(r.status, 1);
    assert.ok(r.stderr.includes(`${join(top, "package.json")}: a postcss field`), r.stderr);
    writeFileSync(join(top, "package.json"), "{ not json");
    r = run();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /package\.json: unreadable/);
    rmSync(join(top, "package.json"));
    r = run();
    assert.equal(r.status, 0, r.stderr);
  } finally {
    rmSync(top, { recursive: true, force: true });
  }
});

test("index.html src and href attributes are scanned", () => {
  for (const text of [
    '<script type="module" src="tool"></script>',
    '<link rel="stylesheet" href="tool/x.css">',
    "<script type=module src=/vendor/x.js></script>",
    '<link href="../x.css">',
    '<SCRIPT TYPE="module" SRC="tool"></SCRIPT>',
    '<style>@IMPORT "tool/x.css";</style>',
  ]) {
    assert.equal(problems({ sources: [{ file: "index.html", text }] }).length, 1, text);
  }
  // In other files src=/href= is not an import (JSX props are scanned by the import forms).
  assert.deepEqual(problems({ sources: [{ file: "src/a.tsx", text: '<img src="x.png" />' }] }), []);
});
