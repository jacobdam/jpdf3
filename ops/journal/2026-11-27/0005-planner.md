## 00:05 UTC Planner 2026-11-27: ready #360 #361 (ADR 0013's tasks)
- Session: planner due
- Picked: rule j (Planner, step 8) because the work mix is below 70% (0 of the last 20 sessions on milestone work) and no Planner record was dated 2026-11-27. The first `cargo xtask next-session` run exited 2 (`gh pr list` returned truncated JSON); a retry a minute later printed this pick.
- Did: `ready` on #360 and #361 (ADR 0013 tasks 1 and 2, filed by the #357 design session without `ready`; sized at about 700 and 300 added lines, each naming its dependency), with a comment on each; rewrote ops/QUEUE.md: #350 and #352 are gone (folded into #360, #361), #322 closed (PR #362 merged at 00:06, during this session; its stale `in-progress` and `ready` labels removed), #302 now waits on #360 only, and the critical path now runs through #357 → #360. No dead claims (#322's claim has PR #362), no milestone moves, no new design issue (the nightly group is #357's), no product issues to add under step 4a (PLAN §8's M0 row has none left without an issue).
- Result: see the PR's records commit.
- Cost: about 20% of the context; review rounds 1/2 (light lane); regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
