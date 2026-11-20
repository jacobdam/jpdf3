# Journeys: reader and navigation

Area `RD`. PLAN §4 (Reader), feature groups F-RD-1 … F-RD-5 in [specs/TRACE.md](../TRACE.md).
Inventory and coverage: [RESEARCH.md](RESEARCH.md). Format: ops/judge/CONTRACT.md §4.

Shared rules for every journey in this file:
- Reader journeys never modify the file: unless a step says otherwise, the file's bytes on
  disk are identical before and after, and the document is not marked as edited.
- "Fixture" means a PDF we generate in the test harness (never a corpus file).
- Web: files are opened from the local disk and never leave the device (PLAN R11). Every web
  journey asserts that after the app shell has loaded the only network requests are
  same-origin GETs for static app assets (code, fonts, CMaps); no request carries file bytes,
  passwords or anything derived from the document; and no cross-origin request is ever made.
- Web file handles: the app keeps persistent file handles behind an adapter. Web tests use
  the desktop Chromium build and inject Origin Private File System handles (always granted)
  through it, and assert the app's own "reopen" buttons, never the browser's permission
  prompt. The fallback for browsers without persistent handles is forced by a test flag.
- All rendering, text, search, page-label resolution and hit-testing come from the Rust
  engine; the native layer only displays results (CONSTITUTION 6). Tints and print output
  are produced by the engine's renderer, not by CSS or platform filters.
- Shortcuts: macOS uses the standard macOS keys. The web app never relies on keys the
  browser reserves (Ctrl+Tab, Ctrl+W, Ctrl+N, Ctrl+T, Ctrl+R, F11); where a step names a
  macOS key that the browser reserves, web uses the app's toolbar or the alternative given.
- "Budgets" refer to PLAN §5 (performance targets): first page < 200 ms for a typical 10 MB
  document, 60 fps scrolling, peak memory bounded by `Limits` and not proportional to file
  size for linearized reading. These timing targets are enforced by the bench (`tools/bench`),
  not by the E2E tests: journeys assert ordering and behavior, which stay stable on shared CI
  runners.
- Engine gate: steps that act "while" a load, search or render is running (Esc, jump, close)
  first pause the engine job through a test-only gate in the engine API (the job blocks
  after N pages or N bytes until the test releases it), act, then release. This keeps those
  steps independent of machine speed. The gate is compiled only into test builds.
- Link policy: the engine classifies every link target (allowed scheme or not), normalizes
  it (punycode hosts, escaped control and bidi characters, stripped mailto attachment
  parameters) and returns the display string and the openable URL. The native layer only
  shows the confirmation and calls the platform open API (CONSTITUTION 5 and 6).
- Random-bytes fixtures use a fixed seed.

## J-RD-01: Open a PDF from the file dialog and see the first page fast
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_dialog, e2e.macos.rd_open_dialog

Steps: launch the app → choose Open (toolbar button; macOS also File > Open and Cmd+O; web
also Ctrl+O) → pick a 20-page, 10 MB fixture in the system file dialog (web: the test answers the
picker through the file-handle adapter from the shared rules; with the no-handles flag, the
`<input type=file>` fallback is driven through Playwright's `filechooser` event) → the first page is rendered before the thumbnails and later pages (the PLAN §5
first-page target is measured by the bench) → the
window title shows the file name → the page indicator reads "1 / 20" → the rendered first
page matches the MuPDF and Poppler renders of the same page within the render-ratchet
tolerance.
Error branch: cancel the dialog (web: the adapter returns "cancelled"; fallback: the
`filechooser` is dismissed) → nothing opens, no error is shown.

## J-RD-02: Open a PDF by drag and drop
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_drop, e2e.macos.rd_open_drop

Steps: drop a fixture PDF onto the empty start screen → it opens on page 1 → drop a second
fixture onto the open document window → it opens as another document (J-RD-23) without
closing the first → drop a non-PDF file (a `.txt`) → a "not a PDF" error is shown, nothing
else changes → drop two PDFs at once → each opens as its own document.
Test mechanism: web dispatches a `drop` event with a `DataTransfer` holding the files
(Playwright); macOS drives the window's drop handler through a test pasteboard hook, and
covers Finder/Dock opening by sending the open-documents Apple Event (`open -a`) → the file
opens the same way.

## J-RD-03: Reopen a file from the recent files list
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_recent_files, e2e.macos.rd_recent_files

Steps: open fixtures A then B → close both → the start screen lists B then A with name,
size and last-opened time → click A → it opens → remove A from the start-screen list → it
disappears → reopen B from the list after deleting it on disk → a "file not found" message
is shown and B is removed from the list → open fixture C, then close it → clear the list →
it is still empty after restarting the app.
macOS: File > Open Recent shows the same files (system recent-documents menu; Clear Menu
empties it). Web: the list keeps local file handles only, never copies of the bytes (test handles per
the shared rules). With the no-handles test flag set, the list shows names only, and
clicking an entry opens the file picker instead of the file.

## J-RD-04: Open a password-protected PDF
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_password, e2e.macos.rd_open_password

Steps: open an AES-256 (R6) fixture with a user password → a password prompt appears with
the file name and no page content or thumbnail visible → enter a wrong password → an
"incorrect password" message, the prompt stays → enter the right password → page 1 is shown
→ cancel the prompt on another encrypted fixture → nothing opens, no page content or
thumbnail is shown.
Security check: after the run, the test searches the app's storage (macOS: the app and
helper containers, the user's Keychain items for the app, its preferences domain and the
unified log for the test process; web: IndexedDB, localStorage,
Cache Storage, console log) for the password string → not found. The recent files list never
holds it.

## J-RD-05: Open a damaged or hostile file
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_damaged, e2e.macos.rd_open_damaged

Steps: open a fixture with a broken xref table (offsets shifted) → it opens after repair
with the same page count MuPDF and Poppler report → open a fixture whose catalog and page
tree come first, truncated after its third page's content (no xref, no trailer) → it opens
by repair with exactly 3 pages (the pages whose content survives) → open a fixture
whose page 1 content nests deeper than `Limits` allows → a typed "file too complex" error
at open or at first render of page 1, never a crash or hang → open a random-bytes file renamed `.pdf` →
a "not a PDF" error → after each error the app stays responsive and opens a valid fixture.
macOS: the app window never crashes, even if the XPC helper has to restart.

## J-RD-06: Navigate between pages
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_navigate_pages, e2e.macos.rd_navigate_pages

Steps: open a 20-page fixture with page labels i–iv on pages 1–4 and 1–16 on pages 5–20 →
Next Page (Right arrow, Page Down, toolbar) → page 2 → Last Page (End) → page 20 → First
Page (Home) → page 1 → type "iii" in the page field → physical page 3 → type "7" → the page
labelled 7 (physical page 11): an exact label match wins, otherwise the text is read as a
physical page number → type "#7" → physical page 7 → type "99" → an "invalid page" hint, the
page doesn't change → Go Back (macOS Cmd+[, web the toolbar Back button) → physical page 11
→ Go Back → physical page 3 → Go Forward (macOS Cmd+], web the toolbar Forward button) →
physical page 11.
The page field shows the label and "(11 of 20)" when labels differ from physical numbers.

## J-RD-07: Switch page layout (single, continuous, two-page, cover)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_view_modes, e2e.macos.rd_view_modes

Steps: with default preferences, open a 9-page fixture whose catalog has no /PageLayout, go
to page 5 → View > Single Page → only page 5 shows; scrolling moves page by page → Single Page
Continuous → pages scroll as one strip → Two Pages → spreads are 1–2, 3–4, 5–6, …, so pages
5–6 show side by side → Two Pages Continuous → a scrolling strip of spreads → turn on "Show
cover page" → page 1 alone, then 2–3, 4–5 → in every mode the current page stays 5 → open a
fixture whose /PageLayout is TwoColumnLeft → it opens in two-page continuous without cover
(the file's /PageLayout wins when the user has not set a default) → set Single Page as the
default layout in Preferences → open both fixtures again → both open in Single Page (an
explicit user default wins over the file) → open a fixture with /ViewerPreferences /Direction
/R2L and switch to Two Pages → its spreads run right to left (page 2 left of page 1).

## J-RD-08: Zoom and fit
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_zoom_fit, e2e.macos.rd_zoom_fit

Steps: open a fixture with a Letter page and an A3 landscape page → Zoom In (macOS Cmd+=,
web Ctrl+= handled by the app, toolbar +) → the zoom goes to the next step and the indicator
shows the percentage → Zoom Out → the previous step → Actual Size → 100%, where 1 PDF point
(times /UserUnit if present) is 1 typographic point on screen (web: 1 CSS pt = 4/3 CSS px)
→ Fit Width → the current page's width fills the view → Fit Page → the whole current page
is visible → Fit Height → the current page's height fills the view → go to the A3 page →
the fit mode is kept and recomputed for it → type 250 in the zoom field → 250% → Ctrl+scroll
(macOS also trackpad pinch, not part of the automated test) → zooms around the pointer →
type 5 and then 99999 → the zoom clamps to 10% and 6400% → text is re-rendered sharp at 400%
(the rendered tile's pixel density matches the zoom; no upscaled bitmap) → set Fit Width as
the default zoom → new files open at Fit Width.

## J-RD-09: Rotate the view
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_rotate_view, e2e.macos.rd_rotate_view

Steps: open a fixture whose page 2 has /Rotate 90 → Rotate Right (macOS Cmd+R, web the
toolbar button) → all pages display rotated 90° clockwise on top of their
own rotation (page 2 shows at 180°) → Rotate Left twice → rotated 90° counter-clockwise →
text selection and links still hit the right glyphs and areas in the rotated view → close
the file → no "save changes?" prompt → the file bytes are unchanged → reopen → the file's
own orientation again.
(Rotating pages in the file itself is a Pages journey, not this one.)

## J-RD-10: Browse thumbnails in the sidebar
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_thumbnails, e2e.macos.rd_thumbnails

Steps: open a 500-page fixture → show the sidebar (View > Sidebar) → Thumbnails tab → the
thumbnails of the visible range appear first, the rest render lazily while scrolling → the
current page is highlighted → click thumbnail 250 → the view jumps to page 250 → scroll the
document → the highlight follows → change the thumbnail size (small/large) → the layout
updates → hide the sidebar → the document area widens. Scrolling all 500 thumbnails keeps
the engine's peak memory under its `Limits` cache bound (thumbnails are evicted, not kept).

## J-RD-11: Navigate with the outline (bookmarks)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_outline, e2e.macos.rd_outline

Steps: open a fixture with a three-level outline (some entries closed by default, one with a
/XYZ destination, one with a named destination, one with a GoTo action, one with a URI
action, one with a JavaScript action, one with a Launch action) → the Outline tab shows the
top level, with entries open or closed as the file says → expand a closed entry → children
show → Expand All / Collapse All → click each destination and GoTo entry → the view goes to
the right page and position → click the URI entry → the J-RD-18 confirmation appears →
click the JavaScript and Launch entries → nothing happens except an "unsupported action"
notice → the entry for the current section is highlighted while scrolling → a fixture with
no outline shows "No outline" → a fixture with an outline loop (/Next pointing back) shows
each entry once and does not hang.
(Adding, renaming and deleting bookmarks is in the editing journeys, #8.)

## J-RD-12: Review annotations in the annotation list
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_annotation_list, e2e.macos.rd_annotation_list

Steps: open a fixture with highlights, notes, ink and a FreeText annotation on pages 1, 3
and 8 → the Annotations tab lists them grouped by page with type icon, author, date and
comment text (one comment is `<b>x</b><img src=x onerror=alert(1)>`, shown
literally with those characters, no image request, no alert) → sort by date → by type → back to page → filter to "Notes" only → click an
entry → the view jumps to it and it is selected on the page → collapse and expand a page
group → a fixture with no annotations shows "No annotations". Popup and Link annotations
are not listed. Annotation text is shown as plain text (never interpreted as markup).

## J-RD-13: See the attachments list without opening anything
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_attachments_list, e2e.macos.rd_attachments_list

Steps: open a fixture with two embedded files (one `.exe`, one `.txt`) and one
FileAttachment annotation → the Attachments tab lists all three with name, size, description
and page (for the annotation) → there is no "Open" action, double-click does nothing, and
nothing is written to disk or launched (CONSTITUTION 7) → a fixture with a Launch action and
a GoToE action on links → clicking each link does nothing except an "unsupported action"
notice → an attachment named "report" + U+202E + "txt.exe" is listed with the control
character shown escaped, so the real `.exe` ending is visible → a fixture with no
attachments shows the Attachments tab with "No attachments".

## J-RD-14: Show and hide layers (optional content)
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_layers, e2e.macos.rd_layers

Steps: open a fixture with four OCGs: A (ON), B (OFF by default), C and D (a radio-button
group, C ON), and A also listed in /Locked; B has /Usage /Print /PrintState /ON, and the default
configuration has an /AS entry for the Print event covering B (ISO 32000-2 §8.11.4.4) → the
Layers tab lists them in the /Order tree with that visibility → B's content is not drawn,
matching MuPDF and Poppler → turn B on → its content appears → turn on D → C turns off →
A's toggle is disabled → turn B off again and print (J-RD-21 mechanism) → the printed output
includes B, because the /AS Print entry applies B's print state; the other layers print as
currently shown → close → the file is unchanged → reopen → default
visibility again.

## J-RD-15: Search the document
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_search, e2e.macos.rd_search

Steps: open a 50-page fixture containing "Report" 3 times, "report" 4 times and
"reporting" 2 times → Find (macOS
Cmd+F, web Ctrl+F handled by the app) → type "report" → matches are highlighted on the
pages, the first one is selected and scrolled into view, and a count shows "1 of 9" → Next
(Enter) and Previous (Shift+Enter) cycle through and wrap → the results list shows each
match with page number and surrounding text → turn on Case Sensitive → "Report" no longer
matches: 6 results → turn on Whole Words → "reporting" no longer matches: 4 results → search for a word that is
not there → "No results" → Esc closes the find bar and removes the highlights.

## J-RD-16: Select and copy text
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_select_copy, e2e.macos.rd_select_copy

Steps: open a two-column fixture → choose the text select tool → drag across a paragraph in
column 1 → the selection follows reading order and does not jump into column 2 → Copy
(Cmd/Ctrl+C) → the clipboard holds that paragraph's text, equal to `pdftotext` output for the
same region after collapsing whitespace → double-click a word → it is selected →
triple-click → the line is selected (macOS: the paragraph, per platform convention) → Select
All (Cmd/Ctrl+A) → the whole document's text is selected → copy text from a fixture with an
"fi" ligature glyph mapped by a ToUnicode CMap → the clipboard has "fi" → open a fixture whose
permissions deny copying (owner password set, empty user password) → Copy is disabled and a
notice explains why (J-RD-29 covers unlocking).

## J-RD-17: Follow an internal link
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_internal_link, e2e.macos.rd_internal_link

Steps: open a fixture with a table of contents made of Link annotations (/Dest arrays,
named destinations, GoTo actions) → hovering a link shows a tooltip with the target page
→ click "Chapter 3" → the view goes to the target page and position → Go Back → returns to
the table of contents → a link with a broken destination does nothing but show a "link
target not found" notice → links still work in two-page mode and in a rotated view.

## J-RD-18: Follow an external link with confirmation
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_external_link, e2e.macos.rd_external_link

Steps: open a fixture with URI links to `https://example.com/a`, `http://example.com/b`, a
`mailto:` address, a host with non-ASCII characters, a link whose visible text shows a
different URL, and a URI containing U+202E → hover → a tooltip shows the real target → click the https link → a
confirmation shows the full URL (non-ASCII hosts in punycode, control and
bidi characters shown escaped) → Cancel → nothing opens →
click again → Open → the system browser opens the URL (web: a new tab with
`noopener,noreferrer`; the test intercepts the open call) → `http:` and `mailto:` ask the
same way.
Only `https`, `http` and `mailto` can ever be opened. Links with `javascript:`, `data:`,
`file:`, `vbscript:`, `smb:` and a custom app scheme (`exampleapp:`), and a GoToR action to
another local file, never open anything and show an "unsupported link" notice. The same holds
for the usual bypass spellings: `JaVaScript:`, a leading space or control character
(`\u0001javascript:`), and a tab or newline inside the scheme (`java\tscript:`). A relative
URI is resolved against the catalog's `/URI /Base` first, then checked the same way.
A `mailto:a@example.com?attach=/etc/passwd&attachment=/etc/passwd&subject=hi` link: the
confirmation shows the full address and query, and the URL handed to the system (intercepted
by the test) keeps `subject` but has no `attach` or `attachment` parameter in any case
spelling.

## J-RD-19: Read in full screen and presentation mode
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_presentation, e2e.macos.rd_presentation

Steps: open a 10-page fixture → Enter Full Screen (macOS Ctrl+Cmd+F; web the toolbar
button, using the Fullscreen API) → toolbar and sidebar hide, the document fills the screen,
moving the pointer to the top shows the toolbar → Esc → back to the window → Start
Presentation from page 3 → page 3 fills the screen on black → Right arrow, Space and click
advance; Left arrow goes back → on the last page an "end of document" hint is shown and
advancing does nothing → Esc returns to the normal view on the current page → page
transitions (/Trans) and auto-advance (/Dur) in the file are ignored: pages change
instantly and only on user input.

## J-RD-20: Read with a dark or sepia page tint
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_page_tint, e2e.macos.rd_page_tint

Steps: open a fixture with black text on white and a color photo → View > Page Tint > Dark
→ the page background renders dark and the text light, while every pixel in the photo
region stays within ΔE 10 of the untinted render (images are not inverted) → Sepia → the
page background renders in the sepia tone and the photo region again stays within ΔE 10 →
Off → the render equals the untinted render → the tint persists for the next file → the app
appearance setting (Light / Dark / Follow system) changes the chrome independently of the
page tint → print output (J-RD-21 mechanism) is never tinted → the file is unchanged.

## J-RD-21: Print a document
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_print, e2e.macos.rd_print

Steps: open a 10-page fixture with three annotations on page 3: one with the Print flag,
one without it, one with Print and Hidden → Print (macOS Cmd+P, web the toolbar button) → the print options
offer content (document only / document and annotations), page range (all, current,
"2-4,7"), scale (fit / actual size / custom %), pages per sheet (1, 2, 4) and auto-rotate →
choose "document and annotations", range "2-4,7", 2 per sheet → the engine's print job has
those 4 pages on 2 sheets, and only the Print-flagged annotation appears (ISO 32000-2
§12.5.3: no Print flag or Hidden means not printed) → choose "document only" → no
annotation appears → an invalid range ("9-3", "0") is rejected with a hint.
Test mechanism: the test inspects the engine-produced print job (rendered sheets) before it
is handed to the platform (web: before `window.print()`; macOS: through a test hook on the
print operation).

## J-RD-22: View document properties
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_properties, e2e.macos.rd_properties

Steps: open a PDF 1.7 fixture, tagged, linearized, encrypted with an owner password, whose
Info /Title is "Info Title" with /ModDate 2024-01-01 and whose XMP dc:title is "XMP Title"
with xmp:ModifyDate 2025-01-01 → File > Document Properties (macOS Cmd+I, web the menu
item) → shows file name, file size, page count, page size of the current page, PDF version,
title, author, subject, keywords, creator, producer, created and modified dates (in local
time), tagged and fast web view yes/no; macOS also shows the location (the web app has no
file path) → the title shown is "XMP Title" and the modified date is 2025-01-01, both from the
newer metadata (ISO 32000-1 §14.3.2 for PDF 1.x files) → open a second fixture where the Info
/ModDate (2026-01-01) is newer than the XMP date → it shows "Info Title" and 2026-01-01 → the Security section lists the encryption method and each permission (print,
high-quality print, copy, modify, annotate, fill forms, assemble); text extraction for
accessibility is always shown as allowed (ISO 32000-2 deprecates that bit) → metadata containing control
characters, U+202E (right-to-left override) and a 10,000-character value is shown escaped
and truncated, never interpreted → Close → the file is unchanged.

## J-RD-23: Work with several documents at once
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_multi_docs, e2e.macos.rd_multi_docs

Steps: open three fixtures → each is its own tab (macOS: native window tabs; web: tabs in
the app's tab bar) → switch tabs (macOS Ctrl+Tab / Ctrl+Shift+Tab; web clicking the tab bar
or Alt+PageDown / Alt+PageUp) → each keeps its own page, zoom, layout and sidebar state →
open a file that is already open → the existing tab is focused, not a duplicate (macOS: same
file URL; web: the same file handle, or, with the no-handles test flag, the same name, size
and content hash) → close the middle tab (macOS Cmd+W; web the tab's close button) →
the others stay → a test-only fault injected into one document's engine session makes that
tab show an error, while the other tabs keep working.

## J-RD-24: Restore the previous session on launch
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_restore_session, e2e.macos.rd_restore_session

Steps: turn on "Restore open documents at launch" in Preferences → open fixtures A (page
12, 150%) and B (page 3, fit width) → quit the app (web: close the browser tab) → launch
again → A and B reopen at the same pages, zoom and layout → a file deleted in between is
skipped with a notice → an encrypted file asks for its password again (passwords are never
stored) → turn the preference off → launch → the start screen shows, nothing reopens.
Web: browsers need a user gesture to re-grant file access, so on launch the app lists the
session's files with one "Reopen all" button; clicking it reopens them as above (test handles
per the shared rules). With the no-handles test flag set, the list shows names only.

## J-RD-25: Inspect detail with the magnifier
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_magnifier, e2e.macos.rd_magnifier

Steps: open a fixture with 4-pt text → View > Magnifier → a loupe follows the pointer and
shows the area under it at 4× the current zoom, rendered by the engine at that resolution →
press "+" while the loupe is shown → 8× → Esc → the loupe turns off → the page view zoom
never changed.

## J-RD-26: Open a very large or web-optimized file progressively
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_progressive_large, e2e.macos.rd_progressive_large

Steps: open a linearized 2,000-page, 300 MB fixture → page 1 renders from the
first-page section before any byte past that section is read (engine read log) → a progress
indicator shows the loading state (the load is held at the engine gate after page 1) →
jump to page 1,900 while the load is still held → it renders
without waiting for the whole file to be indexed → scroll quickly through 200 pages → no
input event waits on the engine (rendering is asynchronous; the 60 fps target is measured
by the bench), and peak engine memory stays under its `Limits` bound and does not grow with
file size (compared with a 30 MB variant of the same fixture) →
hold a fresh load at the engine gate and close the file → the engine job reports
"cancelled" and engine memory returns to its pre-open level (engine memory stats).

## J-RD-27: Respect the document's initial view safely
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_initial_view, e2e.macos.rd_initial_view

Steps: open a fixture whose /OpenAction is a destination to page 4 at /FitH → it opens on
page 4 at fit width → a fixture with /PageMode /UseOutlines → it opens with the Outline tab
shown → /PageMode /UseThumbs → the Thumbnails tab → /PageMode /UseAttachments → the
Attachments tab (nothing is opened) → /PageMode /UseOC → the Layers tab → /PageMode /FullScreen → it opens in the normal window
with a notice offering full screen; full screen starts only if the user accepts → a
/ViewerPreferences with /HideToolbar and /HideMenubar → the toolbar and menus stay visible
(the app never hides its own chrome because a file asks) → /ViewerPreferences /DisplayDocTitle
→ the window title shows the document title instead of the file name.

## J-RD-28: Scripts and automatic actions in a PDF never run
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_scripts_inert, e2e.macos.rd_scripts_inert

Steps: open a fixture with a document /OpenAction JavaScript, a /Names /JavaScript entry, a
page /AA open action, document /AA actions (WC, WS, DS, WP, DP), a link /AA mouse-enter
action, a widget /AA action, and Rendition, Sound, Movie and RichMedia actions; every script
tries something visible (show an alert, go to page 9, open `https://example.com/x`, print)
→ open, scroll through every page, hover and click every link and widget → no alert
appears, the page never changes on its own, no URL is opened, no print starts (CONSTITUTION
7) → a one-time notice says the document contains scripts that were not run → fixtures whose
/OpenAction is a Launch action, a URI action, and whose page /AA is a SubmitForm action and an
ImportData action → nothing launches, no link confirmation appears unprompted, no network
request is made, nothing is read from disk, and the same kind of notice is shown.

## J-RD-29: Unlock permission-restricted features with the owner password
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_owner_unlock, e2e.macos.rd_owner_unlock

Steps: open a fixture with an empty user password and an owner password that denies
printing and copying → it opens without a prompt → Print and Copy are disabled; Document
Properties lists the restrictions → choose "Unlock with password" → enter a wrong password
→ "incorrect password" → enter the owner password → Print and Copy become available for
this session → close and reopen → restricted again (the password is never stored).

## J-RD-30: Open files with every supported encryption variant
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_open_encryption_variants, e2e.macos.rd_open_encryption_variants

Steps: open fixtures encrypted with RC4-40 (R2), RC4-128 (R3), AES-128 (R4) and AES-256
(R6), each with the user password "user" → each prompts and opens with that password, and
its first page matches the MuPDF and Poppler renders → open an R6 fixture whose user
password is non-ASCII ("pässwörd", prepared with SASLprep as ISO 32000-2 §7.6.4.3 requires for R6) → it
opens with that password typed as composed or decomposed Unicode → open an R6 fixture with
an empty user password and an owner password → it opens without a prompt → open a fixture
using the deprecated R5 handler → it opens (read leniently, CONSTITUTION 17).

## J-RD-31: Search across lines, ligatures and non-Latin scripts
- milestone: M4
- platform: web, macos
- test: e2e.web.rd_search_text_forms, e2e.macos.rd_search_text_forms

Steps: open a fixture where "annual report" is split across a line break, "office" uses an
"ffi" ligature glyph mapped by a ToUnicode CMap, and pages hold Japanese and Arabic text →
search "annual report" → found across the line break → search "office" → found on the
ligature → search a Japanese word → found → search an Arabic word → found, highlighted on
the right glyphs (right to left) → start a search in a 2,000-page fixture, held at the engine gate after
50 pages, and press Esc → release the gate → the engine job reports "cancelled" before reaching the last page, no further
results are added, and the page view keeps responding to scrolling throughout (the search
runs as a cancellable engine job).
