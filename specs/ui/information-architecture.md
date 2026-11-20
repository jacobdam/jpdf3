# UI information architecture

Status: ready (M0, #10). Wireframes per platform: #11 (`specs/ui/wireframes-macos.md`,
`specs/ui/wireframes-web.md`). Companions: [shortcuts.md](shortcuts.md),
[accessibility.md](accessibility.md).

This is the shape of the app shared by macOS (SwiftUI, Apple HIG) and web (React + MUI). It
says which areas exist, what each shows in each tool mode and which journeys exercise it. It
borrows the overall idea of a mode-based PDF workspace from the PDFgear reference
(AUTOMATION §5.4) and describes it in our own words; no PDFgear strings, icons or layouts
are copied (CONSTITUTION 20). Where a detail here and a journey disagree, the journey wins
and this file is fixed.

## 0. Rules for every area

1. **Engine owns the logic** (CONSTITUTION 6). The native layer draws what the engine
   returns: rendered tiles, text runs, hit-test results, annotation and page lists, search
   hits, outline, labels, errors. Tool behavior (what a click or drag does) is an engine
   command. The UI keeps only view state: which mode, panel, tab and zoom is shown.
2. **Enabled state comes from the engine.** Whether a command is available (permissions,
   signed-document rules, read-only file, a running job) is an engine capability query. A
   disabled control shows why (tooltip / accessibility hint) using the engine's typed
   reason. The engine still rejects the command if the UI is wrong.
3. **Untrusted text is plain text.** File names, metadata, annotation text, outline titles,
   link targets and error details from the file are shown as plain text, never markup (web:
   no `dangerouslySetInnerHTML`; macOS: no attributed-string parsing of file text). Control
   and bidi characters (for example U+202E) are shown escaped, using the display string the
   engine returns, so a hidden `.exe` ending stays visible (J-RD-13, J-RD-18, J-RD-22). Long
   values are truncated with the full escaped value in the tooltip / accessibility value.
4. **One look per platform.** macOS uses system controls, SF Symbols, system colors and
   materials; web uses MUI components with the MUI theme. The structure below is the same;
   the controls follow each platform (see §10).
5. **Every area has its empty, loading and error states** written down below; no area may
   show a blank region without text.

## 1. Window layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ Title / document tabs                                                │
├──────────────────────────────────────────────────────────────────────┤
│ Main toolbar: file actions · mode switcher · view controls · search  │
├──────────────────────────────────────────────────────────────────────┤
│ Mode tool row (tools of the current mode; hidden in Read)            │
├────────────┬───────────────────────────────────────────┬─────────────┤
│ Sidebar    │ Find bar (when open)                      │ Inspector   │
│ (panels)   ├───────────────────────────────────────────┤ (selection  │
│            │ Document view                             │  properties)│
│            │                                           │             │
├────────────┴───────────────────────────────────────────┴─────────────┤
│ Status bar: page field · zoom · layout · job progress · edited state │
└──────────────────────────────────────────────────────────────────────┘
```

This is a structural sketch, not a wireframe. Sidebar and inspector are independently
collapsible; both are hidden in full screen and presentation (J-RD-19). Each document tab
keeps its own mode, page, zoom, layout, sidebar panel and inspector state (J-RD-23).

| Area | What it holds | Journeys |
|---|---|---|
| Document tabs | One tab per open document; edited marker; close button; error state per tab | J-RD-23, J-RD-24 |
| Main toolbar | Open, Save, Print; mode switcher; sidebar and inspector toggles; zoom out/in; back/forward; full screen; find | J-RD-01, J-RD-06, J-RD-08, J-RD-19, J-RD-21, J-OS-01 |
| Mode tool row | The tools of the current mode (§3) | J-ANN-01…J-ANN-15, J-ED-01…J-ED-10, J-PG-01…J-PG-16 |
| Sidebar | Panels: thumbnails, outline, annotations, attachments, layers, search results (§4) | J-RD-10…J-RD-15, J-ANN-19, J-PG-14 |
| Find bar | Query field, match count, previous/next, case and whole-word toggles, close | J-RD-15, J-RD-31 |
| Document view | Rendered pages, selection, tool interaction, links | J-RD-06…J-RD-09, J-RD-16…J-RD-20, J-RD-25, J-RD-27, J-RD-28 |
| Inspector | Properties of the current selection (§5) | J-ANN-16, J-ED-02, J-ED-03, J-PG-12 |
| Status bar | Page field: "11 / 20", or the page label plus "(11 of 20)" when labels differ from physical numbers (J-RD-01, J-RD-06); zoom field and menu; layout menu, job progress with cancel, edited state | J-RD-06, J-RD-07, J-RD-08, J-RD-26 |
| Menus | macOS menu bar; web app menu in the toolbar (§6) | J-RD-01, J-RD-03, J-RD-22, J-PG-09, J-PG-13, J-OS-02, J-OS-05 |

## 2. Start screen (no document)

Shown when no document is open (first launch, last tab closed).
- A primary Open button and a large drop target ("drop a PDF here" in our own wording)
  (J-RD-01, J-RD-02).
- Recent files: name, size, when last opened; reopen on click; remove one; clear all
  (J-RD-03). Web lists only files whose persistent handles exist (reader.md shared rules);
  with no handles it shows a short explanation instead of an empty list.
- Create: new blank document, new from images, merge files (J-PG-01, J-PG-09, J-OS-04).
- Empty state when there are no recent files: one sentence and the Open button, never a
  blank list.
- Web session restore (J-RD-24): when "Restore open documents at launch" is on, the start
  screen lists the last session's files with one "Reopen all" button (browsers need a user
  gesture to re-grant file access); macOS reopens them directly.
- A recent or session file that no longer exists shows "file not found" and is removed from
  the list (J-RD-03, J-RD-24).

## 3. Tool modes

The mode switcher is a segmented control (macOS) / tabs (web) in the main toolbar. Exactly one
mode is active per tab. Switching mode never changes the document; an unfinished tool action
(a polyline being drawn, an open text editing box) is committed or cancelled first, as the
engine command for that tool defines. Read is the default for every newly opened document.

| Mode | Purpose | Tool row contents | Inspector shows | Journeys |
|---|---|---|---|---|
| **Read** | Read, navigate, select, fill forms | none (the row is hidden); the main toolbar has a text-select / hand (pan) toggle, text select by default; link following and form filling work | nothing unless an annotation or field is selected (read-only view of its properties) | J-RD-01…J-RD-31 |
| **Annotate** | Comment and mark up | select · highlight · underline · strikeout · squiggly · note · text box · callout · shapes menu (rectangle, ellipse, line, arrow, polygon, polyline, cloud) · ink · eraser · stamps menu · signature menu · stroke width, color and opacity quick controls · a More menu with flatten annotations and export / import comments (XFDF, FDF) | annotation properties: color, opacity, stroke width and style, fill, font and size, author, subject, lock state, comment and replies | J-ANN-01…J-ANN-26 |
| **Edit** | Change page content | edit text · add text · add image · link · watermark · header and footer · Bates numbers · background · crop | (the row also starts with the select tool) text format, image arrange/flip/rotate/replace, link target, or the settings of the open watermark/header/Bates/background job | J-ED-01…J-ED-10 |
| **Pages** | Organize pages and the outline | the document view becomes the page organizer grid · insert (blank, from file, from images) · delete · extract · rotate left/right · duplicate · move left/right · split · page labels | the selected pages: count, size, rotation, label | J-PG-01…J-PG-16 |
| **Forms** | Create and edit form fields | field types (text, check box, radio, list, combo, push button, signature field) · align · tab order · flatten | field properties: name, tooltip, required, read-only, format and calculation (built-in set only), appearance | #9 |
| **Secure** | Passwords, permissions, redaction, signatures | encrypt · change permissions · remove security · mark for redaction · apply redactions · scrub metadata · sign · certify · signatures panel | the selected redaction mark or signature field; certificate details of a selected signature | #9 |

Notes:
- **Form filling** happens in Read mode (and every other mode): clicking a field fills it.
  Forms mode is for designing fields. Detailed in #9.
- **Bookmark editing** (add, add child, rename, delete, move) lives in the outline panel and
  is available in every mode except presentation (J-PG-14).
- **Optimize and export** (compress, export pages as images, Save As) are commands in the File
  menu, not a mode (J-OS-02, J-OS-03, J-OS-05).
- The **select tool** of Annotate and Edit selects text or an object, whichever the engine's
  hit-test reports at the pointer (J-ANN-01, J-ANN-17). Esc follows the order in
  [shortcuts.md](shortcuts.md) rule 9.
- A mode whose every command is disabled for the document (for example Edit on a file whose
  permissions forbid modification) stays selectable; its tool row shows the disabled tools with
  the engine's reason and, where the journeys allow it, an unlock action (J-RD-29, J-ANN-23,
  J-PG-15).

## 4. Sidebar

A tab strip at the top of the sidebar selects one panel. The sidebar remembers its width and
panel per document tab. It is resizable, with a minimum width that fits a page thumbnail and a
maximum of one third of the window.

| Panel | Contents | Empty state | Journeys |
|---|---|---|---|
| Thumbnails | Page thumbnails rendered lazily, current page highlighted, page label under each, small/large size; multi-select; drag to reorder in Pages mode | n/a (a document has ≥ 1 page) | J-RD-10, J-PG-06, J-PG-13 |
| Outline | The bookmark tree, expand/collapse (one level, all), click to go; edit actions (§3 notes) | "No outline", with an Add action when editing is allowed | J-RD-11, J-PG-14, J-PG-16 |
| Annotations | Grouped by page, sort by page/date/type, filter by type and author, count, select all, delete, reply and status | "No annotations" | J-RD-12, J-ANN-18, J-ANN-19 |
| Attachments | Name, size, description, page (for attachment annotations); no Open action, double-click does nothing, nothing is written to disk or launched (CONSTITUTION 7) | "No attachments" | J-RD-13 |
| Layers | Optional content groups with visibility toggles; locked groups shown as locked | "No layers" | J-RD-14 |
| Search results | Each hit with page number and context; click to go | before a search: a hint to type a query; after: "No results" | J-RD-15, J-RD-31 |
| Signatures | Each signature with its validation summary | "No signatures" | #9 |

Panels that the document has no data for stay in the tab strip (never hidden) so their position
is stable for keyboard and VoiceOver users.

## 5. Inspector

The right-hand inspector shows the properties of the current selection. It is hidden by
default in Read mode and shown by default in the other modes; the user's toggle wins per tab.
- Nothing selected: the mode's defaults for new objects (Annotate: the active tool's color,
  width, opacity, font) or a short hint, never a blank panel.
- One object: its properties (§3 table). Changes apply live and are one undo step per
  committed change (J-ANN-16, J-ANN-20).
- Several objects of different kinds: only the shared properties; mixed values show as mixed.
- Locked annotations show their properties read-only, with the reason (J-ANN-24).

## 6. Menus and commands

macOS uses the menu bar (HIG: App, File, Edit, View, Go, Tools, Window, Help). Web shows the
same groups as MUI menu buttons at the start of the main toolbar. Every menu command also has a
toolbar, context-menu or keyboard path where the journeys name one ([shortcuts.md](shortcuts.md)).

| Menu | Commands | Journeys |
|---|---|---|
| File | New (blank, from images, from copied pages), Merge Files, Open, Open Recent, Close, Save, Save As, Export (pages as images), Compress, Print, Document Properties | J-RD-01, J-RD-03, J-RD-21, J-RD-22, J-PG-01, J-PG-09, J-PG-13, J-OS-01…J-OS-05 |
| Edit | Undo, Redo, Cut, Copy, Paste, Delete, Select All, Find, Find Next/Previous, Preferences (macOS: in the App menu) | J-ANN-17, J-ANN-20, J-RD-15, J-RD-16 |
| View | Page layout, zoom and fit, rotate view, sidebar and panels, inspector, page tint, magnifier, full screen, presentation | J-RD-07…J-RD-10, J-RD-19, J-RD-20, J-RD-25 |
| Go | Next/previous/first/last page, go to page, back, forward | J-RD-06, J-RD-17 |
| Tools | Switch mode; the current mode's tools | §3 |
| Window (macOS) / tab bar (web) | Next/previous tab, merge windows, the open documents | J-RD-23 |
| Help | Help, keyboard shortcuts list, about | — |

### Context menus

Right-click (macOS also Control-click; web also the context-menu key and Shift+F10) opens a
menu built from the engine's hit-test result at that point:

| Target | Items | Journeys |
|---|---|---|
| Page, no selection | Previous/next page, zoom and fit, rotate view, page layout | J-RD-06…J-RD-09 |
| Text selection | Copy, highlight, underline, strikeout, add note to selection | J-RD-16, J-ANN-01, J-ANN-02 |
| Annotation | Open comment, reply, set status, properties (shows inspector), cut, copy, delete | J-ANN-04, J-ANN-17, J-ANN-18 |
| Link (Edit mode) | Edit target, delete | J-ED-05 |
| Thumbnail(s) | Insert before/after, delete, extract, rotate, duplicate, copy pages, paste pages | J-PG-01…J-PG-08, J-PG-13 |
| Outline entry | Go to, add, add child, rename, cut, paste (as the drag-free way to move an entry), delete | J-RD-11, J-PG-14 |
| Annotation list entry | Go to, reply, set status, delete | J-ANN-19 |
| Document tab | Close, close others, show in Finder (macOS only) | J-RD-23 |

## 7. States

| State | Where | What the user sees | Journeys |
|---|---|---|---|
| Empty (no document) | Window | Start screen (§2) | J-RD-03 |
| Loading | Document tab | The tab shows the file name at once; the first page appears before the rest; pages not yet rendered show a neutral placeholder of the right size (never a spinner over the whole window); the status bar shows progress and a cancel action; Esc cancels | J-RD-01, J-RD-26 |
| Job running | Status bar | Long operations (save, compress, merge, export, search) show progress, can be cancelled where the engine allows, and never block other tabs | J-RD-15, J-OS-03, J-OS-06 |
| Open failed | Document tab | An error view naming the file and the engine's typed error in plain words ("damaged beyond repair", "not a PDF", "too large for the safety limits"), with Close; no stack traces or internal details | J-RD-05 |
| Engine session failed | Document tab | Same error view for that tab only; other tabs keep working | J-RD-23 |
| Password needed | Document tab | Password dialog (§8); no page content or thumbnail is visible behind it | J-RD-04, J-RD-30 |
| Unsupported link / action | Document view, outline | A URI with a disallowed scheme, or a GoToR link, shows an "unsupported link" notice (J-RD-18); a Launch, GoToE or JavaScript action shows an "unsupported action" notice (J-RD-11, J-RD-13); nothing else happens | J-RD-11, J-RD-13, J-RD-18 |
| Scripts present | Document tab | A one-time notice that the document contains scripts that were not run | J-RD-28 |
| Link target missing | Document view | A link whose destination doesn't resolve shows a notice and the view doesn't move | J-RD-17 |
| Initial view from the file | Document tab | The sidebar panel requested by `/PageMode` is shown; `/PageMode /FullScreen` gives a notice offering full screen, which starts only if accepted; `/HideToolbar` and `/HideMenubar` are ignored (the app never hides its own chrome because a file asks) | J-RD-27 |
| No text under a markup drag | Document view | Nothing is created; a hint says no text was found | J-ANN-01 |
| First annotation | Document tab | A non-blocking notice offers to set the author name (default "Guest") | J-ANN-04 (annotate.md shared rules) |
| Restricted | Mode tool row, menus | Disabled commands with the engine's reason and an unlock action | J-RD-29, J-ANN-23, J-PG-15 |
| Edited | Tab, window | macOS: the window's edited dot and "Edited" in the title; web: a dot on the tab and the title | J-OS-01, J-ANN-20 |
| No results / no data | Panels, find bar | One-line empty texts (§4) | J-RD-12, J-RD-15 |

## 8. Dialogs

Dialogs are modal to their document (macOS sheets on the window; web MUI `Dialog`), keep
keyboard focus inside, and default the focused button to the safe choice.

| Dialog | Content | Default button | Journeys |
|---|---|---|---|
| Password | File name, password field (secure entry), wrong-password message in place without closing, Cancel; the password is never stored, logged or added to recent files; web uses a field with `autocomplete="off"` outside any submitting form, macOS turns off password AutoFill for the field, so no password manager offers to save it | Open | J-RD-04, J-RD-30 |
| Owner password unlock | Reached from the "unlock with password" action next to a disabled command; explains which permissions are restricted; password field; the unlock lasts for this session only | Unlock | J-RD-29, J-ANN-23 |
| External link | The engine-normalized URL that will actually open (punycode host, escaped control and bidi characters); for `mailto:` with attachment parameters, a note that they were removed; Open in browser / Cancel. Disallowed schemes never get this dialog, only the notice. Hovering a link shows the same engine display string (or the target page for internal links) as a tooltip | Cancel | J-RD-17, J-RD-18 |
| Unsaved changes | On closing a tab or quitting with edits: Save, Don't Save, Cancel (macOS standard alert wording; web the same three buttons, plus the browser's own leave-page prompt) | Save | #9 |
| File changed on disk | Reload or keep my version | Keep | #9 |
| Confirm delete | Deleting pages, several annotations from the list, or a bookmark with children; states what is deleted and, for pages, that earlier revisions remain in the file after an incremental save (pages-edit.md "History is honest"). Deleting a single object on the page (Delete key) has no confirmation; undo restores it | Cancel | J-PG-04, J-ANN-17, J-ANN-19, J-PG-14 |
| Other confirmations | Only those the journeys define, with their wording rules: flatten (J-ANN-21, J-ANN-26), deleting a reply that has replies (J-ANN-18), Remove for watermark / header and footer / Bates / background (J-ED-06…J-ED-09), page changes on a signed file (J-PG-15), Save As removing earlier versions of a signed file (J-OS-02), compressing a signed file (J-OS-03), output that will not be encrypted (J-PG-09, J-PG-15), replacing existing files with multi-file output (J-PG-10, J-PG-11) | Cancel | as listed |
| Save failed | The engine's or platform's typed error; the original file is untouched | OK | J-OS-06 |
| Print | Content (document only / with annotations), a separate toggle to print comment contents on extra pages, pages, scale, pages per sheet, auto-rotate; then the platform print panel | Print | J-RD-21, J-ANN-25 |
| Document properties | File information, description, security summary (read-only values from the engine) | Close | J-RD-22 |
| Creators | Custom stamp, signature (draw / type / image), watermark, header and footer, Bates, background, page labels, split, merge, compress, export | the action | J-ANN-14, J-ANN-15, J-ED-06…J-ED-09, J-PG-09…J-PG-12, J-OS-03, J-OS-05 |
| Preferences | Appearance (Light / Dark / Follow system), default zoom and layout, "Restore open documents at launch", author name, snapping when dragging, choose print content each time, tool key shortcuts on/off | — | J-RD-07, J-RD-08, J-RD-20, J-RD-24, J-ANN-17 |

## 9. Reading modes

- **Full screen**: toolbar, sidebar and inspector hide; moving the pointer to the top edge
  shows the toolbar; Esc leaves (J-RD-19). On web, full screen uses the Fullscreen API: the
  browser consumes Esc and exits, and the app reacts to `fullscreenchange` (tests assert that
  event, not a keydown).
- **Presentation**: one page at a time on black, no chrome, end-of-document hint; Esc leaves
  on the current page; transitions and auto-advance from the file are ignored (J-RD-19).
- **Page tint** (Off, Dark, Sepia) is rendered by the engine and applies only to page content;
  the app chrome follows the system appearance (J-RD-20).
- **Magnifier**: a loupe under the pointer at a multiple of the current zoom (J-RD-25).

## 10. Platform mapping

| Concept | macOS (SwiftUI, HIG) | Web (React, MUI) |
|---|---|---|
| Document tabs | Native window tabs; one document per window | An app tab bar (MUI `Tabs`) under the AppBar |
| Main toolbar | Window toolbar (`.toolbar`), customizable | MUI `AppBar` + `Toolbar` |
| Mode switcher | Segmented `Picker` in the toolbar's principal area | MUI `Tabs` / `ToggleButtonGroup` |
| Mode tool row | A secondary toolbar row below the window toolbar | A second MUI `Toolbar` |
| Sidebar | `NavigationSplitView` sidebar column; panel tabs as a segmented control | Persistent MUI `Drawer`; panel `Tabs` |
| Inspector | `.inspector` trailing column | Persistent right MUI `Drawer` |
| Status bar | A bottom bar in the window content | A bottom MUI `Toolbar` (dense) |
| Menus | Menu bar `Commands` | MUI `Menu` buttons in the AppBar |
| Dialogs | Sheets and standard alerts | MUI `Dialog` |
| Transient messages | Inline text in the area concerned, or a non-modal bar inside the window with at most one action (author name, full-screen offer, unsupported action); never system notifications | MUI `Snackbar` or `Alert` bar, with at most one action |
| Context menus | `.contextMenu` | MUI `Menu` at the pointer |

Narrow windows (web < 900 px wide, macOS minimum window size): the inspector collapses
first, then the sidebar becomes an overlay; the document view never drops below a usable width.
Exact breakpoints and spacing are #11's.
