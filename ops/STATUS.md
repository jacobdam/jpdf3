# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #59 (#18): bench harness.
  - Contents: `tools/bench` (`jpdf-bench`) and `cargo xtask bench`.
  - Spec: specs/tooling/bench.md.
  - Reviewed: 1 round plus 2 delta reviews. Auto-merge is on once `ai-review` is posted.
  - Not run in `ci linux`: the release build is too slow there. Nightly (#19) calls it.

## Last session
- #58 (fuzz) merged; main green.
- Implemented #18. The bench reports `not_implemented` (never a zero time) for the M0 stub.
  Locally, the info ratio against mutool 1.28.6 was 0.60.

## Next step
- `ready` M0 issues: #20, #22, #53, #54, #55 (lowest number first). #14 closes after #55.
- #22 (corpus tiers) must also never put the sealed tier on agent machines. specs/tooling/bench.md
  relies on that for CONSTITUTION 13.
- #16 (backend admission) can start. Every adapter must pass its
  `jpdf_traits::conformance` suite, add a fuzz target and add seeds (fuzz/README.md).
- #19 (nightly) needs #22. It runs `cargo xtask fuzz`, then `cargo xtask bench --corpus <regress>`,
  and adds the 5% regression check between reports with the same machine id (PLAN §5).
- #15 (metrics dashboard) reads `bench.json` (`schema: jpdf-bench/1`) and goes under `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.
- Audits bump `NIGHTLY` in xtask/src/fuzz.rs and refresh `fuzz/Cargo.lock`.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools, including libfuzzer-sys (NCSA).
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner's fire-drill steps.
