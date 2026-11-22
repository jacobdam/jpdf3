# Limits ceiling and retained bytes per document

Area: engine
ISO 32000-2 clauses: none (resource limits are project policy, PLAN §2.2; the nesting limit
guards the parser of §7.3.6 arrays and §7.3.7 dictionaries)
Journeys: none (no UI; protects every journey that opens a file)
Status: implemented

## Behavior
Issue #79 part 1/3 (audit 0001 items 2 and 3). Parts 2 and 3 are #106 (decoder working memory
and operations budget) and #107 (object resolve cache).

1. **`Limits::clamped_to(&self, ceiling: &Limits) -> Limits`** (jpdf-traits) returns the
   field-wise minimum of `self` and `ceiling`. Every `Limits` field is an upper bound (a
   smaller value is always stricter, `lex_budget_factor` and `lex_budget_slack` included), so
   the result is never looser than either side. The function destructures `Limits`
   exhaustively, so a new field doesn't compile until it is listed there; the tests'
   `all_fields` literal must list it too, and `clamping_works_per_field_one_past_the_ceiling`
   fails if it isn't clamped.
2. **`Document::open(bytes, &Limits)` clamps first.** Its first statement replaces the
   caller's limits with `limits.clamped_to(&Limits::default())`, and only the clamped value
   reaches the size check, the xref parser and the page tree. Front ends (CLI, Swift and JS
   bindings) can tighten limits but never loosen them. Before this, `max_nesting_depth =
   10_000_000` from a binding let a file with 100,000 nested arrays overflow the recursive
   parser's stack (CONSTITUTION 5: values from Swift/JS are hostile).
   - `jpdf_syntax::XrefDocument::parse` and `jpdf_cos` functions still take `&Limits` as
     given. They are internal layers below the engine facade; bindings reach only
     `jpdf-engine` (PLAN §2.3).
3. **New field `max_retained_bytes_per_document: u64`, default 256 MiB.** It bounds what one
   document keeps alive between calls: cached decoded streams, font programs held by cached
   faces (`FontFace` borrows its program), and parsed objects. The rule
   for every cache the engine adds: charge the bytes it holds, and evict or recompute instead
   of going over. The first cache to charge it is the object resolve cache
   (specs/core/resolve-cache.md, #107).
   - `max_decoded_bytes_per_document` (4 GiB) stays. It is a lifetime total (time bound against
     many small bombs), not a memory bound; its doc comment now says so and points here.
   - Budgeted terms: the full process budget, with every uncharged term and a 256 MiB
     reserve, is specs/core/memory-budget.md (`jpdf_syntax::MemoryBudget`, #109). #109 lowered
     `max_xref_entries` to 2^22, `max_raster_working_bytes` to 512 MiB and
     `max_canvas_pixels` to 2^23 so the defaults fit with about 66 MiB to spare (#109 also lowered `max_objects_per_parse` to 2^20; the phase model assumes heap reuse across phases, #146).
   - Charged by the object resolve cache only (specs/core/resolve-cache.md). Docs say the
     future face cache "must" charge it, through one counter shared with the object cache
     (#138).

## Persistence
Nothing is persisted. Limits are per process call.

## Undo
Not applicable.

## Errors
No new error. A caller value above the default behaves as the default, so exceeding it gives
the existing `EngineError::LimitExceeded(<which>)`; e.g. 256 nested arrays inside the catalog
give `LimitExceeded("nesting depth")` even with `max_nesting_depth = 10_000_000`.

## Limits
- Ceiling = `Limits::default()` for every field. Boundary tests:
  - unit, per field: with a ceiling of 5 in every field, 6 comes back as 5, 5 and 4 are kept
    (`clamping_works_per_field_one_past_the_ceiling`); every field at its type's maximum comes
    back as the default; every field at 0 or 1 is kept; against a ceiling whose 25 fields are
    all distinct (1..=25), loose values come back as exactly that ceiling
    (`clamping_uses_each_field_of_the_ceiling`).
  - engine, end to end: with `max_nesting_depth = 10^7` (and loose object and lexing limits),
    a catalog with 255 nested arrays (depth 256, the default) opens and 256 arrays are refused;
    100,000 arrays are refused instead of aborting
    (`caller_limits_cannot_loosen_the_defaults`). A tighter depth of 11 still binds at
    exactly 10/11 arrays (`caller_limits_can_tighten_the_defaults`). The opened document
    reports the defaults as its limits.
  - engine, a second field: with `max_xref_sections = usize::MAX`, a `/Prev` chain of 1024
    sections (the default) opens and 1025 are refused
    (`caller_xref_section_limit_is_clamped`). This fails an `open` that clamps only the depth
    (checked by hand).
- `max_retained_bytes_per_document`: the at-limit / one-past tests are in
  specs/core/resolve-cache.md (`retained_budget_boundary_for_one_object`,
  `retained_budget_adds_up_across_objects`).
- `charged_budgets_do_not_exceed_the_process_budget` checks the four budgeted terms sum to at
  most 2 GiB; the full budget with the uncharged terms is checked in jpdf-syntax
  (specs/core/memory-budget.md).

## Security
- Trust boundary: the clamp sits in `Document::open`, today the only engine entry point, so a
  hostile or buggy binding value cannot raise any limit through it (CONSTITUTION 4, 5).
  `Document` keeps the clamped limits (`Document::limits()`), so later operations use them
  instead of taking a fresh `&Limits`. That bindings call only `jpdf-engine` is a convention
  (ADR 0008); `bindings/` doesn't exist yet, and a dependency check belongs with it.
- Self-check:
  - Arithmetic: `clamped_to` only takes minimums (no arithmetic). The budget test uses
    saturating sums on constants.
  - What broken implementation still passes? A clamp that skips one field, uses `max`, clamps
    a field to a different ceiling field, or always returns the ceiling fails a unit test,
    because they compare whole structs, and one ceiling has a distinct value in every field. Not clamping
    in `Document::open` fails the engine test by stack overflow (verified by removing the
    call). Clamping **after** the parse would also fail it. Still passing: clamping in
    `Document::open` but passing the unclamped `limits` to one later call; today
    `Document::open` uses a shadowed binding, so the unclamped value is no longer in scope.
  - What the tests can't see: stack use for depths below 256 on small thread stacks (wasm,
    XPC threads). 256 levels is the existing default and is exercised by the test on a
    default 2 MiB test thread.

## Accessibility
Not applicable (no UI).

## Platforms
Same on every platform. The macOS and web bindings will call `Document::open` and inherit the
clamp; they must not call `jpdf-syntax` directly.

## Performance
One struct copy per `Document::open`; no measurable cost.

## Tests
- `crates/jpdf-traits/src/limits.rs`: `clamping_lowers_every_looser_field_to_the_ceiling`,
  `clamping_keeps_every_tighter_field`, `clamping_works_per_field_one_past_the_ceiling`,
  `clamping_uses_each_field_of_the_ceiling`,
  `charged_budgets_do_not_exceed_the_process_budget`.
- `crates/jpdf-engine/tests/open.rs`: `caller_limits_cannot_loosen_the_defaults`,
  `caller_limits_can_tighten_the_defaults`, `caller_xref_section_limit_is_clamped`.
- No E2E IDs (no UI).
