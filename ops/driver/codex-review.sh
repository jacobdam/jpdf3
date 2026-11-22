#!/usr/bin/env bash
# PROTECTED (CONSTITUTION 12). Second-opinion review by Codex (ops/proposals/codex-second-reviewer.md).
#   ops/driver/codex-review.sh target/scratch/<brief>.md target/scratch/<out>.json
# Run from a worktree of this repository. Both files must sit directly in that worktree's
# target/scratch/ (plain names, no symlinks or hard links), and every write goes through a rename
# inside that directory, so bad arguments can't make it read or write anything else. (A same-user
# process that deliberately races the checks is out of scope: it can write files directly anyway.)
# Codex reviews a plain export of HEAD (`git archive`, checked byte for byte, in its own fresh git
# repo, no symlinks, sealed corpus manifest left out) in sandbox workspace-write with the network,
# ChatGPT apps and plugins off, strict config and a scrubbed environment.
# Output: <out>.json (codex-review.schema.json plus status/reason) and <out>.log (Codex's log).
# Exit codes: 2 = usage error (nothing written). Otherwise 0, always: on any failure the output
# is {"status":"unavailable","reason":...}.
set -uo pipefail
export PATH=/usr/bin:/bin:/usr/sbin:/sbin; unset CDPATH
for v in $(compgen -e); do [[ "$v" == GIT_* ]] && unset "$v"; done   # no caller GIT_DIR etc.
CODEX=/opt/homebrew/bin/codex     # the real binary, not a terminal wrapper on PATH
LIMIT=2400                        # seconds for the Codex run
FETCH_LIMIT=300                   # seconds for `cargo fetch`
MAX_BRIEF=2000000                 # bytes
HERE="$(cd "$(dirname "$0")" && pwd -P)"
SCHEMA="$HERE/codex-review.schema.json"; PREAMBLE="$HERE/codex-review.preamble.md"

[[ -n "${HOME:-}" ]] || { echo "codex-review: HOME is not set" >&2; exit 2; }
usage() { echo "codex-review: $1" >&2; echo "usage: $0 target/scratch/<brief> target/scratch/<out>.json" >&2; exit 2; }
common() { (cd "$1" && cd "$(git rev-parse --git-common-dir 2>/dev/null)" 2>/dev/null && pwd -P); }
TOP="$(git rev-parse --show-toplevel 2>/dev/null)" || usage "not in a git worktree"
TOP="$(cd "$TOP" && pwd -P)"
[[ -n "$(common "$TOP")" && "$(common "$TOP")" == "$(common "$HERE")" ]] || usage "run from a worktree of this repository"
[[ ! -L "$TOP/target" && ! -L "$TOP/target/scratch" ]] || usage "target/ and target/scratch/ must not be symlinks"
SCR="$TOP/target/scratch"
mkdir -p "$SCR" && cd "$SCR" || usage "cannot use $SCR"
[[ "$(pwd -P)" == "$SCR" ]] || usage "target/scratch moved"
name_in_scr() {   # $1 = argument; prints its plain file name if it names a file directly in $SCR
  local n="${1##*/}" d
  [[ "$1" == */* ]] && d="${1%/*}" || d=.
  [[ "$n" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || return 1
  [[ "$(cd "$OLDPWD" 2>/dev/null && cd "$d" 2>/dev/null && pwd -P)" == "$SCR" ]] || return 1
  echo "$n"
}
OUT="$(name_in_scr "${2:-}")" && [[ "$OUT" == *.json ]] || usage "<out> must be a .json file directly in target/scratch/"
BRIEF="$(name_in_scr "${1:-}")" || usage "<brief> must be a file directly in target/scratch/"
LOG="${OUT%.json}.log"
[[ ! -d "$OUT" && ! -d "$LOG" ]] || usage "<out> or its .log is a directory"
# Leftovers of a run that was killed outright (SIGKILL), once older than any run can last.
# Temp files use the reserved prefix .codex-review-tmp. so this can't match anything else.
find . -maxdepth 1 -name '.codex-review-tmp.*' -mmin +60 \
  -exec chflags -R nouchg {} + -exec chmod -R u+w {} + -exec rm -rf {} + 2>/dev/null

pid=""; dog=""; SRC=""; PROMPT=""; TMARK=""; RAW=""; TLOG=""; T=""
put() {   # put <name> <tmpfile>: rename into place (replaces a link instead of following it)
  mv -fh -- "$2" "$1"   # BSD mv: -h = don't follow a symlink to a directory
}
fail() {
  T="$(mktemp .codex-review-tmp.fail.XXXXXX)" || exit 0
  if command -v jq >/dev/null 2>&1; then
    jq -n --arg r "$1" '{status:"unavailable",reason:$r,verdict:"",findings:[]}' > "$T"
  else
    printf '{"status":"unavailable","reason":"%s","verdict":"","findings":[]}\n' "${1//[\"\\]/_}" > "$T"
  fi
  put "$OUT" "$T"
  echo "codex-review: unavailable: $1" >&2
  exit 0
}
reap() {  # stop what is left of the process group: TERM, then KILL after a grace period
  [[ -n "$pid" ]] || return 0
  if kill -TERM -- -"$pid" 2>/dev/null; then sleep 3; kill -KILL -- -"$pid" 2>/dev/null; fi
  pid=""
}
cleanup() {
  [[ -n "$dog" ]] && kill -TERM "$dog" 2>/dev/null
  reap
  [[ -n "$SRC" ]] && { chflags -R nouchg "$SRC"; chmod -R u+w "$SRC"; rm -rf -- "$SRC"; } 2>/dev/null
  local f; for f in "$PROMPT" "$TMARK" "$RAW" "$TLOG" "$T"; do [[ -n "$f" ]] && rm -f -- "$f"; done
}
trap cleanup EXIT
trap 'fail "interrupted"' TERM INT HUP

# run_limited <seconds> <cmd...>: run cmd in its own process group (set -m), so a timeout or
# cleanup also stops its children (cargo, tests). Returns cmd's exit code; creates $TMARK on timeout.
# The watchdog has its own process group too and polls this script, so it still stops cmd if the
# script's whole group is killed.
run_limited() {
  local lim="$1" rc; shift
  set -m; "$@" & pid=$!
  ( me=$$; i=0
    while (( i < lim )) && kill -0 "$pid" 2>/dev/null; do
      sleep 1; i=$((i + 1)); kill -0 "$me" 2>/dev/null || break
    done
    kill -0 "$pid" 2>/dev/null || exit 0
    (( i >= lim )) && : > "$TMARK"
    kill -TERM -- -"$pid"; sleep 3; kill -KILL -- -"$pid" ) >/dev/null 2>&1 &
  dog=$!; set +m
  wait "$pid"; rc=$?
  kill -TERM "$dog" 2>/dev/null; wait "$dog" 2>/dev/null; dog=""
  reap
  return "$rc"
}

[[ -x "$CODEX" ]] || fail "codex not installed"
command -v jq >/dev/null 2>&1 || fail "jq not installed"
TMARK="$(mktemp .codex-review-tmp.timeout.XXXXXX)" && rm -f -- "$TMARK" || fail "mktemp"
PROMPT="$(mktemp .codex-review-tmp.prompt.XXXXXX)" || fail "mktemp"
[[ -f "$PREAMBLE" && -s "$PREAMBLE" ]] || fail "preamble missing"
# Codex's -o file and log sit in $SCR, outside its writable root, so Codex can't plant a link there.
# Copy the brief once, now; check it is a small regular file with a single link.
[[ -f "$BRIEF" && ! -L "$BRIEF" && "$(stat -f %l -- "$BRIEF")" == 1 ]] || fail "brief is not a plain file"
(( $(stat -f %z -- "$BRIEF") <= MAX_BRIEF )) || fail "brief larger than $MAX_BRIEF bytes"
{ cat -- "$PREAMBLE" && echo && head -c "$MAX_BRIEF" -- "$BRIEF"; } > "$PROMPT" || fail "prompt"

SRC="$(mktemp -d .codex-review-tmp.src.XXXXXX)" || fail "mktemp"
# A plain export of HEAD, sealed manifest excluded (CONSTITUTION 13), in its own fresh git repo:
# git commands inside it stop there instead of walking up into this worktree's repository.
# Git commands here run without the owner's git config, hooks or templates.
XGIT=(env -i PATH="$PATH" HOME="$HOME" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
  git -c core.hooksPath=/dev/null -c init.templateDir= -c core.fsmonitor=false -c commit.gpgsign=false
  -c core.attributesFile=/dev/null -c user.name=export -c user.email=export@invalid)
"${XGIT[@]}" -C "$TOP" archive --format=tar HEAD | tar -x -C "$SRC" --exclude 'ops/judge/corpus/exam.tsv' \
  || fail "export failed"
rm -f -- "$SRC/ops/judge/corpus/exam.tsv"
# A committed symlink could point Codex's reads (and this script's mkdir) outside the export.
[[ -z "$(find "$SRC" -type l -print -quit)" ]] || fail "export contains symlinks"
# Attributes (export-ignore, export-subst, working-tree-encoding, ident, eol, filters) can drop or
# rewrite files during `git archive`, so check that the export is byte for byte HEAD's blobs.
/usr/bin/python3 -I - "$TOP" "$SCR/$SRC" <<'PY' || fail "export differs from HEAD (attributes?)"
import hashlib, os, subprocess, sys
top, src = sys.argv[1].encode(), sys.argv[2].encode()
tree = subprocess.run(["git", "-C", top, "ls-tree", "-r", "-z", "--full-tree", "HEAD"],
                      capture_output=True, check=True).stdout
want = {}
for rec in tree.split(b"\0"):
    if rec:
        meta, path = rec.split(b"\t", 1)
        if meta.split()[1] != b"blob":
            sys.exit(1)  # a submodule: not supported
        want[path] = meta.split()[2]
want.pop(b"ops/judge/corpus/exam.tsv", None)
have = {}
for root, _, files in os.walk(src):
    for f in files:
        p = os.path.join(root, f)
        with open(p, "rb") as fh:
            data = fh.read()
        have[os.path.relpath(p, src)] = hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest().encode()
sys.exit(0 if have == want else 1)
PY
# -f: also tracked files a committed .gitignore matches.
( cd "$SRC" && "${XGIT[@]}" init -q && "${XGIT[@]}" add -A -f \
  && "${XGIT[@]}" commit -q --no-verify -m "export of HEAD" ) >/dev/null 2>&1 || fail "export repo failed"
mkdir -p "$SRC/target/tmp" || fail "mkdir"

BASE_ENV=(env -i HOME="$HOME" USER="${USER:-}" LOGNAME="${LOGNAME:-}" LANG=en_US.UTF-8
  PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:$HOME/.cargo/bin"
  TMPDIR="$SCR/$SRC/target/tmp" CARGO_TARGET_DIR="$SCR/$SRC/target" GIT_CEILING_DIRECTORIES="$SCR")
# Fetch crates (and the pinned toolchain) now, outside the sandbox, so Codex can build offline.
run_limited "$FETCH_LIMIT" "${BASE_ENV[@]}" /bin/sh -c 'cd "$1" && exec cargo fetch --locked -q' _ "$SCR/$SRC" \
  > /dev/null 2>&1 < /dev/null
rc=$?
[[ -e "$TMARK" ]] && fail "cargo fetch timeout after ${FETCH_LIMIT}s"
(( rc == 0 )) || fail "cargo fetch failed ($rc)"

RAW="$(mktemp .codex-review-tmp.out.XXXXXX)" || fail "mktemp"
TLOG="$(mktemp .codex-review-tmp.log.XXXXXX)" || fail "mktemp"
run_limited "$LIMIT" "${BASE_ENV[@]}" CARGO_NET_OFFLINE=true CMUX_COMPUTER_USE_MCP_DISABLED=1 \
  "$CODEX" exec \
  --ignore-user-config --ignore-rules --ephemeral --strict-config \
  --disable apps --disable plugins --disable shell_snapshot --disable hooks \
  --disable computer_use --disable browser_use --disable browser_use_external \
  --disable in_app_browser --disable image_generation \
  --sandbox workspace-write \
  -c sandbox_workspace_write.network_access=false \
  -c sandbox_workspace_write.exclude_slash_tmp=true \
  -c sandbox_workspace_write.exclude_tmpdir_env_var=true \
  -c web_search=disabled \
  -C "$SCR/$SRC" \
  --output-schema "$SCHEMA" \
  -o "$SCR/$RAW" \
  - < "$PROMPT" > "$TLOG" 2>&1
rc=$?
put "$LOG" "$TLOG"

[[ -e "$TMARK" ]] && fail "timeout after ${LIMIT}s (see target/scratch/$LOG)"
(( rc == 0 )) || fail "codex exit $rc (see target/scratch/$LOG)"
jq -e '(.verdict | type == "string") and (.findings | type == "array")' "$RAW" >/dev/null 2>&1 \
  || fail "output not schema JSON (see target/scratch/$LOG)"
T="$(mktemp .codex-review-tmp.out.XXXXXX)" && jq '. + {status:"ok",reason:""}' "$RAW" > "$T" && put "$OUT" "$T" \
  || fail "write failed"
