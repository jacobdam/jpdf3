# CLAUDE.md — jpdf

Read, in order:
1. [CONSTITUTION.md](CONSTITUTION.md): the non-negotiable rules (protected).
2. [ops/TICK.md](ops/TICK.md): what to do in this session (one task per session).
3. [AUTOMATION.md](AUTOMATION.md): how the autonomous system works.
4. [PLAN.md](PLAN.md): scope, architecture, milestones.
5. [ops/judge/CONTRACT.md](ops/judge/CONTRACT.md): what the judge expects from `jpdf-cli` and `cargo xtask`.

## Conventions
- **Rust:** edition 2024, toolchain pinned in `rust-toolchain.toml` to an exact stable version.
  Update it to the latest stable during audit sessions via a normal PR. The judge rejects
  non-stable or unpinned channels.
- **Errors:** `thiserror` enums per crate; no `Box<dyn Error>` in public APIs.
- **Spec references in code comments:** `// ISO 32000-2 §7.5.8`.
- **Feature specs:** `specs/<area>/<feature>.md` from `specs/_TEMPLATE.md`. User journeys go in `specs/journeys/`.
- **Decisions:** `docs/adr/NNNN-title.md`. Retros: `docs/retros/`. Proposals for protected files: `ops/proposals/`.
- **Web:** TypeScript strict, React + MUI, no `dangerouslySetInnerHTML`, strict CSP.
- **macOS:** SwiftUI, Apple HIG, App Sandbox + Hardened Runtime; the PDF engine runs in the XPC helper.
