# Proposal: the nightly job files its issues (`--file-issues`)

Status: proposed for #302 (PR for #302 part 1), 2026-11-27. Touches `.github/workflows/ci.yml`
(protected), so the owner applies it with signed commits.

## Problem

`cargo xtask nightly` gained an `issues` step (specs/tooling/nightly.md item 16): it files a
`crash` or `bug` issue per failure signature (failed limits cases, regress crashes and
timeouts, regress regressions) or comments on the open issue that already has it. It writes
to GitHub only with `--file-issues`, so a local run in a session (which shares the owner's
`gh` login) never files anything. The `project-nightly` job already has `issues: write` and
`GH_TOKEN`, and its step name says it "files issues", but its command has no flag, so the
nightly only prints `xtask: nightly: issues: <n> findings, not filed (no --file-issues)`.

## Proposed diff

In `.github/workflows/ci.yml`, job `project-nightly`:

```diff
       - name: "cargo xtask nightly (agent-owned - long fuzz, regress corpus, bench, files issues)"
         working-directory: pr
         env:
           GH_TOKEN: ${{ github.token }}
-        run: cargo xtask nightly --out ../nightly
+        run: cargo xtask nightly --out ../nightly --file-issues
```

`gh` finds the repository from the `pr` checkout's `origin` remote (actions/checkout sets
it), so no `-R` is needed.

## What it changes

- Issues are created by `github-actions[bot]` with the agent tag first in every body and
  comment. `ops/driver/owner-issues.sh` lists only the owner's untagged text, so they are
  data, never instructions (CONSTITUTION 1). Rule d picks the `crash` ones.
- At most 20 writes per night; no stderr or corpus bytes in any issue (item 16).
- A GitHub failure in the step makes the job exit 1 (a tool failure), as any other nightly
  tool failure does. `project-nightly` is not in `judge-verdict`'s `needs`, so it never turns
  a PR or main red.

## Not in this proposal

Regressions need `--previous` (the last night's `regress.json`). The job doesn't upload
`../nightly` or download an earlier run's copy yet, so the regression findings stay empty in
CI until a later change adds both.

## Test plan

1. After the merge, run the workflow by hand (`workflow_dispatch`) on main.
2. In the `cargo xtask nightly` step's log, find the line `xtask: nightly: issues: <n>
   findings: <c> filed, <u> updated, <s> over the cap of 20`. With today's `jpdf-cli` (no
   crashes in the regress tier, every limits case passing) `<n>` is 0 and no issue appears.
3. If a night does file one: its body starts with `🤖 jpdf-agent **Nightly finding**` and has
   a `nightly-signature:` line; a second run the same day comments `Seen again` on it instead of
   filing a second issue.
