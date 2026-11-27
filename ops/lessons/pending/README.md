# Pending lessons (ops/TICK.md §5, ADR 0011 §3; #386, #391)

A lesson a session finds after its work PR's last review round can't go into `ops/LESSONS.md`
any more: a LESSONS change is reviewed, and a record commit is not. It goes here instead, in
one file per session, so two open PRs never touch the same file (like the journal, retro 0002).

- **Name:** `YYYY-MM-DD-HHMM-<slug>.md`, the session's journal entry
  `ops/journal/YYYY-MM-DD/HHMM-<slug>.md` with its date folded into the name.
- **Content:** one line per lesson, written exactly as its `ops/LESSONS.md` line will read
  (*situation → what to do* plus the journal link, relative to `ops/`), so it moves verbatim.
  Nothing else: no heading, no other text.
- **Written** by that session only, in its record commit. No other session edits it.
- **Moved** by the next session that opens a work PR: before that PR's round 1 it adds each line
  to `ops/LESSONS.md` (refining an existing entry instead of duplicating it) and deletes the
  file, in the same commit. A file that an open agent PR already deletes is left to that PR.
  The retro that curates `ops/LESSONS.md` (ops/TICK.md §7 step 3) folds and deletes any left.

This README is not a pending lesson: it stays, so the directory always exists, and it belongs
to the process area (`ops/areas.toml`), so a change to it is reviewed.
