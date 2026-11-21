# Proposal: guard text rules skip inert gh post text only in a single plain gh command

Status: proposed by the session for owner issue #124, 2026-11-21; revised after review round 1 on
PR #129. Touches `.claude/hooks/guard.py` (protected, CONSTITUTION 12). Diff ready to apply:
[`guard-inert-post-text/guard.diff`](guard-inert-post-text/guard.diff) (against main 4ce8115).

## Problem
1. **Review posts get blocked.** Owner issue #124 asks that every AI review be posted on its PR.
   TICK §4 now has sessions post each reviewer round verbatim with
   `gh pr comment N --body '🤖 jpdf-agent ...'`. Reviews of Rust code often contain all three of:
   the Rust keyword for unchecked code (spelled un-safe in this file), a `.rs` path, and a `>`.
   `check_bash` blocks any command containing all three, to stop a shell redirect that writes the
   keyword into a Rust file, and it scans the whole command, quoted body included. Probe
   (2026-11-21): a `gh pr comment` whose single-quoted body has `lib.rs`, `n > 2^30` and the
   keyword is blocked. Until this is applied, TICK §4 has sessions hyphenate the keyword.
2. **Existing hole in the cmux rule (found by the PR #129 reviewer, reproduced).** The cmux rule
   checks `strip_inert_text(cmd, post_text)`, and `strip_inert_text` removes the *first* matching
   quoted copy of a body anywhere in the command, not the token gh parsed. So
   `bash -c '<tag>; cmux send y'; gh issue comment 5 --body "<tag>; cm""ux send y"` passes today:
   the body parses to the same text, the `bash -c` argument is the copy that gets stripped, and
   `bash -c` runs it. `eval` works the same way. The guard comment calls this rule the only
   barrier between an agent and the owner's other terminal tabs. A first draft of this proposal
   applied the same stripping to the keyword rule and inherited the hole.

## Proposed change
New helper `inert_view(cmd, texts)`: strip the inert titles/bodies as today, then use the
stripped text **only if what remains is one plain gh post**, i.e. it fully matches
`gh (issue|pr) (comment|create|edit) [A-Za-z0-9 _./:#=@,+'-]*`. That leaves no `;`, `&`, `|`, `<`,
`>`, `$`, backtick, `\`, `"` or newline outside the removed text, so no second command can exist
to run a stripped copy. Otherwise the rules see the full (whitespace-collapsed) command, as the
keyword rule does today. Both the cmux rule and the keyword rule use `inert_view`.

Self-test: 7 new cases, 93/93 pass on a scratch copy with the diff applied. The cases are:
- a review-style body is allowed;
- a chained redirect after a body is blocked, both with `;` and with `&&`;
- `$(...)` inside a double-quoted body is blocked;
- the duplicated-body bypass is blocked for `bash -c` and for `eval`, on the keyword rule;
- the duplicated-body bypass is blocked on the cmux rule.

Mutation check: making `inert_view` always return the stripped text fails exactly the 3
duplicated-body cases (90/93). The current guard passes its own 86 cases. The real round-1
review comment of PR #129 is allowed by the patched guard.

Not covered: the other text rules (sealed-corpus name, settings-file name, API host, force push,
`patch`/`git apply` segments, protected-path writes, `-R` repo) still scan quoted bodies, so a
review that quotes such text is blocked. TICK §4 handles that by rewording the one token and
disclosing it. They could move to `inert_view` later, one rule at a time, each with a bypass case.

## Risk if not applied
- Item 2 stays open: an agent can run a cmux command by duplicating it in a gh body. That needs
  an agent that deliberately goes around the guard, but this rule is the only barrier for cmux.
- Item 1: posted reviews keep hyphenating one keyword. That's cosmetic.
