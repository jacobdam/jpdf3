# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #62 (#20 part 1): Arlington ingestion.
  - Contents: `tools/arlington` (`jpdf-arlington`): pinned fetch, TSV parser, model checks,
    generated tables. Also `cargo xtask arlington`, which `ci linux` runs last.
  - Spec: specs/tooling/arlington.md. ADR: 0006.
  - Reviewed: 1 round plus a delta review. The first `project-linux` run fetches the pinned
    commit, which takes about 10 s.
  - Part 2 (valid/boundary object generator) is #61, `ready`.

## Last session
- #59 (bench) merged. Main's `pages` deploy failed the known way ("No artifacts named
  github-pages" after a successful upload), so I ran `gh run rerun --failed 44837141465`.
- Implemented #20 part 1. Nothing from Arlington is committed: its NOTICE puts "other
  documentation" under CC-BY-4.0, and the TSVs could be either. Owner question: #60.

## Next step
- Check that main's `pages` deploy is green after #62 merges. Rerunning run 44837141465
  failed with "Multiple artifacts named github-pages" (attempt 1's artifact still exists).
- `ready` M0 issues: #22, #53, #54, #55, #61 (lowest number first). #14 closes after #55.
  #20 closes after #61.
- #22 (corpus tiers) must also keep the sealed tier off agent machines. specs/tooling/bench.md
  relies on that for CONSTITUTION 13.
- #16 (backend admission) can start. Every adapter must pass its
  `jpdf_traits::conformance` suite, add a fuzz target and add seeds (fuzz/README.md).
- #19 (nightly) needs #22. It runs `cargo xtask fuzz`, then `cargo xtask bench --corpus <regress>`,
  and adds the 5% regression check between reports that share a machine id (PLAN §5).
- #15 (metrics dashboard) reads `bench.json` (`schema: jpdf-bench/1`) and goes under `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.
- Audits bump `NIGHTLY` in xtask/src/fuzz.rs, refresh `fuzz/Cargo.lock`, and consider bumping
  the Arlington pin (`tools/arlington/src/fetch.rs` `COMMIT` + `TSV_TREE`).

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools, including libfuzzer-sys (NCSA).
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #60: may the Arlington TSVs (and tables generated from them) be committed?
- #21: the owner's fire-drill steps.
