## 16:40 UTC Planner 2026-11-25: M0 gating and the work-mix reading
- Session: planner due
- Picked: rule j (Planner, §9) at step 8: the work mix is below 70% (0 of the last 20 sessions on milestone work). Rule a: `owner-issues.sh` lists only comments from before the 2026-11-23 floor and issues already labeled or handled; none needs a reply. b: main green. No open agent PR.
- Did: §1 owner-todo check: PR #343 (ops/proposals/driver-keep-open-worktrees.md) merged at 16:51 UTC and closed #342, so filed #345 (`owner-todo`) with the `ready for owner` comment, and removed the dead `in-progress` from #342. Gated `ready` onto #322, #323, #344 (each scoped and small; #344's #342 is closed). Added #322 to #302's `Depends on:` line (comment on #302). Step 4a: no product issues added (every open M0 issue is `xtask/`, `ops/` or owner work; PLAN §8's M0 row has no product deliverable left without an issue) and none moved out. Rewrote ops/QUEUE.md (ready list, waiting, critical path, Work mix). Removed the worktrees of merged PRs #341 and #343.
- Result: PR #346 (light lane), review 1 round (R1.1 minor, R1.2 nit, both fixed); `ai-review` success posted on this commit, auto-merge on.
- Cost: ~25% of context; review rounds 1/2 (light lane); regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: the work mix counts only `crates/`, `bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` and `tools/fsio/` changes, while `ops/areas.toml` calls `xtask/` product (`tooling`); no open M0 issue is expected to change a product path, so the budget is not expected to be met in M0 and keeps making the Planner due each day.
- Ignored: none
