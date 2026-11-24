# jpdf: what it is for

Status: v1, decided by the owner's approval of its PR (ADR 0011 §7). Nothing below is built
yet: the project is in milestone M0 (Foundation), and the apps don't open PDFs
([README.md](../README.md)). Every feature here is a target, with the milestone that delivers it
(PLAN.md §8).

This file says **what** jpdf is for. [PLAN.md](../PLAN.md) holds the requirements, architecture
and milestones; the user journeys in [specs/journeys/](../specs/journeys/) say what users do,
step by step, and are the scope the milestone gates check; [specs/TRACE.md](../specs/TRACE.md)
maps each feature to its journeys, specs and tests. Where they disagree, PLAN.md and the
journeys win, and this file is fixed.

## 1. Who it is for, and the jobs they do

jpdf is a PDF reader, annotator and editor for people who work with PDF files on a Mac or in a
web browser and want the files to stay on their own device (PLAN R3, R11). It aims at feature
parity with PDFgear (PLAN R7), with PDFgear's ideas and each platform's own look (PLAN R8): the
Apple HIG on macOS, Material on the web.

Their jobs, in the order the milestones deliver them:

| Job | What the user does | Milestone |
|---|---|---|
| **Read** | open any PDF, including damaged and password-protected ones, navigate, search, copy text, print | M4 |
| **Review** | highlight, comment, draw, stamp; reply to and resolve comments; exchange comments with other tools | M5 |
| **Assemble** | insert, delete, extract, reorder and rotate pages; merge and split files; save in place or as a new file | M6 |
| **Protect** | set passwords and permissions | M6 (certificate encryption: M9) |
| **Convert** | images to PDF, pages to images | M6 |
| **Fill and build forms** | fill every AcroForm field type, create and edit fields, flatten | M7 |
| **Change content** | edit text and images, add text, images and links, watermark, headers and footers, Bates numbers, crop, compress | M8 |
| **Redact** | remove content for real, not just cover it, and remove hidden information | M8 |
| **Sign and verify** | verify signatures and what changed after signing; sign and certify (PAdES), with timestamps and long-term validation | M9 |

M10 (Hardening & 1.0) releases v1.0. Phase 2 (§2, last table) comes after it.

## 2. Scope

PLAN §4's Phase 1 features, grouped by job. The journeys are the ones
[specs/TRACE.md](../specs/TRACE.md) lists for each feature group; a milestone gate requires every
journey assigned to it to pass (PLAN §8).

**Read** (M4)
| Feature | Journeys |
|---|---|
| Open, including passwords, a fast first page, progressive loading of web-optimized files; several documents; restore the last session (F-RD-1) | J-RD-01 … J-RD-05, J-RD-23, J-RD-24, J-RD-26, J-RD-27, J-RD-29, J-RD-30; with saving (M6): J-PC-02 (the file changed on disk), J-PC-05 (several windows) |
| Views: single, continuous, two-page, cover; fit, zoom, rotate the view, magnifier (F-RD-2) | J-RD-06 … J-RD-09, J-RD-25 |
| Sidebar: thumbnails, outline, annotations, attachments (listed, never opened), layers (F-RD-3) | J-RD-10 … J-RD-14 |
| Search, select and copy text, internal links, external links after a confirmation; scripts never run (F-RD-4) | J-RD-15 … J-RD-18, J-RD-28, J-RD-31 |
| Read and presentation modes, dark or sepia page tint, print, document properties (F-RD-5) | J-RD-19 … J-RD-22, J-PC-04 |
| The keyboard shortcuts list (PLAN R8; TRACE lists it under R8, not a feature group) | J-PC-06 |

**Review** (M5)
| Feature | Journeys |
|---|---|
| Text markup: highlight, underline, strikeout, squiggly (F-AN-1) | J-ANN-01 … J-ANN-03 |
| Sticky note, text box, callout (F-AN-2) | J-ANN-04 … J-ANN-06 |
| Shapes, freehand ink, eraser (F-AN-3) | J-ANN-07 … J-ANN-12 |
| Stamps, visual signatures (drawn, typed, image) (F-AN-4) | J-ANN-13 … J-ANN-15 |
| Properties, move and copy, replies and status, the annotation list, undo, flatten, XFDF/FDF import and export, restricted files, other tools' annotations, a comment summary (F-AN-5) | J-ANN-16 … J-ANN-25; with the full writer (M6): J-ANN-26 (flatten and save without history) |

**Assemble** (M6)
| Feature | Journeys |
|---|---|
| Insert (blank, from a file, from images), delete, extract, reorder, rotate, duplicate pages (F-PG-1) | J-PG-01 … J-PG-08, J-PG-13, J-PG-15, J-PG-16 |
| Merge, split (by range, size or bookmarks), page labels, bookmark editing (F-PG-2) | J-PG-09 … J-PG-12, J-PG-14 |
| Save: incremental (keeps signatures valid), full rewrite, Save As, unsaved changes, crash recovery (F-OS-1; its compress half is under Change content, M8) | J-OS-01, J-OS-02, J-OS-06, J-PC-01, J-PC-03, J-PC-07 |
| Convert images to a PDF, export pages as images (F-OS-2) | J-OS-04, J-OS-05 |

**Protect** (M6; certificate encryption M9)
| Feature | Journeys |
|---|---|
| Encrypt (AES-256, R6), decrypt, change passwords and permissions, certificate encryption (F-SC-1) | J-SC-01 … J-SC-05 |

**Fill and build forms** (M7)
| Feature | Journeys |
|---|---|
| Fill every AcroForm field type; formatting and calculations from a safe built-in set, written in Rust, with no script interpreter; create, edit and flatten fields; XFA shown as page content only (F-FM-1) | J-FM-01 … J-FM-12 |

**Change content and redact** (M8)
| Feature | Journeys |
|---|---|
| Edit existing text (the same font if possible, else a substitute with a warning) (F-ED-1) | J-ED-01 |
| Add text, images and links; move, resize and delete images (F-ED-2) | J-ED-02 … J-ED-05 |
| Watermark, header and footer, Bates numbers, page background (F-ED-3) | J-ED-06 … J-ED-09 |
| Crop; true redaction and removing hidden information (F-ED-4) | J-ED-10, J-SC-06 … J-SC-10 |
| Compress: object streams, image downsampling, font subsetting, removing unused objects (F-OS-1) | J-OS-03 |

**Sign and verify** (M9)
| Feature | Journeys |
|---|---|
| Verify: integrity, certificate chain, trust, timestamp, certification and field locks, changes after signing (F-SC-2) | J-SG-01 … J-SG-04, J-SG-09, J-SG-11 |
| Sign: PAdES B-B to B-LTA, certification, visible appearances, digital IDs (F-SC-3) | J-SG-05 … J-SG-09, J-SG-11 |
| Timestamps and OCSP/CRL: off until the user turns them on (F-SC-4) | J-SG-09, J-SG-10 |

**Phase 2** (after v1.0; scoped at its start, PLAN §8 row P2, so no journeys yet)
| Feature | PLAN §4 |
|---|---|
| OCR (F-P2-1), PDF → Office then Office → PDF (F-P2-2), AI chat and summary with no backend (F-P2-3), batch, compare and PDF/A (F-P2-4), iOS then Android (F-P2-5), read aloud with on-device voices only (F-P2-6) | Phase 2 |

## 3. Non-goals

From PLAN §1's explicit non-goals:
- running PDF JavaScript or Launch actions, or opening embedded executables (CONSTITUTION 7);
- XFA forms (the AcroForm fallback is shown if there is one, J-FM-12);
- 3D and RichMedia playback (the poster image only);
- any server-side processing (PLAN R11).

From the PDFgear inventory ([specs/journeys/RESEARCH.md](../specs/journeys/RESEARCH.md), status
"out"):
- online services: e-sign requests, online compression or conversion (files never leave the
  device, PLAN R11);
- OS and device integration beyond a reader: watched folders, a menu-bar helper, start at
  login, scanning (an audit may revisit these);
- cropping an image inside the page (images are replaced instead; an audit may revisit it);
- a prompt about placed but unapplied signatures (Sign writes the file at once, J-SG-06).

## 4. Promises users can rely on

Each promise points to what enforces it; this file doesn't restate the rules.

| Promise | Enforced by |
|---|---|
| **A hostile file can't take over the app.** Every PDF and every value from the native layer is treated as hostile and checked under hard resource limits; no `unsafe` code in the engine, and none in its dependencies except a short audited list of cryptographic primitives; the native layers hold no PDF logic | CONSTITUTION 3–6 and 8; PLAN R1, R4, §2.1, §2.2; the judge's crash grading and fuzzing (PLAN §6) |
| **Scripts and automatic actions never run.** | CONSTITUTION 7; J-RD-28, J-FM-05 … J-FM-07 |
| **Security problems block the owner preview and the release.** No open `sev:critical` or `sev:high` issue at the M4 owner preview, no open `sev:medium` or worse at the v1.0 release (M10) | ADR 0009; PLAN §8 rows M4 and M10 |
| **Files stay on the device.** The web app is static with no backend; timestamps and OCSP/CRL requests are off until the user turns them on | PLAN R11, §4 (Security & signatures); the web network rule in [specs/journeys/reader.md](../specs/journeys/reader.md) |
| **Pages look right.** Rendering is compared against other engines on public corpora; damaged real-world files open the way consensus engines open them | PLAN R5, R6, §6; CONSTITUTION 17; PLAN §8 exit criteria for M1–M3 |
| **What jpdf writes is valid PDF.** Strict writing; outputs pass `qpdf --check` and re-render the same; annotations render in other viewers | CONSTITUTION 17; PLAN §8 rows M5 and M6 |
| **Redaction removes the content.** It is not just covered | PLAN §4 (Edit); PLAN §8 row M8; J-SC-07, J-SC-08 |
| **Saving never breaks a signature silently.** Saves append to the signed revision, which stays a byte prefix, so its signature stays intact; a page change that verifiers will report as a change after signing, and a Save As that drops earlier revisions, warn first | PLAN §4 (Optimize & save); J-PG-15, J-OS-02 |
| **Accessible.** WCAG 2.2 AA on the web; VoiceOver, Full Keyboard Access and the other Apple HIG accessibility settings on macOS | [specs/ui/accessibility.md](../specs/ui/accessibility.md); PLAN §8 row M10 |
| **Fast, without trading away safety.** A first page in under 200 ms for a typical 10 MB file, 60 fps scrolling | PLAN R2, §5; PLAN §8 rows M4 and M10 |
| **The web app works offline.** It is to install as a PWA and work with no network | PLAN §8 row M10 (no journey yet: #331) |
| **Open source.** MIT, with only permissively licensed dependencies | PLAN R10; CONSTITUTION 10 |

## 5. Platforms

| Platform | When | Notes |
|---|---|---|
| macOS 14 and later | v1.0 | SwiftUI app; the engine runs in a sandboxed XPC helper; App Sandbox and Hardened Runtime, so it can ship through the Mac App Store or as a notarized download (PLAN §1 assumptions, §2) |
| Web: current Chrome, Safari and Firefox | v1.0 | React + MUI; the engine runs as WebAssembly in a Web Worker; static hosting only (PLAN R11, §2) |
| iOS, then Android | Phase 2 | PLAN R3, §4 Phase 2 |

## 6. How this file changes

A Design: product session (ops/TICK.md §10) changes it, from an audit or user-advocate finding
or an owner comment. It invents no scope: a feature that PLAN.md and the journeys don't have is
a question for the owner, and a missing journey is a `design-product` issue. This v1 merged on
the owner's explicit approval; a later change merges after a 24-hour window unless the owner
`/hold`s it (ADR 0011 §7, ops/TICK.md §10 step 6).
