#!/usr/bin/env bash
# Tests run_session from a driver copy against a fake `claude` (fakebin/claude), about 50 s.
#   ops/proposals/driver-session-exit/test.sh ops/driver/driver.sh
# Each scenario runs in its own box under $JPDF_TEST_DIR (default target/scratch/driver-test,
# git-ignored); the copy's REPO_DIR points there, so nothing touches the real ops/logs.
# The unpatched driver fails 6 of the 13 scenarios; the patched one passes all of them.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
drv="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
work="${JPDF_TEST_DIR:-$PWD/target/scratch/driver-test}"
fails=0
run() {  # run <scenario> <expected rc> <min s> <max s>
  local sc=$1 want=$2 min=$3 max=$4 box rc t0 t
  box="$work/box-$sc"; rm -rf "$box"; mkdir -p "$box/ops/driver" "$box/ops/logs"
  echo "tick" > "$box/ops/TICK.md"; echo '{}' > "$box/ops/driver/mcp.json"
  # The driver copy without its main loop, so sourcing defines the functions only.
  sed -e '/^backoff=300; limit_wait=1800$/,$d' -e "s|^REPO_DIR=.*|REPO_DIR=\"$box\"|" "$drv" > "$box/ops/driver/driver.sh"
  t0=$(date +%s)
  ( export PATH="$here/fakebin:$PATH" FAKE_SCENARIO=$sc FAKE_DIR=$box
    export JPDF_SETTLE_GRACE=3 JPDF_POLL=1 JPDF_SESSION_MAX=${SMAX:-60}
    source "$box/ops/driver/driver.sh"; run_session ) >/dev/null 2>&1
  rc=$?; t=$(( $(date +%s) - t0 ))
  local status=ok
  (( rc == want )) || status="FAIL rc=$rc want=$want"
  (( t >= min && t <= max )) || status="$status FAIL time=${t}s want ${min}-${max}"
  if [[ -f "$box/child.pid" ]] && kill -0 "$(cat "$box/child.pid")" 2>/dev/null; then
    status="$status FAIL child alive"; kill -KILL "$(cat "$box/child.pid")"
  fi
  [[ "$status" == ok ]] || fails=$((fails + 1))
  printf '%-14s rc=%-3s %3ss  %s | %s\n' "$sc" "$rc" "$t" "$status" "$(tail -n 1 "$box/ops/logs/driver.log" | cut -c 32-140)"
}
run normal       0  0 2
run hang_child   0  3 8     # settled: stopped after the 3 s grace, rc from the result line
run bg_wait      0  6 9     # a running background task is not cut off
run limit_result 42 0 2
run limit_429    42 0 2
run words_only   1  0 2     # limit words in the transcript are not a usage limit
run limit_stderr 42 0 2
run noise_stderr 1  0 2
run rl_rejected  42 0 2
run error_exit   1  0 2
run late_turn    0  2 4     # a gap shorter than the grace is not cut off
SMAX=4 run forever 143 4 7  # SESSION_MAX still applies
run stubborn     0  17 24   # TERM ignored: KILL after 15 s
echo "failures: $fails"; exit $(( fails > 0 ))
