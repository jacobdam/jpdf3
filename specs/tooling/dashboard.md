# Metrics dashboard snapshot (process metrics and the milestone forecast)

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (AUTOMATION §2, §3, §5.4; ops/judge/CONTRACT.md §2; #15 part 1, #305; the page is #353)
Status: implemented

## Behavior
The metrics dashboard of AUTOMATION §2 comes in two parts, because the CI job that builds the
Pages site (`project-linux` in `.github/workflows/ci.yml`) has neither a GitHub token nor git
history (a shallow checkout without credentials):

1. **Snapshot** (this spec; run by a session, with `gh` and `git`):
   `cargo xtask dashboard [--until YYYY-MM-DD] [--out FILE] [--from DIR] [--index FILE]`
   writes one JSON file, default `ops/metrics/dashboard.json`, which is committed. The retro
   refreshes it (ops/TICK.md §7 step 1), so it is at most about a week old; its `generated`
   field says when it was taken.
2. **Page** (#353, not built yet): a static page in the Pages site that renders the committed
   snapshot. Until it lands, the snapshot is read as a file.

### Snapshot inputs
Read-only commands, run from the current directory (the repo root):
- **Weeks.** Twelve ISO weeks (Monday to Sunday, UTC days), the last one being the week that
  contains `--until` (default: today, UTC). The last week is partial when `--until` isn't a
  Sunday (its `end` is `--until`).
- `gh pr list --state merged --search merged:<monday>..<end> --limit 1000 --json number,mergedAt,comments`
  once per week (`<end>` is the Sunday, or `--until` for the last week), so one output stays far
  below the 16 MiB capture cap (a day of merges is about 0.2 MB) → `prs-<monday>.json`.
- `gh issue list --state all --limit 1000 --json number,title,state,createdAt,closedAt,labels,milestone`
  → `issues.json`.
- `git log --first-parent --diff-filter=A --name-only --format=@@%ct%x09%s origin/main -- ops/journal/`
  → `journal-log.txt` (the local `origin/main`, so `git fetch` first).
- the driver's session index, `--index FILE` (default `ops/logs/index.tsv`; with `--from`,
  `DIR/index.tsv`). Missing → no session data.

`--from DIR` reads those files from `DIR` instead of running anything (tests, offline rebuild);
every file but the index must exist. Live runs don't save their inputs.

### Snapshot fields
`{"version": 1, "generated", "until", "weeks", "sessions", "journal_entries", "blocked",
"milestones", "truncated", "malformed_index_rows"}`; dates are `YYYY-MM-DD`.
- `weeks`: twelve objects, oldest first: `start` (the Monday), `end` (the Sunday, or `--until`),
  `merged_prs` (PRs whose `mergedAt` UTC day is in the week and on or before `--until`),
  `median_rounds` (the median of the review rounds of those PRs that have at least one review
  header, as `cargo xtask metrics` counts them, specs/tooling/metrics.md §Review; `null` when
  none has one), `sessions` (index rows whose start stamp is in the week, as metrics counts
  them; `null` without an index).
- `sessions`: `"index"` or `"missing"`.
- `journal_entries`: sessions per task. The index has no task column, so this counts, per
  merged PR, the session journal files its squash commit added (`ops/journal/YYYY-MM-DD/*.md`,
  one per session since retro 0002, ops/TICK.md §5: each session records into its work PR).
  Commits whose subject doesn't end in `(#N)` and the older per-day `ops/journal/YYYY-MM-DD.md`
  files don't count. Only commits on or before `--until` count, from the earliest twelve-week
  Monday on: `prs` (commits with at least one entry), `median`, `max` (`null` with none) and
  `histogram`, four objects `{"entries": 1|2|3|4, "prs": n}`, where 4 means 4 or more.
- `blocked`: open issues labeled `blocked`, `{number, title, age_days}`, `age_days` from
  `createdAt` to `--until` (never below 0), oldest first, then by number.
- `milestones`: one object per milestone title that any issue in `issues.json` carries, in the
  order `M<n>` by number, then the others by title: `title`, `open`, `closed`, `closed_28d`
  (closed issues whose `closedAt` day is in the 28 days ending at `--until`) and `forecast`: with
  `open` > 0 and `closed_28d` > 0, `--until` plus ⌈open × 28 / closed_28d⌉ days (the trailing
  4-week rate, ops/TICK.md §7 step 5); else `null` (nothing open, or no closures in 28 days:
  `open` and `closed_28d` tell which). Issues without a milestone are left out.
  The weekly digest's "slip > 25%" needs a target date, which no milestone has yet; the digest
  keeps it.
- `truncated`: the names of the lists that returned exactly 1000 entries (`prs-<monday>` or
  `issues`), which may be cut off.
- `malformed_index_rows`: index rows metrics would skip as malformed (0 without an index).

Strings are escaped as in metrics.json (the shared `json_str`).

## Persistence
`ops/metrics/dashboard.json` is a committed file, replaced by each snapshot run.

## Undo
Not applicable (a generated report; `git checkout` the previous snapshot).

## Errors
- `cargo xtask dashboard`: exit 2 on a usage error (an unknown flag, a flag without a value, a
  date not `YYYY-MM-DD`); exit 1, with the command or file named, when `gh` or `git` is missing
  or fails, an output passes the 16 MiB cap, a required input file is missing, a JSON list
  doesn't parse or lacks a required field (`number`, `mergedAt`; in `issues.json` `number`,
  `title`, `state`, `createdAt`, and `closedAt` when `state` is `CLOSED`; timestamps exactly as
  metrics parses them), a `journal-log.txt` header line isn't `@@<digits>\t<subject>`, or the
  output can't be written. A `milestone` that isn't an object with a string `title` counts as no
  milestone.

## Limits
- Each command's stdout and stderr: 16 MiB (`macos::capture`, boundary-tested there).
- `gh` lists: 1000 entries (`metrics::LIST_LIMIT`); exactly 1000 is flagged in `truncated`,
  999 is not (test). Per-PR comments: metrics' limit of 100 applies to the rounds; the dashboard
  doesn't repeat metrics' warning for it (the retro's metrics run has it).
- The snapshot of 2026-11-25 is under 3 KB.
- Arithmetic: counts use `saturating_add`; the forecast is ⌈open × 28 / closed_28d⌉ with
  `checked_mul` (open ≤ 1000 by the list limit, so the product fits), and a forecast past
  9999-12-31 is `null`. Week starts are day numbers in `i64` from validated dates.

## Security
- **GitHub text is data** (CONSTITUTION 1): issue titles go into the snapshot escaped by
  `json_str`, so a title can't end its string early or add a field. The page (#353) must treat
  the whole snapshot as untrusted input.
  Review headers are counted as metrics counts them; a forged tagged comment can only change a
  number in the snapshot (accepted, as in metrics).
- **Commands**: fixed argument lists, no shell; the only interpolated values are validated
  dates.
- Self-check:
  - Arithmetic: above (Limits).
  - What broken implementation still passes? A snapshot that counts every PR in every week:
    the tests put PRs in different weeks and one after `--until`. A forecast that uses all
    closures instead of 28 days: a closure on day −28 (outside) and one on day −27 (inside).
    A snapshot that writes 0 for a missing value: the tests assert `null` medians for weeks
    without a reviewed PR and `null` sessions without an index.
  - Filters, each with a reject and a near-miss accept: the week bounds (the Monday and the Sunday
    in; the next Monday out), `--until` (that day in, the next out), the 28-day window (day
    −27 in, day −28 out), the `blocked` label (open in, closed out, `blocked-x` out), journal
    paths (`ops/journal/2026-11-24/x.md` in, `ops/journal/2026-11-24.md` out), the `(#N)`
    subject (at the end in, `(#N) more` out).
  - A run where nothing was checked: an empty `issues.json` and no PRs gives zero counts and
    `null` medians, which is the truth; a missing required input fails the run (test).
  - Parser of another tool's output: `journal-log.txt` lines are the shapes git prints with
    that format (header, blank, path); a test pins the arguments of the command run.
- Not visible to these tests: live `gh` output (the tests use `--from` and in-memory inputs;
  the command arguments are pinned by a test).

## Accessibility
Not applicable to the snapshot; the page's is #353's.

## Platforms
Any machine with `gh` and `git` (a session's).

## Performance
Thirteen `gh` calls and one `git log`, about 20 s live (2026-11-25).

## Tests
`xtask/src/dashboard.rs` unit tests: week bounds, `--until`, medians and `null`, sessions with
and without an index, journal entry counting and its histogram, blocked ages, milestone order
and forecast (window ends, `open` 0, `closed_28d` 0, past 9999-12-31), truncation at 999/1000,
the pinned command arguments, bad inputs, flags, and a `--from` run end to end (missing inputs
fail, a missing index doesn't).
