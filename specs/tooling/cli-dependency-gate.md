# jpdf-cli dependency gate: the #146 phase gate on the resolved graph

Area: tooling ISO 32000-2 clauses: none (build structure) Journeys: none Status: implemented

## Behavior
specs/core/memory-budget.md ("Phases run one at a time", **Gate**) says no decode or raster phase
runs on the jpdf-cli path until #146 is closed, by a #25 measurement or by a structural fix.
`tools/jpdf-cli/tests/dependency_gate.rs` (#154) enforces that on the package graph Cargo resolves,
never on Rust sources or manifests read as text (PR #151: a text scan can always be bypassed; owner
instruction, step 2).

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
  `(proc-macro)` before the source) or ending with `) (*` or `) (proc-macro`. Such a line is never
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
  - `tree_args_are_exactly_these`: the command's arguments after the roots are pinned as one string,
    so no flag can be dropped or narrowed (`--target all` to a set of triples, R3.1).
  - `control_cargo_tree_failure_is_an_error`: an unknown `-p` package is `cargo tree failed`.
  - `control_flags_reach_features_other_targets_and_build_edges`: the same command on a fixture
    workspace written under `CARGO_TARGET_TMPDIR` (lockfile from `cargo generate-lockfile
    --offline`) lists, in one run, a feature-gated optional dependency of a second root, a
    `cfg(windows)` and a `cfg(target_os = "linux")` dependency (no single target or host has both,
    so `--target <one triple>` fails it too) and a build-dependency; from the first root alone the
    optional one is not listed (why the gate takes every allowed crate as a root); after an edit to
    the fixture's manifest that its lockfile doesn't record, `cargo tree failed` (`--locked`). The
    fixture directory is removed after a pass.
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
its workspace path <dir>`), the set difference, or `cargo tree failed (<status>): <stderr>`.

## Limits
None of its own; each graph test runs one `cargo tree` (about 40 ms), the fixture control a `cargo
generate-lockfile` and two (about 150 ms).

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
    1, R1.1; no workspace crate sets a target path today). Review is the check for these cases;
    checking each allowed package's target source paths from `cargo metadata` is #235.
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
`synthetic::checker_rejects_relative_sources`.
