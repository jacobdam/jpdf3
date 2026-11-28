## 04:56 UTC #423 test-codex.sh passes against the live codex-review.sh (ADR 0014 task 1)
- Session: worker ready #423
- Picked: rule i because it was the lowest-numbered ready M0 issue with its dependencies (#420) closed (work mix below 70%, no ready product issue). `cargo xtask status` minutes earlier named #440; the scheduler's own run picked #423.
- Did: PR #458 (strict lane). Case 3 of `ops/proposals/codex-review-isolation/test-codex.sh` now evaluates the whole `q()` to `FS_POLICY+=" }"` block in a subshell with the quoted HOME; its `/opt/homebrew` + 8-key assertion (pre-#291 policy) became: no prefix key, 16 entries, all four HOME grants, every other key a normalized path inside the prefix (exact list left to test-policy.sh). `extract` now needs its end marker and every caller stops on `|| exit 2`. Moved the pending lessons of 0304-304-backend-selection into ops/LESSONS.md.
- Result: live file `pass=45 fail=0` on macOS (was an abort in case 3); test-policy.sh 3 passed; 13 hand mutants caught. `ai-review` success posted on this commit, auto-merge on.
- Cost: about 25% of the context; review rounds 2/4 (round 1: 2 minors, fixed; round 2: 1 nit, fixed in the PR description); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
