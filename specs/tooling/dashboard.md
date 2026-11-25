# Metrics dashboard (part 1: process metrics and the milestone forecast)

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (AUTOMATION §2, §3, §5.4; ops/judge/CONTRACT.md §2; #15 part 1, #305)
Status: implemented

## Behavior
Two halves, because the CI job that builds the Pages site (`project-linux` in
`.github/workflows/ci.yml`) has neither a GitHub token nor git history (a shallow checkout
without credentials):

1. **Snapshot** (run by a session, with `gh` and `git`):
   `cargo xtask dashboard [--until YYYY-MM-DD] [--out FILE] [--from DIR] [--index FILE]`
   writes one JSON file, default `ops/metrics/dashboard.json`, which is committed. The retro
   refreshes it (ops/TICK.md §7 step 1), so the page is at most about a week old; it shows its
   `generated` date.
2. **Page** (built by `cargo xtask ci linux`): the web app's Vite build has a second entry,
   `apps/web/dashboard/index.html` (React + MUI, `apps/web/src/dashboard/`), so the page gets
   the web app's CSP meta tag and toolchain (TypeScript strict, Biome, the license check). After
   the web build copied `dist/` to `DIR/site/`, `ci linux` copies the committed snapshot to
   `DIR/site/dashboard/data.json`. Pages serves it at `<site>/dashboard/`. The page fetches
   `./data.json` (same origin, `connect-src 'self'`) and renders it.

### Snapshot inputs
Read-only commands, run from the current directory (the repo root):
- **Weeks.** Twelve ISO weeks (Monday to Sunday, UTC days), the last one being the week that
  contains `--until` (default: today, UTC). The last week is partial when `--until` isn't a
  Sunday; the page labels it `to <until>`.
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
  4-week rate, ops/TICK.md §7 step 5); else `null`. The page shows `done` for `open` = 0 and
  `no closures in 4 weeks` for `closed_28d` = 0. Issues without a milestone are left out.
  The weekly digest's "slip > 25%" needs a target date, which no milestone has yet; the digest
  keeps it.
- `truncated`: the names of the lists that returned exactly 1000 entries (`prs-<monday>` or
  `issues`), which may be cut off. The page shows a warning listing them.
- `malformed_index_rows`: index rows metrics would skip as malformed (0 without an index).

Strings are escaped as in metrics.json (the shared `json_str`).

### Page
Sections, each a heading level 2: *Merged PRs per week*, *Median review rounds per week*,
*Sessions per week*, *Sessions per merged PR*, *Blocked issues*, *Milestone forecast*. Each
chart is an SVG bar chart (`role="img"`, an `aria-label` naming the series) followed by a table
holding the same numbers, so a screen reader and a keyboard user get every value. A missing
value is shown as `missing` (no bar), never as 0: a `null` week value, a missing session index
(*Sessions per week* then says `Missing: the snapshot has no session index.` instead of a chart),
a missing or invalid snapshot (the page says `No metrics snapshot: data.json is missing or
invalid.` and shows no section).

The page validates `data.json` before rendering: `version` must be 1 and every field has its
type (counts are non-negative integers, dates match `YYYY-MM-DD`); any other document is the
"missing or invalid" state. Text from the snapshot (issue titles) is rendered as React text
nodes only.

## Persistence
`ops/metrics/dashboard.json` is a committed file, replaced by each snapshot run. The site is
rebuilt on every push to main.

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
- `ci linux`: a snapshot that is missing is skipped (the page shows its missing state, and
  `ci linux` prints `dashboard: no snapshot`); one that isn't a regular file, is larger than
  1 MiB or can't be copied fails `ci linux` (exit 1). No `site/dashboard/` (the web build
  failed) → nothing is copied; the web failure is already reported.

## Limits
- Each command's stdout and stderr: 16 MiB (`macos::capture`, boundary-tested there).
- `gh` lists: 1000 entries (`metrics::LIST_LIMIT`); exactly 1000 is flagged in `truncated`,
  999 is not (test). Per-PR comments: metrics' limit of 100 applies to the rounds; the dashboard
  doesn't repeat metrics' warning for it (the retro's metrics run has it).
- Snapshot copy: 1 MiB; exactly 1 MiB is copied, 1 MiB + 1 fails (test). The snapshot of
  2026-11-25 is under 10 KB.
- Arithmetic: counts use `saturating_add`; the forecast is ⌈open × 28 / closed_28d⌉ with
  `checked_mul` (open ≤ 1000 by the list limit, so the product fits), and a forecast past
  9999-12-31 is `null`. Week starts are day numbers in `i64` from validated dates.
- The page: a snapshot larger than 1 MiB is never served (the copy refuses it); the page renders
  at most what the snapshot holds (twelve weeks, at most 1000 issues).

## Security
- **GitHub text is data** (CONSTITUTION 1): issue titles go into the snapshot escaped by
  `json_str` and onto the page as React text (no `dangerouslySetInnerHTML`, which Biome forbids).
  Review headers are counted as metrics counts them; a forged tagged comment can only change a
  number on the page (accepted, as in metrics).
- **CSP**: the page is built by Vite with the web app's meta CSP (`script-src 'self'`,
  `connect-src 'self'`, no inline script, Trusted Types `'none'`). Its only request is
  `./data.json`.
- **Commands**: fixed argument lists, no shell; the only interpolated values are validated
  dates.
- Self-check:
  - Arithmetic: above (Limits).
  - What broken implementation still passes? A snapshot that counts every PR in every week:
    the tests put PRs in different weeks and one after `--until`. A forecast that uses all
    closures instead of 28 days: a closure on day −28 (outside) and one on day −27 (inside).
    A page that shows `null` as 0: the Playwright test asserts `missing` for a `null` week and
    no session chart without an index. A page that renders any JSON: a `version` 2 snapshot,
    a negative count and a malformed date each give the invalid state.
  - Filters, each with a reject and a near-miss accept: the week bounds (Monday 00:00 and Sunday
    in; the next Monday out), `--until` (that day in, the next out), the 28-day window (day
    −27 in, day −28 out), the `blocked` label (open in, closed out, `blocked-x` out), journal
    paths (`ops/journal/2026-11-24/x.md` in, `ops/journal/2026-11-24.md` out), the `(#N)`
    subject (at the end in, `(#N) more` out).
  - A run where nothing was checked: an empty `issues.json` and no PRs gives zero counts and
    `null` medians; the page then shows bars of 0 and `missing` medians, which is the truth.
  - Parser of another tool's output: `journal-log.txt` lines are the shapes git prints with
    that format (header, blank, path); a test pins the arguments of the command run.
- Not visible to these tests: the real deploy (Pages); the CI assembly step copies
  `e2e-linux/site/` as a whole, so `site/dashboard/` ships when `ci linux` wrote it.

## Accessibility
Every chart has an `aria-label` and a table with the same values; text and bars meet 4.5:1
(text) and 3:1 (bars) against the background in light and dark mode (`prefers-color-scheme`),
checked by the Playwright test from computed colors. Headings are in order (h1, then h2).

## Platforms
Web (GitHub Pages) only.

## Performance
Not measured: twelve bars per chart and at most 1000 table rows.

## Tests
- `apps/web/scripts/licenses.test.mjs`: the license check follows the dashboard page (the one
  allowed Vite `input`, specs/tooling/licenses.md item 5) and still refuses any other `input`.
- `xtask/src/dashboard.rs` unit tests: week bounds, `--until`, medians and `null`, sessions
  with and without an index, journal entry counting and its histogram, blocked ages, milestone
  order and forecast (window ends, `open` 0, `closed_28d` 0), truncation at 999/1000, the
  `--from` command shapes and arguments, the snapshot copy at 1 MiB and 1 MiB + 1, a missing
  snapshot.
- `apps/web/e2e/dashboard.spec.ts` (Playwright, no journey ID: the run as a whole still fails
  `ci linux` when it fails): a full snapshot (every section, values, the partial week label,
  the truncation warning), `null` values as `missing`, a snapshot without an index, an invalid
  snapshot (version, negative or fractional count, bad dates, an unknown sessions source, a
  non-text title, an array) and a 404, the committed `ops/metrics/dashboard.json` rendering
  without the invalid state (so the writer and the validator agree), contrast in light and dark mode, and
  the shared fixture checks (no console errors, CSP violations or foreign requests).
