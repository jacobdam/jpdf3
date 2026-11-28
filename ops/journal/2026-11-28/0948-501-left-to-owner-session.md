## 09:48 UTC PR #501 left to the owner's interactive session
- Session: worker own-pr #501
- Picked: rule c because PR #501 (#481) had no `ai-review` success on its head 48762e0. The PR was not worked: issue #481 carries `claimed by session 2026-11-28T09:45Z (owner's interactive session, working in parallel with the loop to speed up M0)` (09:33 UTC), and that session posted the PR's round 1 triage at 09:47:57 UTC, 20 seconds before this session read it. Its next steps (records commit, `ai-review`, auto-merge) are in progress there, and a second session on the same branch would race its pushes and posts.
- Did: removed the merged worktree `t/493-done-reply` (PR #496 merged; scheduler note). Nothing posted on #501 or #481.
- Result: records PR only (this file); waiting on owner: nothing.
- Cost: about 10% of the context; review rounds n/a; regraded 0, overturned 0; deferred: none
- Codex: skipped (no review)
- Lesson: none. The scheduler can't see an interactive session's live claim on a PR that lacks `ai-review`, so it hands the same PR to the loop; a tagged `claimed by session` comment from minutes earlier, plus a fresh triage, is the sign to stand aside.
- Ignored: none
