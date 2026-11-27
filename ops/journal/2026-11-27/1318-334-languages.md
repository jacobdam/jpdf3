## 13:18 UTC #334 Product: journeys J-PC-09, J-PC-10, the app's four languages
- Session: design product #334
- Picked: rule k (Design: product) because #334 is the lowest open `design-product` and `ready` issue and no open
  agent PR links a `design-product` issue
- Did: PR #379 (label `design`). specs/journeys/signatures-platform.md: Language rules in the PC shared rules (app
  vs platform-supplied text, per-platform catalogs and their contract test with a same-as-English list and a key per
  engine error code, spot strings, formats with the app language and system region, the document never translated,
  the language-and-script match), J-PC-09 (follow the system or browser language, with a fallback branch) and
  J-PC-10 (choose a language, switch without relaunch or reload). specs/TRACE.md R8; docs/PRODUCT.md §2 row and §4
  promise. Filed #380 (specs/ui Preferences Language row; M10, `discovery`, no `ready`). Three owner choices in the
  PR description (Traditional Chinese → English; macOS in-app picker; region in formats). Removed the merged
  `t/372-done-reply` worktree.
- Result: 4 rounds; `ai-review` success posted on this commit; PR marked ready; `owner-todo` on #334 with the
  24-hour window comment; auto-merge off until `worker design-window #379`
- Cost: ~45% of context; review rounds 4/4; regraded 0, overturned 0; deferred: #380 (R4.1 nit)
- Codex: skipped (no crates/ or tools/ path)
- Lesson: a journey fix that moves a check onto new UI elements must check those elements are on screen at that step
  and, on macOS, who draws them (R2.1, R3.1 each came from the previous round's fix); added to STATUS Pending lessons
- Ignored: none
