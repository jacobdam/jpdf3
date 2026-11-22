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

This adds `jpdf_syntax::MemoryBudget` (crates/jpdf-syntax/src/budget.rs). It **estimates** the
peak memory one document may reach under a set of `Limits`. The default limits must fit in
`PROCESS_BUDGET_BYTES` (2 GiB) by this estimate and leave room for the reserve. It is not a
proven bound: heap holes left by several phases are not counted (known gap below, #146). It lives in jpdf-syntax because
that crate owns the types whose sizes the formula multiplies (the xref `Entry` and
`Object`). Today only its tests use it. The resource-limit harness (#25) is the intended
consumer.

**Model.** Peak = persistent terms + the heap high-water mark an earlier phase left mapped
(the largest phase, #146) + the largest phase + `RESERVE_BYTES` (256 MiB).

| Term | Kind | Formula | Default |
|---|---|---|---|
| file | persistent | `max_file_size` (the CLI reserves the file size + 1 byte; see #141) | 512 MiB |
| retained | persistent | `max_retained_bytes_per_document`, charged as allocator chunks | 128 MiB |
| xref table | persistent | `max_xref_entries × size_of::<Entry>() (12) × 3` | 72 MiB |
| spans | persistent | `(max_xref_entries + max_xref_sections) × SPAN_ENTRY_BYTES (64)` | 128.06 MiB |
| page walk | persistent | `max_pages × PAGE_WALK_BYTES (48)` | 45.78 MiB |
| parse | phase | `max_objects_per_parse × PARSE_OBJECT_BYTES (272) + max_string_bytes_per_parse × 3` | 368 MiB |
| decode | phase | `max_decoded_bytes_per_stream + max_decode_working_bytes` | 384 MiB |
| raster | phase | `max_raster_working_bytes` | 256 MiB |
| heap high-water | earlier phase | the largest phase (`heap_high_water`) | 384 MiB |
| reserve | fixed | itemized below | 256 MiB |

Peak at the defaults: 885.84 + 384 + 384 + 256 = 1909.84 MiB, which leaves about 138 MiB of
the 2 GiB unused.

- **Reserve, itemized** (`reserve_is_itemized`): binary and fixed mappings 64 MiB; thread
  stacks 32 MiB (main 8 MiB plus the engine thread, #114); one glibc thread arena 64 MiB
  (glibc reserves a 64 MiB heap region for each thread other than the main one, and
  `RLIMIT_AS` counts it); fragmentation and small bookkeeping 96 MiB. Per-allocation chunk
  overhead is **not** in the reserve: retained charges chunks, parse uses a per-object worst
  case, and the other terms are large buffers. All three non-binary items (stacks, arena,
  fragmentation) are placeholders until the resource-limit harness (#25) measures them. One
  fragmentation case on today's path: cached objects sit in the brk heap between buffers the
  same parse freed as its containers, strings and names grew, below later cached chunks
  (memory freed by uncached parses can end up there too). Later sub-threshold allocations
  reuse a hole only if they fit (32-byte key holes can't serve a 240-byte buffer), and
  buffers above the mmap threshold (128 KiB at first, at most 32 MiB once raised: arrays over
  4 096 slots, dictionaries over about 2 340 entries, long strings) get new mappings and never
  reuse holes. So holes and the mmapped part of a parse can add up. Since #146 that case is a
  term of its own: holes one earlier phase leaves are within `heap_high_water`, and the
  mmapped part is within the current phase. The fragmentation item covers up to 96 MiB of
  holes that later requests can't reuse because of their size. Build-up across several phases
  is not bounded (known gap in Behavior, #146). The item is **unverified** until #25 measures
  it with a cache filled by growing containers. glibc maps 2 × 64 MiB to align a new
  thread arena before trimming it, and a busy thread arena grows in 64 MiB sub-heaps that can
  strand unused tails. So #114 must either keep large allocations off the engine thread's
  arena (glibc serves requests above the mmap threshold, at most 32 MiB, with their own
  mappings) or limit arenas (`M_ARENA_MAX`), and #25 must confirm the item.
- **Retained is real memory.** The resolve cache charges every allocation as a glibc chunk:
  an 8-byte header, rounded up to 16, at least 32 bytes (`alloc_bytes`,
  specs/core/resolve-cache.md). Before #109 a dictionary of one-byte entries was charged
  72 bytes per entry while glibc held 120. Allocations at or above the mmap threshold
  (128 KiB by default) are page-rounded instead: up to 4 080 bytes more each, at most about
  4 MiB over the 128 MiB budget, inside the fragmentation reserve.
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
  the parse phase, so it is persistent. The walk drops its handle to the `/Pages` node once
  the kid references are extracted; a `/Pages` node that fits the resolve cache stays alive
  there and is charged to `retained`.
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
  tile, never two at once. A finished phase does not always give its address space back, so
  the largest phase counts twice (#146):
  - glibc serves requests below its mmap threshold from the brk heap. The threshold starts at
    128 KiB and rises (to at most 32 MiB) each time a larger mapped chunk is freed, so after a
    big buffer is freed nearly every allocation of a phase can land in the heap. The heap only
    shrinks from the top: one allocation made during the phase that stays alive (a span, a
    cache node, the xref table growing) keeps everything below it mapped.
  - Freed heap memory is reused by later requests that fit, but a request above the threshold
    gets a new mapping. So in the worst case one phase leaves its whole memory in the heap
    (e.g. a parse of nested one-entry dictionaries: all small chunks) and the next phase gets
    all of its memory as new mappings (e.g. a decode with one large output buffer).
  - Both are bounded by the largest phase, so `heap_high_water` = the largest phase. This
    covers **one** earlier phase's heap left mapped.
  - **Known gap, not bounded (#146 stays open):** leftovers from several phases can build up.
    Long-lived chunks (cache nodes, up to `retained`; span nodes; the growing xref table) sit
    between the buffers each parse freed. A later phase whose sub-threshold requests are larger
    than those holes gets new heap above them, and the next long-lived chunk pins that in turn.
    Each phase stays within its own limit, but the holes no later request fits can add up past
    `heap_high_water` plus the 96 MiB fragmentation item: in theory up to one phase's worth for
    each size class a hostile file steps through. This was already possible before #146 (parse
    after parse), and it can't be bounded by a formula over the limits. Closing it needs
    either a measurement showing it doesn't happen at a meaningful scale (#25: many cached
    growing containers, then phases making ever larger sub-threshold requests) or a structural
    fix that keeps every long-lived allocation out of the phases' heap region: the index
    storage (span map, xref table, cache map) **and** the payloads (the cached objects' `Arc`s
    and their buffers, which a parse allocates in the middle of its own scratch memory). Until then the peak is an
    estimate that accounts for one earlier phase, not a proven bound, and a peak under 2 GiB
    does not certify that a run fits.
  - **Gate:** no decode or raster phase runs on the jpdf-cli path until one of these holds
    (#146): (a) #25 runs the cross-phase pattern above through the jpdf-cli path being
    enabled, exercising **every** phase that path runs, each one after the heap the preceding
    phases left (e.g. parse, then decode, then raster, if raster is enabled), at the default
    limits under `RLIMIT_AS` = 2 GiB as the judge runs `jpdf-cli`, and the process neither
    crashes nor fails an allocation, and its measured peak address space stays at or below
    `peak()`; or (b) the structural fix above lands, covering both index storage and payloads.
    The run must also show that each phase reached its stress allocations in that order: it
    records the allocation sizes each phase reached (e.g. the decoder's working buffers at
    `max_decode_working_bytes`), and a phase that ended early (a limit rejection before its
    buffers, a fallback, a skipped phase) is not evidence. A measurement that finds the process
    over the limit doesn't open the gate. A passing run validates only the workloads and phases
    it ran: enabling another phase on the jpdf-cli path needs a new run that includes it. Today
    parse is the only phase on that path; parse after parse can build up holes the same way
    (already true before #146), which the same measurement covers.
  - **Tripwire:** tools/jpdf-cli/tests/phase_gate.rs fails, naming #146, as soon as jpdf-cli's
    normal or build dependency graph (`cargo tree`) holds any crate beyond the parse-only set
    (jpdf-cli, jpdf-engine, jpdf-cos, jpdf-syntax, jpdf-traits). A PR that brings a filter,
    backend or render crate onto the path must satisfy the gate and update that allowlist.
  - The same reasoning applies to whichever glibc arena the phases run in. A thread other than
    the main one (the engine thread, #114) allocates from a non-main arena made of 64 MiB
    sub-heaps, which are also trimmed only from the top.
  - Before #146 the model counted the largest phase once: at those defaults the parse heap
    (464 MiB) next to a decode (512 MiB) came to 2445.84 MiB, over the 2 GiB. Parse was the only
    phase on the jpdf-cli path, so it could not happen yet.
  What a phase produces and is still alive in a later phase must be charged to `retained`
  (see the requirements). The model
  is per process with one document and one job at a time. That is what the judge runs. A
  front end that renders several pages or documents in parallel must divide the budgets among
  them (future work in `jpdf-render` and the bindings).

**New defaults** (lowered; CONSTITUTION 4 forbids only raising). #109 lowered the first four;
#146 lowered three of them again and three more fields, so the defaults fit the estimate with
the largest phase counted twice:
- `max_objects_per_parse` 2 000 000 → 1 048 576 (2^20, #109). At 272 bytes per object, 2 M objects
  alone were 519 MiB. 2^20 still holds a flat `/Kids` array of `max_pages` (1 000 000)
  references.
- `max_xref_entries` 10 000 000 → 4 194 304 (2^22, #109) → 2 097 152 (2^21, #146). The
  original value alone cost 343 MiB for the table and 610 MiB for the spans; 2^21 costs 72 MiB
  and 128 MiB. 2M objects is still far above real documents: a 512 MiB file with that many
  objects averages 256 bytes per object.
- `max_raster_working_bytes` 1 GiB → 512 MiB (#109) → 256 MiB (#146).
- `max_canvas_pixels` 2^24 → 2^23 (#109) → 2^22 (#146; e.g. a 2048 × 2048 tile). A full tile
  with 4 soft-masked layers, 32 clips and one canvas-sized image still fits. `check_scene`'s
  formula gives 2^22 × (4 + 4 × 5 + 32) + 4 × 2^22 = 240 MiB ≤ 256 MiB
  (`default_raster_limits_fit_the_memory_budget`). `jpdf-render` renders larger views in tiles.
- `max_retained_bytes_per_document` 256 MiB → 128 MiB (#146). Objects that don't fit the
  resolve cache are parsed again on every resolve. That costs time, and the re-parses are
  charged to the document's lexing budget, so a file that resolves large uncached objects
  many times hits `LimitExceeded` from the lexing budget sooner than before (e.g. five objects
  with 30 MiB strings each: a pushed string ends with a 32 MiB capacity, which the cache
  charges, so all five fit the old cache but only three fit the new one).
- `max_string_bytes_per_parse` 64 MiB → 32 MiB (#146). Large data lives in streams, which this
  limit doesn't cover; 32 MiB of strings in one object (signature contents are tens of KiB) is
  far above real files.
- `max_decoded_bytes_per_stream` 256 MiB → 128 MiB (#146). One stream's output: a content
  stream, or an image's output (128 MiB holds a 6 500 × 6 500 8-bit RGB image, or an A4 page
  scanned at 600 dpi in RGB). The image decoder's own memory, `max_decode_working_bytes`, stays
  at 256 MiB, but an image whose output passes 128 MiB (e.g. 7 000 × 7 000 RGB, 147 MB) is now
  refused even when its working memory would fit.

The levers were chosen to keep the main user-visible ceilings (file size, pages, image decode
working memory) and give up mostly speed (retained), the largest images (output over
128 MiB), and what real files don't reach (xref entries, string bytes per object).

**Requirements on future code** (the model holds only if they do):
- **Phase outputs that outlive their phase are charged to `retained`, or rejected.**
  `XrefDocument::resolve` returns an uncached `Arc` when the cache is full. A caller that keeps
  it into another phase (a stream dictionary held while its stream decodes, a decoded content
  stream held while it is interpreted and rendered) must charge it to `retained` (one counter,
  #138) or drop it first. The current page walk drops every handle before the next resolve.
- `jpdf-filters`: an output buffer never reallocates past the output cap. It reserves the cap
  (or a known length) up front or writes in fixed chunks, so it never holds 1.5 × the cap
  during a realloc. A stage drops its input before the next stage grows its output. The decode
  term counts one 128 MiB intermediate output plus the image decoder's 256 MiB working bytes.
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
No new error. A file with more than 2 097 152 xref entries now fails with
`LimitExceeded("cross-reference entries")` (CLI exit 2). An object with more than 32 MiB of
decoded string and name bytes fails with `LimitExceeded("string bytes in one object")` (exit 2).
A render target larger than 2^22 pixels, or over 256 MiB of working bytes, and a stream whose
output passes 128 MiB, fail with `BackendError::LimitExceeded`. Objects are charged more to the
resolve cache than before (allocator chunks) and the cache is half as large (#146), so fewer
stay cached; past the budget they are re-parsed, with no error until the lexing budget runs
out.

## Limits
- Exact-boundary tests:
  - `default_entry_limit_is_exact` (xref.rs): with the default limits, a subsection count of
    2 097 152 passes the count check and fails on the missing first entry (`Malformed` at byte
    14). 2 097 153 is refused with the limit error before any entry is read. Across two
    subsections, 1 + 2 097 151 passes the count check and 1 + 2 097 152 is refused.
  - `default_canvas_limit_is_exact` (raster.rs): `check_scene` with `RasterLimits::default()`
    passes 2^22 pixels (2048 × 2048 and 2^22 × 1) and refuses 2^22 + 1 (and 2049 × 2048).
  - `default_working_bytes_limit_is_exact` (raster.rs): a 2048 × 2048 canvas with 15 layers
    is exactly 256 MiB and passes; one more clip is refused.
  - `default_string_bytes_limit_is_exact` (xref.rs): with the default limits, an array of two
    16 MiB strings (one literal, one hex) parses; with one more hex byte it is refused with
    `LimitExceeded("string bytes in one object")`. Two strings, so a per-token check fails it.
  - `max_retained_bytes_per_document` and `max_decoded_bytes_per_stream`: their defaults are
    pinned as literals in `lowered_defaults_are_pinned` (limits.rs); `default_terms` pins
    `retained` and the decode sum. Neither has an exact-boundary test **at its default
    value**: the at-limit and one-past behavior is tested with explicit limits
    (`retained_budget_boundary_for_one_object`, the decode suites' output-cap checks), and
    the check reads the field the same way whatever its value.
  - `page_limit_boundary_opens_at_the_limit` (crates/jpdf-engine/tests/open.rs): 5 kids open
    with `max_pages` 5; `page_limit_is_enforced` refuses 5 with 4. The parameterized
    boundary tests in the raster suite are unchanged.
  - `default_objects_per_parse_limit_is_exact` (xref.rs): with the default limits, an array
    plus 2^20 - 1 integers (2^20 objects) parses and one integer more is refused.
    `flat_page_tree_at_max_pages_fits_one_parse` (xref.rs) parses a `/Pages` dictionary with
    1 000 000 references under the defaults, so the lowered limit still reaches `max_pages`.
  - The retained-bytes boundary tests (`retained_budget_boundary_for_one_object`, at the cost
    and one byte less) compute the cost with `retained_cost`, so they cover the chunk model.
  - `defaults_fit_the_process_budget_with_the_reserve`: peak + 64 MiB ≤ 2 GiB at the
    defaults (the peak is pinned as a literal in `default_terms`). The 64 MiB margin means a small increase to one term fails the test before it
    eats into the reserve.
- `MemoryBudget::of` and `alloc_bytes` use saturating arithmetic (`u64`), so loose caller limits
  (e.g. `usize::MAX`) give `u64::MAX` (`alloc_bytes` rounds the saturated value down to
  0xFFFF_FFFF_FFFF_FFF0; unreachable, since a capacity never exceeds `isize::MAX`), never an
  overflow
  (`loose_limits_saturate_every_term` calls `of` with every contributing field at its type
  maximum all at once, and six of them one at a time; `peak_is_persistent_plus_largest_phase_twice_plus_reserve`
  saturates `peak`, with a persistent term and with a phase term at the maximum (the heap term
  adds that phase twice); `alloc_bytes_follow_glibc_chunks` saturates `alloc_bytes`).

## Security
- Threat: a hostile file that drives every limit to its maximum at once must not exhaust the
  address space (a crash). The model is an **estimate**: a sum of upper bounds for each term
  it lists and for one earlier phase's heap, but it does not count heap holes several phases
  leave (known gap in Behavior, #146), so `MemoryBudget::peak()` under 2 GiB is not a safety
  check. It is not a measurement either.
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
    dictionary slot. `peak_is_persistent_plus_largest_phase_twice_plus_reserve` uses distinct
    powers of two, so a dropped or double-counted term and a wrong phase choice both show up.
    It checks the peak with each phase as the largest, so a heap term tied to one phase (e.g.
    always `parse`) fails the cases where another phase is largest.
    Still passing: a formula that reads an extra field only through a `max` that doesn't bind
    at the defaults (e.g. `raster.max(max_canvas_pixels)`); lowering that field by one changes
    nothing (checked by hand mutation). `reserve_is_itemized` pins each reserve item.
    `alloc_bytes_follow_glibc_chunks` pins the
    minimum, header and rounding at their boundaries (24 → 32, 25 → 48). A budget that is
    wrong because the *model* misses a structure can't be caught by these tests. The
    "requirements on future code" list and the review of each new structure cover that.
  - What the tests can't see: real allocation sizes (no counting allocator; a global
    allocator needs code CONSTITUTION 3 forbids). The resource-limit harness (#25) is meant to
    measure the real peak under `RLIMIT_AS` in the nightly. Until it exists, the spans,
    page-walk and growth factors are reasoned from std's B-tree, hash set and `Vec` layouts,
    not measured.
- The heap term (#146) is reasoned from glibc's documented behavior (brk heap trimmed only from
  the top, the dynamic mmap threshold), not measured. The resource-limit harness (#25) should
  measure, under `RLIMIT_AS`: a parse of nested one-entry dictionaries pinned by a cached
  object, followed by a maximal decode; and many cached growing containers followed by phases
  making ever larger sub-threshold requests (the cross-phase build-up in Behavior, which this
  model does **not** bound; #146 stays open for it).
- `max_string_bytes_per_parse` is exact for every token kind since #113 (merged into this
  branch), which the parse term's `× 3` assumes.
- Known gap: `read_capped` in jpdf-cli reserves the size from `stat`. A file that grows
  during the read (below `max_file_size`) makes `read_to_end` double the buffer, up to 3 ×
  `max_file_size` at the realloc peak. It needs a local attacker who can write to the file
  while we read it. Tracked in #141.

## Accessibility
Not applicable.

## Platforms
`type_sizes_are_pinned`, `default_terms`, `each_term_follows_its_fields`,
`loose_limits_saturate_every_term`, `parse_object_bytes_bound_real_objects` and
`retained_cost_counts_every_allocation` run on 64-bit targets only. On wasm32 the pointer and
`Vec` sizes are smaller, so every term is smaller and the budget still fits.
`defaults_fit_the_process_budget_with_the_reserve` runs on every target. The chunk model is
glibc's; macOS and wasm allocators differ, but neither runs under the judge's limit.

## Performance
No runtime cost for `MemoryBudget` (tests only). `alloc_bytes` adds a few integer operations
per allocation counted by `retained_cost`, which already walks the object once.

## Tests
- tools/jpdf-cli/tests/phase_gate.rs: `jpdf_cli_links_only_parse_crates_until_the_146_gate_opens`
  (new, #146; the gate's tripwire).
- crates/jpdf-syntax/src/budget.rs: `type_sizes_are_pinned`, `reserve_is_itemized`,
  `default_terms`, `defaults_fit_the_process_budget_with_the_reserve`,
  `each_term_follows_its_fields`, `loose_limits_saturate_every_term`,
  `peak_is_persistent_plus_largest_phase_twice_plus_reserve`.
- crates/jpdf-syntax/src/xref.rs: `default_entry_limit_is_exact`, `default_string_bytes_limit_is_exact`,
  `default_objects_per_parse_limit_is_exact`, `flat_page_tree_at_max_pages_fits_one_parse`,
  `parse_object_bytes_bound_real_objects`, `trailer_with_duplicate_keys_is_trimmed_to_two_entries`,
  `alloc_bytes_follow_glibc_chunks`, `retained_cost_counts_every_allocation` (new literals).
- crates/jpdf-traits/src/raster.rs: `default_canvas_limit_is_exact` (2^22 pixels pass with
  the default limits, 2^22 + 1 fail), `default_working_bytes_limit_is_exact`.
- crates/jpdf-engine/tests/open.rs: `page_limit_boundary_opens_at_the_limit`.
- crates/jpdf-traits/src/limits.rs: `lowered_defaults_are_pinned` (new, #146); `default_raster_limits_fit_the_memory_budget` now uses
  `check_scene`'s full formula and pins 256 MiB working bytes and 2^22 canvas pixels;
  `per_parse_objects_hold_a_flat_page_tree` (a flat `/Kids` of `max_pages` references plus
  32 768 other objects fit in one parse).
