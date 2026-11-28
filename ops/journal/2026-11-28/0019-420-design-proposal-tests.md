## 00:19 UTC #420 Design: tests of applied ops/proposals (ADR 0014)
- Session: design-technical design-issue #420
- Picked: rule step 6 (Design: technical, ops/TICK.md §8) because #420 was the only `design` and `ready` issue and steps 1–5 had nothing
- Did: ran the eight tests of the seven applied proposals on macOS against main's protected files (keep-open-worktrees: 8 cases fail, #366; codex-review-isolation/test-codex.sh aborts since PR #291, a new finding; the rest pass). Output 2: ADR 0014 (docs/adr/0014-tests-of-protected-files.md) and principle P5 in PR #422: tests of protected files move into one manifest under `ops/protected-tests/`, run against the live files by path on each target's platform by `cargo xtask protected-tests`, strict lane, proposals carry changed tests until the owner's apply commit moves them. Filed tasks #423–#427 as sub-issues of #420 (no `ready`); `owner-todo` and the 24-hour window comment on #420.
- Result: PR #422 ready, `ai-review` success, labeled `design`, no auto-merge (the window); `worker design-window #422` once 24 h pass or the owner `/approve`s
- Cost: ~45% of context; review rounds 2/4; regraded 0, overturned 0; deferred: none (all 11 findings fixed)
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
