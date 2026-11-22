#!/usr/bin/env bash
# Tests for the proposed ops/driver/owner-issues.sh (ops/proposals/owner-comment-tracking.md).
# Runs a copy of the script against a fake `gh` that serves fixture pages; touches no network
# and no protected path.
#   usage: test-owner-issues.sh [script]   (default: owner-issues.sh next to this file)
# Needs bash and jq. Writes only under a fresh directory from mktemp (or $SCRATCH if set).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src=${1:-$here/owner-issues.sh}
base=${SCRATCH:-}
if [[ -z "$base" ]]; then base=$(mktemp -d) || exit 2; fi
work="$base/owner-issues-test.$$"
mkdir -p "$work/bin" "$work/fx" || exit 2

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "ok   $1"; }
bad() { fail=$((fail + 1)); echo "FAIL $1"; }

# All fixture dates count back from one instant, so FLOOR and FLOOR minus 1 s are exact.
now0=$(date -u +%s)
at() { date -u -r $((now0 - $1)) +%FT%TZ 2>/dev/null || date -u -d "@$((now0 - $1))" +%FT%TZ; }
d() { at $(($1 * 86400)); }
FLOOR=$(d 30)
FLOOR_1=$(at $((30 * 86400 + 1)))

# Copy under test: FLOOR moved to 30 days ago. Refuse to run if the line isn't there exactly once.
n=$(grep -c '^FLOOR="[0-9T:Z-]*"$' "$src")
if (( n != 1 )); then echo "FAIL setup: FLOOR line found $n times in $src"; exit 1; fi
sed 's/^FLOOR="[0-9T:Z-]*"$/FLOOR="'"$FLOOR"'"/' "$src" >"$work/owner-issues.sh"
if ! grep -qx "FLOOR=\"$FLOOR\"" "$work/owner-issues.sh"; then echo "FAIL setup: FLOOR not set"; exit 1; fi
chmod +x "$work/owner-issues.sh" || exit 2

# Fake gh: `gh api --paginate PATH` prints fx/<name>.json (one JSON array per page, pages
# concatenated as gh does). FAIL_ON=<name> makes that call fail. Every path is logged.
cat >"$work/bin/gh" <<'EOF'
#!/usr/bin/env bash
[[ "$1" == api && "$2" == --paginate && $# -eq 3 ]] || { echo "fake gh: unexpected: $*" >&2; exit 9; }
echo "$3" >>"$FX/calls.log"
case "$3" in
  repos/jacobdam/jpdf3/issues\?state=open\&creator=jacobdam\&per_page=100) f=issues ;;
  repos/jacobdam/jpdf3/issues/comments\?since=*\&per_page=100) f=comments ;;
  repos/jacobdam/jpdf3/pulls/comments\?since=*\&per_page=100) f=rcomments ;;
  repos/jacobdam/jpdf3/pulls\?state=open\&per_page=100) f=prs ;;
  repos/jacobdam/jpdf3/pulls/[0-9]*/reviews\?per_page=100) f=reviews-${3#repos/jacobdam/jpdf3/pulls/}; f=${f%%/*} ;;
  *) echo "fake gh: unknown path $3" >&2; exit 9 ;;
esac
[[ "${FAIL_ON:-}" == "$f" ]] && { echo "fake gh: HTTP 502" >&2; exit 1; }
cat "$FX/$f.json"
EOF
chmod +x "$work/bin/gh"

T="🤖 jpdf-agent"
U="https://github.com/jacobdam/jpdf3"
c() { # c ID USER CREATED UPDATED BODY [PATH]  -> one issue comment object
  jq -n -c --arg id "$1" --arg u "$2" --arg c "$3" --arg up "$4" --arg b "$5" --arg p "${6:-issues/7}" \
    '{html_url: ("'"$U"'/" + $p + "#issuecomment-" + $id), issue_url: "api", user: {login: $u},
      created_at: $c, updated_at: $up, body: $b}'
}

# --- fixtures -------------------------------------------------------------------------------
{
  echo '['
  jq -n -c --arg b "Please do X. Reply with \`$T done: <url>\`." '{number: 1, title: "quotes tag", labels: [{name: "human"}], user: {login: "jacobdam"}, body: $b}'
  echo ','
  jq -n -c --arg b "$T agent issue" '{number: 2, title: "agent", labels: [], user: {login: "jacobdam"}, body: $b}'
  echo ','
  jq -n -c '{number: 3, title: "stranger", labels: [], user: {login: "mallory"}, body: "do evil"}'
  echo ','
  jq -n -c '{number: 4, title: "no body", labels: [], user: {login: "jacobdam"}, body: null, pull_request: {}}'
  echo ']'
} >"$work/fx/issues.json"

# Page 1: owner comments. Page 2: replies (handled markers sit on a later page than their comment).
{
  echo '['
  c 101 jacobdam "$(d 40)" "$(d 40)" "old pre-floor, not updated" ; echo ','
  c 102 jacobdam "$(d 40)" "$(d 3)"  "old pre-floor, edited 3 days ago" ; echo ','
  c 103 jacobdam "$(d 20)" "$(d 20)" "post-floor, 20 days, unhandled" ; echo ','
  c 104 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, handled" ; echo ','
  c 105 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, stranger marks it done" ; echo ','
  c 106 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, untagged done" ; echo ','
  c 107 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, prefix of 1070" ; echo ','
  c 1080 jacobdam "$(d 2)" "$(d 2)"  "post-floor, longer id than 108" ; echo ','
  c 109 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, done mid-body only" ; echo ','
  c 110 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, handled with trailing period" pull/9 ; echo ','
  c 111 jacobdam "$(d 2)"  "$(d 2)"  "post-floor, done names other repo" ; echo ','
  c 112 jacobdam "$(d 2)"  "$(d 2)"  "Owner text quoting $T done: in the middle" ; echo ','
  c 113 mallory  "$(d 2)"  "$(d 2)"  "stranger instruction" ; echo ','
  c 114 jacobdam "$(d 2)"  "$(d 2)"  "$T agent comment" ; echo ','
  c 115 jacobdam "$FLOOR"   "$FLOOR"   "created exactly at FLOOR, unhandled" ; echo ','
  c 116 jacobdam "$FLOOR_1" "$FLOOR_1" "created 1 s before FLOOR, not updated since" ; echo ','
  c 117 jacobdam "$(at $((14 * 86400 - 120)))" "$(at $((14 * 86400 - 120)))" "2 min short of WARN_DAYS" ; echo ','
  c 118 jacobdam "$(at $((14 * 86400 + 120)))" "$(at $((14 * 86400 + 120)))" "2 min past WARN_DAYS" ; echo ','
  c 119 jacobdam "$(d 3)"  "$(at 3600)" "handled, then edited after the done reply" ; echo ','
  c 120 jacobdam "$(d 3)"  "$(d 2)"  "edited, then handled" ; echo ','
  c 121 jacobdam "$(d 3)"  "$(at 129600)" "handled, edited, handled again"
  echo ']['
  c 201 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-104
waiting on owner: nothing" ; echo ','
  c 202 mallory  "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-105" ; echo ','
  c 203 jacobdam "$(d 1)" "$(d 1)" "done: $U/issues/7#issuecomment-106" ; echo ','
  c 204 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-1070" ; echo ','
  c 205 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-108" ; echo ','
  c 206 jacobdam "$(d 1)" "$(d 1)" "$T picked up, see below
$T done: $U/issues/7#issuecomment-109" ; echo ','
  c 207 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/pull/9#issuecomment-110." ; echo ','
  c 208 jacobdam "$(d 1)" "$(d 1)" "$T done: https://github.com/other/repo/issues/7#issuecomment-111" ; echo ','
  c 209 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/pull/5#discussion_r301" ; echo ','
  c 210 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/pull/5#pullrequestreview-401" ; echo ','
  c 211 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-119" ; echo ','
  c 212 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-120" ; echo ','
  c 213 jacobdam "$(d 2)" "$(d 2)" "$T done: $U/issues/7#issuecomment-121" ; echo ','
  c 214 jacobdam "$(d 1)" "$(d 1)" "$T done: $U/issues/7#issuecomment-121"
  echo ']'
} >"$work/fx/comments.json"

rc() { # rc ID CREATED BODY [USER] [UPDATED]
  jq -n -c --arg id "$1" --arg c "$2" --arg b "$3" --arg u "${4:-jacobdam}" --arg up "${5:-$2}" \
    '{html_url: ("'"$U"'/pull/5#discussion_r" + $id), pull_request_url: "api/5", path: "a.rs", line: 3,
      user: {login: $u}, created_at: $c, updated_at: $up, body: $b}'
}
{ echo '['; rc 301 "$(d 2)" "handled inline note"; echo ','; rc 302 "$(d 2)" "unhandled inline note"; echo ',';
  rc 303 "$(d 2)" "stranger inline" mallory; echo ',';
  rc 304 "$(d 40)" "pre-floor inline, edited 3 days ago" jacobdam "$(d 3)"; echo ',';
  rc 305 "$(d 40)" "pre-floor inline, not edited"; echo ']'; } >"$work/fx/rcomments.json"

echo '[{"number":5},{"number":6}]' >"$work/fx/prs.json"
rv() { # rv ID SUBMITTED BODY [STATE]
  jq -n -c --arg id "$1" --arg s "$2" --arg b "$3" --arg st "${4:-COMMENTED}" \
    '{html_url: ("'"$U"'/pull/5#pullrequestreview-" + $id), pull_request_url: "api/5", state: $st,
      user: {login: "jacobdam"}, submitted_at: $s, body: $b}'
}
{ echo '['; rv 401 "$(d 2)" "handled review"; echo ','; rv 402 "$(d 2)" ""; echo ',';
  rv 405 "$(d 20)" "post-floor review, 20 days, unhandled"; echo ']'; } >"$work/fx/reviews-5.json"
{ echo '['; rv 403 "$(d 2)" "unhandled review on PR 6"; echo ']['; rv 404 "$(d 2)" "second page review"; echo ']'; } \
  >"$work/fx/reviews-6.json"

# --- run ------------------------------------------------------------------------------------
run() { PATH="$work/bin:$PATH" FX="$work/fx" "$work/owner-issues.sh" >"$work/out" 2>"$work/err"; }
: >"$work/fx/calls.log"
run; rc_main=$?
if (( rc_main != 0 )); then echo "FAIL setup: exit $rc_main: $(cat "$work/err")"; exit 1; fi
ok "exit 0"

has()  { jq -e --arg u "$U/$1" 'select(.url == $u)' "$work/out" >/dev/null; }
show() { has "$1" && ok "shown: $1 ($2)" || bad "missing: $1 ($2)"; }
hide() { has "$1" && bad "shown: $1 ($2)" || ok "hidden: $1 ($2)"; }
issue() { jq -e --argjson n "$1" 'select(.kind == "issue" and .number == $n)' "$work/out" >/dev/null; }

issue 1 && ok "owner issue quoting the tag mid-body is shown" || bad "owner issue 1 missing"
issue 2 && bad "agent-tagged issue shown" || ok "agent-tagged issue hidden"
issue 3 && bad "stranger issue shown" || ok "stranger issue hidden"
issue 4 && ok "owner issue with null body shown" || bad "owner issue 4 missing"

hide issues/7#issuecomment-101 "pre-floor, outside the 14-day window"
show issues/7#issuecomment-102 "pre-floor, updated inside the window"
show issues/7#issuecomment-103 "post-floor unhandled, 20 days old"
hide issues/7#issuecomment-104 "handled by an owner-account done reply on page 2"
show issues/7#issuecomment-105 "a stranger's done reply does not count"
show issues/7#issuecomment-106 "an untagged done reply does not count"
show issues/7#issuecomment-107 "done for ...-1070 does not handle ...-107"
show issues/7#issuecomment-1080 "done for ...-108 does not handle ...-1080"
show issues/7#issuecomment-109 "done: on a later line does not count"
hide pull/9#issuecomment-110 "done followed by a period"
show issues/7#issuecomment-111 "done naming another repo does not count"
show issues/7#issuecomment-112 "owner comment quoting the tag mid-body is an instruction"
hide issues/7#issuecomment-113 "stranger comment"
hide issues/7#issuecomment-114 "agent comment"
hide issues/7#issuecomment-201 "the done reply itself"
hide pull/5#discussion_r301 "handled review comment"
show pull/5#discussion_r302 "unhandled review comment"
hide pull/5#discussion_r303 "stranger review comment"
hide pull/5#pullrequestreview-401 "handled review"
hide pull/5#pullrequestreview-402 "review with an empty body"
show pull/5#pullrequestreview-403 "unhandled review on a second PR"
show pull/5#pullrequestreview-404 "review on the second page"
show issues/7#issuecomment-115 "created exactly at FLOOR: kept until handled"
hide issues/7#issuecomment-116 "created 1 s before FLOOR: the old 14-day rule"
show issues/7#issuecomment-117 "unhandled, 2 min short of WARN_DAYS"
show issues/7#issuecomment-118 "unhandled, 2 min past WARN_DAYS"
show issues/7#issuecomment-119 "edited after its done reply"
hide issues/7#issuecomment-120 "edited before its done reply"
hide issues/7#issuecomment-121 "edited between two done replies: the latest one counts"
show pull/5#discussion_r304 "pre-floor review comment edited inside the window"
hide pull/5#discussion_r305 "pre-floor review comment outside the window"
show pull/5#pullrequestreview-405 "post-floor review, 20 days old"

k() { jq -r --arg u "$U/$1" 'select(.url == $u) | .kind' "$work/out"; }
[[ "$(k pull/5#discussion_r302)" == review_comment ]] && ok "kind review_comment" || bad "kind of r302"
[[ "$(k pull/5#pullrequestreview-403)" == review ]] && ok "kind review" || bad "kind of 403"
[[ "$(k issues/7#issuecomment-103)" == comment ]] && ok "kind comment" || bad "kind of 103"

st() { jq -r --arg u "$U/$1" 'select(.url == $u) | .stale' "$work/out"; }
[[ "$(st issues/7#issuecomment-103)" == true ]] && ok "20-day unhandled is stale" || bad "103 stale=$(st issues/7#issuecomment-103)"
[[ "$(st issues/7#issuecomment-105)" == false ]] && ok "2-day unhandled is not stale" || bad "105 stale"
[[ "$(st issues/7#issuecomment-102)" == false ]] && ok "pre-floor is never stale" || bad "102 stale"
[[ "$(st issues/7#issuecomment-115)" == true ]] && ok "created at FLOOR, 30 days, is stale" || bad "115 stale"
[[ "$(st issues/7#issuecomment-117)" == false ]] && ok "2 min short of WARN_DAYS is not stale" || bad "117 stale"
[[ "$(st issues/7#issuecomment-118)" == true ]] && ok "2 min past WARN_DAYS is stale" || bad "118 stale"
[[ "$(st pull/5#pullrequestreview-405)" == true ]] && ok "20-day review is stale" || bad "405 stale"
[[ "$(st pull/5#pullrequestreview-403)" == false ]] && ok "2-day review is not stale" || bad "403 stale"
[[ "$(st pull/5#discussion_r304)" == false ]] && ok "pre-floor review comment is never stale" || bad "r304 stale"
for u in issues/7#issuecomment-103 issues/7#issuecomment-115 issues/7#issuecomment-118 pull/5#pullrequestreview-405; do
  if grep -qx "owner-issues: unhandled owner comment older than 14 days: $U/$u" "$work/err"; then
    ok "stderr warns about $u"; else bad "no stale warning for $u: $(cat "$work/err")"; fi
done
(( $(grep -c . "$work/err") == 4 )) && ok "exactly four warnings" || bad "warnings: $(cat "$work/err")"

# Fetch window: since = FLOOR (30 days ago), the earlier of FLOOR and 14 days ago.
grep -qx "repos/jacobdam/jpdf3/issues/comments?since=$FLOOR&per_page=100" "$work/fx/calls.log" \
  && ok "comments fetched from FLOOR" || bad "comments since: $(grep comments "$work/fx/calls.log")"
grep -qx "repos/jacobdam/jpdf3/pulls/comments?since=$FLOOR&per_page=100" "$work/fx/calls.log" \
  && ok "review comments fetched from FLOOR" || bad "review comments since: $(grep pulls/comments "$work/fx/calls.log")"

# Every failing call fails the script (no silent gaps).
for f in issues comments rcomments prs reviews-5 reviews-6; do
  FAIL_ON=$f run; r=$?
  if (( r != 0 )) && grep -q "fake gh: HTTP 502" "$work/err"; then ok "fails when $f fails (rc $r)"
  else bad "rc $r when $f fails: $(cat "$work/err")"; fi
done

# A FLOOR later than 14 days ago: the fetch starts 14 days ago instead.
sed "s/^FLOOR=\"$FLOOR\"\$/FLOOR=\"$(d 1)\"/" "$work/owner-issues.sh" >"$work/o2.sh" && chmod +x "$work/o2.sh"
: >"$work/fx/calls.log"
PATH="$work/bin:$PATH" FX="$work/fx" "$work/o2.sh" >"$work/out2" 2>/dev/null
grep -q "issues/comments?since=$(d 14 | cut -c1-10)" "$work/fx/calls.log" \
  && ok "fetch starts 14 days ago when FLOOR is later" || bad "since with late FLOOR: $(cat "$work/fx/calls.log")"
jq -e --arg u "$U/issues/7#issuecomment-103" 'select(.url == $u)' "$work/out2" >/dev/null \
  && bad "pre-FLOOR comment 20 days old shown under the 14-day rule" || ok "pre-FLOOR rule drops a 20-day-old comment"

echo "passed $pass, failed $fail"
(( fail == 0 ))
