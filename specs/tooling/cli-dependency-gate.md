# jpdf-cli dependency gate: the #146 phase gate on the resolved graph

Area: tooling
ISO 32000-2 clauses: none (build structure)
Journeys: none
Status: implemented

## Behavior
specs/core/memory-budget.md ("Phases run one at a time", **Gate**) says no decode or raster phase
runs on the jpdf-cli path until #146 is closed, by a #25 measurement or by a structural fix.
`tools/jpdf-cli/tests/dependency_gate.rs` (#154) enforces that on the package graph Cargo resolves,
never on Rust sources or manifests read as text (PR #151: a text scan can always be bypassed; owner
instruction, step 2). A second check (#235) reads where each allowed crate's targets compile from,
as `cargo metadata` reports it, since `cargo tree` doesn't show that.

- **Allowed set** (`ALLOWED`): `jpdf-cli` (`tools/jpdf-cli`), `jpdf-engine`, `jpdf-cos`,
  `jpdf-syntax`, `jpdf-traits` (under `crates/`) and `jpdf-fsio` (`tools/fsio`). Each entry is a
  package name and its workspace directory.
- **Command:** `cargo tree --manifest-path <workspace>/Cargo.toml -p <root>… -e normal,build
  --all-features --target all --prefix none --no-dedupe --format {p} --charset ascii --color never
  --locked --offline`, with `$CARGO` (falling back to `cargo` on `PATH`). A non-zero exit is an
  error, so a stale lockfile or a missing package fails the gate.
- **Parse:** each non-blank line must be `<name> v<version>`, optionally followed by ` (<source>)`.
  The name has no whitespace; the version starts with a digit and holds only ASCII letters, digits,
  `.`, `-` and `+`; the source is everything between that ` (` and the line's final `)`, so a
  checkout path with parentheses in it (`/work/jpdf (copy)/…`, `/work/jpdf (old) (copy)/…`) parses.
  Anything else (no version, an empty or malformed one, a name with a space, a source without its
  parentheses, a Cargo marker) is an error, and so is an output with no packages: a format change
  fails the gate instead of passing it. Markers are recognized at their token boundaries
  (`has_marker`): the text inside the outer parentheses starting with `proc-macro) (` (Cargo prints
  `(proc-macro)` before the source) or ending with `) (*` or `) (proc-macro`, or exactly
  `proc-macro` or `*` (a registry package's marker with no source, S4.2). Such a line is never
  resolved as a path (round 2, S2.2: a directory named `jpdf-syntax) (*` that is a symlink to
  `crates/jpdf-syntax` would otherwise pass). A clean graph never matches: an allowed source is
  absolute (so it doesn't start with `proc-macro`) and ends in its crate directory's name, whatever
  the checkout path holds.
- **Check:** every package must have an allowed name and a source that is an **absolute** path
  which, canonicalized, equals its allowed directory, canonicalized. A registry package (no source),
  a git package, a relative source (never resolved against the working directory; a git URL is a
  relative path on Unix), an allowed name at another crate's path or at a path that doesn't exist,
  and any other name are each one violation line. A `..` detour that resolves to the right directory
  passes.
- **Target paths** (#235): `cargo metadata --manifest-path <workspace>/Cargo.toml --format-version 1
  --no-deps --locked --offline` (same `$CARGO`; a non-zero exit is an error), parsed with the strict
  JSON reader `jpdf_testgen::json` (moved there from xtask, which uses it too; the workspace takes no
  JSON crate, docs/adr/0002). `--no-deps` lists workspace members only, which is where every allowed
  crate must be; without `--filter-platform` every target is listed. For each allowed crate:
  - it must be listed exactly once with its name and a `manifest_path` that is absolute and whose
    directory, canonicalized, equals its allowed directory, canonicalized; otherwise one violation;
  - it must have at least one target, and every target (any `kind`: lib, bin, custom-build, test,
    bench, example, proc-macro) must have a `src_path` that is absolute and, canonicalized, lies
    inside that directory, compared by path components (`crates/pre-x/…` is not inside
    `crates/pre`). A relative `src_path` is never resolved against the working directory. A
    manifest `[lib] path`, `[[bin]] path` or `build = …` into another crate, or a target root that
    is a symlink out of the directory, is one violation per target. A `..` detour back into the
    directory passes;
  - a metadata shape the check doesn't know (no `packages` array, a package without a string
    `name`, `manifest_path` or a `targets` array, a target without a string `name`, a `kind` array
    of strings or a string `src_path`) is an error, so a format change fails the gate.
- **Tests** (all in `dependency_gate.rs`):
  - `cli_graph_is_parse_only`: roots are all six allowed crates, so `--all-features` turns on every
    feature of each. A feature of `jpdf-engine` that pulls in a decoder is caught even if the
    judge's `-p jpdf-cli` build would not enable it (another workspace member might, and Cargo
    unifies features in a workspace build).
  - `cli_graph_is_exactly_the_allowed_set`: from `jpdf-cli` alone the name set equals `ALLOWED`.
    This proves the listing is transitive (`jpdf-traits` is three edges below) and keeps the list
    from holding crates the CLI no longer uses: removing a dependency means removing its entry.
  - `control_a_real_graph_outside_the_list_is_reported`: the same command on `xtask` reports
    `xtask`, `jpdf-arlington` and `jpdf-testgen`, and not the allowed `jpdf-fsio`.
  - `tree_args_are_exactly_these`: every graph test runs the command `tree_command` builds; this
    test reads that command's arguments back (`Command::get_args`) and compares them as one string,
    so no flag can be dropped or narrowed (`--target all` to a set of triples, R3.1, S4.1) at the
    call site either.
  - `control_cargo_tree_failure_is_an_error`: an unknown `-p` package is `cargo tree failed`.
  - `control_flags_reach_features_other_targets_and_build_edges`: the same command on a fixture
    workspace written under `CARGO_TARGET_TMPDIR` (lockfile from `cargo generate-lockfile
    --offline`) lists, in one run, a feature-gated optional dependency of a second root, a
    `cfg(windows)` and a `cfg(target_os = "linux")` dependency (no single target or host has both,
    so `--target <one triple>` fails it too) and a build-dependency; from the first root alone the
    optional one is not listed (why the gate takes every allowed crate as a root); after an edit to
    the fixture's manifest that its lockfile doesn't record, `cargo tree failed` (`--locked`). The
    fixture directory is removed after a pass.
  - `cli_crates_compile_only_their_own_sources`: the target-path check on the workspace, for all
    six allowed crates.
  - `metadata_args_are_exactly_these`: reads the `cargo metadata` command's arguments back, as
    `tree_args_are_exactly_these` does.
  - `control_cargo_metadata_failure_is_an_error`: a manifest path that doesn't exist is
    `cargo metadata failed`.
  - `control_target_paths_outside_the_crate_are_reported`: real `cargo metadata` output for a
    fixture workspace under `CARGO_TARGET_TMPDIR` whose members point a `[lib] path`, a `[[bin]]
    path`, a `build = …`, a proc-macro `[lib] path` and a `crate-type = ["cdylib"]` `[lib] path` at
    a `victim` crate's files (kinds `lib`, `bin`, `custom-build`, `proc-macro`, `cdylib`), a `[lib] path` at the sibling `pre-x`
    from `pre` (a string-prefix near miss) and, on unix, a `src/lib.rs` symlink to the victim's:
    each is exactly one violation naming the crate and the target kind. The victim, a `[lib] path`
    detour back into its own crate and `pre-x` pass. The fixture is removed after a pass.
  - Target checker cases on synthetic metadata (`synthetic::target_checker_*`): the crate's own
    source passes; another crate's file, a missing file and a relative path that does name the
    crate's own `lib.rs` from the test's working directory are each one violation; the crate
    missing, at another crate's directory, under another name, listed twice, or with no targets is
    one violation each; each unknown shape is an error, with a near miss that passes.
  - Parser and checker cases on synthetic lines (`synthetic::`): path, registry and git lines; each
    rejected shape, including a missing or malformed version, with version near misses
    (`0.0.0-rc.1+b2`); markers before and after the source, also on the allowed path and through a
    symlink alias that resolves to it (unix); parentheses (also `) (`) inside a path; every allowed
    crate at its path passes; each kind of outsider and a relative source that does name the allowed
    directory from the test's working directory are each exactly one violation; one outsider among
    the six allowed crates is reported by name.

Changing the gate: when #146 closes, the gate and this test go away (or the allowed set grows) in
the PR that closes it. Adding a parse-only crate to the CLI path means adding it to `ALLOWED` in a
PR that says why it runs no decode or raster phase.

## Persistence
None.

## Undo
None.

## Errors
Test failures only: the violation lines (`<name> (<source>): not a parse-only crate` or `…: not at
its workspace path <dir>`), the set difference, or `cargo tree failed (<status>): <stderr>`. The
target-path check adds `<name>: target <target> (<kinds>) compiles <src_path>, outside <dir>`,
`<name>: listed <n> times at its workspace path <dir>`, `<name>: no targets`, `<name>: its
workspace path <dir> doesn't resolve`, `unexpected cargo metadata shape: <what>` and `cargo
metadata failed (<status>): <stderr>`.

## Limits
None of its own; each graph test runs one `cargo tree` (about 40 ms), the fixture control a `cargo
generate-lockfile` and two (about 150 ms). The target-path tests run one `cargo metadata` each
(about 60 ms; the fixture one also a `cargo generate-lockfile`). The JSON reader bounds nesting at
128 levels; the input is Cargo's own output for our workspace.

## Security
- Threat: a decode or raster backend reaching the jpdf-cli binary while #146 is open, so the parse
  heap's high-water mark and a decode's mappings can add up past the 2 GiB the judge allows
  (specs/core/memory-budget.md).
- Arithmetic: none.
- What broken implementation still passes?
  - **Code, not packages:** the gate sees crates, not what is in them. Decode or raster code written
    inside an allowed crate (`jpdf-syntax`, `jpdf-engine`, …) passes. `jpdf-traits` holds the
    decoder and rasterizer traits and the conformance suites that drive them; its toy decoders are
    `#[cfg(test)]` only. The same holds for code that an allowed crate compiles from outside its
    directory, which `cargo tree` doesn't show: a manifest `[lib] path`, `[[bin]] path` or `build =
    …` pointing into another crate's sources, or `#[path = …] mod` and `include!` in source (round
    1, R1.1). The target-path check (#235) catches the manifest cases: a target root outside the
    crate's directory, also through a symlink. It sees target roots only: a module file under
    `src/` that is a symlink out of the directory, `#[path = …] mod` and `include!` are still
    review matters. "Inside the directory" is all the check knows: another workspace crate nested
    under an allowed crate's directory (a decode crate at `crates/jpdf-engine/filters/`, with
    `[lib] path = "filters/src/lib.rs"`) counts as inside and passes. No workspace crate is nested
    today; review is the check (R1.3).
  - Edge kinds: dev-dependencies are not on the binary's path and are not checked (`jpdf-cli` uses
    `jpdf-testgen` in its tests). Checked by hand-made mutants on this PR's branch: an optional
    `jpdf-filters` dependency behind a `jpdf-engine` feature fails `cli_graph_is_parse_only` (only
    that test: the feature is off from `jpdf-cli` alone); a `cfg(windows)` dependency and a
    build-dependency of `jpdf-cli` on `jpdf-render` fail both graph tests: `cli_graph_is_parse_only`
    with a `not a parse-only crate` line, `cli_graph_is_exactly_the_allowed_set` with the set
    difference (its first assertion). The lockfile was updated first, so the failure was not
    `--locked`.
  - Builds outside the declared graph: a build script that runs Cargo (or `rustc`) itself to compile
    another package adds no dependency edge, so the gate doesn't see it. No workspace crate has a
    build script today; review is the check.
  - The command's flags: no allowed crate has features or a target-limited dependency today, so the
    graph tests alone would pass without `--all-features`, `--target all` or the `build` edge kind.
    `control_flags_reach_features_other_targets_and_build_edges` pins all three on a fixture
    workspace (dropping any one of them, or replacing `--target all` with a single Windows or Linux
    triple, fails it on any host; checked by hand on this branch). The same test pins `--locked`:
    after a manifest edit the lockfile doesn't record, the command fails.
  - The target-path check: checked by hand-made mutants on this PR's branch (each killed): the
    inside test always true, a string-prefix comparison instead of path components, no
    canonicalization, only the kinds `lib`, `bin` and `custom-build` checked (R1.2), relative paths
    resolved, more than one listing accepted, an empty target
    list accepted, the manifest directory ignored, and the package name ignored.
- What the tests can't see: `--offline` needs every lockfile package's sources downloaded. Today the
  lockfile has no registry packages; once it has, CI builds the workspace before running tests, so
  they are present. A local run without them fails with `cargo tree failed` (fail closed).

## Accessibility
None.

## Platforms
Runs wherever `cargo nextest run --workspace` runs (CI: Linux).

## Performance
About 40 ms per graph test locally.

## Tests
`tools/jpdf-cli/tests/dependency_gate.rs`: `cli_graph_is_parse_only`,
`cli_graph_is_exactly_the_allowed_set`, `control_a_real_graph_outside_the_list_is_reported`,
`control_cargo_tree_failure_is_an_error`, `tree_args_are_exactly_these`,
`control_flags_reach_features_other_targets_and_build_edges`,
`synthetic::parser_reads_path_registry_and_git_lines`,
`synthetic::parser_rejects_unknown_shapes_and_empty_output`,
`synthetic::checker_accepts_every_allowed_crate_at_its_path`,
`synthetic::checker_rejects_each_kind_of_outsider`,
`synthetic::checker_reports_one_outsider_among_allowed_crates`,
`synthetic::parser_keeps_parentheses_inside_a_path`, `synthetic::parser_rejects_marker_shapes`,
`synthetic::marker_line_through_a_symlink_alias_is_refused`,
`synthetic::checker_rejects_relative_sources`, `cli_crates_compile_only_their_own_sources`,
`metadata_args_are_exactly_these`, `control_cargo_metadata_failure_is_an_error`,
`control_target_paths_outside_the_crate_are_reported`,
`synthetic::target_checker_accepts_own_sources_and_rejects_each_outside_one`,
`synthetic::target_checker_requires_the_crate_once_at_its_path`,
`synthetic::target_checker_rejects_unknown_metadata_shapes`.
