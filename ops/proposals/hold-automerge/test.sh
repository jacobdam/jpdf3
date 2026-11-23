#!/usr/bin/env bash
# Harness for ops/proposals/hold-automerge.md (#210). Runs the `run: |` block of an
# owner-commands workflow file against a fake `gh` that keeps the PR's state (draft, auto-merge)
# in files and can be told to fail each call. Usage:
#   ops/proposals/hold-automerge/test.sh [workflow.yml] [scratch dir]
# Defaults: the proposed workflow next to this script, and <repo>/target/scratch.
# Exits 0 only if every case passes. Against today's .github/workflows/owner-commands.yml it must
# fail (cases hold-disable-fails and others), which shows the harness sees the #210 bug.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
wf=${1:-$here/owner-commands.yml}
root=$(git -C "$here" rev-parse --show-toplevel)
scratch=$(mktemp -d "${2:-$root/target/scratch}/hold-automerge.XXXXXX")

# The step's script: every line after `run: |`, with the block's 10-space indent removed.
awk 'f { sub(/^          /, ""); print } /^        run: \|$/ { f = 1 }' "$wf" >"$scratch/step.sh"
grep -q 'gh pr comment' "$scratch/step.sh" || { echo "no run block found in $wf" >&2; exit 2; }

mkdir -p "$scratch/bin"
cat >"$scratch/bin/gh" <<'FAKE'
#!/usr/bin/env bash
# Fake gh: state in $S/draft and $S/auto; a file $S/fail_<name> makes that call exit 1.
echo "$*" >>"$S/calls"
fail() { [[ -e "$S/fail_$1" ]]; }
case "$1 $2" in
  "pr view")
    fail view && exit 1
    if [[ -e "$S/fail_view_once" ]]; then rm "$S/fail_view_once"; exit 1; fi
    if [[ -e "$S/garbage_view" ]]; then echo "null"; exit 0; fi
    echo "$(cat "$S/draft") $(cat "$S/auto")" ;;
  "pr merge")
    [[ "$4" == --disable-auto ]] || exit 9
    fail disable && { echo "GraphQL: Resource not accessible by integration (disablePullRequestAutoMerge)" >&2; exit 1; }
    echo false >"$S/auto" ;;
  "pr ready")
    if [[ "${4:-}" == --undo ]]; then
      fail undo && exit 1
      echo true >"$S/draft"
    else
      fail ready && exit 1
      echo false >"$S/draft"
    fi ;;
  "pr comment")
    printf '%s\n' "$5" >>"$S/comments" ;;
  *) exit 9 ;;
esac
FAKE
chmod +x "$scratch/bin/gh"

failures=0
# case NAME BODY DRAFT AUTO "FLAGS" WANT_RC WANT_COMMENT_PREFIX WANT_DRAFT WANT_AUTO
case_() {
  local name=$1 body=$2 draft=$3 auto=$4 flags=$5 want_rc=$6 want=$7 want_draft=$8 want_auto=$9
  local S=$scratch/$name rc=0
  mkdir -p "$S"
  echo "$draft" >"$S/draft"
  echo "$auto" >"$S/auto"
  : >"$S/comments"
  for f in $flags; do : >"$S/$f"; done
  S=$S PR=7 BODY=$body RUN_URL=https://example.invalid/run PATH="$scratch/bin:$PATH" \
    bash --noprofile --norc -eo pipefail "$scratch/step.sh" >"$S/out" 2>&1 || rc=$?
  local got
  got=$(head -n 1 "$S/comments")
  local ok=1
  [[ "$rc" == "$want_rc" ]] || ok=0
  [[ "$(wc -l <"$S/comments" | tr -d ' ')" == 1 ]] || ok=0
  [[ "$got" == "🤖 jpdf-agent $want"* ]] || ok=0
  [[ "$(cat "$S/draft")" == "$want_draft" && "$(cat "$S/auto")" == "$want_auto" ]] || ok=0
  if ((ok)); then
    echo "ok   $name"
  else
    echo "FAIL $name: rc=$rc (want $want_rc), comment='$got' (want '$want'), draft=$(cat "$S/draft") auto=$(cat "$S/auto")"
    failures=$((failures + 1))
  fi
}

#     name                  body       draft auto  flags            rc  comment             draft auto
case_ hold-ready-auto       /hold      false true  ""               0   "/hold applied"     true  false
case_ hold-disable-fails    /hold      false true  fail_disable     1   "/hold FAILED"      true  true
case_ hold-undo-fails       /hold      false true  fail_undo        1   "/hold FAILED"      false false
case_ hold-already-held     /hold      true  false ""               0   "/hold applied"     true  false
case_ hold-draft-auto-on    /hold      true  true  ""               0   "/hold applied"     true  false
case_ hold-first-view-fails /hold  false true  fail_view_once   0   "/hold applied"     true  false
case_ hold-view-fails       /hold      false false fail_view        1   "/hold FAILED"      true  false
case_ hold-view-garbage     /hold      true  false garbage_view     1   "/hold FAILED"      true  false
case_ hold-with-text        "/hold pls" false true ""               0   "/hold applied"     true  false
case_ release-draft         /release   true  false ""               0   "/release applied"  false false
case_ release-ready-fails   /release   true  false fail_ready       1   "/release FAILED"   true  false
case_ release-view-fails    /release   true  false fail_view        1   "/release FAILED"   false false
case_ release-already-ready /release   false false ""               0   "/release applied"  false false

# An unreadable state is reported as unknown, not as an empty or guessed value.
if ! grep -qF 'draft=unknown, auto-merge on=unknown' "$scratch/hold-view-fails/comments"; then
  echo "FAIL hold-view-fails: comment lacks the unknown state: $(cat "$scratch/hold-view-fails/comments")"
  failures=$((failures + 1))
fi

# A hold on a PR already held must not call the mutations (they fail or warn on a no-op).
if grep -qE '^pr (merge|ready)' "$scratch/hold-already-held/calls"; then
  echo "FAIL hold-already-held: called $(grep -E '^pr (merge|ready)' "$scratch/hold-already-held/calls" | tr '\n' ';')"
  failures=$((failures + 1))
fi

echo "$failures failure(s); scratch: $scratch"
((failures == 0))
