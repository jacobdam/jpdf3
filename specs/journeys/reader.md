# Journeys: reader and navigation

Area `RD`. PLAN §4 (Reader), feature groups F-RD-1 … F-RD-5 in [specs/TRACE.md](../TRACE.md).
Inventory and coverage: [RESEARCH.md](RESEARCH.md). Format: ops/judge/CONTRACT.md §4.

Shared rules for every journey in this file:
- Reader journeys never modify the file: unless a step says otherwise, the file's bytes on
  disk are identical before and after, and the document is not marked as edited.
- "Fixture" means a PDF we generate in the test harness (never a corpus file).
- Web: the file is opened from the local disk and never leaves the device (PLAN R11); the
  test asserts that no network request carries document bytes.
- All rendering, text, search and hit-testing come from the Rust engine; the native layer
  only displays results (CONSTITUTION 6).

## J-RD-01: Open a PDF from the file dialog and see the first page fast
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_dialog, e2e.macos.rd_open_dialog

Steps: launch the app → choose Open (menu, toolbar button, Cmd/Ctrl+O) → pick a 20-page
fixture in the system file dialog (web: the browser file picker) → the first page is visible
within the first-page budget of PLAN §8 M4 → the window title shows the file name → the page
indicator reads "1 / 20" → the rendered first page matches the MuPDF and Poppler renders of
the same page within the render-ratchet tolerance.
Error branch: cancel the dialog → nothing opens, no error is shown.

## J-RD-02: Open a PDF by drag and drop
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_drop, e2e.macos.rd_open_drop

Steps: drag a fixture PDF onto the empty start screen → it opens on page 1 → drag a second
fixture onto the open document window → it opens as another document (tab or window, per
J-RD-23) without closing the first → drag a non-PDF file (a `.txt`) → a "not a PDF" error is
shown, nothing else changes → drag several PDFs at once → each opens as its own document.
macOS: dropping onto the Dock icon also opens the file.

## J-RD-03: Reopen a file from the recent files list
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_recent_files, e2e.macos.rd_recent_files

Steps: open fixtures A then B → close both → the start screen lists B then A with name,
size and last-opened time → click A → it opens → File > Open Recent shows the same list
(macOS: the system recent-documents menu) → remove A from the list → it disappears → clear
the list → it is empty after restarting the app → reopen B from the list after deleting it
on disk → a "file not found" message is shown and B is removed from the list.
Web: the list keeps local file handles only (no copies of the bytes); if the browser can't
keep handles, the list shows names only and asks the user to pick the file again.

## J-RD-04: Open a password-protected PDF
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_password, e2e.macos.rd_open_password

Steps: open an AES-256 (R6) fixture with a user password → a password prompt appears with
the file name and no page content visible → enter a wrong password → an "incorrect password"
message, the prompt stays → enter the right password → page 1 is shown → repeat with an
RC4-40, an RC4-128 and an AES-128 fixture → each opens with the right password → cancel the
prompt on another encrypted fixture → nothing opens, no page content or thumbnail is shown.
Owner-password-only fixture: opens without a prompt; Document Properties (J-RD-22) lists the
restrictions.
Security: the password is never written to disk, logs or the recent files list.

## J-RD-05: Open a damaged or hostile file
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_damaged, e2e.macos.rd_open_damaged

Steps: open a fixture with a broken xref table (offsets shifted) → it opens after repair,
like MuPDF and Poppler, with the same page count they report → open a truncated fixture →
the readable pages are shown, or a clear "could not open" error if none are → open a fixture
that exceeds an engine limit (deep nesting, huge object count) → a typed "file too complex"
error, no crash, no hang longer than the limit → open a random-bytes file renamed `.pdf` → a
"not a PDF" error → after each error the app stays responsive and opens a valid fixture.
macOS: the XPC helper may be restarted; the app window never crashes.

## J-RD-06: Navigate between pages
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_navigate_pages, e2e.macos.rd_navigate_pages

Steps: open a 20-page fixture with page labels i–iv, 1–16 → Next Page (arrow key, Page Down,
toolbar) → page 2 → Last Page (End / Cmd+Down) → page 20 → First Page (Home / Cmd+Up) →
page 1 → type "7" in the page field → the physical page 7 is shown; typing the label "3" →
the page labelled 3 (physical 7) → type "99" → an "invalid page" hint, the page doesn't
change → Go Back (Cmd+[ / Alt+Left) → previous view position → Go Forward → returns.
The page field shows both the label and "(7 of 20)" when labels differ from numbers.

## J-RD-07: Switch page layout (single, continuous, two-page, cover)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_view_modes, e2e.macos.rd_view_modes

Steps: open a 9-page fixture on page 5 → View > Single Page → only page 5 shows; scrolling
moves page by page → Single Page Continuous → pages scroll as one strip → Two Pages → spreads
are 1–2, 3–4, 5–6, …, so pages 5–6 show side by side → Two Pages Continuous → scrolling strip of spreads → turn on "Show cover page" → page 1
alone, then 2–3, 4–5 → in every mode the current page stays 5 → set a default layout in
Preferences → newly opened files use it → a file whose /PageLayout asks for TwoColumnLeft
opens in two-page continuous unless the user changed the default.
Right-to-left documents (/Direction R2L) show spreads right to left.

## J-RD-08: Zoom and fit
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_zoom_fit, e2e.macos.rd_zoom_fit

Steps: open a fixture with a Letter page and an A3 landscape page → Zoom In (Cmd/Ctrl +) →
zoom steps up and the indicator shows the percentage → Zoom Out → steps down → Actual Size →
100% (1 PDF point = 1 CSS px on web, 1 pt on macOS) → Fit Width → the page width fills the
view → Fit Page → the whole page is visible → Fit Height → page height fills the view →
type 250 in the zoom field → 250% → pinch (trackpad) or Ctrl+scroll → zooms around the
pointer → zoom is clamped to the engine limits (e.g. 10%–6400%), never past them → text stays
sharp at 400% (tiles re-rendered, not scaled bitmaps) → set Fit Width as the default zoom →
new files open at Fit Width.

## J-RD-09: Rotate the view
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_rotate_view, e2e.macos.rd_rotate_view

Steps: open a fixture → Rotate Right (Cmd+R) → all pages display rotated 90° clockwise →
Rotate Left twice → rotated 90° counter-clockwise → text selection and links still hit the
right glyphs and areas in the rotated view → close the file → no "save changes?" prompt →
the file bytes are unchanged → reopen → normal orientation.
(Rotating pages in the file itself is a Pages journey, not this one.)

## J-RD-10: Browse thumbnails in the sidebar
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_thumbnails, e2e.macos.rd_thumbnails

Steps: open a 500-page fixture → show the sidebar (View > Sidebar) → Thumbnails tab → the
thumbnails of the visible range appear first, the rest render lazily while scrolling → the
current page is highlighted → click thumbnail 250 → the view jumps to page 250 → scroll the
document → the highlight follows → change thumbnail size (small/large) → layout updates →
hide the sidebar → the document area widens. Memory stays within the M4 budget while
scrolling all 500 thumbnails.

## J-RD-11: Navigate with the outline (bookmarks)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_outline, e2e.macos.rd_outline

Steps: open a fixture with a three-level outline (some entries closed by default, one with a
/XYZ destination, one with a named destination, one with a GoTo action) → Outline tab shows
the top level, with entries open or closed as the file says → expand a closed entry →
children show → Expand All / Collapse All → click each kind of entry → the view goes to the
right page and position → the outline entry for the current section is highlighted while
scrolling → a fixture with no outline shows "No outline" → a fixture with an outline loop
(/Next pointing back) shows each entry once and does not hang.
(Adding, renaming and deleting bookmarks is in the editing journeys, #8.)

## J-RD-12: Review annotations in the annotation list
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_annotation_list, e2e.macos.rd_annotation_list

Steps: open a fixture with highlights, notes, ink and a FreeText annotation on pages 1, 3
and 8 → Annotations tab lists them grouped by page with type icon, author, date and comment
text → sort by date → by type → back to page → filter to "Notes" only → click an entry →
the view jumps to it and it is selected on the page → expand/collapse a page group →
a fixture with no annotations shows "No annotations". Popup and Link annotations are not
listed. Annotation text is shown as plain text (never interpreted as markup).

## J-RD-13: See the attachments list without opening anything
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_attachments_list, e2e.macos.rd_attachments_list

Steps: open a fixture with two embedded files (one `.exe`, one `.txt`) and one
FileAttachment annotation → Attachments tab lists all three with name, size, description
and page (for the annotation) → there is no "Open" action, double-click does nothing, and
nothing is written to disk or launched (CONSTITUTION 7) → a fixture with a Launch action and
a GoToE action on a link → clicking the link does nothing except a "this action is not
supported" notice → a fixture with no attachments has no Attachments tab or shows "None".

## J-RD-14: Show and hide layers (optional content)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_layers, e2e.macos.rd_layers

Steps: open a fixture with three OCGs (one OFF by default, two in a radio-button group,
one locked) → Layers tab lists them in the /Order tree with their default visibility →
the OFF layer's content is not drawn, matching MuPDF/Poppler → turn it on → its content
appears → turn on the other member of the radio group → the first one turns off → the
locked layer's toggle is disabled → print (J-RD-21) uses the layers' print state (/Usage
/Print) → close → the file is unchanged → reopen → default visibility again.

## J-RD-15: Search the document
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_search, e2e.macos.rd_search

Steps: open a 50-page fixture containing "Report", "report" and "reporting" → Find (Cmd/
Ctrl+F) → type "report" → matches are highlighted on the pages, the first one is selected
and scrolled into view, and a count shows "1 of N" → Next (Enter / Cmd+G) and Previous
(Shift+Enter / Shift+Cmd+G) cycle through and wrap → the results list shows each match with
page number and surrounding text → turn on Case Sensitive → "Report" no longer matches →
turn on Whole Words → "reporting" no longer matches → search a phrase that crosses a line
break → it is found → search for a word that is not there → "No results" → search a
non-Latin fixture (CJK, Arabic) → matches are found → Esc closes the find bar and removes
highlights. Search runs in the engine; a long search can be cancelled and the UI never
freezes.

## J-RD-16: Select and copy text
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_select_copy, e2e.macos.rd_select_copy

Steps: open a two-column fixture → choose the text select tool → drag across a paragraph in
column 1 → the selection follows reading order and does not jump into column 2 → Copy
(Cmd/Ctrl+C) → the clipboard holds that paragraph's text, matching `pdftotext` for the same
region except for whitespace → double-click a word → selects it → triple-click → selects the
line → Select All (Cmd/Ctrl+A) → the page text is selected → copy text from a fixture with
ligatures and a ToUnicode CMap → the clipboard has the decoded Unicode (e.g. "fi", not a
private-use glyph) → open a fixture whose permissions deny copying (owner password set) →
Copy is disabled and a notice explains why.

## J-RD-17: Follow an internal link
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_internal_link, e2e.macos.rd_internal_link

Steps: open a fixture with a table of contents made of Link annotations (/Dest arrays,
named destinations, GoTo actions) → hovering a link shows a hand cursor and the target page
→ click "Chapter 3" → the view goes to the target page and position → Go Back → returns to
the table of contents → a link with a broken destination does nothing and shows a "link
target not found" notice → links still work in every view mode and rotation.

## J-RD-18: Follow an external link with confirmation
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_external_link, e2e.macos.rd_external_link

Steps: open a fixture with URI links to `https://example.com/a`, a `mailto:` address, a
`javascript:` URI, a `file:` URI and a URI with misleading display text → hover → the real
target is shown → click the https link → a confirmation shows the full URL (with the
punycode form of any non-ASCII host) → Cancel → nothing opens → click again → Open → the
default browser opens the URL (web: a new tab with `noopener,noreferrer`) → the `mailto:`
link asks for confirmation the same way → the `javascript:` and `file:` links never open
anything and show "unsupported link" → no PDF JavaScript runs at any point (CONSTITUTION 7).

## J-RD-19: Read in full screen and presentation mode
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_presentation, e2e.macos.rd_presentation

Steps: open a 10-page fixture → Enter Full Screen (Ctrl+Cmd+F / F11) → toolbar and sidebar
hide, the document fills the screen, moving the pointer to the top shows the toolbar → Exit
→ back to the window → Start Presentation (slide mode) from page 3 → page 3 fills the screen
on black → arrow keys, Space and click advance; Left/Backspace go back → at the last page a
hint says it is the end and Esc exits → Esc at any time returns to the normal view on the
current page → page transitions (/Trans) are shown as a simple fade or not at all; no
auto-advance (/Dur) unless the user turns it on.

## J-RD-20: Read with a dark or sepia page tint
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_page_tint, e2e.macos.rd_page_tint

Steps: open a fixture with black text on white and a color photo → View > Page Tint > Dark →
pages show light text on a dark background, and the photo keeps recognizable colors (images
are not inverted) → Sepia → a warm paper tone → Off → original colors → the tint persists
for the next file → the app appearance setting (Light / Dark / Follow system) changes the
chrome independently of the page tint → print and export are never tinted → the file is
unchanged.

## J-RD-21: Print a document
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_print, e2e.macos.rd_print

Steps: open a fixture with annotations (one marked NoView+Print, one Hidden) → Print
(Cmd/Ctrl+P) → the print options offer document only / document and annotations, page range
(all, current, "2-4,7"), scale (fit / actual size / custom %), pages per sheet and auto-rotate
→ the preview reflects each choice → print to PDF (macOS: Save as PDF; web: the browser's
print-to-PDF) → the output has the chosen pages, and annotations follow their print flags
(the Print-flagged one appears, the Hidden one doesn't) → an invalid range ("9-3", "0") is
rejected with a hint → a fixture whose permissions deny printing → Print is disabled with a
notice.

## J-RD-22: View document properties
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_properties, e2e.macos.rd_properties

Steps: open a fixture with Info and XMP metadata that disagree on the title, PDF 1.7,
tagged, linearized, encrypted with an owner password → File > Document Properties (Cmd+D /
Ctrl+D) → shows file name, location, file size, page count, page size of the current page,
PDF version, title, author, subject, keywords, creator, producer, created and modified dates
(in local time), tagged and fast web view yes/no → the title follows ISO 32000-2 §14.3.2
precedence and both values are visible → the Security section lists the encryption method
and each permission (print, copy, modify, annotate, fill forms, assemble, accessibility) →
metadata with control characters or very long values is shown escaped and truncated, never
interpreted → Close → the file is unchanged.

## J-RD-23: Work with several documents at once
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_multi_docs, e2e.macos.rd_multi_docs

Steps: open three fixtures → each is its own tab (macOS: native window tabs; web: tabs in
the app's tab bar) → Next Tab / Previous Tab (Ctrl+Tab / Ctrl+Shift+Tab) cycle them → each
keeps its own page, zoom, layout and sidebar state → open a file that is already open →
the existing tab is focused, not a duplicate → close the middle tab (Cmd/Ctrl+W) → the
others stay → one document crashing its engine session (hostile fixture) shows an error in
that tab only; the others keep working.

## J-RD-24: Restore the previous session on launch
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_restore_session, e2e.macos.rd_restore_session

Steps: turn on "Reopen files from the last session" in Preferences → open fixtures A (page
12, 150%) and B (page 3, fit width) → quit the app (web: close the tab) → launch again →
A and B reopen at the same pages, zoom and layout → a file deleted in between is skipped
with a notice → an encrypted file asks for its password again (passwords are never stored)
→ turn the preference off → launch → the start screen shows, nothing reopens.

## J-RD-25: Inspect detail with the magnifier
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_magnifier, e2e.macos.rd_magnifier

Steps: open a fixture with 4-pt text → View > Magnifier → a loupe follows the pointer and
shows the area under it at a higher zoom, rendered sharp from the engine (not an upscaled
bitmap) → scroll or a shortcut changes the loupe zoom → Esc or toggling the menu item turns
it off → the page view zoom never changed.

## J-RD-26: Open a very large or web-optimized file progressively
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_progressive_large, e2e.macos.rd_progressive_large

Steps: open a 2,000-page, 300 MB fixture → the first page appears within the first-page
budget while the rest loads → a progress indicator shows loading state → jump to page 1,900
before loading finishes → it renders without waiting for the whole file to be indexed →
open a linearized fixture → page 1 renders from the first-page section before the rest is
read → scrolling quickly through hundreds of pages keeps the UI responsive and memory within
the M4 budget → close the file mid-load → loading is cancelled and memory is released.
