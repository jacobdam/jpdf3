# NNNN. <The problem class, in one line>

Status: proposed (design issue #<n>; escalated from #<issue> / PR #<pr>, or queued by <audit,
owner, session>). A design ADR merges 24 hours after its last `ai-review` success unless the
owner comments `/hold` (ADR 0011 §7; ops/TICK.md §8 step 5).

<!--
Template for the Design: technical session (ADR 0011 §6, ops/TICK.md §8). Answer every
section; write "none" with a reason rather than deleting one. A design PR holds this ADR,
docs/PRINCIPLES.md entries and nothing else: no code, tests or spec text (ADR 0011 §1).
-->

## The problem class
The group, with numbers: each issue, PR and review ledger in it (links), what they have in
common, and the one class of problem they are instances of.

## Why point fixes don't converge
What each fix changed, and why the next finding appeared anyway (per round or per issue). Name
the missing decision, not the last bug.

## The invariant
One or a few sentences that are true of the code at all times, checkable from outside the code
that maintains them. Write it so a reviewer can say "this diff breaks it" for a concrete input.

## Enforcement
How the invariant is kept, in this order of preference; say for each level why it is used or
why it can't be:
1. **types:** an API that can't express the violation;
2. **a lint or gate:** a check in `cargo xtask` or CI that fails on a violation (CI gates are
   protected: say if this needs an `ops/proposals/` file);
3. **a measurement:** a harness that measures the real property (memory, time, output) and fails
   on a regression;
4. **reviewer vigilance:** a "Reviewers check" line in `docs/PRINCIPLES.md`, the weakest level.

## Verification
How we know the enforcement works: the test, replay or measurement, and **what broken
implementation still passes it**.

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #<n> | superseded (closed with a link to this ADR) / kept / folded into task <k> | … |

Every issue named in "The problem class" has a row.

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | … | #<design issue> |

Each task issue carries `Designed in: #<escalated or design issue>` and `Depends on:` lines
(ops/TICK.md §8 step 3).

## What reviewers check against
The `docs/PRINCIPLES.md` entries this ADR adds or changes, quoted with their IDs, and the
paths they govern. Reviewers of later PRs on those paths get them in their brief (ops/TICK.md §4).

## Requirements on future callers
What code that doesn't exist yet must do to keep the invariant. Findings about future code go
here, not into new issues.

## Consequences
What gets easier, what gets harder, and what the invariant doesn't cover.
