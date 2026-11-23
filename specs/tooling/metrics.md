# cargo xtask metrics

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; AUTOMATION §11, ops/TICK.md §7 step 1)
Status: implemented

## Behavior
`cargo xtask metrics [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--out DIR] [--from DIR] [--index FILE]`
prints the retro input (AUTOMATION §11) for one period as a markdown report, so a retro takes
its numbers from a command instead of counting by hand (#91, retro 0001; the CI section and
`metrics.json` are part 2, #219).

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
  5. `gh issue list --state open --limit 1000 --json number,title,createdAt,labels` → `raw/issues-open.json`;
  6. `gh run list --workflow ci.yml --created <since>..<until> --limit 1000 --json databaseId,conclusion,event,headBranch,attempt,createdAt`
     → `raw/runs.json`;
  7. for each of the newest 100 failed runs (below), `gh run view <id> --json jobs`
     → `raw/jobs-<id>.json`. `<id>` is the run's `databaseId` parsed as an unsigned integer,
     so the file name is digits only. Each output is written to `raw/` and reduced to its
     failing job names before the next one is fetched, so only one is held in memory.

  Nothing is written to GitHub or git. `--from DIR` reads the same files from `DIR` instead of
  running any command (a saved `raw/` directory, or test fixtures), so a report can be rebuilt
  offline. With `--from`, the index defaults to `DIR/index.tsv` (`--index` still overrides it)
  and may be missing, and so may each `jobs-<id>.json` (that run's jobs count as unknown);
  every other file must exist, `runs.json` included (a `raw/` directory saved before #219 has
  none).
- **Outputs.** `DIR/raw/` (the inputs; not written with `--from`), `DIR/metrics.md` (the
  report, also printed on stdout) and `DIR/metrics.json` (the same numbers as JSON, below). `--out` defaults to `target/metrics`. Exit **0** on success,
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
  3. *CI* (workflow `ci.yml`, runs whose `createdAt` UTC day is in the period; each
     `databaseId` counts once, at its first entry):
     - runs, and runs by `conclusion` (an empty conclusion is shown as `in progress`);
     - failed runs (conclusion `failure` only; `cancelled`, `timed_out` and
       `startup_failure` appear in the conclusion counts but not here) on main (head branch
       `main` and an event other than `pull_request`: pushes, the nightly schedule, manual
       runs) and on other branches;
     - failing jobs: for the newest 100 failed runs (by `createdAt` text, which GitHub writes in
       one fixed `YYYY-MM-DDTHH:MM:SSZ` form, ties by the larger `databaseId`), each job whose
       `conclusion` is `failure`, counted once per run, as a table sorted by count and then
       name. The report says how many runs were read, how many had no job data (`--from`
       without that `jobs-<id>.json`) and whether more than 100 failed runs were left out;
     - runs with `attempt` above 1 and how many of them ended in `success`.

     Infra and real failures are not told apart: `gh run list` shows only a run's latest
     attempt, and a failed job's name doesn't say why it failed. The report says so.
  4. *Stuck:* open issues labeled `blocked` and open issues labeled `flaky`, each with its age
     in days from `createdAt` to `--until` (the label's own date isn't in this data).
  5. *Audit and retro lag* (all history, not only the period): each audit record
     (`docs/audits/NNNN.md`) with the date its commit added it and the merges since the
     previous audit record; merges since the last audit record up to the tip of `origin/main`
     (TICK §1's count); each retro record with its date and the days since the previous one;
     days from the last retro record to `--until`.
- **Truncation.** A `gh` list that returns `--limit` (1000) entries may be cut off; the report
  warns for each such list (merged PRs, closed issues, open issues, CI runs).
- **metrics.json.** One object: `since` and `until` (`YYYY-MM-DD`), `truncated` (the list
  names), `throughput` (`merges`, `lines_added`, `lines_deleted`, `merged_prs`,
  `closed_issues`, `sessions`: `null` without an index, else `count`, `nonzero`,
  `total_seconds`, `median_seconds`, `malformed`), `review` (`prs`, `median_rounds`, `at_cap`,
  `final_checks`, `regrades`, `subagent` and `second` as `{blocker, major, minor, nit}`,
  `per_pr`: one object per reviewed PR), `ci` (`runs`, `by_conclusion` as an object,
  `failed_main`, `failed_other`, `reruns`, `reruns_success`, `jobs_read`, `jobs_unknown`,
  `jobs_capped`, `failing_jobs` as an object of name to count), `stuck` (`blocked` and
  `flaky`, each a list of `{number, title, age_days}`) and `lag` (`audits` and `retros`, each a
  list of `{record, date, since_previous}`, plus `merges_since_audit` and `days_since_retro`).
  A missing value (no median, no previous record, no audit) is `null`. Strings are escaped:
  `"` and `\` with a backslash, every control character and U+2028/U+2029 as `\uXXXX`. Titles
  keep their original text (no `|` replacement).

Not covered: review findings
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
  `createdAt`; in `runs.json` `databaseId`, `createdAt`, `conclusion`, `event`, `headBranch`
  and `attempt`; in a `jobs-<id>.json` the `jobs` array, and `name` for a failed job; a comment
  without a string `body` is skipped), a `git log` line that doesn't
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
- JSON nesting is capped at 128 (xtask/src/json.rs).
- `gh` lists are capped at 1000 entries and report truncation. Boundary test, for each of the
  four lists: 999 entries aren't flagged, exactly 1000 are.
- Job reads are capped at 100 failed runs (at most 100 `gh run view` calls). Boundary test:
  100 failed runs are all read and not flagged; with 101 the newest 100 are read, the oldest
  is left out and the report says so. Peak memory for job data is one command's output
  (16 MiB) plus the failing job names kept so far.
- Arithmetic: counts and line sums use `saturating_add`, durations are `u64` seconds summed
  with `saturating_add`; dates become day numbers in `i64` from years 1–9999 only. No input
  value can panic: every index is checked and every parse returns an error or skips the row.

## Security
GitHub text (PR comments, issue titles, job names, branch names, conclusions) is data written
by anyone who can comment or push a branch; this tool only counts lines in it and copies issue
titles, job names and conclusions into the markdown report, with `|` replaced and control
characters and U+2028/U+2029 turned into spaces, so a value can't break a table or list. In
`metrics.json` every string is escaped (Behavior), so a value can't end its string early.
- **What could go wrong:** a stranger's comment starting with the agent tag would be counted
  as a review. Accepted: the report is retro input, not a gate, and a forged comment can only
  change numbers in a report that a session reads as data (CONSTITUTION 1).
- **Commands:** fixed argument lists, no shell; the only interpolated values are the two
  validated dates and run ids parsed as `u64` (digits only, so `jobs-<id>.json` can't name
  another path).
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
    A job counter that read every failed run's jobs, or the oldest 100, passes a fixture with
    fewer than 101 failed runs, so the cap test checks which run is left out. A main/other
    split on the branch alone passes unless a pull request from a branch named `main` is in
    the fixture; it is. A JSON writer that escaped only quotes passes printable titles, so the
    test round-trips a title with a quote, a backslash, a newline, U+0001 and U+2028 through
    xtask's JSON parser.
  - What the tests can't see: `gh`'s own pagination and search semantics (the `merged:` and
    `closed:` ranges); a live run is checked by hand before review and recorded in the PR.

## Accessibility
Plain markdown tables with header rows; no color.

## Platforms
Runs on macOS and Linux wherever `git` and an authenticated `gh` are on `PATH`. Session data
exists only on the machine running the driver.

## Performance
One `git log`, four `gh` lists and one `gh run view` per failed run read (at most 100). A week
took about 20 s locally before the CI section, nearly all of it `gh pr list` fetching comments.
With it, 2026-11-21 to 2026-11-23 (82 merges, 410 CI runs, 5 failed runs read) took 37 s.

## Tests
- Unit tests in xtask/src/metrics.rs: date parsing and day arithmetic (leap years, invalid
  dates, year bounds), option parsing (including `--since` after the default `--until`), git
  log parsing (UTC days from `%ct`, time bounds, excluded paths, binary files, record additions,
  the default `--since`), index rows (malformed rows, impossible times included, skipped and
  counted; period edges), review parsing (tag check, header on the first line only, round K from review headers
  only, final check in round 5 only, finding IDs counted once, restatements and audit IDs ignored, regrades),
  the period filter for merges, PRs, closed issues and CI runs, the CI counts (conclusions,
  main vs other branches including a pull request from a branch named `main`, re-runs,
  duplicate run ids, failing jobs once per run, missing job data), the 100-run job cap,
  stuck ages, lag counts, truncation at exactly 1000 for each list, escaping of titles, and
  `metrics.json` (it parses, its numbers match the report, and escaped strings round-trip).
- xtask/tests/metrics.rs runs `cargo xtask metrics --from <fixture>` end to end: the report's
  numbers for xtask/tests/fixtures/metrics/ (the CI section and `metrics.json` included), the
  default `--since`, `--index`, no `raw/` with `--from`, exit 1 (malformed and missing `prs.json`) and exit 2 (an impossible date, `--since`
  after a given or the default `--until`, a flag without its value, an unknown flag).
