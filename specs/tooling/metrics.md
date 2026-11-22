# cargo xtask metrics

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; AUTOMATION §11, ops/TICK.md §7 step 1)
Status: implemented

## Behavior
`cargo xtask metrics [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--out DIR] [--from DIR]`
prints the retro input (AUTOMATION §11) for one period as a markdown report, so a retro takes
its numbers from a command instead of counting by hand (#91, retro 0001).

- **Period.** `--since` and `--until` are UTC calendar dates, both inclusive. `--until`
  defaults to today (UTC, from the system clock). `--since` defaults to the committer date of
  the commit that added the last `docs/retros/NNNN.md` (exact four-digit name, so
  `0000-pilot.md` doesn't count) on the first-parent history; with no retro record, to
  `--until` minus 7 days. `--since` after `--until` is a usage error.
- **Inputs.** It runs only read commands, from the current directory (the repo root):
  1. `git log --first-parent --no-renames --format=@@%H%x09%cs%x09%s --numstat --summary origin/main`
     → `raw/git-log.txt`;
  2. `ops/logs/index.tsv` (git-ignored, written by the driver; a missing file means no session
     data, and the report says so) → `raw/index.tsv`;
  3. `gh pr list --state merged --search "merged:<since>..<until>" --limit 1000 --json number,title,mergedAt,comments`
     → `raw/prs.json`;
  4. `gh issue list --state closed --search "closed:<since>..<until>" --limit 1000 --json number,title,closedAt,labels`
     → `raw/issues-closed.json`;
  5. `gh issue list --state open --limit 1000 --json number,title,createdAt,labels` → `raw/issues-open.json`;
  6. `gh run list --workflow ci.yml --created <since>..<until> --limit 1000 --json databaseId,conclusion,event,headBranch,attempt,createdAt`
     → `raw/runs.json`;
  7. for each of the 100 newest runs whose conclusion is `failure`: `gh run view <id> --json jobs`
     → `raw/jobs-<id>.json`.

  Nothing is written to GitHub or git. `--from DIR` reads the same files from `DIR` instead of
  running any command (a saved `raw/` directory, or test fixtures), so a report can be rebuilt
  offline. With `--from`, a missing `jobs-<id>.json` means "jobs unknown" for that run, and a
  missing `index.tsv` means no session data; every other file must exist.
- **Outputs.** `DIR/raw/` (the inputs, not written with `--from`), `DIR/metrics.md` (the report,
  also printed on stdout) and `DIR/metrics.json` (the same numbers). `--out` defaults to
  `target/metrics`. Exit **0** on success, **1** when a command fails or an input doesn't parse,
  **2** on a usage error.
- **Report sections** (every count is for the period unless it says otherwise):
  1. *Throughput:* merges on main (first-parent commits whose committer date is in the period;
     main merges by squash, so one commit is one PR); lines added and deleted by them, leaving
     out `Cargo.lock`, any `package-lock.json` and `tools/arlington/data/` (lockfiles and
     generated data, TICK §0 sizing rule; binary files have no line counts); merged PRs and
     closed issues from GitHub; sessions from `index.tsv` (rows whose start stamp
     `YYYYMMDDTHHMMSSZ` falls in the period): count, non-zero exits, total and median duration,
     and session minutes per merged PR.
  2. *Review (ADR 0010):* for each merged PR with at least one review header, a row with its
     rounds, whether a final check ran, findings by severity for each reviewer, and regrades.
     - Only comments whose body starts (after leading whitespace) with `🤖 jpdf-agent` count;
       the agent and the owner share one account, so the author can't tell them apart.
     - Rounds: the highest K in a `**AI review, round K` header on that PR. K counts across
       PRs for one issue (TICK §4), so a replacement PR can start above 1.
     - Final check: a `**AI review, round 5 (final check)` header.
     - Findings: lines in review comments (`**AI review, round K` or
       `**AI review (second reviewer), round K` headers) that start with `R<K>.<n> [<severity>]`
       (subagent) or `S<K>.<n> [<severity>]` (second reviewer), severity one of blocker, major,
       minor, nit. Each ID counts once per PR, at its first severity.
     - Regrades: lines in triage comments (`**AI review triage, round K`) that start with
       `<ID>: regraded`.
     - Summary: PRs reviewed, median rounds, the share at 4 or more rounds and the share with a
       final check, total findings by severity for each reviewer, total regrades.
  3. *CI:* runs of the `ci` workflow created in the period, by conclusion; failed runs on main
     (head branch `main`) and on other branches; failing job names with their counts over the
     runs whose jobs were read; runs with `attempt` above 1 (re-run) and how many of those
     ended in success. `gh run list` reports only the latest attempt, so a failure followed by
     a passing re-run shows as a success with attempt 2: infra vs real failure is not
     classified, and the report says so.
  4. *Stuck:* open issues labeled `blocked` and open issues labeled `flaky`, each with its age
     in days from `createdAt` to `--until` (the label's own date isn't in this data).
  5. *Audit and retro lag* (all history, not only the period): each audit record
     (`docs/audits/NNNN.md`) with the date its commit added it and the merges since the
     previous audit record; merges since the last audit record up to the tip of `origin/main`
     (TICK §1's count); each retro record with its date and the days since the previous one;
     days from the last retro record to `--until`.
- **Truncation.** A `gh` list that returns exactly its `--limit` entries may be cut off; the
  report says so for each such list. When more than 100 runs failed, the job counts cover the
  100 newest, and the report says so.

Not covered (left for later issues or for the retro to add by hand): review findings by
category (the issue proposed a journal field; severity from the posted reviews replaces it),
reopened issues, reverts, sessions per task vs the size estimate, judge metric trends, UX and
mutation scores, and regrades overturned by the next reviewer.

## Persistence
Writes only under `--out`; each run replaces `metrics.md`, `metrics.json` and the `raw/` files
it fetches. Nothing else is stored.

## Undo
Not applicable (read-only report; delete `--out` to undo).

## Errors
- Usage (exit 2): an unknown flag, a flag without its value, a date not in `YYYY-MM-DD` form
  or not a real calendar date, `--since` after `--until`.
- Exit 1, with the failing command or file named on stderr: `git` or `gh` missing or failing,
  a tool printing more than 16 MiB or non-UTF-8 text, a JSON document that doesn't parse or
  lacks a required field (`number`, `mergedAt`/`closedAt`/`createdAt`, `databaseId`,
  `conclusion`, `headBranch`, `attempt`; a comment without a string `body` is skipped), a
  `git log` line that doesn't follow the fixed format, an output file that can't be written.
- `index.tsv` rows that don't have four tab-separated fields with a valid stamp, an integer
  exit code and a `<seconds>s` duration are skipped and counted in the report ("malformed
  rows"), since the driver appends to it and a crash can leave a partial line.

## Limits
- Each command's stdout and stderr are capped at 16 MiB (the `capture` helper shared with
  `ci macos`); past the cap the command is killed and the run fails. Boundary: the helper's own
  tests in xtask/src/macos.rs.
- JSON nesting is capped at 128 (xtask/src/json.rs).
- `gh` lists are capped at 1000 entries and job reads at 100 runs (above); both report
  truncation. Boundary tests: a list with exactly 1000 entries is flagged, 999 is not; 101
  failed runs read 100 job files, 100 read all.
- Arithmetic: counts and line sums use `saturating_add`, durations are `u64` seconds; dates are
  converted to day numbers in `i64` with inputs limited to years 1–9999. No input value can
  panic: every index is checked and every parse returns an error or skips the row.

## Security
GitHub text (PR comments, issue titles) is data written by anyone who can comment; this tool
only counts lines in it and copies issue titles into the markdown report (titles are escaped:
`|` and newlines are replaced so a title can't break the table).
- **What could go wrong:** a stranger's comment starting with the agent tag would be counted
  as a review. Accepted: the report is retro input, not a gate, and the owner's account is the
  only one that writes agent text (CONSTITUTION 1). A forged comment can only change numbers in
  a report a session reads as data.
- **Commands:** fixed argument lists, no shell; the only interpolated values are the two
  validated dates and run ids parsed as integers.
- Self-check:
  - Arithmetic on input-derived values: line counts (`saturating_add`), durations (parsed as
    `u64`, summed with `saturating_add`), day differences (`i64` from bounded years, so no
    overflow), median (index `len / 2` on a non-empty sorted list).
  - What broken implementation still passes? A finding regex that also matched delta-review
    restatements (`R1.1 fixed: …`) would double-count; the tests include such lines and a
    triage comment, which must not add findings. A round counter that read triage headers
    would match the same K and pass, so the tests give a triage header a higher K than any
    review header. A window filter off by one day is caught by commits, sessions and runs
    dated exactly on `--since`, exactly on `--until`, and one day outside each.
  - What the tests can't see: `gh`'s own pagination and search semantics (the `merged:` and
    `--created` ranges); a live run is checked by hand before review and recorded in the PR.

## Accessibility
Plain markdown tables with header rows; no color.

## Platforms
Runs on macOS and Linux wherever `git` and an authenticated `gh` are on `PATH`. Session data
exists only on the machine running the driver.

## Performance
Seven commands plus one `gh run view` per failed run (at most 100). A week takes a few seconds,
dominated by `gh`.

## Tests
- Unit tests in xtask/src/metrics.rs: date parsing and day arithmetic (leap years, invalid
  dates, year bounds), git log parsing (excluded paths, binary files, audit/retro additions),
  index.tsv rows (malformed rows skipped and counted, window edges), review parsing (tag
  check, round K from review headers only, final check, finding IDs counted once, restatements
  and audit IDs ignored, regrades), CI aggregation (main vs branches, reruns, job counts,
  missing job files), stuck ages, lag counts, truncation at exactly 1000 and the 100-run job
  cap, and markdown escaping of titles.
- xtask/tests/metrics.rs runs `cargo xtask metrics --from <fixture>` end to end: exit codes 0, 1
  (malformed JSON) and 2 (bad date, `--since` after `--until`), the written files, and the
  report's numbers for xtask/tests/fixtures/metrics/.
