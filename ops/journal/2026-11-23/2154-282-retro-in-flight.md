## 21:54 UTC #282 retro due while retro PR #281 was in CI: no second retro, scheduler bug filed
- Picked: h, `retro due` (first line of `cargo xtask next-session`; Rule: incident, 4 issues labeled blocked since
  9c39200). a: feed comments predate the 2026-11-23 floor, closed owner issues have no pending "done:", #260 has no
  untagged owner comment. b: main's last completed ci run (8275770) green.
- Did: the retro was already done: PR #281 (retro 0002, #279) was open with `ai-review` success, auto-merge on and
  `project-linux` running. Step 7 looks only at record commits on main, so it can't see it. Wrote no second retro;
  filed #282 (bug, `ready`, M0) for step 7 to skip a due audit or retro while its PR is open. Waited for PR #281 to
  merge (judge-verdict green), then wrote this record in the new one-file-per-session form; STATUS in-flight updated.
- Result: PR #281 merged; #282 filed; this records PR.
- Cost: about 10% of context; review rounds n/a; regraded 0, overturned 0; deferred: none.
- Codex: skipped (no review round).
- Lesson: a scheduler pick for a session type whose PR is already open (audit, retro) → check open PRs before
  starting; don't do the work twice (#282 fixes the scheduler).
- Ignored: #260's ordering request (tagged, no untagged owner comment).
