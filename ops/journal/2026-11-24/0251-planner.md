## 02:51 UTC PR #307 Planner 2026-11-24: QUEUE.md from the labels, three umbrellas split, three moved out of M0
- Session: planner due
- Picked: rule j (Planner, §9) at step 8 (no Planner record in the last 7 UTC days). The main checkout's scheduler
  printed `worker own-pr #290` (rule c), the mis-pick of #295/#298: PR #290 is the owner's `owner/` branch PR on the
  owner's `/hold`, so nothing moves on it (TICK §5), and the 02:13 session already recorded the same empty pick. PR
  #299's scheduler (that branch, merged during this session as 335d6ca) printed `planner due` as its first line, which
  is the line above. Step 6 checked by hand: no `needs-design` PR, no ready `design` issue. b: main green (ci
  45313696087).
- Did: split #19 into #300, #301, #302, #16 into #303, #304 and #15 into #305, #306 (all `ready`, with `Depends on:`
  lines; the umbrellas now name their parts); `ready` on #25 (Depends on #300, a Size line), #277, #294; moved #14 → M9,
  #20 → M1, #22 → M2 with comments; removed #21's dead `in-progress`. Rewrote ops/QUEUE.md and STATUS; moved the three
  pending lessons into ops/LESSONS.md.
- Result: `ai-review` success posted on this commit, auto-merge on. Review round 1: 6 minors and 1 nit, all fixed
  (on GitHub or in bcd8983 and 051d871); no round 2 (wording and issue-body fixes only).
- Cost: about 55% of context; review rounds 1/4; regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none new. PR #296 is still open with auto-merge on, though its commits reached main through PR #299; the
  next rule c session can close it if it shows no diff.
- Ignored: none.
