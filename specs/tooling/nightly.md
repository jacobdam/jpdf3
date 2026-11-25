# Nightly run (`cargo xtask nightly`: the regress tier, the bench, extra fuzz time)

Area: tooling
ISO 32000-2 clauses: none (development tool; it never parses PDF bytes, `jpdf-cli` does)
Journeys: none (development tool; ops/judge/CONTRACT.md §2, PLAN §6.1 "Corpora", issues #19, #300, #301)
Status: implemented (parts 1 and 2 of 3: the regress tier and the diff (#300), the bench and
extra fuzz time (#301); filing issues is #302)

## Behavior
1. **Usage.**
   ```
   cargo xtask nightly [--out DIR] [--cache DIR] [--limit N] [--previous FILE] [--max-minutes M]
                       [--bench-minutes B] [--fuzz-seconds S]
   ```
   - `--out DIR`: where the results go (default `target/xtask-out`, like the other tasks). The
     judge's CI runs `cargo xtask nightly --out ../nightly` (CONTRACT §2).
   - `--cache DIR`: the corpus cache, resolved exactly as `jpdf-corpus` resolves it
     (specs/tooling/corpus.md item 7: `--cache`, `$JPDF_CORPUS_CACHE`, `$HOME/.cache/jpdf-corpus`;
     never inside a git working tree).
   - `--limit N` (1 ≤ N ≤ 200 000): only the first N manifest rows in blob-id order
     (`jpdf_corpus::fetch::select`, the same rows `jpdf-corpus fetch regress --limit N` takes).
   - `--previous FILE`: the `regress.json` of an earlier night, for the diff (item 7).
   - `--max-minutes M` (1 ≤ M ≤ 1440, default 120): the time budget for the runs (item 7).
   - `--bench-minutes B` (0 ≤ B ≤ 120, default 20): the bench's time limit (item 11); 0 skips
     the bench.
   - `--fuzz-seconds S` (0 ≤ S ≤ 3600, the cap of `cargo xtask fuzz --seconds`; default 1200):
     the fuzz time per target (item 12); 0 skips the fuzz pass.
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
6. **Memory limit.** Before the runs, `/bin/sh -c 'ulimit -v 2097152 && ulimit -v'` must print
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
10. **Exit codes.** The nightly exits non-zero only for a tool failure, never for what
    `jpdf-cli` did (part 3, #302, files issues for crashes):
    - 2: usage (item 1);
    - 1: a tool failure: a bench or fuzz pass with status `failed` (items 11 and 12), a
      `nightly.json` that can't be removed or written (item 13), an unreadable or invalid previous file, the build (or no `jpdf-cli`
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
    - 0 otherwise, whatever the runs' statuses and `complete`.
    Once the runs started, `regress.json` and the diff are written (each to a `.tmp` name,
    then renamed over an earlier night's file) before the nightly exits 1 or 75, so nothing
    measured is lost, and an earlier night's files stay on disk until this night's replace
    them (a night that dies mid-run leaves the previous pair intact). So the files in `DIR` are
    this night's only when the nightly exited 0 or 75 after the runs; neither file names its
    night, so a reader (part 3, #302) must check the exit code before it reads `DIR`. If the
    diff can't be written, an earlier `regress-diff.json` is removed (best effort: a failed
    removal is only logged), and a kill between the two renames can leave this night's
    `regress.json` next to an earlier diff; the exit code tells these cases apart too. A
    failed rename can leave `regress.json.tmp` or `regress-diff.json.tmp` in `DIR`. A failure
    before the runs writes nothing. Through the memory wrapper,
    `/bin/sh` always starts; a `jpdf-cli` removed or made non-executable during the runs then
    shows as `crash` with exit 126 or 127 (the wrapper's own failure is 125), like any other
    such exit of `jpdf-cli`.

11. **Bench** (after `regress.json` and the diff are written, whatever their outcome; skipped
    when `--bench-minutes` is 0). The sample is the first 20 files with a verified copy
    (item 3), in blob-id order: they are copied, as `<blob>.pdf`, into `DIR/bench-corpus/`,
    which is emptied first, so the bench never sees a file that failed the blob check or one
    left from an earlier night. An earlier `DIR/bench.json` is removed before the bench runs.
    Then `$CARGO build --release --locked -p jpdf-cli -p jpdf-bench` (`cargo xtask bench`'s
    build, specs/tooling/bench.md item 11) and
    `jpdf-bench --cli <release>/jpdf-cli --out DIR/bench.json --corpus DIR/bench-corpus --sample <n>`
    (absolute paths; the engines' defaults: mutool and pdfium_test are used when found on
    `PATH`), run like a regress run (item 4: its own process group, the watchdog, stdout on
    the null device, the first 4 KiB of stderr kept and printed when it didn't end `ok`)
    and through the memory wrapper when it applies (item 6), so jpdf-bench and every program
    it starts run under the 2 GiB limit. Its time limit is `--bench-minutes`; at the limit its
    group is killed (jpdf-bench's own watchdog then kills the groups of its children,
    specs/tooling/bench.md item 7). Status: exit 0 `ok`; the kill at the limit `timeout`;
    anything else (exit 1 or 2, a signal, no start, a build or copy failure) `failed`. Only an
    `ok` bench that left `DIR/bench.json` has a report. `timeout` is recorded, not a tool
    failure: it is usually what `jpdf-cli` (or a reference engine) did, but a jpdf-bench that
    hangs on its own bug ends the same way, so a `timeout` every night may also be a broken
    bench. `failed` is a tool failure (item 10).
12. **Extra fuzz time** (after the bench; skipped when `--fuzz-seconds` is 0). This is fuzz
    time on top of the judge's own fuzz step (CONTRACT §3, 300 s per target). `DIR/fuzz/` is
    emptied first, then `fuzz/Cargo.toml` must exist (checked before anything is installed),
    then with `CI` set the missing tools are installed exactly as `cargo xtask fuzz` on CI
    does (the pinned nightly and cargo-fuzz), then `cargo xtask fuzz --seconds S`'s pass runs
    every target from `cargo fuzz list` (specs/tooling/fuzz.md: the judge's 2048 MB RSS limit,
    10 s per input, one target per core at a time). The file names in each
    `fuzz/artifacts/<target>/` are noted before the pass; after it, the regular files (no
    symlinks, no directories) that are new there are copied to `DIR/fuzz/<target>/`, at most
    100 per target by name order (more are counted on stderr and left in `fuzz/artifacts/`),
    for each target whose run started (the fuzz pass can stop before later targets start: a
    target that can't start ends it, after the started ones end; those not started are not
    listed). A copy that fails stops that target's copies; the ones made before it stay
    listed.
    Status: a failure before any target ran (no project, a failed install, list or build, a
    seed that can't be written), a target that couldn't start, a log that couldn't be
    created, or an artifact that couldn't be copied is `failed`, a tool failure; else any
    target that crashed, hung or ran out of memory makes it `crash` (what the engine did:
    recorded, not a tool failure); else `ok`.
13. **Summary** `DIR/nightly.json`. An earlier one is removed before the bench (a failed
    removal is a tool failure), and this night's is written after the fuzz pass (tmp and
    rename, as in item 8), keys in this order:
    ```json
    {"schema":1,"regress":"regress.json",
     "bench":{"status":"ok","minutes":20,"files":20,"exit":0,"signal":null,"ms":174,
              "report":"bench.json","error":null},
     "fuzz":{"status":"crash","seconds_per_target":1200,
             "targets":[{"name":"engine_open","ok":false,"artifacts":["fuzz/engine_open/crash-…"]}],
             "error":null}}
    ```
    `bench.status` is `ok`, `timeout`, `failed` or `skipped`; `files` the sample size; `exit`,
    `signal` and `ms` as in item 5 (`null` when it didn't start or was skipped); `report` is
    `"bench.json"` or `null`; `error` says why it `failed`. `fuzz.status` is `ok`, `crash`,
    `failed` or `skipped`; `targets` lists the targets whose run started, in `cargo fuzz list` order, each
    with `ok` and the copied artifacts (paths relative to `DIR`); `error` the tool failure.
    Strings are JSON-escaped (artifact names are read lossily as UTF-8).
14. **Order.** Regress runs, `regress.json`, the diff, then the bench, the fuzz pass and
    `nightly.json`. A nightly that stops before the regress runs (item 10) runs neither the
    bench nor the fuzz pass and writes no `nightly.json`. The exit code is item 10's: a bench
    or fuzz tool failure makes it 1, which beats 75.

## Persistence
No document persistence. `regress.json`, `regress-diff.json`, `nightly.json`, `bench.json`
(and jpdf-bench's `bench.json.work/`), `bench-corpus/`, `fuzz/` and `work/` under `DIR` (and the
`.tmp` names after a failed rename, item 10). In the repository's ignored paths: the fuzz pass's
seeds and corpus under `fuzz/corpus/`, its artifacts under `fuzz/artifacts/`, its logs under
`target/fuzz-logs/` (specs/tooling/fuzz.md). The previous night's file is passed in by the caller: the CI step that downloads
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
can't be removed). The bench and the fuzz pass each print one line,
`xtask: nightly: bench: <status> (<n> files)[: <error>]` and
`xtask: nightly: fuzz: <status> (<t> targets, <S> s each, <a> artifacts)[: <error>]`; the
error is the same text as in `nightly.json` (`release build failed (<status>)`,
`cannot copy <path>: <io error>`, `cannot start <path>`, `jpdf-bench ended with "<status>"`,
`fuzz/Cargo.toml not found …`, the fuzz tool's messages). A usage error prints the problem and the usage line.

## Limits
- Per run: CONTRACT §1 time limits (item 4) and the 2 GiB address-space limit where the OS
  enforces it (item 6); stderr kept ≤ 4 KiB per run, stdout never read.
- Whole run: `--max-minutes` (≤ 1440) plus at most one run's limit; `--limit` ≤ 200 000 rows
  (the corpus manifest's own row cap).
- The previous file: at most 64 MiB, read bounded at 64 MiB + 1 byte (more is an error), at
  most 200 000 files; JSON nesting is capped by `jpdf_testgen::json` (128); each file has at most
  five runs (one per op). Only a regular file is read, opened non-blocking where
  `jpdf_fsio::OPEN_FLAGS` is set (a FIFO is refused, not waited on).
- Memory of the nightly itself: the results of ≤ 200 000 files × 5 runs (each run ≤ 4 KiB of
  stderr, only for crashes and timeouts) and the previous file (≤ 64 MiB).
- Bench: `--bench-minutes` ≤ 120 of wall time, killed at the limit; a sample of ≤ 20 files.
- Fuzz: `--fuzz-seconds` ≤ 3600 per target (libFuzzer's `-max_total_time`), so the pass takes
  ⌈targets / cores⌉ × S plus the build; ≤ 100 artifacts copied per target.
- Exact-boundary tests: `--limit` 200 000 passes and 200 001 fails, 0 fails; `--max-minutes`
  1440 passes, 1441 and 0 fail; `--bench-minutes` 0 and 120 pass, 121 fails, and 1 gives a
  60 s limit; `--fuzz-seconds` 0 and 3600 pass, 3601 fails; a bench sample of exactly 20
  verified files, and 21 give 20; exactly 100 new artifacts are copied and a 101st is not; a run at elapsed = budget − 1 ns runs, at elapsed = budget
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
- **Bench and fuzz:** the bench reads only files that passed the blob check (copies, item 11)
  and runs under the memory wrapper and a time limit; the fuzz pass is `cargo xtask fuzz`'s,
  with its limits. Artifacts are copied, never opened or parsed; their names are escaped.
  Only regular files are copied (a symlink planted in `fuzz/artifacts/` is not followed).
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
  - A bench that reports an earlier night's numbers: the test writes a stale `bench.json` and
    checks it is gone when the fake bench starts and absent after a `timeout` or `failed`
    end; a stale `nightly.json` is replaced.
  - A bench that sees unverified or leftover files: the test stages a sample with a missing
    row next to a stale file and checks the staged names and bytes.
  - A bench or fuzz status that never fails the nightly, or always does: tests check each
    status's `tool_failure` and `run`'s result for `ok`, `timeout`, `failed`, `skipped`, and a
    fuzz pass with no `fuzz/` (`failed`).
  - An artifact copy that copies old, linked or unbounded files: tests with an old file, a
    symlink, a directory and the 100/101 cap.
- Not covered by tests: the real build, the real fetch (network), the CI wiring, and the order
  of steps inside `run` (the previous file loaded before `regress.json` is written: the test
  checks that the loaded copy is held in memory, not where `run` calls it; the bench and fuzz
  pass called after the diff); a real fuzz pass (the nightly toolchain and minutes of fuzzing:
  `run_fuzz` with real targets runs only in the local run and the nightly job, its parts are
  unit-tested in `fuzz.rs` and here); the real jpdf-bench under the memory wrapper (a local
  run). Local runs on the cached tier, including `--previous` naming the output's own
  `regress.json` and the bench and fuzz pass, are recorded in the PRs (#300's, #301's).

## Accessibility
Not applicable (command-line tool).

## Platforms
Linux (the judge's CI) and macOS. On macOS the address-space limit is not applied (item 6),
like the judge's own helper.

## Performance
Today `jpdf-cli` answers `info` and exits 3 for the rest, so the 5250-file tier takes a few
minutes (2 min 39 s on a local run). The budget (item 7) bounds the run once commands are
implemented. The CI job has 240 min (`timeout-minutes`). With the defaults the worst case is
120 + 1 (one run past the budget) + 20 (bench) + ⌈3 targets × 1200 s / 60⌉ = 60 (fuzz on one
core; 20 on the 4-core runner) = 201 min. A test checks that this sum, for the number of fuzz
targets in `fuzz/fuzz_targets/`, leaves at least 30 min of the 240 for the rest: the fetch, the
release builds, the cargo-fuzz install (built from source on CI: this job has no cache for it)
and the fuzz targets' build. That 30 min is an estimate; none of those steps has been timed in
the CI job yet (the first nightly run will show it). A local run
with `--limit 30 --bench-minutes 5 --fuzz-seconds 10` took 58 s.

## Tests
`xtask/src/nightly/extra/tests.rs` (the budget sum, the bench sample and its staging, the bench
statuses with a fake exec and build, the stale `bench.json` and `nightly.json` removal, the
artifact copy and its cap (and a copy that fails midway), the fuzz pass with a fake
`fuzz::run_report` (the snapshot before the pass, only started targets, each target's `ok`),
a bench sample that can't be copied, a stale `nightly.json` that can't be removed, the fuzz
statuses, a tree without `fuzz/`, `nightly.json`'s shape,
the exit decision), `xtask/src/nightly.rs` unit tests (argument parsing and boundaries, the deadline, the classifier
and memory wrapper with fake `jpdf-cli` scripts, missing files, the results JSON, the previous-file
validator, the diff), `tools/bench/src/engine.rs`
`run_child_reports_the_exit_code_or_the_signal`, `tools/corpus/tests/corpus.rs`
`is_verified_accepts_only_the_exact_file`.
