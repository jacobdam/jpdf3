# Process memory budget

Area: engine
ISO 32000-2 clauses: §7.5.4 (cross-reference table), §7.3.10 (indirect objects), §7.7.3.2 (page tree)
Journeys: none (no UI; protects every journey that opens a large or hostile file)
Status: implemented

## Behavior
Issue #109. The judge runs `jpdf-cli` under a 2 GiB address-space limit (ops/judge/CONTRACT.md);
running out is a crash. Before this, the four charged budgets (file 512 MiB + retained 256 MiB
+ decode 256 MiB + raster 1 GiB) summed to exactly 2 GiB. Nothing was left for the binary,
the stacks, the xref table or one parse's objects. The decoder's input was not counted either.

This adds `jpdf_syntax::MemoryBudget` (crates/jpdf-syntax/src/budget.rs). It is the peak
memory one document may reach under a set of `Limits`. The default limits must fit in
`PROCESS_BUDGET_BYTES` (2 GiB) and leave room for the reserve. It lives in jpdf-syntax because
that crate owns the types whose sizes the formula multiplies (the xref `Entry` and
`Object`). Today only its tests use it. The resource-limit harness (#25) is the intended
consumer.

**Model.** Peak = persistent terms + the largest phase + `RESERVE_BYTES` (256 MiB).

| Term | Kind | Formula | Default |
|---|---|---|---|
| file | persistent | `max_file_size` (the CLI reserves exactly the file size; see #141) | 512 MiB |
| retained | persistent | `max_retained_bytes_per_document`, charged as allocator chunks | 256 MiB |
| xref table | persistent | `max_xref_entries × size_of::<Entry>() (12) × 3` | 144 MiB |
| spans | persistent | `(max_xref_entries + max_xref_sections) × SPAN_ENTRY_BYTES (64)` | 256.06 MiB |
| page walk | persistent | `max_pages × PAGE_WALK_BYTES (48)` | 45.78 MiB |
| parse | phase | `max_objects_per_parse × PARSE_OBJECT_BYTES (272) + max_string_bytes_per_parse × 3` | 464 MiB |
| decode | phase | `max_decoded_bytes_per_stream + max_decode_working_bytes` | 512 MiB |
| raster | phase | `max_raster_working_bytes` | 512 MiB |
| reserve | fixed | itemized below | 256 MiB |

Peak at the defaults: 1213.84 + 512 + 256 = 1981.84 MiB, which leaves about 66 MiB of the
2 GiB unused.

- **Reserve, itemized** (`reserve_is_itemized`): binary and fixed mappings 64 MiB; thread
  stacks 32 MiB (main 8 MiB plus the engine thread, #114); one glibc thread arena 64 MiB
  (glibc reserves a 64 MiB heap region for each thread other than the main one, and
  `RLIMIT_AS` counts it); fragmentation and small bookkeeping 96 MiB. Per-allocation chunk
  overhead is **not** in the reserve: retained charges chunks, parse uses a per-object worst
  case, and the other terms are large buffers. The stack and arena items are placeholders
  until the resource-limit harness (#25) measures them. glibc maps 2 × 64 MiB to align a new
  thread arena before trimming it, and a busy thread arena grows in 64 MiB sub-heaps that can
  strand unused tails. So #114 must either keep large allocations off the engine thread's
  arena (glibc serves requests above the mmap threshold, at most 32 MiB, with their own
  mappings) or limit arenas (`M_ARENA_MAX`), and #25 must confirm the item.
- **Retained is real memory.** The resolve cache charges every allocation as a glibc chunk:
  an 8-byte header, rounded up to 16, at least 32 bytes (`alloc_bytes`,
  specs/core/resolve-cache.md). Before #109 a dictionary of one-byte entries was charged
  72 bytes per entry while glibc held 120. Allocations at or above the mmap threshold
  (128 KiB by default) are page-rounded instead: up to 4 080 bytes more each, at most about
  8 MiB over a 256 MiB budget, inside the fragmentation reserve.
- **Growth factor 3.** A `Vec` grown by pushing holds up to twice its length. While it
  reallocates, the old buffer (one length) and the new one (two lengths) are both alive. The
  xref table's stable sort runs after the growth, and its scratch buffer is at most one length,
  so it fits in the same factor.
- **Spans.** `XrefDocument` records one byte span per xref section and per resolved object in a
  `BTreeMap<usize, usize>`. Resolve goes through the entry table, so there is at most one
  object span per entry. A std B-tree leaf is a 192-byte node holding up to 11 pairs. Nothing
  is removed, and a split leaves at least 5 pairs in each half, so every leaf holds at least
  5 whatever the insertion order. Internal nodes add a geometric sum of about 58 bytes per
  leaf. That makes about 50 bytes per span, about 54 with glibc chunk headers; 64 leaves room
  above that.
- **Page walk.** `jpdf_cos::flat_page_count` keeps the kid references (8 bytes each, collected
  by doubling: up to 24) and a duplicate-check `HashSet` with capacity for every kid (at most
  16/7 buckets per kid at 9 bytes each, about 21) while it resolves each kid. That overlaps
  the parse phase, so it is persistent. The `/Pages` node itself is dropped once the kid
  references are extracted.
- **Parse.** The parser builds every array and dictionary with `Vec::new()` + `push`, so a
  container with one entry already holds 4 slots. The worst shape is nested one-entry
  dictionaries: a 224-byte buffer (240-byte chunk) plus a 32-byte key chunk = 272 bytes for
  one counted object (`PARSE_OBJECT_BYTES`). Bigger containers cost less per entry (a 56-byte
  slot, 168 at the growth peak, plus a key chunk of at most 48). String and name bytes are
  decoded into growing buffers (3 × at the growth peak; chunks of short strings are in the
  per-object bound). `parse_object_bytes_bound_real_objects` parses five shapes and checks
  the bound, and the nested shape holds exactly 272 bytes per dictionary.
- **Trailer.** The stored trailer keeps only the first usable `/Root` and `/Size`, at most 2
  entries in an exact buffer (`trim_trailer`). Before #109 it kept every duplicate: 2 M
  `/Root 1 0 R` entries in a 22 MB trailer stayed alive for the document (about 173 MiB).
- **Phases run one at a time.** A document parses an object, decodes a stream or renders a
  tile, never two at once, so only the largest phase counts. What a phase produces and is
  still alive in a later phase must be charged to `retained` (see the requirements). The model
  is per process with one document and one job at a time. That is what the judge runs. A
  front end that renders several pages or documents in parallel must divide the budgets among
  them (future work in `jpdf-render` and the bindings).

**New defaults** (lowered; CONSTITUTION 4 forbids only raising):
- `max_objects_per_parse` 2 000 000 → 1 048 576 (2^20). At 272 bytes per object, 2 M objects
  alone were 519 MiB. 2^20 still holds a flat `/Kids` array of `max_pages` (1 000 000)
  references.
- `max_xref_entries` 10 000 000 → 4 194 304 (2^22). The old value alone cost 360 MiB for the
  table and 640 MiB for the spans. 4M objects is far above real documents: a 512 MiB file with
  that many objects averages 128 bytes per object.
- `max_raster_working_bytes` 1 GiB → 512 MiB.
- `max_canvas_pixels` 2^24 → 2^23 (e.g. a 4096 × 2048 tile). A full tile with 4 soft-masked
  layers, 32 clips and one canvas-sized image still fits. `check_scene`'s formula gives
  2^23 × (4 + 4 × 5 + 32) + 4 × 2^23 = 480 MiB ≤ 512 MiB
  (`default_raster_limits_fit_the_memory_budget`).

**Requirements on future code** (the model holds only if they do):
- **Phase outputs that outlive their phase are charged to `retained`, or rejected.**
  `XrefDocument::resolve` returns an uncached `Arc` when the cache is full. A caller that keeps
  it into another phase (a stream dictionary held while its stream decodes, a decoded content
  stream held while it is interpreted and rendered) must charge it to `retained` (one counter,
  #138) or drop it first. The current page walk drops every handle before the next resolve.
- `jpdf-filters`: an output buffer never reallocates past the output cap. It reserves the cap
  (or a known length) up front or writes in fixed chunks, so it never holds 1.5 × the cap
  during a realloc. A stage drops its input before the next stage grows its output. The decode
  term counts one 256 MiB intermediate output plus the image decoder's working bytes.
- `jpdf-render` and rasterizer adapters: geometry memory that grows with input (flattened
  edges, curve subdivision, dash output up to `max_path_segments`, the display-list commands)
  counts within `max_raster_working_bytes`. `jpdf-render` passes the adapter the working-bytes
  limit minus what its display list and geometry hold. Decoded images a display list borrows
  are charged to `retained`.
- `jpdf-render` picks tiles small enough that `check_scene` passes; it doesn't exist yet.
- Font loading (program ≤ `max_font_program_bytes` 64 MiB plus face tables) stays below the
  decode phase. A cached face charges its program to `retained`.
- Any new structure that grows with input gets its own term here, or is charged to an existing
  limit.

## Persistence
Nothing is persisted.

## Undo
Not applicable.

## Errors
No new error. A file with more than 4 194 304 xref entries now fails with
`LimitExceeded("cross-reference entries")` (CLI exit 2). A render target larger than 2^23
pixels, or over 512 MiB of working bytes, fails with `BackendError::LimitExceeded`.
Objects are charged more to the resolve cache than before (allocator chunks), so fewer stay
cached; past the budget they are re-parsed, with no error until the lexing budget runs out.

## Limits
- Exact-boundary tests:
  - `default_entry_limit_is_exact` (xref.rs): with the default limits, a subsection count of
    4 194 304 passes the count check and fails on the missing first entry (`Malformed` at byte
    14). 4 194 305 is refused with the limit error before any entry is read. Across two
    subsections, 1 + 4 194 303 passes the count check and 1 + 4 194 304 is refused.
  - `default_canvas_limit_is_exact` (raster.rs): `check_scene` with `RasterLimits::default()`
    passes 2^23 pixels (4096 × 2048 and 2^23 × 1) and refuses 2^23 + 1. The parameterized
    boundary tests in the raster suite are unchanged.
  - `max_objects_per_parse`: the existing boundary test (`object_budget`, lexer.rs) takes the
    limit as a parameter.
  - The retained-bytes boundary tests (`retained_budget_boundary_for_one_object`, at the cost
    and one byte less) compute the cost with `retained_cost`, so they cover the chunk model.
  - `defaults_fit_the_process_budget_with_the_reserve`: peak + 64 MiB ≤ 2 GiB at the
    defaults. The 64 MiB margin means a small increase to one term fails the test before it
    eats into the reserve.
- `MemoryBudget::of` and `alloc_bytes` use saturating arithmetic (`u64`), so loose caller limits
  (e.g. `usize::MAX`) give `u64::MAX`, never an overflow
  (`peak_is_persistent_plus_largest_phase_plus_reserve`, `alloc_bytes_follow_glibc_chunks`).

## Security
- Threat: a hostile file that drives every limit to its maximum at once must not exhaust the
  address space (a crash). The model is a sum of upper bounds; it is conservative, not a
  measurement.
- Self-check:
  - Arithmetic: every product and sum in `MemoryBudget::of` and `alloc_bytes` is `saturating_*`
    on `u64`. The inputs are limits and capacities, not file data, but loose caller limits could
    be huge.
  - What broken formula still passes? Each term is pinned twice. `default_terms` pins every
    term at the defaults as a literal. `each_term_follows_its_fields` lowers **every** `Limits`
    field by one, and checks that exactly the right terms move by the right amounts and that
    all other fields move nothing. Its exhaustive destructuring fails to compile when a field
    is added. A term that drops a field, reads an extra field, or uses the wrong field, factor
    or type size fails one of them. `type_sizes_are_pinned` catches a change to `Entry` or the
    dictionary slot. `peak_is_persistent_plus_largest_phase_plus_reserve` uses distinct powers
    of two, so a dropped or double-counted term and a wrong phase choice both show up.
    Still passing: a formula that reads an extra field only through a `max` that doesn't bind
    at the defaults (e.g. `raster.max(max_canvas_pixels)`); lowering that field by one changes
    nothing (checked by hand mutation). `reserve_is_itemized` pins each reserve item. `alloc_bytes_follow_glibc_chunks` pins the
    minimum, header and rounding at their boundaries (24 → 32, 25 → 48). A budget that is
    wrong because the *model* misses a structure can't be caught by these tests. The
    "requirements on future code" list and the review of each new structure cover that.
  - What the tests can't see: real allocation sizes (no counting allocator; a global
    allocator needs code CONSTITUTION 3 forbids). The resource-limit harness (#25) is meant to
    measure the real peak under `RLIMIT_AS` in the nightly. Until it exists, the spans,
    page-walk and growth factors are reasoned from std's B-tree, hash set and `Vec` layouts,
    not measured.
- Known gap: `read_capped` in jpdf-cli reserves the size from `stat`. A file that grows
  during the read (below `max_file_size`) makes `read_to_end` double the buffer, up to 3 ×
  `max_file_size` at the realloc peak. It needs a local attacker who can write to the file
  while we read it. Tracked in #141.

## Accessibility
Not applicable.

## Platforms
`type_sizes_are_pinned`, `default_terms`, `each_term_follows_its_fields` and
`retained_cost_counts_every_allocation` run on 64-bit targets only. On wasm32 the pointer and
`Vec` sizes are smaller, so every term is smaller and the budget still fits.
`defaults_fit_the_process_budget_with_the_reserve` runs on every target. The chunk model is
glibc's; macOS and wasm allocators differ, but neither runs under the judge's limit.

## Performance
No runtime cost for `MemoryBudget` (tests only). `alloc_bytes` adds a few integer operations
per allocation counted by `retained_cost`, which already walks the object once.

## Tests
- crates/jpdf-syntax/src/budget.rs: `type_sizes_are_pinned`, `reserve_is_itemized`,
  `default_terms`, `defaults_fit_the_process_budget_with_the_reserve`,
  `each_term_follows_its_fields`, `peak_is_persistent_plus_largest_phase_plus_reserve`.
- crates/jpdf-syntax/src/xref.rs: `default_entry_limit_is_exact`,
  `parse_object_bytes_bound_real_objects`, `trailer_with_duplicate_keys_is_trimmed_to_two_entries`,
  `alloc_bytes_follow_glibc_chunks`, `retained_cost_counts_every_allocation` (new literals).
- crates/jpdf-traits/src/raster.rs: `default_canvas_limit_is_exact` (2^23 pixels pass with
  the default limits, 2^23 + 1 fail).
- crates/jpdf-traits/src/limits.rs: `default_raster_limits_fit_the_memory_budget` now uses
  `check_scene`'s full formula and pins 512 MiB working bytes and 2^23 canvas pixels.
