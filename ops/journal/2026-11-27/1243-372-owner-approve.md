## 12:43 UTC #372 Owner /approve on design PRs; rule c picks a design PR failing in its window
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/372#issuecomment-7218421995
- Picked: rule a because the owner's "Look good, proceed" on #372 (a tagged issue filed for the owner) had no reply.
- Did: PR #376. `cargo xtask next-session` step 3's window term also picks a design PR by the owner's `/approve`
  (newest of `/hold`, `/release`, `/approve`, at or after the time its `ai-review` success counts from, so a
  record-only re-post keeps it and a new round ends it; never releases a hold); step 1 leaves such an approval to
  step 3; step 3's failing-PR term ignores the window's `owner-todo` on a non-draft `design` PR's issue (the #347/#359
  deadlock). TICK §0, §1, §2, §4, §5, §8 step 5 (void / already merging / pending replies for an approval rule a
  gets), §10 step 6; AUTOMATION.md; ADR 0011 §7; specs/tooling/next-session.md. One test assertion changed (the
  deadlock case was asserted `planner`), explained in the PR. Moved STATUS's 3 pending lessons into LESSONS.
  Removed the merged #359 worktree.
- Result: 4 rounds; `ai-review` success posted on this commit, auto-merge on. "done:" on #372 waits for the merge.
- Cost: ~55% of context; review rounds 4/4; regraded 0, overturned 0; deferred: #377
- Codex: skipped (no crates/ or tools/ path)
- Lesson: an owner command the scheduler acts on is also an owner comment in the feed → step 1 (rule a) outranks it
  every run unless step 1 leaves it to the acting step; test new commands with their feed line (R1.1).
- Ignored: none
