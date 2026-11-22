#!/usr/bin/env bash
# PROTECTED. Prints ONLY owner-authored instructions (CONSTITUTION 1) as JSON lines:
#   - open issues by the owner;
#   - owner comments not yet handled: conversation comments on issues and PRs (kind "comment"),
#     inline PR review comments (kind "review_comment") and PR review bodies on open PRs
#     (kind "review").
# The agent uses the owner's account too, but the guard forces every agent post to start with
# the tag below, so text that starts with it is agent text and is dropped.
#
# Handled marker (ops/TICK.md, #173): an owner comment is handled once a conversation comment
# by the owner's account whose body starts with "<TAG> done: <comment URL>" exists. Only that
# account can mark a comment handled; a stranger's "done:" reply is ignored. A comment edited
# after its latest done: reply is pending again.
# Comments created before FLOOR (the date ops/TICK.md names) predate the done: replies; they
# keep the old rule (shown while updated in the last 14 days). Comments from FLOOR on stay until
# handled, whatever their age; unhandled ones older than WARN_DAYS carry "stale": true and a
# warning goes to stderr. Limit: reviews are read only on open PRs (there is no repo-wide list),
# so a review on a PR that closes before a session handles it drops out.
set -euo pipefail
OWNER="jacobdam"; R="jacobdam/jpdf3"; TAG="🤖 jpdf-agent"
FLOOR="2026-11-23T00:00:00Z"
WARN_DAYS=14
JQ="${JQ:-jq}"

since14=$(date -u -v-14d +%FT%TZ 2>/dev/null || date -u -d '14 days ago' +%FT%TZ)
warn=$(date -u -v-"$WARN_DAYS"d +%FT%TZ 2>/dev/null || date -u -d "$WARN_DAYS days ago" +%FT%TZ)
# Fetch from the earlier of FLOOR and 14 days ago (ISO 8601 UTC strings sort as dates).
if [[ "$FLOOR" < "$since14" ]]; then since="$FLOOR"; else since="$since14"; fi

# One JSON array per call: --paginate prints one array per page, jq -s merges them. A failed
# call stops the script (set -e, pipefail): a missing page must never hide an instruction.
fetch() { gh api --paginate "$1" | "$JQ" -s 'add // []'; }

issues=$(fetch "repos/$R/issues?state=open&creator=$OWNER&per_page=100")
comments=$(fetch "repos/$R/issues/comments?since=$since&per_page=100")
review_comments=$(fetch "repos/$R/pulls/comments?since=$since&per_page=100")
prs=$(fetch "repos/$R/pulls?state=open&per_page=100")
reviews='[]'
nums=$("$JQ" -r '.[].number | select(type == "number")' <<<"$prs")
for n in $nums; do
  page=$(fetch "repos/$R/pulls/$n/reviews?per_page=100")
  reviews=$(printf '%s\n' "$reviews" "$page" | "$JQ" -s 'add')
done

# The data goes through stdin, not --argjson: months of comments can exceed ARG_MAX.
out=$(printf '%s\n' "$issues" "$comments" "$review_comments" "$reviews" | "$JQ" -s -c \
  --arg owner "$OWNER" --arg tag "$TAG" --arg floor "$FLOOR" --arg since14 "$since14" \
  --arg warn "$warn" --arg repo "$R" '
  .[0] as $issues | .[1] as $comments | .[2] as $rcomments | .[3] as $reviews
  | def owner: (.user.login // "") == $owner;
    def esc: gsub("(?<c>[.^$*+?()\\[\\]{}|\\\\])"; "\\\(.c)");
    # Agent text starts with the tag (the guard enforces it). Owner text that only quotes the
    # tag further down (#173 does) is still an instruction.
    def untagged: ((.body // "") | test("^\\s*" + ($tag | esc)) | not);
  # URLs named by "<tag> done: <url>" replies from the owner account: a comment, review
  # comment or review anchor on this repo. [0-9]+ is greedy and the lookup below is exact, so
  # a reply for …-1234 never handles …-123.
    ("^" + ($tag | esc) + " done: (?<u>https://github\\.com/" + ($repo | esc)
      + "/(issues|pull)/[0-9]+#(issuecomment-|discussion_r|pullrequestreview-)[0-9]+)") as $re
  # $done: URL -> created_at of its latest done: reply.
  | ([$comments[] | select(owner) | .created_at as $at | (.body // "") | capture($re)
      | {key: .u, value: $at}]
      | group_by(.key) | map({key: .[0].key, value: (map(.value) | max)}) | from_entries) as $done
  # Pre-FLOOR: the old 14-day window. From FLOOR on: until a done: reply newer than the last edit.
  | def pending($t; $u):
      if $t < $floor then ($u // $t) >= $since14
      else $done[.html_url // ""] as $d | $d == null or ($u // $t) > $d end;
    def stale($t): $t >= $floor and $t < $warn;
  ( $issues[] | select(owner and untagged)
    | {kind: "issue", number, title, labels: [.labels[].name], is_pr: (.pull_request != null), body} ),
  ( $comments[] | select(owner and untagged) | select(pending(.created_at; .updated_at))
    | {kind: "comment", url: .html_url, issue_url, created_at, stale: stale(.created_at), body} ),
  ( $rcomments[] | select(owner and untagged) | select(pending(.created_at; .updated_at))
    | {kind: "review_comment", url: .html_url, pull_request_url, path, line, created_at,
       stale: stale(.created_at), body} ),
  ( $reviews[] | select(owner and untagged and ((.body // "") != ""))
    | select(pending(.submitted_at; null))
    | {kind: "review", url: .html_url, pull_request_url, state, created_at: .submitted_at,
       stale: stale(.submitted_at), body} )
')
[[ -n "$out" ]] && printf '%s\n' "$out"
"$JQ" -r 'select(.stale == true) | "owner-issues: unhandled owner comment older than '"$WARN_DAYS"' days: \(.url)"' \
  <<<"$out" >&2
exit 0
