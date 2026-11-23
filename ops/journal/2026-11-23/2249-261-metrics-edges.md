## 22:49 UTC #261 Audit 0006 tooling 1: metrics edge cases, fuzz exit test, capture boundary test
- Picked: i, `worker ready #261` (first line of `cargo xtask next-session`). a: the owner feed holds only comments
  from before the 2026-11-23 floor; b: no red main run; c: no failing own PR (#285 is another session's, in review).
- Did: PR #286. metrics: retro lag and default `--since` both use the last retro record dated on or before `--until`
  (AC.3, R1.1); regrades count only reported IDs (AC.4); the one-day period overlap and the gap after a late record
  merge are stated in the spec (AC.5, R1.2); index exit codes and durations digits only, GitHub timestamps exactly
  `YYYY-MM-DDTHH:MM:SSZ` (AC.8); merged PRs with 100 comments flagged in the report and `comments_truncated` (CX.10,
  gh 2.96 asks for `comments(first: 100)`); a live run without an index deletes a stale `raw/index.tsv` (CX.11).
  fuzz: `run_result` and `ci_result` extracted and tested on every arm (AC.7). macos: `capture` 16 MiB / 16 MiB + 1
  boundary test on both pipes (AC.2). Specs metrics.md and fuzz.md updated. Live runs: `--until 2026-11-22` gives
  lag 1 (was -1), period 2026-11-21 to 2026-11-22.
- Result: `ai-review` success on 2ac4306 after round 2; records commit added after review; auto-merge on.
- Cost: about 30% of context; review rounds 2/4 (R1: 1 major, 3 minors, all fixed; R2 clean); regraded 0,
  overturned 0; deferred: none.
- Codex: skipped (xtask/ only, no crates/ or tools/ change).
- Lesson: a comment saying a new rule matches an existing one ("skips them the same way") was false because the old
  rule dropped every record while the new one fell back to the previous one; a fixture where both rules gave the same
  day hid it. Test such a pair on an input where the two rules differ.
- Ignored: none.
