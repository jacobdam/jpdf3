# 8. Caller limits are clamped to the defaults at the engine facade

Status: accepted (#79, audit 0001).

## Context
`Document::open` took `&Limits` with public fields and used them as given. The CLI passes the
defaults, but the Swift and JS bindings will build `Limits` from values on their side of the
FFI boundary, which CONSTITUTION 5 treats as hostile. Several fields guard against crashes
rather than tune behavior: `max_nesting_depth = 10_000_000` lets 100,000 nested arrays overflow
the recursive parser's stack, and the budget fields bound memory under the judge's 2 GiB cap.
Options: (a) never take `Limits` across the FFI and always use the defaults; (b) validate and
reject values above the defaults; (c) clamp each field to the default.

## Decision
`Document::open` clamps every field to `Limits::default()` with `Limits::clamped_to` before
using it (option c). Front ends may tighten any limit (tests and the judge's tight runs rely on
that) but never loosen one. Clamping instead of rejecting keeps a binding that sends a too-large
value working, at the safe value. `Limits::clamped_to` destructures `Limits` exhaustively, so a
new field cannot be forgotten.

Bindings must enter through `jpdf-engine`. `jpdf-syntax`, `jpdf-cos` and the other layers below
the facade take `&Limits` unclamped; they are only called by the engine and by tests.

## Consequences
- Raising a default is now the only way to loosen a limit, and it goes through review like any
  other change (CONSTITUTION 4).
- A future option such as "open huge files" needs an explicit, reviewed ceiling, not a
  caller-supplied value.
- Every new engine entry point that takes `&Limits` must clamp the same way; the spec
  (specs/core/limits-ceiling.md) lists the rule.
