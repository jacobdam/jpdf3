# Object resolve cache

Area: syntax
ISO 32000-2 clauses: §7.3.10 (indirect objects and references), §7.5.4 (cross-reference table)
Journeys: none (no UI; protects every journey that opens a file with shared objects)
Status: implemented

## Behavior
Issue #107, part 3/3 of #79 (audit 0001 item 4). Before this, every `XrefDocument::resolve`
re-parsed the object and charged its bytes to the per-document lexing budget (`4 × file +
1 MiB`). A legitimate file whose pages share one large resource dictionary would run out of
budget once rendering resolves resources per page. This adds an object cache before those
resolve callers exist.

1. **`XrefDocument::resolve(r) -> Result<Arc<Object>, SyntaxError>`** (was `Object`). The
   first successful resolve of `r` parses the object as before (overlap check, header check,
   lexing budget). If it fits (item 2), the parsed value is stored in a per-document map keyed
   by the full `ObjRef` (number and generation). Later resolves of `r` return the same `Arc`
   without lexing a byte.
   - `Arc`, not a clone: a deep clone costs as much as the parse, so a cache that cloned would
     turn many references to one large object back into quadratic work. `Arc` (rather than
     `Rc`) keeps `XrefDocument` `Send`.
   - Free, missing and generation-mismatched references (ISO 32000-2 §7.3.10: null) return a
     fresh `Arc::new(Object::Null)` and are not cached: they cost no parsing.
   - A failed resolve (malformed header, overlap, limit) caches nothing, so it fails again the
     same way.
2. **Charged to `Limits::max_retained_bytes_per_document`.** Each cached object costs
   `retained_cost(o)` bytes:
   - `CACHE_ENTRY_OVERHEAD` = 64 bytes (the `Arc` counters, the map's key and pointer, doubled
     for B-tree node slack);
   - plus `size_of::<Object>()` for the root node inside the `Arc`;
   - plus every heap allocation below it, **by capacity**: string and name buffers, array
     buffers (`capacity × size_of::<Object>()`), dictionary buffers (`capacity ×
     size_of::<(Vec<u8>, Object)>()`) and their keys.
   An object is cached only if the running total plus its cost stays `<=` the limit.
   Otherwise it isn't cached; there is **no eviction**, so the cache contents depend only on
   resolve order. An uncached object is re-parsed on every resolve and charged to the
   lexing budget exactly as before (the limits-ceiling rule: evict or recompute, never exceed).
   Later, smaller objects can still fit.
3. **`XrefDocument::retained_bytes() -> u64`** reports the running total.
4. `jpdf_cos::flat_page_count` takes the `Arc` (deref); its "one object alive at a time"
   comment now says "beyond the resolve cache".

The counter lives in `XrefDocument`, the only cache today. The next cache that charges this
field (decoded streams, font faces) must share one per-document counter with it, so the sum
stays under the field. That is tracked in #138.

## Persistence
Nothing is persisted. The cache lives as long as the `XrefDocument` (today: inside
`Document::open`).

## Undo
Not applicable.

## Errors
No new error. A full cache is not an error: the object is re-parsed, and a file that resolves
an uncached object too often still fails with `LimitExceeded("bytes lexed per document")`.

## Limits
- `max_retained_bytes_per_document` (default 256 MiB) bounds what the cache holds, by the cost
  in item 2. Boundary tests:
  - `retained_budget_boundary_for_one_object`: with the limit at exactly the object's cost,
    the second resolve returns the same `Arc` and lexes nothing; at cost − 1 it returns a new
    equal value, the lexing counter grows, and `retained_bytes()` stays 0.
  - `retained_budget_adds_up_across_objects`: at `cost(1) + cost(2)` both are cached; at one
    less only object 1 is, and a smaller object 3 still fits afterwards (exact totals).
- Lexing budget (`lex_budget_factor`, `lex_budget_slack`): unchanged and still charged on every
  parse. `shared_object_resolved_many_times_fits_the_default_budget`: a 2 MiB object resolved 11
  times by `XrefDocument::resolve` stays under the default limits (it parses once); its
  control, with a retained limit of 0, succeeds four times and fails the fifth resolve with
  `bytes lexed per document`, proving the test reaches the re-parse path. There is no engine
  test: today `flat_page_count` resolves each object once.
- Overlap checks (`objects_nested_in_strings_are_rejected_as_overlapping`,
  `overlapping_objects_sharing_one_body_are_rejected` in `crates/jpdf-engine/tests/open.rs`)
  still fail as before: a cache hit needs a successful first resolve, and an overlapping object
  never resolves.
- Worst case: memory grows by at most the retained limit; time per cache hit is one B-tree
  lookup (O(log n), n ≤ cached objects) and an `Arc` increment.
- `retained_cost` walks the object once per successful parse (linear in what the parse already
  built and charged) and recurses to the object's nesting depth, which the parser bounded by
  `max_nesting_depth` with larger frames.

## Security
- Trust boundary unchanged: objects are still parsed from hostile bytes by the same bounded
  parser; the cache only stores results.
- A hostile file can't use the cache to exceed memory: every insert is charged and checked
  first. It can't use it to skip a check: the cache is keyed by the full reference, filled
  only after the header, overlap and limit checks pass, and the xref entries never change after
  `parse`.
- Self-check:
  - Arithmetic: `retained_cost` uses `saturating_add`/`saturating_mul` on `u64` (sizes converted
    with `u64::try_from`, saturating to `u64::MAX`); the total is `saturating_add` then compared
    with `<=`. A saturated cost is over any limit, so it is never cached.
  - What broken implementation still passes? A cache that clones instead of sharing fails the
    `Arc::ptr_eq` asserts. An off-by-one (`<` instead of `<=`) or a check that ignores the
    running total fails the boundary tests. A cost formula that drops a term (capacity vs
    length, dictionary entry size, key bytes, root node, entry overhead) fails
    `retained_cost_counts_every_allocation`, which pins exact values. Caching nulls or errors
    fails `null_references_and_errors_are_not_cached`; keying by number only fails its
    generation case. Caching before the end-of-object overlap check fails
    `object_rejected_for_overlap_is_not_cached`. Charging lengths instead of capacities fails
    the spare-capacity cases (string, name, key, array and dictionary buffers). Switching the
    cache to `Rc` fails to compile `xref_document_is_send`. Still passing: an overhead constant
    that undercounts the allocator's real per-entry cost (the 64 bytes are an estimate, not
    measured).
  - What the tests can't see: real allocator overhead and fragmentation. The cost counts
    capacities, not allocator headers, so actual RSS can exceed `retained_bytes()` by a small
    factor for many tiny objects. The 256 MiB reserve in specs/core/memory-budget.md (#109) covers
    that overhead.

## Accessibility
Not applicable (no UI).

## Platforms
Same on every platform.

## Performance
A cache hit replaces a parse with a map lookup. A miss adds one walk over the parsed object
(`retained_cost`). Measured by the existing open tests; no bench change.

## Tests
- `crates/jpdf-syntax/src/xref.rs`: `shared_object_resolved_many_times_fits_the_default_budget`,
  `retained_budget_boundary_for_one_object`, `retained_budget_adds_up_across_objects`,
  `null_references_and_errors_are_not_cached`, `object_rejected_for_overlap_is_not_cached`,
  `retained_cost_counts_every_allocation`, `xref_document_is_send` (compile-time).
- `crates/jpdf-engine/tests/open.rs`: the existing overlap, quadratic-reparse and budget tests
  pass unchanged.
- No E2E IDs (no UI).
