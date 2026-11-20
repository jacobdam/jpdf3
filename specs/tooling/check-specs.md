# cargo xtask check-specs

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; AUTOMATION §5.1, §5.2; ops/judge/CONTRACT.md §4)
Status: implemented

## Behavior
`cargo xtask check-specs [--root DIR]` (default root: the current directory) checks the
documents under `DIR/specs/` and prints every problem as `<path>: <message>` on stderr. It
exits **0** when there are no problems, **1** when there are, **2** on a usage error.
`cargo xtask ci linux` runs the same check after writing `e2e.json` and fails if it fails.

1. **Feature specs.** Every `specs/<area>/**/*.md` except `_*.md`, `README*`, `journeys/` and
   `ui/` (the same selection as the judge's policy check) must have, as `## ` headings, every
   section that `specs/_TEMPLATE.md` has, plus the eight AUTOMATION §5.2 checklist sections
   (Persistence, Undo, Errors, Limits, Security, Accessibility, Platforms, Performance) even if
   the template ever loses one. Each section needs non-empty content: at least one
   non-blank line that is not an HTML comment, before the next `#`/`##` heading.
   A section heading that appears twice is a problem. Fenced code blocks are ignored when
   looking for headings.
2. **Journeys.** Every `specs/journeys/*.md` is parsed like the judge (CONTRACT §4):
   - a `## J-<AREA>-<NN>: <title>` heading (`AREA` = `[A-Z0-9]+`, `NN` = digits) starts a
     journey; the title must be non-empty;
   - a `## J-…` heading outside a code fence that does not match that form is a problem
     (the judge would silently skip it);
   - `- milestone:` is required and must be `M<digits>`;
   - `- platform:` is required: a comma list of `web`, `macos`, no duplicates;
   - `- test:` is required unless `superseded_by` is set: a comma list of
     `e2e.<platform>.<id>` (`id` = `[a-z0-9_]+`), each platform declared in `platform:`,
     every declared platform covered by at least one test, no duplicate test IDs;
   - `- superseded_by:` (optional) must name another journey that exists and is not the
     journey itself;
   - a field line given twice is a problem (the judge reads only the first);
   - journey IDs are unique across all journey files.
3. **Traceability.** `specs/TRACE.md` must exist once any journey or feature spec exists.
   - Every journey ID must appear in it.
   - Every feature spec path (`specs/<area>/<file>.md`) must appear in it.
   - Every `J-…-NN` it mentions must be a known journey.
   - Every `specs/…md` path it mentions must exist. Glob patterns (containing `*`) are not path mentions.

## Persistence
Read-only: the checker never writes files except `e2e.json` through `ci`, which it already did.

## Undo
Not applicable (read-only tool).

## Errors
- Unreadable file or non-UTF-8 text: reported as a problem for that path (lossy decoding is not
  used, so a bad byte cannot hide a heading).
- Missing `specs/` directory: no problems (nothing to check).
- Missing `specs/_TEMPLATE.md`: reported; the eight §5.2 sections are still required.
- Unknown argument: usage line on stderr, exit 2.

## Limits
Runs on repository files written by the agent, not on user input. Still, no `unwrap`/panicking
index (workspace lints); files above 1 MiB are reported as a problem and not parsed; directory
walks do not follow symlinks.

## Security
Repository text is data. The checker only reads files under `specs/`; it never executes or
fetches anything. It never reads `ops/judge/corpus/` (CONSTITUTION 13). It does not replace the
judge's own policy and journey checks; it runs earlier and is stricter so problems show up
in the agent's local checks.

## Accessibility
Plain-text output, one problem per line, paths first, for screen readers and grep.

## Platforms
Same on Linux and macOS. Run from `cargo xtask ci linux` only (the macOS CI job builds the app).

## Performance
Linear in the size of `specs/`; well under a second for the expected hundreds of files.

## Tests
- Unit tests in `xtask/src/specs.rs`: section parsing (fences, comments, duplicates, empty
  sections), journey field validation (each rule above, good and bad), TRACE cross-checks.
- Fixture trees `xtask/tests/fixtures/{good,bad}/specs/`, checked end to end by
  `xtask/tests/check_specs.rs`, including the exit code.
- The real repository passes `cargo xtask check-specs`.
