# Backend traits (jpdf-traits)

Area: core (`crates/jpdf-traits`)
ISO 32000-2 clauses: §7.4.4 (Flate), §7.4.6 (CCITTFax), §7.4.7 (JBIG2), §7.4.8 (DCT),
§7.4.9 (JPX), §7.6.3–§7.6.4 (encryption algorithms), §8.9.3 (sample data), §9.2.4 (glyph
space), §9.6.5 (built-in encodings), §9.9 (embedded font programs), §8.3.3 (transforms), §8.4.3 (line
width, caps, joins, miter limit, dashes), §8.5.3 (painting, fill rules, clipping), §8.9.4 (image
space), §11.3 (compositing, blend modes), §11.4.7 (isolated groups), §11.6.5 (soft masks),
Annex C (limits)
Journeys: none directly (an internal boundary; every rendering, saving and security journey
depends on it)
Status: implemented (part 1 of 4: #14; part 2, font programs: #53, suite tightened in #68; part 3, rasterizer: #54).
Text shaping: #66. ICC: #70. Signatures: #55.

## Behavior
`jpdf-traits` defines the replaceable-backend boundary of PLAN §3. Third-party crates are
used only through these traits (CONSTITUTION 11), via adapters in `jpdf-backends`. The crate
has no implementations and no dependencies. ADR 0004 records where `Limits` lives and why
the error type is hand-written.

**Rules for every trait** (later parts must follow them too):
1. Every call that produces data takes `&DecodeLimits` (font calls: `&FontLimits`,
   rasterizer calls: `&RasterLimits`) and checks it **before allocating**.
   Output larger than `max_output_bytes`, or an image header above `max_image_pixels`,
   `max_image_dimension` or `max_image_components`, fails with
   `BackendError::LimitExceeded`. The only exception is `HashState`, whose output has a fixed
   size.
2. Every call returns `Result<_, BackendError>`, whatever the input, and never panics
   (CONSTITUTION 5). `BackendError` has five variants: `Malformed`, `Truncated`,
   `LimitExceeded`, `Unsupported` and `InvalidArgument`. `InvalidArgument` means the caller
   passed a bad parameter (key length, `Columns` 0), not that the stream is bad.
3. Decoders read leniently (CONSTITUTION 17). When the input ends early they return what
   they decoded with `DecodeStatus::Truncated`, or `BackendError::Truncated` if they decoded
   nothing. `DecodeStatus::Complete` means the stream ended where its format says it ends.
4. Encoders write strictly. For example, `Deflate` with `FlateFormat::Zlib` must produce a
   valid RFC 1950 header without FDICT and a correct Adler-32 trailer.
5. Backends return pixels as the codec defines them. Mapping to PDF samples (Decode arrays,
   `BlackIs1`, the image dictionary's colour space, Adobe CMYK inversion) happens in
   `jpdf-filters`, so swapping a backend can't change how a page looks.
6. Every trait has a suite in `jpdf_traits::conformance`. Each adapter calls it from a
   test and asserts `report.passed()`. A suite catches panics and reports them as failures.

**Limits.** `Limits` (PLAN §2.2) moved from `jpdf-syntax` into `jpdf-traits`;
`jpdf-syntax` and `jpdf-engine` re-export it, so callers are unchanged. New fields and their
defaults:

| Field | Default | Why |
|---|---|---|
| `max_decoded_bytes_per_stream` | 256 MiB | One backend call's output; fits a 32-bit `usize` (wasm32) |
| `max_decoded_bytes_per_document` | 4 GiB (`u64`) | Lifetime total charged by the engine; time bound against many small bombs |
| `max_image_pixels` | 100,000,000 | Header check before any allocation; binds for 1–2 byte pixels, while the 256 MiB output cap binds first for RGB/CMYK |
| `max_image_dimension` | 2^20 | Long strips stay legal; the pixel limit does the real bounding |
| `max_image_components` | 33 | ISO 32000-2 Annex C: DeviceN has at most 32 colorants, plus one alpha channel (`SMaskInData`) |
| `max_font_program_bytes` | 64 MiB | Large CJK fonts are 20–30 MB; checked before parsing |
| `max_font_glyphs` | 65,535 | Glyph ids are 16-bit in TrueType, CFF and CFF2 |
| `max_outline_points` | 131,072 | One glyph, composites and subroutines expanded. Counted per segment (implied TrueType points and 3 per cubic count), so a simple glyph within `maxp`'s 65,535 raw points can exceed 65,536 |
| `max_outline_ops` | 1,048,576 | One outline call: charstring operators executed plus composite components visited. Bounds CPU for subroutine fan-out and empty-component loops, which emit no points |
| `max_canvas_pixels` | 16,777,216 (2^24) | One render target is one tile (PLAN §3: `jpdf-render` renders tiles), e.g. 4096 × 4096; a 64 MiB RGBA buffer |
| `max_path_segments` | 1,048,576 | Segments in one path, and dashes one dashed stroke may produce (a dash array of tiny lengths otherwise turns one segment into millions) |
| `max_clip_depth` | 64 | Nested clips; each is one 8-bit mask the size of the canvas |
| `max_layer_depth` | 32 | Nested transparency groups; each is one RGBA buffer the size of the canvas |
| `max_raster_working_bytes` | 1 GiB | `pixels × (4 + S) + 4 × I`, half of the 2 GiB process budget. `S` is the most bytes per pixel the clip/layer stack holds at once (1 per clip mask, 4 per layer buffer, 5 per layer with a soft mask, counting the adapter's copy of the mask); `I` is the largest image's pixel count (the adapter's premultiplied copy). Buffers the caller owns (the display list's images) aren't counted, so a maximum-size image plus a deep stack on a full tile needs a smaller tile. Depth and canvas size trade off: deep nesting needs smaller tiles, which the engine can choose |

`Limits::decode_limits(remaining_document_bytes)` builds the `DecodeLimits` for one call.
Its output cap is the smaller of the per-stream limit and what the document has left.
`Limits::font_limits()` builds the `FontLimits` for font calls. Font output is structured
(glyphs, outline segments), so it is bounded by counts instead of bytes.
`Limits::raster_limits()` builds the `RasterLimits` for rasterizer calls.

**Traits in this part.**
- `Inflate::inflate(input, FlateFormat, &DecodeLimits) -> Decoded<Vec<u8>>`.
  - The output buffer never grows beyond the cap; working memory (window, tables) is
    bounded by a constant.
  - `FlateFormat::Zlib` is FlateDecode (§7.4.4.1). A missing Adler-32 trailer counts as
    truncated. A wrong Adler-32 may be ignored, but must not change the bytes.
  - `FlateFormat::Raw` (bare RFC 1951) exists so `jpdf-filters` can recover files whose
    zlib header is missing.
- `Deflate::deflate(input, FlateFormat, CompressionLevel, &DecodeLimits) -> Vec<u8>`.
- `JpegDecoder::decode(input, &JpegOptions, &DecodeLimits) -> Decoded<DecodedImage>`.
  - `JpegOptions::color_transform` is DCTDecode's `/ColorTransform` (§7.4.8). `Some(true)`
    converts YCbCr/YCCK back to RGB/CMYK; `Some(false)` returns components as stored, which
    then are RGB/CMYK; `None` lets the Adobe APP14 flag decide (otherwise 3-component images
    are converted). 1/3/4 components give `Gray`/`Rgb`/`Cmyk` unless an APP2 ICC profile
    gives `Icc`.
  - `DecodedImage::adobe_app14` carries the APP14 transform flag, so `jpdf-filters` can
    apply the Adobe CMYK inversion without parsing JPEG markers itself.
- `Jpeg2000Decoder::decode(input, &DecodeLimits) -> Decoded<DecodedImage>`.
  - The input is a JP2 file or a raw codestream (§7.4.9).
  - Samples are unscaled (a 4-bit component holds 0–15), so an `/Indexed` override in the
    image dictionary still works; `source_bits` gives the codestream depth.
  - Subsampled components are upsampled by pixel replication; signed components get
    `2^(source_bits - 1)` added.
  - Components with different depths fail with `Unsupported`. This is a **known divergence**
    from consensus engines (CONSTITUTION 17), tracked by #57; it needs a per-component depth.
  - The suite requires a fixture with `source_bits` below 8 and checks that every sample is
    below `2^source_bits`, so an adapter that rescales samples fails.
  - An opacity channel comes back as the last component with `has_alpha` set (`SMaskInData`,
    §8.9.5).
- `Jbig2Decoder::decode(input, globals, &DecodeLimits) -> Decoded<Bitmap>`. The input is the
  embedded stream format of §7.4.7: segments, no file header, one page.
- `CcittDecoder::decode(input, &CcittParams, &DecodeLimits) -> Decoded<Bitmap>`.
  - `CcittParams` carries the Table 11 keys except `BlackIs1`, with the Table 11 defaults.
  - `columns == 0` fails with `InvalidArgument`.
  - `columns` or `rows` above `max_image_dimension` fails with `LimitExceeded` before
    decoding.
- `DecodedImage`: rows top to bottom, interleaved components at full resolution, values not
  rescaled. The container is one byte per sample for `source_bits` 1–8 and two bytes
  (big-endian) for 9–16, the §8.9.3 layout without row padding. `expected_len()` computes
  the length with checked arithmetic and rejects a container that doesn't fit
  `source_bits`.
- `Bitmap`: **1 = black**, the JBIG2/CCITT convention. Rows are padded to bytes with 0 bits,
  most significant bit first.
- `Hasher` / `HashState`: MD5, SHA-1, SHA-256/384/512, both incremental and one-shot
  (`digest`). Every algorithm is required.
  - MD5 is used for revisions 2–4 (§7.6.4.3).
  - SHA-2 is used for revision 6 (Algorithm 2.B) and for signatures.
  - SHA-1 is needed to verify old signatures.
- `Cipher::rc4(key, data, &DecodeLimits)`: keys of 1–256 bytes.
- `Cipher::aes_cbc(Direction, key, iv, data, Padding, &DecodeLimits)`: 16- or 32-byte keys.
  - With `Padding::Pkcs7` (the "PKCS#5" padding of §7.6.3), decryption checks every padding
    byte; bad padding fails with `Malformed`.
  - The cap applies to the output when encrypting and to the input (`data`, an upper bound
    of the plaintext) when decrypting, so both are checked before allocating.
  - `Padding::None` serves Algorithm 2.B, the revision 6 key unwrapping, and lenient
    readers that strip broken padding themselves.

**Traits in part 2 (font programs).** Selecting a glyph for a PDF string (encodings,
`Differences`, CMaps, the §9.6.5.4 `cmap` rules) stays in `jpdf-fonts`, so swapping a backend
can't change which glyph a page shows. Backends only answer questions about the program.
- `FontEngine`, `Type1Parser` and `FontFace` are `Send + Sync`: faces are long-lived, cached
  per document and shared across rendering threads.
- `FontEngine::load(data, FontProgram, &FontLimits) -> Box<dyn FontFace + 'a>`.
  `FontProgram::Sfnt` is `FontFile2` or OpenType `FontFile3` (`glyf`, `CFF ` or `CFF2`
  outlines; no collections). `FontProgram::BareCff` is `FontFile3` `Type1C`/`CIDFontType0C`.
  The face may borrow `data`.
- `Type1Parser::load(data, &FontLimits) -> Box<dyn FontFace + 'a>` for `FontFile`. It finds
  the cleartext and `eexec` parts by content, not by `Length1`–`Length3`, which real files
  often get wrong (CONSTITUTION 17).
- Both loads fail with `LimitExceeded` when `data` is longer than `max_font_program_bytes`
  (checked before parsing) or the face has more than `max_glyphs` glyphs.
- `FontFace` (all values in glyph space; every method total):
  - `outline_format()`: TrueType, CFF, CFF2 (default instance) or Type 1.
  - `glyph_count()`: at least 1 (`.notdef`), at most `max_glyphs`.
  - `font_matrix()`: `[1/upem, 0, 0, 1/upem, 0, 0]` for sfnt, `FontMatrix` for Type 1 and
    CFF (the Top DICT's; §9.2.4). Finite, with a normal (not subnormal) determinant. In a
    CID-keyed CFF whose FDArray dictionaries have their own `FontMatrix`, the adapter maps
    each glyph's outline and advance into this matrix's glyph space, so one matrix serves
    every glyph.
  - `metrics()`: bounding box, plus ascender and descender when the program has them.
  - `advance(glyph)` and `outline(glyph, &FontLimits)`. A glyph id not below `glyph_count`
    gives `InvalidArgument`. An outline over `max_outline_points` gives `LimitExceeded`.
    Points are counted per segment: `MoveTo`/`LineTo` 1, `QuadTo` 2, `CurveTo` 3, `Close` 0.
    TrueType implied on-curve points become explicit `QuadTo` end points. An outline that
    needs more than `max_outline_ops` operations also gives `LimitExceeded`. Recursion in
    composite glyphs or subroutines is an error, never a stack overflow. Outlines are well
    formed: every contour starts with `MoveTo` and ends with `Close`. The returned `Vec` is
    bounded by the point limit. A caller-supplied sink was considered; it would let the
    caller count points itself, but the work bound has to live in the interpreter anyway,
    and the suite checks the count on every outline it sees.
  - `cmap_subtables()` and `cmap_lookup(CmapId, code)`. Glyph 0 counts as unmapped.
    `Some(GlyphId(0))` is never returned. `jpdf-fonts` needs (3,1), (3,0) and (1,0) for
    §9.6.5.4.
  - `glyph_name(glyph)` (at most 127 bytes, the PostScript limit) and `glyph_by_name(name)`,
    for `Differences` and text extraction.
  - `cid_to_glyph(cid)`: the charset of a CID-keyed CFF, bare or as an OpenType `CFF `
    table; `None` for every other program.
  - `builtin_encoding()`: `None` (sfnt, CID-keyed), `Standard`, `Expert`, or `Custom` (code,
    glyph name) pairs sorted by code with unique codes. Names rather than glyph ids, because
    Type 1 encodings are name-based. `jpdf-fonts` owns the Standard and Expert tables.
  - Every number returned is finite. Every glyph id returned is below `glyph_count`.
- Advances and coordinates are `f32`: CFF and Type 1 charstring arithmetic is fractional,
  and IEEE `f32` arithmetic gives the same results natively and on wasm32.

**Traits in part 3 (rasterizer).** `jpdf-render` turns a page into a display list in device
space and hands it to a `Rasterizer`. Everything PDF-specific is resolved before this boundary:
colour spaces (colours arrive as device RGB), shadings and patterns (as images or paths), text
(as glyph outlines), soft-mask luminosity (as a ready mask) and knockout groups. So swapping a
backend changes only anti-aliasing and rounding, never what a page shows.
- `Rasterizer::render(width, height, &[DrawCommand], &RasterLimits) -> Pixmap`. Stateless,
  `Send + Sync`.
  - Device space: origin at the top-left, x right, y down; pixel `(i, j)` covers
    `[i, i+1) × [j, j+1)`. The canvas starts transparent.
  - `Pixmap`: 8-bit RGBA, **premultiplied**, rows top to bottom, no padding. Premultiplied is
    what compositing produces and what tiles are composited with. The bindings convert for
    web `ImageData` (straight alpha).
- `DrawCommand`s:
  - `Fill { path, rule, transform, paint }` and `Stroke { path, style, transform, paint }`.
    Paths reuse `PathSegment`, in user space. Each subpath starts with `MoveTo`; `Close` is
    optional, and only `MoveTo` may follow it. Fills close open subpaths; strokes don't.
    `Transform` is a PDF matrix `[a b c d e f]` (§8.3.3) to device space. For strokes it scales
    the pen too.
  - `Paint`: a `Color` (device RGB, straight alpha, each component in `0..=1`; the constant
    alpha `CA`/`ca` is folded into alpha), a `BlendMode` and `anti_alias`. With
    anti-aliasing a pixel's coverage is its covered area; without, its centre decides.
  - `StrokeStyle` (§8.4.3): width ≥ 0 (0 is the thinnest visible line), caps, joins, a miter
    limit ≥ 1 (beyond it a miter bevels, §8.4.3.5), and a dash array (empty is solid;
    non-negative, not all zero; odd length repeats) with a phase. A negative phase, or one
    beyond the period, is reduced modulo the period of the expanded pattern (`[4]` has period
    8). A zero-length dash with round caps is a
    dot (§8.4.3.6).
  - `Image { image, transform, interpolate, opacity, blend }`. The image is 8-bit RGBA with
    straight alpha, colour and soft mask already applied. As in §8.9.4 the transform maps the
    unit square, and the first row maps to `v = 1`. A 1:1, pixel-aligned draw reproduces the
    source with either `interpolate` value. So the smoothing filter must interpolate
    (bilinear, or an interpolating bicubic such as Catmull-Rom), not blur (Mitchell).
  - `PushClip { path, rule, transform, anti_alias }` / `PopClip`: clips intersect.
  - `PushLayer { opacity, blend, mask }` / `PopLayer`: an isolated, non-knockout group
    (§11.4.7). Content draws into a transparent buffer, which then composites onto the
    backdrop. Group opacity applies once, to the group's result; the optional `mask` holds one
    byte per canvas pixel and multiplies alpha (the luminosity or alpha of §11.6.5, computed
    by `jpdf-render`).
  - Clips and layers nest together; a pop must match the innermost push.
- Compositing is §11.3.6 source-over with the blend function `B` of §11.3.5 (all 16 modes,
  including the non-separable `Hue`, `Saturation`, `Color` and `Luminosity`), in the device's
  encoded RGB values. Over a transparent backdrop every mode gives the source.
- **Known divergence:** non-isolated groups (§11.4.8) aren't represented. For groups that use
  only `Normal` inside, isolated rendering gives the same pixels. Groups with other blend
  modes inside differ, because their content should blend with the backdrop. Tracked by a
  discovery issue for `jpdf-render` (#72).
- `check_scene(width, height, commands, &RasterLimits) -> SceneStats` validates a list and
  enforces the limits without drawing. Adapters call it first, so the strict checks live in
  one place. It returns:
  - `InvalidArgument` for a zero-size canvas, any non-finite number, a colour, opacity or
    mask out of range, a negative width, a miter limit below 1, a bad dash array, an image
    or mask whose length doesn't match, a malformed path, a mismatched pop, or a push left
    open;
  - `LimitExceeded` for canvas pixels, segments in one path, clip or layer depth, or working
    bytes over the limit.
  The dash count depends on geometry, so the adapter enforces it. The count covers the
  **whole** path, on and off the canvas. The adapter computes it from the path length and
  the dash period before generating any dash, so the bound holds up front.
- Time isn't bounded by `RasterLimits`: a million full-canvas fills pass `check_scene`. As
  for every backend call, the engine's job budget (PLAN §2.2) bounds total time. `render`
  can't be cancelled, so `jpdf-render` must keep each call within the budget, for example by
  capping the commands per call (#73). What must be bounded here is the work for **one**
  command. Counting dashes over the whole path means every tile pays for dashing the whole
  path. Long dashed CAD paths at high zoom can hit the limit even when most of the path is
  off the tile; that is the accepted cost of a bound known up front.
- Geometry must stay bounded and exact for any finite input. No error describes a hang;
  the suite's huge-value scenes catch one through the test timeout. Adapter duties:
  - flatten curves after clipping to the canvas, or cap subdivision;
  - clip fills and clips geometrically before a backend with a coordinate range sees them
    (tiny-skia skips paths beyond about ±2^29), so a ±1e30 rect still covers the canvas;
  - a stroke wider than the canvas diagonal still paints its outline (fill the clipped
    outline instead);
  - device coordinates can overflow to infinity after transforming even though the list is
    finite: treat such a draw as empty or `LimitExceeded`, never panic;
  - dash loops: at lengths near 1e30, adding a dash length to a position no longer changes
    it, so a naive loop never ends; a phase of 1e30 hangs a subtract-the-period loop.
- Stroke adjustment (§10.7.5) is out of scope: `jpdf-render` snaps hairlines itself.

**Conformance suites** (`jpdf_traits::conformance`). Each returns a `Report` listing every
failed check by a stable name:
- `flate::run(&dyn Inflate, &dyn Deflate)`:
  - Built-in zlib vectors (empty, short and long fixed Huffman, stored, dynamic Huffman) in
    both formats.
  - Every strict prefix of each vector (sampled to about 256 cuts plus the last 16 bytes) is
    an error or a truncated **prefix** of the plaintext, never `Complete`.
  - Salvage (lenient reading): a zlib stream missing only (part of) its Adler-32 trailer
    gives `Ok(Truncated)` with the whole plaintext; a long vector (≥ 64 bytes of plaintext)
    cut in its last 16 deflate bytes gives `Ok(Truncated)` with a non-empty prefix.
  - Each vector with one byte flipped at about 256 positions: no panic, output within the cap.
  - Exact output limit (`len` passes, `len - 1` is `LimitExceeded`).
  - A wrong Adler-32 doesn't change the bytes.
  - Round trips of 5 inputs × 3 levels × 2 formats, with the zlib framing checked.
  - Deflate's own output limit.
  - Zip bombs under a 64 KiB cap must give `LimitExceeded`: 16 MiB of zeros compressed by
    the adapter, and 1 GiB of zeros as a 6.5 MiB fixed-Huffman stream (literal 0, then
    length-258/distance-1 copies) generated by the suite.
- `image::run_jpeg / run_jpeg2000 / run_jbig2 / run_ccitt(&decoder, &fixtures)`:
  - At least one fixture is required, and the fixtures must include both a landscape and a
    portrait image (so a decoder checking only one axis against `max_image_dimension`
    fails). Adapters supply files **we** generate, with the pixels from a reference decoder
    (CONSTITUTION 18: never corpus files).
  - Each fixture decodes `Complete`, matching its expected header (size, components,
    container, `source_bits`, colour, alpha, `adobe_app14`) exactly and its pixels within the
    fixture's per-sample tolerance.
  - Output, pixel, dimension and component limits each pass at the fixture's exact size and
    give `LimitExceeded` one below it.
  - Truncated and mutated inputs never panic, and any output has a length that matches its
    header and stays within the limits.
  - Empty input is an error.
  - JBIG2: truncated globals are checked the same way. CCITT: `Columns` 0 must give
    `InvalidArgument`.
- `crypto::run_hasher(&dyn Hasher)`:
  - Known answers for every algorithm: `""`, `"abc"`, the FIPS 180-4 448-bit and 896-bit
    messages (multi-block chaining), and a fixed pattern at lengths 55, 56, 63, 64, 111, 112,
    127, 128 and 1000 (both padding paths for 64- and 128-byte blocks). Plus SHA-256 of a
    million `a`.
  - Digest lengths.
  - Streaming in 1-, 7-, 64- and 999-byte updates equals one-shot.
- `crypto::run_cipher(&dyn Cipher)`:
  - RC4 known answers, including RFC 6229 offset 0 for the 40-bit and 128-bit keys PDF
    uses; RC4 round trips (a smoke test only: RC4 is its own inverse).
  - SP 800-38A F.2.1/F.2.2/F.2.5/F.2.6 AES-CBC known answers.
  - PKCS#7 round trips for lengths 0–33 with exact padded lengths.
  - Bad padding gives `Malformed`: final byte 0x00, final byte 0x11, and a valid-looking
    final byte whose earlier padding bytes are wrong (`…, 3, 2, 3`).
  - The IV is used.
  - Bad lengths give `Malformed` (decrypt) or `InvalidArgument` (encrypt without padding).
  - Bad key lengths give `InvalidArgument`.
  - Exact output limits for RC4, AES encryption (output) and AES decryption (input).

- `font::run_font_engine(&dyn FontEngine, &fixtures, &hostile)` and
  `font::run_type1(&dyn Type1Parser, &fixtures, &hostile)`:
  - Fixtures are fonts **we** generate under a permissive license, with values from a
    reference tool. Each states the outline format, glyph count, font matrix, metrics,
    built-in encoding, and each listed glyph's advance and full outline. It can also list
    `cmap` lookups, names (checked both ways, for any glyph id; every listed name is looked up,
    plus up to 128 names the face returns) and
    CID lookups, plus glyphs with a known lower bound on the operations their outline needs
    (`min_ops`).
  - Outlines are compared segment by segment within the fixture's tolerance: same segment
    kinds, same order, same points. So reversed winding, permuted points and swapped control
    points all fail. A `LineTo` back to the contour start (within the tolerance) right before
    `Close` is dropped first, because backends differ on emitting it. Contours start at the
    program's first on-curve point. A TrueType contour with no on-curve point starts at the
    implied midpoint of its first two points. Fixture authors should prefer contours that
    start on-curve. The tolerance is in font
    units, must be finite and between 0 and 1, and also applies to advances and metrics. The
    font matrix is compared within 1e-4 times its largest expected entry, so a wrong
    units-per-em fails and a near-zero skew still compares sanely.
  - Engine coverage:
    - TrueType, CFF and CFF2 outlines, each with a curve (`QuadTo` for TrueType, `CurveTo`
      for the others);
    - every container and outline pairing a PDF can embed: sfnt with TrueType, sfnt with
      CFF (OpenType `CFF `), sfnt with CFF2, and bare CFF;
    - per container and outline pairing, a `min_ops` glyph with a bound of at least 100 (for
      TrueType, a composite: each component visited counts as one operation; for CFF, CFF2
      and Type 1, fixture authors should run most operators inside subroutines);
    - mapped `cmap` lookups in (3,1), (3,0) and (1,0);
    - a mapped CID;
    - a CFF custom encoding.

    Type 1 coverage: every fixture is Type 1, one has a `CurveTo`, one has a `Custom`
    encoding, and one has a `min_ops` glyph with a bound of at least 100. At least one
    fixture glyph must have points.
  - Hostile programs (at least one per container and outline pairing; Type 1 has one
    pairing): self-referencing
    composites, recursive subroutines, fan-out bombs.
    - With a control glyph, the program must load. The control glyph must outline (proof
      that the hostile path is reachable), and every listed glyph must fail with
      `LimitExceeded` or `Malformed`.
    - Without one, the load or every listed glyph must fail that way.
    - A program that loads must report the outline format it declares.
    - Listed glyphs must be in range (an out-of-range id fails trivially with
      `InvalidArgument`).
    - Coverage counts only programs that **prove** their format: at least one listed glyph,
      the program loads as the declared format, and its control glyph outlines, so the
      listed glyphs sit on a path the backend reaches. Whether they are rejected is the
      backend's verdict, reported by `font.hostile`, not part of the proof. A program without
      a control glyph may still be supplied (it must not panic and must be rejected as
      above), but junk that fails to load for a dull reason proves nothing, so it never
      satisfies coverage.
    - No panic, and the invariants hold. Unbounded recursion overflows the
    stack and aborts the test process, which fails the run without a report entry.
  - Each fixture loads and matches exactly. Glyph id `glyph_count` gives `InvalidArgument`
    from both `advance` and `outline`.
  - `max_font_program_bytes` and `max_glyphs` pass at the fixture's size and give
    `LimitExceeded` one below. `max_outline_points` passes at the backend's own point count
    for the largest fixture glyph and gives `LimitExceeded` one below. Operation counts are
    backend-specific, so `max_outline_ops` gets lower-bound checks instead of an exact
    boundary:
    - A Type 1, CFF or CFF2 glyph with points runs at least one operator, so with
      `max_outline_ops` 0 it must give `LimitExceeded`. This catches a backend that ignores
      the caller's value and relies on a built-in budget.
    - Each `min_ops` glyph (bound `n`, from 1 to the default `max_outline_ops`) outlines at
      the default limits and gives `LimitExceeded` with `max_outline_ops` `n - 1`. The
      required bound of 100 catches a backend that counts too little (one per glyph, nothing
      for TrueType composite components) and so passes the check at 0. The fixture author
      states a bound they can justify from the program (operators written, components
      visited); a loose bound is allowed but weakens the check.
    - The hostile programs cover the upper end.
  - Every strict prefix and every one-byte flip (about 144 positions each): no panic. A face
    that loads must keep the invariants under limits capped at the fixture's size. The
    suite queries the first 64 glyphs, the last one and two out-of-range ids; every `cmap`
    subtable (up to 16) for codes 0–255, U+FFFF, U+10000 and U+10FFFF; names; and CIDs 0–63.
    The invariants:
    - finite numbers and a matrix with a normal determinant;
    - returned glyph ids in range, and no glyph 0 from `cmap`;
    - out-of-range ids rejected;
    - glyph and point counts within the limits;
    - well-formed outlines;
    - names of at most 127 bytes;
    - a sorted custom encoding;
    - promised empty answers: no `cmap` subtables for a bare CFF or Type 1 program, `None`
      from `cmap_lookup` on a subtable `cmap_subtables` does not list (the suite also asks
      (3,1), (3,0), (1,0), (0,3) and (3,10) for codes 0–255 when they are not listed), and
      `None` from `cid_to_glyph` for every face that is not CFF.

    A fixture is also checked against the answers it promises by omission: every CID the
    suite asks about (0–63 and the listed ones) that the fixture does not list must give
    `None`, so a fixture that is not CID-keyed lists no CIDs, and a CID-keyed one lists every
    mapped CID below 64.
  - Empty input is an error for both containers.
  - **What the suites can't see.** Byte flips and prefixes are a smoke test, not coverage
    of charstring interpreters, `seac`, PFB segments or hex `eexec`. Deep coverage comes
    from per-adapter fuzz targets (#17) and from admission (#16), which must supply hostile
    programs for each format the adapter parses. Also out of reach: points summed across
    composite components (the TrueType `min_ops` composite proves components are visited,
    not that their points are summed against the limit), and the per-FD `FontMatrix`
    normalisation of CID-keyed CFF. Admission adds a multi-FD fixture whose FD matrices
    differ from the Top DICT's. The ops checks bound the count from below at three points
    (0, `n - 1`, the default), so a backend that rejects every caller value below some
    threshold and uses a built-in budget above it still passes. A backend that skips
    operators inside subroutines passes unless the `min_ops` glyph runs its operators there,
    and an uncounted fan-out bomb then hangs the run rather than failing a check.

- `raster::run(&dyn Rasterizer)`. It checks pixels only where every correct backend agrees:
  shapes on pixel boundaries, pixel centres with anti-aliasing off, area coverage within ±20
  of 255 with it on, and the §11.3 formulas within ±2 per channel. Scenes run on a 32 × 32
  canvas, except the canvas-limit checks (64 × 16, 16 × 64) and a 1 × 1 case. Every output is
  checked for size and premultiplied alpha (`raster.output`).
  - `raster.fill`: axis-aligned rects (exact, with and without anti-aliasing), scaled and
    rotated transforms, half alpha, a cubic circle, and a quadratic arch (a backend that
    treats `QuadTo` as a line fails).
  - `raster.fill_rule`: nested rects with the same and opposite winding under both rules.
  - `raster.anti_alias`: 3/4- and 1/4-covered pixels with and without anti-aliasing.
  - `raster.stroke`: butt, square and round caps (with corner probes, so a square cap fails
    the round check); miter, bevel and round joins with an 8-wide pen (probes off the bevel
    line); a miter over its limit; closed and open subpaths; a pen scaled by the transform; a
    hairline.
  - `raster.dash`:
    - `[4 4]` at phases 0, 2, 10 (wrapping), −2 and ±1e30 (exactly 0 mod 8);
    - `[4]` at phases 0 and 12 (period 8, not 4);
    - `[2 2]` at scale 2 (lengths are in user space);
    - hazards: `[1e30]` must give `Ok`; `[1e-30 1e-30]` and the smallest
      normal `f32` on a 32-unit line must give `LimitExceeded`.
  - `raster.clip`: one clip, a clip with its own transform, nested clips, a pop restoring the
    outer clip, an even-odd clip, and an anti-aliased clip edge.
  - `raster.layer`: layer opacity; group opacity applied once (blue over red inside a 50%
    group gives half-transparent blue); nested opacity; a soft mask with values 255, 128 and
    0.
  - `raster.blend`: all 16 modes as fill blend modes and as layer blend modes, against
    expected values the suite computes from the §11.3.5 formulas. Also: three modes over a
    transparent backdrop (which must give the source) and a half-transparent `Multiply`.
  - `raster.image`: a 3 × 2 image with a half-transparent and a clear pixel at 1:1 (both
    `interpolate` values; first row on top), at 4× nearest, with opacity, and with a blend
    mode.
  - `raster.invalid`: 38 invalid lists, each of which must give `InvalidArgument`. They
    include an image of `u32::MAX × u32::MAX` with no data, whose byte length overflows
    `u64`.
  - `raster.degenerate`: an empty list or path, zero area, a lone `MoveTo`, a singular
    transform, an empty clip, a ±1e30 rect (must cover the canvas, with and without
    anti-aliasing), curves with 1e30 control points, a 1e30-wide stroke, zero-length dashes
    with round caps (dots), a 1 × 1 canvas.
  - `raster.limits.*`: each limit passes at the scene's exact value and gives `LimitExceeded`
    one below:
    - canvas pixels (square, landscape and portrait);
    - segments in a fill, a stroke and a clip path;
    - clip depth and layer depth, plus sequential pushes at depth limit 1 (counting pushes
      instead of depth fails);
    - working bytes for 2 layers and 3 clips.

    Dashes get a lower-bound check (1000 dashes pass with limit 2000 and fail with 100),
    because backends may count a dash on a boundary differently. Most of that line is off
    the canvas, so an adapter that counts only visible dashes fails, as the count rule
    requires.
  - `raster.hostile`: 48 pseudo-random valid lists built from extreme values (±1e30, 1e-30,
    the smallest normal `f32`, and values on pixel edges). Each has transforms, clips, layers
    and strokes; in half of them the dash entries and phase are drawn from the same values.
    Each list must give `Ok` or `LimitExceeded`, never panic.
  - **What the suite can't see.** It checks the rounding where backends agree, not the
    anti-aliasing quality. It can't see allocations (the working-bytes formula is a nominal
    bound), or a hang except through the test timeout. Gradients don't cross this boundary.
    The fuzz target and admission (#16) cover deeper geometry.

Images have no "a truncated prefix is never `Complete`" rule, unlike Flate. Some JBIG2 and
CCITT streams have valid end points in the middle, so a prefix can be a complete, smaller
image.

## Persistence
Not applicable: the traits hold no document state. Swapping a backend must not change saved
output beyond what `Deflate` levels allow. The Flate round-trip checks enforce this for
compression.

## Undo
Not applicable (no user-visible state).

## Errors
Errors are `BackendError` values. `jpdf-filters` and `jpdf-crypt` map them to their own
typed errors, and the engine maps those to user messages:
- `LimitExceeded` → "This file is too large or complex to open safely" for the affected
  object.
- `Malformed` and `Truncated` → the object is shown damaged (for example a blank image).
  The page still renders.

A `DecodeStatus::Truncated` result is not an error. The caller uses the partial output and
may record a repair note.

## Limits
- `DecodeLimits` bounds every call's output: at most 256 MiB by default, less once the
  document budget runs low.
- Image headers are checked against pixel, dimension and component limits before any
  allocation.
- `DecodedImage::expected_len` and `Bitmap::expected_len` use checked arithmetic, so a
  hostile header can't overflow the length.
- Time is not bounded here. The engine's job budget (PLAN §2.2) and nightly fuzzing cover it.
- **What the suites can and can't see.** They check the returned variant, not allocations.
  "Check before allocating" is enforced indirectly: the 1 GiB bomb makes a decode-then-check
  adapter allocate over 1 GiB (an out-of-memory failure or a very slow test). The fuzz
  targets (#17) and the resource-limit harness (#25) measure it directly. That harness uses
  a 2 GiB `RLIMIT_AS` and must run on Linux: macOS does not enforce `RLIMIT_AS`.

## Security
- Backends see hostile bytes. The suites feed them empty, truncated and mutated input and
  catch panics, so an adapter that panics or returns more than a cap can't pass (see
  Limits for what they can't observe).
- Fuzz targets per trait come with #17. Font adapters add theirs at admission (#16).
- Display lists come from the engine, but their numbers derive from hostile content streams.
  `check_scene` rejects every non-finite or out-of-range value, with checked arithmetic on
  every caller-supplied size. The limits bound nominal memory before any buffer is
  allocated. A dash array of tiny lengths, or coordinates near 1e30, must not turn one
  command into unbounded work. Total time is the engine's job budget.
- Font programs are a classic attack surface (charstring interpreters, composite recursion).
  The face API never hands out raw tables. The suite checks every answer it gets from a
  damaged program against the invariants, and requires hostile programs; see Conformance
  suites for what it can't see.
- Keys passed to `Cipher` are secrets: backends must not log or keep them. Zeroizing them
  needs a dependency and is deferred to admission (#16).
- MD5 and RC4 exist only because the PDF standard security handler requires them for old
  files. `jpdf-write` must not choose them for new encryption (enforced in `jpdf-crypt`
  when it lands).

## Accessibility
Not applicable (no UI).

## Platforms
Identical on macOS (native) and the web (wasm32). Defaults fit a 32-bit `usize`.
`max_decoded_bytes_per_document` is a `u64` for that reason. Suites rely on
`catch_unwind`, so they run natively (`cargo nextest`), not under wasm `panic=abort`.

## Performance
- Traits take slices and return owned buffers. Copy-free streaming can be added later as a
  provided method without breaking adapters.
- The raster suite takes about 3–7 s against the toy rasterizer in a debug build. The toy
  supersamples 4 × 4 and tests winding numbers per sample. A real backend is much faster.
- The font suites take well under a second against the toy font. A fixture costs about 300
  loads: up to 144 prefixes, up to 144 flips and a few limit loads.
- A full Flate run takes about 1.5 s for the toy codec in a debug build (generating the
  1 GiB bomb's 6.5 MiB stream and the 16 MiB deflate dominate); the other suites take well
  under a second.

## Tests
- `crates/jpdf-traits`: unit tests for `Limits::decode_limits`, `DecodedImage::expected_len`,
  `Bitmap::expected_len`, Adler-32 and the vectors.
- Each suite is tested against a correct toy implementation that must pass, and against
  broken ones whose failing checks must be exactly the expected set:
  - `flate`: a stored + fixed-Huffman codec; variants that ignore the cap (caught by the
    output-limit check and both bombs), panic on empty
    input, call truncated input complete, or drop partial output. The full `run` must fail
    it only on the dynamic-Huffman vector. The bomb generator is checked to decode to zeros
    (and was checked against CPython's zlib when written).
  - `image`: a toy raster codec for all four decoders; variants that ignore limits, check
    only the width, rescale 4-bit samples, panic on mutated input, or return output shorter than its header. Also
    a wrong pixel inside and outside the tolerance, each header field mismatch, a bad
    fixture (including a sample at or above `2^source_bits`), a single orientation, a JPX
    fixture set without a sub-8-bit image, and 16-bit tolerance.
  - `crypto`: table-backed and FNV hashers; real RC4 plus a fake CBC block cipher, with
    variants that ignore the IV, accept bad padding, check only the last padding byte, or
    apply the decryption cap to the plaintext.
  - `font`: a toy text-format font for every format. Broken variants:
    - ignore any of the four limits (the ops limit through a hostile `loop` glyph), or use
      a built-in ops budget instead of the caller's;
    - count one operation per glyph, or nothing for composite components;
    - map CIDs by identity, fall back to identity on unknown `cmap` subtables, or report a
      `cmap` subtable for every program;
    - panic on empty or mutated input;
    - accept out-of-range glyph ids;
    - return an out-of-range glyph or glyph 0 from `cmap`;
    - return a NaN font matrix or a wrong units-per-em;
    - drop `Close`;
    - ignore a custom encoding.

    Also tested: each wrong expected value (advance, reversed contour, moved point, metrics,
    name, CID); invalid fixtures (glyph out of range, tolerance 1.5 or NaN, malformed
    outline); dropping each required fixture, `cmap` subtable or CFF custom encoding; a
    TrueType fixture without a `QuadTo`; missing hostile programs or a missing format among
    them; and weak hostile programs (an out-of-range glyph, a program that fails to load
    for a dull reason, a failing control glyph, no glyphs listed, a mislabelled format, and
    junk without control glyphs standing in for every format). Dropping the sfnt-CFF
    fixture, any `min_ops` glyph, or only the sfnt-CFF heavy glyph or hostile program fails
    coverage; a `min_ops` bound of 0, out of range or above the default limit is an invalid
    fixture; names on glyphs past 64 and more than
    128 names pass against the correct toy.
  - `raster`: a 4 × 4-supersampling toy with f64 winding numbers and strokes built from
    polygons. It must pass. Each broken variant must fail exactly the expected checks:
    - ignores limits; skips validation (and panics on NaN);
    - ignores anti-aliasing;
    - applies layer opacity per object;
    - blends without the `(1 − αb)` term;
    - ignores the miter limit, the even-odd rule or the dash phase;
    - flips images;
    - returns straight alpha;
    - counts nested clips as one level.

    A panicking rasterizer is reported, not propagated. The blend formulas are checked
    against values computed by hand.
  - `check_scene`: path validity, the working-bytes formula (stack peak, masks, images),
    huge image dimensions giving `InvalidArgument` instead of an overflow panic, depth
    counters that never saturate silently, pixel reads bounds-checked.
- Unit tests for `Outline::point_count`, `control_box`, `is_well_formed`, `is_finite` and
  `Limits::font_limits`.
- Existing `jpdf-syntax`/`jpdf-engine` tests cover the moved `Limits` unchanged.
- No E2E IDs (no UI).
