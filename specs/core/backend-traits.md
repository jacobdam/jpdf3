# Backend traits (jpdf-traits)

Area: core (`crates/jpdf-traits`)
ISO 32000-2 clauses: §7.4.4 (Flate), §7.4.6 (CCITTFax), §7.4.7 (JBIG2), §7.4.8 (DCT),
§7.4.9 (JPX), §7.6.3–§7.6.4 (encryption algorithms), §8.9.3 (sample data), Annex C (limits)
Journeys: none directly (an internal boundary; every rendering, saving and security journey
depends on it)
Status: implemented (part 1 of 4: #14). Fonts and shaping: #53. Rasterizer and ICC: #54.
Signatures: #55.

## Behavior
`jpdf-traits` defines the replaceable-backend boundary of PLAN §3. Third-party crates are
used only through these traits (CONSTITUTION 11), via adapters in `jpdf-backends`. The crate
has no implementations and no dependencies. ADR 0004 records where `Limits` lives and why
the error type is hand-written.

**Rules for every trait** (later parts must follow them too):
1. Every call that produces data takes `&DecodeLimits` and checks it **before allocating**.
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

`Limits::decode_limits(remaining_document_bytes)` builds the `DecodeLimits` for one call.
Its output cap is the smaller of the per-stream limit and what the document has left.

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
- Fuzz targets per trait come with #17.
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
- Existing `jpdf-syntax`/`jpdf-engine` tests cover the moved `Limits` unchanged.
- No E2E IDs (no UI).
