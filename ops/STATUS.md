# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #65 (#22 part 1, corpus tiers): `tools/corpus` + `tools/corpus/manifests/regress.tsv`
  (5250 files from 11 pinned git suites). Auto-merge once CI and ai-review pass. Part 2 is #64.

## Last session
- #22 part 1: `jpdf-corpus fetch <smoke|regress>` downloads into `~/.cache/jpdf-corpus/<tier>/`
  (judge layout) with blob-id checks; `build regress` regenerates the manifest from the pins
  in `tools/corpus/src/sources.rs`. Reviewed: 9 findings, all fixed.

## Next step
- `ready` M0 issues: #53, #54, #55, #61, #64 (lowest number first). #14 closes after #55.
  #20 closes after #61. #22 closes after #64.
- #19 (nightly) can now start: `jpdf-corpus fetch regress` (exit 75 = infra), then
  `cargo xtask fuzz`, `cargo xtask bench --corpus ~/.cache/jpdf-corpus/regress`, and the 5%
  regression check between reports that share a machine id (PLAN §5).
- #16 (backend admission) can start. Every adapter must pass its
  `jpdf_traits::conformance` suite, add a fuzz target and add seeds (fuzz/README.md).
- #15 (metrics dashboard) reads `bench.json` (`schema: jpdf-bench/1`) and goes under `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.
- Audits bump `NIGHTLY` in xtask/src/fuzz.rs, refresh `fuzz/Cargo.lock`, consider bumping
  the Arlington pin, and may bump corpus pins (`sources.rs`, then `build regress`).
- Housekeeping for the next audit: 22 remote `t/*` branches of merged PRs can be deleted.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools, including libfuzzer-sys (NCSA).
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #60: may the Arlington TSVs (and tables generated from them) be committed?
- #21: the owner's fire-drill steps.
- Proposal for the owner: ops/proposals/pages-deploy-retry.md (ci.yml is protected).
