# 0015. Issues filed after the day's Planner run wait for the next UTC day while the loop idles

Status: proposed (design issue #438; filed in an owner interactive session, made an owner
instruction by the owner's comment on it, ADR 0011 §7). Amends ADR 0011 §1's Planner bound
("at most once a day"). A design ADR merges 24 hours after the latest of: its PR marked ready,
its last `ai-review` success, the owner's last `/release`, unless the owner comments `/hold`
(ADR 0011 §7; ops/TICK.md §8 step 5).

## The problem class
The Planner is the only agent gate into `ready` (ADR 0011 §1, ops/TICK.md §9), and the
scheduler runs it at most once per UTC day. Step 8 (`planner due`) is off once a
`- Session: planner` record is dated today, and step 11 then prints `idle` instead of
`planner` (specs/tooling/next-session.md steps 8 and 11). Three kinds of session file Worker
issues without `ready` and leave them for the Planner: a design session's ADR tasks
(ops/TICK.md §8 step 5), a sizing split's later parts (§0) and discoveries and review
deferrals (§5, §4 step 4). An issue filed after the day's Planner run therefore waits until
the first session after 00:00 UTC. Once the ready list is empty, every session until then
prints `idle`, even while that issue needs only a label.

Seen on 2026-11-28 (#438):
- The Planner ran at 00:04 UTC (`ops/journal/2026-11-28/0004-planner.md`) and readied #366,
  #367, #369, #383, #403 and #413.
- The design session for #420 (PR #422) filed ADR 0014's tasks #423–#427 at 00:34–00:35 UTC,
  without `ready`. #423 depends only on #420, which is closed. Discovery #431 was filed at
  01:17 UTC.
- By 02:16 UTC #366, #367, #369 and #383 had merged or been worked, #403's PR (#437) was merging
  and #413 was the scheduler's next pick. After #413 the loop would idle until 00:00 UTC on
  2026-11-29, about 20 hours, while six issues in M0 waited for a Planner run.

An earlier case with the same cost, sessions idling while work existed: #412 (about 70 idle
sessions). Its cause was different: the scheduler read a `Depends on:` mention inside prose
as a dependency line. It is named here only for the size of the cost.

The class: **a bound written as a fixed rate** (once per UTC day) where the work it gates
arrives at any time. ADR 0011 §1's table names the trigger "no `ready` issue has its
dependencies met", and its bound "at most once a day" turns that trigger off for the rest of
the day after the first run.

## Why point fixes don't converge
There have been no point fixes yet. Each one at hand fixes a single source of new issues:
- **Design sessions file their first task `ready`** (#438 option 2) covers ADR tasks only.
  Discovery #431 and a split's parts wait the same way. It also moves part of the gate into
  `ready` from the Planner to the design ADR's review, which checks the design, not whether
  the task fits the milestone (the Planner's step 2 milestone moves). And the tasks are
  filed before the ADR merges, with `Depends on: #<design issue>`, so the label would sit
  on issues whose design can still be `/hold`ed.
- **Narrowing the rerun to `Designed in:` issues** (#438 option 3) leaves #431 waiting.
- **The owner adding `ready` by hand** works, and it is what ADR 0011 §1 is meant to replace.

What is missing is a decision about what the Planner's bound protects. A Planner run costs a
session. The bound exists so that the scheduler can't pick the Planner again and again while
nothing changes (ADR 0011 §1: "at most once a day"). A run is wasted only when no issue has
appeared since the last run, because the Planner has already judged every issue that existed
then. So the bound can be a count of runs per day plus a trigger: **an issue the last run
could not have seen**.

## The invariant
1. **One gate.** Agent sessions add `ready` only in a Planner session, except for the
   design and product queues that ops/TICK.md §9's opening names (`design` plus `ready`,
   `design-product` plus `ready`). This ADR doesn't change that.
2. **At most three Planner picks per UTC day.** The scheduler picks `planner due` or a
   `planner…` line only while fewer than 3 distinct journal files dated today (UTC) hold a
   `- Session: planner…` record (first word `planner`, so `planner due` counts). A file found
   on two refs (main and an open agent PR) counts once.
3. **A run after the first needs a new issue.** On a day that already has a Planner record,
   step 11 picks the Planner (first line `planner new-issues`) only when nothing before it
   applies and at least one issue is *new*. An issue is new when it is open, in the current
   milestone (step 10's: the lowest open `M<digits>`), has no `ready`, no skip label
   (`owner-todo`, `blocked`, `deferred`) and none of `in-progress`, `design`,
   `design-pending`, `design-product`, is not in flight (no open agent PR links it), has every
   issue on its `Depends on:` line closed (step 10's test; added by owner issue #478, so a
   design ADR's task waiting for its ADR spends no rerun), and was
   created at or after the end of the minute of today's newest Planner record. That
   record's minute is the `HHMM` at the start of its file name (`ops/journal/<day>/HHMM-<slug>.md`,
   ops/TICK.md §5). A name that doesn't start with four digits forming a valid time
   (`HH` < 24, `MM` < 60) and a `-` reads as the end of its day. Such a record makes no issue
   new, so a malformed name can cost at most the rest of the day, the same as today's bound.
   Otherwise step 11 prints `idle`, as today.
4. **Step 8 is unchanged:** once any Planner record is dated today, its terms are off for the
   day. Step 10 is unchanged: a new issue that the rerun readies is picked there.

A reviewer can test each clause with a fixture. For example: a Planner record
`ops/journal/2026-11-28/0004-planner.md`, an M0 issue created at `2026-11-28T00:04:59Z` (not
new) and one created at `2026-11-28T00:05:00Z` (new). With the second issue and three
Planner files dated today, the pick is `idle`.

### Bound per day, work mix, near misses and forged records (#438's questions)
- **Bound:** 3 Planner sessions per UTC day, the first one as today and at most 2 reruns.
  Each rerun takes the place of an `idle` session, so a rerun replaces a session that would
  have done nothing; it never displaces other work. A rerun's record dates the newest run,
  so every issue that existed before it stops being new. A further rerun needs an issue filed
  after that, and the cap ends the chain. Issues the Planner files during its own run, after
  its record's minute, count as new. That can cost one rerun, which is inside the cap. So do
  a design ADR's tasks, filed with `Depends on: #<design issue>` while that issue stays open
  for its 24-hour window (ops/TICK.md §8 step 5). That rerun isn't wasted: the Planner may
  ready an issue whose dependencies are open (§9 step 2 asks only that they are named), and
  step 10 picks it once the ADR merges. But a design PR's tasks and a split's later parts can
  spend both of the day's reruns, and a discovery filed after them waits until 00:00 UTC.
  Leaving out issues with an open dependency would be worse: they would never be new, so
  they would wait for the next day even after the ADR merges.
- **The cap counts visible records only:** the journal files that `records.txt` lists
  (origin/main and the heads of open agent PRs). A record can leave that list: a Planner's
  records PR closed unmerged (step 11 already says such a record doesn't count), a blob that
  can't be read (input 15), or a session that dies before it pushes. Each lost record allows
  one run beyond the cap, and if it was the newest, issues it had judged count as new again.
  This is the same exposure the once-a-day bound has today (a lost record costs at most one
  more Planner run that day), not a new one.
- **Work mix** (ADR 0011 §3): a rerun's record counts as *other* (in the window, never
  milestone work), like any Planner record. At most 2 more such records a day can push the
  reading below 70% more often. That makes step 8 due (already off for the day once a
  Planner ran) and orders step 10 product-first. Neither one blocks or hides an issue (the
  budget only orders).
- **Near misses the scheduler's tests need:** an issue created one second before the end of
  the record's minute (not new) and at its end (new); the record dated yesterday (no rerun:
  step 11 is then the day's first `planner`); each skip label, `ready`, `in-progress`,
  `design`, `design-pending` and `design-product`; an `Ops` issue, an issue in a later `M<n>`
  and one with no milestone; an issue in flight; 2 Planner files today (rerun) and 3 (`idle`);
  one file on two refs (counts once); a `planner due` record (counts); `planners` and
  `Planner` (don't count, as today); two Planner records today, where only the newer one's
  minute counts, with the newer one listed first and the two on different refs (`0300-…` on
  main, `0004-…` on an open PR, so taking the last-listed record fails); a file name with no
  time, `2460-…` and `0060-…` (end of day); an earlier step
  outranking the rerun; a `createdAt` not in the form `YYYY-MM-DDTHH:MM:SSZ` (exit 1).
- **Forged or late records** (the spec's Security section): a record is self-reported, so
  anyone who can push a branch with a tagged open PR can write one. A forged record named
  `2359-…`, or 3 forged files dated today, suppress reruns for the rest of the day. That is
  today's bound, so the cost is the same as now: at most until 00:00 UTC. A forged record
  can't add a run, because a record that is visible only ever raises the day's count or moves
  the newest minute later (a record that leaves the list is the case above). An issue can be
  made new only by creating it in the current milestone, and
  setting a milestone needs triage access to the repository, which only the shared account
  has. An outsider's issue has no milestone, so it is never new. An agent session that files
  issues on every run can trigger at most 2 reruns a day.

## Enforcement
1. **Types:** not used. The bound is a count over self-reported records and a time
   comparison, and no type makes a fourth run unrepresentable. The rerun is a separate pick
   line (`planner new-issues`) with its own `Rule:` text, so a journal record shows which
   term fired.
2. **A lint or gate:** the scheduler is the gate. `cargo xtask next-session` computes the
   count and the trigger (task 1). Its `--from` fixtures cover every clause and every near
   miss above, and `cargo nextest` runs them in the judge's normal test step. No protected
   file changes: `issues.json` gains the `createdAt` field from the same `gh issue list`
   command.
3. **A measurement:** the retro counts Planner records per UTC day, and the sessions that
   printed `idle` while an issue that would be new existed (from `ops/logs/index.tsv` and the
   issues' creation times), against #438's baseline of about 20 idle hours on 2026-11-28.
4. **Reviewer vigilance:** P6 in `docs/PRINCIPLES.md` (below), for the parts the fixtures
   can't see: that every session which files Worker issues files them without `ready`, and
   that the live `createdAt` field is the one parsed.

## Verification
Task 1's fixtures: each clause of the invariant has a fixture that selects it, plus a near
miss for each filter listed above, and the precedence test gets a rerun entry (every step
applies, each one removed in turn). What broken implementation still passes?
- One that compares the issue's creation **day** to the record's day, not its minute: it
  passes unless a fixture puts an issue and a record on the same day and the issue is not
  new (the `00:04:59` case). That fixture is required.
- One that counts record **lines**, not files, or one file on two refs twice: it passes
  unless the cap fixtures put the same file on main and a PR branch. That fixture is
  required.
- One that takes the **oldest** Planner record's minute: it passes unless two Planner records
  dated today bracket the issue's creation. That fixture is required.
- One that takes the **last-listed** Planner record, not the latest minute: within one ref
  `git ls-tree` lists names sorted, so the two agree there. It passes unless the bracketing
  fixture lists the newer record first, on another ref (`0300-…` on main before `0004-…` on
  an open PR's branch). That fixture is required.
- One that reruns at step 8 rather than step 11: the precedence test catches it, since a
  ready issue (step 10) must outrank the rerun.
- What no fixture can catch: the live `createdAt` field shape (a rename is a parse error,
  exit 1, which TICK §2's failure rule turns into a bug issue), and whether the Planner run
  actually readies the issue (its judgement, ops/TICK.md §9 step 2).

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #438 | closed by this ADR's PR (`Closes #438`); the work is task 1 | the design issue |
| #412 | kept closed | a different cause (dependency-line parsing), cited only for the cost of idle sessions |
| #423, #424, #425, #426, #427 | kept | ADR 0014's tasks, waiting for `ready`. The next Planner run gates them (by the time task 1 merges, the 2026-11-29 run will have), and nothing in them changes |
| #431 | kept | a discovery waiting for `ready` the same way; the Planner gates it as usual |
| #420, #366, #367, #369, #383, #403, #413, PR #437 | kept as they are | cited for the 2026-11-28 timeline only |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `cargo xtask next-session`: step 11's rerun term and the 3-a-day cap (invariant clauses 2–3, pick `planner new-issues`, whose `Rule:` line lists the new issues' numbers, never their titles); `issues.json` reads `createdAt`; specs/tooling/next-session.md (step 11, inputs, parsing, Security, Self-check, tests list) with every near miss of this ADR's list as a fixture; ops/TICK.md §2 (rules j and step 11's picks) and §9 (picked as `planner new-issues` too; its `Session:` line); the AUTOMATION.md lines that say "at most once a day"; a Status note on ADR 0011 that §1's Planner bound is amended by this ADR. Strict lane (#438). | #438 |

Task 1 carries `Designed in: #438` and `Depends on: #438` lines (ops/TICK.md §8 step 5).

## What reviewers check against
P6, added to `docs/PRINCIPLES.md` by this ADR:

> **P6. The Planner gates `ready`, at most three times a UTC day, and reruns only for a new
> issue.** Paths: `xtask/src/next_session.rs`, `xtask/src/next_session/`,
> `xtask/tests/next_session.rs`, `specs/tooling/next-session.md`, `ops/TICK.md`.

The full entry (its invariant, enforcement and "Reviewers check" lines) is in
`docs/PRINCIPLES.md`. Reviewers of task 1 also check the near-miss list above against its
fixtures, one by one.

## Requirements on future callers
- A new session type that files Worker issues (the Gardener, QC, the user advocate, ADR 0011
  §1) files them without `ready`, in the milestone the Discoveries rule gives (ops/TICK.md
  §5). The rerun then picks them up the same day. None of those sessions adds `ready`.
- A session that records itself as the Planner writes `- Session: planner…` only when its
  pick's first line starts with `planner`. Any other session that wrote such a record would
  spend one of the day's runs.
- A change to the cap (3) goes through a retro (ops/TICK.md §7 step 4, task sizing and xtask
  tooling), with its count of reruns and idle sessions per day.
- If the journal's file-name format changes (ops/TICK.md §5), the record minute's parse
  changes in the same PR, or every record reads as the end of its day and reruns stop.

## Consequences
- **Easier:** an issue filed while the ready list is empty gets the Planner's judgement within
  one session, not up to 24 hours later. The Planner stays the only gate (ADR 0011 §1), and
  design sessions file their tasks as they do today.
- **Harder:** up to 2 more Planner sessions and records a day. Each one is a session that
  would otherwise have printed `idle`, and its record lowers the work-mix reading. The
  scheduler reads one more field (`createdAt`) and parses the record file names' times.
- **Not covered:** an issue that becomes gateable after the day's run but was created before
  it is never new, so it waits until the next UTC day as today: an issue whose `blocked`,
  `deferred` or `design-pending` label is removed, or an `Ops` or later-milestone issue moved
  into the current milestone. The Planner's own moves (ops/TICK.md §9 step 2, ADR 0012 §2)
  can ready the issue in the same run. That leaves three cases: a removed label, an issue the
  owner moves in by hand, and a Planner run that closes `Mn` (§9 step 4), after which the
  current milestone is `Mn+1` and its open issues (11 in M1 on 2026-11-28, #308 and #248 among
  them) were all created before that run's record, so none is new. The retro's count
  (Enforcement level 3) shows whether any of them needs a term of its own. An issue filed while other work is ready waits until the ready list
  empties. Step 11 runs last, by design: the Planner orders work, it doesn't interrupt it.
  Issues filed in `Ops` or another milestone never trigger a rerun (step 10 doesn't pick
  them either). A day whose 3 runs are spent waits until 00:00 UTC, as every day does today.
