# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-28 (Planner rerun `planner new-issues`, session 2026-11-28T05:32Z; first run 00:04Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner. GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Design queue (step 6, before the ready list)
None open. #420 (tests of applied ops/proposals) closed with ADR 0014; its tasks are #423
(closed) and #424 to #427 below.

## Ready, in pick order (step 10: lowest number first, dependencies closed)
No `area:` labels exist yet (#308, M1), so no issue counts as a product issue and step 10 picks
lowest number first.
1. #16 Backend selection umbrella. Readied in this run: its parts #303, #413 and #304 are
   closed (ADR in PR #444), so what is left is to check its Deliverables against main and close
   it.
2. #19 Nightly umbrella. Readied in this run: its parts #300, #301, #302 and #383 are closed;
   check and close. Closing it unblocks #21.
3. #195 Proposal test-policy.sh: catch later `-c permissions` arguments and indirect writes
   (R3.1, R3.2), and audit 0005's CX.4, which a comment on the issue folds into the same fix
   (read the grant list declaratively instead of running the extracted block). Readied in this
   run: ADR 0014's issue table keeps it as "workable now" (the test runs against the live
   `ops/driver/codex-review.sh`), with no dependency on #424's move.
4. #424 ADR 0014 task 2: `cargo xtask protected-tests`, the manifest, and the tests moved to
   `ops/protected-tests/`. Readied in this run (#420, #366 and #423 closed). Strict lane.
5. #431 Dashboard contrast test: opacity, text-fill color and pseudo-element content. Readied in
   this run (a PR #430 review deferral, under 100 lines; `apps/web/`).
6. #442 `supply-chain/config.toml` imports `isrg`, which CONSTITUTION 9 doesn't list. Readied
   in this run: the work is an `ops/proposals/` file (protected path); `owner-todo` only once
   that proposal PR merges (ops/TICK.md §0).

## Waiting (labeled `ready`, a dependency still open)
- #21 (pilot fire drill) on #19. The owner's drill steps (#208) are all done, per the owner
  session's comment on #208 dated 2026-11-27.
- #425 (ADR 0014 task 3, `ci linux` on steps.rs), #426 (task 4, `ci macos` on steps.rs) and
  #427 (task 5, the proposal life cycle text) on #424. Readied in this run.

## In flight
None besides this Planner PR.

## Not workable now
- #15: the metrics dashboard umbrella. Its parts #305, #306, #353 and #369 are closed, but #431
  bears on its Deliverable "Charts accessible (labels, contrast, dark mode)", so it closes after
  #431 (PR #460 R1.1; `ready` removed again, with a comment on #15).
- #208: the owner's drill steps, a sub-issue of #21. All are done. The PR for #21 records them
  in docs/retros/0000-pilot.md, closes #21, and closes #208 with it.
- Set aside (`deferred`): #146.

## Sub-issues (step 2b)
#424 to #427 are already sub-issues of #420. #431 and #442 are a review deferral and a review
discovery, not parts, so they stay unlinked.

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The longest chain is ADR 0014's: #424 → #425, #426, #427 (one session each, about
four). The other chain is #19 → #21.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
work mix: 1 of the last 20 sessions on milestone work (5%; budget 70%)

The budget counts a session as milestone work only if a PR for its issue changes `crates/`,
`bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` or `tools/fsio/`. #431 (`apps/web/`) is the one
ready M0 issue expected to count. PLAN §8's M0 row has no product deliverable left without an
issue, so step 4a adds no product issues to M0. No issue moved to `Ops`: #195, #424 to #427 and
#442 are about `ops/proposals/` and protected-file tests (a strict-list subject, TICK §5
Discoveries term 4), and #21 is M0's exit criterion "pilot fire drill passed" (term 1). The mix
is expected to rise only once M0 closes and M1 (syntax and security) opens.
