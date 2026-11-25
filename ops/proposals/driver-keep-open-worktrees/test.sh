#!/usr/bin/env bash
# Tests prune_worktrees from a driver.sh (default: the proposed one next to this file) on real
# git worktrees in a temp repo, with a fake `gh` that prints the open PRs' head branches (or
# fails). Nothing outside the temp dir is touched.
#   bash ops/proposals/driver-keep-open-worktrees/test.sh [driver.sh]
set -u
here="$(cd "$(dirname "$0")" && pwd)"
driver="${1:-$here/driver.sh}"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
funcs="$(sed -n '1,/^backoff=/p' "$driver" | sed '$d' | grep -v -E '^(REPO_DIR=|cd "\$REPO_DIR"|mkdir -p ops)')"
mkdir -p "$tmp/bin"
fail=0
# run GH_MODE OPEN_BRANCHES: builds the repo and worktrees, then runs the prune step.
setup() {
  rm -rf "$tmp/repo"; mkdir -p "$tmp/repo"; cd "$tmp/repo" || exit 1
  git init -q -b main . && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m init
  mkdir -p .claude/worktrees
  for w in "open:t/12-open" "nopr:t/13-nopr" "prefix:t/1" "fresh:t/14-fresh" "suffix:t/12-open-x"; do
    git worktree add -q ".claude/worktrees/${w%%:*}" -b "${w#*:}"
  done
  git worktree add -q --detach .claude/worktrees/detached
  for d in open nopr prefix suffix detached; do touch -d '3 days ago' ".claude/worktrees/$d"; done
}
run() { # GH: "ok" or "fail"; then the open PR branches
  local gh="$1"; shift
  { echo '#!/bin/sh'
    [[ "$gh" == fail ]] && echo 'echo "gh: error" >&2; exit 1'
    for b in "$@"; do echo "echo '$b'"; done; } > "$tmp/bin/gh"; chmod +x "$tmp/bin/gh"
  ( PATH="$tmp/bin:$PATH"; eval "$funcs"; REPO_DIR="$tmp/repo"; GH_REPO=x/y
    log() { echo "$*" >> "$tmp/log"; }
    # The driver runs it from where it starts; the stock driver's inline find is the old rule.
    if declare -F prune_worktrees >/dev/null; then prune_worktrees
    else find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null; git worktree prune; fi )
}
left() { ls "$tmp/repo/.claude/worktrees" | sort | tr '\n' ' '; }
check() { # name, want
  local got; got="$(left)"
  if [[ "$got" == "$2" ]]; then echo "ok   $1"; else echo "FAIL $1: left [$got], want [$2]"; fail=1; fi
}
setup; rm -f "$tmp/log"; run ok t/12-open t/99-other
check "old with an open PR kept; old without one, a prefix (t/1 vs t/12-open), a longer name and a detached HEAD removed; a fresh one kept" "fresh open "
grep -q 'keeping worktree open (open PR on t/12-open)' "$tmp/log" && echo "ok   the kept worktree is logged" || { echo "FAIL no keep line in the log"; fail=1; }
setup; run fail
check "gh fails: nothing removed" "detached fresh nopr open prefix suffix "
setup; run ok
check "no open PRs: every old one removed" "fresh "
setup; run ok t/1 t/13-nopr t/12-open-x
check "exact branch names: t/1, t/13-nopr, t/12-open-x kept; t/12-open (no PR) removed" "fresh nopr prefix suffix "
cd "$tmp/repo" && [[ -z "$(git worktree list --porcelain | grep -c prunable | grep -v '^0$')" ]] && echo "ok   git worktree prune ran (no prunable entries)" || { echo "FAIL prunable worktrees left"; fail=1; }
exit $fail
