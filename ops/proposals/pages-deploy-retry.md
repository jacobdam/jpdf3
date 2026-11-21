# Proposal: make the `pages` job survive the deploy-pages artifact race

Status: open (agent-written, 2026-11-20). Target: `.github/workflows/ci.yml` (protected).

## Problem
`actions/deploy-pages` sometimes runs right after `upload-pages-artifact` succeeds, can't
see the artifact yet, and fails with "No artifacts named github-pages were found". That
turns main red even though `judge-verdict` passed. It has happened twice:

- Run 44786622929 (after #39): `gh run rerun --failed` fixed it.
- Run 44837141465 (after #59): attempt 1 failed the same way. Attempt 2 (`rerun --failed`)
  then failed with "Multiple artifacts named github-pages were unexpectedly found … Artifact
  count is 2". The rerun of `pages` uploaded a second artifact under the same name, and
  artifacts from every attempt of a run stay visible. A run in this state can never deploy.
  Main stayed red until the next push (#62).

Agents can't fix this: `pages` runs only on `push`, so `workflow_dispatch` doesn't help, and
re-running that run is useless.

## Proposed diff
```diff
       - uses: actions/upload-pages-artifact@fc324d3547104276b827a68afc52ff2a11cc49c9 # v5.0.0
         with:
           path: site
+          name: github-pages-${{ github.run_attempt }}
-      - uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
+      - id: deploy
+        uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
+        continue-on-error: true
+        with:
+          artifact_name: github-pages-${{ github.run_attempt }}
+      - if: steps.deploy.outcome == 'failure'
+        run: sleep 30
+      - if: steps.deploy.outcome == 'failure'
+        uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
+        with:
+          artifact_name: github-pages-${{ github.run_attempt }}
```
- A per-attempt artifact name makes `gh run rerun --failed` safe again.
- One delayed retry covers the visibility race without a manual rerun.
- The second deploy step has no `continue-on-error`, so a real failure still fails the job.

## Risk if not applied
Each time the race hits, main stays red until the next push. A session that follows TICK
rule b and reruns the failed run makes that run impossible to deploy. Sessions spend time
diagnosing it, and the Pages metrics site goes stale.
