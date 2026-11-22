#!/usr/bin/env bash
# Tests the leftover cleanup the proposal adds to driver.sh (#161, CX.11), about 20 s. Functions
# are pulled out of the patched script by their exact text. Only pids of processes this test
# spawned are ever signalled.
#   ops/proposals/codex-review-isolation/test-driver.sh <patched driver.sh>
# Run from the repository root; works in target/scratch/driver-leftovers-test (git-ignored).
set -u
[[ $# -eq 1 && -f "$1" ]] || { echo "usage: $0 <path to patched driver.sh>" >&2; exit 2; }
D="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"; W="$(pwd -P)/target/scratch/driver-leftovers-test"
rm -rf "$W"; mkdir -p "$W"; cd "$W" || exit 1
extract() { awk -v a="$1" 'index($0,a)==1{on=1} on{print} on&&/^}/{exit}' "$D"; }
for f in 'secs_since()' 'descendants()' 'start_time()' 'track_tree()' 'stop_leftovers()'; do
  eval "$(extract "$f")"
done
log() { echo "log: $*"; }
pass=0; failn=0
ok() { if "$@"; then pass=$((pass+1)); else failn=$((failn+1)); echo "FAIL: $*"; fi; }
not() { ! "$@"; }

# Fake CLI: own process group (like the driver's perl setpgrp), starts a helper in a new session
# that writes its pid, waits until the driver has polled once, then exits normally.
perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV' -- /bin/bash -c '
  perl -MPOSIX=setsid -e "setsid() or die; open my \$f, q(>), q(helper.pid); print \$f \$\$; close \$f; exec q(sleep), 300" &
  while [[ ! -e polled ]]; do sleep 0.1; done
  exit 0' &
leader=$!
for _ in $(seq 50); do [[ -s helper.pid ]] && break; sleep 0.1; done
helper=$(cat helper.pid)
ok test -n "$helper"
track="$W/pids"; : > "$track"
track_tree "$leader" "$track"; : > polled
wait "$leader"; echo "leader rc=$?"
ok kill -0 "$helper"                       # the helper survived the CLI's normal exit (the bug)
ok test "$(ps -o ppid= -p "$helper" | tr -d ' ')" = 1   # reparented: not a descendant any more

# An unrelated process of ours recorded with a wrong start time must not be touched.
sleep 300 & bystander=$!
echo "$bystander Mon Jan 1 00:00:00 2001" >> "$track"
stop_leftovers "$leader" "$track"
ok not kill -0 "$helper" 2>/dev/null       # stopped
ok kill -0 "$bystander"                    # untouched (start time differs)
kill "$bystander"; wait "$bystander" 2>/dev/null

# Nothing recorded and the group is gone: returns at once, logs nothing.
: > "$track"; out=$(stop_leftovers "$leader" "$track"); ok test -z "$out"

# A helper that ignores TERM is killed after the grace period.
perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV' -- /bin/bash -c '
  perl -MPOSIX=setsid -e "setsid() or die; \$SIG{TERM} = q(IGNORE); open my \$f, q(>), q(h2.pid); print \$f \$\$; close \$f; sleep 300" &
  while [[ ! -s h2.pid ]]; do sleep 0.1; done; exit 0' &
leader=$!; wait "$leader"
h2=$(cat h2.pid); ok test -n "$h2"
echo "$h2 $(start_time "$h2")" > "$track"
t0=$SECONDS; stop_leftovers "$leader" "$track" >/dev/null; dt=$((SECONDS - t0))
for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$h2" 2>/dev/null || break; sleep 0.1; done   # KILL is async
ok not kill -0 "$h2" 2>/dev/null
ok test "$dt" -ge 14 -a "$dt" -le 20

echo "pass=$pass fail=$failn"; (( failn == 0 ))
