# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #58 (#17): fuzz infrastructure.
  - Contents: `fuzz/` with 3 targets, `tools/jpdf-testgen` seeds, `cargo xtask fuzz`, and a
    short fuzz pass inside `cargo xtask ci linux`.
  - Spec: specs/tooling/fuzz.md. ADR: 0005.
  - Reviewed: one fix round plus a delta review.
  - Watch the first `project-linux` run. It installs nightly-2026-11-20 and cargo-fuzz
    0.13.2, which takes about 2 extra minutes.

## Last session
- Merged #56 (jpdf-traits 1/4).
- Implemented #17. Found that the judge's nightly fuzz step has no seeds, so the template
  targets (`from_fuzz_chunks`) do most of the work there.

## Next step
- `ready` M0 issues: #18, #20, #22, #53, #54, #55 (lowest number first). #14 closes after #55.
- #16 (backend admission) can start; #56 has merged. Every adapter must pass its
  `jpdf_traits::conformance` suite, add a fuzz target and add seeds (fuzz/README.md).
- #19 (nightly) is unblocked by #17 once #18 and #22 land.
- #15 (metrics dashboard) can put its pages under `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.
- Audits now also bump `NIGHTLY` in xtask/src/fuzz.rs and refresh `fuzz/Cargo.lock`.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools. It now also covers libfuzzer-sys
  (NCSA), which ADR 0005 uses.
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner's fire-drill steps.
