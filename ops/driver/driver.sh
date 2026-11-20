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
MIN_FREE_GB="${JPDF_MIN_FREE_GB:-30}"
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
  n=$(gh issue list -R "$GH_REPO" --label halt --author "$OWNER" --state open --json number -q 'length' 2>/dev/null || echo 0)
  [[ "${n:-0}" != "0" ]] && { log "owner halt issue open"; return 0; }
  return 1
}

disk_ok() {
  local free
  free=$(df -g "$REPO_DIR" | awk 'NR==2 {print $4}')
  if (( free < MIN_FREE_GB )); then
    log "low disk (${free} GB < ${MIN_FREE_GB} GB): cleaning"
    find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null
    git worktree prune
    cargo clean -q 2>/dev/null
    free=$(df -g "$REPO_DIR" | awk 'NR==2 {print $4}')
    (( free >= MIN_FREE_GB )) || { log "still low disk (${free} GB)"; return 1; }
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

run_session() {
  local ts out start rc sid
  ts=$(date -u +%Y%m%dT%H%M%SZ); out="ops/logs/$ts.jsonl"; start=$(date +%s)
  log "session $ts starting"
  # Agent sessions get only the project MCP config (an isolated headless Chrome), never the
  # owner's claude.ai connectors (mail, drive, calendar) or the owner's own browser.
  JPDF_SESSION=1 ENABLE_CLAUDEAI_MCP_SERVERS=false claude -p "$(cat ops/TICK.md)" \
      --strict-mcp-config --mcp-config ops/driver/mcp.json \
      --permission-mode auto --output-format stream-json --verbose > "$out" 2>&1 &
  local pid=$!
  ( sleep "$SESSION_MAX"; kill -TERM "$pid" 2>/dev/null ) & local watchdog=$!
  wait "$pid"; rc=$?
  kill "$watchdog" 2>/dev/null; wait "$watchdog" 2>/dev/null
  sid=$(grep -m1 -o '"session_id":"[^"]*"' "$out" | cut -d'"' -f4)
  printf '%s\t%s\t%s\t%ss\n' "$ts" "$rc" "${sid:-?}" "$(( $(date +%s) - start ))" >> ops/logs/index.tsv
  log "session $ts finished rc=$rc session=${sid:-?} (resume: claude --resume ${sid:-?})"
  if grep -qiE 'usage limit|rate.?limit|limit reached|resets at|out of (extra )?usage' "$out" && (( rc != 0 )); then
    return 42
  fi
  return "$rc"
}

backoff=300; limit_wait=1800
while true; do
  if halted; then log "halted; exiting"; notify "loop halted"; exit 0; fi
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
