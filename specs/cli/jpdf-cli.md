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

`info`: the CLI reads FILE and hands the bytes to `jpdf_engine::Document::open(bytes, &Limits)`. On success it prints
`{"pages": N}` and a newline and exits 0. The engine currently understands only:
- a classic cross-reference table found through the last `startxref` in the final 64 KiB
  (§7.5.5; trailing junk is tolerated), with `/Prev` chains (§7.5.6);
- an unencrypted document;
- a catalog `/Pages` node whose `/Kids` are all distinct `/Type /Page` leaves (§7.7.3). A
  `/Count` that disagrees with the kids is read leniently (CONSTITUTION 17): the number of
  kids found wins; a missing or non-integer `/Count` is ignored the same way.

`--password` is accepted and ignored for unencrypted files.

Reading FILE (#77), in order:
1. `stat` the path (following symlinks). Anything but a regular file (FIFO, socket, character
   or block device, directory) is refused: `cannot read FILE: not a regular file`. A size above
   `Limits::max_file_size` is refused: `limit exceeded: file size`. Neither case opens the file.
2. Open it with `O_NONBLOCK`, so if the path was swapped for a FIFO after step 1 the open
   still returns at once; on Linux also `O_NOCTTY`, so a swapped-in terminal never becomes the
   controlling terminal (`jpdf_fsio::open_nonblocking`, specs/tooling/fsio.md, which lists the
   values per target). Other targets (mips, sparc, … use different values) open with plain
   flags and rely on step 1 alone. Step 1's checks run again on the opened
   file (`fstat`).
3. Reserve exactly the file size and read at most `max_file_size + 1` bytes (a file growing
   while it is read hits the limit instead of growing memory).

Reading from a pipe (`/dev/stdin`, `<(…)`) is therefore refused with exit 2; CONTRACT §1 always
passes a regular file path.

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
| FILE not a regular file (FIFO, device, directory, socket) | 2 | `jpdf-cli: cannot read FILE: not a regular file` |
| FILE larger than `max_file_size` | 2 | `jpdf-cli: limit exceeded: file size` |
| malformed file (no `startxref`, bad xref, object header mismatch, overlapping objects, bad page tree, …) | 2 | `jpdf-cli: malformed PDF: <what>[ at byte <offset>]` (offset when the lexer knows it) |
| a limit exceeded | 2 | `jpdf-cli: limit exceeded: <which>` |
| unsupported structure (xref stream, hybrid `/XRefStm`, `/Encrypt`, nested page tree) | 3 | `jpdf-cli: not implemented: <what>` |
| any non-`info` command | 3 | `jpdf-cli: not implemented: <command>` |

No other exit code is ever produced; panics are bugs (no `catch_unwind` that would hide them
from the judge).

## Limits
The nightly checks these at scale: `cargo xtask limits` (specs/tooling/limits.md, #25) builds seven
hostile files of about 500 MiB and runs `info` on each under the judge's 2 GiB `RLIMIT_AS` and 20 s
limit.

`jpdf_syntax::Limits` (re-exported by `jpdf-engine`), defaults. `Document::open` clamps the limits it
is given to these defaults, so a caller can only tighten them (specs/core/limits-ceiling.md):
- `max_file_size` 512 MiB (checked by the CLI from `stat` before opening, again on the
  opened file and after reading, and again by the engine). Boundary test: a write-only sparse
  file of exactly 512 MiB passes the size check (the open then fails with "Permission
  denied"); 512 MiB + 1 is refused with the limit error without being opened;
- `max_nesting_depth` 256 for arrays/dictionaries (stack-overflow guard);
- `max_objects_per_parse` 1 048 576 (2^20) objects produced while parsing one indirect object
  (memory bound for huge arrays/dictionaries; lowered from 2 000 000 by #109 because a parsed
  object can hold up to 272 bytes of allocator chunks, specs/core/memory-budget.md);
- `max_objects_per_document` 20 000 000 objects across all parses (time bound: `[]` is 2 bytes
  but costs an allocation). This is the effective ceiling for very large documents: 1 M
  pages with > 20 objects per page dictionary exit 2 even though `max_pages` allows them.
  Scaling it with the page count is a follow-up;
- `max_string_bytes_per_parse` 32 MiB (64 MiB before #146, specs/core/memory-budget.md) of decoded string and name bytes per indirect object (memory
  bound; the reference look-ahead never decodes strings). Exact for every token kind (#113): a
  total of exactly the limit parses, one decoded byte more is refused. Each finished name, hex or
  literal string is checked as a whole, so a name's last byte and an odd hex string's implied
  trailing 0 count too (they used to slip past the in-loop checks, 1 byte per token). The in-loop
  checks stay: they are the memory bound, stopping a long token mid-scan at most one byte past
  the room. Boundary tests: `string_room_is_exact_for_every_token_kind`,
  `string_room_is_shared_and_exact_across_tokens`; `string_room_stops_the_scan_early` shows an
  over-long string, name or hex string fails after ≤ 16 lexed bytes, not at its end.
- `max_xref_entries` 2 097 152 (2^21) across all sections (lowered from 10 000 000 by #109
  and from 2^22 by #146 so the entry table and the overlap map fit the memory budget,
  specs/core/memory-budget.md);
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
  The reference look-ahead (`num gen R`, §7.3.10) is bounded (#113): it reads a generation token
  of at most 64 bytes and a third token of exactly `R`, and stops at either bound (it steps over
  at most that many bytes and peeks at one more).
  A generation fits `u16` (5 digits); 64 bytes leaves ample room for a sign and zero padding, so
  padded generations still read leniently. A longer generation token is deliberately not a
  reference: `1 000…(65 bytes)…3 R` is malformed (`R` is then an unexpected keyword). No real
  producer pads that far, and an unbounded look-ahead is the cost #113 removed. So a long token
  after an integer is lexed and classified once, as itself, not up to 3 times. Whitespace and
  comment runs, which the look-ahead also rewinds over, are scanned once too: the lexer
  remembers the last two runs it skipped (start and end), and a later skip from the same start
  jumps to the end (boundary tests `generation_token_length_boundary`,
  `reference_look_ahead_is_bounded`, `whitespace_and_comments_are_scanned_once`).
  Measured lexing speed ≈ 400 MB/s, so the worst case for a 512 MiB file is ≈ 5 s, under the
  20 s `info` limit. Hostile files can overlap objects or xref
  sections (an object hidden inside another's string, a `/Prev` pointing into the trailer's own
  string); without a global budget, resolving them rescans the same bytes and time grows
  quadratically.

Memory: the CLI reserves exactly the file size before reading (no doubling); xref entries are
packed into a sorted `Vec` of 12-byte records (newest entry per object number); the stored
trailer keeps only the first usable `/Root` and `/Size` (at most 2 entries, since #109); the
page-tree walk keeps at most one uncached parsed object alive. Since #107 the resolve cache
(specs/core/resolve-cache.md) keeps every resolved object that fits
`max_retained_bytes_per_document`, so the catalog, `/Pages` and the kid dictionaries can stay
alive for the whole walk, within that budget. The full peak-memory model, with every term and a reserve,
is specs/core/memory-budget.md.
Measured before #109 lowered the defaults (Linux, release, real `ulimit -v 2097152` as in the
judge): every adversarial file
from review — 512 MiB with 10 M xref entries and four maximal objects, 500 MiB nested
strings, overlapping objects/sections, 600 kids sharing one 2 M-object body — finishes with
exit 0 or 2 in ≤ 3.4 s (1 000 `/Prev` sections over one 2 M-name trailer: 36 s before
section spans were checked; 127 dense non-overlapping kids: 9.9 s before the per-document
object budget). macOS peak footprint for the worst one: 1.31 GB.
Long number tokens (#113; macOS, release, `/usr/bin/time`, 2026-11-21, before → after the
bounded look-ahead): a 534 MB `[1 1 000…0]` 5.9 s → 2.4 s; `[1 1 1.000…1]` of the same size
4.6 s → 1.75 s; eight 63 MiB zero-integers in one array 5.3 s → 2.4 s. Audit 0002 had
measured 6.84 s for the first one.

All input-derived arithmetic is checked, and `clippy::arithmetic_side_effects` is denied in the
library/binary crate roots of `jpdf-syntax`, `jpdf-cos`, `jpdf-engine` and `jpdf-cli` (#77), so
an unchecked `+`/`-`/`*` there fails CI's `cargo clippy -D warnings` (not `cargo build`).
Integration tests under `tests/` and other crates are not covered yet (#105). Integers are exactly `i64::MIN..=i64::MAX` (§7.3.3 leaves the range
to the implementation; Annex C recommends at least 32 bits); one past either end is malformed
(boundary test `integer_range_boundaries`). Number tokens are never copied: reals are parsed
in place from the input slice, so a 290 M-digit real costs no memory beyond the file (it
used to allocate a copy the size of the token, outside every budget). Its bytes are charged
to the lexing budget like any other token. Malformed `/Prev` values (§7.5.5) are matched by
reference and rejected without cloning (a huge `/Prev` array used to be deep-copied first).
Work is linear in file size: the lexing budget bounds total lexing to 4× the file.

## Security
Self-check (#77):
- Arithmetic: the lint above makes every unchecked `+`/`-`/`*` a CI clippy error in the four crates;
  integer accumulation uses `checked_mul`/`checked_add`/`checked_sub`, octal escapes are at
  most three digits (≤ 0o777, saturating ops that never saturate).
- What broken implementation still passes? A CLI that opens before checking the size fails
  `oversized_file_is_rejected_before_opening` (write-only file: "Permission denied" instead of
  the limit); as root only that ordering half is skipped, the limit message is still checked.
  A CLI that blocks on FIFOs fails `fifo_and_devices_exit_2_without_blocking` (20 s timeout).
  A wrong `O_NONBLOCK` value, or a `check_metadata` that accepts an opened FIFO, fails
  `open_never_blocks_on_a_fifo` (Linux in CI; macOS locally, because the macOS CI job runs no
  Rust tests). A missing `take(max + 1)` fails `read_stops_one_byte_past_the_limit` (an
  endless reader). An `i64::MIN` regression fails `integer_range_boundaries`.
  String room (#113): dropping the whole-token check, or making it off by one either way,
  fails the two `string_room_*` tests (each token kind at the limit and one past; the strict
  version also fails the older room tests). Dropping the in-loop room checks (keeping only the
  whole-token one, so a hostile token is decoded in full first) fails
  `string_room_stops_the_scan_early`. An unbounded generation or `R` look-ahead fails
  `reference_look_ahead_is_bounded` (bytes lexed ≤ input + 68); a generation bound of 63 or 65
  fails `generation_token_length_boundary`, and a scan bound 40 bytes looser than the constant
  also fails the bytes-lexed test. Dropping the skip memo, or keeping only one entry, fails
  `whitespace_and_comments_are_scanned_once`. Still passing: an `R` bound of 2 instead of 1
  (equivalent: a 2-byte run is never `R`), and a scan bound up to 3 bytes looser than the
  constant (inside the bytes-lexed slack; the length-boundary test catches it). The look-ahead
  test counts lexed bytes, not classify passes; classifying happens only on a run the bounded
  scan returned, so it is ≤ 64 bytes.
  Still passing: deleting the `fstat` re-check call in `read_limited` (a swap between `stat`
  and `open` is a race no test can stage deterministically; both halves it combines are tested
  on their own), and dropping the pre-open FIFO check (the non-blocking open + `fstat` give the
  same message).
- What the tests can't see: allocations. Nothing asserts that reals and `/Prev` are not
  copied (a counting global allocator is banned by CONSTITUTION 3); review covers it, and the
  adversarial memory harness (#25) is the long-term check. The 4 GiB sparse-file case is
  covered by the ordering test above instead of an RSS measurement.
- Residual: once opened, the descriptor is fixed, so a swap after the `fstat` changes nothing;
  a swap between `stat` and `open` opens non-blocking and the `fstat` check refuses it.

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
  garbage and truncated files exit 2 or 3; an oversized (sparse, write-only) file is refused
  before opening, with the exact-boundary case; a FIFO, a directory, `/dev/null` and `/dev/zero`
  exit 2 at once with "not a regular file". Tests remove their FIFO, directory and sparse files.
- `tools/jpdf-cli/tests/dependency_gate.rs`: the #146 phase gate on the resolved dependency
  graph (only parse-only crates; specs/tooling/cli-dependency-gate.md, #154).
- `tools/jpdf-cli/src/main.rs` unit tests: the non-blocking open (`jpdf-fsio`, which has its
  own FIFO test) returns at once on a FIFO and the opened-file check refuses it; reading stops one byte past the limit (exactly `max`
  passes; `max + 1` and an endless source fail).
- `crates/jpdf-syntax/src/lexer.rs` unit tests: `i64` range boundaries, every real shape
  (`5.`, `.5`, `+.5`, `-3.`, `-0.`), 1 MB-digit reals, non-numbers, octal and hex escapes;
  string room at the limit and one past for names (with `#xx`), even and odd hex strings and
  literal strings (escape or EOL last), shared across tokens; the generation-token length
  boundary (64 bytes is a reference, 65 is not, with the exact error); a 100 001-byte integer,
  real or keyword after `1 1` lexes in one pass plus at most 68 bytes; 100 000-byte whitespace
  and comment runs between integers, or inside `num gen R`, lex in one pass plus 8 bytes (#113).
- `crates/jpdf-engine/tests/open.rs`: page counts for 0/1/3/50 pages, `/Prev` updates, `/Prev`
  loops, lenient `/Count`, deep nesting, overflow numbers, unsupported structures, file-size and
  page limits, objects nested in strings and kids sharing one body (overlap rejection),
  overlapping `/Prev` sections (rejected), per-document object budget, out-of-range xref offsets (read as free).
- Deterministic property test (seeded xorshift; the workspace has no admitted `proptest` yet,
  see #16): thousands of random byte strings and mutated/truncated valid files never panic.
- E2E test IDs: none.
