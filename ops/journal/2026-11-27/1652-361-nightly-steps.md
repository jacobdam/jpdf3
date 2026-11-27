## 16:52 UTC #361 / PR #411 nightly steps 2/2: the bench, the fuzz pass and nightly.json as driver steps
- Session: worker ready #361
- Picked: rule i because #361 was the lowest-numbered ready issue in M0 with its dependency (#360) closed (no
  ready product issue)
- Did: `extra::run` and its `bool` removed; `stale-summary`, `bench`, `fuzz` and `summary` are steps of `drive`
  (xtask/src/nightly.rs), each with its own `Outcome`, mapped by pure adapters in xtask/src/nightly/extra.rs
  (`remove_summary`, `bench_outcome`, `fuzz_outcome`, `write_summary`); driver tests pin item 14's order, the data
  handed to `summary` and #352's case; specs/tooling/nightly.md items 14, 15, Errors, Security and Tests and P4's
  "Enforced by" updated. Linked #361 as a sub-issue of its design issue #357. Hand mutants (driver records `Ok` for
  the removal, wrong fuzz result to `summary`, `timeout` as failure) all caught. Real runs: `--limit 3
  --bench-minutes 1 --fuzz-seconds 0` exit 0; a directory at `nightly.json` gives exit 1 with separate removal and
  write lines. Removed the merged worktrees t/388-done-records (PR #409) and t/406-done (PR #410).
- Result: `ai-review` success posted on this commit, auto-merge on; 674/674 workspace tests locally
- Cost: ~35% of context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no crates/ or tools/ path)
- Lesson: none
- Ignored: none
