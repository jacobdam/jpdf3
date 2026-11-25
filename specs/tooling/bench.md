# Bench harness (`tools/bench`, `cargo xtask bench`)

Area: tooling
ISO 32000-2 clauses: none for the tooling itself; the generated sample uses §7.5.2–§7.5.5 through jpdf-testgen
Journeys: none (development tool; PLAN §5 "CI", §8 M0; issue #18)
Status: implemented

## Behavior
1. **`tools/bench`** is the workspace crate `jpdf-bench` (library + binary, no third-party
   dependencies; it depends only on `jpdf-testgen`). It times `jpdf-cli` as an **external
   program**, and the same work done by reference engines, also only as external programs
   (CONSTITUTION 16: no engine is linked, read or copied).
2. **Usage.**
   ```
   jpdf-bench --cli PATH --out FILE.json [--corpus DIR] [--sample N] [--runs R] [--warmup W]
              [--mutool PATH|none] [--pdfium PATH|none] [--work DIR]
   ```
   - `--cli`: the `jpdf-cli` binary (required).
   - `--out`: the JSON report (required). Parent directories are created.
   - `--corpus DIR`: bench the `*.pdf` files directly inside `DIR` (not recursive), sorted by
     file name (byte order), the first `--sample N` of them (default 20, 1 ≤ N ≤ 10 000).
     Without `--corpus`, the **generated sample** is used (item 3). Point it only at corpus
     tiers agents may read (smoke, regress); the sealed corpus is never benched.
   - `--runs R` timed runs per measurement (default 5, 1–100), after `--warmup W` untimed runs
     (default 1, 0–10).
   - `--mutool`: default `mutool` from `PATH` if `mutool -v` runs; `none` disables it.
   - `--pdfium`: default `pdfium_test` from `PATH` if it starts (run once with no arguments;
     any exit counts as present); `none` disables it.
   - Engine probes (`mutool -v`, bare `pdfium_test`) run under a 5 s limit with the same
     process-group kill as item 7. stdout goes to the null device. mutool's stderr goes to a
     pipe that is read up to 4 KiB and then closed, so a flooding program gets EPIPE and costs
     no memory or disk; pdfium's stderr goes to the null device. `version` (first line, ≤ 200
     chars) is kept only if it starts with `mutool version`; otherwise it is `null`. Probes
     run before anything is written, so a bad explicit path exits 2 with no files created.
   - An explicit `--mutool`/`--pdfium` path that cannot be started is a usage error; the
     default lookup that finds nothing just leaves the engine out.
   - `--work DIR`: scratch directory for the generated sample and the engines' output files;
     default `<out>.work/` next to the report. The bench overwrites its own file names there
     and never deletes anything. A work dir equal to or inside the corpus dir is a usage
     error (exit 2), checked before anything is written, so the bench can never overwrite
     or bench its own files.
   - `--cli`, `--work` and `--corpus` are made absolute before they go into any argv, so
     a path starting with `-` can't be read as a flag.
   - Unknown flags, missing values, out-of-range numbers or a missing `--cli`/`--out` → usage on
     stderr, exit 2. A bench that ran writes the report and exits 0, whatever the engines did.
     A `--cli` that is not a file and I/O failures (cannot write the report, cannot read the
     corpus dir) → exit 1, and no report is written. The report is written to `<out>.tmp`
     and then renamed, so a failure part-way never leaves a truncated report.
   - Corpus entries are followed through symlinks. An entry whose metadata can't be read
     (dangling symlink, removed during listing) is skipped. A non-UTF-8 file name is
     reported lossily (U+FFFD), so two such names can look the same in the report.
3. **Generated sample** (default; offline, deterministic): `jpdf_testgen::minimal_pdf` with 1,
   10, 100 and 1000 empty pages, named `gen-pages-0001.pdf` … `gen-pages-1000.pdf`. It
   exercises the harness and the open path while the engine can't render; the real
   comparison uses `--corpus` once corpus tiers exist (#22).
4. **Operations** (page 1 only, matching what the judge times):
   | id | jpdf-cli | mutool | pdfium_test |
   |---|---|---|---|
   | `info` | `info F` | `show F pages` | — |
   | `render72` | `render F --page 1 --dpi 72 --out W` | `draw -q -r 72 -o W F 1` | `--png --scale=1 --pages=0 F` |
   | `render150` | `render F --page 1 --dpi 150 --out W` | `draw -q -r 150 -o W F 1` | `--png --scale=2.083333 --pages=0 F` |
   | `text` | `text F --page 1` | `draw -q -F txt -o W F 1` | `--txt --pages=0 F` |
   `pdfium_test` writes its output next to its input, so the bench copies each file into the
   work dir first. The PDFium argument lists are unit-tested but **not run in CI**: no runner
   has a PDFium binary today.
5. **Timing.** A run is the wall time from spawn to exit (process start included, like the
   judge's `mu_ms`). stdout and stderr go to the null device. A measurement = `W` warmup runs
   + `R` timed runs; its time is the **median** of the `R` runs, in microseconds.
6. **Status.** Each run's exit is classified with CONTRACT §1: 0 `ok`, 2 `error`, 3
   `not_implemented`, a time-limit kill `timeout`, anything else (including a signal or a
   spawn failure) `crash`. The first non-`ok` run stops the measurement and sets its status;
   its time is `null`. **A failed or unimplemented operation is never reported as a time**,
   and never as zero. For reference engines the same mapping is applied, but only `ok`
   counts; any other status just removes that file from the ratio.
7. **Time limits** per run, from CONTRACT §1: info 20 s, render 30 s, text 60 s. Each child
   runs in its own process group (`process_group(0)`). At the limit, the whole group gets
   SIGKILL (`kill -s KILL -- -PGID`, the `/bin/sh` builtin, so no `kill` binary is needed)
   and the child is reaped, so a wrapper script's children
   can't outlive it and skew later runs. When a child exits on its own, its group also
   gets SIGKILL (right after the reap: std can't wait without reaping, so the group id stays
   reserved only while a member is left; if none is, the kill finds nothing unless the id
   was reused within the few milliseconds the `/bin/sh` group kill takes to start), so a background child an engine leaves behind
   dies too (#83).
   **Watchdog.** Before any engine runs, the bench starts one `/bin/sh` watchdog in its own
   process group, or refuses to run (I/O error, exit 1). The bench writes each engine's
   group id to the watchdog's stdin when the engine starts and `-` once its group is gone.
   At end of input, which comes whenever the bench exits (Ctrl-C, SIGTERM, SIGKILL, a
   panic), the watchdog kills the last registered group. Ctrl-C reaches only the
   terminal's foreground group, so the engine (own group) and the watchdog (own group)
   don't get it; the watchdog is what stops the engine. After a child exits on its own,
   `-` is written right after the reap and before the group kill, so a bench killed during
   that kill can't make the watchdog signal a reused id (a background child may then
   survive: the group kill takes one `/bin/sh` spawn, milliseconds). After a timeout, `-` is
   written before the reap, while the zombie leader still reserves the id (unless the
   bench itself dies between the group kill and `-`: init then reaps the leader). These
   windows, plus spawn → registration and a normal exit's reap → `-`, are microseconds
   wide and not covered; #134 tracks closing them with a per-run group holder. Only lines `2`..`9999999` without leading zeros kill
   (`kill -- -1`, also written `-01`, would hit every process of the user; `-0` is the
   watchdog's own group).
   The watchdog holds one group, so runs never overlap (the bench is sequential).
   Children that leave the group on purpose are out of scope.
8. **Summary per operation and engine:** status counts; `median_ms` and `p95_ms` over the
   per-file times of `ok` measurements (nearest-rank p95; `null` when no file is `ok`).
   **Ratio** per operation and reference engine: the geometric mean over files where both
   jpdf and the reference are `ok` of `max(jpdf_us, 1) / max(ref_us, 1)`, plus
   `ratio_samples`; `null` with 0 samples. This is the same shape as the judge's
   `bench_ratio` (geometric mean of per-page ratios), so local and judge trends agree.
9. **Report** (`schema: "jpdf-bench/1"`, UTF-8 JSON, keys in fixed order, numbers with at
   most 3 decimals):
   ```json
   {"schema":"jpdf-bench/1",
    "machine":{"id":"<16 hex>","os":"macos","arch":"aarch64","cpu":"Apple M2","logical_cpus":8,"memory_bytes":17179869184},
    "revision":"<git HEAD or null>",
    "config":{"sample":"generated"|"corpus","files":4,"runs":5,"warmup":1},
    "engines":{"jpdf":{"present":true},"mutool":{"present":true,"version":"mutool version 1.28.6"},"pdfium":{"present":false,"version":null}},
    "operations":{"info":{"engines":{"jpdf":{"ok":4,"error":0,"not_implemented":0,"timeout":0,"crash":0,"median_ms":1.234,"p95_ms":2.5},"mutool":{…}},
                          "ratio":{"mutool":{"value":0.8,"samples":4},"pdfium":{"value":null,"samples":0}}}, …},
    "files":[{"name":"gen-pages-0001.pdf","bytes":512,"results":{"info":{"jpdf":{"status":"ok","ms":1.2},"mutool":{"status":"ok","ms":1.5}}, …}}]}
   ```
   Absent engines are omitted from `operations.*.engines` and `files[].results`; an engine with
   no command for an operation (pdfium `info`) is omitted from that operation. File names
   are base names only, never absolute paths.
10. **Machine identification.** `id` = FNV-1a 64-bit (hex) of `os|arch|cpu|logical_cpus|memory_bytes`.
    `cpu` comes from `sysctl -n machdep.cpu.brand_string` (macOS) or the first `model name`
    in `/proc/cpuinfo` (Linux); `memory_bytes` from `sysctl -n hw.memsize` or `MemTotal` in
    `/proc/meminfo`. Unknown values are `"unknown"` / `0`, which still hash. No host name,
    user name or serial number is recorded. Trends compare reports with the same `id` only.
    `logical_cpus` is `available_parallelism()`, which follows cgroup quotas and CPU
    affinity. So a CPU-limited container gets its own `id`, which is what we want: its times
    are not comparable to the bare host's.
    `revision` is `git rev-parse HEAD` in the current directory. `cargo xtask bench` runs
    it from the repository root.
11. **`cargo xtask bench [--out DIR] [--corpus DIR] [--sample N] [--runs R] [--warmup W]`** builds
    `jpdf-cli` and `jpdf-bench` with `$CARGO build --release --locked` (the judge's build) and
    runs the bench with `--cli <target>/release/jpdf-cli --out DIR/bench.json` (`<target>` =
    `$CARGO_TARGET_DIR` or `target`; `CARGO_BUILD_TARGET_DIR`/`CARGO_BUILD_TARGET` are not
    supported, matching the judge's plain build; default
    `DIR` = `target/xtask-out`). It forwards the other flags. Exit: the bench's exit code.
    `cargo xtask nightly` runs the same build and then the bench on a sample of the regress
    tier (specs/tooling/nightly.md item 12, #301). Keeping the trend and the 5% regression
    check of PLAN §5 (two reports with the same machine id) are not built yet (#19).

## Persistence
The report is the only output. The dashboard (#15) and the nightly (#19) read it.

## Undo
Not applicable.

## Errors
Usage errors exit 2, I/O errors exit 1, both with one line on stderr. Engine failures are data
in the report, never a bench failure.

## Limits
Sample ≤ 10 000 files, runs ≤ 100, warmup ≤ 10; per-run time limits as in item 7. The bench
reads only file names and sizes from the corpus; engines read the PDFs, under their own limits.
Arithmetic on times uses `u128`/`f64` from `Duration`, saturating where it converts.

## Security
**Sealed corpus (CONSTITUTION 13).** The bench can't tell which directory holds the sealed
files without reading the sealed manifest, and reading it would itself break the rule. The
guard therefore sits where files are fetched: `tools/corpus` (#22) only downloads the
smoke/regress/wide tiers, and the sealed tier never exists on agent machines. Only the
judge fetches it, in CI. `--corpus` is only ever given those tier directories.

Corpus PDFs are hostile, but the bench never parses them: it only passes paths to external
programs, with arguments as an argv list (no shell). Report strings are JSON-escaped (file
names can hold any character). The bench runs only `--cli`, `mutool`, `pdfium_test`, `git`
(for `revision`), `/bin/sh` (group kill and the watchdog) and `sysctl` on macOS. The
watchdog's stdin pipe is close-on-exec, so engines never hold it open (which would keep the
watchdog from seeing the bench exit). It reads only numbers the bench wrote; any other line
kills nothing.

## Accessibility
Not applicable (CLI tool).

## Platforms
macOS and Linux. Windows is out of scope (the judge runs Linux; development is on macOS).

## Performance
The generated sample with defaults is 4 files × 4 operations × up to 2 engines × 6 runs ≈ 200
process starts, a few seconds. The tool's overhead per run is the engine's spawn, polling it every 0.2 ms,
one `/bin/sh` spawn for the group kill after it exits (outside the timed span), and two
short writes to the watchdog.

## Tests
`tools/bench/tests/bench.rs` and unit tests in the crate:
- argument parsing: defaults, every range boundary, unknown flag → exit 2;
- exit classification for 0, 2, 3, 1, signals and timeouts (shell-script fake engines in a
  test temp dir under `target/`). A timed-out wrapper script without `exec` must not leave
  its grandchild (a nested shell's `sleep`) alive, which only a process-group kill reaches;
  the timeout fires no earlier than the limit and less than 3 s after it. The test warms the script up first; a
  run that times out before the wrapper wrote its child's pid proves nothing, so it
  retries at 3, 6, 12 and 24 s and fails only if no attempt got that far (#75);
- an engine that exits 0 leaving `sleep 30 &` behind: that child is gone within 2 s;
- the watchdog script alone (unit test) against a `sleep` in its own group: end of input
  after its id kills it; after `<id>`, `-` it survives; a later id replaces an earlier one
  (a second sleeper with the earlier id survives); lines other than digits (`-<id>`, `<id>x`)
  kill nothing. Only the sleepers' ids are written, never `1`;
- the watchdog's id guard, with `kill` replaced by `echo` (so no signal is sent): `2`,
  `12345`, `9999999` reach the kill; empty input, `-`, `0`, `1`, `00`, `01`, `012345`,
  `10000000`, `4294967297`, `12x`, `-12`, ` 12` and `<id>`,`-` don't;
- a recorder (test builds only) sees `probe` register the group, unregister it, then kill
  the group for a normal exit, and for a timeout register, kill the group, unregister, then
  reap, in that order;
- the bench under Ctrl-C (SIGINT to its process group), SIGTERM and SIGKILL while an engine
  runs: the engine's grandchild (in the engine's group) is gone within 5 s. Without the
  watchdog, or with the watchdog in the bench's group, the INT case fails;
- ratio direction and unit: fake jpdf (~200 ms) against fake mutool (~50 ms) must give a
  ratio > 1.5 and per-file `ms` ≥ 150 (a µs value would be ~200 000). A probe whose output
  isn't a mutool version line records `version: null`;
- probes: usage text with an absolute path gives `version: null` and no path in the report;
  a stderr flood (`yes`) gives the capped version line; a hanging probe is killed at 5 s and
  the engine still counts as present (both within 15 s);
- a fake `pdfium_test` must receive `<work>/pdfium-input.pdf` and is absent from `info`;
- a work dir equal to or inside the corpus → exit 2 with nothing created there; a dangling
  `*.pdf` symlink is skipped;
- a not-implemented engine yields `null` times and is excluded from ratios (never zero);
- statistics: median of even/odd counts, nearest-rank p95 at 1, 19, 20 and 21 values,
  geometric mean, ratio with sub-microsecond clamps;
- JSON escaping of control characters, quotes, U+2028/U+2029 and non-ASCII names; reports
  are checked with Python's strict `json` parser (`python3 -I`, present on macOS and the
  Linux runners), including an odd corpus file name that must round-trip;
- end to end with a fake `jpdf-cli` that behaves like the M0 stub (`info` exits 0, the rest
  exit 3): `info` measurements are `ok` with positive times, `render*`/`text` are
  `not_implemented` with `null` times and 0 ratio samples; with a fake `mutool`, the `info`
  ratio has 4 samples and `render72` has none; no absolute path appears in the report;
- exit codes of the bench itself (usage 2, missing `--cli` or corpus 1, no report written);
- `cargo xtask bench` argument forwarding (xtask unit tests). The real `jpdf-cli` + `mutool`
  run is manual (`cargo xtask bench`): its release build is too slow for every test run.
