# Wireframes: macOS (SwiftUI, Apple HIG)

Status: ready (M0, #11). Structure: [information-architecture.md](information-architecture.md)
(IA). Keys: [shortcuts.md](shortcuts.md). Accessibility: [accessibility.md](accessibility.md).
Web counterpart: [wireframes-web.md](wireframes-web.md); screen numbers (W-NN) match.

These wireframes fix the layout, the SwiftUI/AppKit idiom of each area, the sizes and the
tokens. The audit's vision reviewer scores screenshots against them and the Apple HIG
(AUTOMATION §5.4). They are drawn in our own words from the IA; nothing is copied from
PDFgear (CONSTITUTION 20).

## 0. Rules

1. **Order of authority:** journeys → IA → Apple HIG → this file. Where the HIG and this file
   disagree on a system control's look, the system control wins (use it unstyled).
2. **Strings:** the same rules as [wireframes-web.md](wireframes-web.md) §0 rules 2 and 3
   (journey strings verbatim, first letter may be capitalized; untrusted text in `«…»`,
   plain and escaped). macOS menu items and buttons use title case where the HIG asks for it
   (menu items, push buttons: "Keep My Version"); a journey string inside such a label keeps
   its words. Ellipsis (…) ends every command that opens a dialog or a sheet.
3. **System first.** Standard behaviors come from the system and are not redrawn: window
   tabs, full screen, the Open Recent menu, the print panel, the save panel, the
   unsaved-changes alert of the document architecture, the toolbar customization sheet,
   Settings window, focus ring, Increase Contrast, Reduce Motion, Reduce Transparency.
4. **Logic stays in the engine** (CONSTITUTION 6): the views only draw engine results; the
   PDF engine runs in the XPC helper.
5. ASCII conventions are the web file's §0 rule 4; `{Label ⓘ}` is a disabled control whose
   help tag and accessibility hint carry the engine's reason (IA §0 rule 2).

## 1. Tokens

### 1.1 Spacing and sizes (points)

| Token | Value | Use |
|---|---|---|
| grid | 4 pt; prefer 8 | every margin and gap |
| `window.min` | 640 × 480 | `.frame(minWidth:minHeight:)` on the document window |
| `window.default` | 1200 × 800, or 85 % of the screen if smaller; then the system restores the last frame | first open |
| toolbar | system unified toolbar (`.windowToolbarStyle(.unified)`) | main toolbar |
| `toolrow` | 36 | the mode tool row (a bar under the toolbar) |
| `findbar` | 32 | the find bar |
| `statusbar` | 28 | the status bar |
| `sidebar.width` | ideal 240, min 180, max ⅓ of the window (`navigationSplitViewColumnWidth(min:ideal:max:)`) | sidebar column |
| `inspector.width` | ideal 270, min 240, max 360 (`inspectorColumnWidth(min:ideal:max:)`) | inspector |
| `docview.min` | 320 | the document view never gets narrower (§W-29) |
| `target.min` | 28 × 28 default, never below 20 × 20 (HIG macOS hit targets) | every control |
| `gap.page` | 12 between pages; 20 around the page column | document view |
| `thumb.small` / `thumb.large` | 80 / 140 longest side | Thumbnails panel |
| `grid.cell` | 152 wide (140 page + 12); 16 gaps | Pages organizer |
| sheet margins | 20 on every side; 8 between related controls; 20 between groups | sheets, alerts, Settings |
| control size | `.regular` in sheets and the inspector; `.small` in the tool row, status bar, find bar and sidebar headers | |
| corner radius | system controls' own; page thumbnails 4; drop target 12 | |

### 1.2 Typography

System font (SF Pro) through SwiftUI text styles only, so the system text-size setting
applies where SwiftUI supports it. No custom fonts in the chrome.

| Text style | macOS size / weight | Use |
|---|---|---|
| `.title2` | 17 / regular | start window heading, error view title |
| `.headline` | 13 / semibold | sheet titles, inspector group headers, panel headers |
| `.body` | 13 / regular | default UI text, lists, fields |
| `.callout` | 12 / regular | secondary lines in lists, the tool-row reason line |
| `.caption` | 10 / regular | thumbnail labels |
| `.body.monospaced()` (SF Mono) | 13 | escaped untrusted values with spelled-out control characters, the URL in the external-link sheet |
| status bar | `.callout`, `.monospacedDigit()` for the page field and zoom | |

### 1.3 Color and materials

Only semantic system colors, so light / dark appearance and Increase Contrast work without
custom values (accessibility.md §3). No hex values in the macOS app.

| Token | System value | Use |
|---|---|---|
| accent | `Color.accentColor` (the user's system accent) | selected mode segment, current page border, current search hit outline, insertion bar |
| text | `.primary`, `.secondary`, `.tertiary` (`.tertiary` only for placeholder text) | |
| document backdrop | `NSColor.underPageBackgroundColor` | behind the pages |
| window background | `NSColor.windowBackgroundColor` | start window, status bar |
| separators | `NSColor.separatorColor` (`Divider()`) | between bars |
| destructive / error | `.red` (`NSColor.systemRed`) and `role: .destructive` buttons | redaction outlines, error icons, Delete |
| selection | `NSColor.selectedContentBackgroundColor` / `unemphasizedSelectedContentBackgroundColor` | lists, thumbnails |
| sidebar | the system sidebar material of `NavigationSplitView` | |
| tool row, find bar | `.bar` material | |

Reduce Transparency makes the materials opaque automatically. Pages are never recolored by
the appearance; only the page tint (J-RD-20) changes them, through the engine. Search hits
have a fill and an outline (not color alone).

### 1.4 Motion

System animations only (sidebar and inspector show/hide, sheets, full screen). With Reduce
Motion on: no animated scrolling to a page or hit, and no custom transitions
(`accessibilityReduceMotion`).

### 1.5 Symbols

SF Symbols with `.help(…)` tooltips and accessibility labels, for example: `sidebar.left`,
`sidebar.right`, `minus.magnifyingglass`, `plus.magnifyingglass`, `printer`, `folder`,
`square.and.arrow.down`, `magnifyingglass`, `highlighter`, `underline`, `strikethrough`,
`note.text`, `character.textbox`, `scribble`, `eraser`, `seal`, `signature`,
`rotate.left`, `rotate.right`, `trash`, `lock`, `square.grid.2x2` (thumbnails),
`list.bullet.indent` (outline), `text.bubble` (annotations), `paperclip`,
`square.3.layers.3d` (layers), `checkmark.seal` (signatures). A feature spec may pick
another symbol; it must be an SF Symbol, never a copied icon.

## 2. Screens

### W-01 Start window, with recent files (IA §2; J-RD-01, J-RD-02, J-RD-03)

```
┌─●─●─●───────────────────────── jpdf ─────────────────────────────────────┐
│                                                                          │
│   jpdf                                     (.title2)                     │
│                                                                          │
│   ╭─────────────────────────────────────╮   [New Blank Document]         │
│   │                                     │   [New from Images…]           │
│   │     Drop a PDF here to open it      │   [Merge Files…]               │
│   │            [Open…]                  │                                │
│   ╰─────────────────────────────────────╯                                │
│                                                                          │
│   Recent                                                                 │
│   ┌──────────────────────────────────────────────────────────────────┐   │
│   │ 📄 «quarterly-report.pdf»     2.4 MB   Today, 09:12               │   │
│   │ 📄 «contract-signed.pdf»      310 KB   Yesterday                  │   │
│   └──────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

- A single `Window` scene (fixed 720 × 480), shown at launch when no document window is open
  and after the last one closes. Title bar with the app name; no toolbar.
- The drop target is a rounded rectangle (radius 12, dashed `separatorColor` stroke) that
  highlights with the accent color while a file is dragged over it. `Open…` is the default
  button (`.keyboardShortcut(.defaultAction)`, `.borderedProminent`).
- Create buttons are `.bordered`, stacked, 8 apart.
- Recent: a `List` with file icon (`NSWorkspace` icon for the type), name, size and date
  (`.secondary`); double-click or Return opens; context menu Remove from Recent; Clear Menu is
  in File > Open Recent (system). The same files appear in File > Open Recent.
- A recent file that no longer exists: the row is removed and an inline line under the list
  says `«name»: file not found` (J-RD-03).

### W-02 Start window, no recent files (IA §2)

The Recent list is replaced by one `.secondary` line, "Files you open appear here.", 20 below
the drop target. Never an empty list.

### W-03 Session restore (IA §2; J-RD-24)

macOS reopens the last session's documents directly when "Restore open documents at launch"
is on (window restoration); the start window is not shown. A file that no longer exists is
skipped, and the first restored window shows a notice bar (W-17) `«name»: file not found`.

### W-04 Loading (IA §7; J-RD-01, J-RD-26)

```
┌─●─●─●─ [◧] «big-manual.pdf»     ⟨Read|Annotate|Edit|Pages|Forms|Secure⟩   [−][+] [⌕] [◨]┐
├─────────────┬────────────────────────────────────────────────────────────────────────────┤
│ ⟨▦|≣|💬|📎|◫|⌕|✓⟩│              ┌──────────────────┐                                        │
│ ┌────┐      │              │ page 1           │                                        │
│ │ 1  │      │              └──────────────────┘                                        │
│ ┌────┐      │              ┌──────────────────┐                                        │
│ │░░░░│      │              │░░░░░░░░░░░░░░░░░░│  placeholders of the right size        │
├─────────────┴────────────────────────────────────────────────────────────────────────────┤
│ — / 812    100% ⌄   Single Page ⌄                  Loading…  ▬▬▬▬▭▭▭▭  38%  [Cancel]       │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

The window opens at once with the file name in the title. Progress is a `ProgressView(value:)`
(`.linear`, `.small`) in the status bar with a small `Cancel` button. Placeholders are
`controlBackgroundColor` rectangles with a separator stroke. No spinner over the window.

### W-05 Document open, Read mode (IA §1, §3; J-RD-06…J-RD-10)

```
┌─●─●─●─ [◧] «report.pdf» — Edited                                                         ┐
│  [📂][⬇][🖨]   ⟨▓Read▓|Annotate|Edit|Pages|Forms|Secure⟩   ⟨Select|Hand⟩  [−][+] [‹][›] [⌕] [◨]│ unified toolbar
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ «report.pdf»                         │ «b.pdf»                                    [+]    │ native tab bar (2+ docs)
├─────────────┬────────────────────────────────────────────────────────────────────────────┤
│⟨▦|≣|💬|📎|◫|⌕|✓⟩│                                                                            │
│ Thumbnails  │              ┌──────────────────────────┐                                  │
│ ┌────┐      │              │                          │                                  │
│ │ 10 │      │              │         page 11          │                                  │
│ └────┘      │              │                          │                                  │
│ ┌────┐      │              └──────────────────────────┘                                  │
│ │▓11▓│      │              ┌──────────────────────────┐                                  │
│ └────┘      │              │         page 12          │                                  │
├─────────────┴────────────────────────────────────────────────────────────────────────────┤
│ [ 11 / 20 ]   100% ⌄   Continuous ⌄                                                      │ status bar
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

- Window: a document window per file (native tabs merge them; the tab bar appears with two or
  more tabs, the system default). The title is the file name (middle-truncated by the system),
  with "Edited" added by the system when there are unsaved changes; the proxy icon offers the
  path menu.
- Toolbar (`.toolbar`, customizable, default set as drawn): leading the sidebar toggle;
  Open, Save, Print; principal the mode `Picker` (`.segmented`, labels with symbols); the Read
  mode's Select / Hand `Picker`; zoom out / in; back / forward; Find (shows the find bar);
  trailing the inspector toggle. Full screen is the system's (green button, View menu).
- Sidebar: `NavigationSplitView` sidebar column; at its top a segmented `Picker` of seven
  symbol-only segments (Thumbnails, Outline, Annotations, Attachments, Layers, Search results,
  Signatures; names in help tags and accessibility labels), in IA §4 order; under it the
  panel header (`.headline`) and its small action buttons.
- Thumbnails: the current page has a 3 pt accent border; selected pages use the selection
  color plus a check mark badge; labels in `.caption` under each.
- Document view: an AppKit-hosted scroll view (`NSViewRepresentable`) drawing engine tiles on
  `underPageBackgroundColor`; pages have a soft system shadow. It is one accessibility
  element group per page ("Page N of M") with the engine's text layer (accessibility.md §2).
- Status bar: a 28 pt bar in the window content (`windowBackgroundColor`, top `Divider`),
  `.small` controls: page field (`TextField`, 80 wide, "11 / 20" or the label plus "(11 of
  20)"), zoom pop-up, layout pop-up; trailing job progress and Cancel when a job runs.

### W-06 Annotate mode (IA §3; J-ANN-01…J-ANN-26)

```
│  [📂][⬇][🖨]   ⟨Read|▓Annotate▓|Edit|Pages|Forms|Secure⟩               [−][+] [‹][›] [⌕] [◨]│
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ [↖] | [🖍][U̲][S̶][~] | [🗒][T][💬] | [◻ ⌄][✎][⌫] | [⊛ ⌄][✍ ⌄] | [● ⌄][— ⌄][◐ ⌄]      [⋯]   │ tool row
├─────────────┬──────────────────────────────────────────────────────┬────────────────────┤
│ Annotations │          ┌──────────────────────────┐                │ Highlight          │
│ 12 annot…   │          │ ▓▓▓▓▓▓▓▓ highlighted     │                │ Color    [■ ⌄]     │
│ [Sort ⌄]    │          │ text                     │                │ Opacity  ──●──     │
│ [Filter ⌄]  │          │      [note icon]         │                │ Author   [Guest  ] │
│ ▾ Page 3    │          └──────────────────────────┘                │ Subject  [       ] │
│   Note «…»  │                                                      │ ☐ Locked           │
│   Highli…   │                                                      │ ▾ Comment          │
│             │                                                      │ [                ] │
├─────────────┴──────────────────────────────────────────────────────┴────────────────────┤
│ [ 3 / 20 ]   100% ⌄   Continuous ⌄                                                       │
```

- Tool row: a bar under the toolbar (`.bar` material, `toolrow` high, bottom `Divider`),
  `.small` symbol buttons in a single-selection group (selected tool shows the accent tint),
  groups separated by `Divider`s; Shapes, Stamps, Signature are pull-down buttons
  (`Menu` with a primary action). Quick controls: color well pop-up, width and opacity
  pop-ups. `[⋯]` is the More menu. Every tool also appears in Tools menu with its key.
- When the row doesn't fit, its tail tools move into `[⋯]`; the row never wraps.
- Inspector: `.inspector` column; `Form` with `.formStyle(.grouped)` sections, labels leading.

### W-07 Edit mode (IA §3; J-ED-01…J-ED-10)

```
│ [↖] | [Edit Text][Add Text][Add Image][Link] | [Watermark ⌄][Header & Footer ⌄]          │
│       [Bates ⌄][Background ⌄] | [Crop]                       (row overflows into [⋯])   │
```

The row is drawn here wrapped only for width; on screen it is one row and its tail moves into
`[⋯]`. Text editing uses the engine's caret and selection drawn by the view; Format menu
(Bold, Italic, Underline) is enabled while editing. Watermark, header and footer, Bates and
background open their creator sheets with a live preview. Missing fonts show the
"substituted" note in the inspector.

### W-08 Pages mode (IA §3; J-PG-01…J-PG-16)

```
│ [Insert ⌄] [🗑] [Extract…] | [↺][↻] | [Duplicate] [◀ Move][Move ▶] | [Split…] [Page Labels…] │
├─────────────┬──────────────────────────────────────────────────────┬────────────────────┤
│ Outline     │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐              │ 3 Pages            │
│ ▾ Chapter 1 │  │     │ │▓▓▓▓▓│ │▓▓▓▓▓│ │     │ │     │              │ Size     — mixed   │
│   ▸ 1.1     │  │     │ │▓ ✓ ▓│ │▓ ✓ ▓│ │     │ │     │              │ Rotation 0°        │
│ ▸ Chapter 2 │  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘              │ Label    [iii    ] │
│             │     i      ii     iii      1       2                 │                    │
│             │    (1)     (2)    (3)     (4)     (5)                │                    │
```

- The document view becomes a `LazyVGrid` of `grid.cell` items with selection (click,
  Shift-click, Cmd-click; arrows and Space with Full Keyboard Access). Selected pages: the
  selection color behind the page, 3 pt accent border and a check mark badge.
- Drag reordering shows a 3 pt accent insertion bar; Move ◀ / ▶ are the single-pointer
  alternative; VoiceOver announces "moved to position N" (J-PG-06).
- The sidebar keeps the window's current panel (IA §4); Delete asks first (W-23).

### W-09 Forms mode (IA §3)

```
│ [↖] | [Text Field][Check Box][Radio Button][List Box][Combo Box][Button][Signature Field] │
│       | [Align ⌄] [Tab Order] | [Flatten…]                    (one row; tail into [⋯])   │
```

Fields are drawn as objects being designed (dashed separator outline, name on hover, handles
when selected). Tab Order shows numbered accent badges on the fields and an ordered list in
the inspector with Move Up / Move Down buttons. Format and Calculate pop-ups list only the
built-in set (no scripts; CONSTITUTION 7).

### W-10 Secure mode (IA §3)

```
│ [Encrypt…][Permissions…][Remove Security…] | [Mark for Redaction][Apply Redactions…]      │
│       [Scrub Metadata…] | [Sign…][Certify…] | [Signatures]     (one row; tail into [⋯])   │
```

Redaction marks: a 2 pt `systemRed` outline with a diagonal hatch until applied. Apply
Redactions… shows a confirmation alert (W-23) with the redaction journeys' wording. The Signatures
panel's empty state is "No signatures".

### W-11 Find bar and Search results (IA §1, §4; J-RD-15, J-RD-31)

```
├─────────────┬────────────────────────────────────────────────────────────────────────────┤
│ Search Res… │ [⌕ report              ⓧ]  3 of 9  [‹][›]  [Aa] [Whole Word]  ▬▬▭ [Cancel] [Done]│ find bar
│ 9 results   ├────────────────────────────────────────────────────────────────────────────┤
│ ▾ Page 2    │              ┌──────────────────────────┐                                  │
│   …«report» │              │ the ╔report╗ shows       │                                  │
```

The find bar is a `.bar` strip at the top of the document view (like the system find bar): a
search field (220 wide), the count ("1 of 9", posted as an accessibility announcement),
previous / next, match case and whole word toggles, a small progress bar and Cancel only while
searching, and Done to close. Search results panel: before a search a hint; after one "No
results".

### W-12 Sidebar panels and empty states (IA §4)

Empty texts are centred `.secondary` `.body` text 24 pt below the header (SwiftUI
`ContentUnavailableView` is too large for a 240 pt column; use a plain `Text`): "No outline"
(with an Add Bookmark button when editing is allowed), "No annotations", "No attachments",
"No layers", "No results", "No signatures". The outline is an `OutlineGroup` / `List` with
disclosure triangles; Annotations is a `List` with page sections; Attachments rows show name,
size, description and page and have no Open action, and double-click does nothing
(CONSTITUTION 7); Layers rows have a `Toggle` (`.checkbox` style), locked groups a lock symbol
and a disabled toggle with the reason.

### W-13 Inspector states (IA §5)

Same three states as the web file's W-13 (nothing selected → defaults of the active tool;
several → shared properties with "Mixed" shown as the placeholder of the field; locked →
disabled controls with the reason as help tag and one `.callout` line explaining the lock).
Read mode: hidden by default, read-only when shown.

### W-14 Open failed (IA §7; J-RD-02, J-RD-05)

```
            ┌ alert (sheet on the start window, or app-modal if no window) ┐
            │   ⚠                                                          │
            │   “«notes.txt»” couldn't be opened.          (.headline)     │
            │   It is not a PDF.                                           │
            │                                            [OK]              │
            └──────────────────────────────────────────────────────────────┘
```

No document window is created. The informative text names the engine's typed error in the
journeys' terms ("not a PDF", "file too complex", "file not found"), without internal detail.

### W-15 Document failed after opening (IA §7)

The document view shows a centred error view: `exclamationmark.triangle` symbol (40 pt,
`.secondary`), title (`.title2`) "This document can't be shown", reason (`.body`, `.secondary`)
in the journeys' terms ("file too complex"), and a `Close Document` button. Other windows keep
working.

### W-16 Restricted (IA §3 notes, §7; J-RD-29, J-ANN-23, J-PG-15)

The tool row keeps the disabled tools (dimmed, still reachable with Full Keyboard Access);
a `.callout` line under the row states the reason once, with an `Unlock with Password…`
button at its end where the journeys allow it. Menu items are disabled and their help tag
gives the reason.

### W-17 Notices (IA §7, §10)

```
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ ⓘ This document contains scripts. They were not run.                               [✕]   │ notice bar
├──────────────────────────────────────────────────────────────────────────────────────────┤
```

A non-modal bar (`.bar` material, 32 pt) under the tool row, inside the window, one at a time
with at most one action button; never a system notification. Same notices as the web file's
W-17. Each is posted as an accessibility announcement.

### W-18 Password (IA §8; J-RD-04, J-RD-30)

```
┌ sheet ───────────────────────────────────────────────┐
│ 🔒  “«report.pdf»” is password protected.  (.headline) │
│     Enter the password to open it.                    │
│     Password: [••••••••                          ]    │
│     ⚠ incorrect password                  (.red)      │
│                               [Cancel] [Open]         │
└──────────────────────────────────────────────────────┘
```

A `SecureField` with AutoFill off (`.textContentType(nil)`), paste allowed; the error appears
in place, the field keeps focus with its text selected. Open is the default button. The window
behind shows no page content or thumbnails (the sidebar and document view are empty
`underPageBackgroundColor`). Cancel closes the window.

### W-19 Owner password unlock (IA §8; J-RD-29)

A sheet like W-18: title "Unlock Restricted Actions", a line listing the restricted
permissions in plain words, an owner-password `SecureField`, `[Cancel] [Unlock]` (Unlock is
the default). The unlock lasts for this session only.

### W-20 External link (IA §8; J-RD-17, J-RD-18)

A sheet 480 wide: title "Open This Web Address?", the engine-normalized URL in a selectable
monospaced `Text` that wraps and is never truncated, the host on its own line
(`.headline`), and `[Cancel] [Open]` with **Cancel** as the default button
(`.keyboardShortcut(.defaultAction)` on Cancel). A `mailto:` with attachment parameters adds
a line that they were removed.

### W-21 Unsaved changes (IA §8; journey pending in #42)

```
┌ alert (sheet) ─────────────────────────────────────────┐
│  Do you want to save the changes made to the document  │
│  “«report.pdf»”?                                       │
│  Your changes will be lost if you don't save them.     │
│  [Don't Save]                       [Cancel] [Save]    │
└────────────────────────────────────────────────────────┘
```

The document architecture's standard alert (system wording, which differs from the journeys'
"save changes?" term). Tests therefore find it by its accessibility identifier
`unsaved-changes`, never by text, so an absence check can't pass because of wording. On quit
with several edited documents the system's review-changes flow applies.

### W-22 File changed on disk (IA §8; journey pending in #42)

An alert sheet: "“«report.pdf»” was changed by another app." / "Reload it, or keep your
version and save it later." / `[Reload] [Keep My Version]`, Keep My Version the default.

### W-23 Confirm delete and other confirmations (IA §8)

Alert sheets (`.confirmationDialog` / `.alert`) with the journeys' wording, a
`role: .destructive` action (Delete, Flatten, Remove) and Cancel as the default
(`role: .cancel`). Example: "Delete 3 pages?" / the pages and the earlier-versions note with
"remove earlier versions" (as in the web file's W-23) / `[Cancel] [Delete]`.

### W-24 Save failed (IA §8; J-OS-06)

Alert sheet: "“«report.pdf»” couldn't be saved." / the typed reason in plain words and "The
original file was not changed." / `[OK]`.

### W-25 Print (IA §8; J-RD-21, J-ANN-25)

The system print panel (`NSPrintOperation` with the engine-rendered pages) with an accessory
view (`NSPrintPanelAccessorizing`) holding our options: Content pop-up ("document only",
"document and annotations"), a Comment contents on extra pages checkbox, and Auto-rotate.
Pages, scale and pages per sheet are the system panel's own controls, read back into the
engine's print job. The accessory view has 20 pt margins and `.regular` controls.

### W-26 Document properties (IA §8; J-RD-22)

File > Document Properties… opens a sheet 520 wide with a segmented `Picker` (File,
Description, Security) and a two-column `Grid` (labels trailing-aligned `.secondary`, values
`.body`, selectable). `[Done]`.

### W-27 Settings (IA §8)

The app's `Settings` scene (App menu > Settings…, the current HIG name for the IA's
Preferences) with a toolbar-style `TabView`: General (appearance Light / Dark / Follow
system, "Restore open documents at launch", author name), Viewing (default zoom and layout),
Annotations (alignment guides), Printing (choose print content each time), Keyboard
(single-key shortcuts). `Form` with `.formStyle(.grouped)`, fixed width 520, changes apply
at once.

### W-28 Full screen and presentation (IA §9; J-RD-19)

- Full screen: the system full screen; the toolbar auto-hides and reappears when the pointer
  reaches the top edge; sidebar and inspector are collapsed while it lasts and restored after.
- Presentation: a separate borderless window on the current screen, black background, one
  page fitted, no chrome; the "end of document" hint in white `.callout` at 70 % opacity at
  the bottom centre on the last page; the pointer hides after 2 s without movement.

### W-29 Narrow windows (IA §10)

| Window width | Layout |
|---|---|
| ≥ 1040 pt | Sidebar and inspector shown as columns. |
| 800–1039 pt | The inspector hides when the window shrinks into this range (its toggle reopens it as a column, taking width from the document view); toolbar items that don't fit go to the system overflow (»). |
| 640–799 pt | The sidebar collapses (`NavigationSplitViewVisibility.detailOnly`); its toggle shows it over the document view. |

The document view keeps at least `docview.min`. The window can't be smaller than
`window.min`.

### W-30 Context menu (IA §6)

`.contextMenu` / `NSMenu` built from the engine's hit test at the pointer (Control-click too),
standard menu typography, key equivalents shown where shortcuts.md defines them. Items per
target are in IA §6; Document tab's menu adds Show in Finder.

### W-31 Edited state (IA §7; J-OS-01)

The system handles it: the close button shows the edited dot and the title adds "Edited"
(`NSDocument` / `isDocumentEdited`). Both clear after Save.

## 3. Reviewer checklist (AUTOMATION §5.4)

1. Areas of W-05 present, with the system idioms named here (unified toolbar, segmented mode
   picker, split-view sidebar, inspector column, sheets).
2. Sizes within ±2 pt of §1.1; text only in the §1.2 styles; no custom colors (§1.3).
3. Every empty, loading and error state shows text (W-02, W-04, W-12, W-14, W-15).
4. Both appearances, Increase Contrast and Reduce Transparency screenshots look correct; page
   content never recolored by the appearance.
5. Disabled controls visible with their reason (W-16); sheets with the default button named
   here; Cancel present on every sheet.
6. Untrusted text plain, escaped, middle-truncated (URLs never truncated).
