## 15:00 UTC #326 ADR 0012: a light lane for process PRs, the Ops milestone, ADR 0011 tasks out of M0
- Session: owner interactive session (not picked by `cargo xtask next-session`)
- Picked: the owner asked in the session why M0 grows and to relax the rules for the workflow engine, then "Do it"
- Did: filed #326 (tagged, `human`; the owner's untagged comment on it makes it an instruction, ADR 0011 §7). PR #327:
  docs/adr/0012-light-lane-for-process-prs.md, ops/TICK.md §0, §3, §4 (Lanes, steps 4–6), §5, §6, §7, §9, AUTOMATION.md
  §4.1, §6.8, §7, ADR 0010/0011 status lines, specs/_TEMPLATE.md. Filed #328 (lane-aware metrics, R1.4).
- Result: rounds 1–3 posted and triaged (round 1: 5 majors, round 2: 4 majors, round 3: approved, 1 minor fixed as
  wording). `ai-review` success posted on this commit; auto-merge left off: the owner merges to accept the ADR.
- Cost: ~45% of context; review rounds 3/4; regraded 0, overturned 0; deferred: #328
- Codex: skipped (no `crates/` or `tools/` change)
- Lesson: none new. The PR is its own example: each round found another edge case in a lane rule; dropping the
  `xtask/src/main.rs` exception instead of patching it is what converged.
- Ignored: none.
