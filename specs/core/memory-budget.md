# Process memory budget

Area: engine
ISO 32000-2 clauses: §7.5.4 (cross-reference table), §7.3.10 (indirect objects)
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
`Object`).

**Model.** Peak = persistent terms + the largest phase + `RESERVE_BYTES` (256 MiB).

| Term | Kind | Formula | Default |
|---|---|---|---|
| file | persistent | `max_file_size` (the CLI reserves exactly the file size) | 512 MiB |
| retained | persistent | `max_retained_bytes_per_document` | 256 MiB |
| xref table | persistent | `max_xref_entries × size_of::<Entry>() (12) × 3` | 144 MiB |
| spans | persistent | `(max_xref_entries + max_xref_sections) × SPAN_ENTRY_BYTES (64)` | 256.06 MiB |
| parse | phase | `(max_objects_per_parse × size_of::<(Vec<u8>, Object)>() (56) + max_string_bytes_per_parse) × 3` | 512.4 MiB |
| decode | phase | `max_decoded_bytes_per_stream + max_decode_working_bytes` | 512 MiB |
| raster | phase | `max_raster_working_bytes` | 512 MiB |
| reserve | fixed | binary, thread stacks, allocator metadata and fragmentation | 256 MiB |

Peak at the defaults: 1168.06 + 512.4 + 256 = 1936.5 MiB, which leaves about 111 MiB of the
2 GiB unused.

- **Growth factor 3.** A `Vec` grown by pushing holds up to twice its length. While it
  reallocates, the old buffer (one length) and the new one (two lengths) are both alive. The
  xref table's stable sort runs after the growth, and its scratch buffer is at most one length,
  so it fits in the same factor.
- **Spans.** `XrefDocument` records one byte span per xref section and per resolved object in a
  `BTreeMap<usize, usize>`. Resolve goes through the entry table, so there is at most one
  object span per entry. A std B-tree leaf is a 192-byte node holding up to 11 pairs. A split
  leaves at least 5 pairs per node. Mostly ascending inserts leave the left halves at 5.
  Internal nodes add up to 48 bytes per leaf. That makes 48 bytes per span; 64 allows for
  allocator rounding.
- **Parse.** Each produced object costs at most one dictionary slot (56 bytes on 64-bit,
  which is larger than an array slot of 32). String and name bytes are decoded into growing
  buffers. Parsing keeps one top-level object alive at a time; a finished object is either
  dropped or moved into the resolve cache, which charges it to `retained`.
- **Phases run one at a time.** A document parses an object, decodes a stream or renders a
  tile, never two at once, so only the largest phase counts. The model is per process with one
  document and one job at a time. That is what the judge runs. A front end that renders
  several pages or documents in parallel must divide the budgets among them (future work in
  `jpdf-render` and the bindings).

**New defaults** (lowered; CONSTITUTION 4 forbids only raising):
- `max_xref_entries` 10 000 000 → 4 194 304 (2^22). The old value alone cost 360 MiB for the
  table and 640 MiB for the spans. 4M objects is far above real documents: a 512 MiB file with
  that many objects averages 128 bytes per object.
- `max_raster_working_bytes` 1 GiB → 512 MiB.
- `max_canvas_pixels` 2^24 → 2^23 (e.g. a 4096 × 2048 tile), so a full tile with 4 layers and
  32 clips still fits: 2^23 × (4 × 5 + 32) = 416 MiB ≤ 512 MiB
  (`default_raster_limits_fit_the_memory_budget`). The engine renders larger views in tiles.

**Requirements on future code** (the model holds only if they do):
- `jpdf-filters`: a filter chain keeps at most one intermediate output alive while the next
  stage runs. The decode term counts one 256 MiB intermediate output plus the decoder.
- `jpdf-render`: decoded images a display list borrows are charged to `retained` (one counter
  with every other cache, #138). The display list's own memory (commands, paths) is charged to
  `retained` or kept within the raster phase.
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
pixels, or over 512 MiB of working bytes, fails with `BackendError::LimitExceeded`. The engine
picks smaller tiles before it gets there.

## Limits
- Exact-boundary tests:
  - `default_entry_limit_is_exact` (xref.rs): with the default limits, a subsection count of
    4 194 304 passes the count check and fails on the missing first entry (`Malformed` at byte
    14). 4 194 305 is refused with the limit error before any entry is read. Across two
    subsections, 1 + 4 194 303 passes the count check and 1 + 4 194 304 is refused.
  - The canvas and working-bytes checks (`check_scene`) keep their boundary tests in the
    raster suite. Those tests take the limit as a parameter, so the new defaults change no test.
  - `defaults_fit_the_process_budget_with_the_reserve`: peak + 64 MiB ≤ 2 GiB at the
    defaults. The 64 MiB margin means a small increase to one term fails the test before it
    eats into the reserve.
- `MemoryBudget::of` uses saturating arithmetic (`u64`), so loose caller limits (e.g.
  `usize::MAX`) give `u64::MAX`, never an overflow (`peak_is_persistent_plus_largest_phase_plus_reserve`).

## Security
- Threat: a hostile file that drives every limit to its maximum at once must not exhaust the
  address space (a crash). The model is a sum of upper bounds; it is conservative, not a
  measurement.
- Self-check:
  - Arithmetic: every product and sum in `MemoryBudget::of` is `saturating_*` on `u64`; the
    inputs are limits, not file data, but loose caller limits could be huge.
  - What broken formula still passes? Each term is pinned twice: `default_terms` pins every
    term at the defaults as a literal, and `each_term_follows_its_fields` raises each
    contributing field by one and checks that exactly the right term moves by the right amount.
    A term that drops a field, uses the wrong field, the wrong factor or the wrong type size
    fails one of them. `type_sizes_are_pinned` catches a change to `Entry` or the
    dictionary slot. `peak_is_persistent_plus_largest_phase_plus_reserve` uses distinct powers
    of two, so a dropped or double-counted term and a wrong phase choice both show up. A
    budget that is wrong because the *model* misses a structure can't be caught by these
    tests. That is what the reserve and the "requirements on future code" list are for.
  - What the tests can't see: real allocation sizes (no counting allocator; a global
    allocator needs code CONSTITUTION 3 forbids). The resource-limit harness (#25) measures the
    real peak under `RLIMIT_AS` in the nightly. Until it exists, the spans and growth factors
    are reasoned from std's B-tree layout and `Vec` doubling, not measured.
- Known gap: `read_capped` in jpdf-cli reserves the size from `stat`. A file that grows
  during the read (below `max_file_size`) makes `read_to_end` double the buffer, up to 3 ×
  `max_file_size` at the realloc peak. It needs a local attacker who can write to the file
  while we read it. Tracked in #141.

## Accessibility
Not applicable.

## Platforms
`type_sizes_are_pinned` and `default_terms` run on 64-bit targets only. On wasm32 the pointer
and `Vec` sizes are smaller, so every term is smaller and the budget still fits.
`defaults_fit_the_process_budget_with_the_reserve` runs on every target.

## Performance
No runtime cost: `MemoryBudget` is computed in tests and by tools.

## Tests
- crates/jpdf-syntax/src/budget.rs: `type_sizes_are_pinned`, `default_terms`,
  `defaults_fit_the_process_budget_with_the_reserve`, `each_term_follows_its_fields`,
  `peak_is_persistent_plus_largest_phase_plus_reserve`.
- crates/jpdf-syntax/src/xref.rs: `default_entry_limit_is_exact`.
- crates/jpdf-traits/src/limits.rs: `default_raster_limits_fit_the_memory_budget` now pins
  512 MiB working bytes and 2^23 canvas pixels.
