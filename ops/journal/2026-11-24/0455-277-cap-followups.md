## 04:55 UTC #277 Review cap follow-ups: post-check conflicts, metrics count design final checks
- Session: worker ready #277
- Picked: rule i. The scheduler's step 10 pick (re-run after PR #315 merged mid-orient: same pick). No owner
  instruction pending (feed issues #238, #236, #176 are claimed, `deferred` or `owner-todo`; its comments are from
  before the FLOOR; closed owner issues #240 and #173 have their "done:" replies). main's `ci`: no failed run (the
  newest was in progress for f4d60f7). No proposal PR or protected-path merge needing an `owner-todo` change; no
  design ADR PR open.
- Did: PR #316. ops/TICK.md §4 step 6: an "after the last check" bullet and two state-table rows (a rule c pick after
  a clean final check or a passed §8 check: record-only commits and empty or record-only merges re-post `ai-review`;
  any other change is pushed, drafted, `ai-review` failure, ledger comment, then `blocked` + `owner-todo`).
  `cargo xtask metrics`: `**AI review, round K (design final check)` (K ≥ 5) counted apart from the round-5 final
  check (summary share, table column, `design_final_checks` and per-PR `design_final_check` in metrics.json), unit
  test with near misses; specs/tooling/metrics.md updated. Moved both STATUS pending lessons into ops/LESSONS.md.
  Removed the t/275-step6 worktree (PR #315 merged).
- Result: rounds 1–3 posted and triaged; round 3 had one minor and one nit (wording, fixed), so no round 4.
  `ai-review` success posted on this commit, auto-merge on.
- Cost: ~45% of context; review rounds 3/4; regraded 0, overturned 0; deferred: none
- Codex: skipped (the diff touches xtask/, ops/ and specs/, not crates/ or tools/)
- Lesson: none new (R1.1, "design can't act on a change with no finding", is in the TICK text now).
- Ignored: none.
