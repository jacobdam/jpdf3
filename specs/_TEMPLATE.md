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

## Security
What could leak, be bypassed or be abused? Adversarial verification for security-sensitive
features (redaction, encryption, signatures, sanitize): which external-tool checks prove it.

## Accessibility
Keyboard, VoiceOver/screen reader, contrast, focus order.

## Platforms
Differences between web and macOS.

## Performance
Budget (time to first result, frame time, memory) and how it is measured.

## Tests
Unit/property/fuzz tests and E2E test IDs.
