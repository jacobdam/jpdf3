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
   `working_bytes: u64` and `ops: u64`: what the call charged. Inflate doesn't charge and
   reports `DecodeUsage::default()` (both 0); the flate suite checks it on the full-vector
   decodes (`flate.no_usage`), not on truncated or bomb results.
   Deflate and Cipher return plain bytes, so they report nothing. All three have working
   memory beyond the output that is a constant (window, tables, key schedule), and time linear
   in input plus output, which other limits already bound.
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
      within its limits. A `Truncated` result reports the work it did, and the
      samples it fills in (blank rows after the salvaged ones) count toward the per-sample
      minimum like decoded ones.
4. **Image suite checks** (`jpdf_traits::conformance::image`, every format):
   - `image.usage_valid`: for every fixture decoded at the default limits, `working_bytes` is
     at least the output's byte length, `ops` at least the output's sample count, and the
     whole result (usage included) fits the default limits.
   - `image.working_limit` and `image.ops_limit`: exact boundaries at the usage the decoder
     reported. At the reported value the call succeeds (Complete, matching); one below and 0
     give `LimitExceeded`.
   - Every at-limit success of every boundary probe (output, pixels, dimension, components,
     working bytes, operations) must report the same usage as the default-limits decode, so
     usage that changes with any limit fails that probe.
   - Every `Ok` result in the truncated, mutated, salvage and option checks (the
     ColorTransform, JBIG2 globals, CCITT `K` and `EncodedByteAlign` probes) reports usage
     within its limits and obeys both lower bounds.
   - **Extra-work claim.** JPEG, JPEG 2000 and JBIG2 fixtures get a field
     `extra_work: Option<ExtraWork>`, with `lighter` (the same image encoded with less work: one
     scan instead of several, one quality layer, no overlapping symbol instances) and
     `min_extra_ops` (a lower bound on `ops(input) - ops(lighter)` that the fixture author
     justifies from the two streams). `image.extra_work`: the lighter input decodes Complete to
     the expected image with a valid result, and the input's reported ops exceed the lighter
     input's by at least `min_extra_ops`. Because the suite measures a difference between two
     decodes of the same output, a backend that charges only per output sample, however many
     per sample, reports a difference of 0 and fails. The lighter input must be at least as
     long as the input (authors pad it: a JPEG COM segment, a JP2 `free` box, a JBIG2 extension
     segment), so a flat rate over every input byte gives no difference either. A rate per
     entropy-coded byte or per segment still can (see Security). A `lighter` equal to
     the input or shorter than it, or a `min_extra_ops` above the default `max_decode_ops`,
     makes the fixture invalid (`image.fixture_valid`).
   - `image.work_fixture` (JPEG, JPEG 2000, JBIG2): at least one valid fixture has an
     extra-work claim of at least 4 × its output samples. It is a coverage rule on the fixture
     set; the decoder is judged by `image.extra_work`. CCITT is exempt (no field): its work is
     linear in input plus output.

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
    and ones that report usage depending on `max_ops`, `max_output_bytes`, or the pixel,
    dimension, component or working-bytes limit each fail those checks; one that reports
    over-limit usage (operations or working bytes) only at the default limits fails
    `image.usage_valid`; one that reports 0 operations on the lighter input fails
    `image.extra_work`;
  - `min_extra_ops`: the toy's exact difference passes, one more fails `image.extra_work`; at
    the default `max_decode_ops` the fixture is valid, one past is invalid; the heavy rule
    passes at exactly 4 × samples and fails one below (`image.work_fixture`);
  - unit: `decode_limits` copies both fields (`decode_limits_copy_the_work_budgets`); the
    clamping tests list both fields (`all_fields`, distinct ceilings, one past the ceiling).
- Budget: `charged_budgets_do_not_exceed_the_process_budget` now counts the larger of the
  output cap and `max_decode_working_bytes` as the decode term. The full budget
  (specs/core/memory-budget.md, #109) counts both: its decode term is the output cap plus the
  working bytes.
  - **Known gap:** the decoder's input is not in `working_bytes`. In a filter chain such as
    `[/FlateDecode /DCTDecode]`, or with Flate-compressed `JBIG2Globals`, the up-to-256 MiB
    Flate output that feeds the image decoder is still alive while it runs, and nothing charges
    it. The real decode term is up to 512 MiB. specs/core/memory-budget.md now counts it
    (decode term = 256 + 256 MiB). That holds only if `jpdf-filters`, which doesn't exist yet,
    keeps at most one intermediate output alive while the image decoder runs; the
    requirement is listed there.
- The toy decoder in the suite's tests charges one operation per output sample plus 64 per
  unit of value in the bytes after the samples (a "repeat count"); its heavy fixtures end in a
  count of 4, and their lighter input is the same length with a count of 0. Its loops run over the input once, so they need no step cap.

## Security
- Arithmetic: the suite computes sample counts as `u64` products with `saturating_mul`
  (`width × height × components`), and 4 × samples with `saturating_mul`; saturation can only
  make a check stricter. The toy's charges use `checked_*` arithmetic (the trailing-count sum and the sample product
  included) and fail with `LimitExceeded` on overflow.
- What a broken decoder can still do and pass:
  - allocate more than it reports: the suite sees only the reported numbers, not allocations;
  - treat `max_working_bytes` as a second output cap (report the output length plus a
    constant): nothing tests working memory beyond the output, because unlike operations
    there is no fixture claim for it. The table's "about 51 MiB" for a coefficient-plane
    decoder is therefore policy that adapter review must check, not something the suite proves.
    Follow-up: #136 (a heavy-memory claim mirroring `extra_work`);
  - charge too late (do the work, then fail): the exact boundary still passes. Only timing
    shows it, so the resource-limit harness (#25) and fuzzing (#17) cover it;
  - count the features the fixtures exercise but skip others (count layers but not tiles).
    `image.extra_work` proves that some non-output work is counted, not all of it;
  - report a number that matches the rules but isn't what it really did, as long as it is
    consistent across calls and its limit checks agree with it;
  - charge a flat rate per entropy-coded byte, marker segment or box, and nothing for the
    work those bytes cause. The padding in the lighter input is skipped, not decoded, so the
    heavy input's extra coded bytes give a positive difference and pass `image.extra_work`,
    while a few coded bytes can still cause unbounded work (a large overlapping JBIG2 symbol
    instance). The contract forbids it (work that produces no output counts by what it does);
    adapter review checks it, and #25 and #17 measure the time;
  - skip charging on damaged input whose usage stays under the default budgets: the
    truncated and mutated probes cap only the output, not the work (#137);
  - enforce the work limits only when they are small (for example only below 2^31, or only
    when a limit isn't its default). Every probe uses limits sized to a fixture, far below the
    defaults, and no fixture needs more than the default budgets. Over-budget input at the
    default limits is for the resource-limit harness (#25) and fuzzing (#17).
- What it can't do and pass: ignore either limit, reject at the limit, use a built-in budget
  instead of the caller's (the 0 probe and the one-below probe), count fewer operations than
  output samples, charge only per output sample or a flat rate over every input byte, leave the
  output out of working memory, report usage that changes with any limit, or return an `Ok` whose usage is
  over its limits (option probes included).
- What the tests can't see: allocations and time. #25 (2 GiB `RLIMIT_AS` harness, Linux) and
  the fuzz targets measure them.

## Accessibility
Not applicable.

## Platforms
Same on every platform. `u64` fields, so wasm32's 32-bit `usize` doesn't matter.

## Performance
The checks add two boundary probes (three calls each) per fixture. The suite's run time grows
by about the time of six decodes per fixture, seven for a fixture with an extra-work claim
(its lighter input).

## Tests
- `jpdf-traits` unit tests (limits): `decode_limits_copy_the_work_budgets`, the clamping
  tests, the budget test.
- `jpdf-traits` flate suite self-test: `an_inflate_that_reports_usage_is_caught` (an Inflate
  that reports usage fails `flate.no_usage`).
- `jpdf-traits` image suite self-tests: a correct toy passes every suite; toys that ignore
  the work limits, reject at the limit, report limit-dependent usage, leave the output out of
  working memory, count only output samples, a flat rate per sample or a flat rate per input byte, report over-limit usage
  on truncated input, or report invalid usage only in an option probe each fail the named
  checks; a fixture set without a heavy-work fixture fails `image.work_fixture`; a wrong
  lighter input or an over-large claim fails `image.extra_work` or `image.fixture_valid`.
