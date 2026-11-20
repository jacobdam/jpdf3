# 4. `Limits` and the backend error type live in jpdf-traits

Status: accepted (#14)

## Context
PLAN §2.2 says one `Limits` value is passed to every operation, and #14 requires every
backend trait method to take `Limits` (or a part of it). `Limits` was defined in
`jpdf-syntax`, but PLAN §2.3 makes `jpdf-traits` the bottom of the dependency graph
(`syntax` → `traits`), so the traits can't see a type defined in `jpdf-syntax`. CLAUDE.md
asks for `thiserror` enums per crate. `thiserror` is a proc-macro crate (`thiserror-impl`),
and admission rule 4 (AUTOMATION §6.4) forbids proc-macros without an imported audit. No
crate has been admitted yet (#16).

## Decision
1. **`Limits` moves to `jpdf-traits`** unchanged, plus the decode fields
   (`max_decoded_bytes_per_stream`, `max_decoded_bytes_per_document`, `max_image_pixels`,
   `max_image_dimension`, `max_image_components`). `jpdf-syntax` re-exports it as
   `jpdf_syntax::Limits`, and `jpdf-engine` keeps re-exporting that, so no caller changes.
2. **Backends get `DecodeLimits`**, a small `Copy` struct derived per call by
   `Limits::decode_limits(remaining_document_bytes)`, instead of all of `Limits`. A backend
   sees only the bounds it must enforce. The engine stays the only owner of document-wide
   accounting.
3. **One shared `BackendError`** for all traits: `Malformed`, `Truncated`, `LimitExceeded`,
   `Unsupported`, `InvalidArgument`. Conformance suites check the variant, and the callers
   (`jpdf-filters`, `jpdf-crypt`, …) map it into their own error enums.
4. **Error types are hand-written**: an enum, `Display`, and `std::error::Error`, the same
   pattern `jpdf-syntax` already uses. This keeps first-party crates free of dependencies
   until admission works. If `thiserror` passes admission, switching is a mechanical change
   that keeps the public API.

## Consequences
- Every later trait (#53, #54, #55) uses `BackendError` and `DecodeLimits`, adding fields to
  `Limits` and `DecodeLimits` as needed.
- `DecodeLimits` and `BackendError` are public structs and enums without
  `#[non_exhaustive]`. Adding a field or variant breaks only in-workspace code, which is
  fine while every user is in this repository.
- The CLAUDE.md `thiserror` convention is met in spirit (typed enums per crate, no
  `Box<dyn Error>`), but not by the literal crate until admission.
