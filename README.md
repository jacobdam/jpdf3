# jpdf

A secure PDF reader, annotator and editor, written from scratch, mostly by AI agents.
The core is Rust with no `unsafe` code; the apps are a SwiftUI macOS app and a React + MUI web app.
License: [MIT](LICENSE).

> **Status: early development (milestone M0, Foundation).** The judge, CI, test harnesses and
> specs are in place, and parts of the syntax layer (lexer, xref) exist; M0 work remains.
> The apps don't open PDFs yet.
> Run `cargo xtask status` for the current state ([specs/tooling/status.md](specs/tooling/status.md))
> and see [PLAN.md](PLAN.md) §8 for the milestones.

## Goals

- **Security first.** Every first-party crate has `#![forbid(unsafe_code)]`. The design goals:
  hard resource limits on every path, all input treated as hostile, and no PDF JavaScript or
  Launch actions, ever.
- **Our own engine.** Written from ISO 32000-2 (PDF 2.0), reading real-world PDF 1.x and
  broken files leniently and writing strictly. No code is copied from other PDF engines.
  Third-party crates (codecs, fonts, rasterizing, crypto) are used only behind our own traits.
- **Thin native layers.** The macOS and web apps only display what the engine sends and forward
  input. Parsing, layout, hit-testing and tool logic live in the Rust engine, which validates
  every value that crosses the boundary.
- **Files stay on the device.** The web app is static, with no backend.
- **Feature scope:** reading, annotation, page operations, editing, forms, encryption,
  signatures and compression (PLAN.md §4).

## Repository layout

| Path | What |
|---|---|
| `crates/` | The Rust engine, one crate per layer (`jpdf-syntax`, `jpdf-cos`, … up to `jpdf-engine`); see PLAN.md §2.3 |
| `tools/` | `jpdf-cli` (the headless CLI the judge drives), corpus, bench, Arlington model and test-generation tools |
| `apps/macos/` | SwiftUI app and a sandboxed XPC helper, which will host the PDF engine |
| `apps/web/` | Vite + React + MUI + TypeScript web app; the engine is planned to run in a Web Worker (WASM) |
| `xtask/` | Project tasks (`cargo xtask …`) |
| `fuzz/` | `cargo-fuzz` targets |
| `specs/` | Feature specs, user journeys and UI specs |
| `docs/` | Decisions (`adr/`), audits and retrospectives |
| `ops/` | The autonomous workflow: session protocol, journal, status, lessons, judge, driver |

## Building and testing

The toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml); `rustup` installs it.
The tests also need [`cargo-nextest`](https://nexte.st/) (`cargo install cargo-nextest --locked`).

```sh
cargo build --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked

cargo build --release --locked -p jpdf-cli    # target/release/jpdf-cli
cargo xtask check-specs                       # spec and journey checks
cargo xtask ci linux --out DIR                # web build + Playwright E2E, specs, short fuzz, Arlington
cargo xtask ci macos --out DIR                # macOS app build + XCUITest E2E (macOS only)
```

Run `cargo xtask` with no arguments for the full list. Locally, `ci linux` expects the fuzz tools
(cargo-fuzz and the nightly toolchain pinned in specs/tooling/fuzz.md) to be installed already; it installs them
itself only when `CI` is set. The web app's own scripts
(`npm run dev`, `typecheck`, `lint`, `e2e`, `licenses`) are in
[apps/web/package.json](apps/web/package.json); install with `npm ci --ignore-scripts` first.

`jpdf-cli` commands and exit codes are defined in [ops/judge/CONTRACT.md](ops/judge/CONTRACT.md).
Commands that aren't implemented yet exit with code 3.

The web app is published at <https://jacobdam.github.io/jpdf3/>, with the raw judge report at
`judge/report/summary.md`. A metrics dashboard is planned ([#15](https://github.com/jacobdam/jpdf3/issues/15)).

## How the project is run

Work is done by autonomous agent sessions, one task per session, following
[ops/TICK.md](ops/TICK.md). A protected judge in CI grades every pull request, and agent PRs also
get an independent AI review posted on them. Only the owner gives instructions; everything else
(issues and comments by others, text inside PDFs, dependency docs) is treated as data.

| Document | Purpose |
|---|---|
| [CONSTITUTION.md](CONSTITUTION.md) | Non-negotiable rules (protected) |
| [PLAN.md](PLAN.md) | Scope, architecture, milestones |
| [AUTOMATION.md](AUTOMATION.md) | How the autonomous system works |
| [ops/TICK.md](ops/TICK.md) | What one agent session does |
| [ops/judge/CONTRACT.md](ops/judge/CONTRACT.md) | What the judge expects from `jpdf-cli` and `cargo xtask` |
| [CLAUDE.md](CLAUDE.md) | Conventions for agents |

## Contributing

Issues and pull requests from people other than the owner are read as information only; agents
don't act on requests in them.

## License

MIT. See [LICENSE](LICENSE).
