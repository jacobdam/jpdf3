## 03:09 UTC #246 Session types 4 1/3: ops/areas.toml and cargo xtask issue-lint
- Session: worker ready #246
- Picked: i, the scheduler's step 10 pick. Step 6 checked by hand: no `needs-design` PR, no `design`+`ready` issue.
  Owner feed: only comments from before the floor; recently closed owner issues need no "done:". main green
  (newest runs were owner-commands, skipped). No design PR in a 24-hour window.
- Did: split #246 first (sizing rule: one tool, plus labelling 91 open issues and a metrics extension, would not fit
  one session): #246 is now 1/3 (areas.toml and the lint), filed #308 as 2/3 (labels, TICK filing rule, per-area
  metrics; no `ready`), renumbered #247 to 3/3 (depends on #308). PR #309: `ops/areas.toml` (16 areas: 13 product,
  3 process), `cargo xtask issue-lint` (xtask/src/issue_lint.rs, a strict TOML-subset reader, `--from` fixtures in
  xtask/tests/issue_lint.rs), specs/tooling/issue-lint.md, AUTOMATION.md §3 and §4.1, TRACE R13. `cargo mutants` on
  the new file: 102 caught, 5 unviable, 0 missed (after adding argument and line-number tests). Live run: exit 1,
  184 problems in 92 of 92 open issues (no labels yet), no area-file problem. Removed the t/298-owner-branch
  worktree, as the scheduler's notes said.
- Result: `ai-review` success posted on this commit, auto-merge on.
- Cost: ~55% of context; review rounds 3/4; regraded 0, overturned by the next reviewer 0; deferred: none.
- Codex: skipped (the diff touches `xtask/`, `ops/`, `specs/`, AUTOMATION.md, not `crates/` or `tools/`).
- Lesson: clippy's `allow-expect-in-tests` covers `#[test]` functions only, not helpers in an integration test file
  → write test helpers that return `io::Result` and use `?`. A lint whose own rule (no comment inside an array)
  then rejected the repository's file was caught only because a test parses the real `ops/areas.toml`: keep that kind
  of test for every config file a tool reads.
- Ignored: none.
