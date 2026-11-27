## 22:55 UTC #412 Scheduler: a Depends on: line must start with the phrase
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/412#issuecomment-7229302839
- Picked: rule a because the owner commented "Yes fix it" on #412 (an issue filed in the owner's interactive session), with no done: reply
- Did: PR #415. `dependencies()` in xtask/src/next_session.rs reads only a line that starts with `Depends on:` (after leading whitespace and an optional `**`), one list per line; unit and pick tests for #303's old sentence and mid-line forms, with line-start nearest misses; specs/tooling/next-session.md step 10 and ops/TICK.md rule i say so. Before the change I compared every open issue's `Depends on:` lines under both rules: #70 and #55 named open #14 mid-line (`**Refs:** … Depends on: #14.`), so I moved that onto its own line in both bodies; #66, #369 and #45 lose mid-line references to closed issues (no pick changes).
- Result: `ai-review` success posted on this commit, auto-merge on; the done: reply on #412 waits for the merge (a later session posts it, §0)
- Cost: ~35% of context; review rounds 1/2 (light lane); regraded 0, overturned 0; deferred: none (R1.1 minor and R1.2 nit fixed)
- Codex: skipped (no crates/ or tools/ path)
- Lesson: an owner fix that narrows a parser → list the live inputs whose result changes before merging; here two real dependency lines (#70, #55) would have silently vanished
- Ignored: none
