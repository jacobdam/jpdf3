## 01:36 UTC #364 merge origin/main (STATUS conflict)
- Session: worker own-pr #364
- Picked: rule c because PR #364 (#323) conflicted with main after #368 merged
- Did: merged origin/main into t/323-exec-check (97277f2); the only conflict was ops/STATUS.md: took main's copy,
  dropped the bounded-buffer pending lesson (this PR moves it into ops/LESSONS.md), marked #368 merged, added #364's
  line. The remerge-diff touches only ops/STATUS.md. `cargo nextest run -p xtask`: 239 passed.
- Result: no review round (no code change since round 1's triage; merge and records only); `ai-review` success
  re-posted on this commit, auto-merge on
- Cost: ~10%; review rounds 1/4; regraded 0, overturned 0; deferred: none
- Codex: skipped (no review round)
- Lesson: none
- Ignored: none
