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

`bash ops/proposals/driver-portable-df/test.sh [driver.sh]` from the repo root (default: the
proposed driver). It loads the driver's functions with `REPO_DIR` set to a temp dir, stubs
`find`, `git` and `cargo` (recording calls, so nothing is cleaned), and uses a fake `df` that,
like GNU df, rejects `-g` and anything but `-P -k <dir>`; a case passes only with the wanted
status, cleaning and an empty stderr.

| Case | Proposed driver | Current driver |
|---|---|---|
| 9 GB free, minimum 5 | ok (passes, no cleaning) | FAIL (cleans) |
| exactly 5 GB, minimum 5 | ok | FAIL |
| 1 KB under 5 GB (rounded down to 4) | ok (low, cleans) | FAIL (df error on stderr) |
| `JPDF_MIN_FREE_GB` unset, exactly 5 GB | ok (default 5) | FAIL |
| `JPDF_MIN_FREE_GB` unset, 1 KB under 5 GB | ok (low) | FAIL |
| 4 GB, then 6 GB after cleaning | ok (cleans, passes) | FAIL |
| 4 GB, then exactly 5 GB after cleaning | ok (cleans, passes) | FAIL |
| `df` fails | ok (no cleaning) | FAIL (cleans) |
| `df` prints no number | ok (no cleaning) | FAIL (cleans) |
| a number, then `df` fails after cleaning | ok (low) | FAIL |

The current driver fails every case for one reason: its `df -g` is rejected, and the empty value
reads as 0 GB, so it always cleans. That is the 2026-11-25 run.

Hand mutants of the proposed driver: `df -g` back, `df -k` without `-P`, the old 30 GB default,
and the first empty-value guard removed each fail a case. Removing the guard after cleaning
passes: bash reads an empty `free` as 0 in `(( ))`, so it still returns low with no error; the
guard stays for clarity.

After applying, on the VM: `ops/driver/driver.sh --pilot` runs one session; the log must not show
`low disk` or `disk check failed`. If it shows `disk check failed`, stop it (see below).

## Not in this proposal

In `--pilot` mode a failing disk check sleeps an hour and loops, as it does today, instead of
exiting like a failing halt check; stop it with Ctrl-C. (Round 2 of PR #338: a fix needs a test
of the main loop, which this harness doesn't run.)

The cleanup deletes any worktree directory older than 2 days, including ones whose PRs are
still open (a PR waiting on the owner for days). That is a separate question for the owner.
