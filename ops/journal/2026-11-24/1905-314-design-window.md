## 19:05 UTC #314 (PR #333): next-session step 3, an own design PR past its 24-hour window
- Session: owner interactive session (no `cargo xtask next-session` pick)
- Picked: rule a because the owner said "You do the rest items, i will approve" (#311 and #314 left of ADR 0011)
- Did: strict lane (ADR 0011 §7, on ADR 0012's strict list). Step 3's third term `worker design-window #N`: `prs.json` reads `isDraft`, `autoMergeRequest`, `createdAt` and the `ai-review` `startedAt`; new input `design-ready.json` (timeline `ready_for_review` events); the `/release` time through `newest_commands`, which `held_prs` now shares. TICK §1's by-hand check removed; spec, AUTOMATION.md §4.1 updated. Round 1 fixes: owner-command times validated, a PR listed once in `holds.json`, `blocked`/`deferred` on the linked issue stop the pick.
- Result: `ai-review` success posted on this commit, auto-merge on. 22 + 2 hand mutants: all but 2 (judged equivalent) fail a test.
- Cost: ~15% of context (implementation in a fork); review rounds 2/4; regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none new (the safe direction for an ambiguous filter, R1.3, was the quick fix).
- Ignored: none
