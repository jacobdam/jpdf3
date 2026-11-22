# Proposal: Codex's commands read only Homebrew's install trees, not all of /opt/homebrew (#191)

Status: proposed by a session for issue #191 (audit 0004 finding AS.2, sev:high), 2026-11-22. It
touches a protected path (`ops/driver/codex-review.sh`), so the owner applies it with signed
commits. Agents can't run Codex outside the launcher, and Seatbelt can't be nested inside a
sandboxed session (`sandbox-exec` aborts there), so whether Codex enforces the narrower list is
the owner's test plan (§4, T3 to T6). The static checks in §4 ran.

## 1. Problem

`ops/driver/codex-review.sh:237` (main @ 378c92c) grants Codex's permission profile read access to
the whole Homebrew prefix:

```sh
for r in "$HOME/.rustup/toolchains" "$HOME/.rustup/settings.toml" "$HOME/.cargo/bin" \
         "$HOME/.cargo/registry" /opt/homebrew; do
```

The prefix holds more than programs. On this machine:
- `/opt/homebrew/etc`: configs for gnupg, gnutls, openssl@3, openssl@4, pkcs11, dolt, vde2 and
  others;
- `/opt/homebrew/var`: `log` (service logs), `run`, `cache`, `homebrew` (locks, sandbox, tmp);
  formulas such as databases keep their data here;
- `/opt/homebrew/Library` and the prefix's git checkout and dotfiles (`.git`, `.codex`, `.claude`);
- `/opt/homebrew/Caskroom`: every cask, not only Codex.

None of them are tracked repo files. CONSTITUTION 1.2 lets only tracked files and the brief go to
model providers. Scenario (reasoned, not run): a reviewed branch plants text in a fixture asking
Codex to quote `/opt/homebrew/var/log/<service>.log` in a finding; the profile lets the read
happen, the content goes to OpenAI and the finding is posted on the public PR. The script header
("read only the export and the toolchain") and the comment above the list ("and Homebrew")
overstate the isolation.

## 2. Fix

Grant Homebrew by named subdirectory, never the prefix itself:

| Granted | Why the review needs it |
|---|---|
| `bin/`, `sbin/` | on the commands' `PATH` (`BASE_ENV`) |
| `Cellar/`, `opt/` | where those links resolve; programs load libraries by `opt/` install names |
| `lib/`, `Frameworks/` | shared libraries; Homebrew Python lives in `Frameworks/Python.framework` |
| `share/`, `libexec/`, `include/` | program data (terminfo, locale, helper scripts), headers |
| `Caskroom/codex/` | Codex's own cask: its bundled `rg` (`codex-path/rg`) |

Everything else in the prefix falls under `:root = deny`: `etc/`, `var/`, `Library/`, the dotfiles,
other casks, and any directory Homebrew adds later (an allow list fails closed). A symlink from a
granted directory into a denied one stays denied, because Seatbelt matches the resolved path (T6
checks this on the real Codex). Header and comment now say what is readable.

On this machine (`test-policy.sh`, informational part): of the 523 entries in `/opt/homebrew/bin`,
519 resolve inside the prefix; with the new list every one of them stays readable except 3 that
point into other casks (`claude`, `tuist`, `vlc`), which a review doesn't need. The other 4 point
into `/Applications`, which the profile never granted.

### What this doesn't cover (residual risk, stated)
- The granted trees are Homebrew-managed install trees. A formula that writes user data into
  `share/`, `lib/` or its `Cellar/` keg (rather than `var/` or `etc/`) would be readable. None was
  found here (no file in `share/` or `lib/` is newer than the Codex install, depth 2), but that is
  a spot check, not a rule.
- Programs that read their config from `etc/` (openssl's default `cert.pem`, gnupg) lose it. The
  review runs offline, so certificates aren't needed; a tool that refuses to start without its
  config shows up in T4.
- `:minimal` is still not documented path by path; T5 checks that it doesn't re-grant the prefix.
- `ops/proposals/codex-review-isolation/test-codex.sh` test 3 (from the applied #161 proposal)
  asserts the old list (`/opt/homebrew` and 8 entries). It is a historical record, not run by CI;
  after this change it fails by design, and `codex-homebrew-read/test-policy.sh` replaces it.

## 3. The diff

`ops/proposals/codex-homebrew-read/codex-review.diff`, `diff -u` from main @ 378c92c to the patched
copy the tests in §4 ran against. `git apply --check` passes on main @ 378c92c.

```diff
--- a/ops/driver/codex-review.sh
+++ b/ops/driver/codex-review.sh
@@ -7,8 +7,9 @@
 # process that deliberately races the checks is out of scope: it can write files directly anyway.)
 # Codex reviews a plain export of HEAD (`git archive`, checked byte for byte, in its own fresh git
 # repo, no symlinks, sealed corpus manifest left out) under a permission profile that lets its
-# commands read only the export and the toolchain and write only the export, with the network,
-# ChatGPT apps and plugins off, strict config and a scrubbed environment.
+# commands read only the export and the toolchain (named Homebrew directories, never its etc/ or
+# var/) and write only the export, with the network, ChatGPT apps and plugins off, strict config
+# and a scrubbed environment.
 # Output: <out>.json (codex-review.schema.json plus status/reason) and <out>.log (Codex's log).
 # Exit codes: 2 = usage error (nothing written). Otherwise 0, always: on any failure the output
 # is {"status":"unavailable","reason":...}.
@@ -224,17 +225,26 @@
 
 # Codex's commands may read only the export (its workspace, writable), the platform paths of
 # `:minimal`, and the read-only toolchain: rustup toolchains, cargo's binaries and crate cache, and
-# Homebrew. Everything else is denied by `:root`, so the sealed manifest in the main checkout,
-# ~/.codex (auth), ~/.ssh, ~/.cargo/credentials.toml, ~/jpdf-corpus and ~/jpdf-research are
-# unreadable (AS.52, CX.1; CONSTITUTION 1.2, 13). `--sandbox workspace-write` limited writes and
-# network only. A permission profile replaces the legacy sandbox flags (Codex: "either
+# Homebrew's installed programs and libraries (HB_READ below). Everything else is denied by `:root`,
+# so the sealed manifest in the main checkout, ~/.codex (auth), ~/.ssh, ~/.cargo/credentials.toml,
+# ~/jpdf-corpus, ~/jpdf-research, and Homebrew's etc/ (configs) and var/ (logs, service data) are
+# unreadable (AS.52, CX.1, audit 0004 AS.2; CONSTITUTION 1.2, 13). `--sandbox workspace-write` limited
+# writes and network only. A permission profile replaces the legacy sandbox flags (Codex: "either
 # default_permissions and [permissions], or sandbox_mode / sandbox_workspace_write, but not both"),
 # so none of them may appear here. Profiles are Beta in Codex; --strict-config makes an unknown key a
 # failed run ("unavailable"), never a silently wider sandbox.
 q() { local s="${1//\\/\\\\}"; printf '"%s"' "${s//\"/\\\"}"; }   # TOML basic string
+# Homebrew, by name and never the prefix itself: bin/ and sbin/ (PATH) link into Cellar/ and opt/;
+# programs load libraries from opt/, lib/ and Frameworks/ (Python); some read data from share/,
+# libexec/ or include/. Of Caskroom/ only Codex's own cask (its bundled rg). A directory left out
+# (etc/, var/, Library/, the prefix's dotfiles, any new one) stays denied, and so does a symlink
+# from a granted directory into it (Seatbelt matches the resolved path; test plan T6 checks it).
+HB=/opt/homebrew
+HB_READ=("$HB/bin" "$HB/sbin" "$HB/Cellar" "$HB/opt" "$HB/lib" "$HB/Frameworks" "$HB/share"
+         "$HB/libexec" "$HB/include" "$HB/Caskroom/codex")
 FS_POLICY="{ \":root\" = \"deny\", \":minimal\" = \"read\", \":slash_tmp\" = \"deny\""
 for r in "$HOME/.rustup/toolchains" "$HOME/.rustup/settings.toml" "$HOME/.cargo/bin" \
-         "$HOME/.cargo/registry" /opt/homebrew; do
+         "$HOME/.cargo/registry" "${HB_READ[@]}"; do
   FS_POLICY+=", $(q "$r") = \"read\""
 done
 FS_POLICY+=" }"
```

## 4. Test plan

### Run by the session (static)
- `ops/proposals/codex-homebrew-read/test-policy.sh <patched> ops/driver/codex-review.sh`: 3 passed,
  0 failed. It pulls `q`, `HB`/`HB_READ` and `FS_POLICY` out of the script by their exact text,
  builds the policy with `HOME=/Users/u` and parses it as TOML (Python 3.14 `tomllib`):
  - the patched policy: `:root` deny, `:minimal` read, `:slash_tmp` deny; the read grants are
    exactly the 4 rustup/cargo paths and the 10 Homebrew paths; `/opt/homebrew` itself absent; no
    grant equals or contains `/opt/homebrew/etc`, `etc/gnupg`, `var`, `var/log`, `Library`, `.git`,
    `.codex`, `Caskroom`, `Caskroom/claude-code@latest`, `~`, `~/.codex`, `~/.ssh`,
    `~/.cargo/credentials.toml`, `~/jpdf-corpus` or `~/jpdf-research`;
  - the unpatched policy fails the "no grant contains a denied path" check with the assertion exit
    (4), not a crash, so the check tells the two apart;
  - against this machine: every `/opt/homebrew/bin` entry that resolves inside the prefix resolves
    into a granted directory, except the other casks (counted, by design).
- Hand mutants of the patched script, each caught: `Caskroom/codex` → `Caskroom`; an extra `$HB`,
  `$HB/etc` or `$HB/var/log`; `:slash_tmp` dropped; `$HB/sbin` dropped; `:root` = read;
  `~/.cargo/registry` → `~/.cargo`; `HB=/opt/homebrew/` (double slash in every grant).
- `bash -n` on the patched script.

### For the owner, after applying (Codex runs only through the launcher)
Use a scratch worktree and a brief that asks Codex to run each command and quote its output in a
finding, as in the #161 plan.
- **T1:** `git show 378c92c:ops/driver/codex-review.sh > /tmp/codex-review.old.sh`, then
  `ops/proposals/codex-homebrew-read/test-policy.sh ops/driver/codex-review.sh /tmp/codex-review.old.sh`
  on the applied script: expect `3 passed, 0 failed`.
- **T2:** `git diff` of the applied file against the diff in §3: identical.
- **T3 the profile loads:** run the launcher on any small brief. Expect `status: ok`.
- **T4 the review still builds and has its tools:** the brief asks for `rg --version`,
  `python3 -c 'print(1)'`, `cargo nextest --version` and `cargo test -p jpdf-syntax --offline -q`.
  Expect all to work. A denied path in the error names a directory to add (read-only, inside an
  install tree, never `etc/`, `var/` or the prefix).
- **T5 etc/ and var/ are unreadable:** the brief asks for `ls /opt/homebrew/etc /opt/homebrew/var/log`,
  `cat /opt/homebrew/etc/pkcs11/pkcs11.conf.example`, `ls /opt/homebrew` and
  `ls /opt/homebrew/Caskroom`. Expect a denial for each, and no file content in the output or the
  `.log`. If `ls /opt/homebrew/etc` succeeds, `:minimal` re-grants the prefix: stop and report.
- **T6 a link into etc/ stays denied:** before the run, `ln -s /opt/homebrew/etc/pkcs11/pkcs11.conf.example
  /opt/homebrew/share/jpdf-t6-link`; the brief asks Codex to `cat` the link and to read it with its
  file-reading tool, if it has one. Expect a denial both ways. Remove the link afterwards.

## 5. Risk if not applied
AS.2 stays: a prompt-injected or misbehaving Codex can read Homebrew's configs, service logs and
service data, send them to OpenAI and have them posted on a public PR (sev:high by ADR 0009, like
audit 0003's CX.1, #161). The launcher's header keeps claiming an isolation it doesn't have.
