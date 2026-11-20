# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #44 (#11, UI spec 2/2: wireframes for web and macOS, specs/ui/wireframes-*.md): reviewed;
  auto-merge on, waiting for judge-verdict.

## Last session
- Wrote web and macOS wireframes W-01…W-37 with per-platform tokens and a vision-reviewer
  checklist; aligned IA §6/§8/§10. Split the per-creator wireframes into #45.
- PR #43 (#9, forms and security journeys) merged; #42 labeled `ready`.

## Next step
- #42 (journeys 5/5: signatures and platform conventions) is `ready` → do it next. It must
  fill the IA rows that still say #42 (Signatures panel, Unsaved changes, File changed on
  disk). Wireframes W-21/W-22 already draw those two dialogs and say "journey pending in
  #42"; update them to the new journey IDs.
- Then #12 (web skeleton + Playwright) / #13 (macOS skeleton + XCUITest), which implement
  the W-05 layout from the wireframes. #45 (per-creator wireframes) is low priority.
- #21 still waits on #12, #13 and #19.

## Blockers / owner-todo
- Owner: the rule fire-drill steps in #21 (pilot fire drill) still need the owner.
