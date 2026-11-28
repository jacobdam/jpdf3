## 02:30 UTC #413 cargo xtask admit: vet and build-time steps, feature flags, OFL-1.1 note
- Session: worker ready #413
- Picked: rule i because it was the lowest-numbered ready M0 issue with its dependencies closed (#303 closed; work mix below 70%, no ready product issue)
- Did: PR #441 (strict lane). Ported the vet and build-time steps, `--features`/`--no-default-features`, the metadata read and the OFL-1.1 note from the unreviewed draft e35aa0b onto main's reviewed core (not merged from it); spec specs/tooling/admit.md and specs/TRACE.md updated. Live runs: adler2 admitted through all five steps; serde_derive rejected by unsafe, vet and build-time; a feature set admitted; an unknown feature fails the probe. 16 hand mutants caught. Removed the merged worktree t/403-status-exemption (scheduler note). Filed #442 (R1.4: the store imports `isrg`, which CONSTITUTION 9 doesn't list).
- Result: `ai-review` success posted on this commit, auto-merge on. Local: fmt, clippy, nextest 709/709, check-specs ok.
- Cost: about 45% of the context; review rounds 2/4 (round 1: 4 minors fixed, 1 deferred, 1 nit; round 2: 1 nit fixed); regraded 0, overturned by the next reviewer 0; deferred: #442
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none new (the guard's `unsafe` check also blocks a heredoc python edit of a Markdown spec that names the scan step; LESSONS already says to use Write/Edit)
- Ignored: none. Noted: the owner's "Look ok" on #438 (https://github.com/jacobdam/jpdf3/issues/438#issuecomment-7232225885) is in the owner feed with no "done:" reply; `cargo xtask next-session` picked #413, not rule a, at this session's start, so this session left it; a later session should check why the scheduler skips it.
