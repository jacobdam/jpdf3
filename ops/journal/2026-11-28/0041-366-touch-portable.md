## 00:41 UTC #366 Driver worktree proposal test: portable dates with touch -t
- Session: worker ready #366
- Picked: rule i because the scheduler picked the lowest-numbered ready M0 issue (work mix below 70%, no ready product issue)
- Did: PR #428. `ops/proposals/driver-keep-open-worktrees/test.sh` sets mtimes through a new `aged` helper (POSIX `TZ=UTC0 touch -t` with a UTC stamp from `date -u -d @t`, falling back to `date -u -r t`), reads each one back with `stat -c %Y` / `stat -f %m` and stops with `FAIL harness` if a date didn't take. Updated the proposal's Tests section and the touch lesson in ops/LESSONS.md, and moved the pending lessons of `2026-11-27-2223-303-admit` into LESSONS. Checked on macOS and in a `debian:trixie` container (colima): the applied driver passes all 10 cases, the `-mtime +0/+1/+3` mutants fail 7/7/8 lines, the pre-proposal driver fails the cases in the proposal's table, and a no-op `touch` shim stops the run. Removed the merged worktree t/planner-2026-11-28 (scheduler note). Started colima for the Linux check and stopped it afterwards.
- Result: `ai-review` success posted on this commit, auto-merge on (strict lane; round 1: 2 nits, both fixed; round 2 delta: clean)
- Cost: about 30% of the context; review rounds 2/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none new (the touch lesson in ops/LESSONS.md now gives the portable form)
- Ignored: none
