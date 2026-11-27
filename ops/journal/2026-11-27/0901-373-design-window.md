## 09:01 UTC #373 Design window: a record-only update keeps the window's start
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/373#issuecomment-7214155779
- Picked: rule a because the owner commented "Go ahead" on #373 (an issue their interactive session filed); "picked up" posted.
- Did: PR #374. `cargo xtask next-session`'s design-window term counts the `ai-review` success from the triage comment its `targetUrl` links (same PR, `jacobdam`, header `🤖 jpdf-agent **AI review triage`), else from the status time; tests for the kept window, the restart after a new round (two triage comments), `/release`, near misses and malformed input. ops/TICK.md §4 "after the last check", §8 step 5 (record-only update keeps the window; enable auto-merge in the same session once it has passed, with the pick's terms), §10 step 6; ADR 0011 §7; specs/tooling/next-session.md. Item 4 (records out of STATUS for design PRs) not done, reason in the PR.
- Result: `ai-review` success posted on this commit, auto-merge on. "done:" on #373 waits for the merge.
- Cost: ~45% of context; review rounds 2/4; regraded 0, overturned 0; deferred: none (R2.1 minor fixed)
- Codex: skipped (no crates/ or tools/ path)
- Lesson: a near-miss fixture built with a broad `replace` ("jacobdam" → "someone") also changed the URL, so the test passed for the wrong reason; a hand mutant of the author check caught it.
- Ignored: none
