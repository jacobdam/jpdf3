# cargo xtask admit: a local pre-check of the dependency admission rules

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; #303, part 1 of #16)
Status: implemented

## Behavior
PLAN.md §3 and AUTOMATION.md §6.4 list the rules a third-party crate must pass before it is
added: an allowlisted license (CONSTITUTION 10), no `unsafe` anywhere in its dependency tree
outside the RustCrypto allowlist (CONSTITUTION 8), imported `cargo vet` audits only
(CONSTITUTION 9), no build scripts or proc-macros unless audited, and a pinned version. The
protected judge enforces them on every PR. `cargo xtask admit` answers the same question for a
candidate **before** anyone adds it, so #16 part 2 can record which PLAN §3 candidates are
admissible. It is a pre-check, not a gate: the judge stays the only authority.

```
cargo xtask admit <crate>@<version> --work DIR [--features A,B] [--no-default-features] [--python PATH]
```

Run it from the repository root (it reads `deny.toml`, `supply-chain/`, `rust-toolchain.toml`
and `ops/judge/` there, and writes nothing in the repo).

### Arguments
- `<crate>@<version>`: a crates.io name (an ASCII letter, then ASCII letters, digits, `-` or
  `_`; at most 64 bytes) and an exact version (`MAJOR.MINOR.PATCH`, decimal parts without
  leading zeros, then an optional `-pre` and an optional `+build`, each a non-empty run of
  ASCII letters, digits, `.` and `-`; at most 64 bytes). A range (`^1`, `=1.2.3`, `1.2`,
  `1.2.*`) is refused: rule 5 pins the version, and the probe asks for `=<version>`.
- `--work DIR` (required): where the probe package is created. It must be an absolute path that
  doesn't exist yet (not even as a dangling symlink) and doesn't end in `..`; its parent must
  be an existing directory; and no ancestor of the parent, after resolving symlinks, may be a
  git checkout (a directory holding a `.git` entry, which a worktree's `.git` file also is).
  Inside the repo the probe would be claimed by the repo's workspace, read its `.cargo/`, and
  be walked by the judge's own tree scans.
- `--features A,B`: features of the candidate to enable, comma-separated, at most 32, each 1-64
  ASCII letters, digits, `_`, `-`, `+` or `.` (so `dep:x` and `a/b` are refused, and no value
  can leave its TOML string). `--no-default-features` turns off the default features. Use the
  set the adapter would enable: optional dependencies outside it are not resolved and so not
  checked.
- `--python PATH` (default `python3`): the interpreter that runs the judge's scan. It needs
  Python 3.11 or later (`policy.py` imports `tomllib`); macOS's `/usr/bin/python3` is 3.9, so
  pass `/opt/homebrew/bin/python3.14` there (ops/LESSONS.md).
- Each flag at most once; anything else is a usage error.

### Steps
A multi-step tool (docs/PRINCIPLES.md P4): `drive` in `xtask/src/admit.rs` runs the steps in
this order, records each as a `steps::Outcome` under its name, and the exit code is
`steps::exit_code` of the log.

1. **`probe`**: creates `DIR` (`create_dir`, so an existing one fails), `DIR/src/lib.rs` (empty)
   and `DIR/Cargo.toml`:
   ```toml
   [package]
   name = "jpdf-admit-probe"
   version = "0.0.0"
   edition = "2024"
   publish = false

   [dependencies.<crate>]
   version = "=<version>"
   default-features = <true|false>
   features = [<features>]

   [workspace]
   ```
   and copies, byte for byte, `supply-chain/config.toml`, `supply-chain/audits.toml`,
   `supply-chain/imports.lock` (cargo vet's store) and `rust-toolchain.toml` (so rustup picks the
   pinned toolchain in `DIR`). Every file is created new (`create_new`). Then, in `DIR`,
   `cargo generate-lockfile --color never` and `cargo metadata --format-version 1 --locked
   --color never`, parsed with the strict JSON reader `jpdf_testgen::json`: every package needs
   a string `name` and `version` and a `targets` array whose items each have a `kind` array of
   strings; the probe must be listed exactly once and be the only package without a `source`; a
   non-null `source` must be a string, a `license` a string or null; at least one third-party
   package must be listed. A package with a `custom-build` or `proc-macro` target is
   **build-time**. Any failure (a write, a non-zero exit, an unknown shape) fails the step and
   **stops** the tool: every later step needs the package list.
2. **`deny`**: `cargo deny --color never --manifest-path DIR/Cargo.toml --config
   <repo>/deny.toml check`, all four checks with the repo's policy, as the judge's `cargo deny
   check` runs them. Exit 0 with the summary line `advisories ok, bans ok, licenses ok, sources
   ok` on stdout passes; a non-zero exit whose stdout has a summary line (exactly those four
   `<check> <ok|FAILED>` pairs in that order, joined by `, `) with at least one `FAILED` is a
   **rejection**, listing the failed checks and each stderr line that starts with `error[`,
   followed by its `┌─` location line when the next line is one. Anything else (no summary, a
   summary that disagrees with the exit code) fails the step. Each package whose license text
   names `OFL-1.1` adds the note `note: <name>@<version> is licensed <license>: OFL-1.1 is
   allowed for fonts only (CONSTITUTION 10), check it by hand`; the note changes no outcome.
3. **`unsafe`**: runs the judge's own scan, not a copy of it: `<python> -I -c <snippet>
   <repo>/ops/judge DIR`, where the snippet imports `ops/judge/policy.py`, calls
   `check_deps(Path(DIR), Report())` (which runs `cargo metadata --locked` in `DIR`, scans every
   registry package's `.rs` files for `unsafe` outside comments and strings, and applies
   `ops/judge/unsafe-allowlist.txt` with its repository prefixes) and prints the violation list
   as JSON on its last stdout line. `-I` keeps the working directory and `PYTHON*` variables
   out of the import path. An empty list passes; violations are a **rejection**, one line each.
   A non-zero exit, a last line that isn't a JSON list of strings, or a `cargo metadata failed`
   entry (the judge's scan couldn't run) fails the step.
4. **`vet`**: `cargo vet --manifest-path DIR/Cargo.toml --output-format json`, without
   `--locked`: the repo's `imports.lock` holds only the audits the repo's own crates need, so the
   imports are fetched again (into `DIR`'s copy only). Exit 0 with `"conclusion": "success"`
   passes. A non-zero exit with `"conclusion": "fail (vetting)"` and a non-empty `failures` list
   (each with a string `name` and `version`) is a **rejection**, one line per failure,
   `<name>@<version>: no imported audit for <missing criteria>`; another `fail (...)` conclusion
   (a violation) is a rejection too, printed as `conclusion: <text>`, but it doesn't say which
   crates are unvetted. Anything else (no JSON, vet's `error` object, a conclusion that
   disagrees with the exit code) fails the step, with vet's error message when it gave one.
5. **`build-time`**: AUTOMATION.md §6.4, "no build scripts or proc-macros in dependencies
   unless they have an imported audit". It lists each build-time package as `<name>@<version>:
   audited` or `not audited`, from vet's `failures` (matched by name and version). One not
   audited is a **rejection**. When vet passed, all are audited. When vet didn't say which crates
   are unvetted (it failed, or concluded with a violation), the step fails if there is any
   build-time package, listing them, and passes if there is none.

`deny`, `unsafe`, `vet` and `build-time` each run whatever the others found, so one run shows
every rule a candidate breaks. A command that can't start (`cargo deny` or `cargo vet` not
installed, a wrong `--python`) fails its own step only.

### Output
On stdout, one line per step that ran, `admit: <step>: ok`, `admit: <step>: rejected` or
`admit: <step>: failed: <why>`, each followed by its detail lines indented by two spaces (at
most 40, then `  … <n> more`), and a last line:
- `admit: <crate>@<version>: admitted` (exit 0),
- `admit: <crate>@<version>: rejected by <steps>` (exit 4; the rejecting steps in order),
- `admit: <crate>@<version>: not decided: a step failed` (exit 1).

A failure reason is also printed on stderr as `xtask: admit: <why>` when the step ends. Every
line taken from tool output or crate metadata goes through `clean`: control characters
(ANSI escapes, CR, LF) become `?` and the line is cut at 300 bytes with `…`, so a crate's
license text can't forge or hide a report line. Failure reasons quote one stderr line: the first
that starts with `error:` or `error[` (cargo's own message), else the last non-blank one (a
Python exception).

## Persistence
`DIR` is left in place for inspection (the lockfile, the fetched imports); delete it by hand.
Nothing in the repo changes.

## Undo
None needed: nothing in the repo or on GitHub changes.

## Errors
| Case | Exit |
|---|---|
| every step passes | 0 |
| a rule rejects the candidate and no step failed | 4 |
| a step failed (probe, network, a tool missing or printing an unknown shape) | 1 |
| a usage error, or `--work` breaks a rule above (exists, relative, inside a checkout) | 2 |

A failed step outranks a rejection (`steps::exit_code`: `Failed` 1 beats `Finding` 4), since
the verdict is then incomplete. A version that doesn't exist on crates.io, and a network
failure, both fail `probe` (exit 1): cargo's message says which.

## Limits
- Arguments: name ≤ 64 bytes, version ≤ 64 bytes, ≤ 32 features of ≤ 64 bytes each; tests at
  each limit and one past it.
- Output: ≤ 40 detail lines per step and ≤ 300 bytes per line; tests at each limit and one past.
- Tool output is read whole into memory (`Command::output`). It comes from cargo, cargo-deny,
  cargo-vet and the judge's script on registry crates, not from a document, and is a few
  hundred kilobytes at most for the graphs PLAN §3 names.
- No step has a timeout (macOS has no `timeout`, ops/LESSONS.md): a stalled download stalls the
  tool. It is run by hand in a session, which can stop it.

## Security
- **It never builds the probe.** `generate-lockfile`, `metadata`, `deny` and `vet` read
  manifests and sources; none compiles, so no build script or proc-macro of the candidate runs.
  The downloaded sources land in `~/.cargo/registry`, as for any `cargo fetch`.
- **It only reports** (#303, CONSTITUTION 9): it records no audits, exemptions or trust entries
  (vet runs its default `check`, which may rewrite files under `DIR/supply-chain/` only), writes no protected
  file, and runs the judge's scan without changing it (the snippet imports `policy.py`; the
  allowlist is read from `ops/judge/` as the judge reads it).
- **Writes stay in `DIR`**, which must be new and outside any checkout (Arguments). Each file
  is created with `create_new`, so a planted symlink at a file's path fails the write instead of
  being followed; the directories are created with `create_dir`, which fails on an existing
  entry.
- **What it can't see.** Cargo also reads `.cargo/config.toml` from `DIR`'s ancestors and
  `CARGO_HOME` (registry replacement, build settings). The tool doesn't check them: it is a
  session pre-check, and the judge, which runs on its own checkout, is the gate. A crate whose
  `unsafe` sits only in a `build.rs` or proc-macro of a dependency is scanned the same way the
  judge scans it (every `.rs` file of every registry package); the tool adds nothing the judge
  doesn't check, except the build-time audit match (step 5), which is otherwise vet's job.
- **Crate data is data** (CONSTITUTION 1): names, versions and license strings are compared and
  printed through `clean`, never executed or used as paths. Only the validated `<crate>`,
  `<version>` and features reach the manifest, and their character sets can't leave a TOML
  string or key.
- Self-check:
  - Arithmetic: only `saturating_add` on a counter and `saturating_sub` on a count; `clean`
    compares byte lengths before pushing.
  - What broken implementation still passes? A parser that accepted any deny output on exit 0
    fails `deny_without_a_matching_summary_fails` (exit 0 with `FAILED`, exit 4 with all ok, a
    reordered or three-check summary, a lowercase `failed`). A scan step that passed on a
    non-zero exit or a `cargo metadata failed` entry fails `judge_scan_results`. A vet parser
    that treated `fail (vetting)` without failures, or an exit-0 failure, as a pass fails
    `vet_results_it_cannot_read_fail`. A build-time step matching by name only fails
    `build_time_needs_an_audit` (same name, other version), and one that cleared crates when
    vet didn't decide fails its `unknown` case. A driver that stopped after the first rejection,
    or kept going after a failed probe, fails the driver tests.
  - Each rule has a should-reject and a near-miss should-accept case: names, versions and
    features (`exact_versions_only` holds `1.0.0-a-b+c-d` against `1.2.3-a_b`), the work
    directory (a checkout at the parent, above it, and behind a symlinked parent, against the
    same paths with no checkout), each tool's output shape.
  - A run where nothing was checked fails: metadata with no third-party package fails `probe`,
    and vet's `fail (vetting)` with an empty list fails `vet`.
  - Each parser has one case per line shape the real tools printed (cargo-deny 0.21.0's summary
    and `error[...]` blocks, cargo-vet 0.10.3's `success`, `fail (vetting)` and `error` objects,
    the judge scan's list, cargo 1.100's metadata), and `commands_are_pinned` pins the argument
    lists `drive` runs, which are built by the same functions.
- What the tests can't see: the real network tools. The PR records live runs (an admitted
  crate, a crate rejected by `unsafe`, `vet` and `build-time`, one rejected by `deny`'s license
  check, a missing version, a work directory inside the checkout).

## Accessibility
Not applicable (a command-line tool; plain text output, no colour).

## Platforms
macOS and Linux. `cargo-deny` and `cargo-vet` must be installed (CI installs both; a session
machine has them in `~/.cargo/bin` or Homebrew). On macOS pass `--python` (Arguments).

## Performance
Dominated by downloads: a first run fetches the crate graph, the advisory database and the
audit imports listed in `supply-chain/config.toml`. No budget is enforced.

## Tests
`xtask/src/admit/tests.rs` (`cargo nextest run -p xtask -E 'test(/admit/)'`):
- `parses_target_and_flags`, `rejects_missing_repeated_and_unknown_arguments`, `crate_names`,
  `exact_versions_only`, `feature_lists`: the argument rules and their length and count limits.
- `manifest_pins_the_one_dependency`, `write_probe_creates_new_files_only`: the probe files,
  copied byte for byte, and a second write into the same directory failing.
- `work_dir_rules`, `has_git_sees_dirs_and_worktree_files`: the `--work` rules, including this
  repository's own tree.
- `commands_are_pinned`: every command's program and arguments.
- `metadata_lists_registry_packages`, `metadata_shapes_it_does_not_know_are_errors`.
- `deny_pass_and_rejection`, `deny_without_a_matching_summary_fails`, `deny_notes_ofl_licenses`.
- `judge_scan_results`; `vet_pass_and_rejection`, `vet_results_it_cannot_read_fail`;
  `build_time_needs_an_audit`.
- `driver_runs_every_check_in_order`, `driver_keeps_going_after_a_rejection_or_failure`,
  `driver_stops_when_the_probe_fails`: the step order, each check's rejection and start failure
  with the others still running, and the probe stopping the tool, with the exit codes and the
  verdict line.
- `failure_text_prefers_the_first_error_line`, `clean_escapes_controls_and_caps_length`,
  `render_caps_details`: the output rules.
