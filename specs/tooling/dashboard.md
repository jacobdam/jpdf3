# Metrics dashboard (process metrics, the milestone forecast and the judge's product metrics)

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (AUTOMATION §2, §3, §5.4; ops/judge/CONTRACT.md §2; #15 part 1; #305, the
snapshot; #353, the page; #306, the judge's product metrics)
Status: implemented

## Behavior
The metrics dashboard of AUTOMATION §2 comes in two parts, because the CI job that builds the
Pages site (`project-linux` in `.github/workflows/ci.yml`) has neither a GitHub token nor git
history (a shallow checkout without credentials):

1. **Snapshot** (run by a session, with `gh` and `git`):
   `cargo xtask dashboard [--until YYYY-MM-DD] [--out FILE] [--from DIR] [--index FILE]`
   writes one JSON file, default `ops/metrics/dashboard.json`, which is committed. The retro
   refreshes it (ops/TICK.md §7 step 1), so it is at most about a week old; its `generated`
   field says when it was taken.
2. **Page** (built by `cargo xtask ci linux`): the web app's Vite build has a second entry,
   `apps/web/dashboard/index.html` (React + MUI, `apps/web/src/dashboard/`), so the page gets
   the web app's CSP meta tag and toolchain (TypeScript strict, Biome, the license check). After
   the web build copied `dist/` to `DIR/site/`, `ci linux` copies the committed snapshot to
   `DIR/site/dashboard/data.json`. Pages serves it at `<site>/dashboard/`. The page fetches
   `./data.json` (same origin, `connect-src 'self'`) and renders it.
3. **Judge metrics** (#306): the product metrics come from two places, because the deployed
   site holds only the current judge report. The `pages` job runs on pushes to main only and
   copies that run's `judge-report` artifact to `site/judge/`; a push run's artifact has only
   `report/report.json` and `report/summary.md` (the smoke tier; the exam report and
   `gates/gates.json` are nightly-only and never reach the site). So:
   - the **latest run**: the page also fetches `../judge/report/report.json` (same origin) and
     shows its smoke metrics and journeys passing per milestone;
   - the **trend**: the snapshot run reads the nightly runs' `gates-report` artifacts (each a
     single `gates.json`, kept 90 days, so the twelve weeks fit) into the snapshot's `judge`
     field. `gates.json` holds aggregates only (CONSTITUTION 13); the snapshot takes its
     `smoke_*` metrics, not the `exam_*` ones.

### Snapshot inputs
Read-only commands, run from the current directory (the repo root):
- **Weeks.** Twelve ISO weeks (Monday to Sunday, UTC days), the last one being the week that
  contains `--until` (default: today, UTC). The last week is partial when `--until` isn't a
  Sunday (its `end` is `--until`); the page labels it `to <until>`.
- `gh pr list --state merged --search merged:<monday>..<end> --limit 1000 --json number,mergedAt,comments`
  once per week (`<end>` is the Sunday, or `--until` for the last week), so one output stays far
  below the 16 MiB capture cap (a day of merges is about 0.2 MB) → `prs-<monday>.json`.
- `gh issue list --state all --limit 1000 --json number,title,state,createdAt,closedAt,labels,milestone`
  → `issues.json`.
- `git log --first-parent --diff-filter=A --name-only --format=@@%ct%x09%s origin/main -- ops/journal/`
  → `journal-log.txt` (the local `origin/main`, so `git fetch` first).
- `gh run list --workflow ci.yml --event schedule --branch main --status completed --limit 200 --json databaseId,createdAt`
  → `nightly-runs.json` (`NIGHTLY_LIMIT`, 200: about 28 weeks of daily nightlies, newest first).
- For each week, its nightly runs newest first (`createdAt` UTC day in the week; ties by the
  larger `databaseId`), at most 7 (`MAX_NIGHTLY_TRIES`): `gh run download <id> -n gates-report
  -D <repo>/target/dashboard-gates/<id>` until one succeeds, then `gates.json` from that
  directory → `gates-<id>.json`. A failed download (the artifact expired, the gate step didn't
  run, or a network error) prints `dashboard: run <id>: no gates-report (<first stderr line>)`
  on stderr and tries the next run; the directory `target/dashboard-gates/` is removed before
  and after. With `--from`, `nightly-runs.json` is optional (missing → the history's source is
  `missing`), and a missing `gates-<id>.json` is a failed download.
- the driver's session index, `--index FILE` (default `ops/logs/index.tsv`; with `--from`,
  `DIR/index.tsv`). Missing → no session data.

`--from DIR` reads those files from `DIR` instead of running anything (tests, offline rebuild);
every file but the index must exist. Live runs don't save their inputs.

### Snapshot fields
`{"version": 1, "generated", "until", "weeks", "sessions", "journal_entries", "blocked",
"milestones", "judge", "truncated", "malformed_index_rows"}`; dates are `YYYY-MM-DD`.
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
  `open` and `closed_28d` tell which; the page shows `done` for `open` = 0,
  `no closures in 4 weeks` for `closed_28d` = 0, and `missing` for a `null` forecast with
  both above 0, the past-9999-12-31 case). Issues without a milestone are left out.
  The weekly digest's "slip > 25%" needs a target date, which no milestone has yet; the digest
  keeps it.
- `judge`: `{"source": "nightly" | "missing", "weeks": [...]}`, twelve objects with the same
  `start` dates as `weeks`, oldest first: `start`, `run` (the `databaseId` whose `gates.json`
  was read, or `null` when no run of the week had one), `day` (that run's `createdAt` UTC day
  or `null`), and from that file's `metrics` object `open_rate`, `render_match`, `text_f1`
  (`smoke_open_rate`, `smoke_render_match`, `smoke_text_f1`: numbers from 0 to 1), `crashes`,
  `journeys_passing`, `journeys` (`smoke_crashes`, `smoke_journeys_passing`, `smoke_journeys`:
  counts). A key that is absent or `null` in `gates.json` (an older `gate.py`, a metric the
  judge couldn't measure) is `null`, never 0; every value is `null` in a week with `run`
  `null`. `source` is `missing` (every week `null`) only for a `--from` directory without
  `nightly-runs.json`. A snapshot written before #306 has no `judge` field; the page treats
  that as `missing` too.
- `truncated`: the names of the lists that returned exactly their limit (`prs-<monday>` or
  `issues` at 1000, `nightly-runs` at 200), which may be cut off. The page shows a warning listing them.
- `malformed_index_rows`: index rows metrics would skip as malformed (0 without an index).

Strings are escaped as in metrics.json (the shared `json_str`).

### Page
Sections, each a heading level 2: *Merged PRs per week*, *Median review rounds per week*,
*Sessions per week*, *Sessions per merged PR*, *Blocked issues*, *Milestone forecast*,
*Judge metrics, latest main run*, *Judge metrics per week*. Each
chart is an SVG bar chart (`role="img"`, an `aria-label` naming the series) followed by a table
holding the same numbers, so a screen reader and a keyboard user get every value. A missing
value is shown as `missing` (no bar), never as 0: a `null` week value, a missing session index
(*Sessions per week* then says `Missing: the snapshot has no session index.` instead of a chart),
a missing or invalid snapshot (the page says `No metrics snapshot: data.json is missing or
invalid.` and shows no section).

The page validates `data.json` before rendering: `version` must be 1 and every field has its
type (counts are non-negative integers, dates match `YYYY-MM-DD`), with exactly twelve weeks of
distinct `start` dates and four histogram objects whose `entries` are 1, 2, 3, 4 in order; any
other document is the "missing or invalid" state. Text from the snapshot (issue titles) is
rendered as React text nodes only. The snapshot frames the page: in its missing or invalid
state no section is shown, the two judge sections included.

*Judge metrics per week* has five charts (*Smoke open rate per week*, *Smoke render match per
week*, *Smoke text F1 per week*, *Smoke crashes per week*, *Journeys passing per week*), each
labeled by week as the other charts are; a week without a nightly report is `missing`. With
no `judge` field or `source` `missing` the section says `Missing: the snapshot has no judge
history.` instead. The validator accepts a `judge` field only with `source` `nightly` or
`missing`, exactly twelve weeks whose `start` equals `weeks[i].start`, `run` a count or
`null`, `day` a date or `null`, rates numbers from 0 to 1 or `null`, and counts or `null`;
anything else makes the whole snapshot invalid, as for the other fields.

*Judge metrics, latest main run* reads `../judge/report/report.json` (the deployed judge
report of the newest push to main). It accepts only an object with `tier` `smoke`, a `metrics`
object whose `open_rate`, `render_match`, `text_f1` are numbers from 0 to 1 or `null` and whose
`crashes` is a count, and a `journeys` object whose every value has a string `milestone` and
booleans `pass` and `superseded`; a response over 4 MiB (4 194 304 UTF-16 code units of text)
is not parsed. It shows a table *Smoke corpus, latest run* (Metric, Value: open rate, render
match, text F1, crashes; `null` as `missing`) and a table *Journeys passing per milestone*
(Milestone, Passing, Journeys), counting journeys that aren't superseded, `M<n>` by number
first, then other titles in code-point order. While it loads, the section says `Loading the
judge report…`; a non-2xx answer or a network error gives `Missing: this site has no judge
report.`, and any other document `Invalid: the judge report doesn't have the expected
format.` (the rest of the page is unaffected).

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
- `cargo xtask dashboard`, judge history: exit 1 when `nightly-runs.json` doesn't parse or an
  entry lacks `databaseId` or a `createdAt` timestamp; when a downloaded artifact has no
  `gates.json`, it is not a regular file or it is larger than 1 MiB (`MAX_GATES_BYTES`); or when
  a `gates.json` doesn't parse, has no `metrics` object, or has one of the six keys with
  another type (a rate outside 0..1 or not finite, a count that isn't a non-negative integer).
  A failed download is not an error (see Snapshot inputs).
- `ci linux`: a missing snapshot or a missing `site/dashboard/` (the web build failed; that
  failure is already reported) copies nothing and prints `dashboard: no snapshot, nothing copied`
  or `dashboard: no page, nothing copied`; the page then shows its missing state. A snapshot
  that isn't a regular file, is larger than 1 MiB or can't be copied fails `ci linux` (exit 1).

## Limits
- Each command's stdout and stderr: 16 MiB (`macos::capture`, boundary-tested there).
- `gh` lists: 1000 entries (`metrics::LIST_LIMIT`); exactly 1000 is flagged in `truncated`,
  999 is not (test). Per-PR comments: metrics' limit of 100 applies to the rounds; the dashboard
  doesn't repeat metrics' warning for it (the retro's metrics run has it).
- Nightly runs: 200 (`NIGHTLY_LIMIT`; exactly 200 is flagged in `truncated`, 199 is not, test);
  per week at most 7 downloads (`MAX_NIGHTLY_TRIES`: the 7th run is tried, an 8th isn't, test),
  so at most 84 downloads; each `gates.json` at most 1 MiB (`MAX_GATES_BYTES`; exactly 1 MiB is
  read, 1 MiB + 1 fails, test). gates.json of 2026-11-26 is 13 KB.
- Judge report on the page: 4 MiB of text before `JSON.parse` (the report of 2026-11-27 is
  25 KB; a 4 MiB + 1 body gives the invalid state, test).
- Snapshot copy: 1 MiB (`MAX_SNAPSHOT_BYTES`); exactly 1 MiB is copied, 1 MiB + 1 fails
  (test). The snapshot of 2026-11-25 is under 3 KB.
- The page renders at most what the snapshot holds (twelve weeks, at most 1000 issues); a
  snapshot over 1 MiB never reaches it.
- Arithmetic: counts use `saturating_add`; the forecast is ⌈open × 28 / closed_28d⌉ with
  `checked_mul` (open ≤ 1000 by the list limit, so the product fits), and a forecast past
  9999-12-31 is `null`. Week starts are day numbers in `i64` from validated dates.

## Security
- **GitHub text is data** (CONSTITUTION 1): issue titles go into the snapshot escaped by
  `json_str`, so a title can't end its string early or add a field. The page treats the whole
  snapshot as untrusted input: it validates it and renders titles as React text (no
  `dangerouslySetInnerHTML`, which Biome forbids).
  Review headers are counted as metrics counts them; a forged tagged comment can only change a
  number on the page (accepted, as in metrics).
- **CSP**: the page is built by Vite with the web app's meta CSP (`script-src 'self'`,
  `connect-src 'self'`, no inline script, Trusted Types `'none'`). Its only requests are
  `./data.json` and `../judge/report/report.json`, both same origin.
- **Judge text is data**: the judge report's journey IDs and milestone titles are rendered as
  React text only; journey IDs are not shown, only counted.
- **Sealed corpus** (CONSTITUTION 13): the snapshot reads only `gates.json`, whose `metrics`
  are aggregates, and keeps only six `smoke_*` values; the page reads the smoke report, which
  the judge already publishes on the site. Neither reads exam results per file.
- **Downloads**: `gh run download` writes only under `<repo>/target/dashboard-gates/<id>`, a
  directory removed first, so a stale or planted file there can't be read as a run's report;
  the file read is `gates.json` in it, checked to be a regular file (not a symlink) and size-
  capped before reading.
- **Commands**: fixed argument lists, no shell; the only interpolated values are validated
  dates.
- Self-check:
  - Arithmetic: above (Limits).
  - What broken implementation still passes? A snapshot that counts every PR in every week:
    the tests put PRs in different weeks and one after `--until`. A forecast that uses all
    closures instead of 28 days: a closure on day −28 (outside) and one on day −27 (inside).
    A snapshot that writes 0 for a missing value: the tests assert `null` medians for weeks
    without a reviewed PR and `null` sessions without an index. A page that shows `null` as 0:
    the Playwright test asserts `missing` for a `null` week and no session chart without an
    index. A page that renders any JSON: a `version` 2 snapshot, a negative count and a
    malformed date each give the invalid state.
  - Filters, each with a reject and a near-miss accept: the week bounds (the Monday and the Sunday
    in; the next Monday out), `--until` (that day in, the next out), the 28-day window (day
    −27 in, day −28 out), the `blocked` label (open in, closed out, `blocked-x` out), journal
    paths (`ops/journal/2026-11-24/x.md` in, `ops/journal/2026-11-24.md` out), the `(#N)`
    subject (at the end in, `(#N) more` out).
  - A run where nothing was checked: an empty `issues.json` and no PRs gives zero counts and
    `null` medians, which is the truth; a missing required input fails the run (test).
  - Judge history, what broken implementation still passes? Taking the oldest run of a week:
    a week with two runs whose newer one differs. Not trying the next run after a failed
    download: a week whose newest run has no file. Reading 0 for a missing metric: a
    `gates.json` without `smoke_journeys_passing` must give `null`. Using `exam_*` keys: the
    fixtures give `exam_*` and `smoke_*` different values. Accepting a rate of 1.5: a test. A
    page that counts superseded journeys: a fixture with a superseded passing journey. A page
    that shows a failed report fetch as zeros: the 404 and invalid tests assert the messages
    and no table.
  - Parser of another tool's output: `journal-log.txt` lines are the shapes git prints with
    that format (header, blank, path); a test pins the arguments of the command run.
- Not visible to these tests: live `gh` output (the tests use `--from` and in-memory inputs;
  the command arguments are pinned by a test), and the real Pages deploy (the CI assembly step
  copies `e2e-linux/site/` as a whole, so `site/dashboard/` ships when `ci linux` wrote it).

## Accessibility
Every chart has an `aria-label` and a table with the same values; text and bars meet 4.5:1
(text) and 3:1 (bars) against the background in light and dark mode (`prefers-color-scheme`),
checked by the Playwright test from computed colors. Headings are in order (h1, then h2).

## Platforms
The snapshot: any machine with `gh` and `git` (a session's). The page: web (GitHub Pages).

## Performance
Thirteen `gh` calls and one `git log`, about 20 s live (2026-11-25). The page: not measured
(twelve bars per chart, at most 1000 table rows).

## Tests
`xtask/src/dashboard.rs` unit tests: week bounds, `--until`, medians and `null`, sessions with
and without an index, journal entry counting and its histogram, blocked ages, milestone order
and forecast (window ends, `open` 0, `closed_28d` 0, past 9999-12-31), truncation at 999/1000,
the pinned command arguments, bad inputs, flags, and a `--from` run end to end (missing inputs
fail, a missing index doesn't), and the snapshot copy (no page, exactly 1 MiB, 1 MiB + 1, not
a regular file, no snapshot).
- `xtask/src/dashboard.rs` judge history tests: run choice per week (newest first, ties by
  id, the 7-run cap, runs outside every week and after `--until` ignored), a failed download
  falling back to the next run, missing keys as `null`, `smoke_*` not `exam_*`, bad
  `gates.json` shapes (no `metrics`, a rate 1.5, NaN-like text, a negative count), the 1 MiB
  read cap, `nightly-runs` truncation at 199/200, a `--from` run without `nightly-runs.json`
  (`source` `missing`) and with a partial history, and the pinned `gh run list` and
  `gh run download` arguments.
- `apps/web/scripts/licenses.test.mjs`: the license check follows the dashboard page (the one
  allowed Vite `input`, specs/tooling/licenses.md item 5) and still refuses any other `input`.
- `apps/web/e2e/dashboard.spec.ts` (Playwright, no journey ID: the run as a whole still fails
  `ci linux` when it fails): a full snapshot (every section, values, the partial week label,
  the truncation warning), `null` values as `missing`, a snapshot without an index, an invalid
  snapshot (version, negative or fractional count, bad dates, an unknown sessions source, a
  non-text title, an array; in `weeks[]` a negative, a `null` count and a bad date, eleven and
  thirteen weeks, a repeated start; a histogram with three objects, a repeated `entries`, an
  `entries` past 4 and a fractional count; a `null` blocked age; a milestone forecast that isn't
  a date and a fractional count; in `judge` an unknown source, eleven weeks, a `start` that
  differs from `weeks`, a rate 1.5 and a fractional `run`) and a 404, the judge history (full,
  partial with `missing` bars, absent, `source` `missing`), the latest judge report (metrics,
  journeys per milestone with a superseded journey left out, a `null` rate as `missing`, a 404,
  an invalid report: wrong tier, a rate 1.5, a journey without `pass`, a body over 4 MiB), the committed `ops/metrics/dashboard.json`
  rendering without the invalid state (so the writer and the validator agree), contrast in
  light and dark mode, and the shared fixture checks (no console errors, CSP violations or
  foreign requests).
