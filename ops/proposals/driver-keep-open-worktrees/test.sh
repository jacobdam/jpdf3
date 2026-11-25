#!/usr/bin/env bash
# Tests prune_worktrees from a driver.sh (default: the proposed one next to this file) on real
# git worktrees in a temp repo, with a fake `gh` (needs jq) that acts like `gh pr list --json F -q Q`
# on a list of PRs: it keeps only the fields F, applies Q, fails unless asked for `--state open`
# (or fails on purpose). Nothing outside the temp dir is touched.
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
  for w in "open:t/12-open" "nopr:t/13-nopr" "prefix:t/1" "fresh:t/14-fresh" "suffix:t/12-open-x" "edge:t/15-edge"; do
    git worktree add -q ".claude/worktrees/${w%%:*}" -b "${w#*:}"
  done
  git worktree add -q --detach .claude/worktrees/detached
  for d in open nopr prefix suffix detached; do touch -d '3 days ago' ".claude/worktrees/$d"; done
  # Just inside the age limit: `find -mtime +2` counts whole days, so 2 days 23 hours is not old.
  touch -d "@$(( $(date +%s) - 2*86400 - 23*3600 ))" .claude/worktrees/edge
}
run() { # GH: "ok" or "fail"; then the open PRs' head branches, "fork:<b>" for a fork's PR
  local gh="$1" b json="" sep=""; shift
  for b in "$@"; do
    if [[ "$b" == fork:* ]]; then json+="$sep{\"headRefName\":\"${b#fork:}\",\"isCrossRepository\":true}"
    else json+="$sep{\"headRefName\":\"$b\",\"isCrossRepository\":false}"; fi; sep=","
  done
  echo "[$json]" > "$tmp/prs.json"
  { echo '#!/usr/bin/env bash'
    [[ "$gh" == fail ]] && echo 'echo "gh: error" >&2; exit 1'
    cat <<EOS
state="" fields="" q=""
while (( \$# )); do
  case "\$1" in --state) state="\$2"; shift ;; --json) fields="\$2"; shift ;; -q|--jq) q="\$2"; shift ;; esac
  shift
done
[[ "\$state" == open && -n "\$fields" ]] || { echo "fake gh: want --state open and --json" >&2; exit 1; }
jq --arg f "\$fields" '[.[] | with_entries(select(.key as \$k | \$f | split(",") | index(\$k)))]' "$tmp/prs.json" |
  if [[ -n "\$q" ]]; then jq -r "\$q"; else cat; fi
EOS
  } > "$tmp/bin/gh"; chmod +x "$tmp/bin/gh"
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
check "old with an open PR kept; old without one, a prefix (t/1 vs t/12-open), a longer name and a detached HEAD removed; a fresh one and one just under the age limit kept" "edge fresh open "
grep -q 'keeping worktree open (open PR on t/12-open)' "$tmp/log" && echo "ok   the kept worktree is logged" || { echo "FAIL no keep line in the log"; fail=1; }
setup; run fail
check "gh fails: nothing removed" "detached edge fresh nopr open prefix suffix "
setup; run ok
check "no open PRs: every old one removed" "edge fresh "
setup; run ok t/1 t/13-nopr t/12-open-x
check "exact branch names: t/1, t/13-nopr, t/12-open-x kept; t/12-open (no PR) removed" "edge fresh nopr prefix suffix "
setup; run ok t/12-open fork:t/13-nopr
check "a fork's PR with an agent branch name keeps nothing: t/13-nopr removed" "edge fresh open "
cd "$tmp/repo" && [[ -z "$(git worktree list --porcelain | grep -c prunable | grep -v '^0$')" ]] && echo "ok   git worktree prune ran (no prunable entries)" || { echo "FAIL prunable worktrees left"; fail=1; }
exit $fail
