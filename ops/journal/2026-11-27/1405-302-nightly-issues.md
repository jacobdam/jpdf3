## 14:05 UTC #302 / PR #384 nightly 3/3 part 1: the issues step
- Session: worker ready #302
- Picked: rule i because #302 is the lowest-numbered ready M0 issue with its dependencies (#300, #301, #322, #360) closed (no product issue is ready)
- Did: split #302 before the work (sizing rule): part 1 here, part 2 (fuzz and bench findings) filed as #383 without
  `ready`, depending on #361. PR #384: new `xtask/src/nightly/issues.rs` (findings `limits:<case>`, `regress:<blob>`,
  `regression:<blob>:<op>`; tagged bodies with a `nightly-signature:` line and no stderr; dedup over open `bug` and
  `crash` issues; at most 20 writes a night, creates first; `RealGh` with a 120 s limit and a 64 MiB stdout cap), an
  `issues` step last in the driver (after a stop too, once the limits part ran), `limits::run_results`, and the
  `--file-issues` flag (off by default, so a local session run never writes to GitHub). Spec item 16 and items 1, 15,
  Errors, Limits, Security, Tests. ops/proposals/nightly-file-issues.md adds the flag to the CI job (protected). A
  local run with `--limit 3 --bench-minutes 0 --fuzz-seconds 0` ended with `issues: 0 findings, not filed`, exit 0.
- Result: round 1 had 3 majors (R1.1 comments starved new findings under the cap; R1.2 a limits case that changes
  label got a second issue; R1.3 the stdout cap had no test), 3 minors and 2 nits, all fixed in 80d9195. Round 2
  approved with 2 nits, fixed in 2ca1f07 (wording only, so no round 3). `ai-review` success posted on this commit,
  auto-merge on
- Cost: ~55%; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: a tool that files issues across nights → test two nights over the cap and a finding whose label changes
  (added to ops/LESSONS.md, Process). The STATUS pending lesson (#382 R1.1) was not moved into LESSONS before round
  1 as §5 asks; it stays pending for the next work PR
- Ignored: none
