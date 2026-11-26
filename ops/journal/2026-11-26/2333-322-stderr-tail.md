## 23:33 UTC #322 nightly: keep the tail of jpdf-cli's stderr, not the head
- Session: worker ready #322
- Picked: rule i because it was the lowest-numbered ready M0 issue with its dependencies closed (work mix below 70%,
  no ready product issue); the scheduler skipped #357 (design PR #359 open) and Design: product (PR #347 open).
- Did: PR #362. `jpdf_bench::engine::run_child` gains `Keep::{Head, Tail}`; the tail reader drains the pipe through a
  deque reserved once for the cap plus a stack read chunk, stops at its next read once the run is collected, and keeps
  nothing (still draining) if the reservation fails. `cargo xtask nightly`'s `exec` (regress runs, limits cases, the
  bench run) keeps the last 4 KiB; the bench's engine probes keep the head. The fake `jpdf-cli` now ends its 5000-byte
  log with a panic line the kept bytes must end with (fails with `Keep::Head`, checked by hand). specs/tooling/nightly.md
  items 4, 5, 12, Limits, Security, Self-check; specs/tooling/limits.md item 5. Removed the merged Planner worktree
  (`t/planner-2026-11-26`).
- Result: final check clean; `ai-review` success posted on this commit, auto-merge on.
- Cost: about 45% of the context; review rounds 4/4 plus the final check (round 5); regraded 0, overturned 0;
  deferred: none (every minor and nit was a quick fix).
- Codex: round 1 unavailable (the provider flagged the brief as possible cybersecurity risk, exit 1); rounds 2–4 ok;
  B/M/m/n = 0/6/0/0; unique confirmed: S2.1 (deque capacity outgrowing its length → reserve once, b5f4ce3), S2.2
  (head-mode comparison false, b5f4ce3), S3.1 (failed reservation ignored → cap 0, test with cap `u64::MAX`, 4185a41),
  S4.1 (heap chunk on the failure path → stack array, 1996538); S2.3 and S4.2 duplicated Opus' R2.1 and R4.1;
  Opus-only: R1.1 (bench message said "first 4 KiB", major), R1.2–R1.4, R3.1; disputed: 0. Codex was not run on the
  final check (one fresh reviewer, §4 step 6).
- Lesson: every Codex round after round 1 found one more false sentence about the tail buffer's memory or
  failure path; writing the allocation, failure and cancel behavior down before round 1 would have saved two rounds
  (STATUS Pending lessons). A Codex refusal on one brief doesn't predict the next: it ran on the delta briefs.
- Ignored: none.
