## 14:28 UTC #334 / PR #379 design window: owner /approve, auto-merge on
- Session: worker design-window #379
- Picked: rule c (scheduler step 3) because the owner's `/approve` on PR #379 (issuecomment-7220357575, 14:06)
  came after the head's `ai-review` success (round-4 triage, 13:32) and is the owner's newest command on it
- Did: posted "picked up" (issuecomment-7220887498) and "done:" (issuecomment-7220893806) on PR #379; enabled
  auto-merge (squash). Issue #334 had no `owner-todo`, so no label changed. Head 1842d6e: CI green, `ai-review`
  success.
- Result: auto-merge on for PR #379; this records PR holds only this entry and STATUS
- Cost: ~10%; review rounds n/a (no new round); regraded 0, overturned 0; deferred: none
- Codex: skipped (no review round)
- Lesson: the "picked up" reply named the session `20261127T2330Z`, a wrong time (the session started about 14:28
  UTC); comment edits need `gh api`, which the guard forbids, so the correct time is recorded here → run `date -u`
  before writing the first reply
- Ignored: none. A first `cargo xtask next-session` run failed with an HTTP 504 from GitHub (exit 2, no work);
  the rerun printed the pick.
