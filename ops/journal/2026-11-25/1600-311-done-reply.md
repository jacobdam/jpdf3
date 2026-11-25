## 16:00 UTC #311 the missing "done:" reply to the owner's audience answer
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/311#issuecomment-7139967984
- Picked: rule a because the owner's answer on #311 (who jpdf is for) was applied in PR #332 but had no "done:" reply, so `owner-issues.sh` still listed it
- Did: checked docs/PRODUCT.md §1 on main (ordinary users and people who use it at work in a business, from PR #332, merged as 1351f78); posted the "picked up" and "done:" replies on #311 (`waiting on owner: nothing`); the feed no longer lists the comment. Removed `owner-todo` from #337 (the owner applied the driver proposal in 917d8bc and removed `ops/HALT`). Removed the worktrees of merged PRs #325, #336 and #338 (the scheduler's Notes).
- Result: records PR. main is red from a GitHub outage, not code: the `pages` job of ci run 45649071740 (917d8bc) failed with `Failed to create deployment (status: 502)`, and githubstatus.com reported "degraded experiences across GitHub products" (Issues, Pull Requests, Actions, Pages). `gh run rerun --failed` returned HTTP 500 seven times before it was accepted; attempt 2 failed the same way (status 500). The next session reruns it again (rule b, infra).
- Cost: ~10% of context; review rounds n/a; regraded 0, overturned 0; deferred: none
- Codex: skipped (no review)
- Lesson: during a GitHub outage, `gh issue comment` fails with a bare "GraphQL: Something went wrong" while reads still work → retry in one loop that first checks whether the comment already exists (by its first line), so a write that failed only on the way back isn't posted twice
- Ignored: none
