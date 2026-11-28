# Proposal: the driver keeps a worktree whose PR is still open

Status: proposed 2026-11-25 (owner interactive session). Touches `ops/driver/driver.sh`
(protected, CONSTITUTION 12), so the owner applies it with signed commits.

Files:
- `ops/proposals/driver-keep-open-worktrees/driver.sh`: the whole proposed driver (copy it over
  the current one; it already has the disk-check fix of PR #339).
- `ops/proposals/driver-keep-open-worktrees/test.sh`: runs the prune step on real git worktrees
  in a temp repo, with a fake `gh`.

## Problem

The driver deletes every directory under `.claude/worktrees/` whose own mtime is over 2 days old,
in two places: `sync_main`, before **every** session, and `disk_ok`'s low-disk cleanup:

```sh
find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} +
```

A directory's mtime changes only when an entry directly in it is added, removed or renamed; edits
deeper down (the code, `target/`) don't touch it. So a worktree created more than 2 days ago is
deleted at the next session start even while its PR is open: a PR waiting on the owner
(`owner-todo`, a design PR's window, a `/hold`), or one in review. What is lost:
- `target/scratch/`: the reviewer briefs (`review-<PR>.md`) and Codex output
  (`codex-<PR>.json`) that a resumed session or an audit reads (TICK §4 writes both there);
- any fix committed but not pushed, and the build cache (a full rebuild).

The pushed branch survives on GitHub, so no merged work is lost, but rule c's session on that PR
starts without its scratch files.

## Proposed change

```diff
+# Remove worktree directories older than 2 days, except one whose branch has an open PR
+# (a PR waiting on the owner or a review can sit for days; its worktree holds the session's
+# scratch briefs and an unpushed fix). Only PRs from this repo count: anyone can open a fork PR
+# with any head branch name. If the open-PR list can't be read, remove nothing.
+prune_worktrees() {
+  local open d b
+  open=$(gh pr list -R "$GH_REPO" --state open --limit 200 --json headRefName,isCrossRepository \
+    -q '.[] | select(.isCrossRepository | not) | .headRefName' 2>/dev/null) \
+    || { log "worktree cleanup skipped (gh error)"; return 0; }
+  for d in "$REPO_DIR"/.claude/worktrees/*/; do
+    d="${d%/}"
+    [[ -d "$d" && -n "$(find "$d" -maxdepth 0 -mtime +2 2>/dev/null)" ]] || continue
+    b=$(git -C "$d" symbolic-ref -q --short HEAD 2>/dev/null)
+    if [[ -n "$b" ]] && grep -qxF -- "$b" <<<"$open"; then
+      log "keeping worktree ${d##*/} (open PR on $b)"; continue
+    fi
+    rm -rf "$d"
+  done
+  git worktree prune
+}
@@ disk_ok
-    find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null
-    git worktree prune
+    prune_worktrees
@@ sync_main
-  find "$REPO_DIR/.claude/worktrees" -maxdepth 1 -mindepth 1 -type d -mtime +2 -exec rm -rf {} + 2>/dev/null || true
+  prune_worktrees
```

- The age rule is unchanged (the directory's own mtime, over 2 days).
- An old worktree is kept when its checked-out branch is the head branch of an open PR in this
  repo (exact name: `t/1` doesn't match `t/12-x`). A PR from a fork doesn't count (`isCrossRepository`):
  anyone can open one, with any head branch name. A detached HEAD or a branch with no open PR is
  removed as before, so a dead session's worktree still goes after 2 days, as today.
- If `gh` fails, nothing is removed that run (logged); the next session start tries again.
- Directories whose name starts with `.` are no longer removed (the shell glob skips them); the
  sessions never create such worktrees.
- One `gh pr list` call per session start, two when the disk is low (`disk_ok` and `sync_main`
  both prune); the driver already calls `gh` for the halt check.
- At most 200 open PRs are read; a worktree whose PR is past that is removed, as today.

## Tests

`bash ops/proposals/driver-keep-open-worktrees/test.sh [driver.sh]` (default: the proposed
driver). It builds a temp repo with eight worktrees (`t/12-open`, `t/13-nopr`, `t/1`,
`t/12-open-x`, `t/1.2`, a detached HEAD, all dated 3 days back; `t/15-edge`, dated 2 days 23 hours back,
just inside the age limit; and a fresh `t/14-fresh`) and a fake `gh` (needs `jq`) that acts like
`gh pr list`: it reads at most `--limit` PRs (default 30), keeps only the `--json` fields asked
for, applies the `-q` filter, marks fork PRs
`isCrossRepository: true`, and fails unless asked for `--state open`. It sets the dates with
POSIX `touch -t` and a UTC stamp, so it runs on macOS (where the driver runs) and on Linux, and
reads each mtime back with `stat`: a date that didn't take stops the run with `FAIL harness`
(#366; checked on macOS and in a `debian:trixie` container, with the same results in both):

| Case | Proposed driver | Current driver |
|---|---|---|
| open PRs `t/12-open`, `t/99-other`: only `open`, `edge` and `fresh` stay | ok | FAIL (`open` deleted) |
| the kept worktree is logged | ok | FAIL |
| `gh` fails: nothing removed | ok | FAIL (all old ones deleted) |
| no open PRs: every old one removed | ok | ok |
| open PRs `t/1`, `t/13-nopr`, `t/12-open-x`: those stay, `t/12-open` goes | ok | FAIL |
| 31 open PRs, `t/12-open` last: `open` stays | ok | FAIL (`open` deleted) |
| 200 open PRs, `t/12-open` last: `open` stays | ok | FAIL (`open` deleted) |
| 201 open PRs, `t/12-open` last (past the 200 read): `open` goes | ok | ok |
| open PR `t/12-open` and a fork's PR on `t/13-nopr`: `t/13-nopr` goes | ok | FAIL (`open` deleted) |
| open PR `t/1x2`: `t/1.2` goes (the name is matched as text, not as a regex, #367) | ok | ok |
| open PR `T/12-OPEN`: `open` goes (the match is case-sensitive, #429) | ok | ok |
| `git worktree prune` ran (no prunable entries) | ok | ok |

Hand mutants of the proposed driver, each failing a case: substring match (`grep -qF` without
`-x`), regex match (`grep -qx` without `-F`, #367), case-insensitive match (`grep -qxiF`, #429), ignoring a `gh` failure, dropping the age test, dropping the keep log, dropping the
detached-HEAD guard (`[[ -n "$b" ]]`), dropping `git worktree prune`, dropping the `-q` filter,
`--state all`, `--limit 1`, `--limit 30`, `--limit 199`, `--limit 201` or no `--limit` (any
`--limit` but 200 fails the 200 or the 201 case, #344), dropping the fork filter or its
`isCrossRepository` field, and `-mtime +0`, `+1` or `+3` for `+2`.

After applying: `ops/driver/driver.sh --pilot`, then `grep 'keeping worktree' ops/logs/driver.log`
lists the open PRs' worktrees older than 2 days, if any.

## Not in this proposal

The age is still the directory's own mtime, not the last commit or file change in it; a worktree
of a dead session with no PR is removed 2 days after it was created, as today.
