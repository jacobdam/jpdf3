# 0018. The retro measures activity, not whether the loop finishes milestones without the owner

Status: proposed (design issue #463; filed in an owner interactive session, made an owner
instruction by the owner's comment on it, ADR 0011 §7). Amends ADR 0011 §1's Retro row
(trigger and bound) and §2's "Read-only" bullet, and AUTOMATION.md §11. A design ADR merges
24 hours after the latest of: its PR marked ready, its last `ai-review` success, the owner's
last `/release`, unless the owner comments `/hold` (ADR 0011 §7; ops/TICK.md §8 step 5).

## The problem class
PLAN.md's goal is a product built by agents, with people "stepping in only at milestone
checkpoints" (PLAN.md opening; R13, "No human in the loop"). The retro is the loop's only
self-correction for process (AUTOMATION.md §11, ops/TICK.md §7, ADR 0011 §1's Retro row:
weekly). Its inputs are activity numbers: merged PRs, review rounds, conflicts, context size
(AUTOMATION.md §11 "Input"; specs/tooling/metrics.md). Retros 0001 and 0002 moved those numbers.
None of them says whether the current milestone is getting closer, where the sessions went, or
when the owner had to step in. 2026-11-28 shows three gaps (#463):

- **Idle capacity.** `ops/logs/index.tsv` holds 280 driver sessions on 2026-11-27 (242 under
  200 s) and 43 on 2026-11-28 up to 06:53 UTC (22 under 200 s), while workable M0 issues waited
  for a label (#438, ADR 0015). An `idle` pick writes no journal entry (ADR 0011 §3), so no
  retro input sees these sessions. The index has no pick column: its columns are the start
  time, the exit code, the session id and the duration (`ops/driver/driver.sh`).
- **Stale skip labels.** #146 (`sev:high`) and #236 kept `deferred` for 3 and 5 days after
  their stated conditions held (#461). The retro counts `blocked` (TICK rule h), not `deferred`
  or `owner-todo`.
- **Owner interventions.** The owner stepped in about six times that day: `ready` on #423 and
  #440 by hand, and the findings #433, #438, #461 and #463. Each is an R13 gap. No retro counts
  them.

The class: **a feedback loop that measures its inputs, not its goal.** The retro can lower
review rounds every week while milestones slip and the owner keeps the loop moving by hand,
and its weekly cadence means a stall found on day 1 is diagnosed on day 7 at the earliest.

## Why point fixes don't converge
Each gap above was fixed by hand once it was seen: ADR 0015 for issues waiting on the
Planner (#438), #461's `Undefer when:` lines for stale deferrals, and the owner adding `ready`
by hand in between. Each fix came from the owner noticing, not from a retro, because no retro
number would have moved: the sessions that idled left no record, the deferrals were not
counted, and the owner's label clicks are not in any input. The next stall of a new kind will
be found the same way. What is missing is a decision about what the retro is for (R13: the
milestone, reached without the owner), which numbers answer it, and a cadence short enough
that a stall costs a day, not a week.

## The invariant
1. **The retro's target is R13; quality is its floor.** Every retro record answers three
   questions with numbers from `cargo xtask metrics` (defined below): *is the product moving*,
   *where did sessions go*, *where did the owner step in*, plus the skip-label table. It also
   states the floor: the judge verdict on main, audit findings `sev:high` or worse since the
   last retro, and reverts. An action item that would lower the floor (relax a gate, a review
   rule or a limit) is not allowed (CONSTITUTION 4 and 14 already forbid most of it).
2. **Daily.** `cargo xtask next-session` makes the retro due when no retro record (a
   `docs/retros/NNNN.md` add, specs/tooling/next-session.md step 7) is dated today (UTC, the
   commit's `%ct` day), or on an incident after the newest one (today's terms). The slot in
   flight (#282) holds it while an open agent PR links an open `retro` issue. Step 7 comes
   before step 8, so on a day both are due the retro runs before the day's first Planner.
3. **The retro never gates or orders work** (#463 C1, C4). It never adds or removes a label,
   never moves an issue between milestones, never edits `ops/QUEUE.md` and never reorders
   tasks in PLAN.md. It writes its record, issues filed without `ready`, comments, and the rule
   changes of its action items. A retro that wants another order or a label change writes it
   in an issue or an action item addressed to the Planner (ADR 0012 §2: "a retro asks for it").
   The Planner stays the only agent gate into `ready` (P6).
4. **Stalls are issues in the current milestone** (#463 C2). Each stall the retro finds (an
   idle hour while an issue waited, a stale skip label, an owner intervention) gets one
   `discovery` issue labeled `stall`, in the current milestone (it is a live failure of
   `cargo xtask next-session` or of a rule, ADR 0012 §2 term 2), with the diagnosis only: the
   numbers, the rule that was missing or wrong, and no fix. One open `stall` issue per cause:
   a later retro that sees the same cause comments on it instead of filing a second. The
   Planner gates it the same day (step 8, or ADR 0015's rerun at step 11).
5. **At most 3 action items under measurement at any time.** An action item is one merged
   process change with a success check on a named number. Its window is the 7 UTC days after
   its merge; the first retro after the window ends gives the verdict (keep, revert or extend)
   from the window's numbers, and a revert is that retro's change. A retro may land a new item
   on any day while fewer than 3 are under measurement and no item under measurement has the
   same success number (so each number moves for one known reason). An extended item starts a
   new window and keeps its slot.
6. **The weekly part stays weekly.** A retro whose record is 7 or more days after the newest
   weekly record (or the first one after this ADR merges) is the weekly retro. It does what
   today's TICK §7 does beyond the three questions: the review-convergence numbers, the review
   deferral re-read, `ops/LESSONS.md` curation, the `Ops` count, the dashboard snapshot and the
   digest. Every record's second line says `Kind: daily` or `Kind: weekly`.

A reviewer can check each clause against a record or a fixture: a scheduler fixture with a
retro record dated `2026-11-28T23:59:59Z` and `now` = `2026-11-29T00:00:00Z` is due (another
UTC day), and one with the record at `2026-11-29T00:00:00Z` is not; a retro PR whose session
ran `gh issue edit --add-label` breaks clause 3; a fourth item under measurement breaks clause 5.

### The three questions, defined
All numbers are for the retro's period (`cargo xtask metrics --since --until`, normally one
day) and, for trends, for each of the 7 UTC days before it.

**Is the product moving?**
- the current milestone's open issue count at the end of each day, and the issues filed into
  and closed in it per day;
- the forecast finish date: open count ÷ (closed − filed per day, the 7-day mean), from the
  period's last day; when that rate is 0 or less, `no forecast: the milestone grows`;
- the product share of merges: product merges (ADR 0011 §3) ÷ all first-parent merges on main.

**Where did sessions go?** Every row of `ops/logs/index.tsv` in the period is one session,
classified by the first rule that matches:
1. *idle:* its pick is `idle`, or the scheduler exited 2;
2. *rework:* its pick is rule b or c's (`worker main-red …`, `worker own-pr #N`, the fallback's
   rule c) or `design-technical needs-design #N`;
3. *records:* its journal record is on a PR that changes only record paths (ops/TICK.md §4
   step 6), or its pick is `worker design-window #N`;
4. *product:* its pick names an issue with a product `area:` label (ops/areas.toml), or its
   work PR is a product merge;
5. *process:* everything else;
6. *unknown:* no pick is logged for it (below). These are counted, never guessed from
   duration.

It also reports **idle hours**: for each idle session, the time until the next session that is
not idle, summed; and **idle hours with work waiting**: the part of those hours in which the
idle pick logged at least one waiting issue (below). The baseline, by hand, is #438's: about
20 hours on 2026-11-28 before ADR 0015.

**Where does the pick come from?** The scheduler logs it. Each live run of `cargo xtask
next-session` (not `--from`) appends one line to `ops/state/picks.tsv` in the main checkout
(the common git directory's parent, so a run from a worktree writes the same file; local
state, git-ignored, like `ops/state/judge-alarm-since`): the UTC time, the exit code, the
pick's first line, and for an `idle` pick the numbers of the open current-milestone issues
that are not in flight and carry no skip label, `in-progress` or design label (the issues a
Planner run could still act on; numbers only, as P6 asks). A session's pick is the first line
whose time falls in its window (its index start time plus its duration). The two
alternatives are worse:
- *A `pick` column logged by the driver* (`ops/driver/` is protected, so a proposal): the
  driver doesn't know the pick. It would have to parse the session's transcript
  (`ops/logs/<start>.jsonl`), and transcripts are not a reliable source: on 2026-11-27, the
  first output of the first scheduler call did not start with a pick line in 163 of 280
  sessions (a scan of the session logs that day: 117 did), because the call was chained after
  `cargo xtask status`, `date` or `git log`. The scheduler's own log needs no parse.
- *Duration alone* (index.tsv's heuristic): 200 s separates most idle sessions from the rest,
  but not all (an idle session that is slow to exit, a Planner rerun that finds nothing), and
  it can't tell rework from product. It stays only as a note in the report for sessions before
  the log existed.

This amends ADR 0011 §2's "Read-only" bullet: the scheduler still writes nothing to GitHub or
git, and its only write is that local line. A session can forge or delete lines (the file is
not protected); the numbers are a measurement, never a gate, and a wrong line moves one
session between classes.

**Where did the owner step in?** An owner intervention is any of these in the period:
1. an untagged issue, comment, review body or inline review comment by `jacobdam` (the owner
   feed's own test, `ops/driver/owner-issues.sh`: untagged text is the owner's), including
   `/hold`, `/release` and `/approve` (each counted on its own too);
2. a tagged issue labeled `human` at creation (filed in an owner interactive session,
   CLAUDE.md; driver sessions never add `human`, a rule this ADR adds in task 1);
3. a `ready` label added outside a session whose pick is a Planner's (`planner…`), except on an
   issue labeled `design` or `design-product` (the queues P6 names): only the Planner adds
   `ready` (P6), so any other add is the owner's or a P6 violation, and both are findings. The
   `ready` on #423 at 04:54:15 UTC fell inside a Worker session's window (started 04:53:29),
   which is why "outside every session window" is not the test;
4. a label change, milestone change, close or merge by the account outside every session
   window (index.tsv: start time to start time plus duration). An owner action inside a window
   is missed (an undercount, stated in the report);
5. a commit on main by the owner (`owner/` branches, AUTOMATION.md §6.1).

Two kinds are counted apart and are not R13 gaps: actions a rule leaves only to the owner (an
owner-signed commit on a protected path, CONSTITUTION 12; the owner's answer on an issue or PR
that carried `owner-todo` at the time; `docs/PRODUCT.md` v1's approval, ADR 0011 §7), and
actions at a milestone checkpoint (on the issue or PR that tags `mN`, TICK §9 step 4). Their
count is reported with its trend; the target for the rest is **0**. Each one the rest counts is
traced to the rule that was missing (the owner had to add `ready`: the Planner didn't run, or
didn't gate it) in its `stall` issue (clause 4).

**Skip labels.** For `deferred`, `blocked` and `owner-todo`: the open count, the age of each
(since its newest `labeled` event) and the oldest. For `deferred`, each issue's `Undefer when:`
condition (ops/TICK.md §5) and whether it holds now, for the forms a tool can check (`#A closed`
as completed, `milestone Mn is current`, and `work mix ≥ 70%`, added by task 1); free text is
listed as `judged by the Planner`. A deferral whose condition holds at the retro and held at
the day's last Planner run is stale (a stall). A budget deferral (the Planner's step 4a for the
work mix, #463 C3) carries `Undefer when: work mix ≥ 70%` and is stale only once the mix is at
70% or more.

### Retro 0003
Retro 0003 is due by 2026-11-30 (7 days after retro 0002) and carries ADR 0012's success check.
If task 1 hasn't merged by then, it runs as today's weekly retro (ops/TICK.md §7) and adds the
three questions' numbers it can take by hand (index.tsv, the journals, the issue list) as the
baseline. If task 1 has merged, it is the first daily retro and the weekly one (`Kind: weekly`).
Either way it does ADR 0012's check as written: that check reads 7 days of data, which the
weekly cadence gives it.

## Enforcement
1. **Types:** not used. The retro's numbers and its record are text and counts; no type keeps a
   session from editing a label.
2. **A lint or gate:** the scheduler is the gate for clause 2 (the daily term, with fixtures).
   `cargo xtask metrics` computes the three questions and the skip-label table (tasks 3–5), so a
   retro copies numbers instead of counting. No CI gate checks a retro record's content; the
   judge is protected and its gates are the product's, not the process's.
3. **A measurement:** the retro measures itself: the forecast date's movement, idle hours with
   work waiting, and the owner-intervention count, against the 2026-11-28 baseline (about 20
   idle hours, about 6 interventions).
4. **Reviewer vigilance:** P7 in `docs/PRINCIPLES.md` (below), for clauses 3, 5 and 6, which
   no tool checks: a retro PR changes no label or milestone, its action items stay within 3
   under measurement with distinct numbers, and every record answers the three questions.

## Verification
- Task 1's scheduler fixtures: the UTC-day boundary (the `23:59:59Z` / `00:00:00Z` pair
  above), a record on another day 1 hour ago (due), the slot in flight, an incident on a day
  that already has a record (due), and step 7 before step 8. What broken implementation still
  passes? One that compares 24 hours instead of the UTC day passes unless a fixture puts the
  record at `23:00:00Z` and `now` at `00:30:00Z` the next day (due by day, not by hours); that
  fixture is required. One that uses the record's author date instead of `%ct` passes unless a
  fixture's two dates differ across midnight; required too.
- Task 2's pick log: a test runs the scheduler live against a fake reader and reads the line
  back; `--from` writes nothing (a test asserts the file is absent); a worktree run writes the
  main checkout's file. A broken one that logs the last run instead of the first in a window
  passes unless the metrics fixture has two lines in one window; task 3 requires it.
- Tasks 3–5: `cargo xtask metrics --from` fixtures with one session of each class and the
  nearest miss of each rule above (a rework pick whose journal record is records-only: rework
  wins; a `ready` add inside a Planner window on a non-design issue: not counted; the same add
  inside a Worker window: counted; a `human`-labeled tagged issue: counted; a tagged one
  without it: not counted; a deferral whose `#A` closed as not planned: does not hold).
- What no fixture catches: a session that changes a label by hand inside a retro (clause 3),
  owner actions inside a session window (clause 4 of the intervention list) and a forged pick
  line. The first is P7's reviewer check, the others undercount or misclass one session each.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #463 | closed by this ADR's PR (`Closes #463`); the work is tasks 1–5 | the design issue |
| #438 | kept closed | ADR 0015's fix for the idle sessions; cited for the cost |
| #461 | kept closed | the `Undefer when:` lines this ADR's skip-label table reads |
| #146, #236 | kept as they are | cited for the stale-deferral days |
| #423, #440, #433 | kept as they are | cited for the owner interventions of 2026-11-28 |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | Daily retro rules: `cargo xtask next-session` step 7's retro term becomes "no retro record dated today (UTC)" plus today's incident terms (specs/tooling/next-session.md step 7, with Verification's fixtures); ops/TICK.md §2 rule h and §7 rewritten for invariant clauses 1–6 (daily and weekly parts, `Kind:` line, action-item windows, `stall` issues; §7 step 1's "remove `deferred`" becomes a `stall` issue for the Planner, C1; "PLAN task order" leaves step 4, C4); §5's `Undefer when:` forms gain `work mix ≥ 70%` and §9 step 1a checks it from the brief's `work mix:` note; driver sessions never add `human` (§0); AUTOMATION.md §11 (when, input, allowed changes); a Status note on ADR 0011 that §1's Retro row and §2's read-only bullet are amended by this ADR; create the `stall` label. | #463 |
| 2 | `cargo xtask next-session` pick log: each live run appends one line to the main checkout's `ops/state/picks.tsv` (time, exit code, first line, waiting issue numbers for `idle`); `--from` writes nothing; a write failure warns on stderr and never changes the pick or the exit code; specs/tooling/next-session.md (outputs, Security: the file is unprotected and forgeable, numbers only) | #463 |
| 3 | `cargo xtask metrics` R13 part 1: the milestone section (open count per day, filed and closed, forecast, product share of merges) and the session section (index.tsv joined with `picks.tsv`, the six classes, idle hours and idle hours with work waiting); specs/tooling/metrics.md | #463, task 2 |
| 4 | `cargo xtask metrics` R13 part 2: the skip-label table (count, ages, `Undefer when:` conditions read only from `jacobdam` comments, the three machine forms, stale deferrals) | #463 |
| 5 | `cargo xtask metrics` R13 part 3: owner interventions (the five kinds, the two exempt kinds, the undercount note), reading the owner feed's untagged test from `ops/driver/owner-issues.sh` rather than restating it. Strict lane: it reads who counts as the owner (ops/TICK.md §4 Lanes) | #463, task 3 |

Each task issue carries `Designed in: #463` and `Depends on:` lines (ops/TICK.md §8 step 5).
The lane of tasks 1–4 is set at their PR by ADR 0012 §1 (task 1 changes TICK text about the
retro, which is not on the strict list; task 5 is strict). The same-day stall alarm (#463's
"out of scope": the driver notifies after N idle sessions in a row) is not filed: with a daily
retro and ADR 0015, a stall costs at most a day; if the idle hours with work waiting stay above
0 for 7 days, a retro files it as an action item (a driver proposal).

## What reviewers check against
P7, added to `docs/PRINCIPLES.md` by this ADR:

> **P7. The retro measures R13 daily and never gates or orders work.** Paths:
> `docs/retros/`, `xtask/src/metrics.rs`, `xtask/src/metrics/`, `specs/tooling/metrics.md`,
> `ops/TICK.md`.

The full entry (its invariant, enforcement and "Reviewers check" lines) is in
`docs/PRINCIPLES.md`.

## Requirements on future callers
- A new session type (QC, the Gardener, the user advocate, ADR 0011 §1) gets a session class
  in "Where did sessions go?" in the PR that adds its pick; until then its sessions count as
  *process*.
- A new owner command or owner-only label gets a row in the intervention list (counted, or
  exempt with the rule that leaves it to the owner) in the PR that adds it.
- A change to the pick's first-line format (specs/tooling/next-session.md) updates the class
  rules in the same PR, or its sessions fall to *process* or *unknown*.
- A Planner change that adds `ready` outside a `planner…` pick (a new gate) changes
  intervention kind 3 in the same PR, or every such add counts as the owner's.

## Consequences
- **Easier:** a stall is diagnosed within a day, in an issue the Planner gates the same day;
  the owner's clicks become numbers with a target of 0; the retro's verdicts rest on 7-day
  windows, so a daily cadence doesn't turn into churn.
- **Harder:** one retro session a day (step 7 outranks ready work), a retro PR a day to review
  (records plus, at most, one action item), and up to 365 records a year under `docs/retros/`.
  The scheduler gains its first write (a local log line).
- **Not covered:** owner actions inside a driver session window other than `ready` adds
  (undercounted); sessions before the pick log exists (*unknown*, with the duration note);
  whether an action item caused the change in its number (the 3-item cap and distinct numbers
  make it likelier, not proven).
