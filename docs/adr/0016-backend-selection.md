# 16. Backend selection: one PLAN §3 candidate passes admission, the rest are our own

Status: accepted (#304, part 2 of #16)

## Context
PLAN §3 puts each external capability behind a `jpdf-traits` trait and lists a candidate crate
for each, "to verify in M0". A third-party crate may be used only if it passes the admission
rules (PLAN §3, AUTOMATION.md §6.4): an allowlisted license (CONSTITUTION 10), no `unsafe`
anywhere in its dependency tree outside `ops/judge/unsafe-allowlist.txt` (CONSTITUTION 8),
imported `cargo vet` audits only (CONSTITUTION 9), no unaudited build scripts or proc-macros,
and an exact version pin. If a candidate fails, we write our own implementation behind the
trait (CONSTITUTION 11).

`cargo xtask admit <crate>@<version>` (#303, #413; specs/tooling/admit.md) checks all of these
locally: `deny` (licenses, advisories, bans, sources with the repo's `deny.toml`), `unsafe` (the
judge's own scan), `vet` (the repo's imports, fetched fresh) and `build-time`. It is a
pre-check; the judge stays the authority.

**How the results below were produced.** On 2026-11-28, from main at 8956b00, toolchain
1.100.0 (`rust-toolchain.toml`), cargo-deny 0.21.0, cargo-vet 0.10.3, `--python` Python 3.14,
each `--work` directory outside any checkout. Each candidate was run at its newest stable
crates.io version with default features (23 runs). Eleven were run again at an older
version (11 runs): miniz_oxide 0.8.9, the newest version the imported audit files (Google,
Mozilla, Bytecode Alliance, Zcash, as fetched that day) cover; skrifa 0.29.0, the last one
Google audited (skrifa itself is covered up to 0.50.0 through Mozilla's deltas; the re-run
checked whether an older dependency graph was clean); and for the RustCrypto crates the
newest release of the previous line (md-5 0.10.5, sha2 0.10.8, rc4 0.1.0, aes 0.8.4, cbc
0.1.2, p256 0.13.2, p384 0.13.1, rsa 0.9.9, x509-cert 0.2.5), which has no `hybrid-array`.
Of those, vet passes the crate itself only for miniz_oxide 0.8.9, skrifa 0.29.0, md-5 0.10.5
and sha2 0.10.8; the audit files hold no audit of rc4, cbc, rsa, p384, x509-cert or cms at
any version, and only delta audits of p256 and aes. Four were run with `--no-default-features`
(4 runs), to see whether default features caused the rejection: hayro-jpeg2000 and
hayro-jbig2 (their `unsafe` findings were all in optional dependencies), tiny-skia and
x509-cert 0.2.5 (they were not: both stay rejected by `unsafe`). 38 runs in all. The store's `isrg` import
(#442) contributed no audit to any result: the one admitted graph is covered by Bytecode
Alliance and Mozilla audits.

## Results

"Rejected by" lists the steps that rejected the candidate; no run had a failed step (every run
ended `admitted` or `rejected by`, never `not decided`).

| Trait | Candidate run | Rejected by | What blocks it | Decision | Issue |
|---|---|---|---|---|---|
| `Inflate` / `Deflate` | `miniz_oxide@0.9.1` | vet | miniz_oxide 0.9.1 has no imported audit | – | – |
| | `miniz_oxide@0.8.9` | – (admitted) | graph: miniz_oxide 0.8.9, adler2 2.0.1, both audited | **adopt** | #446 (needs #445) |
| `JpegDecoder` | `zune-jpeg@0.5.15` | unsafe, vet | `unsafe` in zune-jpeg itself | own | #447 |
| | `jpeg-decoder@0.3.2` | unsafe, vet, build-time | `unsafe` in jpeg-decoder itself, rayon, crossbeam | own | #447 |
| `Jpeg2000Decoder` | `hayro-jpeg2000@0.5.0` | unsafe, vet | `unsafe` in bytemuck, fearless_simd, image, moxcms, num-traits, pxfm | – | – |
| | same, `--no-default-features` | vet | no imported audit for hayro-jpeg2000 | own | #448 |
| `Jbig2Decoder` | `hayro-jbig2@0.4.0` | unsafe, vet | as hayro-jpeg2000 | – | – |
| | same, `--no-default-features` | vet | no imported audit for hayro-jbig2, hayro-ccitt | own | #449 |
| `CcittDecoder` | `hayro-ccitt@0.5.0` | vet | no imported audit for hayro-ccitt | own | #450 |
| `FontEngine` | `skrifa@0.50.0` | unsafe, vet, build-time | `unsafe` in font-types, bytemuck, smallvec, yoke, once_cell, the proc-macro stack | – | – |
| | `skrifa@0.29.0` (last Google-audited) | unsafe, vet, build-time | `unsafe` in font-types 0.8.4, bytemuck, bytemuck_derive and the proc-macro stack; bytemuck_derive and proc-macro2 not audited | own | #451 |
| `Type1Parser` | none (PLAN §3: own) | – | – | own | #452 |
| `Rasterizer` | `vello_cpu@0.4.0` | unsafe, vet, build-time | `unsafe` in 27 packages (fearless_simd, bytemuck, euclid, hashbrown, zlib-rs, …) | own | #453 |
| | `tiny-skia@0.12.0` (also `--no-default-features`) | unsafe, vet (+ build-time with defaults) | `unsafe` in tiny-skia itself | own | #453 |
| `ColorTransform` | `moxcms@0.9.2` | unsafe, vet | `unsafe` in moxcms itself, pxfm, num-traits | own | #454 |
| `Hasher` / `Cipher` | `md-5@0.11.0`, `sha2@0.11.2`, `rc4@0.2.0`, `aes@0.9.4`, `cbc@0.2.1` | unsafe, vet (+ build-time for sha2, aes) | `unsafe` in hybrid-array (and libc for sha2, aes); no imported audits for most of the graph | – | – |
| | `md-5@0.10.5`, `sha2@0.10.8`, `rc4@0.1.0`, `aes@0.8.4`, `cbc@0.1.2` | unsafe, vet, build-time | `unsafe` in generic-array 0.14 (and libc for sha2, aes); vet reports unvetted generic-array, typenum, crypto-common, version_check, plus digest (md-5, sha2), block-padding (cbc), cpufeatures and libc (sha2, aes), and rc4 0.1.0, cbc 0.1.2 and aes 0.8.4 themselves (no full imported audit) | own | #455 |
| `SignatureVerifier` | `rsa@0.10.0`, `p256@0.14.0`, `p384@0.14.0` | unsafe, vet, build-time | `unsafe` in hybrid-array, base16ct, base64ct, cmov, pem-rfc7468, getrandom, libc, serde, … | – | – |
| | `rsa@0.9.9`, `p256@0.13.2`, `p384@0.13.1` | deny, unsafe, vet, build-time | RustSec vulnerability advisories (timing side channels), plus the above with generic-array | – | – |
| | `x509-cert@0.3.1`, `x509-cert@0.2.5` (also `--no-default-features`), `cms@0.2.3` | unsafe, vet, build-time | `unsafe` in base64ct, flagset, the proc-macro stack; der_derive not audited | own | #456 |
| `Signer` | none: the platform signer (PLAN §6.4) | – | – | platform | – |
| `OcrEngine` (Phase 2) | `ocrs@0.14.0` | unsafe, vet, build-time | `unsafe` in 24 packages (rten-*, rayon, crossbeam, libc, …) | own, scoped with Phase 2 | – |
| `TextShaper` | `rustybuzz@0.20.1` | deny, unsafe, vet, build-time | RustSec unmaintained advisories (rustybuzz, ttf-parser) | – | – |
| | `harfrust@0.16.2` | unsafe, vet, build-time | `unsafe` in font-types, bytemuck, smallvec, yoke, bitflags, … | own | #457 |

## Decision
1. **`Inflate` and `Deflate` use `miniz_oxide = "=0.8.9"`** with default features, as admitted,
   in a `jpdf-backends` feature (#446). The judge's `cargo vet --locked` reads only
   `supply-chain/imports.lock`, which has no audits yet, so the owner first adds the imported
   ones (#445, `ops/proposals/vet-imports-miniz-oxide.md`). 0.9.1, the newest version, has no
   imported audit; moving to it later goes through admission again (rule 5).
2. **Every other trait gets our own implementation** in `jpdf-backends`, one Cargo feature
   each (PLAN §3), written from the standards each issue names and never from the rejected
   crates' code (CONSTITUTION 16). Each passes its `jpdf_traits::conformance` suite and gets a
   fuzz target. The issues (#447 to #457) are filed without `ready`; most are larger than one
   session, so the Planner splits them (ops/TICK.md §9 step 2).
3. **Milestones.** Crypto (#455) is M1: the standard security handler needs it. The image
   decoders, fonts, the rasterizer and ICC (#447 to #454) are M2, where rendering first needs
   them; PLAN §8 lists "all filters" under M1, but M1's gate (open what qpdf opens, no panics or
   timeouts) decodes no image, so the Planner may move a decoder into M1 if a gate needs it.
   Shaping (#457) is M5 with its trait (#66); signature verification (#456) is M9 with its
   trait (#55). `OcrEngine` gets no issue: Phase 2 is scoped at its start (PLAN §8), and this
   ADR is the record for that scoping.
4. **Vet-only rejections are re-checked.** `hayro-jpeg2000` and `hayro-jbig2` with
   `--no-default-features`, and `hayro-ccitt`, fail only for want of an imported audit. Their
   issues (#448, #449, #450) say to re-run `cargo xtask admit` first and switch to an adapter
   if an imported audit has appeared; the audit session's dependency refresh (ops/TICK.md §6)
   re-runs the rest of this table.
5. **No proposal for `deny.toml` or the `unsafe` allowlist.** No candidate would pass with a
   `deny.toml` change alone: rustybuzz's advisories and the rsa and p256 0.13 vulnerability
   advisories come with `unsafe` and `vet` rejections. The allowlist could take more RustCrypto
   names (hybrid-array, base16ct, base64ct, pem-rfc7468 and cmov are RustCrypto crates not on
   it), but generic-array and libc, which also contain `unsafe`, are not RustCrypto crates,
   and the imported audits (typenum, crypto-common, digest and most of the primitives)
   these graphs lack can't be added by any change of ours (CONSTITUTION 9).

## Consequences
- **Our own crypto.** `ops/judge/unsafe-allowlist.txt` exists because audited RustCrypto code
  was preferred over our own ("writing our own crypto is less safe"). Today that path is
  closed by the audit rule, not only by the allowlist, so #455 and #456 write MD5, SHA-2, RC4,
  AES-CBC, RSA and ECDSA verification, X.509 and CMS ourselves. The risk taken is correctness,
  checked by published known-answer vectors, vectors verified once against an independent
  implementation, the conformance suites and fuzzing. Side-channel resistance is **not**
  claimed, and the specs must say so: the cipher works on a key derived from the document
  password on the user's own machine, signature verification handles public data only, and
  private-key operations never reach this code (the platform `Signer`, PLAN §6.4). If imported
  audits for a RustCrypto graph appear, an adapter can replace our code behind the same trait
  after admission; the allowlist entries for its non-listed crates would then be the owner's
  call.
- **M2 grows.** Eight own implementations land in M2, three of them large (JPEG 2000, JBIG2,
  fonts) and one central (the rasterizer). PLAN §3 already said to "plan for our own
  JPX/JBIG2/rasterizer/ICC code"; the JPEG decoder and the font engine are new to that list.
- **The table is a snapshot.** Versions, advisories and the imported audit sets change; the
  check is `cargo xtask admit`, re-run before any adapter is added or bumped. PLAN §3's
  "Candidate default" column is superseded by this ADR.
- **The first third-party dependency** enters the root lockfile with #446, and with it the
  first non-empty `imports.lock`; P3 (`docs/PRINCIPLES.md`) keeps it inside `jpdf-backends`.
