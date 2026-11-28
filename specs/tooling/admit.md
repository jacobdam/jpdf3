# cargo xtask admit: a local pre-check of the dependency admission rules

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; #303 and #413, parts 1a and 1b of #16)
Status: implemented (#303: the probe, `deny` and the judge's scan; #413: `vet`, `build-time`,
the feature flags and the OFL-1.1 note)

## Behavior
PLAN.md §3 and AUTOMATION.md §6.4 list the rules a third-party crate must pass before it is
added: an allowlisted license (CONSTITUTION 10), no `unsafe` anywhere in its dependency tree
outside the RustCrypto allowlist (CONSTITUTION 8), imported `cargo vet` audits only
(CONSTITUTION 9), no build scripts or proc-macros unless audited, and a pinned version. The
protected judge enforces them on every PR. `cargo xtask admit` answers the same question for a
candidate **before** anyone adds it, so #304 can record which PLAN §3 candidates are
admissible. It is a pre-check, not a gate: the judge stays the only authority.

It runs every one of those rules: the license rule (`deny`, plus a note for OFL-1.1, which
CONSTITUTION 10 allows for fonts only), the `unsafe` rule (the judge's scan), the audit rule
(`vet`), the build-time rule (`build-time`) and the pin (the probe asks for `=<version>`).

```
cargo xtask admit <crate>@<version> --work DIR [--features A,B] [--no-default-features] [--python PATH]
```

Run it from the repository root: it reads `deny.toml`, `rust-toolchain.toml` and `ops/judge/`
there, and writes nothing in the repo.

### Arguments
- `<crate>@<version>`: a crates.io name (an ASCII letter, then ASCII letters, digits, `-` or
  `_`; at most 64 bytes) and an exact version (`MAJOR.MINOR.PATCH`, decimal parts without
  leading zeros, then an optional `-pre` and an optional `+build`, each a non-empty run of
  ASCII letters, digits, `.` and `-`; at most 64 bytes). A range (`^1.2.3`, `=1.2.3`, `1.2`,
  `1.2.*`) is refused: rule 5 pins the version, and the probe asks for `=<version>`. Neither
  character set can leave the manifest's TOML key or string. This is a shape check, not full
  semver: `1.0.0-a..b`, `1.0.0-01` and parts too large for 64 bits pass it, and cargo then
  rejects them in `probe` (exit 1, not 2).
- `--work DIR` (required): where the probe package is created. It must be an absolute path that
  doesn't exist yet (not even as a dangling symlink) and doesn't end in `..`; its parent must
  be an existing directory; and no ancestor of the parent, after resolving symlinks, may be a
  git checkout (a directory holding a `.git` entry; a worktree's `.git` is a file). Inside the
  repo the probe would be claimed by the repo's workspace, read its `.cargo/`, and be walked by
  the judge's own tree scans.
- `--features A,B` (default: none): the features the probe enables on the candidate, as the
  adapter in `jpdf-backends` would. 1 to 32 comma-separated names, each 1 to 64 ASCII letters,
  digits, `_`, `-`, `+` or `.`; an empty name (`a,,b`, a trailing comma), `dep:x`, `a/b`, a
  quote, a backslash or a space is a usage error, so no value can leave its TOML string.
  Whether the crate has the feature is cargo's call: an unknown one fails `probe` (exit 1).
- `--no-default-features`: the probe sets `default-features = false` on the candidate.
- `--python PATH` (default `python3`): the interpreter for the judge's scan. It needs Python
  3.11 or later (`policy.py` imports `tomllib`); macOS's `/usr/bin/python3` is 3.9, so pass
  `/opt/homebrew/bin/python3.14` there (ops/LESSONS.md).
- Each flag at most once, every value present and `--python` non-empty; anything else
  (`--all-features`, say) is a usage error.

### Steps
A multi-step tool (docs/PRINCIPLES.md P4): `drive` in `xtask/src/admit.rs` runs the steps in
this order, records each as a `steps::Outcome` under its name, and the exit code is
`steps::exit_code` of the log.

1. **`probe`**: creates `DIR`, `DIR/src` and `DIR/supply-chain` (`create_dir`, so an existing
   entry fails), an empty `DIR/src/lib.rs`, byte-for-byte copies of `rust-toolchain.toml` (so
   rustup picks the pinned toolchain in `DIR`) and of cargo vet's store,
   `supply-chain/config.toml`, `audits.toml` and `imports.lock` (so `vet` uses the repo's
   imports), and `DIR/Cargo.toml`:
   ```toml
   [package]
   name = "jpdf-admit-probe"
   version = "0.0.0"
   edition = "2024"
   publish = false

   [dependencies.<crate>]
   version = "=<version>"
   default-features = <true, or false with --no-default-features>
   features = [<each --features name, quoted, joined by ", ">]

   [workspace]
   ```
   Every file is created new (`create_new`). Then `cargo generate-lockfile --color never` in
   `DIR` resolves the candidate's graph with those features, and `cargo metadata
   --format-version 1 --locked --color never` reads it: every package but the probe is a
   third-party package, with its name, version, `license` expression and whether it has a
   `custom-build` target (a build script) or a `proc-macro` one. The probe must be listed
   exactly once and be the only package without a `source`, every package needs a name, a
   version and a `targets` list whose `kind`s are strings, and the graph must have at least one
   third-party package; any other shape is an error. A failed write or read, a command that
   can't start, a non-zero exit or an unknown metadata shape fails the step and **stops** the
   tool: every check needs the resolved probe.
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
   `check_deps(Path(DIR), Report())`, which runs `cargo metadata --locked` in `DIR`, scans the
   `.rs` files of every package with a `source` (registry and git; it skips the directories in
   `policy.py`'s `SKIP_DIRS`, such as `target` and `build`) for `unsafe` outside comments and
   strings, and applies
   `ops/judge/unsafe-allowlist.txt` with its repository prefixes; then it prints the violation
   list as JSON on its last stdout line. `-I` keeps the working directory and `PYTHON*`
   variables out of the import path. An empty list passes; violations are a **rejection**, one
   detail line each. A non-zero exit, a last line that isn't a JSON list of strings, or a
   `cargo metadata failed` entry (the judge's scan couldn't run) fails the step; a quoted line
   that names `tomllib` adds the hint to pass a Python 3.11+ `--python`.

4. **`vet`**: `cargo vet --manifest-path DIR/Cargo.toml --output-format json`, in `DIR`, with
   the probe's copy of the store. It runs without `--locked`, unlike the judge's `cargo vet
   --locked`: `cargo vet` prunes the repo's `imports.lock` to the audits the repo's own graph
   needs, so the candidate's audits are fetched from the imported sources in `config.toml`
   (into the copy only). Its JSON report on stdout decides:
   - exit 0 and `"conclusion": "success"` passes;
   - a non-zero exit, `"conclusion": "fail (vetting)"` and a non-empty `failures` list, each
     with a string `name` and `version`, is a **rejection**: one detail line per failure,
     `<name>@<version>: no imported audit for <missing_criteria, joined by ", ">`;
   - a non-zero exit with another `fail (...)` conclusion (a violation) is a **rejection** with
     the detail `conclusion: <it>`, which doesn't say what is unvetted;
   - anything else fails the step: no JSON (the reason quotes stderr), a conclusion that
     disagrees with the exit code, an empty or missing `failures` list, a failure without a
     name or version, an unknown conclusion. vet's own errors are JSON too
     (`{"error": {"message": ...}}`): the reason quotes the message.
5. **`build-time`** (AUTOMATION.md §6.4: no build scripts or proc-macros in dependencies unless
   they have an imported audit): no command; it reads `probe`'s package list and `vet`'s
   result. With no build-time package it passes with no details. Otherwise it lists each one,
   `<name>@<version>: audited` or `not audited`; a package is audited when vet passed or its
   exact name and version are not among vet's failures, since the judge allows no exemptions
   and no own audits in the store (`ops/judge/policy.py`), so vetted means vetted by an
   imported audit. One not audited is a **rejection**. If vet didn't say what is unvetted (it
   failed, couldn't start, or rejected for a violation), the step **fails**, listing the
   build-time packages it couldn't clear. A candidate with an unvetted build-time package is
   rejected by both `vet` and `build-time`; the second names which unvetted packages run code
   at build time.

`deny`, `unsafe` and `vet` each run whatever the others found, so one run shows every rule a
candidate breaks. A command that can't start (`cargo deny` or `cargo vet` not installed, a
wrong `--python`) fails its own step only (and, for `vet`, `build-time` when the graph has a
build-time package).

`deny` also adds, whatever its result, one note per package whose `license` expression contains
`OFL-1.1` in any ASCII case: `note: <name>@<version> is licensed <expression>: OFL-1.1 is allowed for fonts only
(CONSTITUTION 10); check it by hand`. `deny.toml` allows OFL-1.1 for every crate, since it
can't tell a font from code, so a note never changes the outcome. The note is best effort: a
crate with only a `license-file` (no `license` field) that cargo-deny identifies as OFL-1.1
from the file's text gets none.

### Output
On stdout, one line per step that ran, `admit: <step>: ok`, `admit: <step>: rejected` or
`admit: <step>: failed: <why>`, each followed by its detail lines indented by two spaces (at
most 40, then `  … <n> more`), and a last line:
- `admit: <crate>@<version>: admitted` (exit 0),
- `admit: <crate>@<version>: rejected by <steps>` (exit 4; the rejecting steps in order),
- `admit: <crate>@<version>: not decided: a step failed` (exit 1).

A failure reason is also printed on stderr as `xtask: admit: <why>` when its step ends. Every
line taken from tool output goes through `clean`: control characters (ANSI escapes, CR, LF),
U+2028, U+2029 and the bidi embeddings, overrides, isolates and marks (U+202A-U+202E,
U+2066-U+2069, U+200E, U+200F, U+061C) become `?`, and the line is cut at 300 bytes with
`…`, so crate data can't start a new report line or reorder one in a terminal or a pasted
copy. A failure reason quotes one stderr line: the first that starts with `error:` or
`error[` (cargo's own message), else the last non-blank one (a Python exception).

## Persistence
`DIR` is left in place for inspection (the lockfile, and the store copy as `cargo vet` left
it); delete it by hand. Nothing in the repo
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
- Arguments: name ≤ 64 bytes, version ≤ 64 bytes, `--features` ≤ 32 names of ≤ 64 bytes each;
  tests at each limit and one past it.
- Output: ≤ 40 detail lines per step and ≤ 300 bytes per line; tests at each limit and one past.
- Tool output is read whole into memory (`Command::output`). It comes from cargo, cargo-deny,
  cargo-vet (its JSON report, crate-derived `suggest` data included) and the judge's script run
  on registry crates, not from a document.
- No step has a timeout (macOS has no `timeout`, ops/LESSONS.md): a stalled download stalls the
  tool. It is run by hand in a session, which can stop it.

## Security
- **It never builds the probe.** `generate-lockfile`, `cargo metadata`, `cargo deny`, the
  judge's `cargo metadata` and `cargo vet` read manifests and sources; none compiles, so no
  build script or proc-macro of the candidate runs. Downloaded sources land in
  `~/.cargo/registry`, as for any fetch; `cargo vet` also fetches the imported audit files named
  in `supply-chain/config.toml` and, when it rejects, crate sources for its suggestions (into its
  own cache).
- **It only reports** (#303, CONSTITUTION 9): it records no audits or exemptions, writes no
  protected file (the store it hands `cargo vet` is the probe's copy in `DIR`, never the repo's
  `supply-chain/`), and runs the judge's scan unchanged (the snippet imports `policy.py`, which
  reads its allowlist from `ops/judge/` as the judge does).
- **Writes stay in `DIR`**, which must be new and outside any checkout (Arguments). Each file
  is created with `create_new`, so a planted symlink at a file's path fails the write instead of
  being followed.
- **What it can't see.** Cargo also reads `.cargo/config.toml` from `DIR`'s ancestors and
  `CARGO_HOME` (a registry replacement, say). The tool doesn't check them: it is a session
  pre-check, and the judge, on its own checkout, is the gate. It adds no check the judge doesn't
  run (`build-time` reads vet's own verdict); a pass here can still fail the judge if the
  adapter enables other features than `--features` named, adds other crates, or lands in a
  different resolution: the probe resolves the candidate's
  dependencies fresh, to the newest compatible versions, while the judge reads the repo's
  `Cargo.lock`, where a shared dependency unifies with the version already pinned there (the
  probe's `foo 1.9` may pass where the repo's `foo 1.2` has `unsafe`). Re-run the judge's
  checks on the PR that adds the crate; this tool doesn't replace them. Above all, `vet` here
  fetches the imported audits live, while the judge runs `cargo vet --locked` against the
  repo's `supply-chain/imports.lock`, which holds only the audits the repo's current graph
  needs (none today). So a candidate this tool admits still fails the judge's vet step on the
  PR that adds it until `imports.lock` carries its audits, and that file is protected: the
  update needs the owner's signed commit (CONSTITUTION 12).
- **Crate data is data** (CONSTITUTION 1): tool output and crate metadata (names, versions,
  license expressions) are matched and printed through `clean`, never executed or used as a
  path. Only the validated `<crate>`, `<version>` and feature names reach the manifest.
- Self-check:
  - Arithmetic: `saturating_add` and `saturating_sub` on lengths and counts only.
  - What broken implementation still passes? A deny parser that accepted any output on exit 0,
    or any exit with a `FAILED` somewhere, fails `deny_without_a_matching_summary_fails` (exit 0
    with `FAILED`, exit 4 with all ok, reordered, three or five pairs, a lowercase `failed`,
    the malformed lines each holding a real `FAILED`, so a lenient parser would reject instead
    of failing). A scan step that passed on a non-zero exit or a `cargo metadata failed` entry
    fails `judge_scan_results`. A driver that stopped after the first rejection, kept going
    after a failed probe, or let a rejection outrank a failure fails the driver tests. A vet
    parser that passed on any exit with `success`, or rejected on exit 0, fails
    `vet_results_it_cannot_read_fail`; a build-time check that counted every package, matched
    by name only, or cleared crates when vet didn't decide fails `build_time_needs_an_audit`;
    a metadata reader that missed `proc-macro` or `custom-build` kinds fails the driver's and
    `metadata_lists_third_party_packages`' expected lists; an OFL match on `OFL` alone fails on
    the `OFL-1.0` near miss. Hand mutants of each of these, listed in the PR descriptions, were
    all caught.
  - Each rule has a should-reject and a near-miss should-accept case: names, versions
    (`1.0.0-a-b+c-d` against `1.2.3-a_b`), the work directory (a checkout at the parent, above
    it, and behind a symlinked parent, against the same paths with no checkout), each tool's
    output shape.
  - A run where nothing was checked fails: a failed probe is exit 1, never 0, and a check that
    can't start is `failed`, not `ok`.
  - Each parser has one case per line shape the real tools printed (cargo-deny 0.21.0's summary
    and `error[...]` blocks, the judge scan's list, cargo-vet 0.10.3's success, `fail
    (vetting)` and error documents, cargo's metadata and its and Python's error lines), and
    `commands_are_pinned` pins the argument lists, built by the same functions `drive` calls.
- What the tests can't see: the real network tools. The PRs record live runs (#303: an
  admitted crate, one rejected by `unsafe`, one rejected by `deny`'s license check, a missing
  version, a work directory inside the checkout; #413: an admitted crate through all five
  steps, a proc-macro rejected by `unsafe`, `vet` and `build-time`, a feature set, an unknown
  feature, a feature that would leave its string).

## Accessibility
Not applicable (a command-line tool; plain text, no colour).

## Platforms
macOS and Linux. `cargo-deny` and `cargo-vet` must be installed (CI installs both; session
machines have them). On macOS pass `--python` (Arguments).

## Performance
Dominated by downloads: the crate graph, cargo-deny's advisory database and cargo vet's
imported audit files. No budget is enforced.

## Tests
`xtask/src/admit/tests.rs` (`cargo nextest run -p xtask -E 'test(/admit/)'`):
- `parses_target_and_flags`, `rejects_missing_repeated_and_unknown_arguments`, `crate_names`,
  `exact_versions_only`, `feature_lists`: the argument rules and their limits.
- `manifest_pins_the_one_dependency`, `write_probe_creates_new_files_only`: the probe files
  (the feature settings included), the toolchain and the three store files copied byte for
  byte, a missing one failing, and a second write into the same directory failing.
- `work_dir_rules`, `has_git_sees_dirs_and_worktree_files`: the `--work` rules, including this
  repository's own tree.
- `commands_are_pinned`: every command's program and arguments.
- `metadata_lists_third_party_packages`, `metadata_shapes_it_does_not_know_are_errors`.
- `deny_pass_and_rejection`, `deny_without_a_matching_summary_fails`,
  `deny_notes_ofl_licenses_whatever_the_result` (an `OFL-1.0` near miss gets no note),
  `judge_scan_results`.
- `vet_pass_and_rejection`, `vet_results_it_cannot_read_fail`, `build_time_needs_an_audit`
  (the same name at another version and a non-build-time unvetted crate as near misses).
- `driver_runs_every_check_in_order`, `driver_keeps_going_after_a_rejection_or_failure`,
  `driver_stops_when_the_probe_fails`: the step order, each check's rejection and start failure
  with the others still running (`vet` failing also failing `build-time`), and a failed probe
  (the write, the lockfile, the metadata read or its shape) stopping the tool, with exit codes
  and the verdict line.
- `failure_text_prefers_the_first_error_line`, `clean_escapes_controls_and_caps_length`,
  `render_caps_details`: the output rules.
