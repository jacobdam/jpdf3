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
# extract <first line prefix> <last line prefix>: the lines from the first to the last, inclusive.
# Both markers must be found: an end marker that a later change lost would otherwise eval the rest
# of the file (as codex-homebrew-read/test-policy.sh's R1.3). awk exits 3 at the end of the file
# without the end marker.
extract() {
  local t; t="$(awk -v a="$1" -v b="$2" 'index($0,a)==1{on=1} on{print} on&&index($0,b)==1{done=1; exit}
    END{exit done?0:3}' "$S")" || { echo "markers not found in $S: $1 ... $2" >&2; exit 2; }
  [[ -n "$t" ]] || { echo "not found in $S: $1" >&2; exit 2; }
  printf '%s\n' "$t"
}

# 1. rmtree (AS.55): a 0444 file hard-linked from outside keeps its mode; read-only dirs go away.
eval "$(extract 'rmtree() {' '}')"
mkdir -p t/sub; echo x > outside; chmod 444 outside; ln outside t/sub/link; chmod 555 t/sub t
rmtree t
ok test ! -e t
ok test "$(stat -f %Lp outside)" = 444
mkdir -p t2; ln outside t2/link; chmod -R u+w t2; rm -rf t2
ok test "$(stat -f %Lp outside)" = 644   # the old pass on the same layout: the AS.55 bug

# 2. The leftover sweep (R1.1): only old .codex-review-tmp.* entries go, whatever their names.
SWEEP="$(extract 'while IFS= read -r -d '"''"' d; do rmtree "$d"; done' '  < <(find . -maxdepth 1')"
mkdir sweep && (
  cd sweep || exit 1
  mkdir victim keepdir '.codex-review-tmp.x *' '.codex-review-tmp.x victim' .codex-review-tmp.new
  echo b > brief.md; echo o > other.json
  touch -t 202001010000 '.codex-review-tmp.x *' '.codex-review-tmp.x victim' victim brief.md
  eval "$SWEEP"
  [[ -e victim && -e keepdir && -e brief.md && -e other.json && -e .codex-review-tmp.new \
     && ! -e '.codex-review-tmp.x *' && ! -e '.codex-review-tmp.x victim' ]]
); ok test $? -eq 0

# 3. q / FS_POLICY: valid TOML inline table, paths with quotes and backslashes survive.
# One contiguous block from q() to the end of FS_POLICY (q, HB, HB_READ and the loop, since
# codex-homebrew-read, PR #291), built in a subshell with that HOME. Extract first, then eval: a
# lost marker stops the run here instead of evaluating the rest of the file.
POLICY_BLOCK="$(extract 'q() {' 'FS_POLICY+=" }"')" || exit 2
FS_POLICY="$(HOME='/Users/a "b"\c'; eval "$POLICY_BLOCK"; printf '%s' "$FS_POLICY")"
# The exact Homebrew grant list is codex-homebrew-read/test-policy.sh's case; this one checks the
# quoting: all four $HOME grants under the quoted HOME, the rest under the Homebrew prefix by name.
ok /opt/homebrew/bin/python3.14 -I -c '
import sys, tomllib
t = tomllib.loads("x = " + sys.argv[1])["x"]
h = "/Users/a \"b\"\\c/"
assert t[":root"] == "deny" and t[":minimal"] == "read" and t[":slash_tmp"] == "deny", t
home = {h + p for p in (".rustup/toolchains", ".rustup/settings.toml", ".cargo/bin", ".cargo/registry")}
assert all(t.get(k) == "read" for k in home), t
assert all(v == "read" and k.startswith("/opt/homebrew/") for k, v in t.items()
           if not k.startswith(":") and k not in home), t
assert "/opt/homebrew" not in t and len(t) == 3 + len(home) + 9, t
assert not any("credentials" in k or ".codex" in k or ".ssh" in k for k in t), t
' "$FS_POLICY"

# 4. Toolchain file (CX.3, R1.2): required, strict, and its channel goes into RUSTUP_TOOLCHAIN.
TC="$(extract '[[ ! -e "$SRC/rust-toolchain" && ! -L' 'BASE_ENV+=(RUSTUP_TOOLCHAIN')"
tc() { ( SRC="$1"; BASE_ENV=(); fail() { exit 1; }; eval "$TC"; printf '%s\n' "${BASE_ENV[*]}" ) 2>/dev/null; }
tc_ok() { [[ "$(tc "$1")" == "RUSTUP_TOOLCHAIN=$2" ]]; }
mkdir -p r1 r2 r3 r4 r5 r6 r7 r8 r9
printf '[toolchain]\nchannel = "1.100.0"\ncomponents = ["rustfmt", "clippy"]\n' > r1/rust-toolchain.toml
printf '[toolchain]\npath = "/x/y"\n' > r2/rust-toolchain.toml
printf '[toolchain]\nchannel = "nightly"\n' > r3/rust-toolchain.toml
printf '[toolchain]\nchannel = "1.100.0"\r\n' > r4/rust-toolchain.toml
cp r1/rust-toolchain.toml r5/; printf 'x' > r5/rust-toolchain
printf '[toolchain]\nchannel = "1.99.0"\ntargets = ["wasm32-unknown-unknown"]\nprofile = "minimal"\n# c\n\n' > r6/rust-toolchain.toml
printf '[toolchain]\nchannel = "1.100.0"\nchannel = "1.99.0"\n' > r7/rust-toolchain.toml
printf '[toolchain]\ncomponents = ["rustfmt"]\n' > r8/rust-toolchain.toml
ok tc_ok r1 1.100.0; ok not tc_ok r2 ""; ok not tc_ok r3 ""; ok not tc_ok r4 1.100.0
ok not tc_ok r5 1.100.0; ok tc_ok r6 1.99.0
ok not tc_ok r7 1.100.0; ok not tc_ok r8 ""   # two channels; no channel
ok not tc_ok r9 ""                            # no toolchain file at all (R1.2)
ok tc_ok "$REPO" "$(sed -nE 's/^channel = "(.*)"$/\1/p' "$REPO/rust-toolchain.toml")"   # the real file

# 5. .cargo above the export (CX.3, R1.3): only the worktree, the main checkout and $HOME.
WALK="$(extract 'MAIN="$(common "$TOP")"' 'done')"
walk() {  # walk <dir with a .cargo, or ""> [file to create in it]: 0 if the walk accepts the layout
  # The layout sits inside this repo, whose real ancestors have .cargo directories, so above $L the
  # walk jumps straight to / (which has none).
  ( fail() { exit 1; }; common() { printf '%s\n' "$L/main/.git"; }
    dirname() { if [[ "$2" == "$L" ]]; then echo /; else command dirname "$@"; fi; }
    [[ ! -e /.cargo ]] || exit 3
    rm -rf "$L"; mkdir -p "$L/home" "$L/main/.claude/worktrees/wt/target/scratch"; git init -q "$L/main"
    TOP="$L/main/.claude/worktrees/wt"; SCR="$TOP/target/scratch"; HOME="$L/home"; XGIT=(git)
    [[ -n "$1" ]] && mkdir "$1/.cargo"
    [[ -n "${2:-}" ]] && echo x > "$1/.cargo/$2"
    eval "$WALK" ) >/dev/null 2>&1
}
L="$W/layout"
ok walk ""
ok walk "$L/main/.claude/worktrees/wt"; ok walk "$L/main"; ok walk "$L/home"
ok not walk "$L/main" config.toml   # an untracked config in the main checkout's .cargo (R2.2)
ok not walk "$L/main/.claude/worktrees"; ok not walk "$L/main/.claude"
ok not walk "$L/main/.claude/worktrees/wt/target"; ok not walk "$L/main/.claude/worktrees/wt/target/scratch"
ok not walk "$L"

# 6. symlink pre-check (CX.2): a repo with a symlinked directory is refused before extraction.
pre() { ( XGIT=(git); TOP="$1"; fail() { exit 1; }; eval "$(extract '"${XGIT[@]}" -C "$TOP" ls-tree' '  || fail "HEAD contains')" ) 2>/dev/null; }
g() { git -C "$1" -c user.name=x -c user.email=x@x "${@:2}"; }
mkdir -p plain linked; g plain init -q; echo a > plain/a; g plain add -A; g plain commit -qm x
g linked init -q; mkdir linked/ops; ln -s ../../elsewhere linked/ops/judge; echo a > linked/a
g linked add -A; g linked commit -qm x
mkdir empty; g empty init -q
ok pre plain; ok not pre linked; ok not pre empty

# 7. Output check (CX.16, R1.4): exactly one JSON value of the schema's shape.
eval "$(extract "OUT_CHECK='" "    and ([.file, .title, .detail, .fix]")"
oc() { printf '%s' "$1" | jq -s -e "$OUT_CHECK" >/dev/null 2>&1; }
f='{"severity":"major","file":"a","line":3,"title":"t","detail":"d","fix":"f"}'
ok oc '{"verdict":"v","findings":[]}'
ok oc "{\"verdict\":\"v\",\"findings\":[$f]}"
ok oc "{\"verdict\":\"v\",\"findings\":[${f/3/null}]}"
for bad in '{"verdict":"v","findings":[null]}' '{"verdict":"v","findings":[{}]}' \
    "{\"verdict\":\"v\",\"findings\":[${f/major/critical}]}" \
    "{\"verdict\":\"v\",\"findings\":[${f/,\"fix\":\"f\"/}]}" \
    "{\"verdict\":\"v\",\"findings\":[${f/3/3.5}]}" \
    "{\"verdict\":\"v\",\"findings\":[${f/\"d\"/7}]}" \
    '{"verdict":"v","findings":[],"x":1}' '[]' '{"verdict":1,"findings":[]}' '' 'not json' \
    '{"verdict":"v","findings":[null],"x":1} {"verdict":"v","findings":[]}' \
    '{"verdict":"v","findings":[]} {"verdict":"v","findings":[]}'; do
  ok not oc "$bad"
done
# and what gets written is that one object, not a stream
WRITE="$(extract 'T="$(mktemp .codex-review-tmp.out.XXXXXX)" && jq -s' 'T="$(mktemp')"
[[ "$WRITE" == *"jq -s '.[0] + {status:\"ok\",reason:\"\"}'"* ]]; ok test $? -eq 0

echo "pass=$pass fail=$failn"; (( failn == 0 ))
