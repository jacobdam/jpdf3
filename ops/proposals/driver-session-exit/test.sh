#!/usr/bin/env bash
# Tests run_session from a driver copy against a fake `claude` (fakebin/claude), about 3 min.
#   ops/proposals/driver-session-exit/test.sh ops/driver/driver.sh
# Each scenario runs in its own box under $JPDF_TEST_DIR (default target/scratch/driver-test,
# git-ignored); the copy's REPO_DIR points there, so nothing touches the real ops/logs. fakebin
# also shadows gh and git with stubs that fail, in case the main loop ever runs.
# Timing: SETTLE_GRACE=3, POLL=0.25; the driver measures idle and elapsed time below a second, so
# each boundary has a "half a second under the limit survives" and a "stopped right at the limit"
# scenario. Times are in tenths of a second.
set -u
[[ $# -eq 1 && -f "$1" ]] || { echo "usage: $0 <path to driver.sh>" >&2; exit 2; }
here="$(cd "$(dirname "$0")" && pwd)"
drv="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
work="${JPDF_TEST_DIR:-$PWD/target/scratch/driver-test}"
for b in claude gh git; do
  [[ -x "$here/fakebin/$b" ]] || { echo "fakebin/$b is not executable; refusing to run" >&2; exit 2; }
done
[[ "$(PATH="$here/fakebin:$PATH" command -v claude)" == "$here/fakebin/claude" ]] ||
  { echo "fakebin/claude does not shadow claude; refusing to run" >&2; exit 2; }
ds() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 10'; }  # now, in tenths of a second
fails=0
# run <scenario> <expected rc> <min> <max> [pattern...]
#   min/max: run time in tenths of a second. Each pattern must match driver.log; a leading "!"
#   means it must not.
#   env: SMAX (SESSION_MAX), BGMAX (BG_QUIET_MAX), SIG="<signal> <tenths>" (send it to the driver
#   after that delay), AFTER_RETURN=1 (send SIG only after run_session has returned)
run() {
  local sc=$1 want=$2 min=$3 max=$4 box copy rc t0 t dpid p status=ok
  shift 4
  box="$work/box-$sc${AFTER_RETURN:+-after}"; copy="$box/ops/driver/driver.sh"
  rm -rf "$box"; mkdir -p "$box/ops/driver" "$box/ops/logs"
  echo "tick" > "$box/ops/TICK.md"; echo '{}' > "$box/ops/driver/mcp.json"
  # The driver copy without its main loop, so sourcing defines the functions only.
  sed -e '/^backoff=300; limit_wait=1800$/,$d' -e "s|^REPO_DIR=.*|REPO_DIR=\"$box\"|" "$drv" > "$copy"
  if grep -qE '^(while|until)[[:space:]]' "$copy" || ! grep -qxF "REPO_DIR=\"$box\"" "$copy"; then
    echo "driver copy still has a top-level loop or the real REPO_DIR; refusing to run" >&2; exit 2
  fi
  t0=$(ds)
  ( export PATH="$here/fakebin:$PATH" FAKE_SCENARIO=$sc FAKE_DIR=$box
    export JPDF_SETTLE_GRACE=3 JPDF_POLL=0.25 JPDF_SESSION_MAX=${SMAX:-60} JPDF_BG_QUIET_MAX=${BGMAX:-30}
    # shellcheck disable=SC1090
    source "$copy"; run_session; r=$?
    if [[ -n "${AFTER_RETURN:-}" ]]; then touch "$box/returned"; sleep 5; fi
    exit "$r" ) >/dev/null 2>&1 &
  dpid=$!
  if [[ -n "${SIG:-}" ]]; then
    if [[ -n "${AFTER_RETURN:-}" ]]; then
      while [[ ! -e "$box/returned" ]] && kill -0 "$dpid" 2>/dev/null; do sleep 0.1; done
    fi
    sleep "$(perl -e 'print $ARGV[0] / 10' "${SIG#* }")"; kill "-${SIG%% *}" "$dpid"
  fi
  wait "$dpid"; rc=$?; t=$(( $(ds) - t0 ))
  (( rc == want )) || status="FAIL rc=$rc want=$want"
  (( t >= min && t <= max )) || status="$status FAIL time=$t want $min-$max"
  if [[ -f "$box/child.pid" ]] && kill -0 "$(cat "$box/child.pid")" 2>/dev/null; then
    status="$status FAIL child alive"; kill -KILL "$(cat "$box/child.pid")"
  fi
  for p in "$@"; do
    if [[ "$p" == '!'* ]]; then
      grep -qE "${p#!}" "$box/ops/logs/driver.log" && status="$status FAIL log has /${p#!}/"
    else
      grep -qE "$p" "$box/ops/logs/driver.log" || status="$status FAIL log lacks /$p/"
    fi
  done
  if [[ -z "${SIG:-}" || -n "${AFTER_RETURN:-}" ]] && ! grep -q finished "$box/ops/logs/driver.log"; then
    status="$status FAIL no finished line"
  fi
  if [[ -z "${SIG:-}" ]]; then
    if [[ "$sc" == *stderr* ]]; then
      compgen -G "$box/ops/logs/*.stderr" >/dev/null || status="$status FAIL stderr not kept"
    elif compgen -G "$box/ops/logs/*.stderr" >/dev/null; then
      status="$status FAIL empty stderr kept"
    fi
  fi
  [[ "$status" == ok ]] || fails=$((fails + 1))
  printf '%-20s rc=%-3s %4s  %s\n' "$sc${AFTER_RETURN:+ (after)}" "$rc" "$t" "$status"
}
run normal              0   0  20 'finished rc=0 session'
run hang_child          0  30  40 'finished rc=0 stopped=settled'  # stopped right at the grace
run late_turn           0  25  35 'finished rc=0 session' '!stopped='  # quiet grace-0.5 s survives
run bg_wait             0  60  70 'finished rc=0 session' '!stopped='  # a running background task
run slow_turn           0  60  70 'finished rc=0 session' '!stopped='  # a started turn
run escaped_child       0  30  40 'stopped=settled'                # a setsid'd descendant is stopped too
run grandchild          0  30  40 'stopped=settled'                # a setsid'd grandchild too
run orphan              0  30  40 'stopped=settled'                # an in-group orphan (group kill)
run bg_cleared          0  30  40 'finished rc=0 stopped=settled'  # a finished background task
run stubborn            0 180 190 'stopped=settled'                # TERM ignored: KILL after 15 s
run stubborn_escaped    0 180 190 'stopped=settled'                # setsid'd and TERM ignored
run catches_term        0  30  40 'finished rc=0 stopped=settled'  # exits 1 on TERM: rc from the result
run multi_result        1  30  40 'finished rc=1 stopped=settled'  # the last result decides
run self_kill           0  10  20 'finished rc=0 session'          # SIGKILLed after its answer
BGMAX=6 run bg_stuck    0  60  70 'stopped=settled-bg'             # stale background list: BG_QUIET_MAX
BGMAX=3 run bg_near     0  25  35 '!stopped='                      # BG_QUIET_MAX-0.5 s survives
BGMAX=3 run mid_quiet   0  40  50 '!stopped='                      # quiet mid-turn is never settled
SMAX=4 BGMAX=30 run bg_stuck 143 40 50 'stopped=SESSION_MAX'      # cut off after a result: signal rc
SMAX=4 run smax_term  143  40  50 'stopped=SESSION_MAX'           # exits 0 on TERM: still 143
run limit_result       42   0  20
run limit_429          42   0  20
run exit0_limit        42   0  20                                  # error result, exit 0
run limit_then_boom     1   0  20                                  # the last result decides
run boom_then_limit    42   0  20
run words_only          1   0  20                                  # limit words in the transcript only
run limit_stderr       42   0  20
run noise_stderr        1   0  20
run stderr_ratelimit    1   0  20                                  # MCP rateLimit noise is not a limit
run stderr_with_result  1   0  20                                  # a result line overrides stderr
run rl_rejected        42   0  20
run rl_rej_then_ok      1   0  20                                  # the last rate-limit event decides
run ok_rejected         0   0  20                                  # a successful result is not a limit
run error_exit          1   0  20
SMAX=4 run forever    143  40  50 'stopped=SESSION_MAX'           # SESSION_MAX still applies
SIG="HUP 20" run hup_driver  130 20 35 'driver signalled'         # closing tmux stops the session
SIG="TERM 20" run hup_driver 130 20 35 'driver signalled'         # so does a TERM to the driver
# The trap ends with run_session: a later HUP kills the driver normally (129), stop_group never runs.
SIG="HUP 5" AFTER_RETURN=1 run normal 129 5 30 '!driver signalled'
# SIGINT isn't tested: a non-interactive shell starts background jobs with INT ignored, and an
# ignored-at-entry signal can't be trapped, so the harness can't deliver it the way tmux would.
echo "failures: $fails"; exit $(( fails > 0 ))
