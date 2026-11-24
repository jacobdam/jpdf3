## 02:28 UTC #298 next-session: step 3 skips the owner's own PR on an owner/ branch
- Session: worker own-pr #290
- Picked: c, but PR #290 is on the owner's `/hold` (TICK §5: nothing moves), and its fix, PR #296 (#295), had
  `ai-review` success and was still in CI. Run from #296's branch, the scheduler picked `worker own-pr #291`: also an
  owner-apply PR (`owner/codex-homebrew-read`, signed protected change, owner posts its `ai-review`; issue #196 is
  `owner-todo` but the PR says `Refs`). Nothing for a session to do there either, so filed #298 (`bug`) and worked it.
  b: main green at the start (5e4f6f7; the newest main ci run was in progress). No design PR waits out a window.
- Did: PR #299 (branched from PR #296's head): `Pr::parked()` = held or head branch starting with `owner/`; step 3
  skips parked PRs, step 1 treats them as in flight, and the brief notes each owner-branch PR (no branch name). Spec
  specs/tooling/next-session.md (step 3, Output, Security, Tests), one TICK §2 clause, one test with 7 near-miss
  branch names; two hand mutants (skip reduced to the hold check, note loops swapped) fail it. Live run on the
  branch: `worker scheduler-bug #298` with notes for #290 and #291.
- Result: `ai-review` success posted on this commit, auto-merge on (merges after PR #296).
- Cost: ~30% of context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none.
- Codex: skipped (the diff touches `xtask/`, `specs/`, `ops/`, not `crates/` or `tools/`).
- Lesson: none new. A step 3 pick of a PR the session can't move (held, the owner's own) is a scheduler bug to fix,
  not a session to spend; check the fixed scheduler from the fix branch before picking up anything else.
- Ignored: none.
