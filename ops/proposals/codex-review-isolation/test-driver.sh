#!/usr/bin/env bash
# Tests the leftover cleanup the proposal adds to driver.sh (#161, CX.11), about 20 s. Functions
# are pulled out of the patched script by their exact text. Real signals go only to processes this
# test spawned; the cases that name pid 0 or 1 run with `kill` swapped for a recorder.
#   ops/proposals/codex-review-isolation/test-driver.sh <patched driver.sh>
# Run from the repository root; works in target/scratch/driver-leftovers-test (git-ignored).
set -u
[[ $# -eq 1 && -f "$1" ]] || { echo "usage: $0 <path to patched driver.sh>" >&2; exit 2; }
D="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"; W="$(pwd -P)/target/scratch/driver-leftovers-test"
rm -rf "$W"; mkdir -p "$W"; cd "$W" || exit 1
extract() { awk -v a="$1" 'index($0,a)==1{on=1} on{print} on&&/^}/{exit}' "$D"; }
for f in 'secs_since()' 'ps_snap()' 'tree_keys()' 'track_tree()' 'alive_keys()' 'stop_leftovers()'; do
  body="$(extract "$f")"; [[ -n "$body" ]] || { echo "not found in $D: $f" >&2; exit 2; }
  eval "$body"
done
grep -q '^  stop_leftovers "\$pid"$' "$D" || { echo "run_session doesn't call stop_leftovers" >&2; exit 2; }
grep -q '^    track_tree "\$pid"$' "$D" || { echo "the poll loop doesn't call track_tree" >&2; exit 2; }
log() { echo "log: $*"; }
pass=0; failn=0
ok() { if "$@"; then pass=$((pass+1)); else failn=$((failn+1)); echo "FAIL: $*"; fi; }
not() { ! "$@"; }
gone() { local _; for _ in $(seq 20); do kill -0 "$1" 2>/dev/null || return 0; sleep 0.1; done; return 1; }
lstart() { ps -o lstart= -p "$1" | awk '{ print $1, $2, $3, $4, $5 }'; }

# 1. Fake CLI: own process group (like the driver's perl setpgrp), starts a helper in a new
# session, waits until the driver has polled once, then exits normally.
perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV' -- /bin/bash -c '
  perl -MPOSIX=setsid -e "setsid() or die; open my \$f, q(>), q(helper.pid); print \$f \$\$; close \$f; exec q(sleep), 300" &
  while [[ ! -e polled ]]; do sleep 0.1; done
  exit 0' &
leader=$!
for _ in $(seq 50); do [[ -s helper.pid ]] && break; sleep 0.1; done
helper=$(cat helper.pid); ok test -n "$helper"
SESSION_PIDS=""; track_tree "$leader"; : > polled
ok grep -q "^$helper|" <<< "$SESSION_PIDS"            # the poll recorded the helper
ok not grep -q "^$leader|" <<< "$SESSION_PIDS"        # descendants only, not the CLI itself
wait "$leader"
ok kill -0 "$helper"                                   # it survived the CLI's normal exit (the bug)
ok test "$(ps -o ppid= -p "$helper" | tr -d ' ')" = 1  # reparented: no longer a descendant
# A process of ours recorded with a wrong start time must not be touched.
sleep 300 & bystander=$!
SESSION_PIDS+=$'\n'"$bystander|Mon Jan 1 00:00:00 2001"
stop_leftovers "$leader"
ok gone "$helper"
ok kill -0 "$bystander"
ok test -z "$SESSION_PIDS"                             # cleared for the next session
kill "$bystander"; wait "$bystander" 2>/dev/null

# 2. Nothing recorded and the group is gone: returns at once, logs nothing.
SESSION_PIDS=""; out=$(stop_leftovers "$leader"); ok test -z "$out"

# 3. A helper's child started after the last poll is stopped too (descendants of recorded pids).
perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV' -- /bin/bash -c '
  perl -MPOSIX=setsid -e "setsid() or die; open my \$f, q(>), q(h3.pid); print \$f \$\$; close \$f; while (!-e q(go)) { select undef, undef, undef, 0.1 } system q(sh -c \"echo \\\$\\\$ > c3.pid; exec sleep 300\") ; sleep 300" &
  while [[ ! -e polled3 ]]; do sleep 0.1; done; exit 0' &
leader=$!
for _ in $(seq 50); do [[ -s h3.pid ]] && break; sleep 0.1; done
SESSION_PIDS=""; track_tree "$leader"; : > polled3; wait "$leader"
: > go; for _ in $(seq 50); do [[ -s c3.pid ]] && break; sleep 0.1; done
h3=$(cat h3.pid); c3=$(cat c3.pid); ok test -n "$c3"
ok not grep -q "^$c3|" <<< "$SESSION_PIDS"            # never recorded
stop_leftovers "$leader" >/dev/null
ok gone "$c3"; ok gone "$h3"

# 4. A helper that ignores TERM is killed after the grace period.
perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV' -- /bin/bash -c '
  perl -MPOSIX=setsid -e "setsid() or die; \$SIG{TERM} = q(IGNORE); open my \$f, q(>), q(h2.pid); print \$f \$\$; close \$f; sleep 300" &
  while [[ ! -s h2.pid ]]; do sleep 0.1; done; exit 0' &
leader=$!; wait "$leader"
h2=$(cat h2.pid); ok test -n "$h2"
SESSION_PIDS="$h2|$(lstart "$h2")"
t0=$SECONDS; stop_leftovers "$leader" >/dev/null; dt=$((SECONDS - t0))
ok gone "$h2"
ok test "$dt" -ge 14 -a "$dt" -le 20

# 5. Records naming pid 1 (with launchd's real start time), pid 0 and junk signal nothing.
# `kill` is a recorder here, so a wrong result can't reach a real process.
kill() { echo "$*" >> kills.log; [[ "$1" == -0 ]] && return 1; return 0; }
: > kills.log
SESSION_PIDS="1|$(lstart 1)"$'\n'"0|$(lstart 1)"$'\n'"x|y"$'\n'"-1|$(lstart 1)"
stop_leftovers "$leader" >/dev/null
ok not grep -qE 'TERM|KILL' kills.log
unset -f kill

echo "pass=$pass fail=$failn"; (( failn == 0 ))
