# Proposal: cache cargo-fuzz, the fuzz build and the Playwright browser in `project-linux`

Status: proposed for #240 (fixes 2 and 3), 2026-11-22. Touches `.github/workflows/ci.yml`
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
+      # #240: cargo-fuzz, built from source by `cargo xtask ci linux` once per version. xtask
+      # reinstalls it unless `cargo fuzz --version` is exactly its CARGO_FUZZ_VERSION, so a
+      # stale entry costs one rebuild, never a wrong version.
+      - name: Cache cargo-fuzz
+        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: ~/.cargo/bin/cargo-fuzz
+          key: cargo-fuzz-0.13.2-${{ runner.os }}-${{ runner.arch }}
+      # #240: the fuzz targets' build output. Cargo's fingerprints decide what to rebuild,
+      # so a restore from an older key is only slower, never wrong.
+      - name: Cache fuzz build
+        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: pr/fuzz/target
+          key: fuzz-target-${{ runner.os }}-${{ hashFiles('pr/fuzz/Cargo.lock', 'pr/xtask/src/fuzz.rs') }}
+          restore-keys: fuzz-target-${{ runner.os }}-
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

`pr/xtask/src/fuzz.rs` is in the fuzz-build key because it holds the dated `NIGHTLY`: a bump
gets a new entry instead of growing the old one with a second toolchain's output.

## Expected effect

About 15 s (cargo-fuzz) + 20–30 s (fuzz build: only the workspace crates the PR changed and
the targets rebuild; libFuzzer's C++ and the dependencies are cached) + the browser download
share of 37 s. These are estimates, to be checked against the first two runs after the change
(the second one hits the caches). Nightly runs (`project-nightly`) do not get the caches.

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
  writes only its own, so a PR cannot change what `main` or other PRs restore. A PR that
  writes a bad `cargo-fuzz` or build output into its own entry only affects its own later runs,
  which already run its code.
- The cargo-fuzz entry on `main` is written by `cargo install --locked` in a `main` run.
- Size: the fuzz build output is a few hundred MB per entry, inside the 10 GB repository
  cache limit, and GitHub evicts old entries first.

## Test plan

1. Apply on a branch; the first `project-linux` run shows a cache miss on all three steps and
   `cargo install cargo-fuzz` in its log.
2. Re-run it: all three steps hit, the log shows no `cargo install`, `Compiling libfuzzer-sys`
   is gone from the fuzz build, and Playwright downloads no browser. Record the job time and
   the `ci linux` time on #240.
3. Bump nothing else. `judge-verdict` stays green.

## Risk if not applied

`project-linux` stays about 70–80 s slower than needed on every PR; nothing breaks.
