# Adversarial resource-limit harness (`cargo xtask limits`)

Area: tooling
ISO 32000-2 clauses: §7.3.4.2 (literal strings), §7.3.10 (indirect objects), §7.5.4–§7.5.6 (xref tables, trailer, incremental updates), §7.7.3 (page tree); the files are hostile, so most of them break these clauses on purpose
Journeys: none (development tool; issue #25, specs/cli/jpdf-cli.md §Limits, ops/judge/CONTRACT.md §1)
Status: implemented

## Behavior
1. **Usage.**
   ```
   cargo xtask limits [--out DIR] [--mib N]
   ```
   - `--out DIR`: where `limits.json` and the work file go (default `target/xtask-out`).
   - `--mib N` (64 ≤ N ≤ 511, default 500): the target size of each attack file in MiB. 511 is
     the largest size under `jpdf-cli`'s 512 MiB `max_file_size`, so no file is refused by its
     size alone; at every value from 64 up every class reaches half its target (item 3).
   - Each flag at most once; an unknown flag, a missing value, a value out of range or not a
     plain decimal number is a usage error (exit 2) before anything runs.
   `cargo xtask nightly` runs the same cases at the default size (specs/tooling/nightly.md
   item 11).
2. **Build and limits.** `jpdf-cli` is built and checked exactly as the nightly does it
   (specs/tooling/nightly.md items 2 and 10: release, `--locked`, an executable file). The
   memory probe and wrapper are the nightly's (its item 6): on Linux every run is under
   `ulimit -v 2097152` (the judge's 2 GiB `RLIMIT_AS`), and a probe that fails there is a tool
   failure; elsewhere the runs go without it and `limits.json` says `"memory_limit_kib":null`.
   The bench watchdog is started before the runs (specs/tooling/bench.md item 7).
3. **Files.** For each case in item 4's order the generator streams one file to
   `DIR/work/limits.pdf` (through a 1 MiB buffer; the largest piece held in xtask's memory is
   one class's structural body, under 70 MiB). Its size must be at least half the target and at
   most 512 MiB; a file outside that range, a write error, or a file whose size on disk differs
   from the bytes written is a tool failure. The file is removed after its run, so at most one
   file of ≤ 512 MiB is on disk at a time.
4. **Cases** (T = the target size in bytes). Each hostile class is the scaled form of an
   engine test in crates/jpdf-engine/tests/open.rs; its counts stay under every limit except the
   one it is built to reach, so the expected error names the defence it checks.

   | id | file | expected |
   |---|---|---|
   | `nested-strings` | min(T/256, 600 000) page objects, each opening a literal string that holds the next object's header (open.rs `objects_nested_in_strings_are_rejected_as_overlapping`). The first kid's string is all later headers and closes, 600 000 × 45 bytes ≈ 26 MiB, under the 32 MiB string limit. | exit 2, `malformed PDF: indirect objects overlap` |
   | `shared-body` | min(T/1024, 100 000) kid headers on one comment line, all sharing one body with a string of min(T/2, 32 MiB − 4096) bytes (open.rs `overlapping_objects_sharing_one_body_are_rejected`) | exit 2, `malformed PDF: indirect objects overlap` |
   | `stacked-prev` | 1000 xref sections (under the 1024-section limit), each trailer's `/Prev` pointing at the next section hidden inside its own string; min(T/2, 30 MiB) filler bytes in the innermost string (open.rs `overlapping_prev_sections_are_rejected`) | exit 2, `malformed PDF: cross-reference sections overlap` |
   | `xref-entries` | one xref subsection of about T/20 entries (26 M at 500 MiB, 3.3 M at 64 MiB), all over the 2^21-entry limit | exit 2, `limit exceeded: cross-reference entries` |
   | `max-per-object` | max(1, ⌊T/P⌋) page pairs, P being one pair's two payloads (about 37.4 MiB): one page holding a string of 32 MiB − 4096 bytes, one holding a dictionary of 2^19 − 1024 entries (just under 2^20 objects in one parse); 13 pairs (486 MiB) at 500 MiB | exit 0 |
   | `dense-kids` | min(T/128, 1 000 000) distinct, non-overlapping pages (the page limit at 500 MiB), each padded with a string to about T/pages bytes | exit 0 |
   | `long-comment` | a one-page file whose catalog dictionary holds one comment line of about T bytes before `/Pages` | exit 0 |

   `nested-strings`, `shared-body` and `stacked-prev` reach their target through a comment line
   after the header that the engine never lexes (objects are found through the xref): there the
   size stresses reading and holding the file, while the attack itself is the structure after
   it. The other four classes are their target size.
5. **Runs and verdicts.** Each file gets one run, `jpdf-cli info <absolute path>`, with the
   judge's 20 s `info` limit, through `jpdf_bench::engine::run_child` as in the nightly (its own
   process group, killed at the limit; stdin and stdout on the null device; the last 4 KiB of
   stderr kept, specs/tooling/nightly.md item 5). The verdict:
   - `crash`: the run crashed (a signal, an exit code other than 0, 2 or 3, or a spawn failure);
   - `timeout`: killed at the time limit;
   - `pass`: an exit-0 case exited 0, or an exit-2 case exited 2 with `jpdf-cli: <expected text>`
     in its stderr;
   - `wrong_error`: an exit-2 case exited 2 with any other stderr;
   - `wrong_exit`: any other exit (0 where 2 is expected, 2 where 0 is, or 3).
   Running out of memory under the limit aborts `jpdf-cli` (SIGABRT), a `crash`.
6. **Results** `DIR/limits.json` (written to `limits.json.tmp`, then renamed; keys in this
   order):
   ```json
   {"schema":1,"target_bytes":524288000,"memory_limit_kib":2097152,"time_limit_ms":20000,"failed":0,
    "cases":[{"id":"nested-strings","bytes":524288000,"status":"pass","exit":2,"signal":null,"ms":248}, …]}
   ```
   `failed` counts cases whose status isn't `pass`; those cases also carry `stderr` (lossy
   UTF-8, or `cannot start jpdf-cli`). stderr gets one line per case and a summary line.
7. **Exit codes:** 2 usage; 1 a tool failure (build, probe, watchdog, work directory, a file
   write or size, writing `limits.json`); 4 a case did not pass; 0 every case passed. A tool
   failure in a case stops the run before the later cases, and nothing is written.

## Persistence
`DIR/limits.json` only; the work file is removed after each run.

## Undo
Not applicable.

## Errors
As item 7. Messages start with `xtask: limits:` (from the nightly, `xtask: nightly: limits:`).

## Limits
- `--mib` 64–511: boundary tests at 64 and 511 pass, 63 and 512 fail (`parses_flags_and_defaults`,
  `usage_errors`).
- The size check: exactly T/2 and exactly 512 MiB pass, one byte past either fails
  (`the_size_check_is_exact_at_both_ends`); every class meets it at 64, 500 and 511 MiB
  (`every_case_reaches_its_size_at_the_bounds`, which also generates at 75, 76 and 79 MiB, where a
  `max-per-object` count of ⌊T/40 MiB⌋ fell short: PR #349 R1.1); `max_per_object_fits_at_every_size`
  checks that class's count at every `--mib` from 64 to 511.
- The engine limits each class sits under or over are the defaults in specs/cli/jpdf-cli.md
  §Limits; `default_size_parameters_sit_where_the_spec_says` pins the counts item 4 derives
  from them at 500 MiB. If a default changes, a class may hit a different limit first: its
  verdict becomes `wrong_error` or `wrong_exit`, and the case is updated with the change.
- Loops: the generators loop over counts derived from T (at most about 26 M xref entries,
  written 4096 at a time) and over fixed counts; none reads input.
- Time: about 12 s for all seven cases at 500 MiB on a Linux dev machine (writing 3.5 GB plus
  the runs); each run is bounded by its 20 s limit.

## Security
- Inputs: the two flags only. The files are generated, never read from the corpus, so the
  sealed corpus is never touched (CONSTITUTION 13). `DIR` is the caller's; the work file's path
  is absolute, so it never starts with `-` as an argument.
- Arithmetic: the sizes are `u64` from `--mib` ≤ 511 times 1 MiB (≤ 2^29), products of small
  constants; overflow panics in every profile (`overflow-checks = true`), and offsets use
  saturating adds.
- What broken implementation still passes?
  - A `jpdf-cli` that exits 2 for every file fails the exit-0 cases; one that prints one fixed
    error fails the cases expecting another (`run_with_a_fake_cli_writes_the_results`); one
    that exits 0 for everything fails the four exit-2 cases. The expected text must follow the
    `jpdf-cli: ` prefix (`verdicts_follow_the_spec`).
  - An exit-0 case checks the exit code only, not the page count: a `jpdf-cli` that exits 0
    without reading the file passes those three. The page counts are the engine tests' job;
    `the_valid_classes_have_their_page_counts` checks that the generated files have them.
  - A generator that stops building its attack (for example kids that no longer overlap) is
    caught at 64 MiB by `every_case_ends_as_expected_on_the_engine`, which runs the engine
    in-process on every case's file and needs every verdict to be `pass`; a generator that
    writes a small file fails the size check.
  - A run where nothing was checked: every case must produce a verdict, and a case that can't
    be generated is a tool failure, never a pass.
- What the tests can't see: the real peak memory and time at 500 MiB under the release build.
  The nightly run at the default size covers them (on Linux under the 2 GiB limit), and the
  harness records no peak RSS: measuring it is #236's.

## Accessibility
Not applicable (developer tool).

## Platforms
Linux applies the 2 GiB limit; macOS refuses `RLIMIT_AS`, so there the runs go without it
(item 2). The judge runs on Linux.

## Performance
About 12 s at 500 MiB (item "Limits"). Disk: one file of ≤ 512 MiB at a time.

## Tests
`xtask/src/limits/tests.rs`: `parses_flags_and_defaults`, `usage_errors`,
`every_case_ends_as_expected_on_the_engine`, `the_valid_classes_have_their_page_counts`,
`default_size_parameters_sit_where_the_spec_says`, `max_per_object_fits_at_every_size`,
`every_case_reaches_its_size_at_the_bounds`,
`the_size_check_is_exact_at_both_ends`, `verdicts_follow_the_spec`,
`each_file_is_fresh_and_removed_after_its_run`, `an_unwritable_file_is_a_tool_failure`,
`results_json_has_the_spec_shape`, `run_with_a_fake_cli_writes_the_results`. The nightly's exit
codes with the limits cases: `xtask/src/nightly/tests/driver.rs`
`exit_codes_with_the_limits_cases` (specs/tooling/nightly.md items 10 and 15).
