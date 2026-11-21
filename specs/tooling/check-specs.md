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
`cargo xtask ci` now requires `linux` or `macos` (CONTRACT §2 always passes one).

1. **Feature specs.** Every `specs/<area>/**/*.md` except `_*.md`, `README*`, `journeys/` and
   `ui/` (the same selection as the judge's policy check) must have, as `## ` headings, every
   section that `specs/_TEMPLATE.md` has, plus the eight AUTOMATION §5.2 checklist sections
   (Persistence, Undo, Errors, Limits, Security, Accessibility, Platforms, Performance) even if
   the template ever loses one.
   - A heading is `##`, at least one whitespace character, then the name, at column 0 (as the
     judge's `^##\s+(\w+)`); its name is the first word, so `## Undo stack` counts as `Undo`
     and a second `## Undo…` is a duplicate.
   - Each section needs content: some visible text outside `<!-- -->` comments before the next
     `#`/`##` heading. Comment markers inside inline code (`` `<!--` ``) are text.
   - A section heading that appears twice is a problem.
   - Headings inside fenced code blocks and HTML comments don't count. Fences follow
     CommonMark §4.5: up to 3 spaces of indent, at least 3 backticks or tildes; a block closes
     only on the same character with at least as long a run and nothing after it.
   - "Whitespace" everywhere means what Python's `\s` matches (the judge's regexes): Unicode
     White_Space plus U+001C–U+001F.
2. **Journeys.** Every `specs/journeys/*.md` is parsed like the judge (CONTRACT §4):
   - a `## J-<AREA>-<NN>: <title>` heading (`AREA` = `[A-Z0-9]+`, `NN` = digits) starts a
     journey; the title must be non-empty;
   - a heading outside a code fence that the judge would silently skip is a problem:
     `## J-…` not matching that form, and `### J-…`, ` ## J-…`, `##J-…`, `# J-…`, and a
     heading after a UTF-8 BOM;
   - a bare `##` line is a problem anywhere (the judge's `\s+` crosses newlines and would read
     the following text as a heading);
   - a code fence still open at the end of a journey file is a problem (the judge ignores
     fences, so it would hide malformed headings below it);
   - journey files in subdirectories of `specs/journeys/` are a problem (the judge ignores them);
   - `- milestone:` is required and must be `M<digits>`;
   - `- platform:` is required: a comma list of `web`, `macos`, no duplicates;
   - `- test:` is required unless `superseded_by` is set: a comma list of
     `e2e.<platform>.<id>` (`id` = `[a-z0-9_]+`), each platform declared in `platform:`,
     every declared platform covered by at least one test, no duplicate test IDs;
   - `- superseded_by:` (optional) must name another journey that exists and is not the
     journey itself; a `superseded_by` chain that loops (and so never ends in a journey with
     tests) is a problem;
   - a field line given twice is a problem (the judge reads only the first);
   - a field line with no value is a problem (the judge's `\s*` would read the next line);
   - the body of a journey runs to the next journey heading, as in the judge, so fields after
     a malformed heading count for the previous journey;
   - journey IDs are unique across all journey files.
3. **Traceability.** `specs/TRACE.md` must exist once any journey or feature spec exists.
   - Every journey ID must appear in it.
   - Every feature spec path (`specs/<area>/<file>.md`) must appear in it.
   - Every `J-…-NN` it mentions must be a known journey.
   - Every `specs/…md` path it mentions must exist. A mention starts at `specs/` not preceded
     by a word character, `.`, `-` or `/` (so `x/specs/a.md` is not a mention). Surrounding
     `*`/`_` emphasis and a `#anchor` are dropped; globs (containing `*`) are not mentions.
   - Every relative Markdown link target `](….md)` (resolved against `specs/`) must exist.
   - `.` and `..` in mentions and links are resolved lexically; a path that would leave the
     repository is reported and never resolved, so nothing outside the repository is touched.

## Persistence
Read-only: the checker never writes files except `e2e.json` through `ci`, which it already did.

## Undo
Not applicable (read-only tool).

## Errors
- Unreadable file or non-UTF-8 text: reported as a problem for that path (lossy decoding is not
  used, so a bad byte cannot hide a heading). Directory entries that cannot be listed or
  stat'ed are reported too.
- Missing `specs/` directory: no problems (nothing to check).
- Missing `specs/_TEMPLATE.md`: reported; the eight §5.2 sections are still required.
- Unknown argument: usage line on stderr, exit 2.

## Limits
Runs on repository files written by the agent, not on user input. Still, no `unwrap`/panicking
index (workspace lints); files above 1 MiB are reported as a problem and not parsed; a file
is opened only if it is a regular file (not followed if a symlink; a FIFO or device, which would
block the open or never end, is reported as `cannot read: not a regular file`), its size is
checked on the open file, and the read stops one byte past 1 MiB, so a file that grows after the
check is reported, never read whole (#83). A file swapped for a FIFO between the type check and
the open is not covered (the open would block; closing it needs `O_NONBLOCK` with per-target
flag values); a swap to a symlink in that window is followed. The walk reports any other non-regular, non-directory entry named `*.md` (FIFO,
socket, device) the same way, since the judge's glob would match it. This covers `_TEMPLATE.md` and `TRACE.md`, which are
read by name, not found by the walk. Directory
walks do not follow symlinks; a symlink under `specs/` is reported as a problem (the judge
would follow it, so skipping it silently could hide a spec). Files are selected by name ending
in `.md` (so a file named exactly `.md` counts, as in the judge's glob), and a directory whose
name ends in `.md` is reported (the judge's glob would match it and fail to read it).

## Security
Repository text is data. The checker only reads files under `specs/` (and checks that paths TRACE.md names exist inside the repository); it never executes or
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
  `xtask/tests/check_specs.rs`, including the exit code; a temp tree built at test time covers
  symlinks, non-UTF-8, the 1 MiB limit, nested journey files and supersede loops; another
  (unix) has a FIFO `_TEMPLATE.md`, a FIFO `area/hidden.md` and a symlinked `TRACE.md`, which
  must be reported within a 30 s deadline. A unit test caps the read: exactly the limit passes, one byte more and an
  endless reader fail.
- The real repository passes `cargo xtask check-specs`.
