## 23:14 UTC #357 Design: multi-step xtask tools report each step (ADR 0013)
- Session: design-technical design-issue #357
- Picked: step 6 (Design: technical, ops/TICK.md §8) because #357 was the only `design` + `ready` issue and no `needs-design` PR was open
- Did: PR #359 (output 2): docs/adr/0013-multi-step-tools-report-each-step.md and principle P4 in docs/PRINCIPLES.md. The nightly folds step results in three places (`exit_code`, `with_limits`, `extra::run`'s `bool`), so #350 and #352 can't be tested; the ADR adds one shared `steps::Outcome`, one driver per tool over injected steps returning the log, and the exit code as a pure function of the log (1 > 75 > 4 > 0). Tasks: 1, steps.rs and the nightly driver (folds #350); 2, extra's four parts as driver steps (folds #352); #302 gains a dependency on task 1. `ci linux` is the named exception until a PR changes its checks.
- Result: `ai-review` success posted on this commit; after it the tasks are filed, #350 and #352 closed as folded, `owner-todo` on #357 with the 24-hour window comment; no auto-merge (the scheduler's `worker design-window #359` pick enables it).
- Cost: about 35% of the context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none (all 10 findings fixed)
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
