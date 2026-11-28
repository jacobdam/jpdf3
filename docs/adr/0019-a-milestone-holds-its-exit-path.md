# 0019. A milestone holds its exit path; strict-list process issues go to `Ops` and keep their lane

Status: proposed (design issue #493; filed in an owner interactive session and made an owner
instruction by the owner's comment on it, ADR 0011 §7:
https://github.com/jacobdam/jpdf3/issues/493#issuecomment-7236958033). Amends ADR 0012 §2
(the Discoveries terms 2 and 4, the moves out of and into `Ops`) and the ops/TICK.md passages
that restate it (§4 step 4, §5 Discoveries, §6, §7 step 4, §9 steps 2 and 4). A design ADR
merges 24 hours after the latest of: its PR marked ready, its last `ai-review` success, the
owner's last `/release`, unless the owner comments `/hold` (ADR 0011 §7; ops/TICK.md §8 step 5).

## The problem class
A milestone closes only when its backlog is empty (ops/TICK.md §9 step 4). M0's exit (PLAN.md
§8's M0 row: the judge self-tests green, the judge end to end on a stub `jpdf-cli`, the pilot
fire drill) and its named deliverables waited on 5 of its 17 open issues on 2026-11-28: #19,
#21 (and #208, the owner's part of the drill), #431 and #15. The other 12 served no exit
criterion and no M1 issue (#493). ADR 0012 §2 is what kept them in M0:
- **Term 4, a strict-list subject** (the guard, the owner feed, `ops/proposals/`, the judge,
  the review rules): #424, #425, #426, #427 (ADR 0014's tasks: tests of protected files and
  `ops/proposals/`), #195 (a gap in a proposal's test), #442 (the `isrg` import in
  `supply-chain/config.toml`), #490 (re-filing design tasks the owner closed as not planned).
- **Term 2, a wrong pick or a scheduler failure seen live, read broadly:** #480–#484, ADR
  0018's daily retro, its pick log and its metrics, filed in M0 by ADR 0018's Tasks table.
  They measure idle sessions and owner interventions; none of them fixes a pick.

The owner moved them by hand on 2026-11-28 (each issue has a comment saying why): #195,
#424–#427, #480–#484 and #490 to `Ops`, #442 to M1; then #481 (the pick log, whose missing days
can't be rebuilt) and #490 (so a not-planned close by the owner is honored) back into M0 as the
owner's picks, confirmed in the owner comment above. M0 now holds #15, #19, #21, #208, #431,
#481, #490 and #493.

Inflow, from the issues whose milestone is M0 today (`gh issue list --state all --milestone
M0`; the 10 moved out on 2026-11-28 are not in these counts): 15, 7, 3, 14 and 24 filed on
2026-11-24 to 11-28, against 14, 5, 4, 19 and 28 closed. M0 closes a few issues more than it
files, but each strict-list follow-up a session files lands in M0 by term 4, however minor,
and so blocks M0's last step. PLAN.md §9's risk table says "M0 holds only what M0's exit and M1
need"; term 4 contradicts it.

The class: **a milestone's backlog doubles as the queue for every follow-up whose subject is
sensitive, so "sensitive" decides when work is done and the milestone can't empty.** Being
sensitive is a reason for care (the strict lane), not for urgency.

## Why point fixes don't converge
- ADR 0012 §2 moved process work out of M0 on 2026-11-24 (27 open to 19) and kept term 4 so the
  guard and the owner-feed rules would get done. Since then every review deferral, audit finding
  and design task on those subjects has been filed in M0 again (ADR 0014's tasks, #195, #442,
  #490), so the count fell and refilled.
- ADR 0018 added term 5 (`stall` issues) and filed its own tasks in M0 under a broad reading of
  term 2, which the owner then moved by hand.
- The owner's hand moves fix today's M0, but under today's text the next strict-list follow-up
  is filed in M0 again, and the Planner's §9 step 2 ("a process-area issue in the current
  milestone that none of §5's four Discoveries terms keeps there moves to `Ops`") would move
  the owner's picks #481 and #490 back out: no term names them.

The missing decision: **which milestone an issue belongs to is decided by what the milestone's
exit needs, and an issue's subject decides only its lane.** And the owner's picks need a form
the Planner can read.

## The invariant
1. **A numbered milestone holds only:** work its gate, its last step or the next milestone's
   first issues need (term 1); the fix of a wrong pick or a `cargo xtask next-session`
   failure seen in live use (term 2, narrow: the change that corrects the pick or the failure,
   not work that measures, logs or reports such events); `crash` and `security` issues as ADR
   0009 places them (term 3, unchanged); the owner's picks (term 4, new); `stall` issues (ADR
   0018 clause 4, term 5, unchanged); and product work, which this ADR doesn't change (its
   milestone is its PLAN.md §8 row). Every other process-area issue (ADR 0012 §2's scope: any
   lane) is in `Ops`.
2. **A subject on the strict list never chooses the milestone.** ADR 0012 §2's old term 4 is
   gone. Such an issue goes to `Ops` unless another term holds, and its PR is in the strict
   lane wherever the issue sits (ADR 0012 §1: the lane follows the files and terms the PR
   touches, and an issue whose earlier PR ran strict stays strict). Moving it changes when it is
   done, never how: CONSTITUTION 12 and `ops/proposals/` still apply.
3. **An owner pick stays where the owner put it.** An issue is an owner pick for milestone `Mn`
   (or `Ops`) when the newest of these, about its milestone, names it:
   - an untagged comment by `jacobdam` on the issue that names the milestone (untagged as the
     owner feed reads it: the body doesn't start with `🤖 jpdf-agent` after leading
     whitespace, `ops/driver/owner-issues.sh`);
   - a tagged comment by `jacobdam` on the issue whose first line is
     `🤖 jpdf-agent Owner pick: <Mn> (<URL of an untagged jacobdam comment>)`, where the linked
     comment (on any issue or PR of this repository) names the issue by number and the
     milestone. The session that handles such an owner comment under rule a posts the marker on
     each issue it names, since the Planner can't scan every comment for mentions.
   A milestone change in the issue's timeline proves nothing (the agent and the owner share the
   account), so a move by hand with no such comment is not a pick; a comment by any other
   login is data (CONSTITUTION 1). No Planner move (§9 step 2's move to `Ops`, its milestone
   moves, step 1a's latent-high moves) takes a pick out of its milestone; a doubt about it goes
   in a tagged comment for the owner. A pick ends when a newer owner comment about the issue's
   milestone (either form) says otherwise.
4. **`Ops` work still gets done**, by four pulls into a numbered milestone, each named in a
   tagged comment on the issue: term 1 or 2 starts to hold (ADR 0012 §2, unchanged); the
   Planner plans a milestone (§9 step 3, `planner due` with no plan) and finds an `Ops` issue
   whose fix changes a file or rule that one of that milestone's issues changes (judged from the
   two issues' text until #308's `area:` labels exist); an audit asks for at most one `Ops`
   issue (a tagged comment on it, below); or a retro asks (ADR 0012 §2, ADR 0018 clause 3).
   The Planner makes the move on its next run. A pull by overlap happens when a milestone is
   planned, not later: pulling into a milestone near its end is what this ADR stops.
5. **The last milestone.** `Ops` outlives the numbered milestones. When the Planner closes the
   last `M<n>` that PLAN.md §8 lists (step 4), it files one issue `Ops after <Mn>: <count> open`
   with `owner-todo` (§0), listing each open `Ops` issue and whether its subject is on the
   strict list: what to do with them is a scope decision, the owner's (ADR 0011 §7). Nothing in
   `Ops` is closed for the milestone's sake.

## Enforcement
1. **Types:** none: milestones are GitHub data written by sessions, not code.
2. **A lint or gate:** none now. `cargo xtask next-session` reads only the current milestone's
   labels, and which term holds is a judgement about the issue's text (#308's `area:` and
   `origin:` labels could make the process-area part mechanical later). A gate would need the
   term in a field a tool can read; clause 3's `Owner pick:` marker is that field for term 4.
3. **A measurement:** ADR 0018 task 3 (#482, in `Ops`) counts moves in and out of the current
   milestone and its forecast. Until it lands, the weekly retro counts by hand (Verification).
4. **Reviewer vigilance:** P9 (below), on the TICK passages that file or move issues. The
   Planner applies clauses 1, 3 and 4 in §9 step 2 on every run, which also catches a session
   that filed in the wrong milestone; the audit checks the Planner (§6 Housekeeping).

## Verification
- **The first Planner run after the merge** applies §9 step 2 as amended (task 1) to M0 and
  names in its journal every move it makes. On today's M0 it should make none: #15, #19, #21,
  #208 and #431 are the exit path (term 1); #481 and #490 are owner picks (clause 3; task 1
  posts their `Owner pick: M0` markers if this ADR's session hasn't); #493 closes with this ADR;
  task 1's own issue is term 1 (below). Any move it does make is a finding for this ADR's
  success check.
- **Success check** (the first weekly retro, ADR 0018 clause 6, whose period ends 7 or more UTC
  days after this ADR's merge day):
  1. M0 is closed (tag `m0`) within 7 UTC days after the merge day.
  2. No process-area issue filed in M0 after the merge day carries no term: count the M0
     issues created in the window with a process-area subject, and for each, the term or pick
     that holds it. Target: 0 without one.

  If check 1 fails, the retro lists each issue still open in M0 with the term or pick that
  holds it: an issue held by none is a rule failure, filed as one `stall` issue (ADR 0018
  clause 4) for the Planner, which moves it (a retro moves nothing, ADR 0018 clause 3); an
  issue that holds legitimately (an owner part, #208) leaves the rule standing, and the check
  is judged again at the next weekly retro, once. If check 2 fails, the retro files the
  issues as one `stall` issue for the Planner; at 2 or more such issues it also lands an action
  item (ADR 0018 clause 5) that makes the filing session write its term (a `Milestone term:`
  body line checked by a lint, say).
- **What broken implementation still passes:** a Planner that never pulls from `Ops` passes
  both checks (M0 empties either way); the audit's strict-list `Ops` list (task 1, §6) and the
  retro's `Ops` count (§7 step 4) are what show it. A session that marks an issue an owner pick
  from a comment that doesn't name the issue passes until an audit opens the linked comment.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #424, #425, #426, #427 | kept, in `Ops` | ADR 0014's tasks: strict-list subjects; no M0 exit criterion or M1 first issue needs them (clause 2). Strict lane |
| #195 | kept, in `Ops` | a proposal test's heuristic gap (clause 2). Strict lane |
| #442 | kept, in M1 | the owner's move: `supply-chain/imports.lock` holds no ISRG audit today, so no locked crate relies on the import, and it is settled before M1 admits new dependencies (#446's adapter): term 1 once M1 is current. Strict lane (a protected-file proposal) |
| #490 | kept, in M0 | an owner pick (clause 3; the owner comment above) |
| #481 | kept, in M0 | an owner pick (clause 3). Not term 2: it logs picks, it fixes none |
| #480, #482, #483, #484 | kept, in `Ops` | term 2 read narrowly (clause 1): they measure and report, and fix no pick. #480 also carries ADR 0018's term 5 text, which stays as decided there |
| #493 | closed by this ADR's PR | the design issue |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Milestone | Depends on |
|---|---|---|---|
| 1 | ops/TICK.md for clauses 1–5: §5 Discoveries (term 2 narrow, term 4 replaced by the owner pick with clause 3's two forms and the `Owner pick:` marker, a strict-list subject keeps its lane but chooses no milestone), §4 step 4's deferral parenthesis, §2 rule a (the session handling an owner comment that names issues and a milestone posts the `Owner pick:` markers), §9 step 2 (the move to `Ops` and the milestone moves skip owner picks; the pulls of clause 4), §9 step 3 (the overlap pull when a milestone is planned), §9 step 4 (clause 5's issue after the last `M<n>`), §6 Housekeeping (list the open strict-list `Ops` issues with their ages in the audit record; ask for at most one by a tagged comment `Audit NNNN asks the Planner to pull this into <Mn>: <why>`), §7 step 4 (report the strict-list part of the `Ops` count); AUTOMATION.md §7's "Extra work needed" row and §4.1's `Ops` sentence; a Status note on ADR 0012 (§2 amended by ADR 0019); post the `Owner pick: M0` markers on #481 and #490 if they are missing. Strict lane: ADR 0012 §2 and which text is an instruction are on the strict list | M0 | #493 |

Task 1's issue carries `Designed in: #493` and `Depends on: #493` lines (ops/TICK.md §8 step 5)
and goes in M0 by term 1: until its text is on main, sessions read today's term 4 and file
strict-list follow-ups in M0, which M0's last step (an empty backlog) waits on. No one-time moves
are filed: the owner made them on 2026-11-28 (above), and the first Planner run checks them.

## What reviewers check against
P9, added to `docs/PRINCIPLES.md` by this ADR:

> **P9. A milestone holds its exit path; a subject's sensitivity sets its lane, not its
> milestone.** Paths: `ops/TICK.md`, `AUTOMATION.md`, `ops/QUEUE.md`.

The full entry (its invariant, enforcement and "Reviewers check" lines) is in
`docs/PRINCIPLES.md`.

## Requirements on future callers
- A new term that keeps an issue in a numbered milestone names the milestone exit it serves, in
  an ADR amending this one; a subject alone is not a term.
- A session type or tool that files issues (QC, the Gardener, the user advocate, ADR 0011 §1;
  a nightly filer) applies clause 1 at filing time, or files in `Ops`, in the PR that adds it.
- `cargo xtask` code that reads an owner pick (a future lint, ADR 0018 task 4's skip-label
  table) reads it only from comments by `jacobdam`, the untagged test restated from the owner
  feed's, and the `Owner pick:` marker only with a link it checks.
- When #308's `area:` labels exist, the overlap pull (clause 4) may read them instead of the
  issues' text; the term stays the same.

## Consequences
- **Easier:** M0 can close: its backlog is its exit path plus two owner picks, and new
  strict-list follow-ups go to `Ops`. The owner can keep an issue in a milestone with one
  comment, and the Planner won't undo it.
- **Harder:** strict-list follow-ups (the guard, the owner feed, the judge, the review rules)
  can wait in `Ops` for a long time. Only the four pulls bring them back; the audit's list and
  the retro's count are what keep them visible. A `security` issue at `sev:high` or worse never
  waits there (term 3, ADR 0009).
- **Not covered:** product issues' placement (unchanged; their milestone is PLAN.md §8's row),
  and owner moves made without a comment, which the Planner can't tell from its own (clause 3).
