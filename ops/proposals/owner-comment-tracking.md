# Proposal: track owner comments until an agent marks them done (#173)

Status: proposed by a session for owner issue #173, 2026-11-22. Touches the protected
`ops/driver/owner-issues.sh`, so the owner applies it with a signed commit. The agent-side rule
(the "picked up" and "done:" replies) is in `ops/TICK.md` §0 and §2 a, in the same PR.

Files:
- `owner-comment-tracking/owner-issues.sh`: the whole proposed script (a drop-in replacement).
- `owner-comment-tracking/owner-issues.diff`: the same change as a diff against main.
- `owner-comment-tracking/test-owner-issues.sh`: 44 checks against a fake `gh` (§4).

## 1. Problem (#173)

1. **Blind spots.** `owner-issues.sh` reads only conversation comments (`issues/comments`). Owner
   inline review comments (`pulls/comments`) and review bodies (`pulls/reviews`) are never fetched,
   so a session never sees them as instructions.
2. **No handled marker, silent drop.** Every owner comment from the last 14 days is printed, handled
   or not; each session decides from history whether it was handled. After 14 days a comment leaves
   the feed whether or not anyone acted on it.
3. **Found while writing this (same family): owner text that quotes the tag is dropped.** The script
   drops any owner text that *contains* `🤖 jpdf-agent`. #173 itself quotes the reply format, so it
   never appeared in the feed (checked on 2026-11-22: `ops/driver/owner-issues.sh` lists #176 but
   not #173; the session found #173 through the issue list). Agent text *starts with* the tag (the
   guard enforces that, `.claude/hooks/guard.py:542`), so the proposal drops only text that starts
   with it. A scan of every owner-account comment since 2026-11-01 found no agent comment that
   contains the tag without starting with it, and #173 is the only owner issue that does.

## 2. Proposed behavior

Output stays JSON lines on stdout, one object per instruction. Kinds:

| kind | source | fields |
|---|---|---|
| `issue` | open issues by the owner (unchanged) | `number, title, labels, is_pr, body` |
| `comment` | conversation comments on issues and PRs | `url, issue_url, created_at, stale, body` |
| `review_comment` | inline PR review comments (new) | `url, pull_request_url, path, line, created_at, stale, body` |
| `review` | review bodies on open PRs, empty bodies skipped (new) | `url, pull_request_url, state, created_at, stale, body` |

`url` (new) is the comment's `html_url`, the link the "done:" reply names. `issue_url` stays for
compatibility.

**Handled marker.** An owner comment is handled once a conversation comment exists whose author is
the owner's account and whose body **starts with** `🤖 jpdf-agent done: <url>`, with `<url>` the
comment's `html_url` (`…#issuecomment-N`, `…#discussion_rN` or `…#pullrequestreview-N` on this
repo). Matching is exact on the whole URL (`[0-9]+` is greedy, the lookup is a key lookup), so a
reply for `…-1234` never handles `…-123`. Text after the URL (a period, a newline, the summary) is
fine. Not counted: a stranger's reply (anyone can write the tag; only the shared account is
trusted, the same trust the tag check already relies on), a reply without the tag, a `done:`
anywhere but the start of the body, a URL on another repo.

**Age.**
- Comments created before `FLOOR` (`2026-11-23T00:00:00Z`; set it to the day you apply this) keep
  today's rule: shown while updated in the last 14 days. They predate the "done:" replies and are
  all handled; this keeps them from flooding the feed. They are all gone 14 days after `FLOOR`.
- Comments from `FLOOR` on stay until handled, whatever their age. One older than 14 days
  (`WARN_DAYS`) carries `"stale": true`, and the script prints
  `owner-issues: unhandled owner comment older than 14 days: <url>` on stderr.

**Failure.** Any failed `gh` call fails the script (non-zero exit): a missing page must never hide
an instruction. `set -e` already did this for today's two calls; the new calls keep it
(`pipefail` covers the `gh | jq` pipes).

**Cost.** Conversation and review comments are fetched from the earlier of `FLOOR` and 14 days ago
(by `updated_at`, the API's `since`), so the window grows from `FLOOR` on. That is all comments in
the repo since `FLOOR`, a few pages a week at today's rate. Reviews: one call per open PR. The data
goes to jq on stdin, not `--argjson`, so months of comments can't hit `ARG_MAX`. If the feed ever
gets slow, a later change can move `FLOOR` forward after checking that nothing before it is
unhandled.

**Dependency.** `jq` (macOS ships `/usr/bin/jq`; the script honors `$JQ`). Before, `gh --jq`
did the filtering, but the handled set spans pages, which a per-page `--jq` can't see.

## 3. What the agent does (in this PR, `ops/TICK.md`)

- Every owner instruction (issue, issue or PR comment, review comment, review body) gets two
  replies: `🤖 jpdf-agent picked up <link> (session <time>)` when work starts and
  `🤖 jpdf-agent done: <link>` + what changed + `waiting on owner: <action|nothing>` when it ends.
- Both go in the issue or PR conversation (`gh issue comment` / `gh pr comment`). An inline review
  comment can't get an in-thread reply: the guard allows comment writes only through
  `gh issue|pr comment`, not `gh api`. So the reply goes on the PR conversation and links the
  review comment's URL, which is what the handled marker matches.
- Until this proposal is applied, the feed still prints handled comments for 14 days; sessions
  check for a "done:" reply that links the comment before acting on it.

## 4. Test plan

Agents ran `test-owner-issues.sh` (fake `gh` on `PATH`, fixtures built with dates relative to
today, `FLOOR` moved to 30 days ago in a scratch copy; the harness refuses to run if that edit or
the first run fails):

```sh
SCRATCH=<an empty directory> ops/proposals/owner-comment-tracking/test-owner-issues.sh
# passed 44, failed 0
```

It covers: owner issue quoting the tag mid-body shown, tagged and stranger issues hidden, a null
body; pre-`FLOOR` comment outside / inside the 14-day window; post-`FLOOR` unhandled at 20 days
(shown, stale, one stderr warning) and at 2 days (not stale); handled by a reply on a later page;
stranger reply, untagged reply, `done:` on a later line, another repo's URL: all still shown; id
prefixes both ways (`-107` vs `-1070`, `-108` vs `-1080`); a reply ending in a period; review
comments (handled, unhandled, stranger), reviews (handled, empty body, a second PR, a second page);
the `kind` of each; the fetch starting at `FLOOR`, or 14 days ago when `FLOOR` is later; and a
non-zero exit when any one of the six calls fails.

Hand mutants (each one changed the script copy; the suite must fail): drop the owner check on
"done:" replies (2 failures), drop the `^` anchor (1), back to `contains($tag)` (2), pre-`FLOOR`
window always true (2), `stale` always false (3), keep empty review bodies (1), treat every comment
as post-`FLOOR` (2), drop the owner check on review comments (1), read only the first page (5),
accept only `/issues/` URLs (3).

Owner steps after applying:
1. Run `ops/proposals/owner-comment-tracking/test-owner-issues.sh ops/driver/owner-issues.sh`
   (the argument makes it test the applied file): `passed 44, failed 0`.
2. Run `ops/driver/owner-issues.sh` on the live repo: the output lists the same comments as
   before (all pre-`FLOOR`), now with `url`, plus #173 while it is open.
3. After `FLOOR`, leave one PR comment and one inline review comment; the next session should reply
   "picked up" and "done:"; the comment leaves the feed on the next run after the "done:" reply.

## 5. Risk if not applied

Owner review comments and review bodies stay invisible to sessions; unhandled comments still drop
silently after 14 days; owner text quoting the tag (like #173) stays out of the feed. The TICK
replies still make the lifecycle visible to the owner on GitHub.
