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
+      - name: Show the restored file's checksum
+        run: sha256sum prev-nightly/regress.json || echo "no previous regress.json"
       - name: "cargo xtask nightly (agent-owned - long fuzz, regress corpus, bench, files issues)"
+        id: nightly
         working-directory: pr
         env:
           GH_TOKEN: ${{ github.token }}
-        run: cargo xtask nightly --out ../nightly --file-issues
+        run: |
+          set +e
+          cargo xtask nightly --out ../nightly --previous ../prev-nightly/regress.json --file-issues
+          code=$?
+          echo "exit=$code" >> "$GITHUB_OUTPUT"
+          exit "$code"
+      - name: Keep this night's regress results for the next night
+        if: always() && (steps.nightly.outputs.exit == '0' || steps.nightly.outputs.exit == '4') && hashFiles('nightly/regress.json') != ''
+        run: |
+          mkdir -p prev-nightly
+          cp nightly/regress.json prev-nightly/regress.json
+      - name: Save this night's regress results
+        if: always() && (steps.nightly.outputs.exit == '0' || steps.nightly.outputs.exit == '4') && hashFiles('nightly/regress.json') != ''
+        uses: actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
+        with:
+          path: prev-nightly/regress.json
+          key: nightly-regress-${{ github.run_id }}-${{ github.run_attempt }}
+      - name: Upload the nightly report
+        if: always()
+        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
+        with:
+          name: nightly-report
+          path: nightly/*.json
+          retention-days: 14
```

## What it changes

- **The previous night.** The restore step finds no entry under its exact key (the key holds
  this run's id) and takes the newest entry whose key starts with `nightly-regress-` in the
  ref's scope: on `main` (the `schedule` event), the newest night saved on `main`. A
  `workflow_dispatch` run on another branch reads `main`'s entries too and saves its own
  under its branch, which `main` never reads. Pull-request jobs can't write entries that
  `main` restores (GitHub scopes caches by ref).
- **The first night** finds nothing: no file at `../prev-nightly/regress.json` is the
  documented "no previous" case (item 9), not an error: `regress-diff.json` holds
  `"previous":null` with the note `no previous results at ../prev-nightly/regress.json`
  (xtask/src/nightly.rs `load_previous`).
- **Which night is saved.** The run step records the nightly's exit code as the step output
  `exit`, and the keep and save steps run only for 0 or 4 (item 10: the runs finished; 4 is a
  failed limits case, not a regress problem) with a `nightly/regress.json` present. The runner
  starts with an empty `nightly/`, so that file is this night's. A night that exits 1 (a tool
  failure) or 75 (corpus downloads failed, rerun by item 10's rule), or that is cancelled or
  times out (no output), saves nothing, so a rerun of it restores the last saved night, not
  its own first attempt, and the next night diffs against the last night that finished.
- **What a saved night still hides.** A night that ran out of `--max-minutes` exits 0 with
  `complete:false` and `not_run` runs (item 7), and a night with up to `max(1, ⌊N/100⌋)`
  files not downloaded exits 0 with them `missing`. Both are saved; the next night sees
  `not_run` or `missing` → its status for those runs, a change but not a finding, so an
  `ok` → `error` regression on them goes unreported for one night. A rerun of a night that
  exited 0 or 4 restores that night's own first attempt and diffs against it.
- **The artifact** `nightly-report` keeps the night's JSON reports (`regress.json`,
  `regress-diff.json`, `limits.json`, `bench.json`, `nightly.json`) for 14 days. It holds no
  corpus bytes (`bench-corpus/` and `work/` are left out) and no fuzz crash inputs
  (`nightly/fuzz/` is left out, so no reproducer of an unfixed crash is downloadable from this
  public repository). The nightly's `crash` issue names the target and the artifact; the
  input itself is lost with the runner, as it is today, and has to be found again with
  `cargo xtask fuzz --target <target>`. Keeping it for the owner alone needs another store
  and is not in this proposal. `regress.json` keeps the last 4 KiB of stderr of
  `crash` and `timeout` runs (item 5); those runs are on public corpus files whose bytes
  anyone can already fetch, and the 14-day retention bounds how long the tails stay up. The
  regress report is a few megabytes (5250 files, five runs each).
- `project-nightly` is not in `judge-verdict`'s `needs`, so none of these steps can turn a PR
  or `main` red. The job needs no new permission: the cache and artifact steps use the runner's
  own runtime token, as the corpus caches and the other uploads already do.

## Risks

- **A stale format blocks every night.** The nightly checks the previous file before the build
  (item 9) and fails with exit 1 on one it can't read. A later change to `regress.json`'s
  format that bumps its `schema` would fail every night before the runs (item 9 rejects a
  `schema` other than 1, and the code does too), so no new file would be saved, and the
  restore step keeps finding the old one (a restore counts as use, so the 7-day eviction never
  comes). Nothing prevents this today: #500 proposes that an unknown `schema` counts as "no
  previous". Recovery if it happens: delete the entries
  (`gh cache list --key nightly-regress-` and `gh cache delete <id>`), and the next night
  starts without a diff.
- **A night that always exits 1 freezes the baseline.** A night whose runs all finished but
  that exits 1 afterwards (a bench or fuzz pass `failed`, a `gh` failure while filing, a
  `nightly.json` write failure: items 10, 15 and 16) is not saved either. If such a failure
  repeats every night (a broken `cargo-fuzz` install, say), the saved entry is never
  replaced and each restore keeps it from eviction, so every later night diffs against that
  same old night. Regressions since then are still found, but each one is reported again
  every night (the issue gets a "Seen again" comment, item 16) until the failure is fixed.
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
   nightly step logs `nightly: no previous results at ../prev-nightly/regress.json: no diff`
   (not `no --previous results given`, which would mean the flag is missing), the save
   step logs `Cache saved with key: nightly-regress-<run id>-1`, and the run has a
   `nightly-report` artifact holding `regress.json` and `regress-diff.json`.
3. Run it by hand again on `main`. The restore step logs
   `Cache restored from key: nightly-regress-<first run id>-1`, the nightly step logs
   `nightly: regress diff: <C> changes, <N> new crash/timeout, <F> fixed`, and this run's
   `regress-diff.json` names `../prev-nightly/regress.json` as `previous`.
4. Run it a third time. Its restore step logs `Cache restored from key:
   nightly-regress-<second run id>-1`. That shows only that the second run's save step ran
   (the key comes from the run id), so check the content too: download the first and second
   runs' `nightly-report` artifacts; the checksum step of the third run prints the
   `sha256sum` of the second run's `regress.json`, which differs from the first run's (each
   run records its own `ms`). The same check on the second run shows the first run's
   checksum. If the keep step copied nothing, the third run prints the first run's checksum.
5. `gh cache list --key nightly-regress-` lists the three entries, all on `refs/heads/main`.

The plan doesn't exercise a night that exits 1 or 75, or a regression filed end to end; those
paths depend only on the step output and on the nightly's own tests (specs/tooling/nightly.md).
