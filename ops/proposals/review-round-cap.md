# Proposal: cap re-review rounds in TICK §4

Requested by the owner on PR #151 (2026-11-22). ops/TICK.md is not protected, so a retro may apply this directly. It
is written as a proposal because the owner asked for one.

## Problem
TICK §4 step 5 asks for a fresh re-review whenever a blocker or major was found, with no limit. Rule c's 3-attempt
cap covers CI fixes, and sessions have not applied it to review rounds.
- PR #121 took 4 rounds, PR #142 took 6 and PR #151 took 15.
- In PR #151 the subagent approved in round 8. Every later round had a new second-reviewer (Codex) major about an
  unproven bound or a bypass of best-effort tooling. Each fix grew the diff and drew the next major.

## Change (TICK §4, step 5)
- **Cap: 3 re-review rounds per PR** (rounds 1 to 4 in total).
- After the cap:
  - findings from the second reviewer only, and findings about heuristic tooling or an unproven bound, go to a new
    issue (`deferred: #N` in the triage), even if marked major;
  - only real correctness, security or CONSTITUTION findings in the PR's code block the merge, and they are fixed;
  - if such a finding remains after one more round, label the issue `blocked`, as rule c says.
- **Scope rule:** a reviewer's request to add enforcement tooling (scanners, tripwires) for a stated limitation is
  deferred to its own issue, not built in the same PR.
- Record the round count and any deferrals in the journal's `Cost:` line.

## Success check
Over the next 10 merged PRs: no PR goes past 4 review rounds, and each deferred finding has an issue. The retro after
that checks whether any deferred finding turned out to be a real bug that should have blocked its merge (target: 0).
