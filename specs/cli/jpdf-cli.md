# jpdf-cli (M0 stub)

Area: engine
ISO 32000-2 clauses: §7.2 (lexical conventions), §7.3 (objects), §7.5.4 (cross-reference table),
§7.5.5 (file trailer), §7.5.6 (incremental updates), §7.7.2 (document catalog), §7.7.3 (page tree)
Journeys: none (headless tool used by the judge and tests; ops/judge/CONTRACT.md §1)
Status: implemented

## Behavior
`tools/jpdf-cli` (package `jpdf-cli`, binary `target/release/jpdf-cli`) parses every command and
flag of CONTRACT §1:

| command | required flags | optional |
|---|---|---|
| `info FILE` | – | `--password` |
| `render FILE` | `--page N --dpi D --out OUT` | `--password` |
| `text FILE` | `--page N` | `--password` |
| `annotate FILE` | `--out --page --kind highlight --rect --color --save incremental` | – |
| `rewrite FILE` | `--out --mode full` | – |
| `encrypt FILE` | `--out --user-password --owner-password` | – |
| `fill FILE` | `--out --field --value` | – |
| `redact FILE` | `--out --page --rect` | – |
| `sign FILE` | `--out --p12 --p12-password --field` | – |
| `verify-signatures FILE` | – | – |

Argument validation (all failures exit **2** with one line on stderr, nothing on stdout):
- unknown command, unknown flag, flag without value, repeated flag, missing required flag,
  missing or extra positional argument;
- `--page`: integer in 1..=4294967295; `--dpi`: integer 1..=2400; `--rect`: four finite numbers `x0,y0,x1,y1`;
  `--color`: three integers 0..=255; `--kind` must be `highlight`; `--save` must be
  `incremental`; `--mode` must be `full`.

After validation every command except `info` exits **3** (not implemented) without reading FILE.

`info`: the CLI reads FILE (size checked against `Limits::max_file_size` before reading) and
hands the bytes to `jpdf_engine::Document::open(bytes, &Limits)`. On success it prints
`{"pages": N}` and a newline and exits 0. The engine currently understands only:
- a classic cross-reference table found through the last `startxref` in the final 64 KiB
  (§7.5.5; trailing junk is tolerated), with `/Prev` chains (§7.5.6);
- an unencrypted document;
- a catalog `/Pages` node whose `/Kids` are all distinct `/Type /Page` leaves (§7.7.3). A
  `/Count` that disagrees with the kids is read leniently (CONSTITUTION 17): the number of
  kids found wins.

`--password` is accepted and ignored for unencrypted files.

The engine never touches the file system; only the CLI does.

## Persistence
No output files. Nothing to persist.

## Undo
Not applicable (read-only).

## Errors
| condition | exit | stderr |
|---|---|---|
| bad arguments | 2 | `jpdf-cli: <reason>` |
| FILE missing/unreadable | 2 | `jpdf-cli: cannot read FILE: <io error>` |
| malformed file (no `startxref`, bad xref, object header mismatch, bad page tree, …) | 2 | `jpdf-cli: malformed PDF: <what>[ at byte <offset>]` (offset when the lexer knows it) |
| a limit exceeded | 2 | `jpdf-cli: limit exceeded: <which>` |
| unsupported structure (xref stream, hybrid `/XRefStm`, `/Encrypt`, nested page tree) | 3 | `jpdf-cli: not implemented: <what>` |
| any non-`info` command | 3 | `jpdf-cli: not implemented: <command>` |

No other exit code is ever produced; panics are bugs (no `catch_unwind` that would hide them
from the judge).

## Limits
`jpdf_syntax::Limits` (re-exported by `jpdf-engine`), defaults:
- `max_file_size` 512 MiB (checked by the CLI before reading and again by the engine);
- `max_nesting_depth` 256 for arrays/dictionaries (stack-overflow guard);
- `max_objects_per_parse` 4 000 000 objects produced while parsing one indirect object
  (memory bound for huge arrays/dictionaries);
- `max_xref_entries` 10 000 000 across all sections;
- `max_xref_sections` 1 024 `/Prev` hops (plus cycle detection on offsets);
- `max_pages` 1 000 000 kids;
- lexing budget: at most `4 × file size + 1 MiB` bytes lexed per document
  (`lex_budget_factor`, `lex_budget_slack`). Hostile files can overlap objects or xref
  sections (an object hidden inside another's string, a `/Prev` pointing into the trailer's own
  string); without a global budget, resolving them rescans the same bytes and time grows
  quadratically.

Memory: the CLI reserves exactly the file size before reading (no doubling); xref entries are
16 bytes each in a `BTreeMap` (no rehash doubling). Measured worst case so far: a 500 MiB file
with 10 M xref entries peaks at ~0.95 GB RSS, under the judge's 2 GiB address-space limit.

All input-derived arithmetic is checked; numbers that overflow `i64` are malformed input.
Work is linear in file size: the lexing budget bounds total lexing to 4× the file.

## Security
Input is hostile. No `unwrap`/`expect`/panicking index on input-derived data; recursion depth is
bounded; offsets from the file are bounds-checked; `/Prev` loops are detected. No PDF content is
executed. The CLI prints only the page count, never file content.

## Accessibility
Not applicable (headless tool). Messages are plain text on stderr.

## Platforms
Same on Linux and macOS. Not shipped in the apps.

## Performance
`info` on the smoke corpus must stay far below the judge's 20 s limit; it reads the file once and
resolves only the catalog, the page-tree root and its kids.

## Tests
- `tools/jpdf-cli/tests/cli.rs`: every contract command with valid flags exits 3; argument errors
  exit 2 with empty stdout; `info` on generated minimal PDFs prints the right count; empty,
  garbage and truncated files exit 2 or 3.
- `crates/jpdf-engine/tests/open.rs`: page counts for 0/1/3/50 pages, `/Prev` updates, `/Prev`
  loops, lenient `/Count`, deep nesting, overflow numbers, unsupported structures, file-size and
  page limits, overlapping objects and overlapping `/Prev` sections (lexing budget).
- Deterministic property test (seeded xorshift; the workspace has no admitted `proptest` yet,
  see #16): thousands of random byte strings and mutated/truncated valid files never panic.
- E2E test IDs: none.
