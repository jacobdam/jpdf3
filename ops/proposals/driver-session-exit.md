# Proposal: stop finished sessions, and detect usage limits from the result line only

Status: proposed for owner issue #125, 2026-11-21. Touches `ops/driver/driver.sh` (protected), so
the owner applies it with a signed commit. The unprotected part (a TICK §0 rule) ships in the same
PR as this proposal.

## Problem

Two driver bugs, both in `run_session`:

1. **A finished `claude -p` can stay alive, and `wait "$pid"` blocks until the 3 h watchdog.**
   Session `20261121T171454Z` wrote its final result at 17:50:25 UTC and was still running when
   the owner killed it at 18:04:49 (rc 143).
2. **Any non-zero exit counts as a usage limit.** The check greps the whole stream-json log for
   `rate.?limit` (and similar), and every log has `rate_limit_event` lines with
   `"status":"allowed"` (45 in `20261121T171454Z.jsonl`). The owner's kill above was logged as
   `usage limit: sleeping 1800s`.

## Evidence (all 40 logs in `ops/logs/`, 2026-11-20 08:57 → 2026-11-21 18:05 UTC)

- **The cause of bug 1 is a pending ScheduleWakeup, not the MCP child.** All 39 other sessions
  exited within 1 s of their last log line, with the same `chrome-devtools-mcp` config. The stuck
  session had called ScheduleWakeup four times as a "fallback heartbeat" while its review subagent
  ran in the background; the last call (17:48:17, 1200 s) was still pending when it finished, and
  `claude -p` waits for pending wakeups. Without the kill it would have woken at about 18:09, found
  nothing to do, and exited.
- **It happened before, just shorter.** Five earlier sessions sat idle after their final answer
  until a fallback wakeup fired: 1.8, 19.0, 10.2, 19.7 and 12.0 min (`20261120T190223Z`,
  `20261120T193021Z`, `20261121T001547Z`, `20261121T015408Z`, `20261121T135803Z`). The woken turn
  did new work in two of them (a re-check of CI or the merge); the other three said there was
  nothing left. With `171454Z` that is about 77 min of idle loop time in 40 sessions, plus the 30 min
  false usage-limit sleep.
- **Other quiet gaps after a result are real waits.** In every other gap of more than 2 min after
  a result, the last `background_tasks_changed` event listed a running background agent or shell
  (for example `20261120T223424Z`, `20261121T122417Z`, `20261120T134508Z`), and the session went on
  as soon as it finished. Monitors show up there as `local_bash` tasks.
- **No log has a real usage limit**, so the detector below can't be checked against one. It uses
  the documented shapes: the final `result` line (`is_error`, `api_error_status`, the `result`
  text), the CLI's `rate_limit_event` `status` (`allowed` in every log so far; `rejected` when
  blocked), and the CLI's stderr.

## Proposed change

The ready-to-apply diff is [`driver-session-exit/driver.diff`](driver-session-exit/driver.diff)
(about 90 added lines). In `run_session`:

1. **Own process group.** Start `claude` through
   `perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV'`, so pgid = pid and a stop reaches the MCP
   servers, shells and agents it spawned. macOS has no `setsid` binary; `/usr/bin/perl` ships with
   the OS.
2. **Separate stderr** (`ops/logs/<ts>.stderr`, deleted when empty), so the usage-limit check can
   read the CLI's own messages without the transcript.
3. **A polling loop replaces `wait` plus the watchdog subshell.** Every `POLL` (10 s) seconds:
   - at `SESSION_MAX` (unchanged, 3 h): stop the group, as the watchdog did;
   - when **settled**: the last turn event (`assistant`, `user` or `result`) is a `result`, the
     last `background_tasks_changed` list is empty, and the log hasn't changed for `SETTLE_GRACE`
     (120 s) seconds, stop the group.

   Stopping is TERM to the group, then KILL after 15 s.
4. **The exit code comes from the result line** when the process died from a signal after it had
   settled (our stop or the owner's manual kill): 0, or 1 if `is_error`.
5. **Usage limit = return 42 only when rc ≠ 0 and one of these holds:**
   - the last `rate_limit_event` has `status` `rejected`;
   - the last `result` line has `is_error: true` with `api_error_status` 429, or its `result`
     text matches the old phrase list (plus `hit your limit`);
   - there is no `result` line at all, and the stderr file matches that list.

   Words elsewhere in the transcript no longer count.
6. The driver log line gains `stopped=settled` or `stopped=SESSION_MAX`.

A background task's notification restarts the turn (new `system init` and `assistant` lines), so
a session waiting on an agent or a shell isn't settled. The rule only cuts a session that has
answered and has nothing running: in the history above, that's exactly the wakeup waits and the
stuck session.

## Tests

[`driver-session-exit/test.sh`](driver-session-exit/test.sh) sources a copy of the driver
without its main loop, with `REPO_DIR` pointed at a scratch box. It runs `run_session` against
[`fakebin/claude`](driver-session-exit/fakebin/claude) with `SETTLE_GRACE=3` and `POLL=1`, in
13 scenarios (about 50 s in total):

| scenario | expected |
|---|---|
| normal exit | rc 0 |
| result, then a child keeps the process alive | settled stop after the grace, rc 0, child killed |
| result while a background agent runs, which notifies 6 s later | no stop, rc 0 after the second turn |
| result, a 2 s gap (shorter than the grace), then another turn | no stop, rc 0 |
| error result "usage limit reached" | 42 |
| error result with `api_error_status` 429 | 42 |
| limit words in tool output plus an `allowed` rate-limit event, error result "max turns" | rc 1, **not** 42 |
| no result, stderr names a usage limit | 42 |
| no result, unrelated stderr | rc 1 |
| `rate_limit_event` `rejected` | 42 |
| plain error result | rc 1 |
| no result and no exit (`SESSION_MAX=4`) | stopped at 4 s, rc 143 |
| result, then a child that ignores TERM | KILL after 15 s, rc 0, child gone |

Results on the Mac mini (2026-11-21): the patched driver passes 13/13. The current driver fails
6: hang (stall, child left alive), stubborn child (same), max turns, unrelated stderr, plain error
and `SESSION_MAX` (each returned 42). I also ran the new state, result-code and usage-limit
functions over all 40 real logs: every finished session reads `result 0`, rc 0, no limit,
including `171454Z`.

## Applying it

```sh
git apply ops/proposals/driver-session-exit/driver.diff   # from the repo root
ops/proposals/driver-session-exit/test.sh ops/driver/driver.sh   # expect "failures: 0"
```

Commit it signed, then restart the loop (`touch ops/HALT`, wait for the current session to end,
`rm ops/HALT`, start the tmux session again). A running bash keeps its old copy of the script,
so the fix only takes effect after the restart. After that, this proposal directory can be deleted.

## Risk if not applied

- Any session that leaves a timer or child behind costs up to `SESSION_MAX` (3 h) of loop time.
  The TICK §0 rule ("leave nothing running") makes this rarer but can't rule it out: a child or a
  CLI bug can do the same.
- Every failed session (crash, max turns, owner kill) sleeps 30 min and then doubles up to 2 h as
  if it hit a usage limit, instead of the 5 min failure backoff.

## Risk of applying it

- A session that answered while a timer it still needs is pending is stopped after 2 min. TICK §0
  now forbids such timers, and none of the 40 logged sessions needed one (the work done after a
  wakeup was re-checking what had already been verified).
- If a future CLI stops emitting `background_tasks_changed`, background waits would look settled.
  The settle check needs the log to be quiet for the whole grace period, and a notification
  restarts the turn, so the failure mode would be a stopped session, logged as `stopped=settled`.
  It would show up in `driver.log`.
