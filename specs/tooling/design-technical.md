# Design: technical session (1/2)

Area: tooling
ISO 32000-2 clauses: none (process)
Journeys: none (autonomy system; docs/adr/0011-session-types-and-scheduler.md §1, §5, §6, §7)
Status: implemented

## Behavior
ADR 0011 task 3, part 1 (#245): the Design: technical session type, as process text and
templates. Part 2 (#275) adds precedence step 6 to `cargo xtask next-session`
(specs/tooling/next-session.md), which picks the design session.

- **Labels.** `design`: an issue queued for a design session (or a design ADR PR). `needs-design`:
  an agent PR over ADR 0010's review cap, escalated to design.
- **Escalation** (ops/TICK.md §4 step 6, ADR 0011 §5). Where step 6 sent a PR to the owner (the
  final check reported anything, or the post-cap diff was over 100 lines or added a file), the
  PR goes to draft with `ai-review` failure and gets `needs-design`, then its issue gets a
  `🤖 jpdf-agent escalated to design: PR #N` comment with the ledger (that order, so a session
  that dies midway leaves a PR the next design session resumes). If the issue already has such a
  comment naming another PR, or a `Designed in: #<n>` line, the owner gets it as before
  (`blocked`, `owner-todo`, both ledgers). TICK §4 step 6 carries the state table.
- **Picking** (ops/TICK.md §2): the scheduler's step 6, after steps 1–5 (so rules d and e come
  first) and ahead of 7, 8, 10 and 11, prints `design-technical needs-design #N` for the
  lowest-numbered open agent PR labeled `needs-design` where neither it nor its issue carries
  `owner-todo`, `blocked` or `deferred` (not held, not on an `owner/` branch), else
  `design-technical design-issue #N` for the lowest-numbered open issue labeled `design` and
  `ready` without those labels and not linked by an open agent PR. Step 3 no longer picks a
  `needs-design` PR (its `ai-review` failure is on purpose).
- **The session** (ops/TICK.md §8): loads the issue group, ledgers, area code and ADRs,
  `docs/PRINCIPLES.md` and the template; picks exactly one output: 1 split (only lines already
  reviewed), 2 ADR or principle (a design PR closing a design issue, tasks with `Designed in:` and
  `Depends on:`), 3 ruling (never on a blocker, CONSTITUTION, `sev:high`+ security, or an already
  rejected dispute or regrade; then one final check with a line per ruling), 4 owner question
  (`owner-todo`). Outputs 1 and 3 are followed by one final check only when the diff since the
  last reviewed commit adds ≤ 100 lines and no file; otherwise, or when the check reports
  anything, output 4.
- **Resuming:** a design session first finishes what a dead one started (the missing escalation
  comment, an existing ruling and its final check, an existing design issue and ADR PR); it never
  rules twice on one PR.
- **Design PR merge** (ADR 0011 §7): after `ai-review` success the tasks are filed, then
  `owner-todo`; a later session's §1 enables auto-merge once 24 hours have passed since the
  latest of: marked ready, last `ai-review` success, the owner's last `/release`; and no `/hold`
  after the last `/release`. A push to a `design` PR turns auto-merge off first and restarts the
  window after the next `ai-review` success. A `design` PR finished by output 1 or 3 waits out
  the same window.
- **Template:** `docs/adr/_TEMPLATE-design.md` has the ADR 0011 §6 sections: the problem class,
  why point fixes don't converge, the invariant, enforcement (types, lint or gate, measurement,
  reviewer vigilance, in that order), verification, issues in the group, tasks, what reviewers
  check against, requirements on future callers, consequences.
- **Principles:** `docs/PRINCIPLES.md`, first entries P1–P3 from ADR 0008, ADR 0004 and
  CONSTITUTION 11 / ADR 0005, each with its invariant, ADR, enforcement, reviewer checks and
  `Paths:` (git glob pathspecs). The reviewer brief (ops/TICK.md §4) holds the entries whose
  paths match the diff. It also lists the concerns with no principle yet (#236, #223).

## Persistence
Labels and comments on GitHub; the files above on main. Nothing local.

## Undo
A wrong escalation is undone by removing `needs-design` and the escalation comment; the per-issue
bound counts the comment, so deleting it gives the issue its design step back (owner action).

## Errors
- A `needs-design` PR whose issue carried a bound mark before this PR's escalation (an
  `escalated to design:` comment naming another PR, or `Designed in:`): it should not exist
  (step 6 sends such a PR to the owner); a design session that finds one takes output 4.
- A design PR with code, tests or spec text: the reviewer grades it a blocker (ADR 0011 §1).

## Limits
Bounds (ADR 0011 §1 "every loop has a counter and a final state"):
- One design escalation per escalated issue: the `escalated to design:` comment and the
  `Designed in:` line. A chain has at most two design PRs (ops/TICK.md §8 step 6).
- Outputs 1 and 3: exactly one final check; its size limit is the post-cap rule's (≤ 100 added
  lines, no new file), the same boundary as ADR 0010.
- Output 4 ends the loop at the owner; no review runs until an owner comment.
- The per-area bound waits for areas (#246, #247).

## Security
- Labels and the escalation comment are set through the shared account, so anyone with write
  access could set them; strangers can't label. A stranger's comment imitating
  `escalated to design:` only moves the issue to the owner sooner (the safe direction).
- Rulings can't waive blockers, CONSTITUTION findings or `sev:high`+ security findings, and a
  fresh reviewer must accept each ruling. A ruling counts only from the repository's account
  (`jacobdam`, shared by owner and agent) with the tag, so a stranger's imitation of one is data
  and unlocks no review round.
- Self-check: no arithmetic on input; no checker or validator is added here (part 2 adds the
  scheduler step and its fixtures, specs/tooling/next-session.md). What a broken process still
  passes: a session that labels its own PR `needs-design` early, which takes it out of step 3
  and into step 6; the reviewer and the owner feed see it.

## Accessibility
Not applicable (no UI).

## Platforms
Not applicable.

## Performance
Not applicable.

## Tests
Process text: no automated test in this part. Part 2 (#275) adds `--from` fixtures for step 6 and
its near misses (xtask/tests/next_session.rs, `step6_*`). The first escalation is checked by hand against the state table in ops/TICK.md
§4 step 6.
