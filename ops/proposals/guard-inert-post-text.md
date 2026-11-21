# Proposal: guard text rules skip inert gh post text only in a single plain gh command

Status: proposed by the session for owner issue #124, 2026-11-21; revised after review rounds 1
to 4 on PR #129. Touches `.claude/hooks/guard.py` (protected, CONSTITUTION 12). Diff ready to
apply: [`guard-inert-post-text/guard.diff`](guard-inert-post-text/guard.diff) (against main 4ce8115).
Tracked as owner-todo #130.

## Problem
1. **Review posts get blocked (the main reason to apply).** Owner issue #124 asks that every AI
   review be posted on its PR. TICK §4 now has sessions post each reviewer round verbatim with
   `gh pr comment N --body '🤖 jpdf-agent ...'`. Reviews of Rust code often contain all three of:
   the Rust keyword for unchecked code (spelled un-safe in this file), a `.rs` path, and a `>`.
   `check_bash` blocks any command containing all three, to stop a shell redirect that writes the
   keyword into a Rust file, and it scans the whole command, quoted body included. Most reviews
   also contain an apostrophe, which TICK writes as `'\''`, and `strip_inert_text` never strips a
   body that contains one, so the cmux rule then sees the body too. Until this is applied, TICK
   §4 has sessions reword the triggering word and disclose it.
2. **The current stripping can be fooled.** The cmux rule checks `strip_inert_text(cmd,
   post_text)`, which removes the *first* matching quoted copy of a gh title/body anywhere in the
   command, not the token gh parsed. Four shapes pass today (all reproduced):
   - `bash -c '<tag>; cmux send y'; gh issue comment 5 --body "<tag>; cm""ux send y"`: the body
     parses to the same text, and the `bash -c` copy is the one stripped and run;
   - the same with `eval`;
   - a split-quote title (`--title cm'ux send y'`), then a newline and `bash -c 'cmux send y'`;
   - a `#` comment, as in `gh pr edit 5 # --title 'x` + newline + `cmux send y #'`: shlex sees one
     title, but bash and zsh end the comment at the newline and run the second line.

**What this does not fix.** The cmux rule and the keyword rule are plain text matches, and any
command can dodge them without gh at all: an empty quote pair inside the word (`c''mux`), a
backslash inside it, or `echo un''safe > a.rs` (which writes the real keyword). The guard comment
calls the cmux rule "the only barrier" between an agent and the owner's other terminal tabs, but
a text match is no barrier against an agent that wants to get around it. This diff only stops
the gh-post stripping from adding further shapes. A real barrier needs something that doesn't
read command text: socket authentication for cmux, or a permission deny rule. That is an owner
decision, noted in #130.

## Proposed change
- `strip_inert_text` also recognises a single-quoted body with each `'` written as `'\''`. That
  form is still exactly one shell word with no expansion.
- New helper `inert_view(cmd, texts)`: strip the inert titles/bodies, then use the stripped text
  **only if the remainder, before any whitespace is collapsed, is one plain gh post**:
  `gh (issue|pr) (comment|create|edit)` followed only by spaces and words that are either bare
  (`[A-Za-z0-9_./:=@,+-]+`, not starting with `=`), the `''` placeholder, or `--flag=''`. No `#`
  (a comment word), no leading `=` (zsh EQUALS expansion), no newline, CR, tab, quote inside a
  word, shell metacharacter or non-ASCII character. Otherwise the rules see the full command
  (whitespace collapsed), as the keyword rule does today.
- The cmux rule and the keyword rule both use `inert_view`.
- **Tightening:** a post chained with anything (`gh pr comment ... && git status`) now always gets
  the full view, so a quoted cmux mention in a chained post is blocked where it passes today.
  TICK §4 already says one post per Bash command.

The invariant: outside the stripped quoted words, only spaces and words that bash and zsh read
literally remain. So the shell is in the unquoted state at each stripped word, reads it as one
literal word, and runs exactly one simple command, gh. Consequences:
- If a *different* copy of the text was stripped (say a `bash -c` argument), gh's own token
  wasn't stripped, so it still holds quotes or escapes and fails the word check.
- A bare second command (`--body '<tag> x'` + newline + a command) isn't stripped at all, so the
  rules still see it. The newline check is a second, independent barrier for the first case.

Fuzzing by the PR #129 reviewers:
- Round 3: about 1.2 million commands (an earlier version, `#` removed), with a bash oracle.
- Round 4: about 120 000 commands, 34 000 of them on the stripped path, run under both bash and
  zsh with a fake gh on PATH.

In both runs every allowed command ran exactly one gh call. The only difference between shlex
and the shells that round 4 found was the zsh `=word` expansion, which is now rejected.

## Verification (scratch copy of the guard with the diff applied)
- Self-test 135/135: the current 86 cases, 15 new command cases and 34 direct `inert_view` asserts.
  - Allowed: a review-style body; a body with `'\''` and both trigger words; a `--body=` form.
  - Blocked:
    - a redirect chained after a body (`;`, `&&`, CR, newline);
    - `$(...)` in a double-quoted body;
    - a duplicated body run by `bash -c` (both rules) or by `eval`;
    - a split-quote title with a newline and `bash -c` (both rules);
    - a `#` comment that ends inside a quoted body (both rules).
  - `inert_view` asserts:
    - a plain post is stripped, including the `--body=''` form;
    - each of these keeps the full view:
      - a newline, a quote inside a word, a `#` word;
      - each of 23 characters outside the word class;
      - a glued placeholder, or a widened `--flag=` word;
      - a word starting with `=`;
      - another verb, or text before `gh`.
- 27 mutants of the new code, each caught by at least one failing case:
  - `inert_view` always strips;
  - no `'\''` form;
  - collapsing whitespace before the match, alone and together with quotes inside words;
  - a glued placeholder, a widened `--flag=`, any verb, no anchor, allowing `=` words;
  - adding any one of `; | & ` $ > < ( \ ~ * ? { # '` newline, tab or `!` to the class.
- The current guard passes its own 86 cases.

Not covered: the other text rules (sealed-corpus name, settings-file name, API host, force push,
push to main, `-R`, `patch`/`git apply` lines, protected-path writes, gh api writes, repo and
credential commands) still scan quoted bodies, so a review that quotes such text is blocked. TICK
§4 handles that by rewording the token and disclosing the change.

## Risk if not applied
- Posted reviews keep rewording a word or two, with each change disclosed.
- The four gh-post shapes in Problem 2 keep working. Given "What this does not fix", that adds
  little to what an agent that ignores the rules could already do.
