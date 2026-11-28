## 08:11 UTC #478 Design ADR tasks are filed when the window ends; a rejected design closes them
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/478#issuecomment-7235603476
- Picked: rule a because the owner's "Ok" on tagged issue #478 (and their later comment on it about GitHub's approve and merge buttons, https://github.com/jacobdam/jpdf3/issues/478#issuecomment-7236076394) had no "done:" reply
- Did: PR #489 (strict lane). ops/TICK.md §8 step 5: tasks are filed by the session that enables auto-merge (the `design-window` pick or a record-only update), which posts `tasks filed: … (PR #N)`; the window comment says so; step 6 and output 2 follow. The owner's buttons: new §9 step 1b has every Planner run file the tasks of a `design` PR merged in the last 14 days with no `tasks filed:` comment by `jacobdam` (a later `tasks closed:` voids it). A rejected design closes its tasks (closing session, or step 1b), a split passes them on, a non-record push closes them and posts `tasks closed:`. Step 11's rerun needs every `Depends on:` issue closed (xtask/src/next_session.rs, test `step11_rerun_needs_every_dependency_closed`, hand mutant killed). ADR 0017's #467–#477 are named for the one-time case (note on #236). Specs, ADR 0015 §3 and PRINCIPLES P6 updated; the two pending lesson files moved into ops/LESSONS.md.
- Result: `ai-review` success posted on this commit, auto-merge on. "done:" replies on both owner comments wait for the merge (TICK §0).
- Cost: ~55% of context; review rounds 3/4; regraded 0, overturned 0; deferred: #490
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
