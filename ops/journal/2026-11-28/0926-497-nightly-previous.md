## 09:26 UTC #19 → #497 Proposal: the nightly job keeps regress.json between nights and uploads its report
- Session: worker ready #19
- Picked: rule i because #19 (the nightly umbrella) was the lowest-numbered ready issue in M0 with its parts closed; its remaining work was to check the Deliverables against main and close it
- Did: checked #19 against main and the last nightly run (45999791450, on 6bbfd59). Regress tier, bench and fuzz time are met. The diff against the previous night is not: the job passes no --previous and uploads nothing. Issue filing is on main but no nightly has run it yet. Comment: https://github.com/jacobdam/jpdf3/issues/19#issuecomment-7237296750. Filed #497 (part 4, a sub-issue of #19), added it to #19's Depends on line, claimed it. PR #499: ops/proposals/nightly-previous-night.md (restore and save regress.json through the Actions cache, saving only nights that exit 0 or 4, a checksum step, the nightly-report artifact without fuzz inputs, 14 days), specs/tooling/nightly.md wording, and the pending lesson file 2026-11-28-0617-236-memory-adr.md moved into ops/LESSONS.md. Filed #500 (a previous file with an unknown schema counts as no previous; R1.2's code part).
- Result: `ai-review` success posted on this record commit, auto-merge on. Once it merges, the §1 owner-todo check files the owner's part ("Owner: apply ops/proposals/nightly-previous-night.md", `owner-todo`) and adds it to #19's Depends on line; #19 closes after a nightly shows both the regress diff and the issues line.
- Cost: about 45% of context; review rounds 3/4; regraded 0, overturned by the next reviewer 0; deferred: #500
- Codex: skipped (no crates/ or tools/ path)
- Lesson: none
- Ignored: none
