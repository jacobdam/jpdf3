# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-25 (Planner run, session 2026-11-25T16:40Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner; GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Ready, in pick order (step 10: lowest number first, dependencies closed)
No `area:` labels exist yet (#308, M1), so no issue counts as a product issue and step 10 picks
lowest number first.
1. #25 Adversarial resource-limit regression harness in `cargo xtask nightly` (its #300 is closed).
2. #301 `cargo xtask nightly` 2/3: bench and extra fuzz time in the nightly report (its #300 is closed).
3. #303 Backend selection 1/2: `cargo xtask admit`.
4. #305 Dashboard 1/2: the page, process metrics and the forecast.
5. #322 nightly: keep the tail of `jpdf-cli`'s stderr, not the head (PR #321 R1.5; readied today).
6. #323 nightly: check that the runner can execute `jpdf-cli` (PR #321 R3.1; readied today).
7. #344 Driver worktree proposal test: the 200-PR read bound, exactly (PR #343 R3.1; readied
   today; its #342 is closed).

## Waiting (labeled `ready`, a dependency still open)
- #302 (nightly 3/3) on #301 and #322 (#322 added today: #302 files issues from crash stderr).
- #304 (backend selection 2/2) on #303; #306 (dashboard 2/2) on #305.
- #21 (pilot fire drill) on #19, which closes after #301 and #302 (#300 is closed); it also needs
  the owner's drill steps (#208).

## In flight
None. PR #343 (#342) merged at 16:51 UTC; the owner's part is #345.

## Not workable now
- #15, #16, #19: umbrellas; each closes after its parts (#305–#306, #303–#304, #300–#302).
- #195: changes a proposal script that the owner is applying (#196); whether the test outlives
  the applied proposal is still open, so it has no scope yet.
- Owner's turn (`owner-todo`): #179, #196, #345 (filed today: apply
  ops/proposals/driver-keep-open-worktrees.md). #208 (drill steps) has no label but is the
  owner's. Set aside (`deferred`): #146.

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The longest chain is #301 → #302 (after #322) → #19 → #21, which also needs the
owner's #208 steps. #303 → #304 and #305 → #306 run beside it.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
work mix: 0 of the last 20 sessions on milestone work (0%; budget 70%)

The budget counts a session as milestone work only if a PR for its issue changes `crates/`,
`bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` or `tools/fsio/`. Every open M0 issue is
`xtask/`, `ops/` or owner work, and PLAN §8's M0 row has no product deliverable left without an
issue (the backend adapters come from #304). So step 4a added no product issues to M0, and
nothing was moved out: none of the open M0 issues is a process issue that §5's Discoveries terms
would move to `Ops`. The mix can only rise once M0 closes and M1 (syntax and security) opens.
Closing M0 is the way to get there: the critical path above, then #303–#306.
