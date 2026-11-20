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
  kids found wins; a missing or non-integer `/Count` is ignored the same way.

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
| malformed file (no `startxref`, bad xref, object header mismatch, overlapping objects, bad page tree, …) | 2 | `jpdf-cli: malformed PDF: <what>[ at byte <offset>]` (offset when the lexer knows it) |
| a limit exceeded | 2 | `jpdf-cli: limit exceeded: <which>` |
| unsupported structure (xref stream, hybrid `/XRefStm`, `/Encrypt`, nested page tree) | 3 | `jpdf-cli: not implemented: <what>` |
| any non-`info` command | 3 | `jpdf-cli: not implemented: <command>` |

No other exit code is ever produced; panics are bugs (no `catch_unwind` that would hide them
from the judge).

## Limits
`jpdf_syntax::Limits` (re-exported by `jpdf-engine`), defaults:
- `max_file_size` 512 MiB (checked by the CLI before reading and again by the engine);
- `max_nesting_depth` 256 for arrays/dictionaries (stack-overflow guard);
- `max_objects_per_parse` 2 000 000 objects produced while parsing one indirect object
  (memory bound for huge arrays/dictionaries);
- `max_objects_per_document` 20 000 000 objects across all parses (time bound: `[]` is 2 bytes
  but costs an allocation; 1 M pages × ~20 objects fits);
- `max_string_bytes_per_parse` 64 MiB of decoded string and name bytes per indirect object (memory
  bound; the reference look-ahead never decodes strings).
- `max_xref_entries` 10 000 000 across all sections;
- `max_xref_sections` 1 024 `/Prev` hops (plus cycle detection on offsets);
- `max_pages` 1 000 000 kids;
- no overlapping indirect objects or xref sections: each parsed section's and resolved object's
  byte span is recorded, and one that starts inside, or extends over, an earlier one is
  malformed (exit 2). Objects in
  well-formed files never overlap (§7.3.10, §7.5.3); hostile files overlap them so that many
  references re-parse the same expensive bytes (e.g. 600 kids sharing one 2 M-entry body: 30 s
  before this check). Total object parsing is therefore about one pass over the file;
- lexing budget: at most `4 × file size + 1 MiB` bytes lexed per document
  (`lex_budget_factor`, `lex_budget_slack`), charged per byte inside the lexer (including
  reference look-ahead that is re-read), so it fails mid-scan. Legitimate files need ≤ 3×
  (integer arrays re-read look-ahead tokens; a test shows they fit in 3× with no slack).
  Measured lexing speed ≈ 400 MB/s, so the worst case for a 512 MiB file is ≈ 5 s, under the
  20 s `info` limit. Hostile files can overlap objects or xref
  sections (an object hidden inside another's string, a `/Prev` pointing into the trailer's own
  string); without a global budget, resolving them rescans the same bytes and time grows
  quadratically.

Memory: the CLI reserves exactly the file size before reading (no doubling); xref entries are
packed into a sorted `Vec` of 12-byte records (newest entry per object number); the stored
trailer keeps only `/Root` and `/Size`; the page-tree walk keeps at most one parsed object
alive (catalog and `/Pages` are dropped once their references are extracted). With the
per-object caps, peak memory ≈ file + 12 B × xref entries + one capped parse.
Measured (Linux, release, real `ulimit -v 2097152` as in the judge): every adversarial file
from review — 512 MiB with 10 M xref entries and four maximal objects, 500 MiB nested
strings, overlapping objects/sections, 600 kids sharing one 2 M-object body — finishes with
exit 0 or 2 in ≤ 3.4 s (1 000 `/Prev` sections over one 2 M-name trailer: 36 s before
section spans were checked; 127 dense non-overlapping kids: 9.9 s before the per-document
object budget). macOS peak footprint for the worst one: 1.31 GB.

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
  page limits, objects nested in strings and kids sharing one body (overlap rejection),
  overlapping `/Prev` sections (rejected), per-document object budget, out-of-range xref offsets (read as free).
- Deterministic property test (seeded xorshift; the workspace has no admitted `proptest` yet,
  see #16): thousands of random byte strings and mutated/truncated valid files never panic.
- E2E test IDs: none.
