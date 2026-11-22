#!/usr/bin/env bash
# Tests the pieces the proposal adds to codex-review.sh (#161). Each piece is pulled out of the
# patched script by its exact text (so a test can't drift from the diff), then exercised.
#   ops/proposals/codex-review-isolation/test-codex.sh <patched codex-review.sh>
# Run from the repository root; works in target/scratch/codex-isolation-test (git-ignored).
set -u
[[ $# -eq 1 && -f "$1" ]] || { echo "usage: $0 <path to patched codex-review.sh>" >&2; exit 2; }
REPO="$(pwd -P)"; S="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
[[ -f "$REPO/rust-toolchain.toml" ]] || { echo "run from the repository root" >&2; exit 2; }
W="$REPO/target/scratch/codex-isolation-test"
rm -rf "$W"; mkdir -p "$W"; cd "$W" || exit 1
pass=0; failn=0
ok() { if "$@"; then pass=$((pass+1)); else failn=$((failn+1)); echo "FAIL: $*"; fi; }
not() { ! "$@"; }
extract() { awk -v a="$1" -v b="$2" 'index($0,a)==1{on=1} on{print} on&&index($0,b)==1{exit}' "$S"; }

# 1. rmtree (AS.55): a 0444 file hard-linked from outside keeps its mode; read-only dirs go away.
eval "$(extract 'rmtree() {' '}')"
mkdir -p t/sub; echo x > outside; chmod 444 outside; ln outside t/sub/link; chmod 555 t/sub t
rmtree t
ok test ! -e t
ok test "$(stat -f %Lp outside)" = 444
# the old code widened it:
mkdir -p t2; ln outside t2/link; chmod -R u+w t2; rm -rf t2
ok test "$(stat -f %Lp outside)" = 644   # demonstrates the AS.55 bug on the old pass

# 2. q / FS_POLICY: valid TOML inline table, paths with quotes and backslashes survive.
eval "$(extract 'q() {' 'q() {')"
HOME_SAVE="$HOME"; HOME='/Users/a "b"\c'
eval "$(extract 'FS_POLICY="{' 'FS_POLICY+=" }"')"
HOME="$HOME_SAVE"
ok /opt/homebrew/bin/python3.14 -I -c '
import sys, tomllib
t = tomllib.loads("x = " + sys.argv[1])["x"]
assert t[":root"] == "deny" and t[":minimal"] == "read" and t[":slash_tmp"] == "deny", t
assert t["/Users/a \"b\"\\c/.cargo/registry"] == "read", t
assert t["/opt/homebrew"] == "read" and len(t) == 8, t
assert not any("credentials" in k or ".codex" in k or ".ssh" in k for k in t), t
' "$FS_POLICY"

# 3. toolchain file check (CX.3).
tc_ok() { SRC="$1"; ( fail() { exit 1; }; eval "$(extract '[[ ! -e "$SRC/rust-toolchain" ]]' 'fi')" ) 2>/dev/null; }
mkdir -p r1 r2 r3 r4 r5 r6
printf '[toolchain]\nchannel = "1.100.0"\ncomponents = ["rustfmt", "clippy"]\n' > r1/rust-toolchain.toml
printf '[toolchain]\npath = "/x/y"\n' > r2/rust-toolchain.toml
printf '[toolchain]\nchannel = "nightly"\n' > r3/rust-toolchain.toml
printf '[toolchain]\nchannel = "1.100.0"\r\n' > r4/rust-toolchain.toml
printf 'x' > r5/rust-toolchain
printf '[toolchain]\nchannel = "1.100.0"\ntargets = ["wasm32-unknown-unknown"]\nprofile = "minimal"\n# c\n\n' > r6/rust-toolchain.toml
ok tc_ok r1; ok not tc_ok r2; ok not tc_ok r3; ok not tc_ok r4; ok not tc_ok r5; ok tc_ok r6
ok tc_ok "$W"   # no toolchain file at all
ok tc_ok "$REPO"   # the repo's real rust-toolchain.toml

# 4. symlink pre-check (CX.2): a repo with a symlinked directory is refused before extraction.
pre() { ( XGIT=(git); TOP="$1"; fail() { exit 1; }; eval "$(extract '"${XGIT[@]}" -C "$TOP" ls-tree' '  || fail "HEAD contains')" ); }
g() { git -C "$1" -c user.name=x -c user.email=x@x "${@:2}"; }
mkdir -p plain linked; g plain init -q; echo a > plain/a; g plain add -A; g plain commit -qm x
g linked init -q; mkdir linked/ops; ln -s ../../elsewhere linked/ops/judge; echo a > linked/a
g linked add -A; g linked commit -qm x
mkdir empty; g empty init -q
ok pre plain; ok not pre linked; ok not pre empty

echo "pass=$pass fail=$failn"; (( failn == 0 ))
