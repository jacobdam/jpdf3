# Proposal: the driver's disk check works on Linux, and an unknown reading cleans nothing

Status: proposed 2026-11-25 (owner interactive session). Touches `ops/driver/driver.sh`
(protected, CONSTITUTION 12), so the owner applies it with signed commits.

Files:
- `ops/proposals/driver-portable-df/driver.sh`: the whole proposed driver (copy it over the
  current one).
- `ops/proposals/driver-portable-df/test.sh`: runs `disk_ok` against a fake `df`, with `find`,
  `git` and `cargo` stubbed so nothing is cleaned.

## Problem

`disk_ok` reads free space with `df -g`, a BSD (macOS) flag. On the Linux VM that runs the loop
now, `df -g` fails (`df: invalid option -- 'g'`), `free` is empty, and the empty value reads as
0 GB. On 2026-11-25 the owner asked to resume the loop; the driver logged

```
[driver 2026-11-25T00:02:10Z] low disk ( GB < 30 GB): cleaning
[driver 2026-11-25T00:02:10Z] still low disk ( GB)
```

then ran its cleanup (deleted worktrees older than 2 days, `git worktree prune`, `cargo clean`)
and slept 3600 s, to do the same every hour without starting a session. The session stopped it
after the first cleanup; no worktree was old enough to be deleted, and `cargo clean` cost only a
rebuild. The VM also has 16 GB in all (9.4 GB free), under the 30 GB default, so the check
would stop it even with a working `df`.

## Proposed change

```diff
-MIN_FREE_GB="${JPDF_MIN_FREE_GB:-30}"
+MIN_FREE_GB="${JPDF_MIN_FREE_GB:-5}"
@@
+# Free space in whole GB. `df -P -k` is POSIX, so it works on macOS and Linux (`df -g` is
+# BSD-only: on Linux it fails, and the empty result read as 0 GB, which cleaned every hour).
+free_gb() {
+  df -P -k "$REPO_DIR" 2>/dev/null | awk 'NR==2 && $4 ~ /^[0-9]+$/ {print int($4 / 1048576)}'
+}
+
 disk_ok() {
   local free
-  free=$(df -g "$REPO_DIR" | awk 'NR==2 {print $4}')
+  free=$(free_gb)
+  # Unknown is not "low": clean nothing (cleaning deletes worktrees and the build cache) and
+  # start nothing.
+  [[ -n "$free" ]] || { log "disk check failed (df gave no number); not starting a session"; return 1; }
   if (( free < MIN_FREE_GB )); then
     log "low disk (${free} GB < ${MIN_FREE_GB} GB): cleaning"
     ...
-    free=$(df -g "$REPO_DIR" | awk 'NR==2 {print $4}')
-    (( free >= MIN_FREE_GB )) || { log "still low disk (${free} GB)"; return 1; }
+    free=$(free_gb)
+    [[ -n "$free" ]] && (( free >= MIN_FREE_GB )) || { log "still low disk (${free} GB)"; return 1; }
```

- `df -P -k` (POSIX output, 1024-byte blocks) is the same on macOS and Linux; column 4 is the
  available space. Whole GB are rounded down, so 5 GB minus 1 KB reads 4 and is low.
- An unreadable `df` (an error, or no number in column 4) logs, cleans nothing and starts no
  session; the loop retries in an hour as it does for low disk.
- The default minimum is 5 GB (the owner's choice on 2026-11-25). `JPDF_MIN_FREE_GB` still
  overrides it; the Mac mini can set it back to 30.

## Tests

`bash ops/proposals/driver-portable-df/test.sh` from the repo root:

| Case | Proposed driver | Current driver |
|---|---|---|
| 9 GB free, minimum 5 | ok (passes, no cleaning) | ok |
| exactly 5 GB | ok (passes) | ok |
| 1 KB under 5 GB | ok (low, cleans) | FAIL (passes) |
| `df` fails | ok (no cleaning, no session) | FAIL (cleans) |
| `df` prints no number | ok (no cleaning, no session) | FAIL (bash arithmetic error, passes) |

After applying, on the VM: `ops/driver/driver.sh --pilot` runs one session; the log must not show
`low disk` or `disk check failed`.

## Not in this proposal

The cleanup deletes any worktree directory older than 2 days, including ones whose PRs are
still open (a PR waiting on the owner for days). That is a separate question for the owner.
