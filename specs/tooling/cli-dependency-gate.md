# jpdf-cli dependency gate: the #146 phase gate on the resolved graph

Area: tooling
ISO 32000-2 clauses: none (build structure)
Journeys: none
Status: implemented

## Behavior
specs/core/memory-budget.md ("Phases run one at a time", **Gate**) says no decode or raster
phase runs on the jpdf-cli path until #146 is closed, by a #25 measurement or by a structural
fix. `tools/jpdf-cli/tests/dependency_gate.rs` (#154) enforces that on the package graph Cargo
resolves, never on Rust sources or manifests read as text (PR #151: a text scan can always be
bypassed; owner instruction, step 2).

- **Allowed set** (`ALLOWED`): `jpdf-cli` (`tools/jpdf-cli`), `jpdf-engine`, `jpdf-cos`,
  `jpdf-syntax`, `jpdf-traits` (under `crates/`) and `jpdf-fsio` (`tools/fsio`). Each entry
  is a package name and its workspace directory.
- **Command:** `cargo tree --manifest-path <workspace>/Cargo.toml -p <root>… -e normal,build
  --all-features --target all --prefix none --no-dedupe --format {p} --charset ascii --color
  never --locked --offline`, with `$CARGO` (falling back to `cargo` on `PATH`). A non-zero exit
  is an error, so a stale lockfile or a missing package fails the gate.
- **Parse:** each non-blank line must be `<name> v<version>`, optionally followed by one
  ` (<source>)` with no other `)`. Anything else (a `(*)` or `(proc-macro)` marker, a missing
  version, a name with a space) is an error, and so is an output with no packages: a format
  change fails the gate instead of passing it.
- **Check:** every package must have an allowed name and a source path that, canonicalized,
  equals its allowed directory, canonicalized. A registry package (no source), a git package,
  an allowed name at another crate's path or at a path that doesn't exist, and any other name
  are each one violation line. A `..` detour that resolves to the right directory passes.
- **Tests** (all in `dependency_gate.rs`):
  - `cli_graph_is_parse_only`: roots are all six allowed crates, so `--all-features` turns on
    every feature of each. A feature of `jpdf-engine` that pulls in a decoder is caught even if
    the judge's `-p jpdf-cli` build would not enable it (another workspace member might, and
    Cargo unifies features in a workspace build).
  - `cli_graph_is_exactly_the_allowed_set`: from `jpdf-cli` alone the name set equals
    `ALLOWED`. This proves the listing is transitive (`jpdf-traits` is three edges below) and
    keeps the list from holding crates the CLI no longer uses: removing a dependency means
    removing its entry.
  - `control_a_real_graph_outside_the_list_is_reported`: the same command on `xtask` reports
    `xtask`, `jpdf-arlington` and `jpdf-testgen`, and not the allowed `jpdf-fsio`.
  - `control_cargo_tree_failure_is_an_error`: an unknown `-p` package is `cargo tree failed`.
  - `control_flags_reach_features_other_targets_and_build_edges`: the same command on a
    fixture workspace written under `CARGO_TARGET_TMPDIR` (lockfile from `cargo
    generate-lockfile --offline`) lists a feature-gated optional dependency of a second root, a
    `cfg(windows)` dependency and a build-dependency; from the first root alone the optional one
    is not listed (why the gate takes every allowed crate as a root).
  - Parser and checker cases on synthetic lines: path, registry and git lines; each rejected
    shape; every allowed crate at its path passes; each kind of outsider is exactly one
    violation; one outsider among the six allowed crates is reported by name.

Changing the gate: when #146 closes, the gate and this test go away (or the allowed set grows)
in the PR that closes it. Adding a parse-only crate to the CLI path means adding it to
`ALLOWED` in a PR that says why it runs no decode or raster phase.

## Persistence
None.

## Undo
None.

## Errors
Test failures only: the violation lines (`<name> (<source>): not a parse-only crate` or
`…: not at its workspace path <dir>`), the set difference, or `cargo tree failed (<status>):
<stderr>`.

## Limits
None of its own; each graph test runs one `cargo tree` (about 40 ms), the fixture control a `cargo generate-lockfile` and two (about 150 ms).

## Security
- Threat: a decode or raster backend reaching the jpdf-cli binary while #146 is open, so the
  parse heap's high-water mark and a decode's mappings can add up past the 2 GiB the judge
  allows (specs/core/memory-budget.md).
- Arithmetic: none.
- What broken implementation still passes?
  - **Code, not packages:** the gate sees crates, not what is in them. Decode or raster code
    written inside an allowed crate (`jpdf-syntax`, `jpdf-engine`, …) passes. `jpdf-traits`
    holds the decoder and rasterizer traits and the conformance suites that drive them; its
    toy decoders are `#[cfg(test)]` only. The same holds for code that an allowed crate
    compiles from outside its directory, which `cargo tree` doesn't show: a manifest
    `[lib] path`, `[[bin]] path` or `build = …` pointing into another crate's sources, or
    `#[path = …] mod` and `include!` in source (round 1, R1.1; no workspace crate sets a target
    path today). Review is the check for these cases; checking each allowed package's target
    source paths from `cargo metadata` is #235.
  - Edge kinds: dev-dependencies are not on the binary's path and are not checked
    (`jpdf-cli` uses `jpdf-testgen` in its tests). Checked by hand-made mutants on this PR's
    branch: an optional `jpdf-filters` dependency behind a `jpdf-engine` feature fails
    `cli_graph_is_parse_only` (only that test: the feature is off from `jpdf-cli` alone); a
    `cfg(windows)` dependency and a build-dependency of `jpdf-cli` on `jpdf-render` fail both
    graph tests with a `not a parse-only crate` line (the lockfile was updated first, so the
    failure was not `--locked`).
  - The command's flags: no allowed crate has features or a target-limited dependency today,
    so the graph tests alone would pass without `--all-features`, `--target all` or the `build`
    edge kind. `control_flags_reach_features_other_targets_and_build_edges` pins all three on a
    fixture workspace (dropping any one of them fails it; checked by hand on this branch).
- What the tests can't see: `--offline` needs every lockfile package's sources downloaded.
  Today the lockfile has no registry packages; once it has, CI builds the workspace before
  running tests, so they are present. A local run without them fails with `cargo tree failed`
  (fail closed).

## Accessibility
None.

## Platforms
Runs wherever `cargo nextest run --workspace` runs (CI: Linux).

## Performance
About 40 ms per graph test locally.

## Tests
`tools/jpdf-cli/tests/dependency_gate.rs`: `cli_graph_is_parse_only`,
`cli_graph_is_exactly_the_allowed_set`, `control_a_real_graph_outside_the_list_is_reported`,
`control_cargo_tree_failure_is_an_error`, `control_flags_reach_features_other_targets_and_build_edges`,
`synthetic::parser_reads_path_registry_and_git_lines`,
`synthetic::parser_rejects_unknown_shapes_and_empty_output`,
`synthetic::checker_accepts_every_allowed_crate_at_its_path`, `synthetic::checker_rejects_each_kind_of_outsider`,
`synthetic::checker_reports_one_outsider_among_allowed_crates`.
