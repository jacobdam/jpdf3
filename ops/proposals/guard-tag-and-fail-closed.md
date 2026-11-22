# Proposal: guard checks every gh post the shell runs, fails closed, covers Bash reads of secrets (#162)

Status: proposed by a session for issue #162 (audit 0003 findings CX.4, CX.5, AS.53, AS.54),
2026-11-22. Touches `.claude/hooks/guard.py` (protected, CONSTITUTION 12), so the owner applies
it with a signed commit. Diff ready to apply:
[`guard-tag-and-fail-closed/guard.diff`](guard-tag-and-fail-closed/guard.diff), against main
e483671 (guard.py last changed in 5816177). Test scripts in the same directory (§4).

## 1. Findings and fixes

Probes run this session through the hook entry point (JSON on stdin, as Claude Code calls it),
with [`probe.py`](guard-tag-and-fail-closed/probe.py). Exit 2 = blocked, 0 = allowed, 1 = hook
error (Claude Code treats it as non-blocking, so the call runs).

| Finding | Where (main @ e483671) | Probe on main | Fix |
|---|---|---|---|
| CX.4 (codex): an untagged post passes when the gh name is quoted. | `guard.py:503`: `check_gh_posts` returns early unless the **raw** text matches `\bgh\s+(issue\|pr)\s+<verb>`. | `"gh" issue comment 5 --body hi` → 0; `g""h ...` → 0. The session found more shapes that pass the same way: `gh issue \` + newline + `comment ...` (line continuation), `ls;gh ...`, `(gh ...)`, `echo $(gh ...)`, backticks, `bash -c 'gh ...'`, `eval 'gh ...'`: all 0. | (a) The early return now tests a **dequoted** copy (quotes, backslashes and line continuations removed, as `codex_command` already does). (b) After the shlex scan, the Codex tripwire's shell tokenizer counts the posts the shell would run (it splits `;gh`, `(gh`, `$(gh`, backticks, joins continuations, removes quotes); if it finds more than the shlex scan checked, the call is blocked. (c) A post inside `bash -c`/`sh -c`/`eval` text (found by the tripwire's existing recursion, now with a pluggable matcher) is blocked, tagged or not. |
| CX.5 (codex): `$((a << b))` is read as a heredoc with delimiter `b`, so the tokenizer skips the rest of the command, including a later `codex` call. | `guard.py:156`: the shift test looks only at the first character after `<<` (digit, `$`, `(`, empty). | `x=$((a << b))` + newline + `codex exec x` → 0; `(( m = a << b ))` + newline + `codex exec x` → 0. | The tokenizer tracks arithmetic frames: the second `(` of `((` or `$((` opens one; while any is open, `<<` is a shift. A wrong guess only makes the tokenizer scan more text as commands (more blocks, never fewer). Also `$'...'`/`$"..."` now quote like `'...'`/`"..."`, so `$'gh'` and `$'codex'` are seen. |
| AS.53: any exception but `Block` exits 1, a non-blocking hook error (fail open). | `guard.py:612`: `open(fp).read()` decodes strictly; `main()` lets other exceptions escape; bad JSON input exits 0. | A Write of the Rust keyword for unchecked code into an existing `.rs` file holding one `\xff` byte → 1 (`UnicodeDecodeError`), so the write runs. | `run_hook()` catches every `Exception` after `Block` and exits 2 with `guard error (<type>: <message>)`; unreadable or non-object JSON input exits 2 too. The Write reads the old file with `errors="replace"`, so that probe now hits the Rust rule (exit 2, CONSTITUTION 3). |
| AS.54: the secret paths are checked for the Read tool only. | `guard.py:605` | `cat ~/.codex/auth.json` → 0; `cat ~/.ssh/id_ed25519` → 0 (`Bash(cat:*)` is allowlisted). | Each shlex word of each Bash segment that has no whitespace and has `.ssh`, `.codex` or `.config/gh` as a path component (after `^`, `/`, `~`, `<`, `>`, `=`, `:`) blocks: `~/.ssh`, `$HOME/.codex/auth.json`, `.codex/auth.json`, `<~/.ssh/id_rsa`, `~/.co''dex/x`. Not `~/.codex-notes`, `docs/x.ssh`, `~/.sshd`, or prose (a commit message or post body that names the path has spaces). |

With the diff, every probe but the last (a tagged post) exits 2.

## 2. The diff, in order
1. Module docstring: the guard fails closed; a hook timeout still fails open (Claude Code's
   choice, see §5).
2. `_SECRET_WORD` regex next to `SECRET_READ`.
3. `_codex_commands`: `arith` counter and `"a"` stack frames; `$'`/`$"` prefix dropped; `<<`
   inside an arithmetic frame is a shift.
4. `_is_codex(words, i, depth)` split out of `_codex_runs`; `_codex_runs`, `_codex_nested` and
   `_codex_check` take `hit=_is_codex` and pass it down. With the default, behavior is
   unchanged.
5. `check_bash`: the secret-word rule, per segment, before the wrapper stripping.
6. `check_gh_posts`: dequoted early-return test, `checked` count, and the tokenizer cross-check
   (`_gh_post_at`, `_gh_post_nested`) with one block message that says how to write a post.
7. `check`: the Write tool reads the old file as UTF-8 with `errors="replace"`.
8. `run_hook(stdin)` holds the hook logic (so the self-test can call it); `main()` calls it.
9. Self-test: 36 new command cases (each fix, plus allowed look-alikes), 6 `run_hook` cases
   (bad JSON, a JSON array, a crash in `check`, the non-UTF-8 Write with and without the keyword,
   a plain call), and 4 timing inputs for `check_gh_posts` (each must finish in 2 s; the nested
   recursion shares the tripwire's one budget of 2× the command + 4 KiB). The self-test writes
   its non-UTF-8 file under `target/guard-selftest/` (git-ignored) and removes it, not `/tmp`
   (owner instruction #28).

## 3. Behavior changes sessions will notice
- A gh post chained without spaces (`ls;gh issue comment ...`), inside `( )`, `$( )`,
  backticks, `bash -c` or `eval` is blocked even when tagged. TICK §4 already says one post per
  Bash command, as a plain command.
- Any crash in the guard blocks the call that caused it, with the error type in the message.
  A guard bug that crashes on a common call shape would block every such call until the owner
  fixes it; the owner can still work with `JPDF_OWNER=1`. The self-test covers the call shapes
  sessions use (256 cases).
- A Bash word naming `~/.ssh`, `~/.codex` or `~/.config/gh` (or `.ssh/...` relative) is
  blocked, including a grep pattern such as `'.codex/'`. Sessions never need those paths;
  `ops/driver/codex-review.sh` reads Codex's config itself, and its command line names none.

## 4. Test plan

Sessions can't run `git apply`/`patch` (the guard blocks both), so the session checked the diff
by applying it with a strict Python hunk applier to `git show e483671:.claude/hooks/guard.py`
and comparing the result byte for byte with the tested copy. Results this session (macOS,
`/usr/bin/python3` 3.9.6 and Homebrew 3.14):

| # | Step | Expected | This session |
|---|---|---|---|
| T1 | `git apply ops/proposals/guard-tag-and-fail-closed/guard.diff` on main | clean | applied with the Python applier: identical to the tested copy |
| T2 | `python3 -I .claude/hooks/guard.py --selftest` (3.9 and 3.14) | `guard selftest: 256/256 passed`, about 4 s | 256/256 on both |
| T3 | `python3 -I ops/proposals/guard-tag-and-fail-closed/probe.py <guard.py> <empty scratch dir>`, before and after T1 | before: as in §1; after: every probe exits 2 but `tagged ok` (0) | as expected, both |
| T4 | `python3 -I ops/proposals/guard-tag-and-fail-closed/mutants.py .claude/hooks/guard.py <empty scratch dir>` after T1 | `mutants: 14/14 killed` (each mutant removes or flips one new term; the self-test must fail; 60 s cap each) | 14/14 killed |
| T5 | In a normal (not `JPDF_OWNER`) Claude session after T1: `cat ~/.codex/auth.json`; `gh issue comment <n> --body hi`; a tagged `gh issue comment`; `cargo fmt --all`; an Edit of a `.md` file | blocked, blocked, allowed, allowed, allowed | not run (needs the applied hook) |

## 5. What this does not fix
- The guard reads command text; it is not a sandbox. A variable (`g=gh; $g issue comment ...`),
  a gh alias (`gh alias set`), `$'\x67h'` (ANSI-C escapes aren't decoded), or another program
  that calls gh or reads a file (`python3 -c ...`, a glob such as `~/.co*/auth.json`) still
  passes. The judge and the tag check in `owner-issues.sh` remain; CONSTITUTION and TICK forbid
  going around the guard.
- The guard's other gh and git rules (credential subcommands, other-repo targets, API writes,
  repository management, admin merges, force pushes) still match raw text, so a quoted program
  name dodges them too; and Grep and Glob are not in the hook matcher. Filed as #182
  (sev:medium, M10).
- A hook that times out still fails open (Claude Code behavior). An explicit `timeout` on the
  hook in `.claude/settings.json` is a separate owner follow-up (noted in `ops/STATUS.md`).

## 6. Risk if not applied
An injected or careless session can post untagged text that `owner-issues.sh` shows the next
session as an owner instruction (CONSTITUTION 1.1), run Codex outside the launcher behind an
arithmetic shift, slip a forbidden write past the guard through any crash, and print the gh, SSH
or Codex credentials with `cat`.
