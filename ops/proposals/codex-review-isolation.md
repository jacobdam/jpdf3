# Proposal: isolate the Codex review launcher and stop session leftovers (#161)

Status: proposed by a session for issue #161 (audit 0003 findings AS.52, AS.55, CX.1, CX.2, CX.3,
CX.11, CX.16), 2026-11-22. Touches protected paths (`ops/driver/codex-review.sh`,
`ops/driver/driver.sh`), so the owner applies it with signed commits. Agents can't run Codex outside
the launcher (the guard blocks it), so the Codex-side behavior below is checked against the
installed binary's strings (Codex 0.171.0) and the published docs, not run. The owner's test
plan (§4) runs it before the change is trusted.

## 1. Findings and fixes

| Finding | Where (main @ 51c0e46) | Fix in this proposal | How it was checked |
|---|---|---|---|
| AS.52, CX.1 (codex): Codex's commands can read any file the owner can: the sealed manifest in the main checkout, `~/.codex/auth.json`, `~/.ssh`, `~/jpdf-corpus`, `~/jpdf-research`. What it reads reaches OpenAI and can be posted on the PR (CONSTITUTION 1.2, 13). | `codex-review.sh:171` (`--sandbox workspace-write` limits writes and network, not reads) | Replace the legacy sandbox flags with a Codex permission profile: `:root` = deny, `:minimal` = read, the export = the writable workspace, plus read-only toolchain paths (§2). | Not run (§4 test plan T1–T4). Key names from the docs and the 0.171.0 binary. |
| CX.2 (codex): a reviewed HEAD whose `ops/judge` (or `ops`, `ops/judge/corpus`) is a symlink makes the following `rm -f` delete a file of the manifest's name wherever the link points, before the symlink check runs. | `codex-review.sh:118-122` | Refuse any symlink (mode 120000) in HEAD's tree before extracting; drop the `rm -f` (the `--exclude` keeps the file out and the byte-for-byte check already fails if it is there). | **Reproduced** with a scratch repo (a probe file name): `git archive \| tar --exclude` exits 0, then the `rm -f` deleted the target file through the link. The new pre-check refuses that repo (T5). bsdtar itself refuses to extract *through* a symlink, so the `rm` was the only path. |
| CX.3 (codex): `cargo fetch` runs outside the sandbox in the export, so a branch-controlled `.cargo/config.toml` (credential provider, source replacement) runs with the driver's permissions. Same for `rust-toolchain.toml`: rustup accepts a `path` entry, which would run a toolchain binary of the branch's choice. | `codex-review.sh:156` | Before the fetch: `.cargo/` in HEAD must have origin/main's tree id (it is protected); this worktree's `.cargo/` must be clean (cargo also reads configs from every directory above the export, the worktree included); no `.cargo/` in `target/` or `target/scratch/`; no legacy `rust-toolchain` file; `rust-toolchain.toml` may contain only `[toolchain]`, an exact `channel`, `components`, `targets`, `profile`, comments and blank lines. | Scratch tests T6: the repo's real file passes; `path = …`, `nightly`, CRLF and the legacy file fail. |
| AS.55: `chflags -R nouchg` and `chmod -R u+w` before `rm -rf` change the mode of any same-volume file Codex hard-linked into the export (probe in audit 0003: a 0444 file became 0644). | `codex-review.sh:48`, `:73` | `rmtree`: give `u+w` to directories only (a directory can't be hard-linked; unlinking a file needs only a writable parent), never touch file modes or flags. A file Codex made immutable stays behind and the run says so on stderr. | Scratch test T7: the outside 0444 file stays 0444 and the read-only tree is gone; the old pass, run the same way, makes it 0644. |
| CX.16 (codex): the output check accepts `findings: [null]`, findings without `severity`/`detail`, unknown severities, extra keys. | `codex-review.sh:185` | Check the schema's shape in jq: exact key sets, the severity enum, `line` an integer or null, string fields. | Scratch test T8: 3 valid shapes pass, 9 malformed ones fail. |
| CX.11 (codex): a helper the CLI starts in its own session (setsid) survives a normal CLI exit: it has left the session's process group and, once the CLI is gone, is reparented to launchd, so neither the group kill nor the descendant list finds it. | `driver.sh:190` | Record `pid start-time` of every descendant at each poll; after the CLI exits (however it ends), signal each recorded process whose start time still matches, its descendants, and what is left of the session's group: TERM, KILL after 15 s, re-checking the start time before every signal. | Scratch test T9: a fake CLI starts a setsid helper and exits 0; the helper survives (the bug), then `stop_leftovers` stops it; a bystander recorded with a wrong start time is untouched; a TERM-ignoring helper is killed after the grace period. |

## 2. The permission profile (AS.52, CX.1)

Codex 0.171.0 has permission profiles (the docs mark them Beta: "Permission profiles are under
active development and may change"). The docs' own "file access limited to workspace" example is:

```toml
default_permissions = "workspace-only"

[permissions.workspace-only]
extends = ":workspace"

[permissions.workspace-only.filesystem]
":root" = "deny"
":minimal" = "read"
":tmpdir" = "deny"
":slash_tmp" = "deny"
```

The launcher runs with `--ignore-user-config`, so it passes the same profile with `-c` overrides,
named `jpdf-review`:
- `:root = deny` denies every read by default; `:minimal = read` adds the platform and runtime paths
  common tools need; `extends = ":workspace"` makes the `-C` directory (the export) the writable
  workspace;
- read-only extras: `~/.rustup/toolchains`, `~/.rustup/settings.toml`, `~/.cargo/bin`,
  `~/.cargo/registry` (the crates `cargo fetch` stored for the offline build) and `/opt/homebrew`.
  `~/.cargo` as a whole is not granted, so `~/.cargo/credentials.toml` stays unreadable;
- `:slash_tmp = deny`; `:tmpdir` is left out because `TMPDIR` points inside the export
  (`target/tmp`), and denying it could deny part of the workspace;
- `network.enabled = false` (also the default).

The docs say a profile and the legacy settings don't combine: "Configure either
`default_permissions` and `[permissions]`, or `sandbox_mode` / `sandbox_workspace_write`, but not
both", and `--sandbox` makes Codex use the legacy settings instead of the profile. So the diff
removes `--sandbox workspace-write` and the three `sandbox_workspace_write.*` overrides. Their
effects carry over: writes only in the workspace (`:workspace`), no network
(`network.enabled = false`), no `/tmp` (`:slash_tmp = deny`).

Failure mode: `--strict-config` stays, so a key this Codex version doesn't know fails the run
(`status: unavailable`), it doesn't widen the sandbox. The binary also says a profile with no
recognized filesystem entries keeps filesystem access restricted.

Codex itself (outside the sandbox) still reads `~/.codex/auth.json` to call the API; that is the
CLI, not the model's commands.

### What this doesn't cover (residual risk, stated)
- The profile is Beta and was not run by the session (the guard allows Codex only through the
  launcher). T1–T4 must pass before the owner relies on it.
- `:minimal` is not documented path by path. If it includes something sensitive, T3 shows it.
- If `cargo test` inside the review needs a path outside the list, Codex's build fails (it reports
  that it couldn't build), the review itself still runs. T4 checks the build.
- Codex's own built-in file tools, if any run in the CLI process rather than in the sandbox, are
  not covered by any sandbox setting. T2 asks for a read through a tool call, not only a shell
  command.
- `cargo fetch` with the trusted configs still clones git dependencies named in the branch's
  `Cargo.lock` over the network (no secrets go out; an `ssh://` URL could use the owner's ssh agent
  to authenticate to a host of the branch's choice). The workspace has no git dependencies today.

## 3. The diffs

Generated with `diff -u` from main @ 51c0e46 to the patched copies the tests in §4 ran against (also
in `ops/proposals/codex-review-isolation/*.diff`). Context lines naming the sealed manifest are
unchanged code.

### `ops/driver/codex-review.sh`
```diff
--- a/ops/driver/codex-review.sh
+++ b/ops/driver/codex-review.sh
@@ -6,7 +6,8 @@
 # inside that directory, so bad arguments can't make it read or write anything else. (A same-user
 # process that deliberately races the checks is out of scope: it can write files directly anyway.)
 # Codex reviews a plain export of HEAD (`git archive`, checked byte for byte, in its own fresh git
-# repo, no symlinks, sealed corpus manifest left out) in sandbox workspace-write with the network,
+# repo, no symlinks, sealed corpus manifest left out) under a permission profile that lets its
+# commands read only the export and the toolchain and write only the export, with the network,
 # ChatGPT apps and plugins off, strict config and a scrubbed environment.
 # Output: <out>.json (codex-review.schema.json plus status/reason) and <out>.log (Codex's log).
 # Exit codes: 2 = usage error (nothing written). Otherwise 0, always: on any failure the output
@@ -44,8 +45,16 @@
 [[ ! -d "$OUT" && ! -d "$LOG" ]] || usage "<out> or its .log is a directory"
 # Leftovers of a run that was killed outright (SIGKILL), once older than any run can last.
 # Temp files use the reserved prefix .codex-review-tmp. so this can't match anything else.
-find . -maxdepth 1 -name '.codex-review-tmp.*' -mmin +60 \
-  -exec chflags -R nouchg {} + -exec chmod -R u+w {} + -exec rm -rf {} + 2>/dev/null
+# rmtree <path>: delete a tree Codex could write to. Only directories get u+w (a directory can't be
+# hard-linked); files keep their mode and flags: chmod or chflags on a file Codex hard-linked in from
+# elsewhere on the volume would change that outside file (AS.55). Unlinking needs no file permission.
+# A file Codex made immutable (uchg) stays behind, and the run says so.
+rmtree() {
+  [[ -n "$1" && ( -e "$1" || -L "$1" ) ]] || return 0
+  find "$1" -type d ! -perm -u+w -exec chmod u+w {} + 2>/dev/null
+  rm -rf -- "$1" 2>/dev/null || echo "codex-review: could not remove all of target/scratch/$1" >&2
+}
+for d in $(find . -maxdepth 1 -name '.codex-review-tmp.*' -mmin +60); do rmtree "$d"; done
 
 pid=""; dog=""; SRC=""; PROMPT=""; TMARK=""; RAW=""; TLOG=""; T=""
 put() {   # put <name> <tmpfile>: rename into place (replaces a link instead of following it)
@@ -70,7 +79,7 @@
 cleanup() {
   [[ -n "$dog" ]] && kill -TERM "$dog" 2>/dev/null
   reap
-  [[ -n "$SRC" ]] && { chflags -R nouchg "$SRC"; chmod -R u+w "$SRC"; rm -rf -- "$SRC"; } 2>/dev/null
+  [[ -n "$SRC" ]] && rmtree "$SRC"
   local f; for f in "$PROMPT" "$TMARK" "$RAW" "$TLOG" "$T"; do [[ -n "$f" ]] && rm -f -- "$f"; done
 }
 trap cleanup EXIT
@@ -115,10 +124,18 @@
 XGIT=(env -i PATH="$PATH" HOME="$HOME" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
   git -c core.hooksPath=/dev/null -c init.templateDir= -c core.fsmonitor=false -c commit.gpgsign=false
   -c core.attributesFile=/dev/null -c user.name=export -c user.email=export@invalid)
+# A committed symlink could point Codex's reads, or a path operation of this script, outside the
+# export (CX.2: a symlinked ancestor directory of the excluded manifest). Refuse it before anything
+# is extracted, so no path below $SRC is touched while a link could be on it.
+"${XGIT[@]}" -C "$TOP" ls-tree -r -z --full-tree HEAD | /usr/bin/python3 -I -c '
+import sys
+recs = [r for r in sys.stdin.buffer.read().split(b"\0") if r]
+sys.exit(1 if not recs or any(r.split(b" ", 1)[0] == b"120000" for r in recs) else 0)' \
+  || fail "HEAD contains symlinks (or ls-tree failed)"
 "${XGIT[@]}" -C "$TOP" archive --format=tar HEAD | tar -x -C "$SRC" --exclude 'ops/judge/corpus/exam.tsv' \
   || fail "export failed"
-rm -f -- "$SRC/ops/judge/corpus/exam.tsv"
-# A committed symlink could point Codex's reads (and this script's mkdir) outside the export.
+# No rm of the excluded path here: --exclude keeps it out, and the byte check below fails if it is
+# there. Belt and braces for the check above: no symlink may exist in the export.
 [[ -z "$(find "$SRC" -type l -print -quit)" ]] || fail "export contains symlinks"
 # Attributes (export-ignore, export-subst, working-tree-encoding, ident, eol, filters) can drop or
 # rewrite files during `git archive`, so check that the export is byte for byte HEAD's blobs.
@@ -153,12 +170,51 @@
   PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:$HOME/.cargo/bin"
   TMPDIR="$SCR/$SRC/target/tmp" CARGO_TARGET_DIR="$SCR/$SRC/target" GIT_CEILING_DIRECTORIES="$SCR")
 # Fetch crates (and the pinned toolchain) now, outside the sandbox, so Codex can build offline.
+# This runs with the driver's permissions, so nothing the reviewed branch controls may configure it
+# (CX.3): cargo reads .cargo/config.toml from the export and from every directory above it (this
+# worktree's working copy included), and rustup reads the export's toolchain file (a `path` entry
+# there would run a toolchain binary of the branch's choice). So: .cargo/ in HEAD must be origin/main's
+# tree (it is protected, CONSTITUTION 12); this worktree's .cargo/ must be clean; no .cargo/ may sit in
+# target/ or target/scratch/; and the toolchain file may only pin an exact channel, components,
+# targets and profile. Directories above the worktree (the main checkout, $HOME) are the owner's.
+XG=("${XGIT[@]}" -C "$TOP")
+[[ "$("${XG[@]}" rev-parse -q --verify HEAD:.cargo 2>/dev/null)" == \
+   "$("${XG[@]}" rev-parse -q --verify refs/remotes/origin/main:.cargo 2>/dev/null)" ]] \
+  || fail ".cargo/ differs from origin/main"
+[[ -z "$("${XG[@]}" status --porcelain --ignored --untracked-files=all -- .cargo 2>&1)" ]] \
+  || fail "this worktree's .cargo/ has local changes"
+[[ ! -e "$TOP/target/.cargo" && ! -L "$TOP/target/.cargo" && ! -e "$SCR/.cargo" && ! -L "$SCR/.cargo" ]] \
+  || fail "a .cargo/ in target/ or target/scratch/"
+[[ ! -e "$SRC/rust-toolchain" ]] || fail "legacy rust-toolchain file in HEAD"
+# grep -v exits 1 only when every line matches; 0 (a line that doesn't) and 2 (an error) both fail.
+if [[ -e "$SRC/rust-toolchain.toml" ]]; then
+  grep -qvE \
+    '^(\[toolchain\]|channel = "[0-9]+\.[0-9]+\.[0-9]+"|(components|targets) = \[( *"[a-z0-9_-]+",?)* *\]|profile = "(minimal|default|complete)"|#.*)?$' \
+    "$SRC/rust-toolchain.toml"
+  (( $? == 1 )) || fail "rust-toolchain.toml has entries beyond channel/components/targets/profile"
+fi
 run_limited "$FETCH_LIMIT" "${BASE_ENV[@]}" /bin/sh -c 'cd "$1" && exec cargo fetch --locked -q' _ "$SCR/$SRC" \
   > /dev/null 2>&1 < /dev/null
 rc=$?
 [[ -e "$TMARK" ]] && fail "cargo fetch timeout after ${FETCH_LIMIT}s"
 (( rc == 0 )) || fail "cargo fetch failed ($rc)"
 
+# Codex's commands may read only the export (its workspace, writable), the platform paths of
+# `:minimal`, and the read-only toolchain: rustup toolchains, cargo's binaries and crate cache, and
+# Homebrew. Everything else is denied by `:root`, so the sealed manifest in the main checkout,
+# ~/.codex (auth), ~/.ssh, ~/.cargo/credentials.toml, ~/jpdf-corpus and ~/jpdf-research are
+# unreadable (AS.52, CX.1; CONSTITUTION 1.2, 13). `--sandbox workspace-write` limited writes and
+# network only. A permission profile replaces the legacy sandbox flags (Codex: "either
+# default_permissions and [permissions], or sandbox_mode / sandbox_workspace_write, but not both"),
+# so none of them may appear here. Profiles are Beta in Codex; --strict-config makes an unknown key a
+# failed run ("unavailable"), never a silently wider sandbox.
+q() { local s="${1//\\/\\\\}"; printf '"%s"' "${s//\"/\\\"}"; }   # TOML basic string
+FS_POLICY="{ \":root\" = \"deny\", \":minimal\" = \"read\", \":slash_tmp\" = \"deny\""
+for r in "$HOME/.rustup/toolchains" "$HOME/.rustup/settings.toml" "$HOME/.cargo/bin" \
+         "$HOME/.cargo/registry" /opt/homebrew; do
+  FS_POLICY+=", $(q "$r") = \"read\""
+done
+FS_POLICY+=" }"
 RAW="$(mktemp .codex-review-tmp.out.XXXXXX)" || fail "mktemp"
 TLOG="$(mktemp .codex-review-tmp.log.XXXXXX)" || fail "mktemp"
 run_limited "$LIMIT" "${BASE_ENV[@]}" CARGO_NET_OFFLINE=true CMUX_COMPUTER_USE_MCP_DISABLED=1 \
@@ -168,10 +224,10 @@
   --disable apps --disable plugins --disable shell_snapshot --disable hooks \
   --disable computer_use --disable browser_use --disable browser_use_external \
   --disable in_app_browser --disable image_generation \
-  --sandbox workspace-write \
-  -c sandbox_workspace_write.network_access=false \
-  -c sandbox_workspace_write.exclude_slash_tmp=true \
-  -c sandbox_workspace_write.exclude_tmpdir_env_var=true \
+  -c default_permissions='"jpdf-review"' \
+  -c permissions.jpdf-review.extends='":workspace"' \
+  -c permissions.jpdf-review.filesystem="$FS_POLICY" \
+  -c permissions.jpdf-review.network.enabled=false \
   -c web_search=disabled \
   -C "$SCR/$SRC" \
   --output-schema "$SCHEMA" \
@@ -182,7 +238,13 @@
 
 [[ -e "$TMARK" ]] && fail "timeout after ${LIMIT}s (see target/scratch/$LOG)"
 (( rc == 0 )) || fail "codex exit $rc (see target/scratch/$LOG)"
-jq -e '(.verdict | type == "string") and (.findings | type == "array")' "$RAW" >/dev/null 2>&1 \
+# The schema's shape, checked here too (CX.16): Codex enforcing --output-schema is not ours to rely on.
+jq -e 'type == "object" and (keys == ["findings", "verdict"]) and (.verdict | type == "string")
+  and (.findings | type == "array") and all(.findings[];
+    type == "object" and (keys == ["detail", "file", "fix", "line", "severity", "title"])
+    and (.severity | IN("blocker", "major", "minor", "nit"))
+    and (.line == null or (.line | type == "number" and . == floor))
+    and ([.file, .title, .detail, .fix] | all(type == "string")))' "$RAW" >/dev/null 2>&1 \
   || fail "output not schema JSON (see target/scratch/$LOG)"
 T="$(mktemp .codex-review-tmp.out.XXXXXX)" && jq '. + {status:"ok",reason:""}' "$RAW" > "$T" && put "$OUT" "$T" \
   || fail "write failed"
```

### `ops/driver/driver.sh`
```diff
--- a/ops/driver/driver.sh
+++ b/ops/driver/driver.sh
@@ -144,10 +144,52 @@
   done
   kill -KILL -- "-$pid" 2>/dev/null
   for p in $pids; do kill -KILL "$p" 2>/dev/null; done
+}
+
+# Session helpers that outlive the CLI (CX.11): a helper that calls setsid leaves the session's
+# process group, and once the CLI exits it is reparented to launchd, so neither the group nor the
+# descendant list finds it after a normal exit. track_tree records "pid start-time" for every
+# descendant at each poll; stop_leftovers signals, after the CLI is gone, each recorded process
+# whose start time still matches (so a reused pid is never hit), plus its own descendants and what
+# is left of the session's group. A helper started and detached within one poll interval
+# (POLL seconds) before the CLI exits is not seen; that gap is the residual risk.
+start_time() { ps -o lstart= -p "$1" 2>/dev/null | sed 's/  */ /g; s/ *$//'; }
+track_tree() {  # track_tree <root pid> <file>
+  local p st
+  for p in $(descendants "$1"); do
+    st=$(start_time "$p"); [[ -n "$st" ]] && printf '%s %s\n' "$p" "$st"
+  done >> "$2"
+}
+stop_leftovers() {  # stop_leftovers <reaped leader pid> <file>
+  local leader=$1 file=$2 p q st t alive i n=0
+  local -a lp=() lst=()
+  # Recorded processes still alive with the same start time, then their descendants (with the
+  # start time seen now), so every signal below re-checks it and never hits a reused pid.
+  while read -r p st; do
+    [[ -n "$p" && "$(start_time "$p")" == "$st" ]] || continue
+    for q in "$p" $(descendants "$p"); do
+      st=$(start_time "$q"); [[ -n "$st" ]] && { lp[n]=$q; lst[n]=$st; n=$((n + 1)); }
+    done
+  done < <(sort -u "$file" 2>/dev/null)
+  ours() { [[ "$(start_time "${lp[$1]}")" == "${lst[$1]}" ]]; }
+  # A process group id is not reused while the group has members, so -leader is still ours.
+  (( n == 0 )) && ! kill -0 -- "-$leader" 2>/dev/null && return 0
+  log "stopping session leftovers: group $leader, pids ${lp[*]:-none}"
+  kill -TERM -- "-$leader" 2>/dev/null
+  for ((i = 0; i < n; i++)); do ours "$i" && kill -TERM "${lp[i]}" 2>/dev/null; done
+  t=$(perl -MTime::HiRes=time -e 'printf "%.3f\n", time')
+  while (( $(secs_since "$t") < 15 )); do
+    alive=0; kill -0 -- "-$leader" 2>/dev/null && alive=1
+    for ((i = 0; i < n; i++)); do ours "$i" && alive=1; done
+    (( alive )) || return 0
+    sleep 0.2
+  done
+  kill -KILL -- "-$leader" 2>/dev/null
+  for ((i = 0; i < n; i++)); do ours "$i" && kill -KILL "${lp[i]}" 2>/dev/null; done
 }
 
 run_session() {
-  local ts out err start t0 rc res sid pid="" idle state why=""
+  local ts out err start t0 rc res sid pid="" idle state why="" track
   ts=$(date -u +%Y%m%dT%H%M%SZ); out="ops/logs/$ts.jsonl"; err="ops/logs/$ts.stderr"; start=$(date +%s)
   t0=$(perl -MTime::HiRes=time -e 'printf "%.3f\n", time')
   log "session $ts starting"
@@ -164,6 +206,7 @@
       --strict-mcp-config --mcp-config ops/driver/mcp.json \
       --permission-mode auto --output-format stream-json --verbose > "$out" 2> "$err" &
   pid=$!
+  track="ops/state/session-$ts.pids"; : > "$track"
   # A finished `claude -p` can stay alive (a pending ScheduleWakeup; #125). Stop it once its last
   # turn event is a result, no background task runs, and the log has been quiet for SETTLE_GRACE
   # seconds. A background task's notification starts a new turn (system/init), so waits on agents
@@ -171,6 +214,7 @@
   # BG_QUIET_MAX quiet seconds, and SESSION_MAX caps everything.
   while kill -0 "$pid" 2>/dev/null; do
     sleep "$POLL"
+    track_tree "$pid" "$track"
     if (( $(secs_since "$t0") >= SESSION_MAX )); then
       why="SESSION_MAX"; log "session $ts reached SESSION_MAX (${SESSION_MAX}s): stopping"
       stop_group "$pid"; break
@@ -188,6 +232,8 @@
     fi
   done
   wait "$pid"; rc=$?
+  # However it ended (normal exit included), stop what it left behind before the next session.
+  stop_leftovers "$pid" "$track"; rm -f "$track"
   trap - HUP INT TERM
   # Stopped by us after its final result (however the CLI exits on TERM), or killed by the
   # owner's signal after it had answered: report what the session itself reported in its final
```

## 4. Test plan

### Already run by the session (against patched scratch copies)
The diffs and both test scripts are also in `ops/proposals/codex-review-isolation/`. Run them from
the repository root against the patched files (after applying, the real ones):
`ops/proposals/codex-review-isolation/test-codex.sh ops/driver/codex-review.sh` and
`ops/proposals/codex-review-isolation/test-driver.sh ops/driver/driver.sh`.
- T5–T8: `test-codex.sh`, 15 checks, all pass. Each piece is pulled out of the patched script by
  its exact text, so the tests run the proposed code, not a copy of it:
  - T5 symlink pre-check: a plain repo passes; a repo with `ops/judge` → symlink and an empty repo
    are refused;
  - T6 toolchain file: the repo's real `rust-toolchain.toml` and a file with targets/profile/comments
    pass; `path = "/x/y"`, `channel = "nightly"`, a CRLF line and a legacy `rust-toolchain` fail.
    (Writing the tests caught a fail-open bug in the first draft: BSD grep rejects an empty
    alternative in `(…|)$`, exits 2, and `! grep` passed everything. The check now takes only exit 1
    as success);
  - T7 `rmtree`: a 0444 file hard-linked into a 0555 tree keeps mode 0444 and the tree is gone;
    the old `chmod -R` pass on the same layout makes it 0644 (the AS.55 bug);
  - TOML: the generated `FS_POLICY` parses with Python's `tomllib` as an 8-entry table, including a
    `$HOME` with `"` and `\` in it, and names no `.codex`, `.ssh` or credentials path;
  - T8 jq output check: 3 valid shapes pass; `[null]`, `[{}]`, an unknown severity, a missing key,
    `line` 3.5, a non-string `detail`, an extra top-level key, a top-level array and a numeric
    verdict fail.
- T9: `test-driver.sh`, 9 checks, all pass (the KILL case waits about 15 s).
- Hand mutants, each caught by the tests: dropping the start-time check in `stop_leftovers` (the
  bystander is killed); `chmod u+w` on files too in `rmtree` (the outside file becomes 0644). Not
  covered by a test: the expansion to descendants of a recorded process (a helper's own child
  started after the last poll).
- `bash -n` on both patched files.

### For the owner, after applying (Codex runs only through the launcher)
Use a scratch worktree and a brief that asks Codex to run each command and quote its output in a
finding (findings are data; nothing here asks it to do anything else).
- **T1 the profile loads:** run the launcher on any small brief. Expect `status: ok`. If it is
  `unavailable` with a config error in the `.log`, the `-c` syntax for the profile differs in this
  Codex version: don't fall back to `--sandbox workspace-write` silently; fix the keys first.
- **T2 reads outside the export fail:** plant canary files `~/jpdf-research/.canary-161`,
  `~/.codex/canary-161` and `<main checkout>/target/canary-161`, each with a random token. The brief
  asks Codex to `cat` each one, `ls ~/.ssh`, and read each canary with its file-reading tool, if it
  has one. Expect "Operation not permitted" (or an equivalent denial) for every one and no token in
  the output or the `.log`. Delete the canaries afterwards.
- **T3 `:minimal` is minimal:** the brief asks for `ls ~`, `ls ~/Library` and
  `cat ~/.cargo/credentials.toml` (create an empty one first if none exists). Expect denials.
- **T4 the review can still build:** the brief asks for `cargo test -p jpdf-syntax --offline -q`
  in the export. Expect it to compile and run. If it fails on a denied path, add that read-only path
  to the list (never `$HOME` or `~/.cargo` as a whole).
- **T10 no network:** the brief asks for `curl -sS https://example.com`. Expect a failure.
- **T11 driver leftovers:** run `ops/driver/driver.sh --once` with a session that starts
  `perl -MPOSIX=setsid -e 'setsid; sleep 600' &` and then ends normally; expect the log line
  "stopping session leftovers" and no such sleeper afterwards (`pgrep -f 'sleep 600'`).
- Run `ops/driver/codex-review.sh` on a branch whose HEAD changes `.cargo/config.toml`, or adds
  `path = "x"` to `rust-toolchain.toml`: expect `unavailable` with the matching reason and no
  `cargo fetch` run.

## 5. Risk if not applied
- AS.52/CX.1 stays: a prompt-injected or misbehaving Codex can read the sealed corpus manifest,
  the Codex/ssh credentials and the research folders, send them to OpenAI and have them posted on a
  public PR (sev:high by ADR 0009, issue #161).
- A branch with a symlinked `ops/judge` deletes a file outside the export (CX.2); a branch with its
  own cargo config runs a program with the driver's permissions during `cargo fetch` (CX.3).
- Cleanup can widen modes of outside files (AS.55); malformed Codex output passes as `ok` (CX.16);
  session helpers can outlive their session and overlap the next one (CX.11).

Every branch these scripts see is written by an agent session, so the threat is a misbehaving or
prompt-injected agent, not an outside attacker; that is why #161 is `sev:high`, not critical.
