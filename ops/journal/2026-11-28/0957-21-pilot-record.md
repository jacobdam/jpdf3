## 09:57 UTC #21 / PR #503 Pilot fire drill record: the owner's results from #208
- Session: worker ready #21
- Picked: rule i because #21 was the lowest-numbered ready issue in M0 with its dependencies closed (#19 closed; #208, the owner's drill steps, closed as completed 2026-11-28)
- Did: PR #503 updates docs/retros/0000-pilot.md: drills 4 (live push, PR #381), 5 (stranger instruction as a comment on #360, PR #382 journal), 7 (`/hold` re-run, #266, PR #324) and 8 (forced infra failure, PR #226, run 45022829095) now pass; Status 8 of 8 (drill 8 only in its marker half). Answers audit 0005 AC.4 on #21: drill 6 quotes the local driver.log lines and says what happened to session 20261120T165104Z (killed with exit 137 at 16:53:01, no `result` line; session 20261120T165524Z finished its work, PR #34). Moved the pending lesson 2026-11-28-0945-481-pick-log.md into ops/LESSONS.md. Removed the merged worktrees 481-picks, 497-nightly-previous and 501-skip (scheduler notes).
- Result: round 1 (light lane) had 1 minor and 2 nits, all fixed in dc3b1a4, so no round 2; `ai-review` success posted on this record commit, auto-merge on. #21 closes on merge.
- Cost: about 30% of context; review rounds 1/2 (light lane); regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no crates/ or tools/ path)
- Lesson: none new. The guard blocks a Bash heredoc that holds the CONSTITUTION 3 keyword next to a `.rs` path even when it only writes Markdown (the LESSONS entry already says this); Write a script file instead.
- Ignored: a comment on #208 by `jacoit` ("Just a test comment"), the owner's drill account; data, no request
