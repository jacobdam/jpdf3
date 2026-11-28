# Nightly run (`cargo xtask nightly`: the regress tier, the bench, extra fuzz time)

Area: tooling
ISO 32000-2 clauses: none (development tool; it never parses PDF bytes, `jpdf-cli` does)
Journeys: none (development tool; ops/judge/CONTRACT.md §2, PLAN §6.1 "Corpora", issues #19, #300, #301, #302)
Status: implemented (the regress tier and the diff (#300), the bench and extra fuzz time
(#301), and filing issues (item 16) in two parts: limits cases and regress files (#302), the
fuzz pass and the bench (#383))

## Behavior
1. **Usage.**
   ```
   cargo xtask nightly [--out DIR] [--cache DIR] [--limit N] [--previous FILE] [--max-minutes M]
                       [--bench-minutes B] [--fuzz-seconds S] [--file-issues]
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
   - `--bench-minutes B` (0 ≤ B ≤ 120, default 20): the bench's time limit (item 12); 0 skips
     the bench.
   - `--fuzz-seconds S` (0 ≤ S ≤ 3600, the cap of `cargo xtask fuzz --seconds`; default 1200):
     the fuzz time per target (item 13); 0 skips the fuzz pass.
   - `--file-issues` (no value): the `issues` step files with `gh` (item 16); without it the
     step only counts the findings, so a local run never writes to GitHub.
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
   the **last** 4 KiB of stderr kept (item 5).
5. **Classification** (CONTRACT §1): exit 0 `ok`, 2 `error`, 3 `not_implemented`, a kill at
   the time limit `timeout`, any other code, a signal or a spawn failure `crash`. Each run
   records `exit` (the code, or `null`), `signal` (the signal number on unix when the child
   was ended by one that was not our time-limit kill, else `null`) and `ms` (wall time from
   spawn to exit or kill, rounded down; `null` for a run that could not start). Only `crash` and `timeout` runs keep `stderr` (lossy
   UTF-8, the **last** 4 KiB, as the judge keeps the last 8 KiB, so a panic message or abort
   reason after a long log is kept, #322). `run_child`'s tail mode (`Keep::Tail`) reads the
   pipe to its end through a deque reserved once for the cap, plus one 8 KiB read chunk (room
   is made before each chunk is added, so the kept bytes never exceed the cap and the deque
   never reallocates; if the reservation fails, nothing is kept and the pipe is still
   read until its end, a read error or the stop below, with the chunk on the reader's stack
   (no other heap allocation), so the run records no stderr but is classified as usual); a flood costs
   CPU until the run's group is killed (at exit or at the time limit). Once the run is
   collected (at most 1 s after exit, as before), the reader stops at its next read, so a
   grandchild that escaped the group and holds the pipe can't keep it draining; such a run
   records no stderr (what the reader drained is dropped). The head mode loses its bytes the
   same way, unless they filled the 4 KiB cap before the 1 s collection timeout. No nightly child is known to do this (jpdf-bench gives its
   children their own pipe or the null device, never its stderr). The bench's
   engine probes keep the head (`Keep::Head`, specs/tooling/bench.md item 2).
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
    - 1: a tool failure: a bench or fuzz pass with status `failed` (items 12 and 13), a
      `nightly.json` that can't be removed or written (item 14), an unreadable or invalid previous file, the build (or no `jpdf-cli`
      file where the build should have put it), the manifest, the cache, the watchdog, the
      memory probe on Linux, removing or writing the results, an integrity failure in the
      fetch, no executable `jpdf-cli` after the build (checked before the runs: a regular file
      that the runner may execute, by the shell's `test -x`, which checks the bit of the
      runner's own class (for root, any execute bit), so a runner-owned mode like 0o610 or 0o601 is refused here instead
      of failing each run with 126: #323), a run that could not start (without the memory wrapper, a spawn
      failure), or no file to run at all when the fetch was not an
      infrastructure failure (every selected row missing: a run that checked nothing must not
      pass);
    - 75 with `JUDGE-INFRA-FAILURE` on stderr: more than `max(1, ⌊N/100⌋)` of the N rows could
      not be downloaded (the corpus tool's and the judge's rule) and no other tool failure.
      This includes the case where every row is missing because of it (no network): that
      stops before the runs with 75, not 1, since the cause is infrastructure. The line
      `JUDGE-INFRA-FAILURE: <n> regress files could not be fetched` is printed once, at the
      end, exactly when the exit code is 75 (ADR 0013 task 1 changed this: before it,
      the line followed the regress part's own code, so any night whose regress part was 75
      (no network, or too many rows not downloaded while some files ran) and whose limits part
      had a tool failure printed the line and exited 1; now such a night exits 1 without it);
    - 4: a limits case did not pass (item 11) and the regress part would have exited 0: a
      hostile file that crashes, hangs or gets the wrong answer is a regression, unlike a regress
      file's status. When the regress part exits 1 or 75, that code wins (a 75 night is rerun,
      which runs the cases again); the failed case is still in `limits.json` and on stderr;
    - 0 otherwise, whatever the runs' statuses and `complete`.
    A tool failure in the limits part is a 1 too, but the regress part still runs.
    The exit code is `steps::exit_code` of the step log (item 15): `Failed` 1, else `Infra`
    75, else `Finding` 4, else 0 (ADR 0013).
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

12. **Bench** (after `regress.json` is written and the diff step ends, whatever the diff's
    outcome; a failed `regress.json` write stops the night before the diff, item 15; skipped
    when `--bench-minutes` is 0). The sample is the first 20 files with a verified copy
    (item 3), in blob-id order: they are copied, as `<blob>.pdf`, into `DIR/bench-corpus/`,
    which is emptied first, so the bench never sees a file that failed the blob check or one
    left from an earlier night. An earlier `DIR/bench.json` is removed before the bench runs.
    Then `$CARGO build --release --locked -p jpdf-cli -p jpdf-bench` (`cargo xtask bench`'s
    build, specs/tooling/bench.md item 11) and
    `jpdf-bench --cli <release>/jpdf-cli --out DIR/bench.json --corpus DIR/bench-corpus --sample <n>`
    (absolute paths; the engines' defaults: mutool and pdfium_test are used when found on
    `PATH`), run like a regress run (item 4: its own process group, the watchdog, stdout on
    the null device, the last 4 KiB of stderr kept and printed when it didn't end `ok`)
    and through the memory wrapper when it applies (item 6), so jpdf-bench and every program
    it starts run under the 2 GiB limit. Its time limit is `--bench-minutes`; at the limit its
    group is killed (jpdf-bench's own watchdog then kills the groups of its children,
    specs/tooling/bench.md item 7). Status: exit 0 `ok`; the kill at the limit `timeout`;
    anything else (exit 1 or 2, a signal, no start, a build or copy failure) `failed`. Only an
    `ok` bench that left `DIR/bench.json` has a report. `timeout` is recorded, not a tool
    failure: it is usually what `jpdf-cli` (or a reference engine) did, but a jpdf-bench that
    hangs on its own bug ends the same way, so a `timeout` every night may also be a broken
    bench. `failed` is a tool failure (item 10).
13. **Extra fuzz time** (after the bench; skipped when `--fuzz-seconds` is 0). This is fuzz
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
14. **Summary** `DIR/nightly.json`. An earlier one is removed before the bench (the
    `stale-summary` step, item 15; a failed removal is a tool failure, its own `Failed` entry
    whatever the write does: #352), and this night's is written after the fuzz pass (the
    `summary` step; tmp and rename, as in item 8; a failed write is a tool failure), keys in
    this order:
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
15. **Order.** One driver (`drive` in `xtask/src/nightly.rs`, ADR 0013 and
    docs/PRINCIPLES.md P4) calls the steps through an injected interface and records each
    step's outcome (`Ok`, `Finding`, `Infra`, `Failed`) under its name, in this order:
    `previous` (item 9), `build` (item 2), `executable` (item 10), `memory` (item 6),
    `watchdog`, `limits` (item 11), `corpus` (the cache and the manifest, item 3), `fetch`,
    `workdir`, `runs` (items 4–7), `results` (`regress.json`, item 8), `diff` (item 9),
    `stale-summary` (the earlier `nightly.json` removed, item 14), `bench` (item 12), `fuzz`
    (item 13), `summary` (this night's `nightly.json`, item 14; it gets the bench's and the
    fuzz pass's results from the driver), then `issues` (item 16). The `bench` and `fuzz`
    steps are `Failed` only for status `failed` (with `bench: <error>` or `fuzz: <error>`);
    `ok`, `timeout`, `crash` and `skipped` are `Ok` (ADR 0013 task 2, #361). What a failure
    stops:
    - any outcome but `Ok` of `previous`, `build`, `executable`, `memory`, `watchdog`,
      `corpus`, `workdir` or `results` stops the night: no later step runs;
    - `limits`, `fetch`, `runs`, `diff`, `stale-summary`, `bench`, `fuzz` and `summary` never
      stop it (a failed limits case or limits tool failure, a fetch integrity or infrastructure
      failure, a run that could not start, a diff that can't be written, a stale `nightly.json`
      that can't be removed, a failed bench or fuzz pass, a `nightly.json` that can't be
      written: the later steps run);
    - `issues` runs last, after a stop too, whenever the `limits` step ran (so not after a stop
      at `previous` … `watchdog`): it gets the limits cases, the previous night and this night's
      files, or no files when the night stopped before the runs. Its failure is a 1, which beats
      an `Infra` fetch's 75 (item 16);
    - after the fetch, no verified file (item 10) stops the night: the driver records its
      own `files` entry, `Failed` with `no regress file to run (<N> rows missing)`, unless
      the fetch was `Infra` (then the fetch's 75 stands alone).
    So a nightly that stops before the regress runs runs neither the bench nor the fuzz pass
    and writes no `nightly.json` (but files the limits cases' issues). The exit code is item 10's, computed from the log alone: a
    bench or fuzz tool failure makes it 1, which beats 75 and 4.
16. **Issues** (#302; the fuzz pass and the bench #383). The step reads only what
    the earlier steps returned in memory (item 15), never `DIR`, so it can't file from an
    earlier night's files (item 10). **Findings**, in this order:

    | signature | from | label | title |
    |---|---|---|---|
    | `limits:<case>` | a limits case whose status is not `pass` (item 11) | `crash` for `crash` or `timeout`, else `bug` | `nightly: limits case <case> ended <status>` |
    | `regress:<blob>` | a regress file with at least one `crash` or `timeout` run (item 5), in run order (blob-id order, item 4) | `crash` | `nightly: jpdf-cli crash or timeout on regress file <first 12 hex>` |
    | `regression:<blob>:<op>` | with a previous night (item 9): a run `ok` then and `error` or `not_implemented` now, by blob then op | `bug` | `nightly: regress regression: <op> on <first 12 hex> went from ok to <status>` |
    | `fuzz:<target>:<artifact>` | a fuzz target that crashed, hung or ran out of memory (item 13), in run order: each artifact copied for it to `DIR/fuzz/<target>/`, by name order | `crash` | `nightly: fuzz target <target> crashed: <artifact>` |
    | `fuzz:<target>` | the same target when no artifact was copied for it, or any copied artifact's name is outside the set below | `crash` | `nightly: fuzz target <target> crashed` |
    | `bench:timeout` | the bench ended `timeout` (item 12) | `perf` | `nightly: the bench ran out of time` |

    A run that becomes `crash` or `timeout` is the file's `regress:` finding, not a regression;
    a file whose blob is not 40 lowercase hex digits is never filed. A fuzz target or artifact
    name goes into a signature only when it is 1 to 100 bytes of ASCII lowercase letters,
    digits, `_` and `-` (libFuzzer names its artifacts `crash-<hex>`, `oom-<hex>`,
    `timeout-<hex>`): a target whose name is outside that set is never filed, and an artifact
    whose name is outside it (or whose path is not `fuzz/<target>/<name>`) is counted in the
    target's `fuzz:<target>` finding, not named. A target that ran clean is not filed for its
    artifacts, and a `slow-unit-` file (libFuzzer's slow input, not a crash) is never filed by
    name, so a failed target with only such files is its `fuzz:<target>` finding. The
    signature holds libFuzzer's input hash, so the same bug found again through another input
    files a new issue rather than a comment (the write cap bounds them per night), and a fuzz pass that failed as a tool
    (item 13) still files the failed targets that started. The fuzz and bench findings come
    only from a night that reached the fuzz pass's end (a stop before it, item 15, leaves
    them out). A drop in the bench's numbers from one night to the next is not a finding:
    the nightly is not given the previous night's `bench.json` (only `regress.json`, item 9),
    so there is nothing to compare. Signatures hold only ASCII lowercase letters, digits,
    `_`, `:` and `-`.
    **Filing**, only with `--file-issues`; with no finding, `gh` is not called at all. For the
    labels `bug` then `crash` (a limits case changes label with its status, so its open issue
    may carry the other one), then `perf` when a finding carries it (only `bench:timeout`
    does, and always with that label), `gh issue list --state open --label <label> --limit 1000
    --json number,body`. A list of 1000 or more is refused (it may be cut, so a duplicate could
    be missed). An issue matches a signature when its body starts with the agent tag and has
    the line `nightly-signature: <signature>` exactly; the lowest number wins. Then at most 20
    writes (creates plus comments; the rest are counted on stderr and wait for a later night):
    first the creates, then the comments, each in finding order, so a new finding is filed by
    the first night that has room after the creates before it, never starved by comments on
    known ones. A matched signature gets `gh issue comment <n> --body <comment>`, any
    other `gh issue create --title <title> --label <label> --body <body>`. A closed issue never
    matches, so a failure that returns after its fix opens a new issue. The body:
    ```
    🤖 jpdf-agent **Nightly finding**, filed by `cargo xtask nightly --file-issues` (specs/tooling/nightly.md item 16).

    nightly-signature: <signature>

    First seen: <YYYY-MM-DD> UTC[ (CI run <id>)].

    <detail>
    ```
    and the comment `🤖 jpdf-agent Seen again: <night>.` plus the detail. The detail lists, for
    a limits case, its id, size, status, exit, signal and time and `cargo xtask limits`; for a
    regress file, the blob, its manifest URL in backticks (left out with "URL not shown" when it
    is empty or holds a byte outside printable ASCII or a backtick) and each `crash` or
    `timeout` run's op, status, exit, signal and time; for a regression, the blob, URL, op and
    both statuses; for a fuzz finding, the target, the seconds per target and either the
    artifact's path under `DIR` and `cargo fuzz run <target> fuzz/artifacts/<target>/<artifact>`,
    or the counts of artifacts copied and not named and `cargo xtask fuzz --target <target>
    --seconds <S>`; for the bench, its file count, exit, signal and time and `cargo xtask bench`.
    No artifact bytes, fuzz log or tool error text goes into an issue either. **No `jpdf-cli` output (stderr) and no corpus bytes go into an issue**
    (CONSTITUTION 1, 13): to see the message, run the file again. The CI run id comes from
    `GITHUB_RUN_ID` when it is 1 to 20 digits, else it is left out. `gh` runs with
    `GH_PROMPT_DISABLED=1`, no pager, stdin on the null device and stderr to the nightly's
    stderr, its arguments as an argv list (no shell), killed after 120 s; its stdout is read up
    to 64 MiB and must be UTF-8. **Outcome:** `Ok` (stderr `xtask: nightly: issues: <n>
    findings: <c> filed, <u> updated, <s> over the cap of 20`, or `… not filed (no
    --file-issues)`), else `Failed` at the first `gh` failure (it couldn't start, exited
    non-zero, timed out, or its list output was not the expected JSON), with the counts done
    before it; later findings are not filed that night. A GitHub outage is a tool failure (1),
    not `Infra`: a rerun of the whole night would not be cheaper than the next night. So with
    `--file-issues`, a night without network that also has a finding (a failed limits case)
    exits 1, not the fetch's 75 (item 10: `Failed` beats `Infra`).
    The CI job (`.github/workflows/ci.yml`, protected) has `issues: write` and `GH_TOKEN`; it
    needs `--file-issues` added to its command (ops/proposals/nightly-file-issues.md).

## Persistence
No document persistence. GitHub issues and comments (item 16, only with `--file-issues`).
`regress.json`, `regress-diff.json`, `limits.json` (item 11),
`nightly.json`, `bench.json` (and jpdf-bench's `bench.json.work/`), `bench-corpus/`, `fuzz/` and
`work/` under `DIR` (and the `.tmp` names after a failed rename, item 10). In the repository's
ignored paths: the fuzz pass's seeds and corpus under `fuzz/corpus/`, its artifacts under
`fuzz/artifacts/`, its logs under `target/fuzz-logs/` (specs/tooling/fuzz.md). The previous night's file is passed in by the caller (with no previous file there are no regression findings, item 16): the CI step that downloads
it needs the owner (`.github/` is protected) and is proposed in ops/proposals/nightly-previous-night.md (#497).

## Undo
Not applicable (development tool).

## Errors
Each tool failure prints `xtask: nightly: <problem>` on stderr: `release build failed (<status>)`,
the corpus tool's manifest or cache error, `cannot start the engine watchdog (/bin/sh)`,
`ulimit -v 2097152 is not enforced here (the judge's Linux limit)`, `cannot write <path>: <io error>`,
`fetch: <url>: <problem>` lines then `fetch: <summary>`, `previous results <path>: <problem>`,
`no regress file to run (<N> rows missing)`, `no executable jpdf-cli at <path> after the build`,
`<N> runs could not start <path>` (and `cannot remove <path>: <io error>` when a stale diff
can't be removed), `fetch: <N> rows failed the integrity check` (after the fetch's own lines),
`limits: <problem>`, `bench: <error>` and `fuzz: <error>` (each after its step's line below),
`cannot remove <path>/nightly.json: <io error>`, and `issues: <problem> (<c> filed, <u> updated before it)`
(item 16: `cannot start gh: <io error>`, `gh issue <cmd>: <exit status>`, `gh issue <cmd>: no
answer in 120 s`, `gh issue list --label <label>: <problem>`). Each tool failure is printed when its step ends (the step
log prints each `Failed` reason as it is recorded); `JUDGE-INFRA-FAILURE` is printed last
(item 10). The bench and the fuzz pass each print one line,
`xtask: nightly: bench: <status> (<n> files)` and
`xtask: nightly: fuzz: <status> (<t> targets, <S> s each, <a> artifacts)`; when the status is
`failed`, the step's tool failure line follows (`bench: <error>`, `fuzz: <error>`), whose
error is the same text as in `nightly.json` (`release build failed (<status>)`,
`cannot copy <path>: <io error>`, `cannot start <path>`, `jpdf-bench ended with "<status>"`,
`fuzz/Cargo.toml not found …`, the fuzz tool's messages). A usage error prints the problem and the usage line.

## Limits
- Per run: CONTRACT §1 time limits (item 4) and the 2 GiB address-space limit where the OS
  enforces it (item 6); stderr kept ≤ 4 KiB per run (the last bytes; the reader's buffers: a deque reserved once for 4 KiB and one 8 KiB chunk), stdout never read.
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
- Issues (item 16): ≤ 20 writes per night; each `gh` call ≤ 120 s and ≤ 64 MiB of stdout;
  lists of < 1000 open issues per label. Exact-boundary tests: 20 findings give 20 writes and
  21 give 20 plus one counted; a list one short of its limit is used and one at it refused, and stdout of exactly the cap
  is read while one byte more fails (both limits passed in small, the real values pinned); a
  fuzz artifact name of exactly 100 bytes is named in a signature and one of 101 is only
  counted in its target's finding.
- Bench: `--bench-minutes` ≤ 120 of wall time, killed at the limit; a sample of ≤ 20 files.
- Fuzz: `--fuzz-seconds` ≤ 3600 per target (libFuzzer's `-max_total_time`), so the pass takes
  ⌈targets / cores⌉ × S plus the build; ≤ 100 artifacts copied per target.
- Exact-boundary tests: `--limit` 200 000 passes and 200 001 fails, 0 fails; `--max-minutes`
  1440 passes, 1441 and 0 fail; `--bench-minutes` 0 and 120 pass, 121 fails, and 1 gives a
  60 s limit; `--fuzz-seconds` 0 and 3600 pass, 3601 fails; a bench sample of exactly 20
  verified files, and 21 give 20; exactly 100 new artifacts are copied and a 101st is not; a run at elapsed = budget − 1 ns runs, at elapsed = budget
  it is `not_run`; a previous file of exactly the cap is read and one byte more fails, and
  a file list of exactly the cap passes and one more fails (both caps passed in small by the
  tests, which also pin the real values); stderr is cut at exactly 4096 bytes: a tail of exactly the cap is kept whole and one byte more drops the first byte, and a 5000-byte log ending in a panic line keeps that line.
- No panics: no `unwrap`, `expect` or indexing on input-derived data (workspace lints);
  counts use saturating arithmetic; `ms` is `u128` milliseconds written as a JSON number.

## Security
- **Sealed corpus (CONSTITUTION 13):** the tier is fixed to `regress` (item 3); no flag takes a
  manifest path or a tier name.
- **Hostile bytes:** the nightly never parses PDF bytes. `jpdf-cli` runs as a separate process
  under the judge's limits, in its own group, with the watchdog. Only files that pass the blob
  check run (item 3). Paths given to it are absolute.
- **Hostile output:** `jpdf-cli`'s stderr is data (CONSTITUTION 1). It is cut to its last 4 KiB, decoded
  lossily and JSON-escaped (`jpdf_bench::json::string`) before it is written, so it can't
  break the results file. The previous results file is untrusted input: bounded, strictly
  parsed, every value checked (item 9); its blob ids and URLs are only compared and escaped.
- **Shell:** the memory wrapper is a fixed script; the program and its arguments are passed as
  `$0` and `$@`, never interpolated.
- **Bench and fuzz:** the bench reads only files that passed the blob check (copies, item 12)
  and runs under the memory wrapper and a time limit; the fuzz pass is `cargo xtask fuzz`'s,
  with its limits. Artifacts are copied, never opened or parsed; their names are escaped.
  Only regular files are copied (a symlink planted in `fuzz/artifacts/` is not followed).
- **Issues (item 16):** every body and comment starts with the agent tag (CONSTITUTION 1); no
  stderr, no corpus bytes and nothing from the sealed corpus (the tier is `regress`); titles
  and signatures are built from xtask's own names, statuses, numbers and checked blob ids, the
  URL from the repository's manifest is shown only inside backticks and only when it can't
  leave them. Fuzz target and artifact names come from file names under `fuzz/` (written by
  `cargo fuzz` and libFuzzer, so not xtask's own): they reach a title, a signature or a body
  only when they pass item 16's character set and length check, which leaves no character
  that Markdown, a mention (`@`) or a signature line could use. What keeps another account's issue from capturing a finding is the label filter:
  only open issues labeled `bug` or `crash` (and `perf` on a night with a bench timeout) are read, and only accounts with triage rights can
  set labels. The tag at the start of the body (followed by a space) and the exact signature
  line only keep other issues of those labels from matching; the author is not checked, so a
  tagged issue an owner or agent session labels with the same line captures it (it gets the
  comments). Nothing is written without `--file-issues`. `gh` gets
  an argv list, never a shell string.
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
  - A stderr reader that keeps the head, drops the end of the last read, or keeps one byte too
    many or too few: the fake `jpdf-cli`'s 5000-byte log ends in a panic line that must be the
    end of the kept 4096 bytes (it fails with `Keep::Head`), and `read_tail`'s unit tests cover
    the exact cap, one byte past it, a cap across several read chunks, an `Interrupted` read
    (retried), a hard read error (stops, keeps what it read) and an endless reader that the
    stop flag ends mid-drain. Not tested: that `run_child` sets the flag after collecting
    the run (one line after `recv_timeout`; no hook observes the reader thread), and the
    memory bound (stated, not measured).
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
  - A driver that runs steps in another order, skips one, stops where the spec goes on (or
    goes on where it stops), or hands a step the wrong data: the driver tests (ADR 0013
    "Verification") run a recording fake through every step `Ok` (the exact call list, with
    the jobs, paths and flags each step got), each step failing alone, and the no-file stop;
    `steps::exit_code` has a table test over every pair of outcome kinds, and the
    `JUDGE-INFRA-FAILURE` line a test for a 75 log and its near miss (`Infra` in a log whose
    code is 1). What still passes: a real adapter that maps an error to the wrong outcome (each
    is one expression, checked by the reviewer against items 10–13; the bench's, the fuzz
    pass's and `nightly.json`'s mappings are functions in `extra.rs` with their own tests:
    `bench_outcome`, `fuzz_outcome`, `remove_summary`, `write_summary`), and a `run` that calls a
    real step outside the driver (it only builds `Real` and calls `drive`).
  - An issue step that files duplicates, misses one, or matches the wrong issue: a repeat
    night on a fake `gh` (which acts on its real arguments: lists filter by state and label,
    newest first, creates add issues, comments attach) comments on the two issues the first
    night filed and creates nothing; near misses (a closed issue, an untagged body, the line
    inside other text, a longer signature, a tag without its space, a label that is neither `bug`
    nor `crash`) don't match, a limits case that changes label keeps its issue, and a night over
    the cap files the rest the next night, and of two matches the
    lowest wins (hand-mutated: dropping the tag check or the `min` fails a test). The findings
    have one test per source, each with statuses that must not count (`pass`, `error` → `not_implemented`,
    `ok` → `crash`, a missing file). No `gh` call without findings or without `--file-issues`.
    What still passes: a real `gh` whose output differs from the fake's (the list's JSON
    shape is checked strictly, so a change fails the step loudly).
  - Fuzz and bench findings (#383) that file a clean target, drop an artifact, let a name
    outside the set through, or file a bench that didn't time out: the tests have each kind
    of target and every bench status, and hand mutants (no `ok` check, no unnamed-artifact
    finding, `<` for `<=` at the length cap, an empty name allowed, upper case allowed, the
    file name taken without its `fuzz/<target>/` prefix check, a bench filed for any non-`ok`
    status, no `perf` list) each fail a test. A driver that hands `issues` no bench or fuzz
    results: the fake's `issues` line carries them, as `summary`'s does. What still passes:
    the `Real` adapter calling `findings` alone (one expression, checked by the reviewer;
    the order is `night_findings`, which has its own test).
- Not covered by tests: the real build, the real fetch (network), the CI wiring, the real
  adapters (`Real` in `xtask/src/nightly.rs`; the functions they wrap have their own tests);
  a real fuzz pass (the nightly toolchain and minutes of fuzzing:
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
implemented. The limits part (item 11) adds about 15 s before the regress runs (at most 140 s of
runs). The CI job has 240 min (`timeout-minutes`). With the defaults the worst case of the
budgeted parts is 120 + 1 (one run past the budget) + 20 (bench) + ⌈3 targets × 1200 s / 60⌉ =
60 (fuzz on one core; 20 on the 4-core runner) = 201 min. A test checks that this sum, for the
number of fuzz targets in `fuzz/fuzz_targets/`, leaves at least 30 min of the 240 for the rest:
the limits part, the fetch, the release builds, the cargo-fuzz install (built from source on
CI: this job has no cache for it) and the fuzz targets' build. That 30 min is an estimate; none
of those steps has been timed in the CI job yet (the first nightly run will show it). A local
run with `--limit 30 --bench-minutes 5 --fuzz-seconds 10` took 58 s (before the limits part
was added).

## Tests
`xtask/src/nightly/extra/tests.rs` (the budget sum, the bench sample and its staging, the bench
statuses with a fake exec and build, the stale `bench.json` and `nightly.json` removal, the
artifact copy and its cap (and a copy that fails midway), the fuzz pass with a fake
`fuzz::run_report` (the snapshot before the pass, only started targets, each target's `ok`),
a bench sample that can't be copied, a stale `nightly.json` that can't be removed, the fuzz
statuses, a tree without `fuzz/`, `nightly.json`'s shape, and the outcomes of the
`stale-summary`, `bench`, `fuzz` and `summary` steps: only a `failed` bench or fuzz pass, a
failed removal or a failed write is `Failed`), `xtask/src/nightly.rs` unit tests (argument parsing and boundaries, the deadline, the classifier
and memory wrapper with fake `jpdf-cli` scripts, missing files, the results JSON, the previous-file
validator, the diff), `tools/bench/src/engine.rs`
`run_child_reports_the_exit_code_or_the_signal`, `tools/corpus/tests/corpus.rs`
`is_verified_accepts_only_the_exact_file`. The limits part (item 11): `xtask/src/limits/tests.rs`
(specs/tooling/limits.md "Tests"). The order and the exit code (items 10, 11, 12 and 15):
`xtask/src/nightly/tests/driver.rs` (a recording fake of the steps: the full order with each
step's data, one run per failing step, the no-file stop, the limits part's codes, item 10's
4, in `exit_codes_with_the_limits_cases`; item 14's order, the removal before the bench and the
write after the fuzz pass, with the bench's and fuzz pass's results handed to `summary`; and
#352's case, a failed removal as its own entry when the write succeeds) and `xtask/src/steps.rs` (the `exit_code` table, the
`JUDGE-INFRA-FAILURE` line, the log). The `issues` step (item 16): `xtask/src/nightly/issues/tests.rs`
(the findings, the fuzz and bench findings (#383: named and unnamed artifacts, a clean target,
a target name outside the set, slow units, a failed pass, each bench status, the filing order, and the
`perf` list on a bench-timeout night only, with its repeat night), the bodies, the fake `gh`'s repeat night and near misses, both caps, a `gh`
failure mid-way, the night label, and the real runner with fake `gh` scripts: arguments,
exit codes, a hang killed at its limit, a missing program, non-UTF-8 output), and in
`driver.rs` its place last, after a stop, and the data it gets (the bench's and the fuzz
pass's results only from a night that reached them).
