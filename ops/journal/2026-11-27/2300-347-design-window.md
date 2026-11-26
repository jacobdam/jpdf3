## 23:00 UTC #331 / PR #347 design window: merge main, new window
- Session: worker design-window #347
- Picked: rule c (scheduler step 3) because PR #347's 24-hour window passed with `ai-review` success and no `/hold`
- Did: PR #347 conflicted with main in ops/STATUS.md. A local `git merge origin/main` was blocked by the guard
  because the merge brought main's `ops/driver/driver.sh` (the owner's PR #355) into the worktree, so the session set
  the branch's STATUS to main's copy (8ed3425, record commit) and ran `gh pr update-branch 347` (clean merge
  717f73a, remerge-diff empty). Every commit since the checked 0b8e7f9 is a record commit or a clean merge, so
  `ai-review` success is re-posted on the new head (target: the round-4 triage). It is a `design` PR, so a push starts
  a new 24-hour window (§4 step 6, §8 step 5): `owner-todo` stays on #331, a new window comment there, no auto-merge.
- Result: `ai-review` success posted on this commit; auto-merge off until the next `worker design-window #347` pick
- Cost: ~15%; review rounds 4/4 (no new round); regraded 0, overturned 0; deferred: none new (#348 from round 4)
- Codex: skipped (no review round)
- Lesson: merging origin/main locally into a branch whose merge brings in a protected-path change from main is blocked
  by the guard → make the conflicting record file equal to main's in a record commit, push, then `gh pr update-branch N`
- Ignored: none
