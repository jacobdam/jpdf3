## 10:13 UTC #431 dashboard contrast test: opacity, text fill color, pseudo-element content
- Session: none (a worker session started from the owner's interactive session; #431 was claimed for it there)
- Picked: the owner's interactive session handed this session #431 (`ready`, M0), already claimed
- Did: PR #505: `apps/web/e2e/dashboard.spec.ts`'s `textColors`/`expectReadable` now fail a checked text with an ancestor-or-self `opacity` below 1 or a `-webkit-text-fill-color` that differs from `color`, and check each `::before`/`::after` whose `content` isn't `none` (on the matched element or any descendant). Five new tests inject one fade each through a constructed stylesheet and expect the check to reject it. `specs/tooling/dashboard.md` §Accessibility and §Tests updated, with the fades still not checked. Six hand mutants on the invalid-state Alert pass the old test and fail the new one (two only in light mode, where `#eee` is faint); a near miss passes; six checker mutants each fail the new test named for them.
- Result: strict lane; review round 1: 2 minors and 1 nit, all fixed; round 2 (delta): fixes confirmed, no new findings; `ai-review` success posted on this commit, auto-merge on
- Cost: about 35% of the context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` change)
- Lesson: none
- Ignored: none
