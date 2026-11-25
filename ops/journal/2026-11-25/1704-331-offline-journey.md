## 17:04 UTC #331 Journey J-PC-08: the web app offline (PR #347)
- Session: design product #331
- Picked: rule k (Design: product) because #331 is the lowest open `design-product` issue labeled `ready`, and nothing in steps 1–7 applied
- Did: PR #347 (labeled `design`, strict lane): new journey J-PC-08 in specs/journeys/signatures-platform.md (M10, web, `e2e.web.pc_offline`: load once online, check the manifest and that Cache Storage holds exactly the build's asset list; offline with the server stopped, reload, open a defined 12-page fixture with a CMap-dependent Japanese page, search, annotate, save, reopen; no network fetch, no document bytes in Cache Storage); header of the file updated; J-PC-08 added to specs/TRACE.md's R11 row (check-specs requires it); docs/PRODUCT.md §4's offline promise points to J-PC-08. Deferred R4.1/R4.2 to #348 (`design-product`, `ready`, M10).
- Result: 4 review rounds, round 4 approved; `ai-review` success posted on this record commit's head; the PR is a later change to docs/PRODUCT.md, so it gets the 24-hour window (TICK §10 step 6, §8 step 5): `owner-todo` on #331 with the window comment, no auto-merge until `worker design-window #347`.
- Cost: ~45% of context; review rounds 4/4; regraded 0, overturned by the next reviewer 0; deferred: #348
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: a journey fixture that names a non-embedded CJK font can't render: PLAN §3 bundles only CMaps and standard-14 substitutes (R3.1) → embed a subset and check PLAN §3 "Bundled data" before specifying fonts in a fixture (STATUS pending lessons).
- Ignored: none
