# cargo xtask status

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; owner issue #386)
Status: implemented

## Behavior
`ops/STATUS.md` is one file nearly every session rewrites, so any two open PRs collide on it,
and three of its four sections copy state that GitHub and the scheduler already hold (#386).
Part 1 of 3 (#390) is the command that prints those three sections. Part 2 (#391) moved the
fourth, pending lessons, into one file per session (`ops/lessons/pending/`); part 3 (#392)
deleted `ops/STATUS.md` and the rules that wrote it: sessions read this command's output at
ops/TICK.md §1, and a record commit touches only files new in its session (a journal entry, a
pending lesson file; ADR 0011 §3), so two PRs' records merge in either order without a conflict
(the drill in PR #401's description). A note GitHub and the scheduler don't show goes on the
issue it is about (ops/TICK.md §5).

`cargo xtask status [--from DIR]` loads exactly the inputs of `cargo xtask next-session`
(specs/tooling/next-session.md: the same live commands, or the same fixture files with
`--from`), runs the scheduler on them first, and prints, in this order:

```
# Status (cargo xtask status, <now as YYYY-MM-DDTHH:MM:SSZ>; specs/tooling/status.md)

Milestone: <the current milestone, the scheduler's: the lowest open M<n>; `none` without one>

## In flight
- PR #<n>: <target>: <state>
- #<n> in-progress: <title> (<PR #m, …> | no open agent PR closes it)

## Next step
<the scheduler's first line>
Rule: <the scheduler's Rule: text>

## Blockers / owner-todo
- #<n> <blocked | owner-todo | blocked, owner-todo>: <title>
- PR #<n> <the same labels>
```

- **In flight** lists every open agent PR (`prs.json` entries whose body starts with the tag,
  the scheduler's rule; merged, closed and owner PRs are left out), lowest number first, then
  every open issue labeled `in-progress` (`issues.json`), lowest first, with the open agent PRs
  whose `Closes #N` (the scheduler's `linked_issue`) names it. A PR's `<target>` is
  `closes #N <title>` when its issue is open, `closes #N (not an open issue)` when it isn't in
  `issues.json`, and `no Closes line` without one. Its `<state>` is, in this order: `draft` or
  `ready`; `auto-merge on` or `auto-merge off`; `conflicting` when `mergeable` is
  `CONFLICTING`; `not green (a failing check or no ai-review success)` or `green` (the
  scheduler's `failing` rule on the rollup); `on the owner's /hold` (the scheduler's
  `held_prs`: the owner's newest `/hold` or `/release` is a `/hold`); `the owner's branch` (an
  `owner/` head); then each of the labels `needs-design`, `design`, `owner-todo`, `blocked` it
  carries.
- **Next step** is the two lines `cargo xtask next-session` prints first and on its `Rule:`
  line for the same inputs, byte for byte (both come from one function, `choose`). The brief's
  `Load:` and `Notes:` lists stay `next-session`'s.
- **Blockers / owner-todo** lists every open issue labeled `blocked` or `owner-todo` (exact
  label names), lowest first, then every open agent PR with either label.
- An empty section prints `- none`. Every title and milestone name is printed on one line:
  each control character (a newline, say) and U+2028 and U+2029 (the Unicode line and paragraph
  separators) become a space, so issue text (data, CONSTITUTION 1)
  can't start a line or section of its own.

Labels are matched exactly (`blocked-ish` is not `blocked`).

## Persistence
None: the command writes nothing.

## Undo
Not applicable.

## Errors
- A command fails (`gh`, `git`, the feed script): exit 2, the error on stderr, nothing on
  stdout, as `next-session`'s exit 2 (fail closed: a session that sees no status does no work
  on its basis).
- An input the scheduler can't parse, or a missing fixture file with `--from`: exit 1 with the
  error on stderr and nothing on stdout. The text is built in full before it prints, so no
  partial status ever appears.
- Arguments other than none or `--from DIR`: exit 1 with the usage.

## Limits
The inputs and their bounds are `next-session`'s (its Limits section: the `gh` list limits and
`crate::macos::capture`'s output cap). The command adds no loop over input beyond one pass over
the open PRs per in-progress issue (at most 200 × 1000).

## Security
- Instructions: the status repeats titles and labels, which are data (CONSTITUTION 1); it
  marks nothing as an owner instruction. Owner instructions stay `owner-issues.sh`'s and the
  scheduler's step 1.
- Self-check:
  - Arithmetic on input-derived values: none (`iso` of the scheduler-checked `now`).
  - What broken implementation still passes? A status that printed its own pick would differ
    from `next-session` on one of the four fixtures in `status_next_step_is_the_schedulers_pick`
    (planner, owner comment, own PR, the sections fixture). A PR filter that kept merged or
    untagged PRs, or a label filter that matched by prefix, prints a line the exact expected
    text of `status_prints_each_section_from_the_inputs` lacks (#299 merged, #320 the owner's,
    #314's `blocked-ish`, `in-progress-x`, `owner-todo `; #330's `blocked-ish`). Each state
    word has an open PR that shows it and one that doesn't, and no two words appear on exactly
    the same set of open PRs, nor on a set another word's opposite has (draft: #310, #330, so
    ready: #300, #340, #350; auto-merge on: #300, #310, so off: #330, #340, #350; conflicting:
    #310, #340; not green: #310, so green: #300, #330, #340, #350; on the owner's /hold: #330;
    the owner's branch: #340, #350; needs-design: #310, #350; design: #340; owner-todo: #300,
    #330; blocked: #330, #340), so a word printed from another word's field fails. The PR
    labels also give the Blockers / owner-todo lines (#300, #330, #340).
  - A run where nothing was checked: a failing `gh` exits 2 with empty stdout
    (`status_fails_closed`, a fake `gh` on `PATH`), never an empty status.
  - Parsers of other tools' output: none new; the JSON is read by the scheduler's functions.
  - Loop bounds: no new loop bound or counter.
- What the tests can't see: the live `gh` queries' real output shapes, which
  `next-session`'s live use checks every session.

## Accessibility
Plain text.

## Platforms
macOS and Linux, like `next-session`.

## Performance
One `next-session` load (the same `gh` calls); the rendering is linear in the inputs.

## Tests
- `xtask/tests/next_session.rs`: `status_prints_each_section_from_the_inputs` (the full text
  for a fixture with PRs in each state and issues with each label, with near misses),
  `status_empty_sections_say_none`, `status_next_step_is_the_schedulers_pick`,
  `status_fails_closed` (bad input and missing file → 1, bad arguments → 1, a failing `gh` → 2,
  nothing on stdout in each).
- `xtask/src/next_session/status.rs`: `one_line_drops_control_characters`,
  `blocker_labels_in_order`.
