# cargo xtask metrics

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; AUTOMATION §11, ops/TICK.md §7 step 1)
Status: implemented

## Behavior
`cargo xtask metrics [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--out DIR] [--from DIR] [--index FILE]`
prints the retro input (AUTOMATION §11) for one period as a markdown report, so a retro takes
its numbers from a command instead of counting by hand (#91, retro 0001). This is part 1 of 2:
CI failures by job and a JSON summary are part 2 (#219).

- **Period.** `--since` and `--until` are UTC calendar dates, both inclusive. `--until`
  defaults to today (UTC, from the system clock). `--since` defaults to the UTC committer date of
  the commit that added the last `docs/retros/NNNN.md` (exact four-digit name, so
  `0000-pilot.md` doesn't count) on the first-parent history; with no retro record, or one
  dated after `--until`, to `--until` minus 7 days. An explicit `--since` after `--until`
  (given or defaulted) is a usage error.
- **Inputs.** It runs only read commands, from the current directory (the repo root):
  1. `git log --first-parent --no-renames --format=@@%H%x09%ct%x09%s --numstat --summary origin/main`
     → `raw/git-log.txt` (it reads the local `origin/main`, so `git fetch` first). `%ct` is
     Unix seconds, so a commit's day is its UTC day; `%cs` would use the committer's own time
     zone, and some commits on main are `+0700` (PR #220 R1.1);
  2. the driver's session index: `--index FILE`, default `ops/logs/index.tsv`. The file is
     git-ignored and exists only in the main checkout, so a session in a worktree passes the
     main checkout's path. A missing file means no session data, and the report says so
     → `raw/index.tsv`;
  3. `gh pr list --state merged --search merged:<since>..<until> --limit 1000 --json number,title,mergedAt,comments`
     → `raw/prs.json`;
  4. `gh issue list --state closed --search closed:<since>..<until> --limit 1000 --json number,title,closedAt,labels`
     → `raw/issues-closed.json`;
  5. `gh issue list --state open --limit 1000 --json number,title,createdAt,labels` → `raw/issues-open.json`.

  Nothing is written to GitHub or git. `--from DIR` reads the same files from `DIR` instead of
  running any command (a saved `raw/` directory, or test fixtures), so a report can be rebuilt
  offline. With `--from`, the index defaults to `DIR/index.tsv` (`--index` still overrides it)
  and may be missing; every other file must exist.
- **Outputs.** `DIR/raw/` (the inputs; not written with `--from`) and `DIR/metrics.md` (the
  report, also printed on stdout). `--out` defaults to `target/metrics`. Exit **0** on success,
  **1** when a command fails or an input doesn't parse, **2** on a usage error.
- **Report sections** (every count is for the period unless it says otherwise):
  1. *Throughput:* merges on main (first-parent commits whose UTC committer date is in the period;
     main merges by squash, so one commit is one PR, plus any direct owner commit); lines added
     and deleted by them, leaving out every `Cargo.lock` and `package-lock.json` and everything
     under `tools/arlington/data/` (lockfiles and generated data, TICK §0 sizing rule; binary
     files have no line counts); merged PRs and closed issues from GitHub; sessions from the
     index (rows whose start stamp `YYYYMMDDTHHMMSSZ` falls in the period): count, non-zero
     exits, total and median duration in whole minutes, and session minutes per merged PR (one
     decimal, rounded down).
  2. *Review (ADR 0010):* for each merged PR with at least one review header, a row with its
     rounds, whether a final check ran, findings by severity for each reviewer, and regrades.
     - Only comments whose body starts (after leading whitespace) with `🤖 jpdf-agent` count;
       the agent and the owner share one account, so the author can't tell them apart.
     - Headers are read from the first line only. Rounds: the highest K in a
       `**AI review, round K` header on that PR. K counts across PRs for one issue (TICK §4),
       so a replacement PR can start above 1.
     - Final check: a `**AI review, round 5 (final check)` header (round 5 only, TICK §4 step 6).
     - Findings: lines in review comments (`**AI review, round K` or
       `**AI review (second reviewer), round K` headers) that start with `R<K>.<n> [<severity>]`
       (subagent) or `S<K>.<n> [<severity>]` (second reviewer), severity one of blocker, major,
       minor, nit. Each ID counts once per PR, at its first severity.
     - Regrades: findings with a line in a triage comment (`**AI review triage, round K`) that
       starts with `<ID>: regraded`, each ID once.
     - Summary: PRs reviewed, median rounds, the share at 4 or more rounds and the share with a
       final check, total findings by severity for each reviewer, total regrades.
  3. *Stuck:* open issues labeled `blocked` and open issues labeled `flaky`, each with its age
     in days from `createdAt` to `--until` (the label's own date isn't in this data).
  4. *Audit and retro lag* (all history, not only the period): each audit record
     (`docs/audits/NNNN.md`) with the date its commit added it and the merges since the
     previous audit record; merges since the last audit record up to the tip of `origin/main`
     (TICK §1's count); each retro record with its date and the days since the previous one;
     days from the last retro record to `--until`.
- **Truncation.** A `gh` list that returns `--limit` (1000) entries may be cut off; the report
  warns for each such list.

Not covered: CI failures by job and a `metrics.json` summary (part 2, #219); review findings
by category (#91 proposed a new journal field; the severities in the posted reviews replace
it); reopened issues, reverts, sessions per task vs the size estimate, judge metric trends, UX
and mutation scores, and regrades overturned by the next reviewer (the retro adds these by
hand). Medians are the upper middle value for an even count.

## Persistence
Writes only under `--out`; each run replaces `metrics.md` and the `raw/` files it fetches.
Nothing else is stored.

## Undo
Not applicable (read-only report; delete `--out` to undo).

## Errors
- Usage (exit 2): an unknown flag, a flag without its value, a date not in `YYYY-MM-DD` form
  or not a real calendar date, an explicit `--since` after `--until`.
- Exit 1, with the failing command or file named on stderr: `git` or `gh` missing or failing,
  a tool printing more than 16 MiB or non-UTF-8 text, a missing required input file, a JSON
  document that doesn't parse or lacks a required field (`number`, `mergedAt`, `closedAt`,
  `createdAt`; a comment without a string `body` is skipped), a `git log` line that doesn't
  follow the fixed format (including a commit time before 1970 or after 9999-12-31), an output
  file that can't be written.
- Index rows that don't have exactly four tab-separated fields (a valid stamp, with a real date
  and a time up to 23:59:59, an integer exit
  code, a session id, a `<seconds>s` duration) are skipped and counted in the report
  ("malformed rows"), since the driver appends to the file and a crash can leave a partial line.

## Limits
- Each command's stdout and stderr are capped at 16 MiB (the `capture` helper shared with
  `ci macos`); past the cap the command is killed and the run fails. No exact-boundary test yet:
  the helper's tests in xtask/src/macos.rs move 300 000 bytes only (audit 0006 AC.2, #261).
- JSON nesting is capped at 128 (tools/jpdf-testgen/src/json.rs, moved from xtask in #235).
- `gh` lists are capped at 1000 entries and report truncation. Boundary test, for each of the
  three lists: 999 entries aren't flagged, exactly 1000 are.
- Arithmetic: counts and line sums use `saturating_add`, durations are `u64` seconds summed
  with `saturating_add`; dates become day numbers in `i64` from years 1–9999 only. No input
  value can panic: every index is checked and every parse returns an error or skips the row.

## Security
GitHub text (PR comments, issue titles) is data written by anyone who can comment; this tool
only counts lines in it and copies issue titles into the markdown report, with `|` replaced and
control characters and U+2028/U+2029 turned into spaces, so a title can't break the list.
- **What could go wrong:** a stranger's comment starting with the agent tag would be counted
  as a review. Accepted: the report is retro input, not a gate, and a forged comment can only
  change numbers in a report that a session reads as data (CONSTITUTION 1).
- **Commands:** fixed argument lists, no shell; the only interpolated values are the two
  validated dates.
- Self-check:
  - Arithmetic on input-derived values: line counts and durations (`saturating_add`), day
    differences (`i64` from bounded years, so no overflow), shares (`saturating_mul`,
    `checked_div`), median (index `len / 2` through `get`).
  - What broken implementation still passes? A finding matcher that also matched delta-review
    restatements (`R1.1 fixed: …`) or a triage line would double-count; the tests include
    both, and they must not add findings. A round counter that read triage headers would pass
    if the triage K were never higher, so the tests give a triage header a higher K than any
    review header. A period filter off by one day is caught by merges, PRs, sessions and closed
    issues dated on the first and last second of the period and one second outside each end
    (a commit at 23:59:59 UTC on `--until` stands for one dated the next day in UTC+7).
  - What the tests can't see: `gh`'s own pagination and search semantics (the `merged:` and
    `closed:` ranges); a live run is checked by hand before review and recorded in the PR.

## Accessibility
Plain markdown tables with header rows; no color.

## Platforms
Runs on macOS and Linux wherever `git` and an authenticated `gh` are on `PATH`. Session data
exists only on the machine running the driver.

## Performance
One `git log` and three `gh` lists. A week took about 20 s locally, nearly all of it `gh pr
list` fetching comments.

## Tests
- Unit tests in xtask/src/metrics.rs: date parsing and day arithmetic (leap years, invalid
  dates, year bounds), option parsing (including `--since` after the default `--until`), git
  log parsing (UTC days from `%ct`, time bounds, excluded paths, binary files, record additions,
  the default `--since`), index rows (malformed rows, impossible times included, skipped and
  counted; period edges), review parsing (tag check, header on the first line only, round K from review headers
  only, final check in round 5 only, finding IDs counted once, restatements and audit IDs ignored, regrades),
  the period filter for merges, PRs and closed issues, stuck ages, lag counts, truncation at
  exactly 1000 for each list, and
  escaping of titles.
- xtask/tests/metrics.rs runs `cargo xtask metrics --from <fixture>` end to end: the report's
  numbers for xtask/tests/fixtures/metrics/, the default `--since`, `--index`, no `raw/` with
  `--from`, exit 1 (malformed and missing `prs.json`) and exit 2 (an impossible date, `--since`
  after a given or the default `--until`, a flag without its value, an unknown flag).
