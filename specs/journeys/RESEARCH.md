# PDFgear feature inventory → journeys

How this was made: the feature list of the installed `/Applications/PDFgear.app` (UI string
tables in `Contents/Resources/en.lproj/*.strings`: main menu, preferences, sidebar panels,
search filter, print options, properties panel) plus PLAN §4. Items are described in our own
words; no PDFgear text, icons or layouts are copied (CONSTITUTION 20).

Status: **J-…** = covered by that journey · **#N** = covered by a later journeys issue ·
**Phase 2** = PLAN §4 Phase 2 · **out** = not planned (reason given).

Each journeys issue (#6–#9, #42) adds its own section here.

## Reader and navigation (#6)

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Open | Open file dialog | J-RD-01 |
| Open | Drag and drop files to open | J-RD-02 |
| Open | Recent files on the start screen (name, size, opened time; clear list) and Open Recent menu | J-RD-03 |
| Open | Password prompt for encrypted files; wrong-password message | J-RD-04, J-RD-30 |
| Open | Error when a file can't be opened | J-RD-05 |
| Open | Reopen the files from the last session at launch (preference) | J-RD-24 |
| Open | Offer to restore an unsaved file after a crash | J-PC-03 |
| Open | Alert when the open file changed on disk | J-PC-02 |
| Open | Open from a folder; notify when new PDFs arrive; menu-bar helper; start at login | out (OS integration beyond a reader; revisit in an audit) |
| Documents | Several documents in tabs; next/previous tab | J-RD-23 |
| Navigation | Page number field, go to page | J-RD-06 |
| Navigation | Back to previous view / forward to next view | J-RD-06, J-RD-17 |
| Views | Single page, single page scrolling, two pages, two pages scrolling | J-RD-07 |
| Views | Cover page with two-page spreads | J-RD-07 (PLAN §4; PDFgear has no explicit item) |
| Views | Default page layout preference | J-RD-07 |
| Zoom | Zoom in/out, actual size, fit page, fit width, fit height | J-RD-08 |
| Zoom | Default zoom preference | J-RD-08 |
| Zoom | Magnifier (loupe) | J-RD-25 |
| Rotate | Rotate view left/right | J-RD-09 |
| Sidebar | Show/hide sidebar; thumbnails panel | J-RD-10 |
| Sidebar | Bookmarks panel: navigate, expand/collapse all | J-RD-11 |
| Sidebar | Bookmarks panel: add, add child, rename, delete | J-PG-14 |
| Sidebar | Annotation list: group by page, sort by page/date/type, jump to annotation | J-RD-12 |
| Sidebar | Annotation list: select, delete | J-ANN-19 |
| Sidebar | Attachments | J-RD-13 (list only; never opened, CONSTITUTION 7) |
| Sidebar | Layers (optional content) | J-RD-14 (PLAN §4; not seen in PDFgear) |
| Search | Find with next/previous result | J-RD-15, J-RD-31 |
| Search | Case-sensitive and whole-words options | J-RD-15 |
| Text | Select text, copy, select all | J-RD-16 |
| Links | Internal links | J-RD-17 |
| Links | External links open in the browser (we add a confirmation) | J-RD-18 |
| Reading | Full screen | J-RD-19 |
| Reading | Slide (presentation) mode with end-of-document hint and Esc to exit | J-RD-19 |
| Reading | Warm and dark reading tints for pages | J-RD-20 |
| Reading | App appearance light/dark/auto | J-RD-20 |
| Print | Print document only or with annotations | J-RD-21 |
| Print | Preference to choose the content before every print | J-PC-04 |
| Print | Scale, scale to fit, pages per sheet, auto-rotate | J-RD-21 |
| Print | Print note contents on extra pages | J-ANN-25 |
| Print | Permission-restricted printing and copying, unlocked with the owner password | J-RD-29 |
| Properties | File information (location, size, pages, page size, version, producer, dates) | J-RD-22 |
| Properties | Description (title, author, subject, keywords) | J-RD-22 |
| Properties | Security summary | J-RD-22 |
| Performance | Large and web-optimized files load progressively | J-RD-26 (PLAN §4) |
| Open | Initial view from the file (open destination, page mode, viewer preferences) | J-RD-27 (PLAN §4 safety; not a PDFgear item) |
| Security | Scripts and automatic actions in files are never run | J-RD-28 (CONSTITUTION 7) |
| Reading | Read aloud (text to speech: current page, all pages, from here; speed; voice) | Phase 2 (F-P2-6; owner decision on #31) |
| Reading | Full-text translation | Phase 2 (F-P2-3, AI features) |
| Reading | AI assistant / chat about the document | Phase 2 (F-P2-3) |
| Reading | Screenshot OCR, OCR to searchable PDF | Phase 2 (F-P2-1) |
| Create | New blank PDF, new PDF from files/images/copied pages, scan to PDF | J-PG-01, J-PG-09, J-OS-04, J-PG-13; scan: out (see #8 section) |

## Annotate and comment (#7)

Sources: the annotation toolbar, annotation list, stamp and signature dialogs, the Annotate
ribbon and the edit menu in the same string tables, plus PLAN §4 (Annotate).

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Markup | Highlight selected text | J-ANN-01, J-ANN-03 |
| Markup | Underline, strikethrough | J-ANN-02 |
| Markup | Squiggly underline | J-ANN-02 (PLAN §4; not seen in PDFgear) |
| Notes | Sticky note: add, edit | J-ANN-04 |
| Notes | Text box | J-ANN-05 |
| Notes | Callout | J-ANN-06 |
| Shapes | Rectangle, ellipse | J-ANN-07 |
| Shapes | Line, arrow | J-ANN-08 |
| Shapes | Polygon, polyline, cloud | J-ANN-09, J-ANN-10 (PLAN §4; PDFgear's shape menu is smaller) |
| Drawing | Freehand ink (pen) | J-ANN-11 |
| Drawing | Eraser | J-ANN-12 |
| Stamps | Built-in stamps; stamp with date, or date and time | J-ANN-13 (our set: the ISO 32000-2 standard stamp names) |
| Stamps | Custom text stamp with color; custom image stamp with background removal | J-ANN-14 |
| Signatures | Create a signature by drawing (trackpad), typing (keyboard) or from an image, with background removal; a small saved-signature limit | J-ANN-15 |
| Signatures | Digital signatures, certify, signature panel, deferred signatures | J-SG-01, J-SG-06, J-SG-07, J-SG-08 (deferred signatures are signature fields: J-FM-08, signed in J-SG-07) |
| Signatures | Online signature requests (e-sign service) | out (needs a backend; PLAN R11) |
| Properties | Stroke width presets, font size presets, color, opacity | J-ANN-16 |
| Editing | Copy, cut, paste, remove annotation | J-ANN-17 |
| Editing | Magnetism / auto-align when dragging annotations (preference) | J-ANN-17 |
| Comments | Add or edit a remark on an annotation | J-ANN-04, J-ANN-18 |
| Comments | Replies and review status | J-ANN-18 (PLAN §4) |
| List | Sort by page, date, type; expand/collapse; select all; delete | J-RD-12, J-ANN-19 |
| History | Undo and redo | J-ANN-20 |
| Flatten | Flatten annotations; save without earlier versions | J-ANN-21, J-ANN-26 (PLAN §4) |
| Exchange | Export and import comments (XFDF, FDF) | J-ANN-22 (PLAN §4) |
| Security | Password prompt to unlock annotating a restricted file | J-ANN-23 |
| Robustness | Other tools' annotations kept as they are | J-ANN-24 (PLAN §4 safety; not a PDFgear item) |
| Print | Print with annotations; print comment contents | J-RD-21, J-ANN-25 |
| Links | Insert a link on an area or on text | J-ED-04 |
| Forms | Highlight form fields | J-FM-04 |

## Pages, edit, optimize and convert (#8)

Sources: the page ribbon, page organizer, insert, merge, split, compress, image-to-PDF, crop,
edit toolbar, link tool, bookmark panel, watermark, header and footer, page number and convert
string tables in the same bundle, plus PLAN §4 (Pages, Edit, Optimize & save).

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Pages | Insert a new blank page | J-PG-01 |
| Pages | Append or insert pages from another file, at the beginning, end, before or after the selected page, with a page range | J-PG-02 |
| Pages | Insert a local image as a page | J-PG-03 |
| Pages | Delete pages (a document keeps at least one page) | J-PG-04 |
| Pages | Extract pages | J-PG-05 |
| Pages | Reorder pages by dragging thumbnails; undo for page moves | J-PG-06, J-PG-16 |
| Pages | Rotate pages left/right | J-PG-07 |
| Pages | Duplicate pages | J-PG-08 (PLAN §4; not seen in PDFgear) |
| Pages | Merge files (add files, per-file range, order, output name) | J-PG-09 |
| Pages | Split by custom page ranges or every N pages | J-PG-10 |
| Pages | Split by size or by bookmarks | J-PG-11 (PLAN §4; PDFgear splits by range only) |
| Pages | Page labels | J-PG-12 (PLAN §4; not seen in PDFgear) |
| Create | New blank PDF | J-PG-01 |
| Create | New PDF from copied pages | J-PG-13 |
| Create | New PDF from files | J-PG-09 |
| Create | New PDF from images; image to PDF with page size (match the image, A3, A4, A5, Letter, Legal), orientation, margins, one file or one per image | J-OS-04 |
| Create | Scan a document to PDF (scanner, camera, phone app) | out (device integration beyond a reader; revisit in an audit) |
| Sidebar | Bookmarks panel: add, add child, rename, delete | J-PG-14 |
| Security | Page changes on restricted and signed files | J-PG-15 (PLAN §4 safety; not a PDFgear item) |
| Edit | Edit existing text | J-ED-01 |
| Edit | Add text with font, size and style presets | J-ED-02 |
| Edit | Add an image; move, resize, rotate, flip, replace, arrange (layer order), delete | J-ED-03 |
| Edit | Crop an image inside the page | out for M8 (images are cropped by replacing them; revisit in an audit) |
| Links | Insert a link on an area or on text, to a page or a web address | J-ED-04 |
| Links | Edit and delete links | J-ED-05 |
| Edit | Watermark: text or image, font, size, color, opacity, rotation, above or below the content, page range; remove | J-ED-06 |
| Edit | Header and footer: six positions, file name, date and page number fields, margins, page subset (all, odd, even, custom range); remove | J-ED-07 |
| Edit | Page numbers: style, position, alignment, start number, range | J-ED-07 |
| Edit | Bates numbering | J-ED-08 (PLAN §4; not seen in PDFgear) |
| Edit | Page background | J-ED-09 (PLAN §4; not seen in PDFgear) |
| Edit | Crop pages, with numeric width and height and "apply to all pages" | J-ED-10 |
| Save | Save (incremental), Save As | J-OS-01, J-OS-02, J-PC-07 |
| Optimize | Compress with low, medium and high levels and a size report | J-OS-03 (our levels: Lossless, Balanced, Smallest) |
| Optimize | Compress several files in a list | Phase 2 (F-P2-4, batch) |
| Optimize | Online compression or conversion | out (files never leave the device, PLAN R11) |
| Convert | PDF to PNG, PDF to JPEG | J-OS-05 |
| Convert | PDF to Word, Excel, PowerPoint, RTF, HTML, XML, text | Phase 2 (F-P2-2) |
| Convert | Word, Excel, PowerPoint, RTF, text to PDF | Phase 2 (F-P2-2) |
| Convert | Merge, split and convert several files in a list | Phase 2 (F-P2-4, batch) |
| Robustness | A failed save leaves the original file intact | J-OS-06 (platform convention; not a PDFgear item) |

## Forms and security (#9)

Sources: the form ribbon, form field properties panel, security ribbon, password dialog,
security summary and the redaction confirmation in the same string tables, plus PLAN §4
(Forms; Security & signatures) and the F-ED-4 redaction and metadata-scrub items.

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Forms | Fill text, check box, radio, dropdown and list fields | J-FM-01, J-FM-02, J-FM-03 |
| Forms | Highlight form fields; preference to highlight them in newly opened files | J-FM-04 |
| Forms | Number, date and other formats; built-in calculations (no scripts run) | J-FM-05, J-FM-06 (PLAN §4) |
| Forms | Reset buttons; submit and script buttons stay inert | J-FM-07 (PLAN §4 safety; not a PDFgear item) |
| Forms | Create text field, check box, radio button, dropdown, list box, button and digital signature field | J-FM-08 |
| Forms | Field properties: name, tooltip, required, read only, visibility (visible, hidden, visible but not printed, hidden but printed), font and size, alignment, border and fill color, default value, maximum length, comb, multiline, items with export values | J-FM-08 |
| Forms | Radio buttons share one name to form a group (hint in the properties panel) | J-FM-08 |
| Forms | Move, resize, align, rename, copy, delete fields; tab order | J-FM-09 (align, distribute and tab order: PLAN §4; not seen in PDFgear) |
| Forms | Flatten form fields | J-FM-10 (PLAN §4) |
| Forms | Fill forms when permissions allow filling only | J-FM-11 |
| Forms | XFA forms | J-FM-12 (shown as page content only; XFA is a PLAN §1 non-goal) |
| Security | Set an open password (password and confirm, mismatch message) | J-SC-01 |
| Security | Permissions: printing, changing, copying, commenting, form filling, page assembly; security summary in properties | J-SC-02, J-RD-22 |
| Security | Change password | J-SC-03 |
| Security | Remove password | J-SC-04 |
| Security | Unlock printing, annotating or changing with the owner password | J-RD-29, J-ANN-23, J-FM-11, J-SC-02 |
| Security | Encrypt with certificates | J-SC-05 |
| Redaction | Mark text and areas, then redact with a permanent-removal confirmation | J-SC-06, J-SC-07, J-SC-08 |
| Redaction | Search and mark every match | J-SC-06 (PLAN §4; not seen in PDFgear) |
| Redaction | Remove hidden information (metadata, scripts, attachments, earlier versions) | J-SC-09 (PLAN §4 F-ED-4; not a PDFgear item) |
| Redaction | Redact and remove hidden information in protected files | J-SC-10 (PLAN §4 safety) |
| Signatures | Digital IDs (import, create self-signed), certificate manager, trust a certificate, deferred signatures | J-SG-04, J-SG-05, J-FM-08, J-SG-07 |

## Signatures and platform conventions (#42)

Sources: the digital ID, certificate, signature panel, signature validity and certify strings
in the same string tables, the unsaved-file restore prompt, plus PLAN §4 (Security &
signatures; Optimize & save), PLAN R12 and macOS / web platform conventions (Apple HIG, the
document architecture, browser file handles).

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Signatures | Signature panel with a validity summary per signature and for the document | J-SG-01 |
| Signatures | Valid, invalid and unknown validity; "changed since signing" message | J-SG-01, J-SG-02, J-SG-03 |
| Signatures | Validate as of the signing time | J-SG-01, J-SG-09 (timestamp time when one validates) |
| Signatures | Show certificate; trust a certificate; unknown-signer explanation | J-SG-04 |
| Signatures | Certificate manager (trusted certificates) | J-SG-04 |
| Signatures | Digital IDs: import a P12 file with its password, create a self-signed ID | J-SG-05 |
| Signatures | Sign by drawing a rectangle; reason, location, contact; appearance | J-SG-06 |
| Signatures | Sign an empty signature field ("deferred" signature) | J-SG-07 |
| Signatures | Certify with a choice of allowed changes | J-SG-08 |
| Signatures | Prompt about placed signatures that were not applied | out (our signatures are applied when placed: Sign writes the file at once, J-SG-06) |
| Signatures | Request signatures from others online | out (files never leave the device, PLAN R11) |
| Signatures | View the signed version of a revision | J-SG-03 (PLAN §4 "changes after signing"; not seen in PDFgear) |
| Signatures | FieldMDP locks and seed values | J-SG-03, J-SG-07 (PLAN §4; not seen in PDFgear) |
| Signatures | Timestamps, OCSP/CRL, long-term validation (B-T, B-LT, B-LTA) | J-SG-09, J-SG-10 (PLAN §4; not seen in PDFgear) |
| Signatures | Signing and verifying encrypted files | J-SG-11 (PLAN §4; not a PDFgear item) |
| Platform | Unsaved changes on close and quit | J-PC-01 (platform convention) |
| Platform | File changed on disk | J-PC-02 (platform convention) |
| Platform | Restore an unsaved file after a crash | J-PC-03 |
| Platform | Preference to choose the print content each time | J-PC-04 |
| Platform | Several windows; move a tab to a new window | J-PC-05 (platform convention) |
| Platform | Keyboard shortcuts list | J-PC-06 |
| Platform | Save As naming, replace an existing file, Revert to Saved | J-PC-07 (platform convention) |
