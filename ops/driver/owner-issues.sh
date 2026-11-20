#!/usr/bin/env bash
# PROTECTED. Prints ONLY owner-authored instructions (CONSTITUTION 1) as JSON lines:
# open issues by the owner, plus owner comments from the last 14 days.
# The agent uses the owner's account too, but the guard forces every agent post to start with
# the tag below, so anything containing it is agent text and is dropped.
set -euo pipefail
OWNER="jacobdam"; R="jacobdam/jpdf3"; TAG="🤖 jpdf-agent"
since=$(date -u -v-14d +%FT%TZ 2>/dev/null || date -u -d '14 days ago' +%FT%TZ)
gh api "repos/$R/issues?state=open&creator=$OWNER&per_page=100" --paginate \
  --jq '.[] | select(.user.login=="'"$OWNER"'" and ((.body // "") | contains("'"$TAG"'") | not)) | {kind:"issue", number, title, labels:[.labels[].name], is_pr:(.pull_request!=null), body}'
gh api "repos/$R/issues/comments?since=$since&per_page=100" --paginate \
  --jq '.[] | select(.user.login=="'"$OWNER"'" and ((.body // "") | contains("'"$TAG"'") | not)) | {kind:"comment", issue_url, created_at, body}'
