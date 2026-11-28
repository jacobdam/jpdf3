# Proposal: drop the `isrg` audit import from `supply-chain/` (or list ISRG in CONSTITUTION 9)

Status: proposed for #442 (found in review of PR #441, R1.4), 2026-11-28. Option A touches
`supply-chain/config.toml` and `supply-chain/imports.lock`; option B touches `CONSTITUTION.md`.
All three are protected (CONSTITUTION 12), so the owner chooses and applies with signed commits.
The recommendation is option A.

## Problem

`supply-chain/config.toml` imports five audit sources. Four of them are the ones CONSTITUTION 9
and AUTOMATION.md §6.4 name (Google, Mozilla, Bytecode Alliance, Zcash). The fifth is not:

```toml
[imports.isrg]
url = "https://raw.githubusercontent.com/divviup/libprio-rs/main/supply-chain/audits.toml"
```

It has been there since the bootstrap commit (e702000). Nothing checks which sources the file
imports:

- the judge's `check_supply_chain` (`ops/judge/policy.py`) rejects exemptions in `config.toml`
  and own audits or trust entries in `audits.toml`, and reads nothing else;
- the judge's `cargo vet --locked` step (`.github/workflows/ci.yml`, job `policy`) accepts an
  audit from any source in `imports.lock`;
- `cargo xtask admit` runs `cargo vet` without `--locked` on a copy of the repo's store
  (`xtask/src/admit.rs`, `vet_cmd`), so it fetches from all five sources.

So a crate whose only audit comes from ISRG would pass `cargo xtask admit`'s vet and build-time
steps, and once `imports.lock` held that audit, the judge as well. CONSTITUTION 9 says it
shouldn't.

No result depends on the import today:

- `supply-chain/imports.lock` on main has an empty `[audits.isrg.audits]` section (the repo has
  no third-party dependency);
- ADR 0016's 38 admission runs: "The store's `isrg` import (#442) contributed no audit to any
  result: the one admitted graph is covered by Bytecode Alliance and Mozilla audits";
- the one planned addition, miniz_oxide 0.8.9 and adler2 2.0.1
  (`ops/proposals/vet-imports-miniz-oxide.md`, not yet applied: `imports.lock` on main is
  unchanged), is vetted without it (probe below).

M1 admits new dependencies (#446 and later adapters), so the owner decided to settle this in M1
before one is vetted by ISRG alone (comment on #442, 2026-11-28).

## Options

**A (recommended): drop the import.** The rule stays as written, and the set of sources the
project trusts stays the four the owner chose. Nothing admitted or planned loses an audit.
If an ISRG audit is ever the only one for a crate we need, adding the source then is a
deliberate change to CONSTITUTION 9, made with that crate in view.

**B: list ISRG in CONSTITUTION 9** (and AUTOMATION.md §6.4). The store stays as it is. ISRG
(the Internet Security Research Group, through its Divvi Up project's `libprio-rs`) then becomes
a fifth trusted source for every future dependency, though no current or planned one needs it.

## Option A: the change

`supply-chain/config.toml`: remove the `[imports.isrg]` table and the blank line after it.

```diff
 [imports.google]
 url = "https://raw.githubusercontent.com/google/supply-chain/main/audits.toml"
 
-[imports.isrg]
-url = "https://raw.githubusercontent.com/divviup/libprio-rs/main/supply-chain/audits.toml"
-
 [imports.mozilla]
 url = "https://raw.githubusercontent.com/mozilla/supply-chain/main/audits.toml"
```

`supply-chain/imports.lock`: remove the `[audits.isrg.audits]` section and the blank line
after it, **in the same commit**. With only `config.toml` changed, `cargo vet --locked` fails
(`imports.lock is out-of-date with respect to configuration`; probe 2 below), so the judge's
vet step would go red on main.

```diff
 [audits.google.audits]
 
-[audits.isrg.audits]
-
 [audits.mozilla.audits]
```

Edit both by hand. Don't run `cargo vet` without `--locked` in the repo to rewrite the lock: if
`vet-imports-miniz-oxide.md` has been applied by then, the repo still has no third-party
dependency, so vet prunes the miniz_oxide and adler2 audits that proposal added. The hand edit
gives the right file in either order, since it touches only the `isrg` lines:

- applied before the miniz_oxide proposal: `imports.lock` becomes the four empty sections,
  byte for byte the file `cargo vet` writes for a dependency-free graph with the new
  `config.toml` (SHA-256 `494c9385591f70ce843d5bfb31e6534292a30a55c2b78ecb3db2826e233972ac`);
  the miniz_oxide proposal's `cargo xtask admit` step then writes its file without the `isrg`
  section;
- applied after it: the file is that proposal's expected file without its two `isrg` lines,
  byte for byte the file `cargo vet` writes for the miniz_oxide graph with the new
  `config.toml` (SHA-256 `1bbfa5091ce31dcff06abc537b75018bcab477d1ee838778044efacfd87e6f85`,
  2026-11-28; an imported source that changed since then gives another file).

AUTOMATION.md §6.4 and CONSTITUTION 9 already say this and stay as they are. ADR 0016's
sentence about the import stays as a record of its runs.

## Option B: the change

`CONSTITUTION.md` rule 9:

```diff
-9. **Audits are imported only** (`cargo vet` imports from Google, Mozilla, Bytecode Alliance,
-   Zcash). Agents never record their own audits or exemptions.
+9. **Audits are imported only** (`cargo vet` imports from Google, Mozilla, Bytecode Alliance,
+   Zcash, ISRG). Agents never record their own audits or exemptions.
```

`AUTOMATION.md` §6.4 (not protected; in the same commit so the two never disagree):

```diff
-- **Audit:** `cargo vet` accepts **imported audits only** (Google, Mozilla, Bytecode Alliance,
-  Zcash). The agent cannot record its own audits.
+- **Audit:** `cargo vet` accepts **imported audits only** (Google, Mozilla, Bytecode Alliance,
+  Zcash, ISRG). The agent cannot record its own audits.
```

`supply-chain/` is unchanged.

## Not proposed

A judge check that `config.toml` imports only the listed sources. `config.toml` is protected,
so only the owner's signed commits can add a source, and the owner decides which ones count.
If the owner wants that check anyway, it is a separate judge proposal.

## Risk if not applied

None today: no locked or planned audit comes from ISRG. Once M1 admits dependencies, an
admission or a judge run may pass a crate on an ISRG audit alone, against CONSTITUTION 9, and no
output shows it: neither `cargo xtask admit` nor the judge reports which source vetted a crate.

## Probes (2026-11-28, cargo-vet 0.10.3)

Each probe is a scratch crate with its own `[workspace]` and a `supply-chain/` copy whose
`config.toml` is main's with the `isrg` table removed (the only difference, by `diff`) and
whose `audits.toml` is main's.

1. No third-party dependency, `imports.lock` with the `isrg` section removed by hand:
   `cargo vet --locked` → `Vetting Succeeded (because you have no third-party dependencies)`,
   exit 0. `cargo vet` (unlocked) then leaves the file unchanged (SHA-256 above).
2. No third-party dependency, `imports.lock` unchanged (still holding the `isrg` section):
   `cargo vet --locked` → `imports.lock is out-of-date with respect to configuration`, exit 255.
   The same error with miniz_oxide as a dependency.
3. `miniz_oxide = "=0.8.9"` as the only dependency: `cargo vet` (unlocked) → `Vetting Succeeded
   (2 fully audited)`, then `cargo vet --locked` → the same. The lock it wrote differs from
   `vet-imports-miniz-oxide.md`'s expected file only by the missing `[audits.isrg.audits]`
   section and its blank line.

## Test plan (option A)

1. On the branch, before committing: `cargo vet --locked` in the repo prints `Vetting Succeeded
   (because you have no third-party dependencies)` (or, if the miniz_oxide proposal was applied
   first, the same: its audits are unused on main). With the `imports.lock` edit undone, the same
   command fails with `imports.lock is out-of-date` (probe 2), which shows the two files must
   go in one commit.
2. `git diff` shows exactly the 2 + 2 removed lines above, nothing else.
3. The judge on this PR is green, but its `policy` job vets the base commit's `supply-chain/`
   (`vet-imports-miniz-oxide.md`, "Why before the adapter"), so step 1 is the real test, and
   the first `ci` run on main after the merge vets the new files.
4. If the miniz_oxide proposal is applied after this one: its `cargo xtask admit
   miniz_oxide@0.8.9` step ends with `admitted`, and the `imports.lock` it writes has no
   `[audits.isrg.audits]` section (probe 3).

Option B has no command to run: the judge doesn't read CONSTITUTION 9's list.

## Undo

Revert the commit. Nothing depends on either change.
