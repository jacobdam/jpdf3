# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-24 (Planner run, session 2026-11-24T02:51Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner; GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Ready, in pick order (step 10: lowest number first, dependencies closed)
1. #246 Session types 4 1/2: `ops/areas.toml`, `area:`/`origin:` labels, `xtask issue-lint`.
2. #249 Session types 6: Design: product and `docs/PRODUCT.md` v1 (v1 waits for the owner's approval).
3. #275 Session types 3 2/2: next-session step 6.
4. #277 Review cap follow-ups (a K >= 6 PR that conflicts after its last check; metrics count design final checks).
5. #288 Session types 2 2/2: the work-mix budget.
6. #294 Planner PR closed at the cap: TICK §9 step 6 wording (PR #289 R5.1–R5.3).
7. #300 `cargo xtask nightly` 1/3: the regress tier through `jpdf-cli`.
8. #303 Backend selection 1/2: `cargo xtask admit`.
9. #305 Dashboard 1/2: the page, process metrics and the forecast.

## Waiting (labeled `ready`, a dependency still open)
- #301 (nightly 2/3) on #300; #302 (nightly 3/3) on #300 and #301; #25 (resource-limit harness)
  on #300.
- #304 (backend selection 2/2) on #303; #306 (dashboard 2/2) on #305.
- #247 (task 4 2/2) on #246; #248 (QC) on #247; #250 (Design: UI) on #249; #251 (User advocate)
  on #247 and #249.
- #21 (pilot fire drill) on #19, which closes after #300, #301 and #302; it also needs the
  owner's drill steps (#208).

## In flight
- #295 (`in-progress`, PR #296). PR #299 (#298) merged with #296's commits in it.

## Not workable now
- #15, #16, #19: umbrellas split on 2026-11-24 into #305–#306, #303–#304 and #300–#302; each
  closes after its parts.
- #195: changes a proposal script that the owner is applying (#196, PR #291); whether the test
  outlives the applied proposal is open, so it has no scope yet.
- Owner's turn: #208 (drill steps; its `/hold` re-run waits for #266), #260 (an untagged owner
  comment would make it an instruction; its close condition, #245 merged, is met, so it only needs closing); `owner-todo`: #179, #196, #266. Set aside
  (`deferred`): #146.
- Moved out of M0 on 2026-11-24 (their remaining work is in later milestones): #14 → M9 (#66,
  #70, #55), #20 → M1 (#61), #22 → M2 (#64).

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The longest chain is #300 → #301 → #302 (closing #19) → #21, which also needs the
owner's #208 steps. ADR 0011's tasks #246 → #247 → #248 (and #251), with #249 → #250 beside
them, come next.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
Not measured yet: #288 adds the reading.
