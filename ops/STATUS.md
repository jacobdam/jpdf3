# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- **PR #151 (#146, sev:high), draft, `ai-review` = failure on 26295fa. Resume it first (rule c).**
  - The budget change itself converged: the subagent approved it in round 8.
    - `heap_high_water`: the peak counts the largest phase twice.
    - Six lowered defaults: xref entries 2^21; retained 128 MiB; strings per parse 32 MiB; decoded per stream
      128 MiB; raster working 256 MiB; canvas 2^22.
    - Peak estimate 1909.84 MiB.
  - Still open: two Codex majors in the tripwire, tools/jpdf-cli/tests/phase_gate.rs (S14.1, S14.2; see the round-14
    triage).
  - Plan:
    - replace the manifest text scan with `cargo metadata --no-deps` target `src_path`/kind checks;
    - make the source name scan advisory, or move it to its own issue;
    - keep the `cargo tree` crate and package-identity check as the enforced part;
    - re-review, post `ai-review`, merge.
  - #146 stays open after the merge: the cross-phase heap build-up is a stated, unbounded gap. Its gate needs a
    structural fix or an enforced runtime budget before decode or raster reaches the jpdf-cli path.

## Last session
- #146 / PR #151 (rule d): 14 review rounds, not merged (see In flight). Journal 2026-11-22 02:28.

## Next step (suggested order, TICK letters)
0. c: finish PR #151 (above).
1. **g: AUDIT is overdue**: 18+ merges since audit 0002. d is now empty apart from #146.
   - Security issues: high is #146 (in flight). Medium/low wait in M10: #114, #116, #117, #141 (medium); #83, #122,
     #132, #134, #136, #137 (low).
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i) in M0, lowest number first: #21, #86, #91, #115.
   - #14, #20 and #22 close only after issues in later milestones (#55, #70; #61; #64).
- **Next retro (rule h): by 2026-11-28.** Inputs:
  - review rounds: PR #121 took 4, PR #142 took 6 and PR #151 took 14+. Rule c's 3-attempt cap isn't applied to
    review rounds. Decide a cap, and when a second reviewer's majors may be deferred.
  - PR #151: a text-scan tripwire turned review into a bypass hunt (LESSONS).
  - STATUS "in flight" goes stale as soon as its PR merges.
  - the template's `Area:` list lacks `tooling`.

## Blockers / owner-todo
- Local XCUITest on the Mac mini needs an unlocked, logged-in console.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- #126 (open): the Codex second-reviewer trial is live. It ran every round on PR #151 (status ok each time).
- Suggested owner follow-up (no issue yet): a command size cap in guard.py and an explicit hook `timeout`.
- 8+ merged remote `t/*` branches wait for the owner to delete them (the guard blocks remote deletes).
