## 16:20 UTC main-red run 45649071740: Pages deploy 500, rerun green
- Session: worker main-red run 45649071740
- Picked: rule b because main's `ci` run for 917d8bc (PR #339) failed only in `pages`: deploy-pages got HTTP 500 ("Failed to create deployment", a Pages outage), after `upload-pages-artifact` succeeded; `judge-verdict` and every other job passed.
- Did: `gh run rerun --failed 45649071740`; attempt 2 passed (pages 42 s, the run is `success`). No code change. #337 is closed with no `owner-todo` (the owner applied the proposal in #339; `ops/HALT` is gone), so the §1 check had nothing to change. STATUS: dropped the halt line.
- Result: main green; records PR only.
- Cost: ~5% of context; review rounds n/a; regraded 0, overturned 0; deferred: none
- Codex: skipped (no review)
- Lesson: LESSONS says not to rerun a pages-only failure once the upload succeeded, because the next push redeploys. Here a rerun of a 500 from deploy-pages worked: attempt 2 uploaded `github-pages-3` and deployed. When main's head has no newer push, the rerun is the only way to get main green.
- Ignored: none
