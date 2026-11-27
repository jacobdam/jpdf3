# Journeys: signatures and platform conventions

Areas `SG` (digital signatures: verify, sign, certify, digital IDs, trust, timestamps and
long-term validation) and `PC` (platform conventions: unsaved changes, file changed on disk,
crash recovery, Save As, print preference, multiple windows, the shortcuts list, offline use
on the web, the app's language). PLAN §4
(Security & signatures; Optimize & save), PLAN §1 R12 and R11 (files never leave the device), feature groups F-SC-2 … F-SC-4 and the
platform rows of F-RD-1, F-RD-5 and F-OS-1 in [specs/TRACE.md](../TRACE.md). Inventory and
coverage: [RESEARCH.md](RESEARCH.md). Format: ops/judge/CONTRACT.md §4.
Milestones (PLAN §8): signatures are M9; conventions that need saving (unsaved changes, file
changed on disk, crash recovery, Save As, several windows) are M6 (writer); the print
preference and the shortcuts list are M4; offline use on the web is M10 (PWA offline), and so is the app's language (localization).

Shared rules for every journey in this file. The reader rules at the top of
[reader.md](reader.md), the annotate rules in [annotate.md](annotate.md), the pages-edit rules
in [pages-edit.md](pages-edit.md) (save and verify, write and verify, history is honest,
permissions) and the forms-security rules in [forms-security.md](forms-security.md)
(passwords, encryption writing, signed files) also apply.

### Signature rules (SG)

- **Engine owns signatures** (CONSTITUTION 5, 6). Parsing CMS, X.509 certificates, PKCS#12,
  timestamp tokens, OCSP responses and CRLs; computing digests; building and checking
  certificate chains; classifying changes made after signing; planning the byte range;
  generating signature appearances; and writing the signature all happen in the Rust engine.
  Every one of those inputs is hostile and parsed under `Limits` (input size, ASN.1 nesting
  depth, certificates per chain, chain length, revisions and objects compared), with a typed
  error, never a panic. Engine tests and fuzz targets (not these E2E tests) cover time and
  memory bounds. The native layer does only three things: the private-key operation through
  the platform signer (macOS: a Keychain `SecKey`; web: a non-extractable WebCrypto
  `CryptoKey`), sending the network requests the user started, and displaying the engine's
  results.
- **The returned signature is checked before it is embedded** (PLAN §6.4). The engine verifies
  the value the platform signer returns against the signing certificate's public key and the
  exact signed attributes it asked to sign. Web ECDSA returns IEEE P1363 `r‖s`; the engine
  converts it to DER and checks it. A wrong value (test signer hook: one bit flipped, a value
  for other data, a value from another key, a value longer than the reserved `/Contents`
  space) or a signer cancellation (test hook) is a typed error, and no file is written.
- **Key hand-off.** The engine parses a PKCS#12 file (J-SG-05). The binding has exactly one
  call that releases key bytes: the import hand-off, which returns the PKCS#8 key once, in a
  zeroizing buffer, to the native layer, which passes it straight to the platform key store
  (macOS `SecKeyCreateWithData` then `SecItemAdd`; web `importKey` with `extractable: false`)
  and zeroizes it; the engine zeroizes its own copy. No other call returns key bytes.
- **Algorithms.** We create SHA-256 digests (SHA-384 for P-384 keys; SHA-384 or SHA-512 when
  a seed value requires it, J-SG-07) with RSA PKCS#1 v1.5 (2048 bits or more) or ECDSA
  P-256/P-384 signatures. Web limit: a WebCrypto RSA key fixes its hash when imported, so web
  RSA IDs are stored for SHA-256 and cannot meet a seed value asking for another digest
  (signing such a field is inert with that reason). We verify SHA-256, SHA-384 and SHA-512
  with RSA PKCS#1 v1.5, RSA-PSS and ECDSA P-256/P-384/P-521. MD5, SHA-1 and RSA keys under
  2048 bits are a weak algorithm; any other algorithm is unsupported.
- **Signing certificates** must carry the key usage digitalSignature or contentCommitment
  (nonRepudiation) (RFC 5280 §4.2.1.3, ETSI EN 319 412-2); a certificate with neither is not
  allowed for signing.
- **Trust list.** Trust anchors come only from the app's own trust list, which is empty after
  installation. The user adds certificates to it, always through a confirmation naming each
  certificate's SHA-256 fingerprint (J-SG-04). It is stored in app storage (macOS: the app
  container; web: the origin's IndexedDB) and passed to the engine as untrusted bytes. System
  root stores and public trust lists are not used (a later decision,
  #48). Certificates of the user's own digital IDs are not trusted automatically.
- **Statuses.** For each signature the engine returns four facts and a summary, shown in the
  Signatures panel (IA §4) and the signature details:
  - Integrity: `Intact` (the byte range checks hold, the message digest matches the signed
    bytes, the signature value verifies over the signed attributes, and the signer
    certificate is the one the SignerInfo `sid` names and whose hash signing-certificate-v2
    holds), `Altered` (the digest does not match, the signature value does not verify, or the
    signer certificate does not match `sid` or signing-certificate-v2) or `Cannot check`
    (the signature is malformed, or a byte range check fails).
  - Signer: `Trusted` (the chain reaches the trust list and every certificate was valid at
    the validation time), `Not trusted`, `Not allowed for signing`, `Expired`, `Revoked`,
    `Weak algorithm` or `Unsupported algorithm`; plus the line "Revocation was not checked"
    when no revocation data was embedded and no online check was made (the summary stays as
    computed: revocation data is optional for B-B and B-T).
  - Changes since signing, with the engine's list of changes per later revision:
    - `No changes`: no later revision, or later revisions that only add validation data
      (DSS, document timestamps), which ISO 32000-2 §12.8.2.2.2 does not count as changes;
      the list then says "Validation data added".
    - `Allowed changes`: every change is in the allowed set: filling form fields (`/V`,
      `/AS`, and a regenerated `/AP` of a widget whose `/Rect`, `/F` and `/MK` are
      unchanged), signing existing signature fields, adding a new signature field with its
      widget (appended to `/AcroForm /Fields` and to the page's `/Annots`) and, unless a
      certification forbids it (J-SG-03), adding, changing or deleting annotations other
      than widgets (with the matching changes to the page's `/Annots` array).
    - `Changes not allowed`: any change a DocMDP certification (ISO 32000-2 §12.8.2.2) or a
      FieldMDP lock (§12.8.2.4) forbids.
    - `Signed content changed`: for a signature with no certification or lock covering the
      change, any change outside the allowed set: page content streams or resources, the
      page tree (other than the `/Annots` changes above), optional content (`/OCProperties`)
      state, the catalog's other entries, replacing or removing existing `/AcroForm /Fields`
      entries, new fields other than signature fields, or an existing widget's
      `/Rect`, `/F` or `/MK`. These changes are legal for an approval signature, but they
      change what was signed (shadow and incremental-saving attacks).
  - Time: "Signing time (claimed by the signer): …" from `/M`, and, when a timestamp token
    validates, "Timestamped: … by «TSA name»". Tests run with the time zone set to UTC, so
    times show as `2026-03-01 10:00`.
  - Validation time: past signature validation as in ETSI EN 319 102-1 §5.6.2.4: the newest
    document timestamp is validated at the current time; each older timestamp, and then the
    signature, is validated at the time of the next newer timestamp that covers it, using
    the certificates and revocation data in the DSS. With no valid timestamp, the
    validation time is the current time.
  - Summary: `Invalid` if integrity is Altered or Cannot check, the signer is Revoked (at or
    before the validation time) or uses a weak algorithm, or there are changes not allowed;
    otherwise `Unverified` if the signer is Not trusted, Not allowed for signing, Expired or
    uses an unsupported algorithm, or the signed content changed; otherwise `Valid`. The
    panel header shows a one-line document summary: "All signatures are valid", or else
    "1 invalid signature" / "N invalid signatures" if any is Invalid, or else "1 signature
    could not be verified" / "N signatures could not be verified".
- **Byte range checks** (ISO 32000-2 §12.8.1). `/ByteRange` must have exactly four
  non-negative integers `[0 a b c]` with a > 0 and b = a + the length of the signature's own
  `/Contents` hex string including its `<` `>` delimiters, which starts at offset a; the
  integers must not overflow when added, overlap or point past the end of the file; and b +
  c must be the end of a revision: an offset just after a `%%EOF` marker (plus one optional
  EOL) that the engine reached through the file's own `startxref` / `/Prev` chain, never a
  `%%EOF` found by scanning (one inside a stream does not count). The signature dictionary
  the engine reports is the one at that position, so a dictionary that its own byte range
  does not frame is `Cannot check`.
- **Clock.** The engine reads the current time through its clock trait; tests set it.
- **Fixtures** are generated by the harness, never taken from a corpus. A test PKI made with
  Certomancer (MIT, run as an external program, from committed test-only key files) provides:
  root `jpdf Test Root`; intermediate `jpdf Test CA` (with a CRL distribution point served
  for the root's CRL); signer certificates issued by `jpdf Test CA`, each with an OCSP URL
  for the CA's responder (valid RSA-2048, valid ECDSA P-256, expired, revoked, one with
  neither digitalSignature nor contentCommitment, one signed with SHA-1, one RSA-1024); a TSA
  certificate `jpdf Test TSA` (issued by `jpdf Test CA`, timeStamping extended key usage, an
  OCSP URL); a second, long-lived TSA certificate `jpdf Test Archive TSA` (issued directly by
  `jpdf Test Root`, valid 30 years, with a CRL URL for the root's CRL; the root is valid 40
  years); and an OCSP responder certificate. Certomancer serves both TSAs (two URLs), the
  OCSP and the CRL endpoints on `127.0.0.1`, and answers CORS preflights (including for
  `application/timestamp-query` POSTs) because the web test app runs on another loopback
  port. Each signer also exists as a PKCS#12 file (OpenSSL with `-keypbe
  AES-256-CBC -certpbe AES-256-CBC -macalg sha256`, chain included). Signed fixtures are made
  with pyHanko's Python API (an external program) from that PKI, embedding the whole chain up
  to and including the root in the CMS certificates; incremental updates on signed fixtures
  are written with pyHanko's `IncrementalPdfFileWriter`, never with a tool that rewrites the
  file.
- **External validators.** Signatures are checked with pyHanko's Python API in the harness
  (`validate_pdf_signature` with a validation context trusting `jpdf Test Root`, revocation
  checking off for B-B and B-T; `validate_pdf_ltv_signature` with
  `RevocationInfoValidationType.PADES_LT` or `PADES_LTA` and fetching disabled
  (`allow_fetching=False`, endpoints stopped), so the embedded data alone must be enough, for
  B-LT and B-LTA) and Poppler's `pdfsig -nssdir <test NSS db>` (its "Signature
  Validation", "Certificate Validation", "Signature Type" and "Total document signed"
  lines). The CMS is decoded with asn1crypto. "Signature check" in a journey means: pyHanko
  reports the signature `intact`, `valid` and `trusted` with `bottom_line` true (and the LTV
  validation succeeds where the journey names B-LT or B-LTA); `pdfsig` prints "Signature is
  Valid." and "Certificate is Trusted.", and "Total document signed" for the last signature;
  and our engine reports `Valid`.
- **PAdES output** (ETSI EN 319 142-1, ISO 32000-2 §12.8). Every signature we create is
  `/Type /Sig`, `/Filter /Adobe.PPKLite`, `/SubFilter /ETSI.CAdES.detached`, with `/M`, a
  `/Contents` reservation sized by the engine and zero-padded, and a CMS SignedData whose
  signed attributes are exactly content-type, message-digest and signing-certificate-v2 (no
  signing-time attribute: the claimed time is `/M`). It is written as one incremental update
  appended to the document, and pending edits go into the same update.
- **Network is off by default** (PLAN §4). Requests happen only after an explicit user action:
  signing with "Timestamp new signatures" on (Preferences > Signatures, with a timestamp
  server URL the user typed; an optional separate archive timestamp server is used for
  document timestamps, defaulting to the same URL), or pressing "Check revocation online", "Add validation data" or
  "Add document timestamp" in a signature's details. Opening a document never makes a request
  (certificate URLs come from the file and would reveal that it was opened).
  - URLs: the TSA URL may be `https:` or `http:`; certificate-derived OCSP and CRL URLs may be
    `http:` or `https:` (most are `http:`; the responses are signed). Certificate-derived
    URLs must not reach a loopback, private, link-local or unique-local address: macOS checks
    the resolved address before connecting; web rejects hosts that are IP literals or
    `localhost` (web cannot see the resolved address, so DNS rebinding to a private address
    stays a stated residual risk on web). Test builds have a test-only flag that allows
    `127.0.0.1` for certificate URLs; a binding check asserts release builds lack it.
  - Requests send no cookies, credentials or referrer and stop at the `Limits` response size
    and a 15-second timeout. macOS follows at most one redirect, and only to the same host;
    web uses `redirect: 'error'` and follows none. A timestamp request carries the message
    imprint, a nonce and `certReq` true; an OCSP request only the certificate IDs. Responses
    are hostile input (CONSTITUTION 5).
  - Web: these are the only requests besides the reader rules' same-origin asset GETs. A
    page served over `https:` cannot fetch `http:` URLs (mixed content), and the server must
    allow CORS, so web reaches only CORS-enabled `https:` endpoints; the UI says so next to
    the server URL, and a blocked request is reported as "blocked by the browser (the server
    must allow cross-origin requests over HTTPS)". The test build serves the app and the
    harness endpoints from loopback, which browsers treat as secure; the harness endpoints
    are cross-origin to the app and send CORS headers. The web content
    security policy for these requests and the macOS App Transport Security exception for
    plain `http:` are decided before M9 (#47).

## J-SG-01: Verify a valid signature in the Signatures panel
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_verify_valid, e2e.macos.sg_verify_valid

Steps: open an unsigned 2-page fixture → open the Signatures panel → it shows "No signatures"
→ open fixture S1, signed by pyHanko (PAdES B-B, the RSA-2048 signer, SHA-256, visible
signature on page 1, `/M` 2026-03-01T10:00:00Z, reason "Approved"), with the trust list
empty → the panel lists one signature with the signer's common name, the summary
`Unverified`, integrity `Intact`, signer `Not trusted`, changes `No changes`, "Signing time
(claimed by the signer): 2026-03-01 10:00" and "Revocation was not checked" → the panel
header says "1 signature could not be verified" → add `jpdf Test Root` to the trust list
(J-SG-04) → the summary becomes `Valid`, signer `Trusted`, and the header "All signatures
are valid" → click the signature in the panel → the view scrolls to its widget on page 1 and
selects it → the details show reason "Approved", the type `ETSI.CAdES.detached`, the digest
`SHA-256`, the two signed byte ranges and "covers the whole document" → open fixture S2 (S1
plus a second approval signature by the ECDSA signer in a new revision) → the panel lists
both in signing order; the first says `Allowed changes` (1 later revision: "Signature field
Signature2 signed") and both are `Valid` → the bytes of S1 and S2 on disk are unchanged
(reader rule).
Error branch: fixture S3 has an invisible signature (widget `/Rect [0 0 0 0]`, `/F` Hidden) →
it is listed, and clicking it says "This signature has no visible appearance".

## J-SG-02: Reject altered, malformed and forged signatures
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_verify_invalid, e2e.macos.sg_verify_invalid

Steps: with `jpdf Test Root` trusted, open each fixture below → each opens and renders, and the
Signatures panel shows exactly the stated integrity, signer and summary. Fixtures 1–6 and 12
are S1 (J-SG-01) with bytes edited by the harness in a `Vec<u8>`; 7–10 are signed by pyHanko
with the named signer; 11 is built by the harness; 13–15 are S1 plus an update written with
`IncrementalPdfFileWriter`:
1. One byte of page 1's content stream changed inside the signed range → `Altered`,
   `Invalid`.
2. `/ByteRange` gap 10 bytes wider than `/Contents` → `Cannot check`, `Invalid`.
3. `/ByteRange [0 0 0 0]`, missing, with five entries, with a negative entry, with two values
   near 2^63 (overflow when added), and with a range past the end of the file → each `Cannot
   check`, `Invalid`.
4. `/Contents` missing, `()` (empty), a literal string instead of hex, and random bytes →
   each `Cannot check`, `Invalid`.
5. Signature wrapping: the signed revision's bytes moved after a new revision that redefines
   the signature dictionary, so the byte range frames a different `/Contents` → `Cannot
   check`, `Invalid`.
6. One byte of the CMS signature value flipped → `Altered`, `Invalid`; a CMS whose
   signing-certificate-v2 holds the hash of another certificate in the bag → `Altered`,
   `Invalid`.
7. Signed by the SHA-1 signer → `Weak algorithm`, `Invalid`; by the RSA-1024 signer → `Weak
   algorithm`, `Invalid`.
8. Signed by the revoked signer with revocation checking off in pyHanko, then a DSS added in an
   update holding an OCSP response that says the certificate was revoked one day before `/M`
   → `Revoked`, `Invalid`.
9. Signed by the expired signer, no timestamp, test clock after expiry → `Expired`,
   `Unverified`.
10. Signed with the certificate that has neither digitalSignature nor contentCommitment →
    `Not allowed for signing`, `Unverified`.
11. Hostile CMS → each `Cannot check`, `Invalid`: ASN.1 nested deeper than `Limits`; a length
    field larger than the file; more certificates than `Limits` allow; indefinite-length
    encodings. A CMS whose only path to the root is a loop (A issued by B, B by A) → integrity
    `Intact`, `Not trusted`, `Unverified`. A CMS with an unknown signature algorithm OID (the
    message digest itself matches) → integrity `Intact`, signer `Unsupported algorithm`,
    `Unverified`.
12. A `%%EOF` line placed inside page 1's content stream (stream length adjusted) and the byte
    range edited to end just after it → `Cannot check`, `Invalid`.
13. Hide and replace: the update redefines page 1's content stream object with new text →
    `Intact`, `Trusted`, changes `Signed content changed` ("Page 1 content changed"),
    `Unverified`.
14. The update turns off an optional content group whose content was visible when signed →
    `Signed content changed` ("Layer visibility changed"), `Unverified`.
15. The update replaces `/AcroForm /Fields` with a new array holding a new text field over the
    signed text → `Signed content changed` ("Form fields replaced"), `Unverified`.
Check: a document holding fixture 7's SHA-1 signature followed by a second signature by the
RSA-2048 signer (pyHanko signs fixture 7's file again in an incremental update) shows "1
invalid signature" in the panel header (the second is `Valid` with no later changes; the
first is `Invalid` with `Allowed changes`); `jpdf-cli verify-signatures` (CONTRACT) gives
the same results as the panel for every fixture; pyHanko reports `intact` false for fixture
1, and `valid` false for fixtures 1, 6, 7 and 8.

## J-SG-03: Changes after signing, certification levels and field locks
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_changes_mdp, e2e.macos.sg_changes_mdp

Steps: with `jpdf Test Root` trusted, open fixtures made by pyHanko from a 2-page form (text
field `name`, check box `ok`, empty signature field `Approver`), each with a certification
signature by the RSA-2048 signer and then one incremental update written with
`IncrementalPdfFileWriter`:
1. `/P 2` (ISO 32000-2 §12.8.2.2), the update fills `name` → `Allowed changes` (1 revision:
   "Form field name filled"), `Valid`.
2. `/P 2`, the update adds a Square annotation on page 1 → `Changes not allowed` ("Annotation
   added on page 1"), `Invalid`.
3. `/P 3`, the same Square annotation → `Allowed changes`, `Valid`.
4. `/P 1`, the update fills `name` → `Changes not allowed`, `Invalid`.
5. `/P 2`, the update signs `Approver` with an approval signature by the ECDSA signer → both
   `Valid`; the certification lists "Signature field Approver signed".
6. Three fixtures, `/P 1`, `/P 2` and `/P 3`, each with an update that adds only a DSS and a
   document timestamp → each `No changes` ("Validation data added"), `Valid`.
7. `/P 2`, the update redefines page 1's content stream object and also fills `name` →
   `Changes not allowed` ("Page 1 content changed"), `Invalid`.
8. `/P 2`, the update fills `name` and also enlarges its widget `/Rect` to half of page 1 with
   an opaque appearance → `Changes not allowed` ("Field name: widget position changed"),
   `Invalid`.
9. An approval signature (no certification) whose field has a FieldMDP `/Lock` with
   `/Action /Include /Fields [(name)]`, and an update that changes `name` → `Changes not
   allowed` ("Locked field name changed"), `Invalid`; a second fixture whose update changes
   only `ok` → `Allowed changes`, `Valid`.
Then, for fixture 2: open the signature's details → View signed version → a read-only tab
opens with the revision the signature covers (the bytes up to the end of its byte range),
labeled as the signed version → page 1 has no Square annotation → Save in that tab is inert
with the reason "this is the signed version of the document", and the binding's save call on
that session returns a typed error.
Engine check: a fixture whose update re-serializes unchanged objects (same values, new object
stream packing, written by the harness writer) reports `No changes`; a fixture with more
changed objects than `Limits` allow reports changes `Cannot check` with a typed reason and the
summary `Unverified`.
Residual risk, stated in the signature details help: under `/P 2`, a field that was already
large and empty when signed can be filled with an opaque appearance without any reported
change; View signed version shows what was signed.

## J-SG-04: Certificate details and the trust list
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_trust, e2e.macos.sg_trust

Steps: open S1 (J-SG-01) with the trust list empty → in the signature's details choose Show
certificate → the chain lists signer → `jpdf Test CA` → `jpdf Test Root` (all three are in the
CMS) → for each certificate the view shows subject, issuer, serial number, validity dates,
key type and size, key usage, extended key usage and SHA-256 fingerprint, as the engine's
display strings (control and bidi characters escaped as in the reader link policy) → select
`jpdf Test Root` → Trust → a confirmation names the certificate and its fingerprint and says
that signatures it vouches for will show as valid; Cancel is the default → Cancel → nothing
changes → Trust and confirm → the signature becomes `Valid` → restart the app and reopen S1
→ still `Valid` → Preferences > Signatures > Trusted certificates lists `jpdf Test Root` →
Import → pick a `.pem` file holding two certificates → one confirmation lists both with their
fingerprints → confirm → both are listed (3 in total) → import a DER `.cer` → its confirmation
→ 4 in total (the `.pem` and `.cer` hold three certificates from a second Certomancer PKI,
unrelated to `jpdf Test Root`) → remove `jpdf Test Root` → confirmation, Cancel default → confirm → S1 returns
to `Unverified`.
Error branch: importing a file that is not a certificate, a PEM with garbage between blocks,
and a file one byte over the `Limits` input size → each a typed error, nothing added; a
certificate whose subject holds U+202E (right-to-left override) shows it escaped as
`\u{202E}`; trusting the signer certificate itself (an end-entity certificate) makes S1
`Valid`, and the ECDSA signature in S2 stays `Unverified`.

## J-SG-05: Manage digital IDs (import a PKCS#12, create a self-signed ID)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_digital_ids, e2e.macos.sg_digital_ids

Steps: Secure > Digital IDs → the list shows "No digital IDs" → Import → pick the RSA-2048
signer's PKCS#12 → the password prompt appears → enter a wrong password → "Wrong password"
appears in place → enter the right one (a 24-character random sentinel) → the list shows the
ID with name, issuer, expiry, key type `RSA 2048` and storage (macOS "Keychain"; web "This
browser") → the key hand-off rule was followed: macOS `SecKeyCopyExternalRepresentation` on
the stored key fails; web `crypto.subtle.exportKey` on it rejects → Create → name `Ada Test`,
organization `jpdf`, email `ada@example.test`, country `GB`, key type ECDSA P-256 (the
default; RSA 3072 is the other choice), validity 3 years → Create → the platform generates the
key (macOS `SecKeyCreateRandomKey` in the Keychain with `kSecAttrIsExtractable` false;
the Secure Enclave is not used, so behavior is the same on CI machines, which have none; web `generateKey` with `extractable: false`) →
the engine builds the certificate (X.509 v3, subject equal to issuer, key usage
digitalSignature and contentCommitment, basic constraints not a CA, serial from the engine's
random trait), has the platform sign it and verifies the result (PLAN §6.4) → the list shows
`Ada Test` → Export certificate → a `.cer` file is written that OpenSSL parses with those
fields and that holds no private key → restart the app → both IDs are listed → remove `Ada
Test` → the confirmation says signing with it will no longer be possible and this cannot be
undone; Cancel is the default → confirm → it is gone and its key is deleted (macOS
`SecItemCopyMatching` finds nothing; web: its IndexedDB record is gone).
Storage check: the J-RD-04 storage search for the password sentinel, the PKCS#12 file's bytes
and the private key's PKCS#8 bytes finds nothing in app storage, logs or the console. On web
the Digital IDs dialog states, and the test checks the text, that the browser keeps the
non-extractable key in its own profile storage, which that search does not cover.
Error branch: a PKCS#12 with no private key, one whose key does not match its certificate, one
encrypted with RC2-40 (rejected: "Re-export this digital ID with AES encryption"), one with a
KDF iteration count above `Limits`, one nested deeper than `Limits` and one a byte over the
`Limits` input size → each a typed error, nothing added; a PKCS#12 encrypted with
`pbeWithSHAAnd3-KeyTripleDES-CBC` (common in older exports) imports; Create with an empty
name, a country that is not two letters, or a validity of 0 or 11 years → the field shows the
engine's error, Create is inert, and the binding call returns the typed error; an imported
ID whose certificate has expired is listed as `Expired` and is not offered in the Sign dialog.

## J-SG-06: Sign a document (approval signature, PAdES B-B)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_sign, e2e.macos.sg_sign

Steps: import the RSA-2048 signer's PKCS#12 (J-SG-05) and trust `jpdf Test Root` → open an
unsigned 3-page fixture `contract.pdf` → add a highlight on page 2 (an unsaved edit) → Secure
> Sign → the status bar says "Drag a rectangle where the signature goes" → drag a 200 × 60
pt rectangle on page 3 → the Sign dialog opens with: the digital ID (the imported one
selected), reason, location and contact (optional, empty), appearance options (name, date,
reason, a saved visual signature from J-ANN-15 or none), an engine-rendered preview,
Timestamp (off and inert with the reason "No timestamp server is set in Preferences"), and
the note "Unsaved changes will be saved with the signature" → type reason `Approved` → Sign
→ the save panel suggests `contract-signed.pdf` → save → the platform signer is asked once →
the document shows `contract-signed.pdf` and is not marked edited → write and verify:
- the new file starts with `contract.pdf`'s bytes unchanged; one update section follows with
  the highlight, a new signature field `Signature1` on page 3 whose widget `/Rect` is the
  dragged rectangle, an `/AP /N` generated by the engine (MuPDF's and Poppler's renders of
  the widget match ours within the render-ratchet tolerance) and `/AcroForm /SigFlags 3`;
- the PAdES output rule holds; `/Reason (Approved)`; `/M` equals the test clock;
- the signature check holds; `contract.pdf`'s bytes on disk are unchanged.
→ the Signatures panel lists it as `Valid`, `No changes` → Sign again, choosing "Invisible
signature" in the dialog instead of dragging → the save panel suggests `contract-signed.pdf`
(the current file) → save and confirm Replace → the file is the previous bytes plus one update
holding a signature field whose widget has `/Rect [0 0 0 0]` and the Hidden flag set (ISO
32000-2 §12.7.5.5) → both signatures are `Valid`; the first reports `Allowed changes` (1
revision).
Error branch: the test signer hook returns each wrong value from the "returned signature is
checked" rule, then a cancellation → each is a typed error, no file is written and the open
document still has its unsaved highlight; an ID whose certificate is not allowed for signing
is not offered, and the binding call signing with it returns a typed error; a rectangle
smaller than 12 × 12 pt, or not inside the page's crop box, is rejected by the engine with a
typed error.

## J-SG-07: Sign an existing signature field, with seed values and a field lock
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_sign_field, e2e.macos.sg_sign_field

Steps: import the ECDSA P-256 signer's PKCS#12 (J-SG-05) and trust `jpdf Test Root` → open the
J-FM-08 output form (fields filled, empty signature field `Signature1` on page 2) in Read mode
→ hovering `Signature1` shows the tooltip "Unsigned signature field" → click it → the Sign
dialog opens for that field (no rectangle to drag) → sign → write and verify: the existing
field gets `/V` pointing at the new signature dictionary; no field is added; the digest is
SHA-256; the signature check holds → open a fixture whose empty signature field has a seed
value dictionary `/SV` with `/Reasons [(I approve) (I reviewed)]`, `/Ff` marking reasons as
required, `/DigestMethod [/SHA384]`, and a `/Lock` with `/Action /All` → click it and sign
with the same ECDSA ID → the reason is a choice between the two seed reasons only → the
signature's `/Reason` is the chosen one, its digest is SHA-384, and its `/Reference` holds a
FieldMDP transform with `/Action /All` (ISO 32000-2 §12.8.2.4) → every form field is now
read-only in our UI with the reason "Locked by signature Signature1", and the binding call to
fill one returns a typed error → the signature check holds.
Error branch: a seed value requiring `/SubFilter /adbe.pkcs7.sha1` (which we do not create) →
signing that field is inert with the engine's reason, and the binding call returns a typed
error; web only: signing the SHA-384 seed-value field with the RSA-2048 ID is inert with the
web RSA limit reason (Algorithms rule); a `/Lock` naming a field that does not exist → the
field is signed and the lock covers only existing fields.

## J-SG-08: Certify a document
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_certify, e2e.macos.sg_certify

Steps: import the RSA-2048 and ECDSA signers' IDs and trust `jpdf Test Root` → open an
unsigned 2-page form fixture `form.pdf` (text field `name`, empty signature field `Approver`)
→ Secure > Certify → the Certify dialog adds "Allowed changes after certifying" with "No
changes", "Form filling and signing" (default) and "Form filling, signing and comments" →
keep the default → drag a rectangle → Certify with the RSA ID → the save panel suggests
`form-certified.pdf` → save → write and verify: the signature dictionary has `/Reference`
with `/TransformMethod /DocMDP` and `/TransformParams` with `/P 2` and `/V /1.2`; the catalog
has `/Perms /DocMDP` pointing at it (ISO 32000-2 §12.8.2.2); the signature check holds and
pyHanko reports a certification signature with `/P 2` → the Signatures panel shows it as
certifying → Certify is inert with the reason "Only the first signature can certify", and its
binding call returns a typed error → the Annotate tools are inert with the reason "The
certification does not allow comments"; their binding calls return typed errors → fill
`name` → save (incremental) → the certification stays `Valid` with `Allowed changes` → sign
`Approver` with the ECDSA ID (J-SG-07) → both `Valid` → open `form.pdf` again and certify it
with "No changes" → Sign, form filling and every editing command are inert with the reason
"The certification does not allow changes" (binding calls return typed errors), while Add
validation data and Add document timestamp stay available (validation data is not a change)
→ certify a third copy with "Form filling, signing and comments" → add a highlight and save →
the certification stays `Valid` with `Allowed changes`.
Error branch: a fixture that already holds an approval signature → Certify is inert with
"Only the first signature can certify"; in the `/P 2` file, dragging a new signature
rectangle → refused with "The certification allows signing existing signature fields only"
(ISO 32000-2 Table 257 permits signing, not adding fields), and the binding call returns a
typed error.

## J-SG-09: Timestamps and long-term validation (PAdES B-T, B-LT, B-LTA)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_timestamp_ltv, e2e.macos.sg_timestamp_ltv

Steps: import the RSA-2048 signer's ID and trust `jpdf Test Root` → Preferences > Signatures
shows "Timestamp new signatures" off and the server URL empty → set the URL to the Certomancer
TSA (`http://127.0.0.1:<port>/tsa`) and turn the option on → open an unsigned fixture → Sign
→ the dialog's Timestamp is on and shows the server's host → Sign → exactly one request is
made: a POST of `application/timestamp-query` to that URL whose body decodes as an RFC 3161
TimeStampReq with the SHA-256 imprint of the signature value, a nonce and `certReq` true (web:
the network policy; macOS: the harness server's log) → write and verify: the CMS has a
signature-time-stamp unsigned attribute whose token's imprint and nonce match; the signature
check holds; the panel shows "Timestamped: … by «jpdf Test TSA»" → in the signature's details
choose Add validation data → the details list the URLs that will be contacted → Continue →
exactly 3 requests: OCSP for the signer, OCSP for `jpdf Test TSA`, and the CRL for `jpdf Test
CA` → save (incremental) → the catalog has `/DSS` with `/Certs`, `/OCSPs` and `/CRLs` holding
those responses and every certificate of both chains (ISO 32000-2 §12.8.4.3) → the signature
check holds at B-LT → Preferences > Signatures: set the archive timestamp server URL to the `jpdf Test Archive
TSA` endpoint → Add document timestamp → 1 request to that server, then 1 CRL request for
`jpdf Test Archive TSA`'s CRL → the save appends two updates: one holding a `/Type
/DocTimeStamp` signature with `/SubFilter /ETSI.RFC3161` whose byte range ends at the end of
that revision, then a DSS-only update adding the archive TSA's certificate and CRL (its
certificate is known only from the response, so it can't be inside the revision the
timestamp covers) → the document timestamp reports `No changes` ("Validation data added") → the signature check holds at
B-LTA → set the test clock past the expiry of the signer, `jpdf Test CA` and `jpdf Test TSA`,
but before `jpdf Test Archive TSA` and `jpdf Test Root` expire, with no online check → the
signature is still `Valid`, validated at the document timestamp's time by past signature
validation from the DSS, and no request is made.
Verify others' output: open a B-LTA fixture made by pyHanko → `Valid` with no request; a copy
whose timestamp token has one byte of its signature flipped → the "Timestamped" line is
absent, the details say "Timestamp is not valid", and the summary is computed at the current
time.
Error branch: with the TSA stopped, signing with Timestamp on shows a typed error naming the
server and writes nothing, and the dialog then offers "Sign without timestamp" (B-B);
opening five signed fixtures whose certificates carry OCSP and CRL URLs makes no request at
all.

## J-SG-10: Hostile timestamp, OCSP and CRL responses
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_network_hostile, e2e.macos.sg_network_hostile

Steps: point the timestamp server and the chain's OCSP and CRL URLs at a harness server that
answers each request with one of the bad responses below → sign with Timestamp on, and press
"Check revocation online" on S1 → each response is rejected with a typed error naming the
server and the problem, nothing is written to the file, and the app keeps working:
1. a TimeStampResp with a different nonce; with an imprint for other data; signed by a
   certificate without the timeStamping usage; with status "rejection";
2. a body one byte larger than the `Limits` response size (reading stops at the limit); a
   response that sends one byte per second (stopped by the 15 s timeout; the harness checks
   that less than 20 s passed);
3. DER nested deeper than `Limits`; a truncated body; an HTML page;
4. macOS: a redirect to another host (not followed) and one to the same host (followed once, after the resolved address passes the SSRF rule again);
   web: any redirect fails the request (`redirect: 'error'`);
5. an OCSP response for a different certificate; one signed by an untrusted responder; one
   whose `thisUpdate` is in the future → each leaves "Revocation was not checked" with the
   reason;
6. a CRL whose signature does not verify; a CRL with more entries than `Limits` allow.
SSRF check, with the test-only loopback flag off: a signed fixture whose signer certificate's
OCSP URL is `http://127.0.0.1:<port>/ocsp`, one with `http://localhost/`, and (macOS) one with
a host name the harness resolves to `10.0.0.1` → "Check revocation online" makes no request
and says the address is not allowed.
Check: across all of these no request goes anywhere except the configured TSA URL or a URL
from a certificate; no request carries cookies or a referrer (harness server log); no
response bytes are written to app storage.

## J-SG-11: Signatures in encrypted documents
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_encrypted, e2e.macos.sg_encrypted

Steps: import the RSA-2048 signer's ID and trust `jpdf Test Root` → open a fixture encrypted
with AES-256 R6 (open password a random sentinel; `/P` allows modifying, annotating, filling
and signing) and signed by pyHanko → enter the password → the signature is `Valid` (the
signature's `/Contents` is not encrypted, ISO 32000-2 §7.6.2) → sign it again → write and
verify: the update's strings and streams are encrypted with the file's key, except the new
signature's `/Contents`; qpdf, MuPDF and Poppler open it with the password; pyHanko's Python
API (decrypting with the password first) reports both signatures `intact` and `valid` → open
a fixture whose permissions forbid modifying, annotating and filling (bits 4, 6 and 9 clear)
→ Sign and Certify are inert with the permission reason, and their binding calls return
typed permission errors → enter the owner password (J-RD-29) → both are enabled.

### Platform convention rules (PC)

- **Writes are explicit.** The app never writes the user's file except when the user saves
  (Save, Save As, Sign, Certify, or the Save button of a dialog in this file). Crash-recovery
  data (J-PC-03) goes to app storage only.
- **Where the platform has a standard, use it** (Apple HIG; [wireframes-macos.md](../ui/wireframes-macos.md)
  §0 rule 3): the macOS document architecture's unsaved-changes alert and review-changes on
  quit, the system save panel, window tabs and the Window menu. Web draws the same choices
  with MUI ([wireframes-web.md](../ui/wireframes-web.md) W-21, W-22). macOS settings this
  needs: `autosavesInPlace` false, no autosave elsewhere, and system window restoration off
  for document windows (J-RD-24 and J-PC-03 do reopening), so the system never writes or
  reopens documents by itself.
- **Engine owns document state.** Whether a document has unsaved changes, what a recovery
  checkpoint holds, and whether the file on disk still matches what was opened are engine
  answers (CONSTITUTION 6); the native layer reports file events and shows dialogs.
- **A session reads one version.** At open the session takes a private snapshot of the file
  (macOS: an APFS clone in the app's temporary directory, falling back to a copy; web: a copy
  into OPFS, because a `File` from the handle throws `NotReadableError` once the file
  changes; the app renders from the `File` while the copy runs in the background, so the
  first page is not delayed, and a `NotReadableError` before the copy finishes is handled as
  a file change, J-PC-02) and reads only from it, so a document never mixes bytes from two
  versions of the file. After each save the snapshot is replaced by the saved bytes; it is
  deleted when the document closes; snapshots left by a crash are deleted at the next launch
  (web: only snapshots whose owning tab's Web Lock is free, so a live tab's snapshot is never
  deleted; recovery never needs them: J-PC-03 restores only against an unchanged source file). The
  forms-security.md redaction residue search covers snapshots.

### Language rules (J-PC-09, J-PC-10)

- **The four languages** are English (`en`), Vietnamese (`vi`), Japanese (`ja`) and Simplified
  Chinese (`zh-Hans`) (PLAN §8 row M10). The app's own text is everything it writes: menus,
  toolbars, sidebars, inspectors, dialogs, notices, error messages, tooltips, accessible names
  and the shortcuts list (J-PC-06). Text the platform supplies (macOS: the document
  architecture's alerts, the open, save and print panels, and the menu items AppKit adds, such
  as Services, Hide Others and Enter Full Screen) is the system's, in the language the app was
  launched in.
- **String catalogs.** Each platform has one catalog per language, keyed as its English one; a
  key with plural forms has the forms of that language's CLDR plural rules (English: one and
  other; Vietnamese, Japanese, Chinese: other). A contract test per platform checks that every
  catalog has exactly English's keys, no empty value, and per key the same placeholders; that
  every non-English value equal to its English value is on that catalog's reviewed
  same-as-English list (such as "PDF"), so an untranslated copy of English fails; and that
  every typed error code the binding exports has a catalog key.
- **Errors are translated by the app, not the engine.** The engine returns typed errors, a code
  and its values (CONSTITUTION 5); the app maps each code to its catalog entry and shows no
  engine text (CONSTITUTION 6: the native layer only displays).
- **Dates, times, numbers and sizes** use the platform's formatter (macOS: `FormatStyle`; web:
  `Intl`) with the app's language and the system's (web: the browser's first preferred
  locale's) region, as macOS does for a per-app language: `vi` on an `en-US` system formats as
  `vi-US`. The tests compare against that formatter's output, not against fixed strings.
- **The document is never translated.** Page content, the outline, annotation text and author
  names, form field values, the file name and the text of document properties (title, author,
  subject, keywords, producer) are shown exactly as stored, in every language, even when they
  equal a catalog string.
- **Which language.** With "Follow system" (the default), the app uses the first language in
  the user's preferred list (macOS: the system and per-app language settings; web:
  `navigator.languages`) that is one of the four, matched by language and script, the script
  being the tag's own or else its CLDR likely script: `vi-VN` is `vi`; `zh`, `zh-CN` and `zh-SG`
  are `zh-Hans`; `zh-TW`, `zh-HK`, `zh-Hant` and `zh-Hant-CN` match none. If none matches,
  English.
- **Spot strings** (fixed, so a catalog can't pass by copying English), two labels the app
  draws itself on both platforms: the sidebar's Thumbnails tab is "Thumbnails", "Hình thu nhỏ",
  "サムネイル", "缩略图", and the toolbar's Highlight tool is "Highlight", "Tô sáng",
  "ハイライト", "高亮" (en, vi, ja, zh-Hans), compared exactly.

## J-PC-01: Unsaved changes when closing and quitting
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_unsaved_changes, e2e.macos.pc_unsaved_changes

Steps: open fixtures A and B → rotate page 1 of A (A is marked edited) → close A's tab
(macOS Cmd+W; web the tab's close button) → the unsaved-changes dialog (W-21) appears,
attached to A, with Save the default, plus Don't Save and Cancel → Cancel → A stays open,
edited and rotated → close again → Don't Save → A closes; its file bytes are unchanged →
reopen A, rotate page 1 again, close → Save → A is saved incrementally (save and verify) and
closes → reopen A → page 1 is rotated → close B (not edited) → no dialog appears → New blank
document (J-PG-01), add a page, close → the dialog appears → Save → the Save As panel opens
(an untitled document has no file) → cancel the panel → the document stays open and edited
→ edit A and B, then quit (macOS Cmd+Q; web: close the browser tab) → macOS: the system's
review-changes flow asks for each edited document (the test answers Don't Save for A and
Save for B) → A's bytes are unchanged and B is saved; web: the browser's leave-page prompt
appears (Playwright `dialog` event of type `beforeunload`); a `beforeunload` handler is
registered only while some document is edited (checked again after Save clears the last
edit: closing the tab then raises no prompt).
Check: Return in the dialog chooses Save; Esc chooses Cancel; the dialog is found structurally
(macOS: a sheet with "Don't Save" and "Save" buttons; web `data-testid="unsaved-changes"`),
and closing an unedited document leaves the window with no sheet or dialog.

## J-PC-02: The open file changed on disk
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_file_changed, e2e.macos.pc_file_changed

Steps: open a 3-page fixture A at page 2 → the harness replaces A on disk with a 4-page
version (macOS: written by another process; web: a write through the file-handle adapter to
the same OPFS file) → the app checks (macOS: the file presenter's change event; web: when the
window regains focus, and every 5 s while visible, comparing size and modification time
through the adapter; the test triggers the focus check) → A is not edited, so it reloads by
itself, stays on page 2, the page indicator reads "2 / 4", and the notice "Reloaded because
the file changed on disk" appears → rotate page 1 (edited) → the harness changes the file
again → the file-changed dialog (W-22) appears with Reload and "Keep my version" (macOS title
case "Keep My Version"), Keep the default and the Esc action → Keep → the rotation stays and
nothing reloads → Save → the saved file is the session's snapshot bytes plus one update
section (save and verify); the other app's version is replaced, as the dialog said → the
harness changes the file again and the test triggers a check → Reload → the edits are gone,
the new version shows, and the document is not edited.
Error branch: the harness deletes A → the notice "The file was moved or deleted" appears; the
document stays open from its snapshot and is marked edited; Save opens the Save As panel →
macOS only: the harness renames A in its folder → the window title follows the new name (the
file presenter's move event) and Save writes to the new path.

## J-PC-03: Recover unsaved work after a crash
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_crash_recovery, e2e.macos.pc_crash_recovery

Steps: open fixture A, rotate page 2, add a blank page at the end, and wait for the recovery
checkpoint (written 2 s after the last committed change; the test waits for the app's
"checkpoint written" test event) → the checkpoint is in app storage only (macOS: the app
container, written by the app process from bytes the engine in the XPC helper produced; web:
OPFS) and holds the source file's identity (path or recent-files record, size, SHA-256) and
the edits as an incremental update against the session snapshot, not a copy of the whole file
→ kill the app (macOS `kill -9` on the app, which also ends its XPC helper; web: crash the
renderer with the DevTools protocol `Page.crash`) → launch again → the "Restore unsaved
changes?" dialog lists A with the time of its last checkpoint, with Restore (default) and
Discard → Restore → A opens with page 2 rotated and 4 pages, marked edited, and the notice
"Unsaved changes were recovered" → Save → save and verify; A on disk has the edits → A's
recovery data is deleted → kill and relaunch → no restore dialog, and the session snapshot left by the kill (PC rules)
is gone from app storage.
Second run: edit A, kill, relaunch → Discard → A's bytes are unchanged and its recovery data
is deleted → edit A, close it with Don't Save (J-PC-01) → its recovery data is deleted at
once.
Web, two tabs: open A in tab 1 and B in tab 2, edit both, wait for both checkpoints, crash tab
1 only, and open the app in a new tab → only A is offered (each app tab holds a Web Lock for
its own recovery records, and only records whose lock is free are offered) → tab 2's session snapshot is still in OPFS and
B keeps working in tab 2.
Encrypted documents: open an AES-256 R6 fixture whose open password is a 24-character random
sentinel, edit it, wait for the checkpoint → the update is encrypted with the document's key
as a save would write it; searching app storage for the page text sentinels and the password
finds nothing → kill, relaunch, Restore → the password prompt appears before anything shows.
Changed source: edit A, kill, change A on disk, relaunch → A's row says "The file changed
since; these changes can't be restored" and offers Discard only (the update cannot be applied
to other bytes).
XPC helper crash (macOS): edit A, wait for the checkpoint, kill only the XPC helper → A's tab
shows the document-failed state (W-15) with "Restore unsaved changes", which reopens A with
the edits as above.
Error branch: a recovery record truncated by the harness, one whose update section is hostile
(objects nested deeper than `Limits`), and one whose source file was deleted → each row names
the document and the problem and offers Discard only; nothing crashes.
Redaction: the forms-security.md redaction check (residue) also requires that a saved
redaction deletes recovery data holding the removed content.

## J-PC-04: Choose print content each time, or reuse the last choice
- milestone: M4
- platform: web, macos
- test: e2e.web.pc_print_preference, e2e.macos.pc_print_preference

Steps: open the J-RD-21 fixture → Preferences > Printing shows "Choose print content each
time" on (the default) → Print → the print options (J-RD-21) appear → choose "document only",
scale 50 %, 2 pages per sheet → print (the engine's print job is inspected as in J-RD-21; web
stubs `window.print()` in the test and records the call) → turn the preference off → Print →
our print options are skipped and the platform print step starts directly (macOS: the system
print panel; web: one call to the stubbed `window.print()`) with an engine job using the last
content, scale and pages per sheet, and page range All (the range is never reused) → the job
has no annotations and 2 pages per sheet → restart the app → the preference is still off and
the last choices are reused → turn it on → Print → the options appear, prefilled with the last
choices.
Error branch: open a fixture whose permissions forbid printing (bit 3 clear) → Print is inert
with the engine's reason (J-RD-29) with the preference on and off, and the binding call
returns a typed permission error.

## J-PC-05: Several windows
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_windows, e2e.macos.pc_windows

Steps (macOS): open fixtures A, B and C as tabs (J-RD-23) → Window > Move Tab to New Window on
B → B is in its own window, A and C stay as tabs → each window keeps its own sidebar, mode and
inspector state → the Window menu lists all three documents, and choosing C brings its window
to the front → open B again with File > Open → B's window comes to the front, no duplicate →
Window > Merge All Windows → the three are tabs of one window → close every window → the app
keeps running with no window → the reopen Apple Event (a Dock click) → the start screen
window opens.
Steps (web): open A in the app → open the app in a second browser tab → choose A from the
start screen's recent files (the stored handle; the click is the user gesture that
`requestPermission` needs) → the second tab says "This file is open in another jpdf tab" and
opens it read-only: the first tab holds a Web Lock named after A's recent-files record ID
(records are matched to handles with `isSameEntry` against the stored handles) → rotate is
inert there with that reason, and the binding call returns a typed error → close A in the
first tab → the second tab offers "Make editable" → choose it → rotate works there.

## J-PC-06: Keyboard shortcuts list
- milestone: M4
- platform: web, macos
- test: e2e.web.pc_shortcuts_list, e2e.macos.pc_shortcuts_list

Steps: Help > Keyboard Shortcuts (web: also `?` with the document view focused) → a window
(macOS) / dialog (web) lists the commands in the groups of [shortcuts.md](../ui/shortcuts.md)
with this platform's keys → the list is built from the app's command registry; a contract
test checks that the registry's commands, keys and groups equal the shortcuts.md rows whose
journeys belong to milestones up to the current one (from M10 on: every row) → type
`rotate view` in the filter field → exactly one row remains, "Rotate view right / left" → clear the filter → turn
off single-key shortcuts in Preferences → reopen the list → single-key rows are marked off
with the reason, and `?` no longer opens the list → Esc closes it (shortcuts.md rule 9).
Check: the list passes the accessibility audits (axe-core; the XCUITest audit), and each row's
accessible name is the command followed by its key.

## J-PC-07: Save As conventions and Revert
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_save_as, e2e.macos.pc_save_as

Steps: open fixture `report.pdf` → edit it → Save As (macOS Cmd+Shift+S; web the File menu) →
the name suggested is `report.pdf` (macOS: the save panel's name field; web:
`showSaveFilePicker` called with `suggestedName` `report.pdf` and the PDF type, through the
adapter) → enter `final` → the file is saved as `final.pdf` (the extension is added) → the
title and recent files show `final.pdf` → Save As to the name of an existing file → macOS: the
system asks to replace it (the test answers Replace); web: the adapter reports the overwrite
→ the file is replaced atomically (J-OS-06: a temporary file, then replace) → Save As onto
the open document's own file → a full rewrite in place (the J-OS-02 rewrite rule) → web with
the no-handles test flag: Save As shows the app's own name field (prefilled `final.pdf`) and
then downloads the file under that name (Playwright `download` event); the document stays
marked edited and the notice "Downloaded a copy; Save will not update it" appears → edit again
→ File > Revert to Saved → a confirmation, Cancel the default → confirm → the document reloads
from disk, not edited, the edits gone.
Error branch: Save As onto a file that is open in another tab → a typed error asks to close it
first; Save As into a folder the harness made read-only (macOS) / the adapter failing with
`NoModificationAllowedError` (web) → an error names the cause and nothing is written.

## J-PC-08: Use the web app offline
- milestone: M10
- platform: web
- test: e2e.web.pc_offline

Fixture `offline.pdf`: 12 pages; the word "harbor" appears exactly 3 times, on pages 2, 7 and
11; page 5 holds Japanese text in a Type0 font with `/Encoding /90ms-RKSJ-H` (a predefined
CMap, loaded from the CMaps the app ships) and an embedded subset of a permissively licensed
font as its CIDFont descendant; page 9 holds a random text sentinel.

Steps: load the app once with the network on → on this load the reader.md network rule holds
(only same-origin GETs for static app assets, none cross-origin) → its service worker installs
and becomes active, and the page links a same-origin web app manifest (`<link rel="manifest">`)
with a `name`, a `start_url` in the app's scope, `display` `standalone` and icons (the test
checks these, not the browser's own install prompt) → Cache Storage now holds exactly the
build's list of static assets (code, the engine's WebAssembly, fonts, CMaps and the strings of
every language the build ships; the list gives each entry's cache URL, the start URL included):
the same keys, no extra entries, and each body byte for byte the built file → turn the network
off (Playwright `setOffline`) and stop the test's static server → reload → the app starts from
the cache with no error and no offline warning → open `offline.pdf` (test handles per the
[reader.md](reader.md) shared rules) → all 12 thumbnails render, page 5's Japanese text renders
and matches the MuPDF and Poppler renders within the render-ratchet tolerance → search "harbor"
→ 3 hits, on pages 2, 7 and 11 → add a highlight and a sticky note → Save → the file is written
through its handle, as online → close the document → reopen it from the recent files list
(J-RD-03) → the highlight and the note are there, and MuPDF and Poppler render them → from the
offline reload on, every page request is answered by the service worker (Playwright
`response.fromServiceWorker()`) and none fails; with the server stopped, any fetch that reached
the network would fail, and the browser context's service-worker network events record none
except the browser's own update check, which refetches the registered service worker script and
the scripts it imports (matched by those URLs only), may fail offline and changes nothing →
Cache Storage still holds exactly the asset list above, and searching it for page 9's sentinel
and the fixture's bytes finds nothing.
Offline does not make every feature work: features that need the network and that the user
turns on (timestamps, online revocation checks, J-SG-09 and J-SG-10) can't reach their
servers; this journey uses none of them.

## J-PC-09: The app follows the system or browser language
- milestone: M10
- platform: web, macos
- test: e2e.web.pc_language_system, e2e.macos.pc_language_system

Fixture `languages.pdf`: 3 pages, over 1 MB (so its size shows a decimal part); document
title "Save" and author "Lan"; a creation date; outline entries "Chapter One" and "Open"; a
sticky note with the text "Ghi chú tiếng Việt", author "Lan" and a modification date (`/M`).
Steps, run once per language `en`, `vi`, `ja`, `zh-Hans` (macOS: launch with
`-AppleLanguages (<lang>)`; web: a Playwright context with `locale` `en-US`, `vi-VN`, `ja-JP`,
`zh-CN`): launch → the start screen, and every menu item and toolbar control from the app's
command registry (J-PC-06), equal that catalog's entries; the spot strings are that
language's; web: `<html lang>` is the language tag → open `languages.pdf` → the page
indicator, the outline ("Chapter One" and "Open", untranslated), the annotation list (the
note's text and author unchanged, its date in the formatter's output) and Document Properties
(title "Save" and author "Lan" unchanged; the creation date and the file size in the
formatter's output) → open the shortcuts list (J-PC-06) → every command name and group
heading is the catalog's → open `not-a-pdf.pdf` (text bytes) → the open-failed notice
([information-architecture.md](../ui/information-architecture.md), "Open failed") is the
catalog's entry for the engine's "not a PDF" code, with the file name unchanged → the
accessibility audits (axe-core; the XCUITest audit) pass, and each accessible name checked is
the catalog's.
Fallback branch (web: the context `locale` is the first tag, and an init script sets
`navigator.languages` to the list): the list `fr-FR`, `ja-JP` → Japanese; `zh` → Simplified
Chinese; `zh-SG` → Simplified Chinese; `zh-TW` → English; `zh-HK` → English; `zh-Hant` →
English; `fr-FR` → English.

## J-PC-10: Choose the app's language
- milestone: M10
- platform: web, macos
- test: e2e.web.pc_language_choose, e2e.macos.pc_language_choose

Steps: launch with the system (web: browser) locale `en-US` → Preferences > Language lists
"Follow system" (selected) and the four languages, each named in its own language (English,
Tiếng Việt, 日本語, 简体中文); on macOS a note under it says that system menus and dialogs
change at the next launch → open `languages.pdf` (J-PC-09's fixture), go to page 2, set the
zoom to 150%, add a highlight without saving; web: set a sentinel property on `window` →
choose Tiếng Việt → the app's own text (the command registry's items the app draws, the spot strings, the
sidebar, the inspector, Preferences) switches to Vietnamese at once (on macOS the menu items the
system supplies, the File menu's document commands among them, keep the launch language until
the relaunch), with no relaunch (macOS: the same
process) and no page reload (web: the sentinel is still set, and Playwright records no `load`
or main-frame `framenavigated` event); web: `<html lang>` is `vi` → the document is still open
on page 2 at 150%, the highlight is there, the document is marked edited, and Undo removes the
highlight → Redo → the note's date and Document Properties' creation date and size are the
formatter's output for `vi` with the region US, not for `en-US` → quit (J-PC-01's
unsaved-changes prompt; discard) → launch again → the app is in Vietnamese while the system
locale is still `en-US`; on macOS the platform-supplied text is too: add a highlight to
`languages.pdf` and close it → the unsaved-changes alert's text equals that alert's text in a
launch with `-AppleLanguages (vi)`; discard → choose 日本語, then 简体中文 → each applies at
once (its spot strings) → choose "Follow system" → English again, at once; after a relaunch,
still English → launch with the system locale `vi-VN` and "Follow system" → Vietnamese.
