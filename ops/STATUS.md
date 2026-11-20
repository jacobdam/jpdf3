# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- Nothing.

## Last session
- M0 bootstrap step 2 done: M0 backlog written as issues #4–#22 (milestone M0).
  `ready` (deps met): #4 cli stub, #5 spec check + TRACE, #6–#8 journeys, #10 UI IA,
  #12 web + Playwright, #13 macOS + XCUITest, #14 jpdf-traits, #17 fuzz, #18 bench,
  #20 Arlington, #22 corpus tiers.
  Waiting on deps: #9 (←#6–#8), #11 (←#10), #15 (←#12), #16 (←#14), #19 (←#17,#18,#22),
  #21 pilot fire drill (←#4,#12,#13,#19).
- Removed the merged leftover worktree `t/2-bootstrap-labels`.
- #2 (enable GitHub Pages) is resolved and closed; the `pages` job is green on main.

## Next step
- Work `ready` M0 issues lowest number first: #4 jpdf-cli stub. When an issue merges, add
  `ready` to issues whose dependencies are now all closed.

## Blockers / owner-todo
- Owner: import ops/rulesets/main.json; grant Screen Recording + Accessibility to Terminal
  (macOS UI tests, PDFgear comparison). #21 will need the owner for some drills.
