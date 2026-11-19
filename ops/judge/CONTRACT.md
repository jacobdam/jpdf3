# Judge contract (PROTECTED)

The judge grades jpdf **only** through these interfaces. The contract is fixed for the life of
the project: implement against it; never ask the judge to change.

## 1. `jpdf-cli` (package `jpdf-cli`, built with `cargo build --release --locked -p jpdf-cli`)

The binary is `target/release/jpdf-cli`. Until it exists, every item counts as "missing"
(not opened, but no crash).

### Exit codes (every command)
| code | meaning | graded as |
|---|---|---|
| 0 | success | ok |
| 2 | clean, typed error (invalid/corrupt/unsupported input, wrong password) | allowed, not a pass |
| 3 | command or feature not implemented yet | allowed, not a pass |
| anything else, a signal, or a timeout | **crash** | **always fails the verdict** |

Each invocation runs with a time limit (info 20 s, render 30 s, other commands 60 s) and a
2 GiB address-space limit. Running out of either is a crash. Diagnostics go to stderr;
stdout carries only the documented output.

### Coordinates
Pages are 1-based. Rectangles are `x0,y0,x1,y1` in **view space**: PDF points (1/72 in),
origin at the **top-left of the rendered page**, after applying CropBox and /Rotate. This is
the same space as a 72-dpi render's pixel grid.

### Commands
```
jpdf-cli info FILE [--password PW]
    stdout: {"pages": N}

jpdf-cli render FILE --page N --dpi D --out OUT.png [--password PW]
    writes an 8-bit RGB or RGBA PNG, size = round(view width/height × D/72); transparent = white

jpdf-cli text FILE --page N [--password PW]
    stdout: the page's text, UTF-8, in reading order

jpdf-cli annotate FILE --out OUT --page N --kind highlight --rect x0,y0,x1,y1 --color R,G,B --save incremental
    adds a Highlight annotation with an appearance stream; OUT = FILE bytes + incremental update

jpdf-cli rewrite FILE --out OUT --mode full
    full rewrite of the current document state (no incremental history)

jpdf-cli encrypt FILE --out OUT --user-password U --owner-password O
    standard security handler, AES-256 (V5/R6)

jpdf-cli fill FILE --out OUT --field FULLNAME --value VALUE
    sets a text field's value, regenerates its appearance, NeedAppearances not set to true

jpdf-cli redact FILE --out OUT --page N --rect x0,y0,x1,y1
    TRUE redaction: content inside rect is removed from the document (text, image pixels,
    vector paths, annotations, form values) and from every earlier revision; the area is
    painted black; content outside the rect is preserved

jpdf-cli sign FILE --out OUT --p12 P12 --p12-password PW --field NAME
    PAdES B-B signature (incremental, covers the whole document) with a new signature field

jpdf-cli verify-signatures FILE
    stdout: {"signatures": [{"field": NAME, "integrity": "valid"|"invalid", "covers_whole_document": bool}]}
```

## 2. Project tasks (`cargo xtask`)
The protected CI calls these. The agent implements and extends them freely.
```
cargo xtask ci linux --out DIR     # web build + Playwright E2E, Rust extras; exit 0 = green
cargo xtask ci macos --out DIR     # macOS app build + XCUITest E2E; exit 0 = green
cargo xtask nightly --out DIR      # long fuzz, bench, regress corpus; files issues itself
```
Each `ci` run writes `DIR/e2e.json`:
```json
{"results": {"<test-id>": "pass" | "fail" | "skip"}}
```
`cargo xtask ci linux` may also write `DIR/site/`: a static site (the web app and the metrics
dashboard). On every push to main it is deployed to GitHub Pages, with the judge's reports
copied to `site/judge/`.

Test IDs are the ones named in `specs/journeys/*.md` (`- test:` lines). A journey passes when
all of its test IDs report `pass`.

## 3. Fuzzing
`fuzz/` is a cargo-fuzz project. Nightly, the judge runs every target from `cargo fuzz list`
for up to 5 minutes each (60 minutes total). Any crash artifact fails the night and opens a `crash` issue.

## 4. Journeys (`specs/journeys/*.md`)
```
## J-AREA-NN: title
- milestone: M5
- platform: web, macos
- test: e2e.web.ann_highlight_persists, e2e.macos.ann_highlight_persists
- superseded_by: J-AREA-MM          (optional)
```
IDs never disappear (append-only). See CONSTITUTION 15.
