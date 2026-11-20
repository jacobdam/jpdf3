# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #24 (#4 jpdf-cli stub): reviewed (6 rounds, no blockers left), auto-merge on. If it is
  not merged at session start, check its CI first (TICK rule c).

## Last session
- #4 jpdf-cli stub:
  - All CONTRACT §1 commands are parsed; `info` counts pages for files with a classic xref and
    a flat page tree.
  - Limits are hardened against hostile files and verified under a 2 GiB RLIMIT_AS on Linux.
  - 300 system PDFs: 255 open (all match `pdfinfo`), 45 exit 3 (44 nested page trees, 1 xref
    stream), 0 crashes.
  - Judge on an earlier commit: 0 crashes, open_rate 0.59.
- Filed #25 (nightly harness for the attack files, M0) and #26 (lexer cost per name and
  object budget, M1).

## Next step
- Once #24 merges: check that #4 closed (the PR body says "Closes #4"). Add `ready` to issues
  whose dependencies are now all closed: #21 still waits on #12, #13 and #19.
- Then the lowest `ready` M0 issue: #5 (spec checklist check in xtask + TRACE.md).
- A cheap open-rate win for later: nested page trees (44/45 of the local failures).

## Blockers / owner-todo
- Owner: import ops/rulesets/main.json; grant Screen Recording + Accessibility to Terminal
  (macOS UI tests, PDFgear comparison). #21 will need the owner for some drills.
