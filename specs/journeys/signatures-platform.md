# Journeys: signatures and platform conventions

Areas `SG` (digital signatures: verify, sign, certify, digital IDs, trust, timestamps and
long-term validation) and `PC` (platform conventions: unsaved changes, file changed on disk,
crash recovery, Save As, print preference, multiple windows, the shortcuts list). PLAN §4
(Security & signatures; Optimize & save), PLAN §1 R12, feature groups F-SC-2 … F-SC-4 and the
platform rows of F-RD-1, F-RD-5 and F-OS-1 in [specs/TRACE.md](../TRACE.md). Inventory and
coverage: [RESEARCH.md](RESEARCH.md). Format: ops/judge/CONTRACT.md §4.
Milestones (PLAN §8): signatures are M9; conventions that need saving (unsaved changes, file
changed on disk, crash recovery, Save As) are M6 (writer); reader conventions (print
preference, windows, shortcuts list) are M4.

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
  Every one of those inputs is hostile and parsed under `Limits` (byte size, ASN.1 nesting
  depth, certificates per chain, chain length, revisions compared, objects compared), with a
  typed error, never a panic. The native layer does only three things: the private-key
  operation through the platform signer (macOS: a Keychain `SecKey`; web: a non-extractable
  WebCrypto `CryptoKey`), sending the network requests the user enabled, and displaying the
  engine's results.
- **The returned signature is checked before it is embedded** (PLAN §6.4). The engine verifies
  the signature value the platform signer returns against the signing certificate's public key
  and the exact signed attributes it asked to sign. A wrong value (test signer hook: flipped
  bit, value for other data, value from another key, a value longer than the reserved
  `/Contents` space) is a typed error, and no file is written.
- **Algorithms.** We create SHA-256 (SHA-384 for P-384 keys) digests with RSA PKCS#1 v1.5
  (2048 bits or more) or ECDSA P-256 or P-384 signatures. We verify SHA-256, SHA-384 and
  SHA-512 with RSA PKCS#1 v1.5, RSA-PSS and ECDSA P-256/P-384/P-521. MD5, SHA-1 and RSA keys
  under 2048 bits are reported as a weak algorithm (summary Invalid, below); any other
  algorithm is reported as unsupported (summary Unverified).
- **Trust list.** Trust anchors come only from the app's own trust list, which is empty after
  installation. The user adds certificates to it (J-SG-04); it is stored in app storage
  (macOS: the app container; web: the origin's IndexedDB) and passed to the engine as
  untrusted bytes. System root stores and public trust lists are not used (a later decision,
  tracked as a discovery issue). Certificates of the user's own digital IDs are not trusted
  automatically.
- **Statuses.** For each signature the engine returns four facts and a summary, shown in the
  Signatures panel (IA §4) and the signature details:
  - Integrity: `Intact` (the digest matches and the byte range checks below hold), `Altered`
    (the digest does not match) or `Cannot check` (the signature is malformed, or the byte
    range checks fail).
  - Signer: `Trusted` (the chain reaches the trust list and every certificate was valid at
    the validation time), `Not trusted`, `Expired`, `Revoked`, `Weak algorithm` or
    `Unsupported algorithm`; plus the line "Revocation was not checked" when no revocation
    data was embedded and no online check was made.
  - Changes since signing: `No changes`, `Allowed changes` (with the count of later
    revisions) or `Changes not allowed`, with the engine's list of changes per revision.
  - Time: "Signing time (claimed by the signer): …" from `/M`, and, when a timestamp token
    validates, "Timestamped: … by «TSA name»". The validation time is the timestamp's time
    when one validates, otherwise the current time.
  - Summary: `Invalid` if integrity is Altered or Cannot check, the signer is Revoked (at or
    before the validation time) or uses a weak algorithm, or there are changes not allowed;
    otherwise `Unverified` if the signer is Not trusted, Expired (with no valid timestamp
    from before expiry) or uses an unsupported algorithm; otherwise `Valid`. The panel header
    shows a one-line document summary built from these: "All signatures are valid", "N
    invalid signatures" or "N signatures could not be verified" (invalid wins).
- **Byte range checks** (ISO 32000-2 §12.8.1). `/ByteRange` must have exactly four
  non-negative integers `[0 a b c]` with a > 0, b = a + the length of the signature's own
  `/Contents` hex string including its `<` `>` delimiters, which starts at offset a; the
  second range must end exactly at the end of a revision the engine found (its `%%EOF`
  line); the integers must not overflow or overlap and must lie inside the file. Every
  signature dictionary the engine reports is the one at that position, so a dictionary
  shown in the page tree but not covered by its own byte range is `Cannot check`.
- **Clock.** The engine reads the current time through its clock trait; tests set it.
- **Fixtures** are generated by the harness, never taken from a corpus. A test PKI made with
  Certomancer (MIT, run as an external program, fixed seed) provides: root `jpdf Test Root`,
  intermediate `jpdf Test CA`, signer certificates (valid RSA-2048, valid ECDSA P-256,
  expired, revoked, without the digitalSignature key usage, one signed with SHA-1, one
  RSA-1024), a TSA certificate with the timeStamping extended key usage and an OCSP responder
  certificate. Certomancer also serves the test TSA, OCSP and CRL endpoints on
  `127.0.0.1`. Signed fixtures are made with pyHanko (an external program) from that PKI.
- **External validators.** Signatures we create are checked with pyHanko
  (`pyhanko sign validate --pretty-print --trust <root.pem>`, adding `--ltv-profile pades` or
  `--ltv-profile pades-lta` for B-LT and B-LTA) and Poppler's `pdfsig -nssdir <test NSS
  db>` (its "Signature Validation", "Certificate Validation", "Signature Type", "Signed
  Ranges" and "Total document signed" lines). The CMS structure is decoded with asn1crypto
  in the harness. "Signature check" in a journey means: pyHanko reports the signature intact
  and valid with the stated level, `pdfsig` reports "Signature is Valid." and "Total document
  signed" (for the last signature), and our engine reports the same.
- **PAdES output** (ETSI EN 319 142-1, ISO 32000-2 §12.8). Every signature we create is
  `/Type /Sig`, `/Filter /Adobe.PPKLite`, `/SubFilter /ETSI.CAdES.detached`, with `/M`, a
  `/Contents` reservation sized by the engine and zero-padded, and a CMS SignedData whose
  signed attributes are exactly content-type, message-digest and signing-certificate-v2 (no
  signing-time attribute: the claimed time is `/M`). It is written as one incremental update
  appended to the document, and pending edits go into the same update.
- **Network is off by default** (PLAN §4). Timestamps need "Timestamp new signatures" turned
  on and a timestamp server URL set in Preferences > Signatures; online revocation checks run
  only when the user presses "Check revocation online" in a signature's details, never when a
  document opens (URLs in certificates come from the file and would reveal that it was
  opened). URLs must be `https:`, or `http:` to a loopback address; requests send no cookies
  or credentials and no referrer, follow no redirect to another host, and stop at the
  `Limits` response size and a 15-second timeout. A timestamp request carries only the
  message imprint and a nonce; an OCSP request only the certificate IDs. Responses are hostile
  input (CONSTITUTION 5). Web: these are the only requests allowed besides the reader rules'
  same-origin asset GETs, and each one goes to the configured TSA URL or the URL the engine
  returned from the certificate. The web build's content security policy must allow them;
  how is decided before M9 (tracked as a discovery issue); the test build allows the harness
  endpoints.

### Platform convention rules (PC)

- **Writes are explicit.** The app never writes the user's file except when the user saves
  (Save, Save As, Sign, Certify, or the Save button of a dialog in this file). Crash-recovery
  data (J-PC-03) goes to app storage only.
- **Where the platform has a standard, use it** (Apple HIG; [wireframes-macos.md](../ui/wireframes-macos.md)
  §0 rule 3): the macOS document architecture's unsaved-changes alert and review-changes on
  quit, the system save panel, window tabs and the Window menu. Web draws the same choices
  with MUI ([wireframes-web.md](../ui/wireframes-web.md) W-21, W-22).
- **Engine owns document state.** Whether a document has unsaved changes, what a recovery
  snapshot holds, and whether the file on disk still matches what was opened (size,
  modification time and a SHA-256 of the bytes the session read) are engine answers
  (CONSTITUTION 6); the native layer reports file events and shows dialogs.

## J-SG-01: Verify a valid signature in the Signatures panel
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_verify_valid, e2e.macos.sg_verify_valid

Steps: open an unsigned 2-page fixture → open the Signatures panel → it shows "No signatures"
→ open a fixture signed by pyHanko (PAdES B-B, RSA-2048 signer under `jpdf Test CA`, SHA-256,
visible signature on page 1, `/M` 2026-03-01T10:00:00Z, reason "Approved") with the trust
list empty → the panel lists one signature with the signer's common name, the summary
`Unverified`, integrity `Intact`, signer `Not trusted`, changes `No changes`, "Signing time
(claimed by the signer): 2026-03-01 10:00 UTC" shown in local time, and "Revocation was not
checked" → the panel header says "1 signatures could not be verified"; Secure mode's tool row
is unchanged and nothing is modified → add `jpdf Test Root` to the trust list (J-SG-04) →
the summary becomes `Valid`, signer `Trusted`, and the header "All signatures are valid" →
click the signature in the panel → the view scrolls to its widget on page 1 and selects it →
the details show reason "Approved", the signature type `ETSI.CAdES.detached`, the digest
`SHA-256`, the signed byte ranges and "covers the whole document" → open a fixture with two
approval signatures in two revisions → the panel lists both in signing order; the first says
`Allowed changes` (1 later revision: the second signature) and both are `Valid` → reopen the
first file → the file bytes are unchanged (reader rule).
Error branch: a fixture whose signature widget is hidden (`/F` Hidden) and has a zero-size
`/Rect` is still listed (an invisible signature), and clicking it says it has no visible
appearance.

## J-SG-02: Reject altered, malformed and forged signatures
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_verify_invalid, e2e.macos.sg_verify_invalid

Steps: with `jpdf Test Root` trusted, open each fixture below (each derived from the J-SG-01
fixture by the harness editing bytes in a `Vec<u8>`) → each opens and renders, and the panel
shows the stated result; nothing panics, and opening each takes no longer than the reader's
`Limits` allow:
1. One byte of page 1's content stream changed inside the signed range → integrity `Altered`,
   summary `Invalid`.
2. `/ByteRange` edited so its gap is 10 bytes wider than `/Contents` → `Cannot check`,
   `Invalid`.
3. `/ByteRange [0 0 0 0]`, `/ByteRange` missing, `/ByteRange` with five entries, with a
   negative entry, with values near 2^63 (overflow when added) and with a range past the end
   of the file → each `Cannot check`, `Invalid`.
4. `/Contents` missing, `/Contents ()` (empty), `/Contents` as a literal string instead of a
   hex string and `/Contents` holding random bytes → each `Cannot check`, `Invalid`.
5. Signature wrapping: the signed revision's bytes moved after a new revision that redefines
   the signature dictionary, so the byte range points at a different `/Contents` → `Cannot
   check`, `Invalid` (the byte range checks).
6. A CMS whose message digest is for other bytes → `Altered`, `Invalid`.
7. CMS signed by the SHA-1 signer and by the RSA-1024 signer → signer `Weak algorithm`,
   `Invalid`; one using an unknown signature algorithm OID → `Unsupported algorithm`,
   `Unverified`.
8. Signed by the revoked signer, with an embedded OCSP response (pyHanko, revocation info
   archival) saying revoked before `/M` → `Revoked`, `Invalid`.
9. Signed by the expired signer with no timestamp → `Expired`, `Unverified`.
10. Signed with the certificate that lacks the digitalSignature key usage → `Not trusted`
    with the reason "certificate not allowed for signing", `Unverified`.
11. Hostile CMS: ASN.1 nested deeper than `Limits`, a length field larger than the file,
    10 000 certificates in the SignedData, a certificate chain that loops (A issued by B,
    B by A) and indefinite-length encodings inside DER → each `Cannot check` or `Not
    trusted` with a typed reason, `Invalid` or `Unverified` as the rules say, and the
    engine's peak memory stays within `Limits`.
Check: the panel header for a document holding fixtures 1 and 9 (two signatures) says "1
invalid signatures"; the same results come from the engine through `jpdf-cli
verify-signatures` (CONTRACT); pyHanko agrees on fixtures 1, 2, 6, 7 and 8 (it reports them
as not intact or not valid).

## J-SG-03: Changes after signing, certification levels and field locks
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_changes_mdp, e2e.macos.sg_changes_mdp

Steps: with `jpdf Test Root` trusted, open fixtures made by pyHanko from a 2-page form (text
field `name`, check box `ok`, empty signature field `Approver`), each with a certification
signature and then an incremental update made by the harness:
1. `/P 2` (DocMDP; ISO 32000-2 §12.8.2.2), update fills `name` → changes `Allowed changes`
   (1 revision: "Form field name filled"), summary `Valid`.
2. `/P 2`, update adds a Square annotation on page 1 → `Changes not allowed` ("Annotation
   added on page 1"), `Invalid`.
3. `/P 3`, the same Square annotation → `Allowed changes`, `Valid`.
4. `/P 1`, update fills `name` → `Changes not allowed`, `Invalid`.
5. `/P 2`, update signs `Approver` with a second approval signature → both `Valid`; the
   certification signature lists "Signature field Approver signed".
6. Any `/P`, update adds a DSS dictionary and a document timestamp → `Allowed changes`
   (validation data is allowed at every level, ISO 32000-2 §12.8.4.3 and ETSI EN 319 142-1).
7. `/P 2`, update redefines the object of page 1's content stream (same object number,
   new content: "hide and replace") → `Changes not allowed` ("Page 1 content changed"),
   `Invalid`, even though the update also fills `name`.
8. `/P 2`, update gives `name` a new `/AP /N` that draws a white box over half of page 1
   while filling it → `Changes not allowed` ("Appearance of field name covers page
   content": an appearance whose bounding box exceeds the widget `/Rect`), `Invalid`.
9. An approval signature (no certification) whose signature field has a FieldMDP `/Lock`
   (`/Action /Include /Fields [(name)]`), update changes `name` → `Changes not allowed`
   ("Locked field name changed"), `Invalid`; a second fixture changing only `ok` →
   `Allowed changes`, `Valid`.
Then, for fixture 2: open the signature's details → View signed version → a read-only tab
opens with the revision the signature covers (the bytes up to the end of its byte range) →
page 1 has no Square annotation → it is labeled as the signed version, and Save is
disabled with that reason.
Engine check: fixtures that differ only in object numbering or object stream packing (the
same objects re-serialized by qpdf in an update) report `No changes` for content; the
comparison stops at the `Limits` object count with `Cannot check` changes and a typed reason,
never a hang.

## J-SG-04: Certificate details and the trust list
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_trust, e2e.macos.sg_trust

Steps: open the J-SG-01 fixture with the trust list empty → in the signature's details
choose Show certificate → the chain lists signer → `jpdf Test CA` → `jpdf Test Root` (taken
from the CMS) → for each certificate the view shows subject, issuer, serial number, validity
dates, key type and size, key usage, extended key usage and the SHA-256 fingerprint, all as
the engine's display strings (control and bidi characters escaped as in the reader link
policy) → select `jpdf Test Root` → Trust → a confirmation names the certificate and its
fingerprint and says signatures it vouches for will be shown as valid; Cancel is the default
→ Cancel → nothing changes → Trust and confirm → the signature becomes `Valid` → restart the
app and reopen → still `Valid` (the trust list persisted) → Preferences > Signatures >
Trusted certificates lists `jpdf Test Root` → import a certificate file (`.cer` DER and
`.pem` with two certificates) → both certificates are added → remove `jpdf Test Root` →
confirmation, Cancel default → confirm → the J-SG-01 signature returns to `Unverified`.
Error branch: importing a file that is not a certificate, a PEM with garbage between blocks,
and a 50 MB file are rejected with a typed error and add nothing; a certificate whose subject
holds a right-to-left override character shows it escaped; trusting an end-entity certificate
(the signer itself) works and makes only signatures by that certificate `Valid`.

## J-SG-05: Manage digital IDs (import a PKCS#12, create a self-signed ID)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_digital_ids, e2e.macos.sg_digital_ids

Steps: Secure > Digital IDs → the list shows "No digital IDs" → Import → pick the RSA-2048
signer's PKCS#12 (made with `-keypbe AES-256-CBC -certpbe AES-256-CBC -macalg sha256`, chain
included) → the password prompt appears → enter a wrong password → a wrong-password message
appears in place → enter the right one → the list shows the ID with name, issuer, expiry,
key type `RSA 2048` and storage (macOS: "Keychain"; web: "This browser") → the private key
was handed to the platform key store as non-extractable (macOS: `kSecAttrIsExtractable`
false; web: `extractable: false`, checked by calling `crypto.subtle.exportKey` in the test,
which fails) and the engine keeps no copy (the binding has no call that returns key bytes) →
Create → fill name `Ada Test`, organization `jpdf`, email `ada@example.test`, country `GB`,
key type ECDSA P-256 (the default; RSA 3072 is the other choice), validity 3 years → Create →
the platform generates the key (macOS `SecKeyCreateRandomKey` in the Keychain; web
`generateKey` with `extractable: false`) → the engine builds the certificate (X.509 v3,
subject and issuer equal, key usage digitalSignature and nonRepudiation, basic constraints
not a CA, serial from the engine's random trait), has the platform sign it and verifies the
signature (PLAN §6.4) → the list shows `Ada Test` → Export certificate → a `.cer` file is
written that OpenSSL parses with the fields above and no private key → restart the app → both
IDs are still listed → remove `Ada Test` → confirmation says signing with it will no longer
be possible and it cannot be undone; Cancel is the default → confirm → it is gone, and its
key is deleted from the platform store (macOS: `SecItemCopyMatching` finds nothing; web: the
IndexedDB record is gone).
Storage check: the J-RD-04 storage search for the PKCS#12 password, the PKCS#12 file's bytes
and the private key's PKCS#8 bytes finds nothing in app storage, logs or the console. Web
honesty note shown in the Digital IDs dialog and tested: the browser keeps the
non-extractable key in its own profile storage, which this search does not cover.
Error branch: a PKCS#12 with no private key, one whose key does not match its certificate,
one encrypted with RC2-40 (rejected: "re-export the ID with AES"), one with a PBKDF2 or
PKCS#12 KDF iteration count above `Limits`, one nested deeper than `Limits` and a 50 MB file
→ each a typed error, nothing added, no hang; Create with an empty name, a country that is
not two letters or a validity of 0 or 11 years → the field shows the engine's error and Create
stays inert; an expired imported ID is listed as `Expired` and is not offered for signing.

## J-SG-06: Sign a document (approval signature, PAdES B-B)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_sign, e2e.macos.sg_sign

Steps: import the RSA-2048 signer's PKCS#12 (J-SG-05) and trust `jpdf Test Root` → open an
unsigned 3-page fixture → add a highlight on page 2 (an unsaved edit) → Secure > Sign → the
status bar says to drag a rectangle where the signature goes → drag a 200 × 60 pt rectangle
on page 3 → the Sign dialog opens with: the digital ID (the imported one is selected), reason,
location and contact (optional, empty), appearance options (name, date, reason, a saved
visual signature from J-ANN-15 or none), an engine-rendered preview, "Timestamp" (off and
disabled with the reason "no timestamp server set in Preferences"), and a note that unsaved
changes will be saved with the signature → type reason `Approved` → Sign → the save panel
suggests `<name>-signed.pdf` → save → the platform signer is asked once to sign → the
document now shows the new file name and is not marked edited → write and verify:
- the new file starts with the original file's bytes unchanged; one update section follows
  with the highlight, a new signature field `Signature1` on page 3 whose widget `/Rect` is
  the dragged rectangle, its `/AP /N` generated by the engine (it renders the name, the date
  and `Approved` in MuPDF and Poppler within the render-ratchet tolerance of our render) and
  `/AcroForm /SigFlags 3`;
- the PAdES output rule holds; `/Reason (Approved)`; `/M` equals the test clock;
- the signature check holds (pyHanko level B-B, `pdfsig` valid and total document signed);
- the original file's bytes are unchanged on disk.
→ the Signatures panel lists the signature as `Valid`, `No changes` → Sign again with the
"Invisible signature" choice instead of a rectangle → a second update holds a signature field
with a zero-size `/Rect` and the widget flags Hidden and Print clear (ISO 32000-2 §12.7.5.5)
→ both signatures are `Valid`; the first reports `Allowed changes` (1 revision).
Error branch: the test signer hook returns each wrong value from the "returned signature is
checked" rule → each is a typed error, no file is written and the open document is unchanged;
the user cancels the platform's key-use prompt (macOS Keychain; test hook) → nothing is
written; an ID whose certificate lacks digitalSignature is not offered; a rectangle smaller
than 12 × 12 pt or off the page is rejected by the engine with a typed error.

## J-SG-07: Sign an existing signature field, with seed values and a field lock
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_sign_field, e2e.macos.sg_sign_field

Steps: open the J-FM-08 output form (fields filled, empty signature field `Signature1` on
page 2) in Read mode → `Signature1` shows an "unsigned signature field" tooltip → click it →
the Sign dialog opens for that field (no rectangle to drag) → sign with the ECDSA P-256 ID →
write and verify: the existing field object gets `/V` pointing at the new signature
dictionary; no new field is added; the signature check holds; the digest is SHA-256 → open a
fixture whose empty signature field has a seed value dictionary `/SV` with `/Reasons [(I
approve) (I reviewed)]` and `/Ff` making reasons required, `/DigestMethod [/SHA384]`, and a
`/Lock` with `/Action /All` → click it → the reason is a choice between the two seed reasons
only, and the digest used is SHA-384 → sign → the signature dictionary's `/Reason` is the
chosen one; the signature field gets the `/Lock` applied as a FieldMDP transform
(`/Reference` with `/TransformMethod /FieldMDP`, ISO 32000-2 §12.8.2.4) → after signing,
every form field is read-only in our UI with the reason "locked by signature Signature1" and
the binding call to fill one returns a typed error → the signature check holds.
Error branch: a seed value `/SubFilter` we do not create (`/adbe.pkcs7.sha1`) with the
required flag set → Sign is inert for that field with the engine's reason; a field whose
`/Lock` lists a field name that does not exist is signed and the lock covers only existing
fields.

## J-SG-08: Certify a document
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_certify, e2e.macos.sg_certify

Steps: open an unsigned 2-page form fixture (text field `name`, empty signature field
`Approver`) → Secure > Certify → the Certify dialog adds "Allowed changes after certifying":
"No changes", "Form filling and signing" (default) and "Form filling, signing and comments"
→ keep the default → drag a rectangle → Certify → save as `<name>-certified.pdf` → write and
verify: the signature dictionary has `/Reference` with `/TransformMethod /DocMDP` and
`/TransformParams` `/P 2` `/V /1.2`; the catalog has `/Perms /DocMDP` pointing at it (ISO
32000-2 §12.8.2.2); the signature check holds and pyHanko reports it as a certification
signature with fill-forms permissions → the Signatures panel shows it as certifying →
Certify is now inert with the reason "the document is already signed" → in Annotate mode the
tools are inert with the reason "the certification does not allow comments"; their binding
calls return typed errors → fill `name` → save (incremental) → the certification stays
`Valid` with `Allowed changes` → sign `Approver` (J-SG-07) → `Valid` and allowed → open the
same fixture again and certify it with "No changes" → afterwards Sign, form filling and every
editing command are inert with the reason "the certification does not allow changes" →
certify a third copy with "Form filling, signing and comments" → adding a highlight is
allowed and the certification stays `Valid` after saving.
Error branch: a fixture that already holds an approval signature → Certify is inert with the
reason "only the first signature can certify"; a fixture with `/P 2` certification where the
user drags a new signature rectangle → Sign is inert with "the certification allows signing
existing fields only" (ISO 32000-2 Table 257 lists signing, not adding fields).

## J-SG-09: Timestamps and long-term validation (PAdES B-T, B-LT, B-LTA)
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_timestamp_ltv, e2e.macos.sg_timestamp_ltv

Steps: import the RSA-2048 signer's ID and trust `jpdf Test Root` → in Preferences >
Signatures, "Timestamp new signatures" is off and the server URL empty → set the URL to the
Certomancer TSA (`http://127.0.0.1:<port>/tsa`) and turn the option on → open an unsigned
fixture → Sign → the dialog's "Timestamp" is on, with the server's host shown → Sign → exactly
one request is sent: a POST of `application/timestamp-query` to that URL whose body decodes as
an RFC 3161 TimeStampReq with the SHA-256 imprint of the signature value and a nonce, and no
other request is made (web: the network policy; macOS: the harness's request log) → write and
verify: the CMS has a signature-time-stamp unsigned attribute whose token's imprint matches;
the signature check holds at level B-T; the panel shows "Timestamped: … by «jpdf Test TSA»"
→ open the signature's details → "Add validation data" → the details list the OCSP and CRL
URLs from the chain and the TSA chain → Continue → one OCSP request per certificate that has
an OCSP URL and one CRL request per certificate that has only a CRL URL → save (incremental)
→ the catalog has `/DSS` with `/Certs`, `/OCSPs` and `/CRLs` covering the signer and TSA
chains (ISO 32000-2 §12.8.4.3) → pyHanko validates level B-LT → in the details choose "Add
document timestamp" → a second TSA request → a new update with a `/Type /DocTimeStamp`
signature (`/SubFilter /ETSI.RFC3161`) → pyHanko with `--ltv-profile pades-lta` reports it
valid → set the test clock 20 years ahead (every certificate expired) with online checks off
→ the signature is still `Valid`, validated at the timestamp's time from embedded data, with
no network request.
Error branch: with the TSA stopped, signing with Timestamp on shows a typed error naming the
server and writes nothing; the dialog then offers to sign without a timestamp (B-B); with
online checks never pressed, opening a signed document makes no request at all (the network
log is empty after opening five signed fixtures whose certificates carry OCSP and CRL URLs).

## J-SG-10: Hostile timestamp, OCSP and CRL responses
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_network_hostile, e2e.macos.sg_network_hostile

Steps: point the timestamp server and the chain's OCSP and CRL URLs at a harness server that
answers each request with one of the bad responses below → sign with Timestamp on, and press
"Check revocation online" on a signed fixture → each response is rejected with a typed error
that names the server and the problem, nothing is written to the file, and the app keeps
working:
1. a TimeStampResp with a different nonce, with an imprint for other data, signed by a
   certificate without the timeStamping usage, or with status "rejection";
2. a body larger than the `Limits` response size (the read stops at the limit), and a
   response that sends one byte per second (stopped at 15 s; the test clock does not apply
   to network timeouts, so the harness checks the elapsed time is under 20 s);
3. DER nested deeper than `Limits`, a truncated body, and a non-DER body (an HTML page);
4. an HTTP redirect to another host (not followed) and one to the same host (followed once);
5. an OCSP response for a different certificate, one signed by an untrusted responder, and
   one whose `thisUpdate` is in the future → the revocation status stays "Revocation was not
   checked" with the reason;
6. a CRL whose signature does not verify, and one with 1 000 000 entries (over `Limits`).
Check: across all of these no request goes anywhere except the configured URL or the URL
from the certificate; no request carries cookies or a referrer (harness server log); no
response bytes are written to app storage.

## J-SG-11: Signatures in encrypted documents
- milestone: M9
- platform: web, macos
- test: e2e.web.sg_encrypted, e2e.macos.sg_encrypted

Steps: open a fixture encrypted with AES-256 R6 (open password `open`, signing and form
filling allowed by `/P`) and signed by pyHanko → enter the password → the signature is
`Valid` (the engine decrypts strings and streams to render, but the signature's `/Contents`
is not encrypted, ISO 32000-2 §7.6.2) → sign it again with our ID → write and verify: the
update's strings and streams are encrypted with the file's key, except the new signature's
`/Contents`; qpdf, MuPDF and Poppler open it with the password; pyHanko (given the password)
reports both signatures valid → open a fixture whose permissions forbid modifying, filling and
annotating (bits 4, 6 and 9 clear) → Sign and Certify are inert with the permission reason,
and their binding calls return typed permission errors; entering the owner password (J-RD-29)
enables them.

## J-PC-01: Unsaved changes when closing and quitting
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_unsaved_changes, e2e.macos.pc_unsaved_changes

Steps: open fixtures A and B → rotate page 1 of A (A is marked edited) → close A's tab
(macOS Cmd+W; web the tab's close button) → the unsaved-changes dialog (W-21) appears,
attached to A, with Save as the default and Don't Save and Cancel → Cancel → A stays open,
still edited and rotated → close again → Don't Save → A closes; its file bytes are unchanged
→ reopen A, rotate page 1 again, close → Save → A is saved incrementally (save and verify) and
closes → reopen A → page 1 is rotated → close B (not edited) → no dialog appears → New blank
document (J-PG-01), add a page, close → the dialog appears → Save → the Save As panel opens
(an untitled document has no file) → cancel the panel → the document stays open and edited
→ edit A and B, then quit (macOS Cmd+Q; web: close the browser tab) → macOS: the system's
review-changes flow asks for each edited document (test hook answers Don't Save for A and
Save for B) → A's bytes are unchanged and B is saved; web: the browser's leave-page prompt
appears (`beforeunload` registered only while a document is edited; Playwright's `dialog`
event of type `beforeunload`) and no `beforeunload` handler is registered while nothing is
edited.
Check: pressing Return in the dialog chooses Save; Esc chooses Cancel; the dialog is found
structurally (macOS: a sheet with "Don't Save" and "Save" buttons; web
`data-testid="unsaved-changes"`), and when no document is edited closing leaves the window
with no sheet or dialog.

## J-PC-02: The open file changed on disk
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_file_changed, e2e.macos.pc_file_changed

Steps: open a 3-page fixture A at page 2 → the harness replaces A on disk with a 4-page
version (macOS: written by another process; web: the file-handle adapter's write to the
same OPFS file) → when the app next checks (macOS: on the file presenter's change event;
web: when the window regains focus, and every 5 s while visible, comparing size and
modification time through the adapter; tests trigger the focus check) → A is not edited, so
it reloads by itself, stays on page 2, the page indicator reads "2 / 4", and a notice says it
was reloaded because the file changed on disk → rotate page 1 (edited) → the harness changes
the file again → the file-changed dialog (W-22) appears with Reload and Keep my version, Keep
the default and the Esc action → Keep my version → the rotation stays and nothing reloads →
Save → the saved file is the bytes the session had opened plus one update section (save and
verify); the other app's version is replaced, as the dialog said → the harness changes the
file once more and the test triggers a check → Reload → the edits are discarded, the new
version shows, and the document is not edited.
Error branch: the harness deletes A on disk → a notice says the file was moved or deleted;
the document stays open from the session's bytes and is marked edited; Save opens the Save As
panel → macOS: the harness renames A in the same folder → the window title follows the new
name (file presenter move event) and Save writes to the new path; web has no path and keeps
the handle. The engine never mixes bytes from two versions: any later read of the file finds
the session's SHA-256 changed and reads from the session's own copy or fails with a typed
error, never rendering a mix.

## J-PC-03: Recover unsaved work after a crash
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_crash_recovery, e2e.macos.pc_crash_recovery

Steps: open fixture A and rotate page 2, add a blank page at the end, and wait for the
recovery checkpoint (written 2 s after the last committed change; the test waits for the
app's "checkpoint written" test event) → the recovery data is in app storage only (macOS: the
app container; web: OPFS) and holds the source file's identity and the edits as an
incremental update, not a copy of the whole file → kill the app (macOS `kill -9`; web: crash
the renderer with the DevTools protocol `Page.crash`) → launch again → the "Restore unsaved
changes?" dialog lists A with the time of its last checkpoint and offers Restore and Discard
(Restore is the default) → Restore → A opens with page 2 rotated and 4 pages, marked edited,
and a notice says the edits were recovered → Save → save and verify; A on disk now has the
edits → the recovery data for A is deleted → kill and relaunch again → no restore dialog
appears.
Second run: edit A, kill, relaunch → Discard → A's file bytes are unchanged and its recovery
data is deleted → edit A, close it normally with Don't Save (J-PC-01) → its recovery data is
deleted at once.
Encrypted documents: open an AES-256 R6 fixture with password `open`, edit, wait for the
checkpoint → the recovery data is encrypted with the document's own key (the update section
is encrypted as a save would write it); searching app storage for page text sentinels and for
`open` finds nothing → kill, relaunch, Restore → the password prompt appears before anything
is shown.
Changed source: edit A, kill, change A on disk, relaunch, Restore → the recovered edits are
shown against the session's original bytes, kept in the recovery data for this case only
(the dialog said so) and the file-changed dialog of J-PC-02 follows.
Error branch: a recovery record truncated by the harness, one whose update section is
hostile (objects nested deeper than `Limits`), and one pointing at a deleted source file →
each shows a notice naming the document and is offered only for Discard; nothing crashes.
Redaction (forms-security.md redaction check, residue): recovery data for a document holds
no content removed by a saved redaction; tested in J-SC-07's residue step.

## J-PC-04: Choose print content each time, or reuse the last choice
- milestone: M4
- platform: web, macos
- test: e2e.web.pc_print_preference, e2e.macos.pc_print_preference

Steps: open the J-RD-21 fixture → Preferences > Printing shows "Choose print content each
time" on (the default) → Print → the print options (J-RD-21) appear → choose "document only",
scale 50 %, 2 pages per sheet → print (the engine's print job is inspected, J-RD-21's test
mechanism) → turn the preference off → Print → our print options are skipped and the platform
print panel opens directly (macOS: the system print panel; web: the job is handed to
`window.print()`) with the engine job using the last content, scale and pages per sheet, and
page range All (the range is never reused) → the job has no annotations and 2 pages per sheet
→ restart the app → the preference is still off and the last choices are reused → turn it on
→ Print → the options appear again, prefilled with the last choices.
Error branch: open a fixture whose permissions forbid printing (bit 3 clear) → Print is inert
with the engine's reason (J-RD-29) whether the preference is on or off, and the binding call
returns a typed permission error.

## J-PC-05: Several windows
- milestone: M4
- platform: web, macos
- test: e2e.web.pc_windows, e2e.macos.pc_windows

Steps (macOS): open fixtures A, B and C as tabs (J-RD-23) → Window > Move Tab to New Window on
B → B is in its own window, A and C stay as tabs → each window keeps its own sidebar, mode and
inspector state → the Window menu lists all three documents, and choosing C brings its window
to the front → open B again from File > Open → B's window comes to the front; no duplicate →
Window > Merge All Windows → the three are tabs of one window → close every window → the app
keeps running with no window (macOS convention) → click the Dock icon (test: the reopen Apple
Event) → the start screen window opens.
Steps (web): open A in the app → open the app in a second browser tab and open the same file
handle there → the second tab says the file is open in another jpdf tab and opens it read-only
(the first tab holds the file's Web Lock; handles are compared with `isSameEntry`) → editing
commands in the second tab are inert with that reason → close A in the first tab → the second
tab offers "Make editable" → choose it → editing works there.

## J-PC-06: Keyboard shortcuts list
- milestone: M4
- platform: web, macos
- test: e2e.web.pc_shortcuts_list, e2e.macos.pc_shortcuts_list

Steps: Help > Keyboard Shortcuts (web: also `?` with the document view focused) → a window
(macOS) / dialog (web) lists the commands in the groups of [shortcuts.md](../ui/shortcuts.md)
with the current platform's keys → the list is built from the app's command registry, and a
contract test checks that the registry's commands, keys and groups equal the shortcuts.md
tables for that platform (no command missing or extra) → type `rot` in the filter field → only
the rotate commands remain → clear it → turn off single-key shortcuts in Preferences →
reopen the list → single-key entries are shown as off with the reason, and `?` no longer opens
the list (web: Help menu only) → Esc closes it (shortcuts.md rule 9).
Check: the list passes the accessibility audits (axe-core; XCUITest audit) and every row is
read by VoiceOver as command then key.

## J-PC-07: Save As conventions and Revert
- milestone: M6
- platform: web, macos
- test: e2e.web.pc_save_as, e2e.macos.pc_save_as

Steps: open fixture `report.pdf` → edit it → Save As (macOS Cmd+Shift+S; web the File menu)
→ the save panel suggests `report.pdf` with only `report` selected (macOS; web:
`showSaveFilePicker` with `suggestedName` `report.pdf` and the PDF type through the adapter)
→ type `final` → the file is saved as `final.pdf` (the extension is added) → the title and
recent files show `final.pdf` → Save As to the name of an existing file → macOS: the system
asks to replace it (test hook answers Replace); web: the picker's overwrite choice is the
adapter's → the file is replaced atomically (J-OS-06: temporary file, then replace) → Save As
onto the open document's own file → it is a full rewrite in place (the J-OS-02 rewrite rule)
→ Save As with the no-handles test flag (web) → the file downloads as `final.pdf`, the
document stays marked edited and a notice says the download is a copy that further Saves will
not update → edit again → File > Revert to Saved → a confirmation, Cancel the default →
confirm → the document reloads from disk, not edited, the edits gone.
Error branch: Save As to a file that is open in another tab → a typed error asks to close it
first; Save As to a name the platform rejects (macOS: a name with `:`; web: the adapter
returns an invalid-name error) → an error names the cause and nothing is written.
