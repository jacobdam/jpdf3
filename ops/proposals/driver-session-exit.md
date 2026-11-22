# Proposal: stop finished sessions, and detect usage limits from the result line only

**Applied** by the owner in PR #144 (noted by audit 0003); kept as the design record.

Status: proposed for owner issue #125, 2026-11-21, revised after one TICK §4 review round. Touches
`ops/driver/driver.sh` (protected), so the owner applies it with a signed commit. The unprotected
part (a TICK §0 rule) ships in the same PR as this proposal.

## Problem

Two driver bugs, both in `run_session`:

1. **A finished `claude -p` can stay alive, and `wait "$pid"` blocks until the 3 h watchdog.**
   Session `20261121T171454Z` wrote its final result at 17:50:25 UTC and was still running when
   the owner killed it at 18:04:49 (rc 143).
2. **Any non-zero exit counts as a usage limit.** The check greps the whole stream-json log for
   `rate.?limit` (and similar). Every log has `rate_limit_event` lines with `"status":"allowed"`:
   `20261121T171454Z.jsonl` has 15 of them, with 45 regex matches. The owner's kill above was
   logged as `usage limit: sleeping 1800s`.

## Evidence (`ops/logs/`, 40 logs, 2026-11-20 08:57 → 2026-11-21 18:05 UTC)

- **The cause of bug 1 is a pending ScheduleWakeup, not the MCP child.** The other 38 sessions
  with a `finished` line exited within 1 s of their last log line, with the same
  `chrome-devtools-mcp` config. (`20261120T165104Z` has no `finished` line: the driver itself was
  stopped at 16:53. The 40th log is this session's.)
  - The stuck session called ScheduleWakeup four times as a "fallback heartbeat" while its review
    subagent ran in the background.
  - The last call (17:48:17, 1200 s) was still pending when it finished, and `claude -p` waits for
    pending wakeups.
  - Without the kill it would have woken at about 18:09, found nothing to do, and exited.
- **It happened before, just shorter.** Five earlier sessions sat idle after their final answer
  until a fallback wakeup fired:

  | session | idle |
  |---|---|
  | `20261120T190223Z` | 1.8 min |
  | `20261120T193021Z` | 19.0 min |
  | `20261121T001547Z` | 10.2 min |
  | `20261121T015408Z` | 19.7 min |
  | `20261121T135803Z` | 12.0 min |

  The woken turn did new work in two of them (a re-check of CI or the merge); the other three said
  there was nothing left. With `171454Z`, that's about 77 min of idle loop time in 40 sessions,
  plus the 30 min false usage-limit sleep.
- **Other quiet gaps after a result are real waits.** In every other gap of more than 2 min after
  a result, the last `background_tasks_changed` event listed a running background agent or shell
  (for example `20261120T223424Z`, `20261121T122417Z`, `20261120T134508Z`). Each session went on as
  soon as that task finished. Monitors show up there as `local_bash` tasks.
- **No log has a real usage limit**, so the detector below can't be checked against one. It reads:
  - the final `result` line: `is_error`, `api_error_status` and the `result` text, which the CLI
    has used for "usage limit reached" messages;
  - the CLI's `rate_limit_event` `status`. It is `allowed` in every log; that it becomes `rejected`
    when blocked is an **assumption**;
  - the CLI's stderr, only when there is no result line.

**Later finding (session `20261121T180515Z`):** the CLI has its own ceiling for this case. Its
log ends with "Background tasks still running 10m after the last turn (subagent …); stopping them.
Set CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0 to wait indefinitely." So background tasks don't keep
`claude -p` alive for more than about 10 min after the final answer. That doesn't cover a pending
ScheduleWakeup (the stuck session waited for its timer), and `BG_QUIET_MAX` stays as a backstop in
case the ceiling changes or a listed task is stale.

## Proposed change

The ready-to-apply diff is [`driver-session-exit/driver.diff`](driver-session-exit/driver.diff)
(about 130 added lines). All changes are in `run_session` and new helpers:

1. **Own process group, plus descendants.** `claude` starts through
   `perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV'`, so pgid = pid. (macOS has no `setsid`
   binary; `/usr/bin/perl` ships with the OS.)
   - The Bash tool's shells and the MCP server call setsid themselves, so the group alone doesn't
     reach them.
   - `stop_group` therefore also collects every descendant from one `ps -axo pid=,ppid=`
     snapshot, taken before any signal.
   - It sends TERM to the session itself, its group and each descendant, then KILL to whatever is
     left after 15 s (a clock deadline, checked every 0.2 s).
   - It does nothing when the pid is empty or already gone, so a stale or recycled pid is never
     signalled from the trap or after the poll.
2. **Signal pass-through.** In its own group, the session no longer gets the terminal's HUP or INT,
   so closing the tmux session or pressing Ctrl-C would leave it running. A `trap` on HUP, INT and
   TERM stops the session group and exits 130. It is set just before the launch (no gap between
   `&` and the trap) and reset once the session has been reaped. Without the reset, a later HUP
   would run the trap outside `run_session`, where `$ts` is unbound under `set -u`.
3. **Separate stderr** (`ops/logs/<ts>.stderr`, deleted when empty). The usage-limit check can then
   read the CLI's messages without the transcript.
4. **A polling loop replaces `wait` and the watchdog subshell.** It runs every `POLL` (10 s) and
   tracks the last turn event, one of:
   - `assistant`, `user` or `result`;
   - a turn start: `system/init`, or a `command_lifecycle` with `state: started`, which is how a
     wakeup begins.

   On each tick:
   - at `SESSION_MAX` (unchanged, 3 h): stop, as the watchdog did;
   - **settled:** the last turn event is a `result`, the last `background_tasks_changed` list is
     empty, and the log has been quiet for `SETTLE_GRACE` (120 s). Stop.
   - Quiet time and elapsed time are measured below a second (perl `Time::HiRes`). `stat -f %m`
     has 1 s resolution, which put the real grace anywhere in [grace − 1, grace] and made the
     boundary untestable.
   - **settled-bg:** the last turn event is a `result` and the log has been quiet for
     `BG_QUIET_MAX` (90 min), even though background tasks are still listed. This covers a stale
     list, or a shell or dev server left running. Stop. Logged background tasks ran up to
     27.5 min, and a silent background shell (a long `cargo` run) emits nothing while it runs,
     so the cap is set well above that.
   - A missing log file skips the tick. If perl fails, `secs_since` prints a huge number, so a
     broken clock stops the session rather than silently skipping `SESSION_MAX` or the 15 s KILL
     wait (an empty `(( ))` operand would just return false).
5. **The exit code comes from the last result line** (0, or 1 if `is_error`) when the driver
   stopped a settled session, whatever the CLI does on TERM (it may exit 0 or 1 on its own), and
   when a signal killed it after it had answered (the owner's kill, or a crash after the answer,
   which is then logged as the answer's rc). A `SESSION_MAX` stop is always rc 143, even if the
   CLI exits 0 on TERM or an earlier turn had answered: the session was cut off.
6. **A usage limit returns 42** when rc ≠ 0 **or** the final result has `is_error` (the CLI's exit
   code for an error result isn't confirmed), **and** one of these holds:
   - the last `rate_limit_event` has `status` `rejected`;
   - the final result is an error with `api_error_status` 429, or its text matches the old phrase
     list plus `hit your limit`;
   - there is no result line, and stderr matches a narrower list. stderr also carries MCP server
     output, so `rate.?limit` and `resets at` are dropped there.

   Words anywhere else in the transcript no longer count.
7. The driver log line gains `stopped=settled`, `stopped=settled-bg` or `stopped=SESSION_MAX`.

A background task's notification starts a new turn (`system/init` comes before the first output),
so a session waiting on an agent or a shell never settles. The rule only cuts a session that has
answered and has nothing running. In the history above, that's exactly the wakeup waits and the
stuck session.

## Tests

[`driver-session-exit/test.sh`](driver-session-exit/test.sh) sources a copy of the driver
without its main loop, with `REPO_DIR` pointed at a scratch box. It runs `run_session` against
[`fakebin/claude`](driver-session-exit/fakebin/claude) with `SETTLE_GRACE=3` and `POLL=0.25`
(about 3 min in total). Times are measured in tenths of a second. Each limit has a scenario
half a second under it (must survive) and one that must stop right at it, so an off-by-one in
either direction fails.

Safety checks: it refuses to run without a driver path, if the copy still has a top-level loop, if the `REPO_DIR`
substitution failed, or if `fakebin/claude` doesn't shadow the real `claude`. `fakebin` also
shadows `gh` and `git` with stubs that fail.

Settle rule and process handling:

| scenario | expected |
|---|---|
| normal exit | rc 0 |
| result, then a child keeps the process alive | stopped at 3.0-4.0 s (right at the grace), rc 0, child killed |
| result, quiet for grace − 0.5 s, then another turn | not stopped |
| result while a background agent runs, which notifies 6 s later | not stopped, rc 0 |
| the agent finishes, then the new turn's first output takes 5 s (more than the grace) | not stopped |
| result, then a child in its own process group | stopped, child killed |
| result, then a setsid'd grandchild under an intermediate shell | stopped, grandchild killed |
| result, then an in-group process whose parent exited (reparented) | stopped by the group kill |
| a background task came and went before the result | `stopped=settled` at the grace |
| result, then a child that ignores TERM | KILL at 18.0-19.0 s (grace + 15 s), child gone |
| the same, setsid'd | KILL at 18.0-19.0 s, child gone |
| SIGKILLed right after a good result | rc 0 |
| result, then the CLI exits 1 on its own when it gets TERM | rc 0 (from the result) |
| a good result, then a failed turn, then it lingers | rc 1 (the last result decides) |
| result with a background task listed forever (`BG_QUIET_MAX=6`) | `stopped=settled-bg` at 6.0-7.0 s |
| result with a background task listed, quiet `BG_QUIET_MAX` − 0.5 s, then another turn | not stopped |
| mid-turn (no result yet) quiet longer than `BG_QUIET_MAX=3` | not stopped |
| result with a background task listed, `SESSION_MAX=4` | stopped at 4.0-5.0 s, rc 143 (not 0) |
| no result and no exit (`SESSION_MAX=4`) | stopped at 4.0-5.0 s, rc 143 |
| no result, the CLI exits 0 on TERM (`SESSION_MAX=4`) | rc 143 |
| HUP, or TERM, to the driver mid-session | rc 130, the setsid'd child is killed |
| HUP to the driver after `run_session` returned | default death (129), no `driver signalled` |

SIGINT isn't tested: a non-interactive shell starts background jobs with INT ignored, and an
ignored-at-entry signal can't be trapped, so the harness can't deliver it the way tmux would.

Usage-limit detection:

| scenario | expected |
|---|---|
| error result "usage limit reached" | 42 |
| error result with `api_error_status` 429 | 42 |
| limit error result, process exits 0 | 42 |
| limit words in tool output plus an `allowed` event, error result "max turns" | rc 1, **not** 42 |
| a limit error result, then a later turn's different error | rc 1 (the last result decides) |
| a different error, then a later limit error result | 42 |
| no result, stderr names a usage limit | 42 |
| no result, unrelated stderr | rc 1 |
| no result, stderr says `rateLimit` (MCP noise) | rc 1 |
| stderr names a limit, but the result line is a different error | rc 1 |
| `rate_limit_event` `rejected` | 42 |
| `rejected`, then a later `allowed` event, no result | rc 1 (the last event decides) |
| `rejected` event, then a successful result, exit 0 | rc 0 |
| plain error result | rc 1 |

Every scenario also checks that the driver log line carries the right `stopped=` field (or none),
that an empty stderr file is removed, and that a non-empty one is kept.

**Results on the Mac mini (2026-11-21):**
- The patched driver passes 37/37. Stops land 0.1-0.4 s after their limit; the upper bounds
  leave about 0.6 s, because a `>`-for-`>=` mutant stops about 1 s late (4.1 s against 4.0 s).
- Round 1: the then-current driver failed 12 of the first 20 scenarios: the four hang scenarios
  (each stalled until the harness's 60 s `SESSION_MAX`, with the child left alive), exit-0 limit,
  five false usage limits, `SESSION_MAX` and HUP.
- Round 1 mutants, each caught: `>` for `>=` at the grace, a doubled grace, no turn-start marker,
  no descendant kill, no trap, and the wide regex on stderr.
- Round 2 mutants, each caught by the scenario named: grace − 1 (late_turn and the at-grace
  stops), `>` at `BG_QUIET_MAX` (bg_stuck) and at `SESSION_MAX` (forever, bg_stuck), a 16 s KILL
  delay (stubborn), the first result instead of the last (multi_result), the first rate-limit
  event instead of the last (rl_rejected), no trap reset (HUP after return), no result gate on
  settled-bg (mid_quiet), the limit check on rc 0 with a successful result (ok_rejected), no rc
  override for a settled stop (catches_term and the stop scenarios), and the override applied to
  `SESSION_MAX` (bg_stuck with `SESSION_MAX=4`).
- Round 3 mutants, each caught: a sticky background count (bg_cleared), no group kill (orphan),
  direct children only (grandchild), the first result in the limit check (limit_then_boom,
  boom_then_limit), no rc override after a signal death (self_kill), no per-pid KILL
  (stubborn_escaped), no forced 143 on `SESSION_MAX` (smax_term), and `>` at the grace (nine
  stop scenarios).
- One mutant passes: dropping the session's own pid from `stop_group`'s list. It matters only if
  `setpgrp` failed or the signal arrived before it ran, which the harness can't produce.
- I also ran the new state, result-code and usage-limit functions over the 39 finished real logs:
  each reads `result 0`, rc 0, no limit, including `171454Z`.

## Applying it

```sh
git apply ops/proposals/driver-session-exit/driver.diff          # from the repo root
ops/proposals/driver-session-exit/test.sh ops/driver/driver.sh   # expect "failures: 0"
```

Commit it signed, then restart the loop: `touch ops/HALT`, wait for the current session to end,
`rm ops/HALT`, then start the tmux session again. A running bash keeps its old copy of the script,
so the fix only takes effect after the restart. After that, this proposal directory can be deleted.

## Risk if not applied

- Any session that leaves a timer behind costs up to `SESSION_MAX` (3 h) of loop time. The TICK §0
  rule ("leave nothing running") makes this rarer but can't rule it out.
- Every failed session (crash, max turns, owner kill) sleeps 30 min, doubling up to 2 h, as if it
  hit a usage limit. It should get the 5 min failure backoff.

## Risk of applying it

- **A pending timer that the session still needs.** A session that answers while such a timer is
  pending is stopped after 2 min. TICK §0 now forbids those timers, and none of the 40 logged
  sessions needed one: the work done after a wakeup only re-checked what had already been verified.
- **The CLI stops emitting `background_tasks_changed` or `system/init`.** Waits would then look
  settled. The rule needs the whole grace period to pass quietly, so the failure mode is a stopped
  session, logged as `stopped=settled` in `driver.log`.
- **pid reuse.** `stop_group` refuses a session pid that is already gone, but descendant pids are
  signalled for up to 15 s after the snapshot, so a recycled pid could be hit in that window. That
  needs a pid wrap-around within seconds on an idle machine.
- **The usage-limit shape is unverified** (see Evidence). A missed limit falls back to the
  failure backoff (5 min, doubling to 1 h). A false limit costs 30 min to 2 h.
