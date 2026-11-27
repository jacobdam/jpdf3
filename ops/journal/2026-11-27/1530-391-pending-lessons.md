## 15:30 UTC #386 / #391 / PR #396 pending lessons as one file per session (STATUS 2/3)
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/386#issuecomment-7221268891
- Picked: rule a because the owner's "Ok" on #386 still has no "done:" reply; part 1 (#390, PR #393) merged, so
  this session worked part 2, #391
- Did: claimed #391; PR #396: a lesson found after a PR's last review round goes into the session's own
  `ops/lessons/pending/YYYY-MM-DD-HHMM-<slug>.md` instead of STATUS's `Pending lessons` list; the next work PR moves it
  into ops/LESSONS.md before round 1 and deletes the file, the retro folds any left (both skip a file an open agent PR
  already deletes). Record commits (TICK §4 step 6, §8, §9, ADR 0011 §3) may add or edit pending lesson `.md` files
  not on origin/main; `ops/lessons/pending/README.md` (the format) is the process area's in ops/areas.toml and keeps
  the directory tracked for `cargo xtask issue-lint`. Moved the one pending lesson from STATUS into LESSONS. Removed
  the merged #393's worktree t/390-status.
- Result: `ai-review` success posted on this commit, auto-merge on
- Cost: ~35%; review rounds 2/4; regraded 0, overturned 0; deferred: #397
- Codex: skipped (no crates/ or tools/ path)
- Lesson: a record-path rule is stated in five places (TICK §4 step 6, §8 output 3, §9 step 6, ADR 0011 §3 and its
  Consequences); round 1 found the two my grep for "record commit" missed, which say "skip review" instead
- Ignored: none
