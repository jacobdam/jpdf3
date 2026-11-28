# Proposal: the nightly job keeps `regress.json` between nights and uploads its report

Status: proposed for #497 (part 4 of #19), 2026-11-28. Touches `.github/workflows/ci.yml`
(protected), so the owner applies it with signed commits.

## Problem

`cargo xtask nightly` diffs this night's regress results against an earlier night's
`regress.json` given with `--previous FILE` (specs/tooling/nightly.md item 9), and files a
`bug` issue for each run that went from `ok` to `error` or `not_implemented` (item 16, the
`regression:<blob>:<op>` signature). The `project-nightly` job passes no `--previous`, and
nothing under `../nightly` outlives the runner, so:

- every night logs `nightly: no --previous results given: no diff` (run 45999791450, job
  140508197918, 2026-11-27) and writes a `regress-diff.json` with `"previous":null`;
- the regression findings are always empty, so a night that breaks a file which worked the
  night before files no issue (crashes and timeouts still do, without the diff);
- #19's deliverable "records per-file results and diffs against the previous night
  (artifact)" is not met: no artifact holds `regress.json`, `regress-diff.json`,
  `limits.json`, `bench.json`, `nightly.json` or the copied fuzz crashes.

`ops/proposals/nightly-file-issues.md` left this out ("Not in this proposal").

## Proposed diff

In `.github/workflows/ci.yml`, job `project-nightly` (the action pins are the ones the file
already uses: `actions/cache/restore` and `actions/cache/save` come from the same
`actions/cache` commit as the corpus caches, `upload-artifact` from the other uploads):

```diff
     steps:
       - *judge-checkout
       - *pr-checkout
       - *pin-config
+      - name: Restore the previous night's regress results
+        uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: prev-nightly/regress.json
+          key: nightly-regress-${{ github.run_id }}-${{ github.run_attempt }}
+          restore-keys: nightly-regress-
       - name: "cargo xtask nightly (agent-owned - long fuzz, regress corpus, bench, files issues)"
         working-directory: pr
         env:
           GH_TOKEN: ${{ github.token }}
-        run: cargo xtask nightly --out ../nightly --file-issues
+        run: cargo xtask nightly --out ../nightly --previous ../prev-nightly/regress.json --file-issues
+      - name: Keep this night's regress results for the next night
+        if: always() && hashFiles('nightly/regress.json') != ''
+        run: |
+          mkdir -p prev-nightly
+          cp nightly/regress.json prev-nightly/regress.json
+      - name: Save this night's regress results
+        if: always() && hashFiles('nightly/regress.json') != ''
+        uses: actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: prev-nightly/regress.json
+          key: nightly-regress-${{ github.run_id }}-${{ github.run_attempt }}
+      - name: Upload the nightly report
+        if: always()
+        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
+        with:
+          name: nightly-report
+          path: |
+            nightly/*.json
+            nightly/fuzz
+          retention-days: 30
```

## What it changes

- **The previous night.** The restore step finds no entry under its exact key (the key holds
  this run's id) and takes the newest entry whose key starts with `nightly-regress-` in the
  ref's scope: on `main` (the `schedule` event), the newest night saved on `main`. A
  `workflow_dispatch` run on another branch reads `main`'s entries too and saves its own
  under its branch, which `main` never reads. Pull-request jobs can't write entries that
  `main` restores (GitHub scopes caches by ref).
- **The first night** finds nothing: no file at `../prev-nightly/regress.json` is the
  documented "no previous" case (item 9: `"previous":null` and a "no diff" line), not an error.
- **Which night is saved.** The runner starts with an empty `nightly/`, so a
  `nightly/regress.json` there is this night's: the nightly writes it once the runs started,
  even when it then exits 1 or 75 (item 10). A night that fails before the runs saves
  nothing, and the next night diffs against the last night that ran. A night with download
  failures (exit 75) is saved too, with those files `missing`; the next night then sees
  `missing` → `ok` for them (a change, not a finding), and can't report an `ok` → `error`
  regression on them for one night.
- **The artifact** `nightly-report` keeps the night's JSON reports (`regress.json`,
  `regress-diff.json`, `limits.json`, `bench.json`, `nightly.json`) and the fuzz crashes the
  nightly copied to `nightly/fuzz/` (item 13, at most 100 per target) for 30 days. It holds no
  corpus bytes: `bench-corpus/` and `work/` are left out. The regress report is a few megabytes
  (5250 files, five runs each).
- `project-nightly` is not in `judge-verdict`'s `needs`, so none of these steps can turn a PR
  or `main` red. The job needs no new permission: the cache and artifact steps use the runner's
  own runtime token, as the corpus caches and the other uploads already do.

## Risks

- **A stale format blocks every night.** The nightly checks the previous file before the build
  (item 9) and fails with exit 1 on one it can't read. A later change to `regress.json`'s
  format that drops reading the saved one (schema 1) would fail every night before the runs,
  so no new file would be saved, and the restore step keeps finding the old one (a restore
  counts as use, so the 7-day eviction never comes). A change to the format must keep reading
  the saved one, or treat an unknown `schema` as "no previous"; that is the code's rule
  (specs/tooling/nightly.md), not this file's. Recovery if it happens: delete the entries
  (`gh cache list --key nightly-regress-` and `gh cache delete <id>`), and the next night
  starts without a diff.
- **Eviction.** The repository's cache is shared with the corpus caches (10 GB, least recently
  used goes first). An evicted entry costs one night without a diff.
- **Trust.** The previous file is written by the agent's own code in an earlier run of this
  same job and read by the agent's own code in the next; the nightly bounds it (64 MiB,
  specs/tooling/nightly.md Limits) and checks every field. No judge step reads it.

## If it isn't applied

Regressions from `ok` to `error` or `not_implemented` are never filed, nobody can read a past
night's results, and #19 stays open.

## Test plan

1. After the merge, run the workflow by hand on `main` (`gh workflow run ci.yml --ref main`).
2. In the `project-nightly` job: the restore step logs `Cache not found for input keys: …` (no earlier entry), the
   nightly step logs `nightly: no --previous results given: no diff` (no file there), the save
   step logs `Cache saved with key: nightly-regress-<run id>-1`, and the run has a
   `nightly-report` artifact holding `regress.json` and `regress-diff.json`.
3. Run it by hand again on `main`. The restore step logs
   `Cache restored from key: nightly-regress-<first run id>-1`, the nightly step logs
   `nightly: regress diff: <C> changes, <N> new crash/timeout, <F> fixed`, and this run's
   `regress-diff.json` names `../prev-nightly/regress.json` as `previous`.
4. `gh cache list --key nightly-regress-` lists both entries, both on `refs/heads/main`.
