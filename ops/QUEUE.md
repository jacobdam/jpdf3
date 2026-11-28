# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-28 (Planner run, session 2026-11-28T00:04Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner. GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Design queue (step 6, before the ready list)
- #420 Design: tests of applied ops/proposals. Filed today (step 2a) for the follow-up chain
  #342 → #344 → #366, #367, plus #195 (its open question is whether a proposal's test outlives
  the applied proposal). It decides whether #366 and #367 are kept and gives #195 its scope.

## Ready, in pick order (step 10: lowest number first, dependencies closed)
No `area:` labels exist yet (#308, M1), so no issue counts as a product issue and step 10 picks
lowest number first.
1. #366 Driver worktree proposal test: portable `touch` (it fails on macOS). Readied today.
2. #367 The same test: kill the `grep -x` without `-F` mutant. Readied today; after #366, since
   both edit one file.
3. #369 Dashboard page: contrast test for every text style, and a test for `ci linux`'s
   snapshot-copy wiring. Readied today (#353 closed).
4. #383 nightly 3/3 part 2: issues for fuzz crashes and bench timeouts. Readied today (#302 and
   #361 closed).
5. #403 Record commits: tie the retired ops/STATUS.md exemption to origin/main. Strict lane.
   Readied today.
6. #413 Backend selection 1b/2: `cargo xtask admit`'s vet and build-time steps. Readied today
   (#303 closed, PR #414). An unreviewed draft is on branch `t/413-admit-vet` (e35aa0b).

## Waiting (labeled `ready`, a dependency still open)
- #304 (backend selection 2/2) on #413.
- #21 (pilot fire drill) on #19, which closes after #383. The owner's drill steps (#208) are
  all done, per the owner session's comment on #208 dated 2026-11-27.

## In flight
None: no agent PR is open.

## Not workable now
- #15, #16, #19: umbrellas. Each closes after its open sub-issues: #15 after #369, #16 after #413
  and #304, #19 after #383.
- #195: waits for #420's decision on proposal tests.
- #208: the owner's drill steps. All are done, and #21's PR records them and closes it.
- Set aside (`deferred`): #146.

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The two longest chains are #383 → #19 → #21 and #413 → #304 → #16. #304 then files
the adapter and own-implementation issues, which go to later milestones.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
work mix: 2 of the last 20 sessions on milestone work (10%; budget 70%)

The budget counts a session as milestone work only if a PR for its issue changes `crates/`,
`bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` or `tools/fsio/`. #369 (`apps/web/`) is the one
ready M0 issue expected to count. PLAN §8's M0 row has no product deliverable left without an
issue (#304 files the backend adapters), so step 4a added no product issues to M0. No issue moved
to `Ops`: #366, #367, #195 and #420 are about `ops/proposals/`, and #403 is review-rule text.
Both are strict-list subjects (TICK §5 Discoveries term 4). #21 is M0's exit criterion "pilot
fire drill passed" (term 1). The mix is expected to rise only once M0 closes and M1 (syntax and
security) opens.
