## 01:50 UTC #295 next-session: step 3 skips an own PR on the owner's /hold
- Session: worker own-pr #290
- Picked: c because the scheduler picked PR #290 (no `ai-review` on its head), but the owner's `/hold` on it has no
  `/release` after it (TICK §5: nothing moves), and the 01:41Z session already spent a session on the same pick. With
  nothing to do on #290, the session fixed the scheduler so the pick stops repeating: filed #295 (`bug`) and worked it.
- Did: PR #296: new input `holds.json` (comments of open agent PRs); a PR whose newest owner command (as
  `.github/workflows/owner-commands.yml` matches it: case-insensitive prefix and tag check, hold only on `/hold`
  exactly) is a hold is skipped by step 3, and step 1 treats it like a green PR; the brief notes it. Spec
  specs/tooling/next-session.md, one TICK §2 clause, tests with near misses; two mutants checked to fail the test.
  Live run on the branch: pick moved from `worker own-pr #290` to `worker own-pr #291`.
- Result: `ai-review` success posted on this commit, auto-merge on.
- Cost: ~35% of context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
  (R1.3 disputed, accepted in round 2).
- Codex: skipped (the diff touches `xtask/`, `specs/`, `ops/TICK.md`, not `crates/` or `tools/`).
- Lesson: `sed -i.bak` then `mv` back restores the old mtime, so cargo keeps the mutated binary; `touch` the file after
  restoring a mutant before any live run. Also: GitHub Actions' `startsWith`/`contains` ignore case, so mirroring a
  workflow's `if:` in Rust needs case folding (R1.1).
- Ignored: none.
