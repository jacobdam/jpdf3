# Nightly regress run (`cargo xtask nightly`, part 1: the regress tier)

Area: tooling
ISO 32000-2 clauses: none (development tool; it never parses PDF bytes, `jpdf-cli` does)
Journeys: none (development tool; ops/judge/CONTRACT.md §2, PLAN §6.1 "Corpora", issues #19, #300)
Status: implemented (part 1 of 3: the regress tier and the diff; bench and fuzz time are #301,
filing issues is #302)

## Behavior
1. **Usage.**
   ```
   cargo xtask nightly [--out DIR] [--cache DIR] [--limit N] [--previous FILE] [--max-minutes M]
   ```
   - `--out DIR`: where the results go (default `target/xtask-out`, like the other tasks). The
     judge's CI runs `cargo xtask nightly --out ../nightly` (CONTRACT §2).
   - `--cache DIR`: the corpus cache, resolved exactly as `jpdf-corpus` resolves it
     (specs/tooling/corpus.md item 7: `--cache`, `$JPDF_CORPUS_CACHE`, `$HOME/.cache/jpdf-corpus`;
     never inside a git working tree).
   - `--limit N` (1 ≤ N ≤ 200 000): only the first N manifest rows in blob-id order
     (`jpdf_corpus::fetch::select`, the same rows `jpdf-corpus fetch regress --limit N` takes).
   - `--previous FILE`: the `regress.json` of an earlier night, for the diff (item 7).
   - `--max-minutes M` (1 ≤ M ≤ 1440, default 120): the time budget for the runs (item 6).
   - Each flag at most once; an unknown flag, a missing value, a value out of range or not a
     plain decimal number is a usage error (exit 2) before anything runs.
2. **Build.** `$CARGO build --release --locked -p jpdf-cli` (the judge's build, CONTRACT §1), then
   `<target>/release/jpdf-cli` (`<target>` = `$CARGO_TARGET_DIR` or `target`, as in
   specs/tooling/bench.md item 11; `CARGO_BUILD_TARGET_DIR`, a config `build.target-dir` or
   `build.target` are not supported: with one of them set, an older binary at that path would
   be run). A failed build, or no executable file at that path, is a tool failure.
3. **Files.** The tier is always `regress` (`tools/corpus/manifests/regress.tsv`, through
   `jpdf_corpus::Tier::Regress`); there is no flag that names a manifest or another tier, so the
   sealed corpus is never read (CONSTITUTION 13). The manifest is loaded with the corpus tool's
   parser and limits, the rows are selected (item 1), and `jpdf_corpus::fetch::fetch` downloads
   what the cache lacks (8 jobs, its usual checks and retries). Then every row's
   `<cache>/regress/<blob>.pdf` is checked again with `jpdf_corpus::fetch::is_verified` (a
   regular file with the manifest's size and git blob id; a FIFO doesn't block it). A row that
   fails the check is **missing**: it is listed in the results and never run, so a stale or
   planted file found under that name at check time is never given to `jpdf-cli`. The check
   runs once, before all runs; a process that changes the cache during the runs (a concurrent
   `jpdf-corpus fetch` or cleanup of the same cache) is out of scope: the CI job has a fresh
   cache and nothing else writing to it.
4. **Runs.** For each verified file, in blob-id order, one at a time, these five runs (the
   items the judge grades: `info` and pages 1 and 2 at 72 dpi, ops/judge/lib/common.py
   `PAGES_PER_FILE = 2`), each with the CONTRACT §1 time limit:

   | op | arguments | limit |
   |---|---|---|
   | `info` | `info F` | 20 s |
   | `render1` | `render F --page 1 --dpi 72 --out W` | 30 s |
   | `render2` | `render F --page 2 --dpi 72 --out W` | 30 s |
   | `text1` | `text F --page 1` | 60 s |
   | `text2` | `text F --page 2` | 60 s |

   `F` is the cached file's path (absolute, under the canonical cache directory, so it never
   starts with `-`); `W` is `DIR/work/render.png`, removed before each render. Page 2 runs on
   one-page files too: the judge asks for it only when the file has two pages, but an
   out-of-range page must also exit 2, never crash. Each run goes through
   `jpdf_bench::engine::run_child`: stdin and stdout on the null device, its own process
   group killed at exit and at the limit, the bench watchdog (started first; if it can't start
   the nightly is a tool failure) killing it if xtask dies (specs/tooling/bench.md item 7), and
   at most 4 KiB of stderr kept.
5. **Classification** (CONTRACT §1): exit 0 `ok`, 2 `error`, 3 `not_implemented`, a kill at
   the time limit `timeout`, any other code, a signal or a spawn failure `crash`. Each run
   records `exit` (the code, or `null`), `signal` (the signal number on unix when the child
   was ended by one that was not our time-limit kill, else `null`) and `ms` (wall time from
   spawn to exit or kill, rounded down; `null` for a run that could not start). Only `crash` and `timeout` runs keep `stderr` (lossy
   UTF-8, the **first** 4 KiB; the judge keeps the last 8 KiB, so a long log can push the panic
   message out: #322 keeps the tail instead).
6. **Memory limit.** Before the runs (and since #25 before the fetch: the limits cases of
   item 11 use it too), `/bin/sh -c 'ulimit -v 2097152 && ulimit -v'` must print
   exactly `2097152` (2 GiB in KiB, the judge's `RLIMIT_AS`). If it does, every run is
   `/bin/sh -c 'ulimit -v 2097152 || exit 125; exec "$0" "$@"' <jpdf-cli> <arguments>`: the
   shell sets the limit and replaces itself with `jpdf-cli` (same process, same group), so the
   program runs under the judge's address-space limit. If `ulimit` fails anyway, the run
   exits 125, a `crash` (loud, never a silent run without the limit). Running out of memory
   is a crash like in the judge (an allocation failure aborts: SIGABRT). If the probe fails:
   on Linux (where the judge enforces the limit) the nightly is a tool failure before any
   run; elsewhere (macOS refuses `RLIMIT_AS`, as the judge's helper notes) the runs go
   without it, the results say `"memory_limit_kib": null`, and stderr says so.
7. **Time budget.** Before each run, if the time since the first run started is at least
   `--max-minutes`, that run and every later one is `not_run` (no `exit`, `signal` or `ms`),
   and `complete` is `false`. A run that started keeps its own limit, so the overshoot is at
   most one run's limit (60 s) plus the kill.
8. **Results** `DIR/regress.json` (written to `regress.json.tmp`, then renamed; keys in this
   order):
   ```json
   {"schema":1,"tier":"regress","rows":5250,"memory_limit_kib":2097152,"max_minutes":120,
    "complete":true,
    "counts":{"ok":3815,"error":238,"not_implemented":22197,"timeout":0,"crash":0,"not_run":0,"missing":0},
    "files":[{"blob":"<40 hex>","url":"https://…","missing":false,
              "runs":[{"op":"info","status":"ok","exit":0,"signal":null,"ms":12}, …]}, …]}
   ```
   `rows` is the number of selected rows; `counts` counts runs by status, and `missing`
   counts files. A missing file has `"missing":true` and `"runs":[]`; any other file has all
   five runs in the item 4 order.
9. **Diff** `DIR/regress-diff.json` (same tmp-and-rename write):
   - The previous file is read and checked **before the build and the runs**, and held in
     memory, so a `--previous` that names this night's own `DIR/regress.json` still diffs
     against the earlier night, and an invalid file fails before hours of runs.
   - No `--previous`, or no file at that path: `{"schema":1,"previous":null,"note":"…"}`,
     and stderr says there is no diff.
   - Otherwise the previous file is read (item "Limits") and checked: an object with
     `"schema":1` and a `files` array; each file an object with a `blob` of 40 lowercase hex
     digits (each blob once), a `url` string, a boolean `missing`, and either `"missing":true` with no runs or a `runs` array whose
     items have a known `op` (each op at most once per file) and a known status (item 5 plus
     `not_run`). Anything else is a tool failure before any run (item 10). Other members are
     ignored.
   - Each (blob, op) pair present on either side gets a status per side: the run's status, or
     `missing` for every op of a missing file, or `absent` when that side doesn't list the
     blob (or the op). Pairs whose two statuses differ are the changes, sorted by blob, then
     op in the item 4 order:
     ```json
     {"schema":1,"previous":"<FILE as given>","changes":[{"blob":"…","url":"…","op":"info","before":"ok","after":"crash"}],
      "new_failures":1,"fixed":0}
     ```
     `url` is this night's (or the previous night's, for a blob only that side lists).
     `new_failures` counts changes to `crash` or `timeout` from any other status; `fixed`
     counts changes from `crash` or `timeout` to `ok`, `error` or `not_implemented`.
     stderr gets `nightly: regress diff: <C> changes, <N> new crash/timeout, <F> fixed`.
10. **Exit codes.** The nightly exits non-zero only for a tool failure or a failed limits case
    (item 11), never for what `jpdf-cli` did with a regress file (part 3, #302, files issues for
    crashes):
    - 2: usage (item 1);
    - 1: a tool failure: an unreadable or invalid previous file, the build (or no `jpdf-cli`
      file where the build should have put it), the manifest, the cache, the watchdog, the
      memory probe on Linux, removing or writing the results, an integrity failure in the
      fetch, no executable `jpdf-cli` after the build (checked before the runs: a regular file
      with an execute bit; any class's bit counts, so a mode like 0o610 passes and then fails
      each run with 126: #323), a run that could not start (without the memory wrapper, a spawn
      failure), or no file to run at all when the fetch was not an
      infrastructure failure (every selected row missing: a run that checked nothing must not
      pass);
    - 75 with `JUDGE-INFRA-FAILURE` on stderr: more than `max(1, ⌊N/100⌋)` of the N rows could
      not be downloaded (the corpus tool's and the judge's rule) and no other tool failure.
      This includes the case where every row is missing because of it (no network): that
      stops before the runs with 75, not 1, since the cause is infrastructure;
    - 4: a limits case did not pass (item 11) and the regress part would have exited 0: a
      hostile file that crashes, hangs or gets the wrong answer is a regression, unlike a regress
      file's status. When the regress part exits 1 or 75, that code wins (a 75 night is rerun,
      which runs the cases again); the failed case is still in `limits.json` and on stderr;
    - 0 otherwise, whatever the runs' statuses and `complete`.
    A tool failure in the limits part is a 1 too, but the regress part still runs.
    Once the runs started, `regress.json` and the diff are written (each to a `.tmp` name,
    then renamed over an earlier night's file) before the nightly exits 1 or 75, so nothing
    measured is lost, and an earlier night's files stay on disk until this night's replace
    them (a night that dies mid-run leaves the previous pair intact). So the files in `DIR` are
    this night's only when the nightly exited 0, 4 or 75 after the runs; neither file names its
    night, so a reader (part 3, #302) must check the exit code before it reads `DIR`. If the
    diff can't be written, an earlier `regress-diff.json` is removed (best effort: a failed
    removal is only logged), and a kill between the two renames can leave this night's
    `regress.json` next to an earlier diff; the exit code tells these cases apart too. A
    failed rename can leave `regress.json.tmp` or `regress-diff.json.tmp` in `DIR`. A failure
    before the regress runs writes neither file (`limits.json`, item 11, is written once the
    limits part ran). Through the memory wrapper,
    `/bin/sh` always starts; a `jpdf-cli` removed or made non-executable during the runs then
    shows as `crash` with exit 126 or 127 (the wrapper's own failure is 125), like any other
    such exit of `jpdf-cli`.

11. **Limits cases** (#25). After the build, the memory probe and the watchdog, and before the
    fetch (so a night without network still runs them), the nightly runs the seven cases of
    specs/tooling/limits.md at its default size (500 MiB) with the same `jpdf-cli` and memory
    wrapper, and writes `DIR/limits.json`. A failed case gives exit 4 (item 10). A tool failure
    there is logged as `xtask: nightly: limits: <problem>` and gives exit 1; the regress part
    runs either way.

## Persistence
No document persistence. `regress.json`, `regress-diff.json`, `limits.json` (item 11) and `work/` under `DIR` (and their
`.tmp` names after a failed rename, item 10); nothing in the repository. The previous night's file is passed in by the caller: the CI step that downloads
it is part 3 (#302) and needs the owner (`.github/` is protected).

## Undo
Not applicable (development tool).

## Errors
Each tool failure prints `xtask: nightly: <problem>` on stderr: `release build failed (<status>)`,
the corpus tool's manifest or cache error, `cannot start the engine watchdog (/bin/sh)`,
`ulimit -v 2097152 is not enforced here (the judge's Linux limit)`, `cannot write <path>: <io error>`,
`fetch: <url>: <problem>` lines then `fetch: <summary>`, `previous results <path>: <problem>`,
`no regress file to run (<N> rows missing)`, `no executable jpdf-cli at <path> after the build`,
`<N> runs could not start <path>` (and `cannot remove <path>: <io error>` when a stale diff
can't be removed). A usage error prints the problem and the usage line.

## Limits
- Per run: CONTRACT §1 time limits (item 4) and the 2 GiB address-space limit where the OS
  enforces it (item 6); stderr kept ≤ 4 KiB per run, stdout never read.
- Whole run: the limits part (item 11: seven generated files of ≤ 512 MiB, each run ≤ 20 s, so
  at most 140 s of runs plus the writes; about 15 s locally), then `--max-minutes` (≤ 1440) plus
  at most one run's limit for the regress part, whose clock starts after the limits part; `--limit` ≤ 200 000 rows
  (the corpus manifest's own row cap).
- The previous file: at most 64 MiB, read bounded at 64 MiB + 1 byte (more is an error), at
  most 200 000 files; JSON nesting is capped by `jpdf_testgen::json` (128); each file has at most
  five runs (one per op). Only a regular file is read, opened non-blocking where
  `jpdf_fsio::OPEN_FLAGS` is set (a FIFO is refused, not waited on).
- Memory of the nightly itself: the results of ≤ 200 000 files × 5 runs (each run ≤ 4 KiB of
  stderr, only for crashes and timeouts) and the previous file (≤ 64 MiB).
- Exact-boundary tests: `--limit` 200 000 passes and 200 001 fails, 0 fails; `--max-minutes`
  1440 passes, 1441 and 0 fail; a run at elapsed = budget − 1 ns runs, at elapsed = budget
  it is `not_run`; a previous file of exactly the cap is read and one byte more fails, and
  a file list of exactly the cap passes and one more fails (both caps passed in small by the
  tests, which also pin the real values); stderr is cut at exactly 4096 bytes.
- No panics: no `unwrap`, `expect` or indexing on input-derived data (workspace lints);
  counts use saturating arithmetic; `ms` is `u128` milliseconds written as a JSON number.

## Security
- **Sealed corpus (CONSTITUTION 13):** the tier is fixed to `regress` (item 3); no flag takes a
  manifest path or a tier name.
- **Hostile bytes:** the nightly never parses PDF bytes. `jpdf-cli` runs as a separate process
  under the judge's limits, in its own group, with the watchdog. Only files that pass the blob
  check run (item 3). Paths given to it are absolute.
- **Hostile output:** `jpdf-cli`'s stderr is data (CONSTITUTION 1). It is cut at 4 KiB, decoded
  lossily and JSON-escaped (`jpdf_bench::json::string`) before it is written, so it can't
  break the results file. The previous results file is untrusted input: bounded, strictly
  parsed, every value checked (item 9); its blob ids and URLs are only compared and escaped.
- **Shell:** the memory wrapper is a fixed script; the program and its arguments are passed as
  `$0` and `$@`, never interpolated.
- **CONSTITUTION 8–11:** no new crates; xtask gains path dependencies on the workspace's own
  `jpdf-bench` and `jpdf-corpus`.

Self-check:
- Arithmetic: counts and totals use `saturating_add`; the deadline compares `Duration`s; `ms`
  comes from `Duration::as_millis` (`u128`, no cast).
- What broken implementation still passes?
  - A nightly that runs nothing: every selected row missing is exit 1 unless the fetch was an
    infrastructure failure (75) (test); a run where the
    memory wrapper silently didn't apply: on Linux the probe must succeed or the nightly
    fails, and a test runs a fake `jpdf-cli` through the wrapper that prints its `ulimit -v`
    (2097152 expected wherever the probe succeeds).
  - A classifier that maps everything to one status: tests run fake programs that exit 0, 2,
    3, 7 and 101, abort (SIGABRT) and hang past a short limit, and check each status, `exit`,
    `signal` and the stderr rule.
  - A diff that ignores one side, a status or the `absent`/`missing` mapping: tests cover
    added, removed and missing files, each status change direction for `new_failures` and
    `fixed`, an unchanged pair (not listed) and the sort order.
  - A deadline off by one: the boundary test in "Limits".
  - A validator that accepts anything: one test per rejected shape of the previous file
    (schema 2, a string schema, no `files`, a bad blob, a repeated blob, an unknown op, a
    repeated op, an unknown status, a missing file with runs, too many files, too large), each
    with a near-miss that is accepted (extra members, `missing:false` with runs).
- Not covered by tests: the real build, the real fetch (network), the CI wiring, and the order
  of steps inside `run` (the previous file loaded before `regress.json` is written: the test
  checks that the loaded copy is held in memory, not where `run` calls it); a local run on the
  cached tier, including `--previous` naming the output's own `regress.json`, is recorded in
  the PR.

## Accessibility
Not applicable (command-line tool).

## Platforms
Linux (the judge's CI) and macOS. On macOS the address-space limit is not applied (item 6),
like the judge's own helper.

## Performance
Today `jpdf-cli` answers `info` and exits 3 for the rest, so the 5250-file tier takes a few
minutes (2 min 39 s on a local run). The budget (item 7) bounds the run once commands are implemented; part 2 (#301) adds
bench and fuzz time to the same CI job (240 min). The limits part (item 11) adds about 15 s before the regress runs.

## Tests
`xtask/src/nightly.rs` unit tests (argument parsing and boundaries, the deadline, the classifier
and memory wrapper with fake `jpdf-cli` scripts, missing files, the results JSON, the previous-file
validator, the diff), `tools/bench/src/engine.rs`
`run_child_reports_the_exit_code_or_the_signal`, `tools/corpus/tests/corpus.rs`
`is_verified_accepts_only_the_exact_file`. The limits part (item 11): `xtask/src/limits/tests.rs`
(specs/tooling/limits.md "Tests"; `nightly_exit_codes_with_the_limits_cases` for item 10's 4).
