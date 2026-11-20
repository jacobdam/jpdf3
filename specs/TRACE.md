# Traceability: requirement → journeys → specs → tests

Checked by `cargo xtask check-specs` (spec: [specs/tooling/check-specs.md](tooling/check-specs.md)):
every journey ID and every feature spec must appear here; every journey ID and `specs/` path
mentioned here must exist.

How to read it:
- **Requirement:** a PLAN §1 requirement (R1–R13) or a PLAN §4 feature group (F-…).
- **Journeys:** IDs from `specs/journeys/*.md`. A journey's E2E test IDs are on its own
  `- test:` line (CONTRACT §4), so they are not repeated here.
- **Specs:** feature specs that implement the requirement.
- **Other tests:** checks that are not journey E2E tests (judge probes, unit, fuzz, policy).

Empty cells are open work: the journeys are written by #6–#9 and #42, and the specs by the milestone
that builds each feature (PLAN §8).

## PLAN §1 requirements

| Req | Requirement | Journeys | Specs | Other tests |
|---|---|---|---|---|
| R1 | Security first: forbid unsafe, limits, no JS, fuzzing | J-RD-04, J-RD-05, J-RD-13, J-RD-18, J-RD-27, J-RD-28, J-RD-29, J-ANN-09, J-ANN-11, J-ANN-14, J-ANN-18, J-ANN-22, J-ANN-23, J-ANN-24, J-PG-02, J-PG-11, J-PG-15, J-ED-04, J-ED-05, J-OS-01, J-OS-06, J-FM-05, J-FM-06, J-FM-07, J-FM-12, J-SC-01, J-SC-05, J-SC-07, J-SC-08, J-SC-09 | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge policy; judge crash grading; `jpdf-cli` limit tests |
| R2 | Performance (never at the cost of R1) | J-RD-01, J-RD-26 | | judge bench (later, #18) |
| R3 | Platforms: macOS and web first | | | `cargo xtask ci linux`, `cargo xtask ci macos` |
| R4 | Untrusted native layer | J-RD-05, J-RD-23, J-ANN-16, J-ANN-23, J-ANN-24, J-PG-03, J-ED-03, J-ED-04, J-FM-01, J-FM-08, J-FM-11, J-SC-07 | | |
| R5 | Spec coverage: ISO 32000-2 + real-world 1.x + repair | | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge open rate |
| R6 | Test corpus with rendering compared to other engines | J-ANN-01, J-ANN-07, J-ANN-21, J-ANN-26, J-PG-03, J-OS-02, J-OS-03, J-OS-04, J-OS-05 | | judge render ratchet |
| R7 | Feature parity with PDFgear | (see F-rows below) | | |
| R8 | UI: PDFgear ideas, native look | (every journey; mapping in the UI spec) | [specs/ui/information-architecture.md](ui/information-architecture.md), [specs/ui/shortcuts.md](ui/shortcuts.md), [specs/ui/accessibility.md](ui/accessibility.md) | judge `ui_spec_present`; axe-core and XCUITest accessibility audits (#12, #13) |
| R9 | Dependencies behind our own traits | | | judge dependency policy |
| R10 | Licensing: MIT, permissive deps, no committed corpora | | | judge license policy |
| R11 | Web: static only, files never leave the device | J-RD-01, J-RD-03, J-RD-24, J-ANN-14, J-ANN-15, J-ANN-22, J-PG-13, J-OS-05, J-FM-07 | | |
| R12 | Digital signatures: verify, PAdES, timestamps, LTV | | | |
| R13 | No human in the loop: specs, tests, ratchets decide | | [specs/tooling/check-specs.md](tooling/check-specs.md) | `cargo xtask check-specs` |

## PLAN §4 features (Phase 1)

| Req | Feature group | Journeys | Specs | Other tests |
|---|---|---|---|---|
| F-RD-1 | Reader: open (incl. password), fast first page, progressive loading | J-RD-01, J-RD-02, J-RD-03, J-RD-04, J-RD-05, J-RD-23, J-RD-24, J-RD-26, J-RD-27, J-RD-29, J-RD-30 | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge `info` |
| F-RD-2 | Reader: views (single, continuous, two-page, cover), fit, zoom, rotate view | J-RD-06, J-RD-07, J-RD-08, J-RD-09, J-RD-25 | | |
| F-RD-3 | Reader: sidebar (thumbnails, outline, annotations, attachments, layers) | J-RD-10, J-RD-11, J-RD-12, J-RD-13, J-RD-14 | | |
| F-RD-4 | Reader: search, text select + copy, links | J-RD-15, J-RD-16, J-RD-17, J-RD-18, J-RD-28, J-RD-31 | | judge `text` |
| F-RD-5 | Reader: read/presentation mode, page tint, print, properties | J-RD-19, J-RD-20, J-RD-21, J-RD-22 | | |
| F-AN-1 | Annotate: text markup (highlight, underline, strikeout, squiggly) | J-ANN-01, J-ANN-02, J-ANN-03 | | judge `annotate` |
| F-AN-2 | Annotate: sticky note, text box, callout | J-ANN-04, J-ANN-05, J-ANN-06 | | |
| F-AN-3 | Annotate: shapes, ink, eraser | J-ANN-07, J-ANN-08, J-ANN-09, J-ANN-10, J-ANN-11, J-ANN-12 | | |
| F-AN-4 | Annotate: stamps, visual signatures | J-ANN-13, J-ANN-14, J-ANN-15 | | |
| F-AN-5 | Annotate: properties, replies/status, flatten, XFDF/FDF | J-ANN-16, J-ANN-17, J-ANN-18, J-ANN-19, J-ANN-20, J-ANN-21, J-ANN-22, J-ANN-23, J-ANN-24, J-ANN-25, J-ANN-26 | | |
| F-ED-1 | Edit: existing text (font substitution warning) | J-ED-01 | | |
| F-ED-2 | Edit: add text, images, links; move/resize/delete images | J-ED-02, J-ED-03, J-ED-04, J-ED-05 | | |
| F-ED-3 | Edit: watermark, header/footer, Bates, background | J-ED-06, J-ED-07, J-ED-08, J-ED-09 | | |
| F-ED-4 | Edit: crop; true redaction + metadata scrub | J-ED-10, J-SC-06, J-SC-07, J-SC-08, J-SC-09, J-SC-10 | | judge `redact` |
| F-PG-1 | Pages: insert, delete, extract, reorder, rotate, duplicate | J-PG-01, J-PG-02, J-PG-03, J-PG-04, J-PG-05, J-PG-06, J-PG-07, J-PG-08, J-PG-13, J-PG-15, J-PG-16 | | |
| F-PG-2 | Pages: merge, split, page labels, bookmark editing | J-PG-09, J-PG-10, J-PG-11, J-PG-12, J-PG-14 | | |
| F-FM-1 | Forms: fill, safe calculations, create/edit fields, flatten | J-FM-01, J-FM-02, J-FM-03, J-FM-04, J-FM-05, J-FM-06, J-FM-07, J-FM-08, J-FM-09, J-FM-10, J-FM-11, J-FM-12 | | judge `fill` |
| F-SC-1 | Security: encrypt AES-256 R6, decrypt, permissions/passwords | J-SC-01, J-SC-02, J-SC-03, J-SC-04, J-SC-05 | | judge `encrypt` |
| F-SC-2 | Signatures: verify (integrity, chain, trust, timestamp, MDP, changes) | | | judge `verify-signatures` |
| F-SC-3 | Signatures: sign PAdES B-B…B-LTA, certification, visible appearance | | | judge `sign` |
| F-SC-4 | Timestamps and OCSP/CRL (opt-in, parsed as untrusted) | | | |
| F-OS-1 | Save: incremental, full rewrite, compress | J-OS-01, J-OS-02, J-OS-03, J-OS-06 | | judge `rewrite` |
| F-OS-2 | Convert: image → PDF, PDF → images | J-OS-04, J-OS-05 | | judge `render` |

## PLAN §4 features (Phase 2)

| Req | Feature group | Journeys | Specs | Other tests |
|---|---|---|---|---|
| F-P2-1 | OCR | | | |
| F-P2-2 | PDF → Office, then Office → PDF | | | |
| F-P2-3 | AI chat/summarize (no backend) | | | |
| F-P2-4 | Batch, compare, PDF/A | | | |
| F-P2-5 | iOS, then Android | | | |
| F-P2-6 | Read aloud (text to speech) | | | |
