#!/usr/bin/env bash
# PROTECTED (CONSTITUTION 12). Runs one fresh Claude Code session per task, forever.
#   ops/driver/driver.sh            unattended loop (run inside tmux; watch it with `cmux tmux attach jpdf`)
#   ops/driver/driver.sh --pilot    exactly one session, then exit (pilot shakedown)
# Stop: `touch ops/HALT`, or open an issue labeled `halt` (owner only).
set -uo pipefail

REPO_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
OWNER="jacobdam"
GH_REPO="jacobdam/jpdf3"
PAUSE="${JPDF_PAUSE:-120}"              # seconds between sessions
SESSION_MAX="${JPDF_SESSION_MAX:-10800}" # hard cap per session (3 h)
SETTLE_GRACE="${JPDF_SETTLE_GRACE:-120}" # idle seconds allowed after a session's final result
POLL="${JPDF_POLL:-10}"                  # seconds between checks of a running session
BG_QUIET_MAX="${JPDF_BG_QUIET_MAX:-5400}" # quiet seconds after a result while background tasks are listed
MIN_FREE_GB="${JPDF_MIN_FREE_GB:-5}"
MODE="loop"; [[ "${1:-}" == "--pilot" || "${1:-}" == "--once" ]] && MODE="once"

cd "$REPO_DIR" || exit 1
mkdir -p ops/logs ops/state
log() { echo "[driver $(date -u +%FT%TZ)] $*" | tee -a ops/logs/driver.log; }
# Desktop notification through cmux when it is running (the guard stops agents calling cmux;
# the driver is not an agent). Silent no-op otherwise.
notify() { command -v cmux >/dev/null 2>&1 && cmux notify --title "jpdf" --body "$*" >/dev/null 2>&1 || true; }

halted() {
  [[ -f ops/HALT ]] && { log "HALT file present"; return 0; }
  local n
  # A gh failure (network, auth, outage) is "unknown", not "no halt": return 2 and start nothing.
  if ! n=$(gh issue list -R "$GH_REPO" --label halt --author "$OWNER" --state open --json number -q 'length' 2>/dev/null); then
    log "halt check failed (gh error); not starting a session"; return 2
  fi
  [[ "${n:-0}" != "0" ]] && { log "owner halt issue open"; return 0; }
  return 1
}

# Free space in whole GB. `df -P -k` is POSIX, so it works on macOS and Linux (`df -g` is
# BSD-only: on Linux it fails, and the empty result read as 0 GB, which cleaned every hour).
free_gb() {
  df -P -k "$REPO_DIR" 2>/dev/null | awk 'NR==2 && $4 ~ /^[0-9]+$/ {print int($4 / 1048576)}'
}

disk_ok() {
  local free
  free=$(free_gb)
  # Unknown is not "low": clean nothing (cleaning deletes worktrees and the build cache) and
  # start nothing.
  [[ -n "$free" ]] || { log "disk check failed (df gave no number); not starting a session"; return 1; }
  if (( free < MIN_FREE_GB )); then
    log "low disk (${free} GB < ${MIN_FREE_GB} GB): cleaning"
    find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null
    git worktree prune
    cargo clean -q 2>/dev/null
    free=$(free_gb)
    [[ -n "$free" ]] && (( free >= MIN_FREE_GB )) || { log "still low disk (${free} GB)"; return 1; }
  fi
  return 0
}

sync_main() {
  if [[ -n "$(git status --porcelain)" ]]; then
    log "main checkout dirty; stashing"
    git stash push -u -m "driver-autostash $(date -u +%FT%TZ)" >/dev/null
  fi
  git fetch -q origin && git checkout -q main && git pull -q --ff-only origin main
  git worktree prune
  mkdir -p "$REPO_DIR/.claude/worktrees"
  find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null || true
}

slow_mode() {  # judge broken > 7 days: 1 session/day (AUTOMATION.md §6.2)
  [[ -f ops/state/judge-alarm-since ]] || return 1
  local since; since=$(cat ops/state/judge-alarm-since)
  (( $(date +%s) - since > 7*86400 ))
}

# Prints "<last> <bg>" for a stream-json session log. <last> is the last turn event: assistant,
# user, result, or start (a turn began: system/init, or a started command_lifecycle such as a
# wakeup), "none" before the first. <bg> is the length of the last background_tasks_changed list
# (running background agents and shells, Monitors included). Non-JSON lines are skipped.
session_state() {
  jq -Rr 'fromjson? | objects
    | if .type == "system" and .subtype == "background_tasks_changed" then "bg \(.tasks // [] | length)"
      elif (.type == "system" and .subtype == "init")
        or (.type == "command_lifecycle" and .state == "started") then "start"
      elif .type == "assistant" or .type == "user" or .type == "result" then .type
      else empty end' "$1" 2>/dev/null |
    awk '$1 == "bg" { bg = $2; next } { last = $1 }
         END { print (last == "" ? "none" : last), (bg == "" ? 0 : bg) }'
}

# Exit status from the session's last result line (0, or 1 if is_error); empty if there is none.
result_rc() {
  jq -Rr 'fromjson? | objects | select(.type == "result") | if .is_error == true then 1 else 0 end' \
    "$1" 2>/dev/null | tail -n 1
}

# A real usage limit: the last result line is an error that names a limit (or carries HTTP 429),
# or the CLI's last rate_limit_event has status "rejected" (assumed shape: every logged event so
# far says "allowed"; overageStatus is ignored); with no result line at all, stderr names one.
# Never grep the whole transcript: every log has rate_limit_event lines, and tool output can quote
# any of these words. stderr also carries MCP server output, so it gets the narrower list.
LIMIT_RE='usage limit|rate.?limit|limit reached|hit your limit|resets at|out of (extra )?usage'
STDERR_LIMIT_RE='usage limit|limit reached|hit your limit|out of (extra )?usage'
usage_limited() {
  local out=$1 err=$2 v
  v=$(jq -Rrn --arg re "$LIMIT_RE" '[inputs | fromjson? | objects] as $e
    | ([$e[] | select(.type == "result")] | last) as $r
    | ([$e[] | select(.type == "rate_limit_event")] | last) as $l
    | if ($l.rate_limit_info.status // "") == "rejected" then "yes"
      elif $r == null then "none"
      elif $r.is_error == true and ($r.api_error_status == 429
             or (($r.result // "") | tostring | test($re; "i"))) then "yes"
      else "no" end' "$out" 2>/dev/null)
  case "$v" in
    yes) return 0 ;;
    none) grep -qiE "$STDERR_LIMIT_RE" "$err" 2>/dev/null ;;
    *) return 1 ;;
  esac
}

# Whole seconds since epoch time $1 (fractional), or since file $1 was last modified; sub-second
# clocks so the grace is exact (stat's 1 s mtime made it anywhere in [grace-1, grace]).
# secs_since prints a huge number if perl fails, so a broken clock stops a session instead of
# silently skipping SESSION_MAX or the 15 s KILL wait (an empty (( )) operand just returns 1).
secs_since() { perl -MTime::HiRes=time -e 'printf "%d\n", time - $ARGV[0]' "$1" 2>/dev/null || echo 999999; }
idle_secs() { perl -MTime::HiRes=time,stat -e '@s = stat $ARGV[0] or exit 1; printf "%d\n", time - $s[9]' "$1"; }

# All live descendants of pid $1, from one ps snapshot (breadth first).
descendants() {
  ps -axo pid=,ppid= | awk -v root="$1" '{ kids[$2] = kids[$2] " " $1 }
    END { n = split(kids[root], q, " ")
          for (i = 1; i <= n; i++) { m = split(kids[q[i]], c, " "); for (j = 1; j <= m; j++) q[++n] = c[j] }
          for (i = 1; i <= n; i++) print q[i] }'
}

# Stop a session: TERM it, its process group and every descendant (the Bash tool's shells and MCP
# servers call setsid, so the group alone misses them), then KILL what is left after 15 s. The
# descendant list is taken before signalling, while the tree is still connected.
stop_group() {
  local pid=$1 pids p alive t
  # Not started, or already gone (reaped: the pid may belong to someone else by now).
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null || return 0
  pids="$pid $(descendants "$pid")"; t=$(perl -MTime::HiRes=time -e 'printf "%.3f\n", time')
  kill -TERM -- "-$pid" 2>/dev/null
  for p in $pids; do kill -TERM "$p" 2>/dev/null; done
  while (( $(secs_since "$t") < 15 )); do
    alive=0
    kill -0 -- "-$pid" 2>/dev/null && alive=1
    for p in $pids; do kill -0 "$p" 2>/dev/null && alive=1; done
    (( alive )) || return 0
    sleep 0.2
  done
  kill -KILL -- "-$pid" 2>/dev/null
  for p in $pids; do kill -KILL "$p" 2>/dev/null; done
}

# Session helpers that outlive the CLI (CX.11): a helper that calls setsid leaves the session's
# process group, and once the CLI exits it is reparented to launchd, so neither the group nor the
# descendant list finds it after a normal exit. track_tree records "pid|start-time" for every
# descendant of the CLI at each poll, in the driver's memory (SESSION_PIDS; a file the session can
# write would let it name any process). stop_leftovers signals, after the CLI is gone, each
# recorded process whose start time still matches (so a reused pid is never hit), plus its current
# descendants, and the session's group while it still has members. Every pass takes one `ps`
# snapshot. A helper started and detached within one poll interval (POLL seconds) before the CLI
# exits is not seen; that gap is the residual risk.
SESSION_PIDS=""
ps_snap() {  # "pid ppid start-time" for every process, start time as one space-separated string
  ps -axo pid=,ppid=,lstart= 2>/dev/null |
    awk '$1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+$/ { print $1, $2, $3, $4, $5, $6, $7 }'
}
# On stdin, a snapshot. Prints "pid|start" for the descendants of the pids in $1 (space-separated),
# and with $2 = self, for those pids too. Never pid 0 or 1, never the driver itself.
tree_keys() {
  awk -v roots="$1" -v self="${2:-}" -v me="$$" '
    { start[$1] = $3 " " $4 " " $5 " " $6 " " $7; kids[$2] = kids[$2] " " $1 }
    END {
      n = 0; r = split(roots, x, " ")
      for (i = 1; i <= r; i++) {
        if (self == "self") q[++n] = x[i]
        else { k = split(kids[x[i]], c, " "); for (j = 1; j <= k; j++) q[++n] = c[j] }
      }
      for (i = 1; i <= n; i++) {   # breadth first; pids 0 and 1 are never entered (0 is its own parent)
        p = q[i]; if (p + 0 <= 1 || p == me || (p in seen) || !(p in start)) continue
        seen[p] = 1; print p "|" start[p]
        k = split(kids[p], c, " "); for (j = 1; j <= k; j++) q[++n] = c[j]
      }
    }'
}
track_tree() {  # track_tree <CLI pid>: add its current descendants to SESSION_PIDS
  SESSION_PIDS=$( { [[ -n "$SESSION_PIDS" ]] && printf '%s\n' "$SESSION_PIDS"; ps_snap | tree_keys "$1"; } | sort -u)
}
# Prints the pids of the "pid|start" keys in $1 that are alive now with the same start time.
alive_keys() {
  # (BSD awk takes no newline in -v, so the keys come through the environment.)
  ps_snap | KEYS="$1" awk 'BEGIN { n = split(ENVIRON["KEYS"], k, "\n"); for (i = 1; i <= n; i++) want[k[i]] = 1 }
    { if (($1 "|" $3 " " $4 " " $5 " " $6 " " $7) in want) print $1 }'
}
stop_leftovers() {  # stop_leftovers <reaped leader pid>: uses SESSION_PIDS, then clears it
  local leader=$1 roots keys grp=0 t alive p
  [[ "$leader" =~ ^[0-9]+$ ]] || leader=""
  # Recorded processes still alive with the same start time, then their current descendants.
  roots=$(alive_keys "$SESSION_PIDS" | tr '\n' ' ')
  keys=""; [[ -n "${roots// /}" ]] && keys=$(ps_snap | tree_keys "$roots" self)
  SESSION_PIDS=""
  # A process group id is not reused while the group has members, so -leader is ours as long as
  # kill -0 finds members; once it doesn't, the group is never signalled again.
  [[ -n "$leader" ]] && kill -0 -- "-$leader" 2>/dev/null && grp=1
  [[ -z "$keys" ]] && (( ! grp )) && return 0
  log "stopping session leftovers: group $leader (members: $grp), pids $(printf '%s\n' "$keys" | cut -d'|' -f1 | tr '\n' ' ')"
  (( grp )) && kill -TERM -- "-$leader" 2>/dev/null
  for p in $(alive_keys "$keys"); do kill -TERM "$p" 2>/dev/null; done
  t=$(perl -MTime::HiRes=time -e 'printf "%.3f\n", time')
  while (( $(secs_since "$t") < 15 )); do
    alive=0
    if (( grp )); then kill -0 -- "-$leader" 2>/dev/null && alive=1 || grp=0; fi
    [[ -n "$(alive_keys "$keys")" ]] && alive=1
    (( alive )) || return 0
    sleep 0.2
  done
  (( grp )) && kill -0 -- "-$leader" 2>/dev/null && kill -KILL -- "-$leader" 2>/dev/null
  for p in $(alive_keys "$keys"); do kill -KILL "$p" 2>/dev/null; done
}

run_session() {
  local ts out err start t0 rc res sid pid="" leader idle state why=""
  ts=$(date -u +%Y%m%dT%H%M%SZ); out="ops/logs/$ts.jsonl"; err="ops/logs/$ts.stderr"; start=$(date +%s)
  t0=$(perl -MTime::HiRes=time -e 'printf "%.3f\n", time')
  log "session $ts starting"
  # Its own group no longer gets the terminal's HUP/INT (closing tmux, Ctrl-C), so pass them on.
  # Set before the launch so no signal falls in between; stop_group ignores an empty pid.
  trap 'log "driver signalled: stopping session $ts"; stop_group "$pid"; stop_leftovers "$pid"; exit 130' HUP INT TERM
  # Agent sessions get only the project MCP config (an isolated headless Chrome), never the
  # owner's claude.ai connectors (mail, drive, calendar) or the owner's own browser.
  # perl puts the session in its own process group (pgid = pid) for stop_group; stderr goes to
  # its own file so usage_limited can read it without the transcript.
  JPDF_SESSION=1 ENABLE_CLAUDEAI_MCP_SERVERS=false \
    perl -e 'setpgrp(0, 0); exec { $ARGV[0] } @ARGV or die "exec $ARGV[0]: $!\n"' -- \
      claude -p "$(cat ops/TICK.md)" \
      --strict-mcp-config --mcp-config ops/driver/mcp.json \
      --permission-mode auto --output-format stream-json --verbose > "$out" 2> "$err" &
  pid=$!
  SESSION_PIDS=""
  # A finished `claude -p` can stay alive (a pending ScheduleWakeup; #125). Stop it once its last
  # turn event is a result, no background task runs, and the log has been quiet for SETTLE_GRACE
  # seconds. A background task's notification starts a new turn (system/init), so waits on agents
  # or shells are not cut short. A result with background tasks still listed is stopped only after
  # BG_QUIET_MAX quiet seconds, and SESSION_MAX caps everything.
  while kill -0 "$pid" 2>/dev/null; do
    sleep "$POLL"
    track_tree "$pid"
    if (( $(secs_since "$t0") >= SESSION_MAX )); then
      why="SESSION_MAX"; log "session $ts reached SESSION_MAX (${SESSION_MAX}s): stopping"
      stop_group "$pid"; break
    fi
    state=$(session_state "$out")
    [[ "$state" == result\ * ]] || continue
    idle=$(idle_secs "$out" 2>/dev/null) || continue
    if [[ "$state" == "result 0" ]] && (( idle >= SETTLE_GRACE )); then
      why="settled"; log "session $ts idle ${idle}s after its final result, no background tasks: stopping"
      stop_group "$pid"; break
    fi
    if (( idle >= BG_QUIET_MAX )); then
      why="settled-bg"; log "session $ts idle ${idle}s after its final result, background tasks listed: stopping"
      stop_group "$pid"; break
    fi
  done
  wait "$pid"; rc=$?
  # Reaped: the pid may be reused from now on, so the signal trap must not use it (stop_group
  # ignores an empty pid). However it ended (normal exit included), stop what it left behind.
  leader=$pid; pid=""
  stop_leftovers "$leader"
  trap - HUP INT TERM
  # Stopped by us after its final result (however the CLI exits on TERM), or killed by the
  # owner's signal after it had answered: report what the session itself reported in its final
  # result. A SESSION_MAX stop keeps the signal's rc: the session was cut off.
  if [[ "$why" == settled* ]] || { [[ -z "$why" ]] && (( rc > 128 )) &&
       [[ "$(session_state "$out")" == result\ * ]]; }; then
    res=$(result_rc "$out"); rc=${res:-$rc}
  fi
  # However the CLI exits on TERM, a SESSION_MAX stop is a failure.
  [[ "$why" == SESSION_MAX ]] && (( rc <= 128 )) && rc=143
  [[ -s "$err" ]] || rm -f "$err"
  sid=$(grep -m1 -o '"session_id":"[^"]*"' "$out" | cut -d'"' -f4)
  printf '%s\t%s\t%s\t%ss\n' "$ts" "$rc" "${sid:-?}" "$(( $(date +%s) - start ))" >> ops/logs/index.tsv
  log "session $ts finished rc=$rc${why:+ stopped=$why} session=${sid:-?} (resume: claude --resume ${sid:-?})"
  # Checked on rc 0 too when the final result is an error: the CLI's exit code for an error
  # result isn't confirmed.
  if { (( rc != 0 )) || [[ "$(result_rc "$out")" == 1 ]]; } && usage_limited "$out" "$err"; then
    return 42
  fi
  return "$rc"
}

backoff=300; limit_wait=1800
while true; do
  halted; h=$?
  if (( h == 0 )); then log "halted; exiting"; notify "loop halted"; exit 0; fi
  if (( h == 2 )); then [[ "$MODE" == "once" ]] && exit 1; sleep 300; continue; fi
  if ! disk_ok; then sleep 3600; continue; fi
  sync_main || { log "git sync failed"; sleep 600; continue; }
  run_session; rc=$?
  [[ "$MODE" == "once" ]] && exit "$rc"
  if (( rc == 42 )); then
    log "usage limit: sleeping ${limit_wait}s"; notify "usage limit: sleeping $((limit_wait/60)) min"; sleep "$limit_wait"
    limit_wait=$(( limit_wait * 2 > 7200 ? 7200 : limit_wait * 2 )); continue
  fi
  limit_wait=1800
  if (( rc != 0 )); then
    log "session failed rc=$rc: backing off ${backoff}s"
    (( backoff >= 1200 )) && notify "sessions keep failing (rc=$rc); see ops/logs/driver.log"
    sleep "$backoff"
    backoff=$(( backoff * 2 > 3600 ? 3600 : backoff * 2 )); continue
  fi
  backoff=300
  if slow_mode; then log "judge alarm > 7 days: slow mode (24 h)"; notify "judge alarm: slow mode"; sleep 86400; else sleep "$PAUSE"; fi
done
