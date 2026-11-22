#!/usr/bin/env bash
# Tests the read policy the proposal gives Codex's commands (#191, audit 0004 AS.2). The block from
# `q()` to `FS_POLICY+=" }"` (HB, HB_READ and FS_POLICY) is pulled out of a codex-review.sh by its
# exact text (so the test can't drift from the diff) and built with a fake HOME.
#   ops/proposals/codex-homebrew-read/test-policy.sh <patched codex-review.sh> [<unpatched one>]
# With the second argument it also checks that the unpatched script fails the "no grant covers a
# denied Homebrew directory" check (the AS.2 bug), so the check can tell the two apart.
# Static only: Seatbelt can't be nested inside a sandboxed session, so whether Codex enforces the
# policy is the owner's test plan (T5 and T6 in ../codex-homebrew-read.md).
set -u
[[ $# -ge 1 && -f "$1" ]] || { echo "usage: $0 <patched codex-review.sh> [<unpatched>]" >&2; exit 2; }
PY=/opt/homebrew/bin/python3.14   # tomllib (3.11+); /usr/bin/python3 is 3.9
pass=0; failn=0
ok() { if "$@"; then pass=$((pass+1)); else failn=$((failn+1)); echo "FAIL: $*"; fi; }
rc_is() { local want="$1"; shift; "$@"; [[ $? -eq "$want" ]]; }
extract() {   # extract <file> <first line prefix> <last line prefix>: the lines between, inclusive
  # Both markers must be found: an end marker that a reflow lost would otherwise eval the rest of
  # the file (R1.3). awk exits 3 when it reaches the end without the end marker.
  local t; t="$(awk -v a="$2" -v b="$3" 'index($0,a)==1{on=1} on{print} on&&index($0,b)==1{done=1; exit}
    END{exit done?0:3}' "$1")" || { echo "markers not found in $1: $2 ... $3" >&2; exit 2; }
  [[ -n "$t" ]] || { echo "not found in $1: $2" >&2; exit 2; }
  printf '%s\n' "$t"
}
policy() {   # policy <script>: prints the FS_POLICY it builds, with HOME=/Users/u
  local s="$1"
  ( HOME=/Users/u
    # One contiguous block from q() to the end of FS_POLICY, so no line between the pieces escapes
    # the test (R1.2). Extract first, then eval: a failed extract stops here.
    local b; b="$(extract "$s" 'q() {' 'FS_POLICY+=" }"')" || exit 2
    eval "$b"
    printf '%s' "$FS_POLICY" )
}

# check <policy> <mode>: mode "strict" = the full patched-policy assertions; "denied" = only that no
# grant covers a directory that must stay denied (the property the AS.2 bug breaks). Exit 0 = holds,
# 4 = an assertion failed, anything else = the check itself broke (so "expected to fail" can't pass
# on a crash).
check() {
  "$PY" -I -c '
import sys, tomllib
t = tomllib.loads("x = " + sys.argv[1])["x"]
mode = sys.argv[2]
def fail(*a):
    print("assertion:", *a, file=sys.stderr); sys.exit(4)
def covers(grant, path):          # a read grant on `grant` includes `path`
    return path == grant or path.startswith(grant.rstrip("/") + "/")
must_deny = ["/opt/homebrew/etc", "/opt/homebrew/etc/gnupg", "/opt/homebrew/var",
             "/opt/homebrew/var/log", "/opt/homebrew/Library", "/opt/homebrew/.git",
             "/opt/homebrew/.codex", "/opt/homebrew/Caskroom", "/opt/homebrew/Caskroom/claude-code@latest",
             "/Users/u/.codex", "/Users/u/.ssh", "/Users/u/.cargo/credentials.toml",
             "/Users/u/jpdf-corpus", "/Users/u/jpdf-research", "/Users/u"]
reads = [k for k, v in t.items() if v == "read" and not k.startswith(":")]
bad = [(g, p) for g in reads for p in must_deny if covers(g, p)]
if bad: fail("grant covers a denied path", bad)
if mode == "strict":
    if not (t[":root"] == "deny" and t[":minimal"] == "read" and t[":slash_tmp"] == "deny"): fail(t)
    if not set(t.values()) <= {"read", "deny"}: fail(t)
    want_hb = {"/opt/homebrew/" + d for d in
               ["bin", "Cellar", "opt", "lib", "Frameworks", "share", "libexec", "include",
                "Caskroom/codex"]}
    want = want_hb | {"/Users/u/.rustup/toolchains", "/Users/u/.rustup/settings.toml",
                      "/Users/u/.cargo/bin", "/Users/u/.cargo/registry"}
    if set(reads) != want: fail(sorted(set(reads) ^ want))
    if "/opt/homebrew" in t or len(t) != 3 + len(want): fail(t)
' "$1" "$2"
}

P="$(policy "$1")"
ok check "$P" strict
if [[ $# -ge 2 ]]; then
  O="$(policy "$2")"
  ok rc_is 4 check "$O" denied 2>/dev/null            # the AS.2 bug: /opt/homebrew covers etc/ and var/
fi

# The grant list against this machine: every PATH tool in /opt/homebrew/bin that resolves inside
# /opt/homebrew must resolve into a granted directory (else the review loses a tool it had), and
# nothing granted may resolve into etc/ or var/. Links into /Applications were never readable to
# Codex's commands, and links into other casks are dropped on purpose: both are counted, not failed.
if [[ -d /opt/homebrew/bin ]]; then
  "$PY" -I -c '
import os, sys, tomllib
t = tomllib.loads("x = " + sys.argv[1])["x"]
grants = [os.path.realpath(k) for k, v in t.items() if v == "read" and k.startswith("/opt/homebrew/")]
def covered(p): return any(p == g or p.startswith(g + "/") for g in grants)
inside, outside, lost, casks = 0, 0, [], []
for n in sorted(os.listdir("/opt/homebrew/bin")):
    r = os.path.realpath(os.path.join("/opt/homebrew/bin", n))
    if not r.startswith("/opt/homebrew/"): outside += 1; continue
    inside += 1
    if covered(r): continue
    # Other casks (claude-code, gcloud, ...) are dropped on purpose: a review needs none of them.
    (casks if r.startswith("/opt/homebrew/Caskroom/") else lost).append(n)
for d in ("etc", "var"):
    for g in grants: assert not (g + "/").startswith("/opt/homebrew/" + d + "/"), g
print(f"info: /opt/homebrew/bin: {inside} tools resolve inside the prefix, {outside} outside it; "
      f"{len(casks)} from other casks dropped on purpose {casks}; {len(lost)} others lost: {lost[:5]}")
sys.exit(1 if lost else 0)
' "$P"
  ok test $? -eq 0
fi

echo "test-policy: $pass passed, $failn failed"
(( failn == 0 ))
