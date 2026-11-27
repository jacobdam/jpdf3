# cargo xtask admit: a local pre-check of the dependency admission rules

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; #303, part 1 of #16)
Status: implemented (part 1: #303; the `cargo vet` and build-time steps are #413)

## Behavior
PLAN.md §3 and AUTOMATION.md §6.4 list the rules a third-party crate must pass before it is
added: an allowlisted license (CONSTITUTION 10), no `unsafe` anywhere in its dependency tree
outside the RustCrypto allowlist (CONSTITUTION 8), imported `cargo vet` audits only
(CONSTITUTION 9), no build scripts or proc-macros unless audited, and a pinned version. The
protected judge enforces them on every PR. `cargo xtask admit` answers the same question for a
candidate **before** anyone adds it, so #304 can record which PLAN §3 candidates are
admissible. It is a pre-check, not a gate: the judge stays the only authority.

This part runs the license and `unsafe` rules and pins the version. The `cargo vet` rule and
the build-time rule (each build script or proc-macro audited), the `--features` and
`--no-default-features` flags and a note for OFL-1.1 licenses (fonts only) are #413; until
then an `admitted` verdict says it covers only the checks run.

```
cargo xtask admit <crate>@<version> --work DIR [--python PATH]
```

Run it from the repository root: it reads `deny.toml`, `rust-toolchain.toml` and `ops/judge/`
there, and writes nothing in the repo.

### Arguments
- `<crate>@<version>`: a crates.io name (an ASCII letter, then ASCII letters, digits, `-` or
  `_`; at most 64 bytes) and an exact version (`MAJOR.MINOR.PATCH`, decimal parts without
  leading zeros, then an optional `-pre` and an optional `+build`, each a non-empty run of
  ASCII letters, digits, `.` and `-`; at most 64 bytes). A range (`^1.2.3`, `=1.2.3`, `1.2`,
  `1.2.*`) is refused: rule 5 pins the version, and the probe asks for `=<version>`. Neither
  character set can leave the manifest's TOML key or string.
- `--work DIR` (required): where the probe package is created. It must be an absolute path that
  doesn't exist yet (not even as a dangling symlink) and doesn't end in `..`; its parent must
  be an existing directory; and no ancestor of the parent, after resolving symlinks, may be a
  git checkout (a directory holding a `.git` entry; a worktree's `.git` is a file). Inside the
  repo the probe would be claimed by the repo's workspace, read its `.cargo/`, and be walked by
  the judge's own tree scans.
- `--python PATH` (default `python3`): the interpreter for the judge's scan. It needs Python
  3.11 or later (`policy.py` imports `tomllib`); macOS's `/usr/bin/python3` is 3.9, so pass
  `/opt/homebrew/bin/python3.14` there (ops/LESSONS.md).
- Each flag at most once, every value present and `--python` non-empty; anything else is a
  usage error.

### Steps
A multi-step tool (docs/PRINCIPLES.md P4): `drive` in `xtask/src/admit.rs` runs the steps in
this order, records each as a `steps::Outcome` under its name, and the exit code is
`steps::exit_code` of the log.

1. **`probe`**: creates `DIR` and `DIR/src` (`create_dir`, so an existing entry fails), an empty
   `DIR/src/lib.rs`, a byte-for-byte copy of `rust-toolchain.toml` (so rustup picks the pinned
   toolchain in `DIR`) and `DIR/Cargo.toml`:
   ```toml
   [package]
   name = "jpdf-admit-probe"
   version = "0.0.0"
   edition = "2024"
   publish = false

   [dependencies]
   <crate> = "=<version>"

   [workspace]
   ```
   Every file is created new (`create_new`). Then `cargo generate-lockfile --color never` in
   `DIR` resolves the candidate's graph with its default features. A failed write, a command
   that can't start or a non-zero exit fails the step and **stops** the tool: both checks need
   the resolved probe.
2. **`deny`**: `cargo deny --color never --manifest-path DIR/Cargo.toml --config
   <repo>/deny.toml check`, all four checks with the repo's policy, as the judge's `cargo deny
   check` runs them. Exit 0 with the summary line `advisories ok, bans ok, licenses ok, sources
   ok` on stdout passes. A non-zero exit whose stdout has a summary line (exactly those four
   `<check> <ok|FAILED>` pairs in that order, joined by `, `) with at least one `FAILED` is a
   **rejection**: its details are `failed: <checks>` and each stderr line that starts with
   `error[`, followed by its `┌─` location line (which names the crate's `Cargo.toml`) when the
   next line is one. Anything else (no summary line, a summary that disagrees with the exit
   code) fails the step.
3. **`unsafe`**: runs the judge's own scan, not a copy of it: `<python> -I -c <snippet>
   <repo>/ops/judge DIR`. The snippet imports `ops/judge/policy.py` and calls
   `check_deps(Path(DIR), Report())`, which runs `cargo metadata --locked` in `DIR`, scans every
   registry package's `.rs` files for `unsafe` outside comments and strings, and applies
   `ops/judge/unsafe-allowlist.txt` with its repository prefixes; then it prints the violation
   list as JSON on its last stdout line. `-I` keeps the working directory and `PYTHON*`
   variables out of the import path. An empty list passes; violations are a **rejection**, one
   detail line each. A non-zero exit, a last line that isn't a JSON list of strings, or a
   `cargo metadata failed` entry (the judge's scan couldn't run) fails the step.

`deny` and `unsafe` each run whatever the other found, so one run shows every rule a candidate
breaks. A command that can't start (`cargo deny` not installed, a wrong `--python`) fails its
own step only.

### Output
On stdout, one line per step that ran, `admit: <step>: ok`, `admit: <step>: rejected` or
`admit: <step>: failed: <why>`, each followed by its detail lines indented by two spaces (at
most 40, then `  … <n> more`), and a last line:
- `admit: <crate>@<version>: admitted by the checks run (cargo vet is #413)` (exit 0),
- `admit: <crate>@<version>: rejected by <steps>` (exit 4; the rejecting steps in order),
- `admit: <crate>@<version>: not decided: a step failed` (exit 1).

A failure reason is also printed on stderr as `xtask: admit: <why>` when its step ends. Every
line taken from tool output goes through `clean`: control characters (ANSI escapes, CR, LF)
become `?` and the line is cut at 300 bytes with `…`, so crate data can't forge or hide a
report line. A failure reason quotes one stderr line: the first that starts with `error:` or
`error[` (cargo's own message), else the last non-blank one (a Python exception).

## Persistence
`DIR` is left in place for inspection (the lockfile); delete it by hand. Nothing in the repo
changes.

## Undo
None needed: nothing in the repo or on GitHub changes.

## Errors
| Case | Exit |
|---|---|
| every step passes | 0 |
| a rule rejects the candidate and no step failed | 4 |
| a step failed (the probe, the network, a tool missing or printing an unknown shape) | 1 |
| a usage error, or `--work` breaks a rule above (exists, relative, inside a checkout) | 2 |

A failed step outranks a rejection (`steps::exit_code`: `Failed` 1 beats `Finding` 4), since
the verdict is then incomplete. A version that doesn't exist on crates.io and a network failure
both fail `probe` (exit 1); cargo's quoted message says which.

## Limits
- Arguments: name ≤ 64 bytes, version ≤ 64 bytes; tests at each limit and one past it.
- Output: ≤ 40 detail lines per step and ≤ 300 bytes per line; tests at each limit and one past.
- Tool output is read whole into memory (`Command::output`). It comes from cargo, cargo-deny and
  the judge's script run on registry crates, not from a document.
- No step has a timeout (macOS has no `timeout`, ops/LESSONS.md): a stalled download stalls the
  tool. It is run by hand in a session, which can stop it.

## Security
- **It never builds the probe.** `generate-lockfile`, `cargo deny` and the judge's
  `cargo metadata` read manifests and sources; none compiles, so no build script or proc-macro
  of the candidate runs. Downloaded sources land in `~/.cargo/registry`, as for any fetch.
- **It only reports** (#303, CONSTITUTION 9): it records no audits or exemptions, writes no
  protected file, and runs the judge's scan unchanged (the snippet imports `policy.py`, which
  reads its allowlist from `ops/judge/` as the judge does).
- **Writes stay in `DIR`**, which must be new and outside any checkout (Arguments). Each file
  is created with `create_new`, so a planted symlink at a file's path fails the write instead of
  being followed.
- **What it can't see.** Cargo also reads `.cargo/config.toml` from `DIR`'s ancestors and
  `CARGO_HOME` (a registry replacement, say). The tool doesn't check them: it is a session
  pre-check, and the judge, on its own checkout, is the gate. It adds no check the judge doesn't
  run; a pass here can still fail the judge if the adapter enables other features (#413) or
  adds other crates.
- **Crate data is data** (CONSTITUTION 1): tool output is matched and printed through `clean`,
  never executed or used as a path. Only the validated `<crate>` and `<version>` reach the
  manifest.
- Self-check:
  - Arithmetic: `saturating_add` and `saturating_sub` on lengths and counts only.
  - What broken implementation still passes? A deny parser that accepted any output on exit 0,
    or any exit with a `FAILED` somewhere, fails `deny_without_a_matching_summary_fails` (exit 0
    with `FAILED`, exit 4 with all ok, reordered, three or five pairs, a lowercase `failed`,
    the malformed lines each holding a real `FAILED`, so a lenient parser would reject instead of failing). A scan
    step that passed on a non-zero exit or a `cargo metadata failed` entry fails
    `judge_scan_results`. A driver that stopped after the first rejection, kept going after a
    failed probe, or let a rejection outrank a failure fails the driver tests. Hand mutants of
    each of these (17, listed in the PR) were all caught.
  - Each rule has a should-reject and a near-miss should-accept case: names, versions
    (`1.0.0-a-b+c-d` against `1.2.3-a_b`), the work directory (a checkout at the parent, above
    it, and behind a symlinked parent, against the same paths with no checkout), each tool's
    output shape.
  - A run where nothing was checked fails: a failed probe is exit 1, never 0, and a check that
    can't start is `failed`, not `ok`.
  - Each parser has one case per line shape the real tools printed (cargo-deny 0.21.0's summary
    and `error[...]` blocks, the judge scan's list, cargo's and Python's error lines), and
    `commands_are_pinned` pins the argument lists, built by the same functions `drive` calls.
- What the tests can't see: the real network tools. The PR records live runs (an admitted
  crate, one rejected by `unsafe`, one rejected by `deny`'s license check, a missing version, a
  work directory inside the checkout).

## Accessibility
Not applicable (a command-line tool; plain text, no colour).

## Platforms
macOS and Linux. `cargo-deny` must be installed (CI installs it; session machines have it). On
macOS pass `--python` (Arguments).

## Performance
Dominated by downloads: the crate graph and cargo-deny's advisory database. No budget is
enforced.

## Tests
`xtask/src/admit/tests.rs` (`cargo nextest run -p xtask -E 'test(/admit/)'`):
- `parses_target_and_flags`, `rejects_missing_repeated_and_unknown_arguments`, `crate_names`,
  `exact_versions_only`: the argument rules and their length limits.
- `manifest_pins_the_one_dependency`, `write_probe_creates_new_files_only`: the probe files,
  the toolchain copied byte for byte, and a second write into the same directory failing.
- `work_dir_rules`, `has_git_sees_dirs_and_worktree_files`: the `--work` rules, including this
  repository's own tree.
- `commands_are_pinned`: every command's program and arguments.
- `deny_pass_and_rejection`, `deny_without_a_matching_summary_fails`, `judge_scan_results`.
- `driver_runs_every_check_in_order`, `driver_keeps_going_after_a_rejection_or_failure`,
  `driver_stops_when_the_probe_fails`: the step order, each check's rejection and start failure
  with the other still running, and a failed probe stopping the tool, with exit codes and the
  verdict line.
- `failure_text_prefers_the_first_error_line`, `clean_escapes_controls_and_caps_length`,
  `render_caps_details`: the output rules.
