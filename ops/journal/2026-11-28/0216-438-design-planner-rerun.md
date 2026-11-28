## 02:16 UTC #438 Design: Planner reruns for issues filed after the day's run (ADR 0015)
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/438#issuecomment-7232225885
- Picked: rule a because the owner commented "Look ok" on #438, a tagged design issue filed in an owner interactive session, which made it an owner instruction (ADR 0011 §7). The instruction is the issue's design question, so this session ran it as a Design: technical session (ops/TICK.md §8, output 2).
- Did: posted "picked up" and claimed #438. PR #439 adds docs/adr/0015-planner-reruns-for-new-issues.md and P6 in docs/PRINCIPLES.md. The Planner stays the only gate into `ready`, capped at 3 runs per UTC day. After the day's first run, step 11 picks `planner new-issues` only when nothing else applies and a current-milestone issue without `ready` (no skip, claim or design label, not in flight) was created after the newest Planner record's minute. #438 options 2 and 3 are rejected in the ADR. Removed the merged worktree t/433-done-record.
- Result: PR #439: review rounds 1 and 2 had no blocker or major (9 minors and nits, all fixed). Steps after this record commit: mark ready, `ai-review` success on this commit, file task 1 (`Designed in: #438`, `Depends on: #438`, sub-issue of #438, no `ready`), `owner-todo` on #438 and the 24-hour window comment. No auto-merge: `worker design-window #439` enables it after the window or an owner `/approve`. The "done:" reply to the owner's comment waits for the merge.
- Cost: about 45% of the context; review rounds 2/4 (strict); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
