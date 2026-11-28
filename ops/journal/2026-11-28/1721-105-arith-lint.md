## 17:21 UTC #105 / PR #546 Lint: deny clippy::arithmetic_side_effects workspace-wide
- Session: worker ready #105
- Picked: rule i because #105 was the lowest-numbered ready issue in M1 with no open dependency (the work mix is below 70% and no ready product issue had its dependencies closed).
- Did: PR #546: `arithmetic_side_effects = "deny"` in the root `[workspace.lints.clippy]` (every member inherits it, so every target: libraries, binaries, unit and integration tests, xtask, tools; `fuzz/` is its own workspace); rewrote the 170 sites it found (jpdf-traits `check_scene` and conformance suites/toys, engine tests, arlington, corpus, testgen, xtask) with checked/saturating ops or iterators that keep every value; kept the four #77 crate-root `#![deny]`s. specs/cli/jpdf-cli.md now states what the lint covers and skips. Folded the pending lessons of #78 and #87 into ops/LESSONS.md. Removed the merged worktree `t/87-mutants`. Filed #547 (Ops: add the lint to the judge's `REQUIRED_CLIPPY`, a proposal).
- Result: `ai-review` success posted on this commit, auto-merge on. Local: fmt, clippy `-D warnings`, nextest 764/764.
- Cost: about 45% of context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: #547
- Codex: unavailable: usage limit (codex exit 1); B/M/m/n = 0/0/0/0; unique confirmed: none; Opus-only: R1.1 (false coverage claim, spec narrowed); disputed: 0
- Lesson: none
- Ignored: none
