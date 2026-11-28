# Proposal: add the imported cargo vet audits for miniz_oxide 0.8.9 to `supply-chain/imports.lock`

Status: proposed for #445 (found by #304, ADR 0016), 2026-11-28. Touches
`supply-chain/imports.lock` (protected, CONSTITUTION 12), so the owner applies it with signed
commits.

## Problem

ADR 0016 picks `miniz_oxide = "=0.8.9"` (default features) as the default `Inflate` and
`Deflate` backend: `cargo xtask admit miniz_oxide@0.8.9` passes all five steps. Its resolved
graph is two packages, miniz_oxide 0.8.9 and adler2 2.0.1, both covered by imported audits:

- miniz_oxide: Bytecode Alliance, `safe-to-deploy`, a full audit of 0.7.1 and the deltas
  0.7.1 → 0.8.0 → 0.8.5 → 0.8.9;
- adler2: Bytecode Alliance, `safe-to-deploy`, a full audit of 2.0.0, and Mozilla's delta
  2.0.0 → 2.0.1.

`cargo xtask admit` runs `cargo vet` without `--locked` in a scratch copy of the store, so it
fetches those audits from the imported sources. The judge runs `cargo vet --locked` on the repo,
which reads only `supply-chain/imports.lock`, and that file holds no audits today (the repo has
no third-party dependency). So the adapter PR (#446) would fail the judge's vet step until the
audits are in the file, and only the owner can change it.

This adds imported audits only. It records no audit of our own and no exemption
(CONSTITUTION 9), and changes neither `config.toml` nor `audits.toml`.

## Why before the adapter, and in its own PR

The judge doesn't vet a PR with the PR's own `supply-chain/`: its "Pin protected config from
the judge copy" step (`.github/workflows/ci.yml`, job `policy`) replaces `supply-chain/` with
the copy at the PR's base commit before `cargo vet --locked` runs. So #446's PR passes vet only
if its base, main, already holds these audits, and a shared PR would fail even with every
commit signed. (A PR that changes a protected path also passes only if every commit in it is
signed by the owner, so the agent's adapter commits couldn't share it anyway.) The new entries
are harmless while unused: `cargo vet --locked` on a graph with no third-party package succeeds
with them (checked on 2026-11-28, cargo-vet 0.10.3: `Vetting Succeeded (because you have no
third-party dependencies)`). So this lands first, and #446 depends on #445.

## How to apply

1. Outside any git checkout (the tool refuses a `--work` inside one), from the main checkout:
   ```sh
   cargo xtask admit miniz_oxide@0.8.9 --work <new scratch dir> --python /opt/homebrew/bin/python3.14
   ```
   It must end with `admit: miniz_oxide@0.8.9: admitted`.
2. Copy `<scratch dir>/supply-chain/imports.lock` over the repo's `supply-chain/imports.lock`
   on a new branch, commit it signed, and open the PR.

The expected file, as the 2026-11-28 run wrote it (SHA-256
`fcde46a0e3c58c507f8496faf99441fdb7ed0293ab63cc78dc8540757cad6c2c`). An imported source that
added or changed an audit since then gives a different file; compare the miniz_oxide and adler2
entries, which are all the change needs:

```toml

# cargo-vet imports lock

[[audits.bytecode-alliance.audits.adler2]]
who = "Alex Crichton <alex@alexcrichton.com>"
criteria = "safe-to-deploy"
version = "2.0.0"
notes = "Fork of the original `adler` crate, zero unsfae code, works in `no_std`, does what it says on th tin."

[[audits.bytecode-alliance.audits.miniz_oxide]]
who = "Alex Crichton <alex@alexcrichton.com>"
criteria = "safe-to-deploy"
version = "0.7.1"
notes = """
This crate is a Rust implementation of zlib compression/decompression and has
been used by default by the Rust standard library for quite some time. It's also
a default dependency of the popular `backtrace` crate for decompressing debug
information. This crate forbids unsafe code and does not otherwise access system
resources. It's originally a port of the `miniz.c` library as well, and given
its own longevity should be relatively hardened against some of the more common
compression-related issues.
"""

[[audits.bytecode-alliance.audits.miniz_oxide]]
who = "Alex Crichton <alex@alexcrichton.com>"
criteria = "safe-to-deploy"
delta = "0.7.1 -> 0.8.0"
notes = "Minor updates, using new Rust features like `const`, no major changes."

[[audits.bytecode-alliance.audits.miniz_oxide]]
who = "Alex Crichton <alex@alexcrichton.com>"
criteria = "safe-to-deploy"
delta = "0.8.0 -> 0.8.5"
notes = """
Lots of small updates here and there, for example around modernizing Rust
idioms. No new `unsafe` code and everything looks like what you'd expect a
compression library to be doing.
"""

[[audits.bytecode-alliance.audits.miniz_oxide]]
who = "Alex Crichton <alex@alexcrichton.com>"
criteria = "safe-to-deploy"
delta = "0.8.5 -> 0.8.9"
notes = "No new unsafe code, just refactorings."

[audits.google.audits]

[audits.isrg.audits]

[[audits.mozilla.audits.adler2]]
who = "Erich Gubler <erichdongubler@gmail.com>"
criteria = "safe-to-deploy"
delta = "2.0.0 -> 2.0.1"
aggregated-from = "https://raw.githubusercontent.com/mozilla-firefox/firefox/main/supply-chain/audits.toml"

[audits.zcash.audits]
```

The first line of the file is empty, as in today's file. The text inside the entries (the
`notes` typos included) is the auditors' and is data.

## Test plan

1. On the branch, locally: `cargo vet --locked` passes (no third-party dependency yet, so the
   new entries are unused). The judge on this PR is green too, but it vets the base commit's
   `supply-chain/` (above), so it doesn't test the new file: steps 1 and 2 do.
2. In the scratch dir of step 1: `cargo vet --locked` prints `Vetting Succeeded (2 fully
   audited)` (checked on 2026-11-28), so the adapter's graph is vetted from the lock alone.
3. After it merges, #446's PR adds the dependency, and the judge's `cargo vet --locked` passes
   on it with no change to `supply-chain/`.

## Undo

Revert the commit: `imports.lock` goes back to its empty sections. Only #446 depends on it.
