# PDFgear feature inventory → journeys

How this was made: the feature list of the installed `/Applications/PDFgear.app` (UI string
tables in `Contents/Resources/en.lproj/*.strings`: main menu, preferences, sidebar panels,
search filter, print options, properties panel) plus PLAN §4. Items are described in our own
words; no PDFgear text, icons or layouts are copied (CONSTITUTION 20).

Status: **J-…** = covered by that journey · **#N** = covered by a later journeys issue ·
**Phase 2** = PLAN §4 Phase 2 · **out** = not planned (reason given).

Each journeys issue (#6–#9) adds its own section here.

## Reader and navigation (#6)

| Area | PDFgear feature (our words) | Status |
|---|---|---|
| Open | Open file dialog | J-RD-01 |
| Open | Drag and drop files to open | J-RD-02 |
| Open | Recent files on the start screen (name, size, opened time; clear list) and Open Recent menu | J-RD-03 |
| Open | Password prompt for encrypted files; wrong-password message | J-RD-04, J-RD-30 |
| Open | Error when a file can't be opened | J-RD-05 |
| Open | Reopen the files from the last session at launch (preference) | J-RD-24 |
| Open | Offer to restore an unsaved file after a crash | #9 (platform conventions) |
| Open | Alert when the open file changed on disk | #9 (platform conventions) |
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
| Sidebar | Bookmarks panel: add, add child, rename, delete | #8 (editing) |
| Sidebar | Annotation list: group by page, sort by page/date/type, jump to annotation | J-RD-12 |
| Sidebar | Annotation list: select, delete | #7 (annotate) |
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
| Print | Preference to choose the content before every print | #9 (platform conventions: preferences) |
| Print | Scale, scale to fit, pages per sheet, auto-rotate | J-RD-21 |
| Print | Print note contents on extra pages | #7 (annotate: comment summary) |
| Print | Permission-restricted printing and copying, unlocked with the owner password | J-RD-29 |
| Properties | File information (location, size, pages, page size, version, producer, dates) | J-RD-22 |
| Properties | Description (title, author, subject, keywords) | J-RD-22 |
| Properties | Security summary | J-RD-22 |
| Performance | Large and web-optimized files load progressively | J-RD-26 (PLAN §4) |
| Open | Initial view from the file (open destination, page mode, viewer preferences) | J-RD-27 (PLAN §4 safety; not a PDFgear item) |
| Security | Scripts and automatic actions in files are never run | J-RD-28 (CONSTITUTION 7) |
| Reading | Read aloud (text to speech: current page, all pages, from here; speed; voice) | out of PLAN §4 → discovery #31 (proposed Phase 2) |
| Reading | Full-text translation | Phase 2 (F-P2-3, AI features) |
| Reading | AI assistant / chat about the document | Phase 2 (F-P2-3) |
| Reading | Screenshot OCR, OCR to searchable PDF | Phase 2 (F-P2-1) |
| Create | New blank PDF, new PDF from files/images/copied pages, scan to PDF | #8 (pages, convert) |
