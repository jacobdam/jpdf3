# 6. The Arlington model is fetched by pinned commit and tree id, never committed

Status: accepted (#20)

## Context
PLAN §6.1 wants valid and boundary test objects generated from the Arlington PDF Model.
Arlington is one TSV file per object in `tsv/latest/` of
`github.com/pdf-association/arlington-pdf-model`. Its LICENSE is Apache-2.0, but NOTICE.txt
covers only "Software" with Apache-2.0 and puts "Other Documentation" under CC-BY-4.0. The
TSV files fit neither label clearly, and CC-BY-4.0 is not in CONSTITUTION 10. Issue #20
says: vendor only if the license is allowed, else fetch at build/test time.
The workspace has no third-party crates, so the tool can't use a hashing or HTTP crate
without going through admission.

## Decision
1. **Don't commit the data or anything generated from it** until the owner rules (#60).
   Committed tests use invented fixtures written in Arlington's grammar.
2. **Pin by git ids.** `tools/arlington/src/fetch.rs` names the commit and the git tree id
   of `tsv/latest`. A bare cache repository in `target/arlington/` gets the commit once
   (`git fetch --depth 1`). Every run checks both ids and reads the files only from git
   objects reached through them. Git hashes every object it receives, so a tampered cache
   or mirror can't change the bytes without changing the ids. The data is parsed, never
   executed, so git's SHA-1 (with collision detection) is enough here.
3. **Generated tables live in `target/arlington/tables.rs`**, one self-contained file that
   carries the Arlington copyright and Apache-2.0 notice. `cargo xtask arlington` writes it
   and type-checks it. `cargo xtask ci linux` runs the same on every PR, so the pin keeps
   working. Later consumers (#61, `jpdf-cos` tests) include it from there.
4. **Sync.** The pin is bumped in audit sessions (like `rust-toolchain.toml` and the fuzz
   nightly) or when a newer model is needed: update both ids, run `cargo xtask arlington`,
   and fix new grammar in the same PR. There is no checked-in copy, so nothing else can drift.

## Consequences
- Every PR and the first local run need network once (about 10 s, 35 MB cache).
- If the owner allows committing (#60, option 1), the generated tables can move into a
  crate and the fetch step becomes an audit-time regeneration with a diff check.
- `ci linux` fails if GitHub or the pinned commit becomes unreachable. That's an infra
  failure: rerun, don't change code.
