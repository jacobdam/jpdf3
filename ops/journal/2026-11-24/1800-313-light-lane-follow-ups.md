## 18:00 UTC #313, #319 (PR #330, PR #329): ADR 0012's moves and the first light-lane PRs
- Session: owner interactive session (no `cargo xtask next-session` pick)
- Picked: rule a because the owner asked to work ADR 0011's remaining items with ADR 0012, then chose "build the Ops follow-ups, light lane"
- Did: ADR 0012 §3–§4 moves (normally the next Planner's): created milestone `Ops` (#12); #308, #248 → M1 (#248's and #251's `Depends on:` now #308), #250 → M3, #251 → M4, #247 → `Ops` + `deferred`, #313, #314, #319, #328 → `Ops`, a comment on each. PR #330 (#313): one assertion group in `step8_design_product`, hand mutant fails it. PR #329 (#319): `Delete tag mN` filed only when the undo reopens `Mn`. Unclaimed #314: strict lane (it computes ADR 0011 §7's owner-veto window, on ADR 0012's strict list).
- Result: both PRs `ai-review` success after round 1, auto-merge on. M0 28 → 20 open (ADR 0012 said 27 → 19: its count missed one issue).
- Cost: ~15% of context; review rounds 1/2 each; regraded 0, overturned 0; deferred: none (PR #330 R1.1–R1.2 and PR #329 R1.1–R1.2 fixed)
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: PR #329's first fix patched the case (a new destination) and round 1 found the side effect; the issue's other option (drop the case) closed both findings. Same as PR #327: when a rule's edge case needs a patch, try deleting the case first.
- Ignored: none
