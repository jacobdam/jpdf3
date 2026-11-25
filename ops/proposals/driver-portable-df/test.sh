#!/usr/bin/env bash
# Tests disk_ok from a driver.sh (default: the proposed one next to this file) against a fake
# `df` that, like GNU df, rejects `-g` and anything but `-P -k <dir>`. `find`, `git` and
# `cargo` are stubbed to record their calls, and REPO_DIR is a temp dir, so nothing is cleaned.
#   bash ops/proposals/driver-portable-df/test.sh [driver.sh]
set -u
here="$(cd "$(dirname "$0")" && pwd)"
driver="${1:-$here/driver.sh}"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/repo"
for c in find git cargo; do printf '#!/bin/sh\necho %s >> "%s/calls"\n' "$c" "$tmp" > "$tmp/bin/$c"; done
chmod +x "$tmp/bin/"*
# Only the functions and settings: everything before the main loop, without the lines that
# set REPO_DIR, cd or create directories.
funcs="$(sed -n '1,/^backoff=/p' "$driver" | sed '$d' | grep -v -E '^(REPO_DIR=|cd "\$REPO_DIR"|mkdir -p ops)')"
hdr='Filesystem 1024-blocks Used Available Capacity Mounted on'
fail=0
# check NAME MIN AVAIL1 AVAIL2 WANT_STATUS WANT_CLEANED
#   MIN: JPDF_MIN_FREE_GB, or "unset"; AVAILn: column 4 of the n-th df call, or "fail" (df exits
#   1 with no output), or "-" (a line without a number).
check() {
  cat > "$tmp/bin/df" <<DF
#!/bin/sh
n=\$(cat "$tmp/dfcalls" 2>/dev/null || echo 0); n=\$((n + 1)); echo \$n > "$tmp/dfcalls"
[ "\$1" = "-P" ] && [ "\$2" = "-k" ] && [ "\$3" = "$tmp/repo" ] && [ \$# -eq 3 ] || { echo "df: invalid option" >&2; exit 1; }
if [ \$n -eq 1 ]; then a="$3"; else a="$4"; fi
[ "\$a" = fail ] && exit 1
echo "$hdr"; echo "/dev/sda1 16000000 6000000 \$a 38% /"
DF
  chmod +x "$tmp/bin/df"; rm -f "$tmp/calls" "$tmp/dfcalls" "$tmp/err"
  ( PATH="$tmp/bin:$PATH"
    if [[ "$2" == unset ]]; then unset JPDF_MIN_FREE_GB; else export JPDF_MIN_FREE_GB="$2"; fi
    eval "$funcs"; REPO_DIR="$tmp/repo"; log() { :; }; disk_ok ) 2> "$tmp/err"; got=$?
  cleaned=no; [[ -f "$tmp/calls" ]] && cleaned=yes
  if [[ "$got" == "$5" && "$cleaned" == "$6" && ! -s "$tmp/err" ]]; then echo "ok   $1"
  else echo "FAIL $1: status $got (want $5), cleaned $cleaned (want $6), stderr: $(head -c 200 "$tmp/err")"; fail=1; fi
}
G=1048576
check "9 GB free, minimum 5"                5     $((9*G))     -  0 no
check "exactly 5 GB passes"                 5     $((5*G))     -  0 no
check "1 KB under 5 GB is low, cleans"      5     $((5*G-1))   $((5*G-1)) 1 yes
check "default minimum is 5 GB (unset)"     unset $((5*G))     -  0 no
check "default: 1 KB under 5 GB is low"     unset $((5*G-1))   $((5*G-1)) 1 yes
check "cleaning frees enough"               5     $((4*G))     $((6*G)) 0 yes
check "df fails: no cleaning"               5     fail         -  1 no
check "df prints no number: no cleaning"    5     -            -  1 no
check "df fails after cleaning"             5     $((4*G))     fail 1 yes
exit $fail
