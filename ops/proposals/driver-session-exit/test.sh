#!/usr/bin/env bash
# Tests run_session from a driver copy against a fake `claude` (fakebin/claude), about 2 min.
#   ops/proposals/driver-session-exit/test.sh ops/driver/driver.sh
# Each scenario runs in its own box under $JPDF_TEST_DIR (default target/scratch/driver-test,
# git-ignored); the copy's REPO_DIR points there, so nothing touches the real ops/logs. fakebin
# also shadows gh and git with stubs that fail, in case the main loop ever runs.
# Timing: SETTLE_GRACE=3, POLL=1, so a settled stop lands at 3-4 s (mtime has 1 s resolution).
set -u
here="$(cd "$(dirname "$0")" && pwd)"
drv="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
work="${JPDF_TEST_DIR:-$PWD/target/scratch/driver-test}"
for b in claude gh git; do
  [[ -x "$here/fakebin/$b" ]] || { echo "fakebin/$b is not executable; refusing to run" >&2; exit 2; }
done
[[ "$(PATH="$here/fakebin:$PATH" command -v claude)" == "$here/fakebin/claude" ]] ||
  { echo "fakebin/claude does not shadow claude; refusing to run" >&2; exit 2; }
fails=0
# run <scenario> <expected rc> <min s> <max s> [log pattern]
#   env: SMAX (SESSION_MAX), BGMAX (BG_QUIET_MAX), HUP_AFTER (send HUP to the driver after N s)
run() {
  local sc=$1 want=$2 min=$3 max=$4 pat=${5:-} box copy rc t0 t dpid
  box="$work/box-$sc"; copy="$box/ops/driver/driver.sh"
  rm -rf "$box"; mkdir -p "$box/ops/driver" "$box/ops/logs"
  echo "tick" > "$box/ops/TICK.md"; echo '{}' > "$box/ops/driver/mcp.json"
  # The driver copy without its main loop, so sourcing defines the functions only.
  sed -e '/^backoff=300; limit_wait=1800$/,$d' -e "s|^REPO_DIR=.*|REPO_DIR=\"$box\"|" "$drv" > "$copy"
  if grep -q '^while true' "$copy" || ! grep -qxF "REPO_DIR=\"$box\"" "$copy"; then
    echo "driver copy still has its main loop or the real REPO_DIR; refusing to run" >&2; exit 2
  fi
  t0=$(date +%s)
  ( export PATH="$here/fakebin:$PATH" FAKE_SCENARIO=$sc FAKE_DIR=$box
    export JPDF_SETTLE_GRACE=3 JPDF_POLL=1 JPDF_SESSION_MAX=${SMAX:-60} JPDF_BG_QUIET_MAX=${BGMAX:-30}
    # shellcheck disable=SC1090
    source "$copy"; run_session ) >/dev/null 2>&1 &
  dpid=$!
  if [[ -n "${HUP_AFTER:-}" ]]; then sleep "$HUP_AFTER"; kill -HUP "$dpid"; fi
  wait "$dpid"; rc=$?; t=$(( $(date +%s) - t0 ))
  local status=ok
  (( rc == want )) || status="FAIL rc=$rc want=$want"
  (( t >= min && t <= max )) || status="$status FAIL time=${t}s want ${min}-${max}"
  if [[ -f "$box/child.pid" ]] && kill -0 "$(cat "$box/child.pid")" 2>/dev/null; then
    status="$status FAIL child alive"; kill -KILL "$(cat "$box/child.pid")"
  fi
  if [[ -n "$pat" ]] && ! grep -qE "$pat" "$box/ops/logs/driver.log"; then status="$status FAIL log lacks /$pat/"; fi
  if [[ -z "${HUP_AFTER:-}" ]] && ! grep -q finished "$box/ops/logs/driver.log"; then status="$status FAIL no finished line"; fi
  if [[ "$sc" != *stderr* && -z "${HUP_AFTER:-}" ]] && compgen -G "$box/ops/logs/*.stderr" >/dev/null; then status="$status FAIL empty stderr kept"; fi
  [[ "$status" == ok ]] || fails=$((fails + 1))
  printf '%-18s rc=%-3s %3ss  %s\n' "$sc" "$rc" "$t" "$status"
}
run normal             0  0 2  'finished rc=0 session'
run hang_child         0  3 4  'finished rc=0 stopped=settled'  # stopped right at the grace
run late_turn          0  2 3  'finished rc=0 session'          # quiet for grace-1 s survives
run bg_wait            0  6 8  'finished rc=0 session'          # a running background task is not cut off
run slow_turn          0  6 8  'finished rc=0 session'          # a started turn is not cut off
run escaped_child      0  3 5  'stopped=settled'                # a setsid'd descendant is stopped too
run stubborn           0 18 20 'stopped=settled'                # TERM ignored: KILL after 15 s
BGMAX=6 run bg_stuck   0  6 8  'stopped=settled-bg'             # stale background list: BG_QUIET_MAX
run limit_result      42  0 2
run limit_429         42  0 2
run exit0_limit       42  0 2                                   # error result, exit 0
run words_only         1  0 2                                   # limit words in the transcript only
run limit_stderr      42  0 2
run noise_stderr       1  0 2
run stderr_ratelimit   1  0 2                                   # MCP rateLimit noise is not a limit
run stderr_with_result 1  0 2                                   # a result line overrides stderr
run rl_rejected       42  0 2
run error_exit         1  0 2
SMAX=4 run forever   143  4 6  'stopped=SESSION_MAX'           # SESSION_MAX still applies
HUP_AFTER=2 run hup_driver 130 2 4 'driver signalled'          # closing tmux stops the session
echo "failures: $fails"; exit $(( fails > 0 ))
