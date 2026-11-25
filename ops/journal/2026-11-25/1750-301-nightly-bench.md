## 17:50 UTC #301 cargo xtask nightly 2/3: bench and extra fuzz time in nightly.json
- Session: worker ready #301
- Picked: rule i because the scheduler's step 10 named #301 (the lowest M0 `ready` issue with its dependency #300 closed and no open PR; no product issue is ready, work mix below 70%). Rules a–h: nothing applied (main green, no own PR failing; PR #349 for #25 and #347 for #331 were other sessions' and open).
- Did: PR #351. `cargo xtask nightly` now runs the bench (jpdf-bench on the first 20 verified regress files, under the memory wrapper and `--bench-minutes`, default 20) and extra fuzz time (`--fuzz-seconds` per target, default 1200, artifacts copied to `DIR/fuzz/<target>/`), then writes `DIR/nightly.json`. New `xtask/src/nightly/extra.rs` with tests; `bench::build`/`bench_args` and `fuzz::run_report`/`Report.started` split out. specs/tooling/nightly.md items 11–14. Moved STATUS's four pending lessons into LESSONS. Filed #352 (R2.1, no `ready`).
- Result: `ai-review` success posted on this commit, auto-merge on. 213 xtask tests, 596 workspace tests pass; local run (`--limit 30 --bench-minutes 5 --fuzz-seconds 10`) exit 0 in 58 s.
- Cost: ~45% of context; review rounds 2/4; regraded 0, overturned 0; deferred: #352
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: `cargo` was not on PATH in this session's shell (`~/.cargo/bin` missing); `export PATH=$HOME/.cargo/bin:$PATH` before `cargo xtask next-session`. A test of "removal failure is a tool failure" whose blocking state also breaks the later write proves nothing (R2.1): pick a state that fails only the step under test, or make the function report which step failed.
- Ignored: none
