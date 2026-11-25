#!/usr/bin/env bash
# Tests disk_ok from the proposed driver.sh against a fake `df` (no cleaning runs: `find`,
# `git` and `cargo` are stubbed to record their calls). Run from the repo root:
#   bash ops/proposals/driver-portable-df/test.sh
set -u
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
for c in find git cargo; do printf '#!/bin/sh\necho %s >> "%s/calls"\n' "$c" "$tmp" > "$tmp/bin/$c"; done
chmod +x "$tmp/bin/"*
# Load only the functions: everything before the main loop.
funcs="$(sed -n '1,/^backoff=/p' "$here/driver.sh" | sed '$d' | grep -v '^cd "\$REPO_DIR"' | grep -v '^mkdir -p ops')"
fail=0
check() { # name, fake df script, expected disk_ok status, expect cleaning (yes/no)
  printf '#!/bin/sh\n%s\n' "$2" > "$tmp/bin/df"; chmod +x "$tmp/bin/df"; rm -f "$tmp/calls"
  ( PATH="$tmp/bin:$PATH"; REPO_DIR="$tmp"; JPDF_MIN_FREE_GB=5
    eval "$funcs"; log() { :; }; disk_ok ); got=$?
  cleaned=no; [[ -f "$tmp/calls" ]] && cleaned=yes
  if [[ "$got" == "$3" && "$cleaned" == "$4" ]]; then echo "ok   $1"; else echo "FAIL $1: status $got (want $3), cleaned $cleaned (want $4)"; fail=1; fi
}
hdr='echo "Filesystem 1024-blocks Used Available Capacity Mounted"'
check "Linux 9 GB free, minimum 5"   "$hdr; echo '/dev/sda1 16000000 6000000 9437184 38% /'" 0 no
check "exactly 5 GB passes"           "$hdr; echo '/dev/sda1 16000000 6000000 5242880 38% /'" 0 no
check "one KB under 5 GB is low"      "$hdr; echo '/dev/sda1 16000000 6000000 5242879 38% /'" 1 yes
check "df fails: no cleaning"         "echo 'df: invalid option' >&2; exit 1" 1 no
check "df prints no number"           "$hdr; echo '/dev/sda1 - - - - /'" 1 no
exit $fail
