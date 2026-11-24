# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-24 (seeded by #244 from the labels on that day, not by a Planner run)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner; GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Ready, in pick order (step 10: lowest number first, dependencies closed)
1. #246 Session types 4 1/2: `ops/areas.toml`, `area:`/`origin:` labels, `xtask issue-lint`.
2. #249 Session types 6: Design: product and `docs/PRODUCT.md` v1 (v1 waits for the owner's approval).
3. #275 Session types 3 2/2: next-session step 6.

## Waiting (labeled `ready`, a dependency still open)
- #247 (task 4 2/2) on #246; #248 (QC) on #247; #250 (Design: UI) on #249; #251 (User advocate)
  on #247 and #249; #288 (task 2 2/2, the work-mix budget) on #244.

## In flight
- #244 (task 2 1/2, this file's PR #289): `in-progress`.

## Not workable now
- #21 (pilot fire drill): `in-progress`; its `Depends on:` names #19 (open) and it waits for
  the owner's drill steps (#208).
- #14, #20, #22: umbrellas that close after issues in later milestones (#55, #70; #61; #64);
  the first Planner run moves each to the latest of those milestones (ops/TICK.md §9 step 2).
- #15, #16, #19, #25, #195, #277: no `ready` label yet; the first Planner run checks each one
  (ops/TICK.md §9 step 2).
- Owner's turn: #208 (drill steps), #260 (an untagged owner comment would make it an
  instruction); `owner-todo`: #179, #196, #266. Set aside (`deferred`): #146.

## Critical path
The judge's M0 gate passes (nightly 45179218903, 2026-11-23). The `m0` tag waits for an empty
M0 backlog. The umbrellas #14, #20 and #22 can't close in M0 (their work is in M1, M2 and M9),
so M0 can empty only after a Planner moves them. Of the work left in M0, the longest chains are
#19 → #21 (with the owner's #208) and ADR 0011's tasks #246 → #247 → #248, with #249 → #250
and #251 beside them.

## Design ahead (ADR 0011 §6)
None yet. The Planner lists the next milestone's cross-cutting concerns when it opens that
milestone's backlog.

## Work mix
Not measured yet: #288 adds the reading.
