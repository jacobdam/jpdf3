# jpdf — Project Plan

A PDF reader, annotator and editor written from scratch, mostly by AI agents, with
people stepping in only at milestone checkpoints.
License: **MIT**. Platforms: **macOS + Web first**, then iOS, then Android.

This is the source of truth for scope, architecture and milestones. Rules for agents
working in this repo are in [CLAUDE.md](CLAUDE.md).

---

## 1. Requirements (confirmed)

| # | Requirement | Decision |
|---|---|---|
| R1 | **Security first** | Core in Rust, `#![forbid(unsafe_code)]` in every first-party crate. Hard resource limits. No PDF JavaScript execution. Continuous fuzzing. |
| R2 | **Performance** | Important, but never at the cost of R1. No `unsafe`, no skipped checks for speed. Borrow *techniques* from PDFium / MuPDF / pdf.js / Poppler / qpdf, never their code. |
| R3 | **Platforms** | macOS (SwiftUI) and Web (React + MUI) first; iOS and Android later. |
| R4 | **Untrusted native layer** | The core validates every value that crosses the FFI/WASM boundary. Native code is a thin shell with as little logic as possible. |
| R5 | **Spec coverage** | All of ISO 32000-2:2020 (PDF 2.0) plus real-world PDF 1.x and broken-file recovery. |
| R6 | **Test corpus** | Well-known public corpora + generated + mutated PDFs, with rendering compared against other engines. |
| R7 | **Features** | Feature parity with PDFGear: reader, annotation, editing, pages, forms, security, signatures, compress. OCR, Office conversion and AI chat come in Phase 2. |
| R8 | **UI** | PDFGear's ideas, native look: Apple HIG on macOS, Material (MUI) on web. No PDFGear branding or assets. |
| R9 | **Dependencies** | Our own PDF engine. Audited, pure-safe-Rust crates allowed for codecs, fonts, rasterizing and crypto, **always behind our own traits** so each one can be replaced. |
| R10 | **Licensing** | MIT. Dependencies must be permissively licensed. Corpora are never committed. |
| R11 | **Web** | No backend. Static hosting only, and files never leave the device. |
| R12 | **Digital signatures** | All of it: verify, create (PAdES), timestamps, long-term validation (LTV). |
| R13 | **No human in the loop** | Specs, tests, comparison against other engines and ratcheted metrics decide whether work is done. Fully autonomous loop per [AUTOMATION.md](AUTOMATION.md). |

### Explicit non-goals
- Running PDF JavaScript, Launch actions, or opening embedded executables.
- XFA forms (removed in PDF 2.0). We show the AcroForm fallback if there is one.
- 3D / RichMedia playback. We show the poster image only.
- Any server-side processing.

### Assumptions (change if wrong)
- macOS build is designed for the **App Sandbox + Hardened Runtime**, so it can ship
  through the Mac App Store or as a notarized direct download.
- Minimum targets: macOS 14+, current evergreen browsers (Chrome, Safari, Firefox).
- Project/crate name `jpdf` is a working name.

---

## 2. Architecture

```
┌─────────────────────────── macOS app ───────────────────────────┐
│ SwiftUI shell (no PDF logic)                                    │
│   • shows bitmaps/tiles, forwards input events as commands      │
│   • OS services: open/save panels, clipboard, print, keychain   │
│        │  XPC (serialized commands, validated by core)          │
│ ┌──────▼────────────── XPC helper (sandboxed, no file access ───┐│
│ │ uniffi bindings → jpdf-engine (Rust core)  gets bytes, not   ││
│ └───────────────────────────────────────────── paths) ─────────┘│
└─────────────────────────────────────────────────────────────────┘
┌──────────────────────────── Web app ────────────────────────────┐
│ React + MUI (no PDF logic)                                      │
│        │ postMessage (serialized commands, validated by core)   │
│ ┌──────▼──────── Web Worker ──────────┐                         │
│ │ wasm-bindgen → jpdf-engine (WASM)   │  strict CSP, no backend │
│ └─────────────────────────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

### 2.1 Trust boundary (R4)
- **One entry point.** Both platforms talk to `jpdf-engine` through one versioned
  **command/response protocol**. The same schema is used for UniFFI and WASM.
- **Handles, not pointers.** Native code only ever holds opaque handles
  (`DocId`, `PageId`, `AnnotId`, `JobId`). The engine checks every handle against its own tables.
- **Every field is validated.** Page indexes, rectangles (finite, in range), zoom
  (clamped), strings (valid UTF-8, length-limited), colors, and byte buffers (size-limited).
  If a value fails validation, the engine returns a typed error and keeps its state unchanged.
- **The core owns the logic; native code only displays it.** The engine decides
  hit-testing, selection, layout, undo/redo, and what each tool does. Native code draws
  RGBA tiles plus simple overlay primitives (selection rects, handles, caret) that the engine sends.
- **No panics across the boundary.** Every entry point is wrapped. A panic counts as a
  bug (it shows up in CI fuzzing), but at runtime it becomes an `Internal` error and
  the document is marked poisoned.
- **The engine never touches the file system.** Native code hands in bytes and receives
  bytes back to save. On macOS the XPC helper has no file entitlements at all.

### 2.2 Resource limits (part of security, not optional)
A `Limits` struct is passed to every operation and enforced by the engine:
- max file size, object count, xref entries, nesting depth (objects, content-stream
  operators, Form XObjects, patterns, shading functions, outline/page tree)
- max decompressed bytes per stream and per document (zip-bomb guard), max image
  pixels, max path segments, max glyphs per page
- a time/operation budget per job, with cooperative cancellation (`JobId` can be cancelled)
- cycle detection on all reference chasing (page tree, outlines, Form XObjects, name trees)

### 2.3 Crate layout (Cargo workspace)

```
crates/
  jpdf-traits      # replaceable backend traits (§3) — no implementations
  jpdf-syntax      # lexer, objects, xref/xref streams, object streams, repair/recovery
  jpdf-crypt       # standard security handler R2–R6 (RC4, AES-128, AES-256), permissions
  jpdf-filters     # Flate/LZW/ASCII85/ASCIIHex/RunLength/CCITT/DCT/JPX/JBIG2 via traits
  jpdf-cos         # document object model: page tree, inheritance, name/number trees
  jpdf-font        # font loading: Type1/CFF/TrueType/Type3/CID, cmaps, encodings, ToUnicode
  jpdf-color       # color spaces, ICC (via trait), separations/DeviceN, rendering intents
  jpdf-content     # content-stream interpreter → display list (with caching)
  jpdf-render      # display list → tiles (via Rasterizer trait); transparency groups, blend, masks
  jpdf-text        # text extraction, reading order, search, selection geometry
  jpdf-annot       # all annotation types: model, appearance-stream generation
  jpdf-forms       # AcroForm: fields, values, appearance, flatten
  jpdf-edit        # page ops, content edit (text/image), watermark, header/footer, redaction
  jpdf-write       # serializer: incremental update, full rewrite, object streams, compress
  jpdf-sign        # CMS/PAdES verify + create, DSS/LTV, timestamps (crypto via traits)
  jpdf-engine      # facade: sessions, command protocol, validation, limits, undo/redo, jobs
  jpdf-backends    # default adapters wrapping vetted third-party crates (§3)
bindings/
  jpdf-uniffi      # Swift bindings (macOS/iOS), later Kotlin (Android)
  jpdf-wasm        # wasm-bindgen + worker glue
apps/
  macos/           # SwiftUI app + XPC helper target
  web/             # Vite + React + MUI + TypeScript
tools/
  jpdf-cli         # headless: info, render, text, check, write (used by tests)
  corpus           # fetch/manifest/dedupe/triage scripts
  oracle           # reference-engine renderers + image diff
  bench            # benchmark harness + comparison with PDFium/MuPDF
  arlington        # Arlington PDF Model → generated validators/tests
fuzz/              # cargo-fuzz targets
specs/             # one feature spec per feature (see §7)
```

Dependency direction is strictly downward: `engine` → feature crates → `content/font/color` →
`cos` → `syntax` → `traits`. Bindings depend **only** on `jpdf-engine`.

---

## 3. Replaceable backends (R9)

Each external capability sits behind a trait in `jpdf-traits`. Default
implementations live in `jpdf-backends`, one Cargo feature per adapter. Each trait has
a **conformance test suite** that any implementation must pass, so swapping one out is safe.

| Trait | Purpose | Candidate default (to verify in M0) |
|---|---|---|
| `Inflate` / `Deflate` | Flate | `miniz_oxide` |
| `JpegDecoder` | DCTDecode | `zune-jpeg` or `jpeg-decoder` |
| `Jpeg2000Decoder` | JPXDecode | `hayro-jpeg2000`, else our own |
| `Jbig2Decoder` | JBIG2Decode | `hayro-jbig2`, else our own |
| `CcittDecoder` | CCITTFax | `hayro-ccitt` / own (small) |
| `FontEngine` | outlines, metrics, hinting for TT/CFF/CFF2 | `skrifa` (Google fontations) |
| `Type1Parser` | Type 1 fonts | own |
| `Rasterizer` | paths, fills, strokes, clips, AA, blend | `vello_cpu` or `tiny-skia` |
| `ColorTransform` | ICC | `moxcms` |
| `Hasher` / `Cipher` | MD5, SHA-2, RC4, AES | RustCrypto crates |
| `SignatureVerifier` | RSA/ECDSA public-key verify, X.509, CMS | RustCrypto (`rsa`, `p256`, `p384`, `x509-cert`, `cms`) |
| `Signer` | private-key operations | **platform**: Keychain/CryptoKit (macOS), WebCrypto (web) — see §6.4 |
| `OcrEngine` *(Phase 2)* | text recognition | `ocrs` (pure Rust) |
| `TextShaper` | shaping for edited/added text | `rustybuzz` / `harfrust` |

**Admission rules for any third-party crate** (enforced by the protected judge, no human step;
full rules in AUTOMATION.md §6.4):
1. License in the allowlist (MIT, Apache-2.0, BSD-2/3-Clause, Zlib, ISC, Unicode-3.0, OFL-1.1 for fonts).
2. **No `unsafe` anywhere in the dependency tree.** Sole exception: RustCrypto cryptographic
   primitives that carry an imported audit (listed in the protected `ops/judge/unsafe-allowlist.txt`).
3. `cargo vet` with **imported audits only** (Google, Mozilla, Bytecode Alliance, Zcash).
4. No build scripts / proc-macros unless audited; fuzzed through our trait before becoming default.
5. Pinned version; updates go through the same gates.

If a candidate fails any rule, we write our own implementation behind the trait. Several
candidates in the table above (e.g. `hayro-*`, `vello_cpu`, `moxcms`) may fail rule 3, so
plan for our own JPX/JBIG2/rasterizer/ICC code.

Bundled data: Adobe `cmap-resources` (BSD-3) and standard-14 substitute fonts with
permissive licenses (e.g. PDFium's Foxit fonts (BSD) or Liberation (OFL)).
**Not** the URW base35 fonts (AGPL).

---

## 4. Feature scope (R7)

### Phase 1 — v1.0 (macOS + Web)

**Reader**
- Open (including password), fast first page, progressive loading for web-optimized files
- Views: single, continuous, two-page, cover + two-page; fit width/page; zoom; rotate view
- Sidebar: thumbnails, outline/bookmarks, annotation list, attachments, layers (OCGs)
- Search (case/whole-word), text select + copy, links (internal; external with confirmation)
- Read mode / presentation mode, dark/sepia page tint, print, document properties

**Annotate**
- Highlight, underline, strikeout, squiggly; sticky note; text box (FreeText); callout
- Shapes: rectangle, ellipse, line, arrow, polygon, polyline, cloud; ink/freehand; eraser
- Stamps (built-in + custom image); handwritten/typed/image signatures (visual)
- Edit properties (color, opacity, width, font), reply/status in comment threads
- Flatten annotations; import/export (XFDF/FDF)

**Edit**
- Edit existing text (same font if possible, else font substitution with a warning)
- Add text, images, links; move/resize/delete images
- Watermark (text/image), header/footer, Bates numbering, page background
- Crop pages; **true redaction** (removes content, not just covers it) + metadata scrub

**Pages**
- Insert (blank, from file, from image), delete, extract, reorder (drag), rotate, duplicate
- Merge files, split (by range/size/bookmarks), page labels

**Forms**
- Fill all AcroForm field types, calculations limited to a safe built-in subset
  (sum/avg/min/max/product via `AFSimple_Calculate`; formatting via `AFNumber_Format`/`AFDate`),
  implemented natively in Rust (**not** as a JS interpreter); create/edit fields; flatten

**Security & signatures**
- Encrypt (AES-256 R6), decrypt, change permissions/passwords
- Verify signatures: integrity, certificate chain, trust, timestamp, DocMDP/FieldMDP,
  report changes made after signing
- Sign: PAdES B-B/B-T/B-LT/B-LTA, certification signatures, visible appearances
- Timestamps (RFC 3161) and OCSP/CRL: **off by default, enabled by the user**; the request
  is sent by the platform layer, and the response bytes are parsed by the core as untrusted input

**Optimize & save**
- Incremental save (fast, keeps signatures valid), full rewrite, compress (object streams,
  image downsampling/recompression, font subsetting, removing unused objects)
- Image → PDF (JPEG/PNG), PDF → images (PNG/JPEG)

### Phase 2 — after v1.0
- **OCR** (`OcrEngine` trait; `ocrs` on-device, Apple Vision as an optional macOS backend
  whose output the core validates like any untrusted input)
- **PDF → Office** (DOCX/XLSX/PPTX writers in Rust), then **Office → PDF** (needs a layout engine;
  biggest item, scoped separately)
- **AI chat/summarize** — no backend: either the user brings their own API key (the client calls the
  provider directly) or an on-device model (Apple Foundation Models / WebGPU LLM). Decided at Phase 2 start.
- Batch processing, compare documents, PDF/A conversion and validation
- **iOS** (reuses UniFFI + much of the SwiftUI), then **Android** (UniFFI Kotlin + Jetpack Compose)

---

## 5. Performance (R2)

**Rule:** a speed-up may not add `unsafe`, remove a limit, loosen validation, or skip a
boundary check. Techniques are borrowed from other engines' designs; code is never copied
(MuPDF/Poppler are AGPL/GPL; PDFium/pdf.js are permissive, but the code is C++/JS and
still not to be ported line by line).

| Technique | Inspired by |
|---|---|
| Load objects only when needed via xref; bounded LRU object cache | PDFium, MuPDF |
| Parse content once → display list; replay for any zoom, thumbnail, hit test, text | MuPDF, pdf.js |
| Tiled rendering, visible-first priority queue, low-res preview first | PDFium/Chrome |
| Glyph cache by (font, size, subpixel offset) | MuPDF, Poppler |
| Shared resource store (fonts, images, ICC transforms), size-bounded | MuPDF |
| Parse/render off the UI thread (XPC helper / Web Worker) | pdf.js |
| Progressive loading of web-optimized files; byte-range requests | pdf.js, PDFium |
| Incremental save (append changes) | qpdf, PDFium |
| Object streams + xref streams on write | qpdf |
| Decode images at display resolution (DCT scaling, JPX resolution levels) | MuPDF, PDFium |
| Parallel page/tile rendering with cancellation (Rayon / multiple workers) | MuPDF |

Safe-Rust specifics: zero-copy parsing over `&[u8]`, arena/index-based object graph,
iterator-style loops so the compiler can drop bounds checks and auto-vectorize,
profile-guided optimization for release builds, no memory-mapped files.

**Targets** (measured on M1-class Mac / mid-range laptop browser):
- first page visible < 200 ms for a typical 10 MB document; 60 fps scrolling
- render throughput within **1.5× of PDFium** natively, **2×** on WASM
- peak memory bounded by `Limits`; never proportional to file size for linearized reading

**CI:** `tools/bench` runs on a fixed benchmark corpus. A regression of more than 5% on any
metric fails the build unless the PR includes a justification that a human approves.

---

## 6. Testing & verification (R6, R13)

### 6.1 Corpora (fetched by `tools/corpus`, never committed; manifest + hashes are committed)
- **Engine test suites:** pdf.js, PDFium, Poppler, qpdf, PDFBox, Apache Tika regression
- **Standards suites:** PDF Association PDF 2.0 examples, veraPDF corpus, Isartor, Bavaria
- **Security-focused:** OSS-Fuzz seed corpora (pdfium, poppler, qpdf, mupdf), known CVE samples
- **Large real-world sets:** Digital Corpora GovDocs1, SafeDocs CommonCrawl (CC-MAIN-2021, ~8M);
  sampled into tiers: `smoke` (~500, every PR), `regress` (~20k, nightly), `wide` (rotating ~50–100k shards weekly, millions over time)
- **Generated:**
  - printed from many producers: LaTeX (pdfTeX/XeTeX/LuaTeX), LibreOffice, Chrome, Ghostscript,
    Cairo, ReportLab, qpdf transforms (linearize, object streams, encrypt every R/V combo)
  - spec-driven: generate valid + boundary objects from the **Arlington PDF Model**
  - mutation: structure-aware mutators (broken xref, truncated streams, cycles, huge numbers)
- Every PDF that triggers a bug is minimized and kept permanently in `regress`.

### 6.2 Rendering comparison against other engines
- Render each corpus page at 72/150 dpi with **PDFium, MuPDF, Poppler, pdf.js**, used only as
  external test programs (never linked, never shipped, code never read).
- Expected image = the consensus where ≥2 engines agree within tolerance. Pages where the
  engines disagree go to a "disputed" list for spec-based triage (usually by an agent reading
  the spec, escalated to a human only if still unclear).
- Metric: per-page perceptual diff. Goals: ≥ 97% of consensus pages pass in `regress`
  at v1.0, and the pass rate never goes down.

### 6.3 Other checks
- **Fuzzing:** cargo-fuzz targets for parser, every filter, fonts, content interpreter,
  render, text, writer round-trip, command protocol. Run on every PR (short) and nightly (long);
  submit to OSS-Fuzz once public.
- **Round-trip:** open → save (incremental and full) → `qpdf --check` + re-render equals original.
- **Output validation:** veraPDF for PDF/A output, external signature validators for PAdES.
- **Property tests** (`proptest`) for the object model, writer, and command validation.
- **Protocol fuzzing:** random/malformed command sequences against `jpdf-engine` must never
  panic or corrupt state.
- **UI E2E:** Playwright (web), XCUITest (macOS), driven from feature specs.
- **Text:** extraction compared against pdftotext/pdf.js on the corpus (order-insensitive metric).

### 6.4 Signature-specific security
Private-key operations go through the platform `Signer` (Keychain / Secure Enclave / WebCrypto),
which avoids timing side channels in pure-Rust RSA private-key code. The core **verifies the
returned signature against the certificate** before embedding it, so a wrong or malicious
signature from the native layer is rejected.

---

## 7. How the AI agents work (R13)

1. **Specs first.** Each feature gets `specs/<area>/<feature>.md`: behavior, spec clauses
   (ISO 32000-2 section numbers), commands it adds, limits, acceptance tests.
2. **Tests first.** Acceptance tests and fuzz targets land with or before the implementation.
3. **Definition of done** (CI checks, all required):
   `fmt` · `clippy -D warnings` · `forbid(unsafe_code)` · `cargo deny` · `cargo vet` ·
   unit/property tests · `smoke` corpus (0 panics, 0 timeouts, 0 limit escapes) ·
   no drop in render pass rate · bench within budget · short fuzz run · E2E for touched UI.
4. **Review:** every PR gets an automated code review + security review pass before merge.
5. **No human gate.** Escalations are replaced by mechanical checks and fixed decision
   rules. The full loop, guardrails and discovery handling are in [AUTOMATION.md](AUTOMATION.md).
6. **Progress tracking:** GitHub Issues as the task queue, `ops/STATUS.md` + `ops/journal/`
   for agent state, `docs/adr/` for decisions.

---

## 8. Milestones

Each milestone ends at an **automated gate** (`cargo xtask gate`, criteria in protected `ops/judge/gates.toml`): judge metrics + held-out exam corpus + every user journey assigned to the milestone green (AUTOMATION.md §5.1).
Milestones M4+ can overlap once their dependencies are met.

| M | Name | Main deliverables | Exit criteria |
|---|---|---|---|
| **M0** | Foundation | Workspace, protected judge + gates.toml, project CI via xtask, guard hooks, driver, `jpdf-traits`, backend crates selected + vetted, corpus tiers + exam set, bench harness, fuzz infra, Arlington ingestion, spec template, user journeys (from PDFGear research) + UI spec, judge self-tests, metrics + forecast dashboard | Judge self-tests green; judge runs end to end on a stub `jpdf-cli`; pilot fire drill passed (AUTOMATION.md §14) |
| **M1** | Syntax & security | lexer, objects, xref (table/stream/hybrid), object streams, **repair**, all filters, standard security handler R2–R6 | `wide` corpus: 0 panics/timeouts; opens ≥ 99.5% of what qpdf opens |
| **M2** | Rendering core | content interpreter, display list, graphics state, paths, clipping, all fonts, images, all color spaces, shadings 1–7, patterns, transparency/blend/soft masks, OCGs, annotation appearances | ≥ 95% consensus pass on `regress`; bench within 2× PDFium |
| **M3** | Text | extraction, reading order, search, selection geometry, ToUnicode/ActualText, structure tree | text metric parity with pdf.js on corpus |
| **M4** | Engine API + Reader apps | command protocol, validation, sessions, jobs/cancel, tiles; **web reader** + **macOS reader** (XPC) | all reader features of §4; E2E green; first-page/fps targets met |
| **M5** | Annotations + basic save | all types + appearance generation, threads, flatten, XFDF; **minimal incremental save** (pulled forward so annotations persist) | journeys J-ANN-* green (annotate → save → reopen, renders in MuPDF/Poppler) |
| **M6** | Writer & pages | full rewrite, Save As, object streams, page ops, merge/split, encrypt/permissions | `qpdf --check` clean on all outputs; re-render equal |
| **M7** | Forms | fill/create/flatten, safe built-in calculations | form corpus round-trip; no JS executed |
| **M8** | Content editing | text/image edit, add text/images/links, watermark, header/footer, Bates, crop, redaction, compress | redaction verified by text-extraction + render; compress ratio vs qpdf/gs reported |
| **M9** | Signatures | verify (all PAdES levels, MDP), sign via platform signer, LTV/DSS, timestamps | external validators accept our signatures; known-bad corpus rejected |
| **M10** | Hardening & 1.0 | long fuzz campaign, perf tuning, accessibility, localization scaffolding, sandbox/notarization, PWA offline, CSP | 1.5× PDFium target; 30-day fuzz with 0 open crashes; release |
| **P2** | Phase 2 | OCR, PDF→Office, AI chat, iOS, Office→PDF, Android | scoped at the start of P2 |

---

## 9. Key risks

| Risk | Mitigation |
|---|---|
| Real-world PDFs break the spec constantly | repair/recovery mode is in M1, fed by the `wide` corpus and engine comparison |
| Text editing in existing PDFs is hard (subset fonts, kerning) | font substitution with a visible warning; spec explicitly scopes "good enough" |
| Pure-Rust codec gaps (JBIG2, JPX quality/perf) | trait boundary lets us switch or write our own without touching callers |
| WASM performance / threads need cross-origin isolation | host with COOP/COEP headers (Cloudflare Pages / Netlify); fall back to multiple workers |
| AI agents copying code from AGPL engines | rule in CLAUDE.md; reference engines are used only as external programs; similarity check in review |
| Disagreement between reference engines | consensus + "disputed" list + spec-based triage |
| Corpus licensing | never committed; manifest + fetch script only |
