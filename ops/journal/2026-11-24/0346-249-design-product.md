## 03:46 UTC #249 / PR #312 Design: product session and its next-session trigger (escalated to design)
- Session: worker ready #249
- Picked: rule i because #249 was the lowest-numbered ready M0 issue with its dependencies closed (#246 is in
  flight as PR #309). Step 6 checked by hand: no `needs-design` PR, no ready `design` issue. Main green (ci
  45318618850). No pending owner comments after the feed floor; the closed owner issues #172, #173, #240 predate it.
- Did: split #249 by the sizing rule (the owner-approval wait of v1 would have held the scheduler code): #249 is
  part 1/2, filed #311 (part 2/2, `docs/PRODUCT.md` v1, label `design-product`); created the `design-product` label.
  PR #312: next-session step 8 rule k after the Planner's terms (`design product v1 [#N]` while `docs/PRODUCT.md` is
  not on main, read from `main-log.txt`; `design product #N` for a ready `design-product` issue once it is; one
  product PR at a time via step 7's slot rule; step 10 skips `design-product`), ops/TICK.md §10 (new) plus §0, §2,
  §8 and §9 hooks, AUTOMATION.md, specs/tooling/next-session.md, tests (`step8_design_product`, a precedence entry;
  the base log's root commit now adds `docs/PRODUCT.md`). LESSONS: refined the PR #289 entry.
- Result: escalated to design (ops/TICK.md §4 step 6): PR #312 back to draft, `ai-review` failure, `needs-design`,
  the ledger on #249. Rounds 1–4: 2+2+3+2 majors (all fixed), the final check approved R4.1–R4.3 but reported R5.1
  (minor: no assertion that a waiting `ready` gap issue doesn't block v1), and §4 step 6 escalates on any finding.
- Cost: about 70% of context; review rounds 4/4 plus the final check; regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: the over-cap routing of a session type with its own merge rule (owner approval, the 24-hour window) drew
  R2.1 and R3.1–R4.2, each fix opening the next; moved into ops/LESSONS.md (PR #289's entry).
- Ignored: none.
