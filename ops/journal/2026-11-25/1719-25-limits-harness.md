## 17:19 UTC #25 Adversarial resource-limit harness (`cargo xtask limits`) in the nightly
- Session: worker ready #25
- Picked: rule i because #25 is the lowest M0 `ready` issue with its dependency (#300) closed; the work mix (0 of 20)
  has no ready product issue in M0, so the lowest-numbered ready issue.
- Did: PR #349: `xtask/src/limits.rs` streams seven ~500 MiB attack files (nested-strings, shared-body, stacked-prev,
  xref-entries, max-per-object, dense-kids, long-comment) and runs `jpdf-cli info` on each under `ulimit -v 2097152`
  and 20 s; hostile cases must exit 2 with their defence's error, valid ones exit 0; `limits.json`; exit 4 on a failed
  case. `cargo xtask nightly` runs it before the fetch (specs/tooling/nightly.md items 6, 10, 11). Spec
  specs/tooling/limits.md, TRACE R1, jpdf-cli.md §Limits pointer. Live: 7/7 pass at 500 MiB in ~12 s; a hand mutant
  disabling the overlap checks fails 3 cases. Removed the dead worktree `planner-1125` (PR #346 merged). Filed #350.
- Result: `ai-review` success posted on this commit, auto-merge on.
- Cost: about 55% of context; review rounds 2/4; regraded 0, overturned 0; deferred: #350
- Codex: skipped (no `crates/` or `tools/` path in the diff); B/M/m/n = 0/0/0/0
- Lesson: a scaled generator's count from a rounded per-item estimate (40 MiB per pair) broke 5 in-range `--mib`
  values; R1.1 found it by simulating every value (Pending lessons in STATUS).
- Ignored: none
