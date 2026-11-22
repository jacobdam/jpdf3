# 7. The Arlington TSVs are vendored and checked against the pin offline

Status: accepted (#60). Supersedes [ADR 0006](0006-arlington-model-fetched-by-pin-not-committed.md).

## Context
ADR 0006 fetched the Arlington model by pinned commit and tree id at build/test time and
committed neither the data nor anything generated from it, because NOTICE.txt puts "Software"
under Apache-2.0 and "Other Documentation" under CC-BY-4.0, and the TSV files fit neither
label clearly. The owner ruled on #60 (option 1): the TSVs and tables generated from them may
be committed, on these conditions:
- upstream `LICENSE` and `NOTICE.txt` from the pinned commit stay next to the committed data;
- generated files keep their header with the copyright, the Apache-2.0 notice and the source
  commit;
- tests and tools only; if a shipped crate ever needs the data, ask the owner first;
- the commit and tree pin stay; bumping it is an audit-time regeneration with a diff check.

The owner left the choice to us: vendor the raw TSVs, or commit only the generated tables
(prefer the smaller diff). At the pin, the 613 TSV files are 493,082 bytes; the generated
`tables.rs` is about 2.4 MB.

## Decision
1. **Vendor the raw TSVs**, byte for byte, in `tools/arlington/data/tsv/`, with upstream's
   `LICENSE` and `NOTICE.txt` from the pinned commit in `tools/arlington/data/`. It is the
   smaller diff (about a fifth of the tables), the PR shows upstream's own text when the pin
   moves, and the parser and checks keep running on the real data on every test run.
   `tools/arlington/data/.gitattributes` sets `* -text`, so git never rewrites line endings.
2. **The generated tables stay a build artifact** (`target/arlington/tables.rs`, written by
   `cargo xtask arlington`, header unchanged: Arlington copyright, Apache-2.0 notice, source
   commit). Tests that want the model use `jpdf-arlington` as a dev-dependency, or include
   the generated file, rather than committing a second copy.
3. **Offline pin check.** `fetch.rs` keeps `COMMIT` and `TSV_TREE` and adds the blob ids of
   `LICENSE` and `NOTICE.txt`. Loading the vendored copy reads each file once, computes its
   git blob id and the `tsv/` tree id from those same bytes (our own SHA-1, `std` only), and
   refuses any mismatch, any non-`.tsv` entry, any stray file next to the data, any symlink
   (files or directories), a changed `.gitattributes`, and anything over the size caps.
   So a hand edit, a missing or extra file, or a partial copy fails every test run and
   `cargo xtask ci linux`, and CI needs no network for Arlington any more.
4. **Bumping the pin** (audit sessions, or when a newer model is needed): update the four
   ids, delete `tools/arlington/data`, run `jpdf-arlington vendor --out tools/arlington/data`
   (it fetches the commit through ADR 0006's hardened, verified git cache, writes the files
   into a staging directory, loads them back through the offline check, and only then renames
   it into place), then `cargo xtask arlington`. The diff check is
   the PR diff of `tools/arlington/data/`, and the tree id proves the files equal upstream's.
5. **Not shipped.** `tools/arlington` is a development tool. Nothing in `crates/`,
   `bindings/` or `apps/` may depend on it or include its data or tables without asking the
   owner. A test checks the resolved dependency graph (normal and build edges) and scans
   shipped sources for the name.

## Consequences
- The repository grows by about 0.5 MB, plus that much again in history at each bump.
- `cargo xtask ci linux` and the crate's tests are offline. Only `jpdf-arlington vendor`
  (and `--cache` loads) touch the network.
- SHA-1 here guards against mistakes, not crafted collisions: the data reaches the
  repository only through a reviewed PR, and the ids are git's own.
- ADR 0006's git hardening (fsck, replace refs off, scrubbed environment) still applies to
  the bump path.
