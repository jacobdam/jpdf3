# <Feature name>

Area: <syntax|render|text|annot|forms|edit|write|sign|engine|ui-web|ui-macos>
ISO 32000-2 clauses: §…
Journeys: J-…
Status: draft | ready | implemented

## Behavior
What it does, precisely. Engine commands added/changed (name, inputs, validation, outputs, errors).

## Persistence
Does it survive save → reopen? In which other viewers (MuPDF, Poppler, Acrobat semantics)?

## Undo
Undo/redo behavior.

## Errors
Every error state and what the user sees.

## Limits
Which `Limits` apply; worst-case input sizes; time/memory bounds.
Per-document budgets, not just per-object caps (overlapping or repeated input re-parses the
same bytes). Every limit names its exact-boundary test: at the limit passes, one past fails.
Every loop over input-derived geometry or counts has a step cap, test toys in suites included
(floats stop progressing: at 1e30, `pos + len == pos`).

## Security
What could leak, be bypassed or be abused? Adversarial verification for security-sensitive
features (redaction, encryption, signatures, sanitize): which external-tool checks prove it.
Self-check before review (write the answers here):
- Arithmetic on input- or caller-derived values: which products/sums are checked
  (`checked_mul` for three or more factors; overflow panics in every profile)?
- For each checker, conformance suite or validator: **what broken implementation still
  passes?** Known answers reach multi-block/padding boundaries; hostile fixtures have a
  control case proving the attack path was reached; only the specific error kinds pass.
- What this spec's tests cannot see (allocations, timing) and who covers it instead.

## Accessibility
Keyboard, VoiceOver/screen reader, contrast, focus order.

## Platforms
Differences between web and macOS.

## Performance
Budget (time to first result, frame time, memory) and how it is measured.

## Tests
Unit/property/fuzz tests and E2E test IDs.
