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

Empty cells are open work: the journeys are written by #6–#9 and the specs by the milestone
that builds each feature (PLAN §8).

## PLAN §1 requirements

| Req | Requirement | Journeys | Specs | Other tests |
|---|---|---|---|---|
| R1 | Security first: forbid unsafe, limits, no JS, fuzzing | | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge policy; judge crash grading; `jpdf-cli` limit tests |
| R2 | Performance (never at the cost of R1) | | | judge bench (later, #18) |
| R3 | Platforms: macOS and web first | | | `cargo xtask ci linux`, `cargo xtask ci macos` |
| R4 | Untrusted native layer | | | |
| R5 | Spec coverage: ISO 32000-2 + real-world 1.x + repair | | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge open rate |
| R6 | Test corpus with rendering compared to other engines | | | judge render ratchet |
| R7 | Feature parity with PDFgear | (see F-rows below) | | |
| R8 | UI: PDFgear ideas, native look | | | |
| R9 | Dependencies behind our own traits | | | judge dependency policy |
| R10 | Licensing: MIT, permissive deps, no committed corpora | | | judge license policy |
| R11 | Web: static only, files never leave the device | | | |
| R12 | Digital signatures: verify, PAdES, timestamps, LTV | | | |
| R13 | No human in the loop: specs, tests, ratchets decide | | [specs/tooling/check-specs.md](tooling/check-specs.md) | `cargo xtask check-specs` |

## PLAN §4 features (Phase 1)

| Req | Feature group | Journeys | Specs | Other tests |
|---|---|---|---|---|
| F-RD-1 | Reader: open (incl. password), fast first page, progressive loading | | [specs/cli/jpdf-cli.md](cli/jpdf-cli.md) | judge `info` |
| F-RD-2 | Reader: views (single, continuous, two-page, cover), fit, zoom, rotate view | | | |
| F-RD-3 | Reader: sidebar (thumbnails, outline, annotations, attachments, layers) | | | |
| F-RD-4 | Reader: search, text select + copy, links | | | judge `text` |
| F-RD-5 | Reader: read/presentation mode, page tint, print, properties | | | |
| F-AN-1 | Annotate: text markup (highlight, underline, strikeout, squiggly) | | | judge `annotate` |
| F-AN-2 | Annotate: sticky note, text box, callout | | | |
| F-AN-3 | Annotate: shapes, ink, eraser | | | |
| F-AN-4 | Annotate: stamps, visual signatures | | | |
| F-AN-5 | Annotate: properties, replies/status, flatten, XFDF/FDF | | | |
| F-ED-1 | Edit: existing text (font substitution warning) | | | |
| F-ED-2 | Edit: add text, images, links; move/resize/delete images | | | |
| F-ED-3 | Edit: watermark, header/footer, Bates, background | | | |
| F-ED-4 | Edit: crop; true redaction + metadata scrub | | | judge `redact` |
| F-PG-1 | Pages: insert, delete, extract, reorder, rotate, duplicate | | | |
| F-PG-2 | Pages: merge, split, page labels | | | |
| F-FM-1 | Forms: fill, safe calculations, create/edit fields, flatten | | | judge `fill` |
| F-SC-1 | Security: encrypt AES-256 R6, decrypt, permissions/passwords | | | judge `encrypt` |
| F-SC-2 | Signatures: verify (integrity, chain, trust, timestamp, MDP, changes) | | | judge `verify-signatures` |
| F-SC-3 | Signatures: sign PAdES B-B…B-LTA, certification, visible appearance | | | judge `sign` |
| F-SC-4 | Timestamps and OCSP/CRL (opt-in, parsed as untrusted) | | | |
| F-OS-1 | Save: incremental, full rewrite, compress | | | judge `rewrite` |
| F-OS-2 | Convert: image → PDF, PDF → images | | | judge `render` |

## PLAN §4 features (Phase 2)

| Req | Feature group | Journeys | Specs | Other tests |
|---|---|---|---|---|
| F-P2-1 | OCR | | | |
| F-P2-2 | PDF → Office, then Office → PDF | | | |
| F-P2-3 | AI chat/summarize (no backend) | | | |
| F-P2-4 | Batch, compare, PDF/A | | | |
| F-P2-5 | iOS, then Android | | | |
