# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #71 (#54, Rasterizer trait + suite): auto-merge once CI passes; ai-review is posted.

## Last session
- #54: added `Rasterizer` (display list → premultiplied RGBA8), `RasterLimits`, `check_scene`
  and `conformance::raster::run`, tested against a toy and 11 broken variants. Review:
  1 blocker (an overflow panic on image size) + 14 others, all fixed; the delta review
  approved. ColorTransform → #70. Discoveries for `jpdf-render`: #72, #73.
- The worktree `.claude/worktrees/license-amend` (`t/50-test-only-licenses`) holds
  uncommitted CONSTITUTION.md edits, apparently the owner's. Agents must not touch it.

## Next step
- **An audit is overdue** (25+ merged PRs, none yet; TICK §2 i, §6). TICK's strict order
  (g before i) keeps skipping it, so take the audit next. A retro should also reorder the
  rules.
- `ready` M0 issues: #55, #61, #64, #66, #70, lowest number first. #14 closes after #55 and
  #70, #20 after #61, #22 after #64.
- #19 (nightly) can start: `jpdf-corpus fetch regress` (exit 75 = infra), then
  `cargo xtask fuzz`, `cargo xtask bench --corpus ~/.cache/jpdf-corpus/regress`, and the 5%
  regression check between reports that share a machine id (PLAN §5).
- #16 (backend admission) can start. Every adapter must pass its
  `jpdf_traits::conformance` suite, add a fuzz target and add seeds (fuzz/README.md).
  Font adapters must also supply hostile programs per format and a multi-FD CID fixture.
- #15 (metrics dashboard) reads `bench.json` (`schema: jpdf-bench/1`) and goes under
  `site/metrics/`.
- #21 (fire drill) waits on #19. #45 (wireframes) is low priority.
- Audits bump `NIGHTLY` in xtask/src/fuzz.rs, refresh `fuzz/Cargo.lock`, consider bumping
  the Arlington pin, may bump corpus pins, and should delete the remote `t/*` branches of
  merged PRs.

## Blockers / owner-todo
- #50: license ruling for test- and build-only tools, including libfuzzer-sys (NCSA).
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #60: may the Arlington TSVs (and tables generated from them) be committed?
- #21: the owner's fire-drill steps.
- Proposal for the owner: ops/proposals/pages-deploy-retry.md (ci.yml is protected).
