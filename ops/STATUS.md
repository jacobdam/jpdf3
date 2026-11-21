# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- Nothing. #62 (#20 part 1, Arlington ingestion) merged; part 2 is #61 (`ready`).

## Last session
- Main went red after #59 when the `pages` deploy flake hit (it first hit after #39). The
  rerun of #59's run then failed on duplicate artifacts. #62's push run 44840300299 is green.
- Wrote ops/proposals/pages-deploy-retry.md (per-attempt artifact name + one deploy retry).
  When this flake hits again, don't rerun; the next push to main redeploys.

## Next step
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
- Proposal for the owner: ops/proposals/pages-deploy-retry.md (ci.yml is protected).
