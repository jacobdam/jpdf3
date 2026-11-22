#!/usr/bin/env bash
# PROTECTED (CONSTITUTION 12). Second-opinion review by Codex (ops/proposals/codex-second-reviewer.md).
#   ops/driver/codex-review.sh target/scratch/<brief>.md target/scratch/<out>.json
# Run from a worktree of this repository. Both files must sit directly in that worktree's
# target/scratch/ (plain names, no symlinks or hard links), and every write goes through a rename
# inside that directory, so bad arguments can't make it read or write anything else. (A same-user
# process that deliberately races the checks is out of scope: it can write files directly anyway.)
# Codex reviews a plain export of HEAD (`git archive`, checked byte for byte, in its own fresh git
# repo, no symlinks, sealed corpus manifest left out) under a permission profile that lets its
# commands read only the export and the toolchain and write only the export, with the network,
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
# rmtree <path>: delete a tree Codex could write to. Only directories get u+w (a directory can't be
# hard-linked); files keep their mode and flags: chmod or chflags on a file Codex hard-linked in from
# elsewhere on the volume would change that outside file (AS.55). Unlinking needs no file permission.
# A file Codex made immutable (uchg) stays behind, and the run says so.
rmtree() {
  [[ -n "$1" && ( -e "$1" || -L "$1" ) ]] || return 0
  find "$1" -type d ! -perm -u+w -exec chmod u+w {} + 2>/dev/null
  rm -rf -- "$1" 2>/dev/null || echo "codex-review: could not remove all of target/scratch/$1" >&2
}
# NUL-separated, so a name with spaces or glob characters stays one path (no word splitting).
while IFS= read -r -d '' d; do rmtree "$d"; done \
  < <(find . -maxdepth 1 -name '.codex-review-tmp.*' -mmin +60 -print0)

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
  [[ -n "$SRC" ]] && rmtree "$SRC"
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
# A committed symlink could point Codex's reads, or a path operation of this script, outside the
# export (CX.2: a symlinked ancestor directory of the excluded manifest). Refuse it before anything
# is extracted, so no path below $SRC is touched while a link could be on it.
"${XGIT[@]}" -C "$TOP" ls-tree -r -z --full-tree HEAD | /usr/bin/python3 -I -c '
import sys
recs = [r for r in sys.stdin.buffer.read().split(b"\0") if r]
sys.exit(1 if not recs or any(r.split(b" ", 1)[0] == b"120000" for r in recs) else 0)' \
  || fail "HEAD contains symlinks (or ls-tree failed)"
"${XGIT[@]}" -C "$TOP" archive --format=tar HEAD | tar -x -C "$SRC" --exclude 'ops/judge/corpus/exam.tsv' \
  || fail "export failed"
# No rm of the excluded path here: --exclude keeps it out, and the byte check below fails if it is
# there. Belt and braces for the check above: no symlink may exist in the export.
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
# This runs with the driver's permissions, so nothing the reviewed branch or a session controls may
# configure it (CX.3):
# - cargo reads .cargo/config.toml from the export and from every directory above it. .cargo/ in
#   HEAD must be origin/main's tree (it is protected, CONSTITUTION 12); above the export, a .cargo
#   may exist only in this worktree (it is HEAD's) and the main checkout (protected, synced to main
#   by the driver), both clean (no local changes, untracked or ignored files), and in $HOME (cargo's
#   own home). Any other one (target/,
#   target/scratch/, .claude/worktrees/, which sessions can write) fails the run.
# - rustup takes the toolchain from the first toolchain file it finds upward, and a `path` entry
#   there would run a toolchain binary of anyone's choice. The export must have a rust-toolchain.toml
#   that pins only an exact channel, components, targets and profile, and RUSTUP_TOOLCHAIN (which
#   wins over every toolchain file and directory override) is set to that channel.
# Not covered: a session that moves the local ref refs/remotes/origin/main (git update-ref) can make
# a changed .cargo/ look like main's; and ~/.cargo (its config.toml, the cargo/rustup proxies in
# ~/.cargo/bin on PATH) is writable by sessions, as it was before this check. Both need a deliberately
# hostile local process (see the top); the guard doesn't protect ~/.cargo.
XG=("${XGIT[@]}" -C "$TOP")
[[ "$("${XG[@]}" rev-parse -q --verify HEAD:.cargo 2>/dev/null)" == \
   "$("${XG[@]}" rev-parse -q --verify refs/remotes/origin/main:.cargo 2>/dev/null)" ]] \
  || fail ".cargo/ differs from origin/main"
[[ -z "$("${XG[@]}" status --porcelain --ignored --untracked-files=all -- .cargo 2>&1)" ]] \
  || fail "this worktree's .cargo/ has local changes"
MAIN="$(common "$TOP")"; [[ "${MAIN##*/}" == .git ]] && MAIN="${MAIN%/*}" || fail "main checkout not found"
[[ -z "$("${XGIT[@]}" -C "$MAIN" status --porcelain --ignored --untracked-files=all -- .cargo 2>&1)" ]] \
  || fail "the main checkout's .cargo/ has local changes"
HOMEP="$(cd "$HOME" && pwd -P)" || fail "HOME"
d="$SCR"
while :; do
  if [[ -e "$d/.cargo" || -L "$d/.cargo" ]]; then
    [[ "$d" == "$TOP" || "$d" == "$MAIN" || "$d" == "$HOMEP" ]] || fail "a .cargo in ${d#"$TOP/"} (above the export)"
  fi
  [[ "$d" == / ]] && break
  d="$(dirname -- "$d")"
done
[[ ! -e "$SRC/rust-toolchain" && ! -L "$SRC/rust-toolchain" ]] || fail "legacy rust-toolchain file in HEAD"
[[ -f "$SRC/rust-toolchain.toml" ]] || fail "no rust-toolchain.toml in HEAD"
# grep -v exits 1 only when every line matches; 0 (a line that doesn't) and 2 (an error) both fail.
grep -qvE \
  '^(\[toolchain\]|channel = "[0-9]+\.[0-9]+\.[0-9]+"|(components|targets) = \[( *"[a-z0-9_-]+",?)* *\]|profile = "(minimal|default|complete)"|#.*)?$' \
  "$SRC/rust-toolchain.toml"
(( $? == 1 )) || fail "rust-toolchain.toml has entries beyond channel/components/targets/profile"
CHANNEL="$(sed -nE 's/^channel = "([0-9]+\.[0-9]+\.[0-9]+)"$/\1/p' "$SRC/rust-toolchain.toml")"
[[ "$CHANNEL" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "rust-toolchain.toml needs exactly one exact channel"
BASE_ENV+=(RUSTUP_TOOLCHAIN="$CHANNEL")
run_limited "$FETCH_LIMIT" "${BASE_ENV[@]}" /bin/sh -c 'cd "$1" && exec cargo fetch --locked -q' _ "$SCR/$SRC" \
  > /dev/null 2>&1 < /dev/null
rc=$?
[[ -e "$TMARK" ]] && fail "cargo fetch timeout after ${FETCH_LIMIT}s"
(( rc == 0 )) || fail "cargo fetch failed ($rc)"

# Codex's commands may read only the export (its workspace, writable), the platform paths of
# `:minimal`, and the read-only toolchain: rustup toolchains, cargo's binaries and crate cache, and
# Homebrew. Everything else is denied by `:root`, so the sealed manifest in the main checkout,
# ~/.codex (auth), ~/.ssh, ~/.cargo/credentials.toml, ~/jpdf-corpus and ~/jpdf-research are
# unreadable (AS.52, CX.1; CONSTITUTION 1.2, 13). `--sandbox workspace-write` limited writes and
# network only. A permission profile replaces the legacy sandbox flags (Codex: "either
# default_permissions and [permissions], or sandbox_mode / sandbox_workspace_write, but not both"),
# so none of them may appear here. Profiles are Beta in Codex; --strict-config makes an unknown key a
# failed run ("unavailable"), never a silently wider sandbox.
q() { local s="${1//\\/\\\\}"; printf '"%s"' "${s//\"/\\\"}"; }   # TOML basic string
FS_POLICY="{ \":root\" = \"deny\", \":minimal\" = \"read\", \":slash_tmp\" = \"deny\""
for r in "$HOME/.rustup/toolchains" "$HOME/.rustup/settings.toml" "$HOME/.cargo/bin" \
         "$HOME/.cargo/registry" /opt/homebrew; do
  FS_POLICY+=", $(q "$r") = \"read\""
done
FS_POLICY+=" }"
RAW="$(mktemp .codex-review-tmp.out.XXXXXX)" || fail "mktemp"
TLOG="$(mktemp .codex-review-tmp.log.XXXXXX)" || fail "mktemp"
run_limited "$LIMIT" "${BASE_ENV[@]}" CARGO_NET_OFFLINE=true CMUX_COMPUTER_USE_MCP_DISABLED=1 \
  "$CODEX" exec \
  --ignore-user-config --ignore-rules --ephemeral --strict-config \
  -m gpt-6.2-sol -c model_reasoning_effort=high \
  --disable apps --disable plugins --disable shell_snapshot --disable hooks \
  --disable computer_use --disable browser_use --disable browser_use_external \
  --disable in_app_browser --disable image_generation \
  -c default_permissions='"jpdf-review"' \
  -c permissions.jpdf-review.extends='":workspace"' \
  -c permissions.jpdf-review.filesystem="$FS_POLICY" \
  -c permissions.jpdf-review.network.enabled=false \
  -c web_search=disabled \
  -C "$SCR/$SRC" \
  --output-schema "$SCHEMA" \
  -o "$SCR/$RAW" \
  - < "$PROMPT" > "$TLOG" 2>&1
rc=$?
put "$LOG" "$TLOG"

[[ -e "$TMARK" ]] && fail "timeout after ${LIMIT}s (see target/scratch/$LOG)"
(( rc == 0 )) || fail "codex exit $rc (see target/scratch/$LOG)"
# The schema's shape, checked here too (CX.16): Codex enforcing --output-schema is not ours to rely on.
# -s: the file must hold exactly one JSON value (jq -e alone judges only the last of several).
OUT_CHECK='length == 1 and (.[0] | type == "object" and (keys == ["findings", "verdict"])
  and (.verdict | type == "string") and (.findings | type == "array") and all(.findings[];
    type == "object" and (keys == ["detail", "file", "fix", "line", "severity", "title"])
    and (.severity | IN("blocker", "major", "minor", "nit"))
    and (.line == null or (.line | type == "number" and . == floor))
    and ([.file, .title, .detail, .fix] | all(type == "string"))))'
jq -s -e "$OUT_CHECK" "$RAW" >/dev/null 2>&1 || fail "output not schema JSON (see target/scratch/$LOG)"
T="$(mktemp .codex-review-tmp.out.XXXXXX)" && jq -s '.[0] + {status:"ok",reason:""}' "$RAW" > "$T" && put "$OUT" "$T" \
  || fail "write failed"
