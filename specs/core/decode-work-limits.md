# Decoder working memory and operations budget

Area: engine
ISO 32000-2 clauses: §7.4.6 (CCITTFaxDecode), §7.4.7 (JBIG2Decode), §7.4.8 (DCTDecode),
§7.4.9 (JPXDecode). The limits themselves are project policy (PLAN §2.2).
Journeys: none (no UI; protects every journey that shows an image)
Status: implemented

## Behavior
Issue #106 (trait contracts 2/3, from #79 and audit 0001 item 1). Before this change only
Inflate promised bounded working memory (specs/core/backend-traits.md). The four image decoders
bounded only their output. Two gaps:
- **Memory.** A JPEG 2000 image just under the 256 MiB output cap needs about 1 GiB of 32-bit
  wavelet coefficients. A progressive JPEG keeps 16-bit coefficients for every sample.
- **CPU.** A JBIG2 text region with millions of symbol instances, or a JPEG 2000 codestream
  with many tiles, quality layers or code-block passes, can do unbounded work while producing a
  tiny image. The output cap doesn't see that work.

The change has four parts.

1. **Two new `Limits` fields** (jpdf-traits), clamped by `Limits::clamped_to` like every other
   field (specs/core/limits-ceiling.md):

   | Field | Default | Why |
   |---|---|---|
   | `max_decode_working_bytes: u64` | 256 MiB | Peak memory one image decode holds, output included. It takes the place of "one stream being decoded" in the 2 GiB budget sum, so the sum stays at 2 GiB. A decoder with a full-size coefficient plane (4 bytes per sample) therefore decodes at most about 51 MiB of 8-bit output; larger images fail with `LimitExceeded` (CONSTITUTION 4: security beats performance) |
   | `max_decode_ops: u64` | 2^32 | Operations one image decode may perform: about 43 per pixel at the 100,000,000-pixel limit. That leaves room for a several-stage decode of the largest legal image, and still bounds the time of one call |

   `Limits::decode_limits(remaining)` copies both into the new `DecodeLimits` fields
   `max_working_bytes` and `max_ops`. Neither field shrinks with the document budget: that
   budget bounds the output total, and output is part of working memory anyway.
2. **Usage report.** `Decoded<T>` gets a field `usage: DecodeUsage`, with
   `working_bytes: u64` and `ops: u64`: what the call charged. Inflate, Deflate and Cipher don't
   charge and report `DecodeUsage::default()` (both 0). Their working memory beyond the output is
   a constant (window and tables), and their time is linear in input plus output, which other
   limits already bound.
3. **Contract for `JpegDecoder`, `Jpeg2000Decoder`, `Jbig2Decoder` and `CcittDecoder`:**
   1. `working_bytes` is the most bytes the call holds at once, by the backend's own nominal
      accounting, **including the output buffer**. So it is at least the output's byte length.
      The backend computes each charge from headers before the allocation it covers.
   2. `ops` counts the work the call did, in a backend-chosen unit with two rules:
      - at least one per output sample (an image's width × height × components, or a bitmap's
        width × height);
      - work that produces no output counts too: every tile, quality layer, resolution level,
        code-block pass, progressive scan, symbol instance (by its area), refinement step, and
        region decoded and then discarded.

      The backend charges an operation before doing the work it covers, so an over-budget call
      stops early instead of finishing and then failing.
   3. When a charge would take `working_bytes` past `max_working_bytes` or `ops` past
      `max_ops`, the call fails with `LimitExceeded`, never with a `Truncated` result. A call
      that needs exactly the limit succeeds.
   4. Usage depends only on the input and the options, never on the limits. The same call
      under any limits that it fits reports the same usage, and every `Ok` result reports usage
      within its limits. A `Truncated` result reports the work it did.
4. **Image suite checks** (`jpdf_traits::conformance::image`, every format):
   - `image.usage_valid`: for every fixture decoded at the default limits, `working_bytes` is
     at least the output's byte length, `ops` is at least the output's sample count, and `ops`
     is at least the fixture's `min_ops`.
   - `image.working_limit` and `image.ops_limit`: exact boundaries at the usage the decoder
     reported. At the reported value the call succeeds (Complete, matching) and reports the
     same usage again; one below and 0 give `LimitExceeded`.
   - Every `Ok` result in the truncated, mutated, salvage and option checks reports usage
     within its limits and obeys both lower bounds.
   - New fixture field `min_ops: u64`: a lower bound on the operations the fixture needs,
     which the fixture author justifies from the stream (scans, layers, instances written).
     0 claims nothing beyond the per-sample rule. A `min_ops` above the default `max_decode_ops`
     makes the fixture invalid (`image.fixture_valid`).
   - `image.work_fixture` (JPEG, JPEG 2000, JBIG2): at least one valid fixture has
     `min_ops` of at least 4 × its output samples, so a backend that counts only output
     samples fails it. Examples: a progressive JPEG with several full scans, a JPEG 2000 image
     with several quality layers, a JBIG2 page whose symbol instances overlap. CCITT is exempt:
     its work is linear in input plus output.

## Persistence
Nothing is persisted.

## Undo
Not applicable.

## Errors
No new error variant. Exceeding either budget gives `BackendError::LimitExceeded`, which
`jpdf-filters` maps like every other limit: the image is shown damaged (blank) and the page
still renders (specs/core/backend-traits.md, Errors).

## Limits
- `max_decode_working_bytes` and `max_decode_ops` are new; both are clamped to the defaults by
  `Document::open` (limits-ceiling).
- Exact-boundary tests:
  - suite, per fixture and both fields: at the reported usage passes, one below and 0 fail
    (`image.working_limit`, `image.ops_limit`);
  - suite self-tests: a toy that ignores the new limits, one that rejects at the limit (`>=`),
    and one that reports usage depending on the limits each fail those checks;
  - unit: `decode_limits` copies both fields (`decode_limits_copy_the_work_budgets`); the
    clamping tests list both fields (`all_fields`, distinct ceilings, one past the ceiling).
- Budget: `charged_budgets_do_not_exceed_the_process_budget` now counts the larger of the
  output cap and `max_decode_working_bytes` as the decode term. The sum is still exactly 2 GiB,
  with no headroom (#109).
- The toy decoder in the suite's tests charges one operation per output sample plus 256 per
  trailing byte after the samples. Its loops run over the input once, so they need no step cap.

## Security
- Arithmetic: the suite computes sample counts as `u64` products with `saturating_mul`
  (`width × height × components`), and 4 × samples with `saturating_mul`; saturation can only
  make a check stricter. The toy's charges use `checked_*` and fail with `LimitExceeded` on
  overflow.
- What a broken decoder can still do and pass:
  - allocate more than it reports: the suite sees only the reported numbers, not allocations;
  - charge too late (do the work, then fail): the exact boundary still passes. Only timing
    shows it, so the resource-limit harness (#25) and fuzzing (#17) cover it;
  - count the features the fixtures exercise but skip others (count layers but not tiles).
    `image.work_fixture` proves that some non-output work is counted, not all of it;
  - report a number that matches the rules but isn't what it really did, as long as it is
    consistent across calls and its limit checks agree with it.
- What it can't do and pass: ignore either limit, reject at the limit, use a built-in budget
  instead of the caller's (the 0 probe and the one-below probe), count fewer operations than
  output samples, leave the output out of working memory, report usage that changes with the
  limits, or return an `Ok` whose usage is over its limits.
- What the tests can't see: allocations and time. #25 (2 GiB `RLIMIT_AS` harness, Linux) and
  the fuzz targets measure them.

## Accessibility
Not applicable.

## Platforms
Same on every platform. `u64` fields, so wasm32's 32-bit `usize` doesn't matter.

## Performance
The checks add two boundary probes (three calls each) per fixture. The suite's run time grows
by about the time of six decodes per fixture.

## Tests
- `jpdf-traits` unit tests (limits): `decode_limits_copy_the_work_budgets`, the clamping
  tests, the budget test.
- `jpdf-traits` image suite self-tests: a correct toy passes every suite; toys that ignore
  the work limits, reject at the limit, report limit-dependent usage, leave the output out of
  working memory, count only output samples, or report over-limit usage on truncated input each
  fail the named checks; a fixture set without a heavy-work fixture fails `image.work_fixture`;
  a `min_ops` above the default is an invalid fixture.
