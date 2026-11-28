## 02:07 UTC #403 Record commits: drop the retired ops/STATUS.md exemption
- Session: worker ready #403
- Picked: rule i because it was the lowest-numbered ready M0 issue with its dependencies closed (work mix below 70%, no ready product issue)
- Did: PR #437 (strict lane). Took the issue's second fix: no open PR carries `ops/STATUS.md` (#388 merged), so the record-commit test in ops/TICK.md §4 step 6, §8 output 3, §9 step 6 and ADR 0011 §3 no longer exempts it, TICK §5's merge-conflict note for it is gone, and the ops/areas.toml records comment says it is not a record path. No code classifies record commits, so no test changes.
- Result: `ai-review` success posted on this commit, auto-merge on. Local: check-specs ok, nextest 702/702.
- Cost: about 15% of the context; review rounds 1/4 (1 nit, fixed); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none
