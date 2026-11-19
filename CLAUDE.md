# CLAUDE.md — rules for agents working on jpdf

Read [PLAN.md](PLAN.md) (scope, architecture, milestones) and [AUTOMATION.md](AUTOMATION.md)
(how you work autonomously). Start every session with the tick protocol in AUTOMATION.md §3.
The non-negotiable rules below will move to `CONSTITUTION.md` (protected) in M0.

## Non-negotiable rules
1. **No `unsafe`.** Every first-party crate has `#![forbid(unsafe_code)]`. Never remove it,
   never add `unsafe` for any reason. No exceptions.
2. **Security beats performance.** Never remove/raise a `Limits` check, skip validation,
   or weaken the trust boundary to make something faster.
3. **Never copy code from other PDF engines** (MuPDF, Poppler, Xpdf, Ghostscript, PDFium,
   pdf.js, qpdf, PDFBox, …). Learn from their design docs/architecture only; write from the
   ISO 32000-2 spec. Reference engines are used **only** as external programs in `tools/oracle`.
4. **All input is hostile:** PDF bytes, and every value from Swift/JS/native code. Validate at
   `jpdf-engine`; return typed errors, never panic. No `unwrap`/`expect`/indexing that can
   panic on input-derived data (clippy lints enforce this).
5. **Native layers stay thin.** No PDF parsing, layout, hit-testing or tool logic in Swift/TS.
   If you are tempted to add logic there, add an engine command instead.
6. **Third-party crates only via `jpdf-traits` adapters in `jpdf-backends`**, and only after
   passing the automated admission checks (PLAN.md §3, AUTOMATION.md §5.3).
7. **Never execute PDF JavaScript** or Launch actions; never open embedded files.
8. **Never commit corpus PDFs** (licensing). Add to `tools/corpus` manifests instead.
   Minimized repro files we created ourselves may be committed under `tests/regress/`.
9. **License:** MIT. Allowed dep licenses: MIT, Apache-2.0, BSD-2/3, Zlib, ISC, Unicode-3.0, OFL-1.1 (fonts).

## Workflow
- Spec → tests → implementation. Each feature has `specs/<area>/<feature>.md` citing ISO 32000-2 clauses.
- Every bug found by fuzzing/corpus gets a minimized regression test.
- Definition of done = all CI checks in PLAN.md §7 green. Don't mark a task done otherwise;
  report failures honestly with output.
- Never wait for a human. Decide using AUTOMATION.md §4 rules and record an ADR. If a
  non-negotiable would be broken, label `needs-human` and move on to other work.
- New dependencies go through automated admission (AUTOMATION.md §5.3), not a human.

## Conventions
- Rust edition 2024, toolchain pinned in `rust-toolchain.toml`.
- Errors: `thiserror` enums per crate; no `Box<dyn Error>` in public APIs.
- Spec references in code comments: `// ISO 32000-2 §7.5.8`.
- Web: TypeScript strict, React + MUI, no `dangerouslySetInnerHTML`, strict CSP.
- macOS: SwiftUI, Apple HIG, App Sandbox + Hardened Runtime; PDF engine lives in the XPC helper.
