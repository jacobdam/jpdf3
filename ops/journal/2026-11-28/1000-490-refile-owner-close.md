## 10:00 UTC #490 re-file only design tasks the push or rejection rule closed
- Session: none (owner interactive session, a worker started from it; #490 was claimed for it and is an owner M0 pick, https://github.com/jacobdam/jpdf3/issues/493#issuecomment-7236958033)
- Picked: the owner's request in the interactive session; #490 is M0 (owner pick), strict lane (the owner-veto window, ADR 0012 §1)
- Did: PR #504: ops/TICK.md §8 step 5 gets "Only a task this step closed is filed again": the push and rejected-design rules close with fixed comment starts (the push form is the one already on #467–#477), and only a not-planned task carrying one (by `jacobdam`, naming a `design` PR, no `reopened` event after it in the paginated timeline, newest copy decides) is filed again; any other close, the owner's included, stands. §9 step 1b's skip rule points to it. No spec or xtask code restates the rule.
- Result: round 1: 2 minors and 2 nits fixed; round 2 (delta): 1 nit (PR description) fixed; `ai-review` success posted on this commit, auto-merge on
- Cost: about 30% of the context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` change)
- Lesson: none (the live check that mattered: #467–#477's existing close comments had to match the new push form, or step 1b would never file ADR 0017's tasks from merged PR #465)
- Ignored: none
