# Proposal: `/hold` turns auto-merge off, and the owner-commands comment reports the real state

Status: proposed for #210, 2026-11-23. Touches `.github/workflows/owner-commands.yml`
(protected), so the owner applies it with signed commits.

Files:
- `ops/proposals/hold-automerge/owner-commands.yml`: the whole proposed workflow (copy it over
  the current one).
- `ops/proposals/hold-automerge/test.sh`: a harness that runs the workflow's script against a
  fake `gh`.

## Problem

The owner's `/hold` is meant to make a PR unmergeable until `/release` (AUTOMATION.md, TICK §5).
It doesn't turn off auto-merge, and its comment says it did:

- Run 44976342149 (the owner's `/hold` on PR #151, 2026-11-22): `gh pr merge --disable-auto`
  failed with `GraphQL: Resource not accessible by integration (disablePullRequestAutoMerge)`.
  The step's `|| true` hid the error, and the bot commented "this PR is a draft again and
  auto-merge is off" ([comment](https://github.com/jacobdam/jpdf3/pull/151#issuecomment-7072941931)).
  The 2026-11-20 pilot drill failed the same way (docs/retros/0000-pilot.md, row 7).
- The workflow grants only `pull-requests: write`. GitHub's own example that runs
  `gh pr merge --auto` from Actions (docs.github.com, "Automating Dependabot with GitHub
  Actions", section "Enabling automerge on a pull request") grants `contents: write` and
  `pull-requests: write`. No GitHub page I found states the permission of
  `disablePullRequestAutoMerge` itself, so that it is enough is an inference from that example,
  checked by test T1 below.
- The draft half wasn't exercised: PR #151 was a draft already (`gh pr ready --undo` printed
  `already "in draft"`).

Impact: a held PR keeps auto-merge on. While it is a draft it can't merge, but `/release`
(ready again) would merge it at once if its checks are green, before the agent's next session
decides anything. If the draft conversion ever fails too, nothing stops the merge, and the
comment says the opposite.

## Proposed change

The full diff of `.github/workflows/owner-commands.yml` (the result is
`ops/proposals/hold-automerge/owner-commands.yml`):

```diff
@@ -7,7 +7,12 @@
   issue_comment:
     types: [created]
 
+# #210: disablePullRequestAutoMerge fails with "Resource not accessible by integration" under
+# pull-requests: write alone (run 44976342149). GitHub's own `gh pr merge --auto` example
+# (Dependabot docs) grants contents: write as well. The job checks out nothing and runs only
+# fixed gh commands; the comment body is only pattern-matched, never run.
 permissions:
+  contents: write
   pull-requests: write
 
 concurrency:
@@ -29,12 +34,44 @@
           GH_REPO: ${{ github.repository }}
           PR: ${{ github.event.issue.number }}
           BODY: ${{ github.event.comment.body }}
+          RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
         run: |
+          # #210: the comment reports the PR's state read back from GitHub after the commands,
+          # never what the commands were meant to do. Anything unreadable counts as failed.
+          set -u
+          # readstate sets draft and auto to the words gh prints ("true"/"false"), or to unknown
+          # if gh fails. Every check below compares exact words, so any other output fails closed.
+          readstate() {
+            draft=unknown auto=unknown
+            local s
+            s=$(gh pr view "$PR" --json isDraft,autoMergeRequest --jq '"\(.isDraft) \(.autoMergeRequest != null)"') || return 0
+            read -r draft auto <<<"$s" || true
+          }
+          err=""
+          readstate
           if [[ "$BODY" == /hold* ]]; then
-            gh pr merge "$PR" --disable-auto || true
-            gh pr ready "$PR" --undo
-            gh pr comment "$PR" --body "🤖 jpdf-agent /hold applied: this PR is a draft again and auto-merge is off. Only the owner's /release undoes it."
+            if [[ "$auto" != false ]]; then
+              gh pr merge "$PR" --disable-auto || err="${err}disabling auto-merge failed. "
+            fi
+            if [[ "$draft" != true ]]; then
+              gh pr ready "$PR" --undo || err="${err}converting to draft failed. "
+            fi
+            readstate
+            if [[ "$draft" == true && "$auto" == false ]]; then
+              gh pr comment "$PR" --body "🤖 jpdf-agent /hold applied: this PR is a draft and auto-merge is off (read back from GitHub). Only the owner's /release undoes it."
+            else
+              gh pr comment "$PR" --body "🤖 jpdf-agent /hold FAILED: ${err}State read back: draft=$draft, auto-merge on=$auto. Turn auto-merge off and convert the PR to a draft by hand. Run: $RUN_URL"
+              exit 1
+            fi
           else
-            gh pr ready "$PR"
-            gh pr comment "$PR" --body "🤖 jpdf-agent /release applied: this PR is ready again. The next agent session re-enables auto-merge."
+            if [[ "$draft" != false ]]; then
+              gh pr ready "$PR" || err="${err}marking ready failed. "
+            fi
+            readstate
+            if [[ "$draft" == false ]]; then
+              gh pr comment "$PR" --body "🤖 jpdf-agent /release applied: this PR is ready again (read back from GitHub). The next agent session re-enables auto-merge."
+            else
+              gh pr comment "$PR" --body "🤖 jpdf-agent /release FAILED: ${err}State read back: draft=$draft. Mark the PR ready by hand. Run: $RUN_URL"
+              exit 1
+            fi
           fi
```

What changes:
1. **Permission:** `contents: write` is added, so the auto-merge mutation can run.
2. **No hidden errors:** `|| true` is gone. A failed command is named in the comment.
3. **The comment reports the real state:** after the commands, the script reads `isDraft` and
   `autoMergeRequest` back from GitHub. It says "applied" only if the PR is a draft with
   auto-merge off (`/hold`) or not a draft (`/release`); otherwise it says "FAILED" with the
   state it read and the run link, and the job fails (a red run). If the state can't be read, it
   is `unknown`, which counts as failed.
4. **No no-op calls:** a mutation runs only when the state needs it (or is unknown), so a second
   `/hold` on a held PR doesn't depend on how `gh` treats a no-op.

## Risk

- `contents: write` lets the job's token push to the repository. The job runs only for an
  untagged `/hold` or `/release` comment by `jacobdam` on a PR, checks nothing out, and runs only
  the fixed `gh pr` commands above; the comment body reaches the script only through the `BODY`
  environment variable and is only compared with `/hold*`. The `main` ruleset (`ops/rulesets/main.json`:
  pull request and required status checks, no force push or deletion) still applies to the
  token, so it can't change `main` directly; other branches have no ruleset.
- If `contents: write` turns out not to be enough (test T1 fails with the same GraphQL error),
  the job now fails visibly with a "FAILED" comment instead of a false "applied", and the PR is
  still a draft. Then the owner turns auto-merge off by hand and this proposal needs another
  permission; the risk of applying it is no worse than today.
- If it isn't applied: every `/hold` leaves auto-merge on and reports the opposite, and a
  `/release` merges the PR at once.

## Test plan

**T0 (local, done in the proposal PR; rerun before applying):**
`ops/proposals/hold-automerge/test.sh` runs the `run: |` block of a workflow file against a
fake `gh` that keeps the PR's draft and auto-merge state and can fail each call. 13 cases: hold
on a ready PR with auto-merge, disable failing (the #210 case), draft conversion failing, already
held, draft with auto-merge on, state unreadable once, always, or garbled, `/hold` with trailing
text, and four `/release` cases. It also checks that a held PR gets no mutation calls and that an
unreadable state is reported as `unknown`.
- On the proposed file: `0 failure(s)`, exit 0.
- On today's `.github/workflows/owner-commands.yml`
  (`ops/proposals/hold-automerge/test.sh .github/workflows/owner-commands.yml`): 8 failures,
  exit 1, including `hold-disable-fails` (rc 0 and "applied" while auto-merge stays on).
- 11 hand mutants of the proposed script (each check term flipped or dropped, `exit 1` removed,
  each `readstate` re-read removed, the `unknown` default changed) all fail the harness.

**On GitHub, after applying (the owner; add to #208):**
- **T1** (the #210 case): on a ready PR with auto-merge on (a docs-only scratch PR), comment
  `/hold`. Expect: the run is green, the comment says "/hold applied … (read back from GitHub)",
  and `gh pr view <N> --json isDraft,autoMergeRequest` shows `isDraft: true`,
  `autoMergeRequest: null`. The run log has no `Resource not accessible` line.
- **T2:** comment `/hold` again. Expect: "/hold applied", green, and the run log shows no
  `gh pr merge` or `gh pr ready` output.
- **T3:** comment `/release`. Expect: "/release applied", `isDraft: false`, auto-merge still off
  (the next session re-enables it).
- **T4:** close the scratch PR.

A failing GitHub run can't be forced without changing the workflow again, so the FAILED path is
covered by T0 only.
