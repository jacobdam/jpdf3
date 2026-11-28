## 11:42 UTC #529 / PR #530 Design: technical for escalated PR #524 (ADR 0020 revised)
- Session: design-technical needs-design #524
- Picked: step 6 (Design: technical, ops/TICK.md §8) because PR #524 (ADR 0020 for #522) was escalated to design after its round 5 final check (R5.1 major, R4.2 not closed); #522's design step was not spent.
- Did: §8 output 2. Filed design issue #529 (`design`, `Designed in: #522`), opened PR #530 (label `design`, strict lane) with a revised docs/adr/0020-owner-feed-session.md and P10: review bodies are no owner items and keep today's per-item pick (behind an older unrouted item on the same PR), and a routed PR, or the PR of a route issue, is picked through one failing-own-PR function shared with step 3, so a skip label parks both picks; owner issues are routed only by their own `owner-work` label; the rework route no longer drafts the PR. Closed PR #524 unmerged (no tasks had been filed) and #522 as folded into #529. Carried PR #524's session record (journal 1047 and its pending lesson) unchanged into PR #530. Removed the worktree of PR #524.
- Result: rounds 1 to 4 (2, 2, 1 and 1 majors, all fixed) and a clean round 5 final check; `ai-review` success posted on this commit, PR marked ready, `owner-todo` on #529 for the 24-hour window (tasks are filed when it ends). The `done:` for https://github.com/jacobdam/jpdf3/issues/522#issuecomment-7238115763 waits for PR #530's merge (its body links it).
- Cost: about 55% of the context; review rounds 4/4 plus the final check (round 5); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: see ops/lessons/pending/2026-11-28-1142-529-owner-feed-design.md
- Ignored: none
