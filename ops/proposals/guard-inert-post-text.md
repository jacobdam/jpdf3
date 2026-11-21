# Proposal: guard text rules skip inert gh post text only in a single plain gh command

Status: proposed by the session for owner issue #124, 2026-11-21; revised after review rounds 1
and 2 on PR #129. Touches `.claude/hooks/guard.py` (protected, CONSTITUTION 12). Diff ready to
apply: [`guard-inert-post-text/guard.diff`](guard-inert-post-text/guard.diff) (against main 4ce8115).

## Problem
1. **Existing hole in the cmux rule (found in review, reproduced).** The rule checks
   `strip_inert_text(cmd, post_text)`, and `strip_inert_text` removes the *first* matching quoted
   copy of a gh title/body anywhere in the command, not the token gh parsed. So this passes today:
   `bash -c '<tag>; cmux send y'; gh issue comment 5 --body "<tag>; cm""ux send y"`. The body
   parses to the same text, the `bash -c` argument is the copy that gets stripped, and `bash -c`
   runs it. `eval` works the same way, and so does a title written with a split quote
   (`--title cm'ux send y'`) followed by a newline and `bash -c 'cmux send y'`. The guard
   comment calls this rule the only barrier between an agent and the owner's other terminal tabs.
   Tracked as owner-todo #130.
2. **Review posts get blocked.** Owner issue #124 asks that every AI review be posted on its PR.
   TICK §4 now has sessions post each reviewer round verbatim with
   `gh pr comment N --body '🤖 jpdf-agent ...'`. Reviews of Rust code often contain all three of:
   the Rust keyword for unchecked code (spelled un-safe in this file), a `.rs` path, and a `>`.
   `check_bash` blocks any command containing all three, to stop a shell redirect that writes the
   keyword into a Rust file, and it scans the whole command, quoted body included. Most reviews
   also contain an apostrophe, which TICK writes as `'\''`, and `strip_inert_text` never strips
   a body that contains an apostrophe, so even the cmux rule sees those bodies. Until this is
   applied, TICK §4 has sessions hyphenate the triggering word and disclose it.

## Proposed change
- `strip_inert_text` also recognises a single-quoted body with each `'` written as `'\''`. That
  form is still exactly one shell word with no expansion.
- New helper `inert_view(cmd, texts)`: strip the inert titles/bodies, then use the stripped text
  **only if the remainder, before any whitespace is collapsed, is one plain gh post**:
  `gh (issue|pr) (comment|create|edit)` followed only by spaces and words that are either bare
  (`[A-Za-z0-9_./:#=@,+-]+`), the `''` placeholder, or `--flag=''`. That leaves no newline, CR,
  tab, quote character inside a word, shell metacharacter or non-ASCII character. Otherwise the
  rules see the full command (whitespace collapsed), as the keyword rule does today.
- The cmux rule and the keyword rule both use `inert_view`.

Why a second command can't hide: if a *different* copy of the text was stripped (say a `bash -c`
argument), then gh's own token was not stripped, so it still holds quotes or escapes and fails
the word check. A bare second command (`--body '<tag> x'` + newline + a command) isn't stripped
at all, so the rules still see it. The newline/whitespace check is a second, independent barrier
for the first case.

## Verification (scratch copy of the guard with the diff applied)
- Self-test 99/99: the current 86 cases plus 13 new ones.
  - Allowed: a review-style body; a body with `'\''` and both trigger words; a `--body=` form.
  - Blocked: a redirect chained after a body (`;`, `&&`, CR, newline); `$(...)` in a
    double-quoted body; a duplicated body run by `bash -c` (both rules) or by `eval`; a split-quote
    title with a newline and `bash -c` (both rules).
- Mutants of the new code:
  - `inert_view` always strips: 94/99, so 5 cases fail.
  - No `'\''` form: 98/99.
  - Collapsing whitespace before the match, together with allowing quotes inside words: 97/99.
  - Each of those two alone passes 99/99. The two checks back each other up: the split-quote
    attack needs both a quote inside a word and a newline.
- The current guard passes its own 86 cases. The round-1 review comment of PR #129 passes the
  patched guard. Round 2 still needed rewording, because of the push-to-main rule (not covered
  here).

Not covered: the other text rules (sealed-corpus name, settings-file name, API host, force push,
push to main, `-R`, `patch`/`git apply` segments, protected-path writes) still scan quoted bodies,
so a review that quotes such text is blocked. TICK §4 handles that by rewording the one token
and disclosing it. Those rules could move to `inert_view` later, one rule at a time, each with a
bypass case.

## Risk if not applied
- Problem 1 stays open: an agent can run a cmux command by putting a copy of it next to a gh
  body. That needs an agent deliberately going around the guard, but this rule is the only
  barrier for cmux. This is the main reason to apply.
- Problem 2: posted reviews keep rewording a word or two, with the change disclosed.
