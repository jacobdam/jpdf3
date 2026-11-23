# Proposal: cache cargo-fuzz, the fuzz build and the Playwright browser in `project-linux`

Status: **applied** by the owner's interactive session for owner issue #254, 2026-11-23 (the
diff below, unchanged; #253's wording fix folded in). Proposed for #240 (fixes 2 and 3), 2026-11-22. Touches `.github/workflows/ci.yml`
(protected), so the owner applies it with signed commits. Fix 1 of #240 (the fuzz targets run
in parallel) is project code and lands in the same PR as this file.

## Problem

`project-linux` takes about 237 s on a small codebase, and `judge-verdict` waits for it
(#240, run 45044913523). After fix 1 the fuzzing itself takes about 30 s, but every run still
pays these fixed costs again:

| Step | Time | Why it repeats |
|---|---|---|
| `cargo install cargo-fuzz --locked --version 0.13.2` | 15 s | built from source on every run |
| `cargo fuzz build --debug-assertions` (nightly, ASan, release + debuginfo) | 34 s | no cache for `fuzz/target/` |
| `playwright install chromium --with-deps` | 37 s | the browser download has no cache; the apt part (`--with-deps`) stays |

## Proposed diff

In `.github/workflows/ci.yml`, job `project-linux`, insert three cache steps between
`actions/setup-node` and `cargo xtask ci linux`. They reuse the `actions/cache` pin already in
the file.

```diff
       - uses: actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1 # v7.1.0
         with:
           node-version: "24"
+      # #240: cargo-fuzz, built from source by `cargo xtask ci linux`. The key follows
+      # xtask/src/fuzz.rs, which holds CARGO_FUZZ_VERSION, so a version bump gets a new entry
+      # (built once, then saved). xtask reinstalls unless `cargo fuzz --version` is exactly
+      # that version, so a restored binary is never used at a wrong version.
+      - name: Cache cargo-fuzz
+        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: ~/.cargo/bin/cargo-fuzz
+          key: cargo-fuzz-${{ runner.os }}-${{ runner.arch }}-${{ hashFiles('pr/xtask/src/fuzz.rs') }}
+      # #240: the fuzz targets' build output, for one exact fuzz/Cargo.lock and
+      # xtask/src/fuzz.rs (which holds NIGHTLY). No restore-keys: restoring an older entry
+      # would make the new entry hold two toolchains' output and grow with every change.
+      - name: Cache fuzz build
+        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: pr/fuzz/target
+          key: fuzz-target-${{ runner.os }}-${{ hashFiles('pr/fuzz/Cargo.lock', 'pr/xtask/src/fuzz.rs') }}
+      # #240: the Playwright browser download (`--with-deps` still installs the apt packages).
+      - name: Cache Playwright browsers
+        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: ~/.cache/ms-playwright
+          key: playwright-${{ runner.os }}-${{ hashFiles('pr/apps/web/package-lock.json') }}
       - name: cargo xtask ci linux
         working-directory: pr
         run: cargo xtask ci linux --out ../e2e-linux
```

`pr/xtask/src/fuzz.rs` is in both keys because it holds `CARGO_FUZZ_VERSION` and the dated
`NIGHTLY`. Because the fuzz-build cache has no `restore-keys`, each entry holds the output of
one toolchain and one lockfile. The cost is a cold build (about 34 s, as today) on any run
whose `fuzz/Cargo.lock` or `xtask/src/fuzz.rs` differs from every saved entry it can read, or
after the entry is evicted (7 days without access, or LRU at the 10 GB limit):
every run after such a change until one succeeds and saves the entry (actions/cache saves only
when the job succeeds), on `main` or in a PR (later runs of that PR then read it).

## Expected effect

All estimates, to be checked against the first two runs after the change (the second one hits
the caches):
- cargo-fuzz: saves about 15 s (no build).
- Fuzz build: saves the third-party part of the 34 s (libfuzzer-sys's C++ and the other
  dependencies). The workspace crates the targets depend on still rebuild on every run,
  changed or not, because Cargo fingerprints path dependencies by file mtime and every
  checkout gives the sources new mtimes. Guess: 10–20 s saved.
- Playwright: saves the browser download, part of the 37 s; the apt packages
  (`--with-deps`) still install. Guess: 10–20 s saved.

Total: about 35–55 s saved per run. Nightly runs (`project-nightly`) do not get the caches.

## Rejected alternative

A prebuilt cargo-fuzz through `taiki-e/install-action` (the pin in `policy`/`untrusted`): at
that pin the action has no `cargo-fuzz` manifest, so it falls back to cargo-binstall, which
downloads `cargo-fuzz-0.13.2-x86_64-unknown-linux-musl.tar.gz` from the upstream release
without a checksum. The cache keeps the binary we build ourselves from crates.io with
`--locked`, for the same time saved.

## Security

- `project-linux` already runs PR code, and nothing in these caches reaches the grading job
  (`judge-verdict`), which restores none of them.
- GitHub scopes caches by ref: a PR run reads its own and the base branch's entries, and
  writes only its own, so a PR cannot change what `main` or other PRs restore. A bad
  `cargo-fuzz` or build output in a PR's own entry reaches only that PR's later
  `project-linux` runs, which already run its code.
- Not solved here, and not caused by this proposal: the trusted `judge-verdict` job restores
  its own caches (`corpus-smoke-…`, `ref-smoke-v1-…`) from the same PR scope, which PR code
  holding the run's cache token could write. That is #252, with its own fix. These three new
  keys are not ones the judge restores.
- The cargo-fuzz entry on `main` is written by `cargo install --locked` in a `main` run.
- Size: the fuzz build output is a few hundred MB per entry (one toolchain, no
  `restore-keys`, so entries don't accumulate), inside the 10 GB repository cache limit;
  GitHub evicts the least recently used entries first.

## Test plan

1. Apply on a branch; the first `project-linux` run shows a cache miss on all three steps and
   `cargo install cargo-fuzz` in its log.
2. Re-run it: all three steps hit, the log shows no `cargo install`, `Compiling libfuzzer-sys`
   is gone from the fuzz build, and Playwright downloads no browser. Record the job time and
   the `ci linux` time on #240.
3. Bump nothing else. `judge-verdict` stays green.

## Risk if not applied

`project-linux` stays about 35–55 s slower than needed on every PR (the estimate above);
nothing breaks.
