## 01:50 UTC #433 Move the ci linux dashboard copy into ci_linux.rs
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/433#issuecomment-7231751189
- Picked: rule a because the owner's untagged "Agree" on #433 (filed on the owner's behalf, tagged) makes its "Do:" an owner instruction with no "done:" reply
- Did: PR #435 (strict lane). `copy_into_site`, `Copied`, `SNAPSHOT`, `MAX_SNAPSHOT_BYTES` and the 1 MiB boundary test `copies_the_snapshot_up_to_one_mib` moved unchanged from xtask/src/dashboard.rs (process) to xtask/src/ci_linux.rs (tooling); `cargo xtask dashboard` imports `SNAPSHOT` from `ci_linux`; specs/tooling/dashboard.md and ci-linux.md name the new file. Checked the move by diffing the sorted removed and added lines. Posted "picked up" (https://github.com/jacobdam/jpdf3/issues/433#issuecomment-7231949672). Removed the merged worktree t/422-design-window-records (scheduler note).
- Result: `ai-review` success posted on this commit, auto-merge on; "done:" on #433 waits for the merge (a later session posts it, §0). Local: fmt, clippy, nextest 702/702, check-specs ok.
- Cost: about 15% of the context; review rounds 1/4 (1 nit, fixed); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
