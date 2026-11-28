## 01:30 UTC #383 Nightly issues: file fuzz crashes and bench timeouts
- Session: worker ready #383
- Picked: rule i because it was the lowest-numbered ready M0 issue with its dependencies (#302, #361) closed (work mix below 70%, no ready product issue)
- Did: PR #434. `xtask/src/nightly/issues.rs` gains `extra_findings` (per failed fuzz target: `fuzz:<target>:<artifact>` per copied artifact whose name is 1 to 100 bytes of a-z, 0-9, `_`, `-`, plus `fuzz:<target>` when none or any other; `slow-unit-` files never named; a clean target not filed; `bench:timeout` with label `perf`), `night_findings` (filing order) and a `perf` dedup list only on a night with a perf finding. The driver keeps the bench and fuzz results and hands them to `issues`. Spec item 16 updated; a bench drop between nights is stated as not filed (no previous `bench.json`), so no `--previous-bench` flag. The driver fake's five `issues` strings gained the bench/fuzz suffix (explained in the PR). Hand mutants of every new condition fail a test. Removed the merged worktrees t/369-dashboard-tests and t/420-proposal-tests (scheduler notes).
- Result: `ai-review` success posted on this commit, auto-merge on (strict lane; round 1: 2 minors, 1 nit, all fixed; round 2 delta: 1 minor, a wording fix, so no round 3)
- Cost: about 35% of the context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
