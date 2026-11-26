# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-26 (Planner run, session 2026-11-26T23:04Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner; GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Design queue (step 6, before the ready list)
- #357 Design: xtask nightly steps, their order and per-step failures are not testable (filed
  today, step 2a: 6 open issues on `cargo xtask nightly`, #19, #302, #322, #323, #350, #352,
  with no design issue or ADR covering them).

## Ready, in pick order (step 10: lowest number first, dependencies closed)
No `area:` labels exist yet (#308, M1), so no issue counts as a product issue and step 10 picks
lowest number first.
1. #303 Backend selection 1/2: `cargo xtask admit`.
2. #322 nightly: keep the tail of `jpdf-cli`'s stderr, not the head (PR #321 R1.5).
3. #323 nightly: check that the runner can execute `jpdf-cli` (PR #321 R3.1).
4. #344 Driver worktree proposal test: the 200-PR read bound, exactly (PR #343 R3.1).
5. #350 nightly: test the limits part's order (PR #349 R1.4; readied today, #25 is closed).
6. #352 nightly: pin the stale `nightly.json` removal failure (PR #351 R2.1; readied today).
7. #353 Dashboard 1b/2: the static page in the Pages site (readied today, #305 is closed; the
   finished work is on branch `t/353-dashboard-page`, 75d990f).

## Waiting (labeled `ready`, a dependency still open)
- #302 (nightly 3/3) on #322.
- #304 (backend selection 2/2) on #303.
- #306 (dashboard 2/2) on #353 (added today: it extends the page, which split off from #305).
- #21 (pilot fire drill) on #19, which closes after #302; it also needs the owner's drill steps
  (#208).

## In flight
- PR #347 (#331, a `design-product` journey; not an M0 issue).

## Not workable now
- #15, #16, #19: umbrellas; each closes after its parts (#353, #306; #303–#304; #302).
- #195: changes a proposal script whose proposal the owner applied (#196 is closed); whether the
  test outlives the applied proposal is still open, so it has no scope yet.
- Owner's turn: #179 (`owner-todo`); #208 (drill steps) has no label but is the owner's. Set
  aside (`deferred`): #146.

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The longest chain is #322 → #302 → #19 → #21, which also needs the owner's #208
steps. #303 → #304 and #353 → #306 run beside it.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
work mix: 0 of the last 20 sessions on milestone work (0%; budget 70%)

The budget counts a session as milestone work only if a PR for its issue changes `crates/`,
`bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` or `tools/fsio/`. #353 (`apps/web/`) is the
one open M0 issue expected to count. PLAN §8's M0 row has no product deliverable left without
an issue (the backend adapters come from #304), so step 4a added no product issues to M0. No
issue moved to `Ops`: `xtask/` (except the scheduler, metrics, dashboard and issue-lint files)
is the `tooling` area, a product area in ops/areas.toml, and the process-area ones left (#344,
#195) are about `ops/driver/` and `ops/proposals/`, strict-list subjects that TICK §5 term 4
keeps here. The mix is expected to rise only once M0 closes and M1 (syntax and security) opens.
