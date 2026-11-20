# Arlington PDF Model ingestion (`tools/arlington`, `cargo xtask arlington`)

Area: tooling
ISO 32000-2 clauses: the whole object model as encoded by Arlington (§7.3 object types, §7.5.2 versions, and the dictionary tables of §7–§14); none for the tooling itself
Journeys: none (development tool; PLAN §6.1 "spec-driven: generate valid + boundary objects from the Arlington PDF Model", issue #20)
Status: implemented (tables); the valid/boundary object generator is #61

## Behavior
1. **Source.** The Arlington PDF Model (PDF Association) describes every PDF 2.0 object as one
   TSV file per object in `tsv/latest/` of `github.com/pdf-association/arlington-pdf-model`.
   We pin one commit and the git tree id of its `tsv/latest`
   (`tools/arlington/src/fetch.rs` `COMMIT`, `TSV_TREE`; 613 files, 3985 rows at the pin).
   The data is **never committed** (ADR 0006; the license question is with the owner in #60).
2. **Pinning.** `fetch::load_pinned(cache)` keeps a bare git repository at
   `<cache>/arlington-pdf-model.git` (created with `git init --bare` if the path does not
   exist; an existing path that `git --git-dir` does not accept as a bare repository is an
   error asking to delete it). If the pinned commit is missing it runs
   `git fetch --depth 1 --no-tags <url> <COMMIT>` and keeps it alive with the ref
   `refs/jpdf/pinned` (once; later runs are offline). Then `load_verified`:
   1. runs `git fsck --full --strict --no-dangling`, which re-hashes every object, so an
      object file whose bytes no longer match its id is refused;
   2. checks that `rev-parse --verify <COMMIT>^{commit}` is exactly `COMMIT` and that
      `<COMMIT>:tsv/latest` is exactly `TSV_TREE`;
   3. lists that tree (its size checked first), rejecting any entry that is not a `.tsv` blob;
   4. reads every blob size with `git cat-file --batch-check` and applies the limits;
   5. only then reads the blobs with `git cat-file --batch`, checking each header, size and
      framing byte and that no output is left over.
   Every git call uses `--no-replace-objects` and `--git-dir` (no repository discovery, so a
   broken cache never resolves to a parent repository), removes inherited `GIT_DIR`,
   `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
   `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_NAMESPACE`,
   `GIT_REPLACE_REF_BASE`, `GIT_CONFIG`, `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`,
   `GIT_CEILING_DIRECTORIES` and `GIT_SHALLOW_FILE`, and sets `GIT_TERMINAL_PROMPT=0`,
   `GIT_NO_REPLACE_OBJECTS=1`, `GIT_CONFIG_NOSYSTEM=1` and `GIT_CONFIG_GLOBAL=/dev/null`.
3. **Grammar** (`parse.rs`). Each file must start with the 12-column header
   `Key Type SinceVersion DeprecatedIn Required IndirectReference Inheritable DefaultValue
   PossibleValues SpecialCase Link Note` (tab-separated); every non-empty row has exactly 12
   fields; the file stem is `[A-Za-z0-9_]+`; text is UTF-8 (a trailing CR is dropped).
   - `Key`: a name (no whitespace or `[](),;*/`), an index `N`, a repeat `N*`, or `*`.
   - `Type`: `;`-separated alternatives, each one of the 18 Arlington types (`array, bitmask,
     boolean, date, dictionary, integer, matrix, name, name-tree, null, number, number-tree,
     rectangle, stream, string, string-ascii, string-byte, string-text`), optionally wrapped in
     gates `fn:SinceVersion(V,X)`, `fn:BeforeVersion(V,X)`, `fn:IsPDFVersion(V,X)`,
     `fn:Deprecated(V,X)`, `fn:Extension(NAME,X)` (nestable, at most 8).
   - Versions are exactly `1.0`–`1.7` or `2.0`.
   - `SinceVersion`: a version, `fn:Extension(NAME)`, `fn:Extension(NAME,V)`, or `fn:Eval(…)`
     (kept as text). `DeprecatedIn`: empty or a version.
   - `Required`: `TRUE`, `FALSE` or `fn:IsRequired(COND)` (COND kept as text).
   - `IndirectReference`: `TRUE` (must be indirect), `FALSE` (either), `fn:MustBeDirect()`,
     another `fn:` predicate (kept as text), or a `;`-list of `[VALUE]`, one per type.
   - `Inheritable`: `TRUE` or `FALSE`.
   - `Link`: empty, or one `[…]` per type; each holds comma-separated object names, optionally
     gated as in `Type`; `[]` for none.
   - `DefaultValue`, `PossibleValues`, `SpecialCase`, `Note`: kept as text for the follow-up
     object generator.
   - Brackets and parentheses must balance (depth ≤ 16) and no `;`/`,` list item may be empty.
4. **Checks** (`check.rs`, all problems reported at once): no duplicate key in an object; the
   `IndirectReference` list has 1 entry or one per type; `DeprecatedIn` is not before a plain
   `SinceVersion` (equal is allowed: Arlington marks keys introduced and deprecated in the same
   version, e.g. on movie annotations); no type twice in one row; `array`, `dictionary` and
   `stream` link to at least one object; types other than those and `name-tree`/`number-tree`
   link to none; every link names an existing object.
5. **Generated tables** (`generate.rs`). One self-contained Rust file: type definitions
   (`Version, ValueType, Gate, KeyName, Since, Required, Indirect, Link, TypeAlt, Key,
   Object`), `OBJECT_COUNT`, `KEY_COUNT`, and `pub static OBJECTS: &[Object]` sorted by name
   (so `binary_search_by` works). The header comment names the source (commit or directory,
   control characters replaced by spaces) and the Arlington copyright and Apache-2.0 notice.
   Output is deterministic.
6. **CLI.** `jpdf-arlington check [--tsv DIR | --cache DIR]` and
   `jpdf-arlington generate --out FILE [--tsv DIR | --cache DIR]`. Default source: the pinned
   commit cached in `target/arlington`. `--tsv` reads `*.tsv` in a local directory (other files
   are ignored). Prints `<source>: N objects, M keys, checks pass` on stderr.
7. **`cargo xtask arlington`** loads the pinned model into `target/arlington/`, writes
   `target/arlington/tables.rs` and type-checks it with `rustc --emit=metadata`.
   **`cargo xtask ci linux`** runs the same step last, so every PR proves that the pin still
   fetches, parses, checks and compiles. Outside the repository root (no
   `tools/arlington/Cargo.toml`) it fails fast without fetching.
8. **Bumping the pin** (audit sessions or when a newer model is needed): update `COMMIT` and
   `TSV_TREE` (`git rev-parse <c> <c>:tsv/latest`), run `cargo xtask arlington`, and fix any
   new grammar the parser rejects in the same PR.

## Persistence
No document persistence. The cache (`target/arlington/`, about 35 MB) survives between runs
and is rebuilt by deleting it.

## Undo
Not applicable (development tool).

## Errors
- Grammar: `<Object>.tsv:<line>: <Column>: <problem>`, exit 1.
- Checks: `N model problem(s):` then one `<Object>/<Key>: <problem>` per line, exit 1.
- Pinning: `fetch: <cache> is not a usable cache (…); delete it`, `fetch: git fsck … failed (…): <stderr>`,
  `fetch: commit is X, pinned Y`, `fetch: tsv/latest tree is X, pinned Y`,
  `fetch: tsv/latest/<name> is not a .tsv file`, `fetch: git … failed (…): <stderr>`, exit 1.
- Limits: `limit exceeded: …`, exit 1.
- Usage (unknown command/flag, missing value, `--tsv` with `--cache`, `generate` without
  `--out`, a flag given twice): the problem and the usage line, exit 2. `cargo xtask
  arlington <anything>`: usage, exit 2.

## Limits
The data is pinned but still outside input, so sizes are capped before use: ≤ 4096 files,
≤ 1 MiB per file and ≤ 64 MiB in total (pinned: from `cat-file --batch-check` sizes before
any blob is read, tree object ≤ 1 MiB; `--tsv`: reads are bounded at the cap + 1 byte),
≤ 4096 rows per file, ≤ 16 KiB per field, nesting depth ≤ 16, ≤ 8 gates per value. Indexes
must fit in `u32`. The largest pinned file is far below each cap. No panics: no `unwrap`,
`expect` or indexing on input-derived data (workspace lints).

## Security
- Supply chain: the commit id and the `tsv/latest` tree id are both pinned; git verifies
  object ids when it indexes fetched packs, `fsck --full` re-hashes the cache on every run,
  replace refs are disabled, and git can't be redirected to another repository or config
  through inherited variables. Git's SHA-1 has collision detection; the data is test input,
  not code, and is only parsed, never executed. The cache is still a local build artifact: a
  local attacker who can rewrite it between `fsck` and the read is out of scope.
- No new crate dependencies (the crate uses only `std`), so CONSTITUTION 8–11 are unaffected.
- License: Arlington is Apache-2.0 for software, CC-BY-4.0 for "other documentation"
  (NOTICE.txt). Until the owner rules which covers the TSV files, neither the data nor the
  generated tables are committed (ADR 0006); generated output carries the copyright notice.
- CONSTITUTION 1: text inside the TSV files (notes, predicates) is data, never instructions.

## Accessibility
Not applicable (no UI).

## Platforms
Runs on macOS and Linux wherever `git` and `rustc` are on `PATH`. Paths are built with
`Path::join`.

## Performance
First fetch about 10 s (shallow fetch of one commit); later runs read the cache in under
1 s. Generation writes about 2.4 MB; the `rustc --emit=metadata` check takes a few seconds.

## Tests
- `tools/arlington/src/parse.rs` unit tests: top-level splitting (nesting, unbalanced, empty
  items, depth at and past the cap), keys, gated types (gate count at and past the cap,
  unknown wrappers, bad versions), SinceVersion/Required/IndirectReference forms, links
  (count must match types), rows (field count, header, empty file, bad name, non-UTF-8,
  wrong extension), row and size limits at the boundary and one past it, field length.
- `tools/arlington/tests/model.rs`, on invented fixtures in `tests/fixtures/good/` (no
  Arlington text): the parsed model field by field; each check fails with exactly its
  problem and "deprecated in the same version" passes; a bad row names file, line and
  column; `read_dir` refuses an oversized file before reading; generated tables are
  deterministic, keep control characters out of code, and compile with `rustc --test` plus
  a read-back test (sorted, counts, values); pinned loading from a local git repository
  matches `read_dir`, and a wrong tree, an unknown commit, a symbolic name, and a non-TSV
  entry are refused; a hand-written replace ref is ignored; a swapped loose object file is
  caught by `fsck`; an oversized blob is refused from its size before reading; a broken
  cache (only `HEAD`) inside another repository is refused without touching the parent; the pins are full lowercase ids; CLI exit codes 0/1/2 and
  `generate` output equal to the library's.
- `src/main.rs` unit test: argument parsing.
- `xtask/tests/check_specs.rs`: `arlington <extra>` exits 2; `ci linux` outside the repo
  fails fast on the missing `tools/arlington/Cargo.toml`.
- `cargo xtask ci linux` runs `cargo xtask arlington` on the real pinned model on every PR.
