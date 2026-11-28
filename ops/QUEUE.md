# QUEUE: the Planner's plan for the current milestone (ops/TICK.md §9, ADR 0011 §1)

Milestone: M0
Planned: 2026-11-28 (Planner run by the owner's instruction in an interactive session, 10:29Z; earlier runs 00:04Z and 05:32Z)

`cargo xtask next-session` reads only the `Milestone:` line (step 8: a milestone with no plan
makes the Planner due). The rest is for sessions and the owner. GitHub labels and `Depends on:`
lines stay the source of truth, and step 10 picks from them, not from this list.

## Design queue (step 6, before the ready list)
None open (no open issue is labeled `design` or `design-product`).

## Ready, in pick order (step 10: lowest number first, dependencies closed)
No issue has a product `area:` label yet (#308, M1), so step 10 picks lowest number first.
1. #509 ADR 0019 task 1: the ops/TICK.md and AUTOMATION.md text for the milestone terms, owner
   picks and `Ops` pulls. Filed and readied in this run (step 1b: the owner merged PR #495
   before its window ended, so no design-window session filed it). M0 by ADR 0019's Tasks table
   (term 1). Its one dependency, #493, is closed. Strict lane.

## Waiting (labeled `ready`, a dependency still open)
None.

## In flight
None besides this Planner PR. The 10:37Z `planner new-issues` session ran no Planner pass
because this run was under way; its journal entry is on main (PR #521, records only).

## Not workable now
None in M0: #509 is its only open issue.

## Filed in this run for M1 (step 1b; no `ready` until M1 is current)
ADR 0017's 11 tasks, filed again from the ADR as merged (the push rule had closed #467 to #477
at push 61152f4, and the owner merged PR #465 before a design-window session ran), each a
sub-issue of #236 with `Designed in: #236`:
#510 (task 1, `jpdf-mem`), #511 (2, the allocator-exception proposal), #512 (3, the owner
applies it), #513 (4), #514 (5), #515 (6, `mem-lint`), #516 (7, `jpdf-alloc`), #517 (8, closes
#146), #518 (9), #519 (10), #520 (11). Their chain: #510 → #513 → #514 → #515; #511 → #512;
#512 and #514 → #516 → #517 → #518; #516 → #519; #515 and #517 → #520.

## Sub-issues (step 2b)
#509 is linked under #493, and #510 to #520 under #236. No other open M0 issue is a part.

## Critical path
The judge's M0 gate passes on the latest nightly, run 45999791450 (2026-11-27, head 6bbfd59):
`gates/gates.json` has M0 `pass: true`, `failing: []`. The only work left in M0 is #509 (one
session). Once it merges, M0's backlog is empty and the next Planner run tags `m0`, writes
`docs/milestones/m0.md`, closes M0 and opens M1 (§9 step 4). Today has 3
`- Session: planner…` journal files on main (00:04Z, 05:32Z, and the 10:37Z one from PR #521),
the cap, plus this run's in this PR, so step 11 picks no rerun today; that run is the 2026-11-29 one (step 8: the work mix is below
70%) unless the owner runs one by hand.
The tag isn't pushed in this run: step 1b put #509 in M0, so the backlog is not empty.

## Design ahead (ADR 0011 §6)
None for M0. The run that opens M1 lists M1's cross-cutting concerns (§9 step 3).

## Deferred (step 1a)
- #146 (M1, `sev:high`, latent: parse heap high-water mark next to mmap-backed buffers).
  Undefer when: #517 closed. Updated in this run from "the issue for ADR 0017 task 8 … closed
  as completed" to the refiled task's number; the owner set its milestone and the task-8
  condition, so both stay.
- #176 (no milestone, also `owner-todo`): Undefer when: the owner says to start it (issue body:
  the owner decides when to start). No owner comment says so.
- #247 (`Ops`): Undefer when: a retro finds an issue group the Planner missed (ADR 0012 §3).
  No retro since 2026-11-23 (docs/retros/0002.md).

## Work mix
work mix: 1 of the last 20 sessions on milestone work (5%; budget 70%)

This run's pick was `idle`, whose brief prints no reading, so the line above is what
`cargo xtask next-session --from` printed on this session's live inputs with today's Planner
records renamed, so that step 8 ran (a Planner record counts as other work, so the window is
the same). PLAN §8's M0 row has no product deliverable without an issue, so step 4a adds no
product issues to M0. The mix is expected to rise only once M1 (syntax and security) opens.
