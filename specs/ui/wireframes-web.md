# Wireframes: web (React + MUI)

Status: ready (M0, #11). Structure: [information-architecture.md](information-architecture.md)
(IA). Keys: [shortcuts.md](shortcuts.md). Accessibility: [accessibility.md](accessibility.md).
macOS counterpart: [wireframes-macos.md](wireframes-macos.md) (same screen numbers).

These wireframes fix the layout, the MUI component of each area, the sizes and the tokens. The
audit's vision reviewer scores screenshots against them (AUTOMATION §5.4). They are drawn
in our own words from the IA; nothing is copied from PDFgear (CONSTITUTION 20).

## 0. Rules

1. **Order of authority:** journeys → IA → this file. A wireframe that contradicts the IA or a
   journey is a bug here. Wireframes show *where* things are and *which component* draws
   them; what a control does is in the IA and the journeys.
2. **Strings.** A string a journey quotes appears verbatim (for example "No outline", "11 /
   20", "incorrect password"); only its first letter may be capitalized when it starts a
   sentence or a label. Other labels here are placeholders in our own words and may be
   reworded in a feature spec, as long as they stay short and in sentence case.
3. **Untrusted text** (file names, URLs, metadata, annotation text) is drawn in `«…»` below.
   It is always plain text, escaped by the engine, truncated in the middle except where IA §0
   rule 3 forbids truncation (URLs and hosts).
4. **ASCII conventions:** `[Label]` button, `[▾ Label]` menu button, `(•)` / `( )` radio,
   `[x]` / `[ ]` check box, `[____]` text field, `|` separator, `░` page placeholder, `▓`
   selected, `ⓘ` the reason of a disabled control (tooltip + `aria-describedby`), `…`
   overflow. Icons are written as words; the implementation uses Material Symbols
   (Apache-2.0) bundled with the app, never a CDN (strict CSP).
5. **Disabled controls** use `aria-disabled="true"`, keep focus and show the engine's reason
   (IA §0 rule 2). They are drawn as `{Label ⓘ}`.

## 1. Tokens

### 1.1 Spacing and sizes

| Token | Value | Use |
|---|---|---|
| `spacing` unit | 8 px (MUI `theme.spacing(1)`) | every margin and gap is a multiple of 4 px; prefer 8 |
| `appbar.row` | 48 px (`Toolbar variant="dense"` with 48 px min height) | the AppBar's two rows |
| `toolrow` | 40 px | the mode tool row |
| `statusbar` | 32 px | the status bar |
| `findbar` | 44 px | the find bar |
| `sidebar.width` | 280 px default; min 200 px (fits a small thumbnail plus its label); max ⅓ of the window | left `Drawer` |
| `sidebar.tabs` | 40 px high, icon tabs with labels in tooltips and `aria-label` | panel `Tabs` |
| `inspector.width` | 300 px default; min 260 px; max 400 px | right `Drawer` |
| `docview.min` | 360 px | the document view never gets narrower (§W-29) |
| `target.min` | 32 × 32 px for toolbar and list buttons (`IconButton size="small"` with padding); never below 24 × 24 px (WCAG 2.2 SC 2.5.8) | every interactive element |
| `gap.page` | 16 px between pages; 24 px around the page column | document view |
| `thumb.small` / `thumb.large` | 96 px / 160 px longest side | Thumbnails panel |
| `grid.cell` | 168 px wide (160 px page + 8 px); 16 px gaps | Pages organizer |
| `dialog.width` | `maxWidth="xs"` (444 px) for alerts and the password dialog; `"sm"` (600 px) for forms (print, creators); `"md"` (900 px) for preferences | `Dialog` |
| `radius` | 4 px controls, 8 px dialogs and menus (MUI `shape.borderRadius: 4`) | |
| `elevation` | AppBar 0 with a bottom divider; menus 8; dialogs 24; pages 1 (a 1 px shadow on the backdrop) | |
| breakpoints | MUI defaults: sm 600, md 900, lg 1200 | §W-29 |

### 1.2 Typography

Font: the system stack `system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue",
Arial, sans-serif`; no web-font download (strict CSP, no third-party requests). Base size 14 px
(`theme.typography.fontSize: 14`), rem-based so browser zoom and text size scale it.

| MUI variant | Size / line height / weight | Use |
|---|---|---|
| `h6` | 20 / 28 / 500 | dialog titles, the start screen heading |
| `subtitle2` | 14 / 20 / 600 | panel headers, inspector group headers |
| `body2` | 14 / 20 / 400 | default UI text, lists, fields |
| `caption` | 12 / 16 / 400 | thumbnail labels, status bar, secondary lines in lists |
| `button` | 14 / 20 / 500, sentence case (`textTransform: "none"`) | buttons and tabs |
| monospace (`ui-monospace, Menlo, Consolas, monospace`) 13 px | | escaped untrusted values where control characters are spelled out, the URL in the external-link dialog |

### 1.3 Color (MUI palette; contrast checked by the automated test, accessibility.md §3)

| Token | Light | Dark | Contrast checked |
|---|---|---|---|
| `background.default` (chrome, document backdrop) | `#f5f5f5` | `#121212` | |
| `background.paper` (bars, drawers, dialogs) | `#ffffff` | `#1e1e1e` | |
| `text.primary` | `#1f1f1f` | `#e8e8e8` | 16.5:1 / 13.6:1 on paper |
| `text.secondary` | `#5f5f5f` | `#b0b0b0` | ≥ 5.8:1 / ≥ 7.6:1 on paper and default |
| `primary.main` (selected tab, primary button, current page outline) | `#1565c0` | `#90caf9` | ≥ 5.2:1 / ≥ 9.5:1 on paper and default |
| `primary.contrastText` | `#ffffff` | `#0d1b2a` | 5.75:1 / 9.9:1 |
| `error.main` | `#c62828` | `#f28b82` | ≥ 5.1:1 / ≥ 6.9:1 |
| control boundary (field outlines, dividers that bound a control) | `#8a8a8a` | `#8a8a8a` | ≥ 3.1:1 / ≥ 4.8:1 (SC 1.4.11) |
| focus ring | 2 px `#0b0b0b` inside a 2 px `#ffffff` outline (both appearances) | | ≥ 3:1 against any background (accessibility.md §1) |

Pages are never recolored by the theme; only the page tint (J-RD-20) changes them, through the
engine. Search hits on the page have a fill and a 2 px outline; the current hit uses
`primary.main` for its outline (accessibility.md §3: color is never the only signal).

### 1.4 Motion

Transitions use MUI defaults (`shortest` 150 ms to `standard` 300 ms) and are removed under
`prefers-reduced-motion` (theme `transitions.create` returns `none`). Scrolling to a page or
hit is instant under reduced motion.

## 2. Screens

### W-01 Start screen, with recent files (IA §2; J-RD-01, J-RD-02, J-RD-03)

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ [▾ File] [▾ Edit] [▾ View] [▾ Go] [▾ Tools] [▾ Help]                                 │ AppBar row 1
├──────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                      │
│      jpdf                                                                (h6)        │
│                                                                                      │
│   ┌─────────────────────────────────────────────┐   Create                          │
│   │                                             │   [New blank document]            │
│   │         Drop a PDF here to open it          │   [New from images]               │
│   │                                             │   [Merge files]                   │
│   │                [Open file]                  │                                   │
│   └─────────────────────────────────────────────┘                                   │
│                                                                                      │
│   Recent files                                              [Clear all]              │
│   ┌──────────────────────────────────────────────────────────────────────────────┐   │
│   │ «quarterly-report.pdf»            2.4 MB    Opened today 09:12        [✕]    │   │
│   │ «contract-signed.pdf»             310 KB    Opened yesterday          [✕]    │   │
│   │ «scan-0042…2025.pdf»              18 MB     Opened 12 Nov             [✕]    │   │
│   └──────────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

- Components: the drop target is an outlined `Paper` (2 px dashed control boundary, radius 8)
  that is also the drop zone for the whole window; `Open file` is a `Button
  variant="contained"`. Create is a vertical `Stack` of `Button variant="outlined"`. Recent
  files is a `List` with `ListItemButton` rows (name `body2`, size and date `caption` in
  `text.secondary`) and an `IconButton` "Remove from recent files" per row.
- While a file is dragged over the window the drop target's border turns `primary.main` and
  its text says it will open the file; the rest of the screen is unchanged.
- Max content width 960 px, centred; 48 px top margin; 24 px between blocks.
- Menus without a document: the same groups as W-05 (stable positions); commands that need a
  document are disabled with the reason "no document open".
- A recent file that no longer exists: the row is removed and a `Snackbar` says
  `«name»: file not found` (J-RD-03).

### W-02 Start screen, no recent files (IA §2)

```
│   ┌─────────────────────────────────────────────┐   Create …                        │
│   │         Drop a PDF here to open it          │                                   │
│   │                [Open file]                  │                                   │
│   └─────────────────────────────────────────────┘                                   │
│                                                                                      │
│   Recent files                                                                       │
│   Files you open appear here.                                     (body2, secondary) │
```

When the browser holds no persistent file handles (reader.md shared rules) the sentence instead
explains that this browser doesn't keep access to files between visits, so recent files can't
be listed. Never an empty list box.

### W-03 Start screen, session restore (IA §2; J-RD-24)

```
│   ┌ Alert severity="info" ────────────────────────────────────────────────────────┐  │
│   │ Your last session had 3 open documents.                       [Reopen all]    │  │
│   │ «a.pdf» · «b.pdf» · «c.pdf»                                                   │  │
│   └───────────────────────────────────────────────────────────────────────────────┘  │
│   ┌─ drop target ─┐   Create …                                                        │
```

Shown above the drop target only when "Restore open documents at launch" is on. `Reopen all`
is the user gesture that re-grants file access. Files that no longer exist are listed as
`«name»: file not found` and are skipped.

### W-04 Loading (IA §7; J-RD-01, J-RD-26)

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ [▾ File] [▾ Edit] [▾ View] [▾ Go] [▾ Tools] [▾ Help] │ «big-manual.pdf» ✕ │           │ row 1
│ [Open][Save][Print] │ Read  Annotate  Edit  Pages  Forms  Secure │ [◧][◨] [-][+] …    │ row 2
├──────────────┬───────────────────────────────────────────────────────┬───────────────┤
│ Thumbnails   │                ┌──────────────────┐                   │               │
│ ┌────┐       │                │ page 1 rendered  │                   │               │
│ │ 1  │       │                │                  │                   │               │
│ └────┘       │                └──────────────────┘                   │               │
│ ┌────┐       │                ┌──────────────────┐                   │               │
│ │░░░░│ 2     │                │░░░░░░░░░░░░░░░░░░│  placeholders of  │               │
│ └────┘       │                │░░░░░░░░░░░░░░░░░░│  the right size   │               │
├──────────────┴───────────────────────────────────────────────────────┴───────────────┤
│ — / 812     100% ▾    Single ▾        Loading… ▬▬▬▬▬▭▭▭▭ 38%  [Cancel]               │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

- The tab shows the file name at once. Before page 1 appears the document view shows only
  placeholders (`background.paper` rectangles with a control-boundary outline, no shimmer
  under reduced motion) and the page field shows "—".
- Progress is a determinate `LinearProgress` (indeterminate if the engine gives no total)
  inside the status bar with a `Button size="small"` Cancel. No spinner covers the window.

### W-05 Document open, Read mode (IA §1, §3; J-RD-06…J-RD-10)

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│[Skip to document] (visible only on focus)                                            │
│ [▾ File] [▾ Edit] [▾ View] [▾ Go] [▾ Tools] [▾ Help] │ «report.pdf» ● ✕ │ «b.pdf» ✕ │ │ row 1: menus + document Tabs
│ [Open][Save][Print] │ ▓Read▓ Annotate Edit Pages Forms Secure │ [Select|Hand]          │ row 2: main toolbar
│                       [◧ Sidebar][◨ Inspector] [-][+] [‹ Back][Forward ›] [⛶] [Find] │
├──────────────┬───────────────────────────────────────────────────────────────────────┤
│[▤][≣][✎][📎] │                                                                       │
│[◫][⌕][✓]     │            ┌──────────────────────────┐                               │
│ Thumbnails   │            │                          │                               │
│ ┌────┐       │            │        page 11           │                               │
│ │ 10 │       │            │                          │                               │
│ └────┘       │            │                          │                               │
│ ┌────┐       │            └──────────────────────────┘                               │
│ │▓11▓│ ✓     │            ┌──────────────────────────┐                               │
│ └────┘       │            │        page 12           │                               │
├──────────────┴───────────────────────────────────────────────────────────────────────┤
│ [ 11 / 20 ]    [100% ▾]   [Continuous ▾]                                    Edited   │ status bar
└──────────────────────────────────────────────────────────────────────────────────────┘
```

- Row 1 (`AppBar position="static"`, `background.paper`, bottom divider): menu buttons
  (`Button` + `Menu`, IA §6), then the document `Tabs variant="scrollable"`; each tab has the
  file name (middle-truncated, max 240 px, full escaped name in the tooltip), an 8 px dot when
  edited, and a close `IconButton`. The tab strip is shown even with one document.
- Row 2 (`Toolbar`): file actions; the mode switcher as `Tabs` (labels with icons, 40 px
  high, `primary.main` indicator); then the Read-only Select / Hand `ToggleButtonGroup`; then
  view controls. With less than about 1100 px the view controls after zoom move into a
  `[⋯ More]` menu (§W-29). Inspector toggle is present in every mode.
- Mode tool row: hidden in Read (IA §3).
- Sidebar: persistent `Drawer`, panel `Tabs` with icon tabs (Thumbnails, Outline,
  Annotations, Attachments, Layers, Search results, Signatures), always all seven, in this
  order (IA §4). Under the tabs: the panel title (`subtitle2`) and its actions.
- Thumbnails: current page has a 2 px `primary.main` border; selected pages also show a check
  mark (accessibility.md §3). Page label under each (`caption`).
- Document view: `background.default` backdrop, pages centred, `gap.page` between them. It is
  a focusable `region` named "Document" (shortcuts.md rule 5).
- Status bar (`Toolbar` dense, `background.paper`, top divider, `caption` text): page field
  (`TextField size="small"`, 96 px, shows "11 / 20", or the label plus "(11 of 20)"), zoom
  `Select`, layout `Select`; trailing: job progress (when any) and the edited state.

### W-06 Annotate mode (IA §3; J-ANN-01…J-ANN-26)

```
│ [Open][Save][Print] │ Read ▓Annotate▓ Edit Pages Forms Secure │ [◧][◨] [-][+] …        │
├──────────────────────────────────────────────────────────────────────────────────────┤
│ [Select] | [Highlight][Underline][Strikeout][Squiggly] | [Note][Text box][Callout]    │ tool row
│ [▾ Shapes][Ink][Eraser] | [▾ Stamps][▾ Signature] | [■ color][— width][◐ opacity] [⋯] │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Annotations  │        ┌──────────────────────────┐               │ Highlight         │
│ 12 annot…    │        │  ▓▓▓▓▓▓▓▓ highlighted    │               │ Color   [■ ▾]     │
│ [▾ Sort]     │        │  text                    │               │ Opacity [══●══]   │
│ [▾ Filter]   │        │                          │               │ Author  [Guest__] │
│ Page 3       │        │       [note icon]        │               │ Subject [_______] │
│  ▸ Note «…»  │        │                          │               │ [ ] Locked        │
│  ▸ Highl…    │        └──────────────────────────┘               │ Comment           │
│              │                                                   │ [_______________] │
│              │                                                   │ Replies (2)  …    │
├──────────────┴───────────────────────────────────────────────────┴───────────────────┤
│ [ 3 / 20 ]   [100% ▾]  [Continuous ▾]                                       Edited   │
```

- Tool row: a second `Toolbar` (`toolrow`, `background.paper`, bottom divider). Tools are a
  single-selection `ToggleButtonGroup` (`size="small"`, icon + tooltip with the tool name and
  its key); groups separated by vertical `Divider`s. `Shapes`, `Stamps`, `Signature` are
  split buttons: the button uses the last choice, the arrow opens a `Menu`. The quick
  controls (color, width, opacity) edit the active tool's defaults or the selection. `[⋯]`
  is the More menu (flatten annotations, export / import comments).
- When the row doesn't fit, tools from the end move into the More menu, never wrap to a
  third line; the select tool and the markup group never move.
- The first annotation shows the author notice as a `Snackbar` (W-17).

### W-07 Edit mode (IA §3; J-ED-01…J-ED-10)

```
│ [Select] | [Edit text][Add text][Add image][Link] | [Watermark][Header and footer]    │
│ [Bates numbers][Background] | [Crop]                                                 │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Thumbnails   │     ┌──────────────────────────┐                  │ Text              │
│              │     │  ┌──────────────────┐    │                  │ Font [▾ «Helv…»]  │
│              │     │  │ editing text|    │    │                  │ Size [11 ▾]       │
│              │     │  └──────────────────┘    │                  │ [B][I][U] [■]     │
│              │     └──────────────────────────┘                  │ Align [≡][≡][≡]   │
```

- The text box being edited has a 1 px `primary.main` outline and handles at the corners
  (8 × 8 px visual inside a 24 × 24 px hit area).
- Watermark, header and footer, Bates numbers and background open their creator `Dialog`
  (`sm`, W-25 style) with a live preview; their inspector shows the settings of the open job.
- Missing fonts: the inspector shows the "substituted" note under Font (J-ANN-05).

### W-08 Pages mode (IA §3; J-PG-01…J-PG-16)

```
│ [Insert ▾] [Delete] [Extract] | [Rotate left][Rotate right] | [Duplicate]            │
│ [Move left][Move right] | [Split] [Page labels]                         3 selected   │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Outline      │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐      │ 3 pages           │
│ ▾ Chapter 1  │  │      │ │▓▓▓▓▓▓│ │▓▓▓▓▓▓│ │      │ │      │      │ Size  — mixed     │
│   ▸ 1.1      │  │      │ │▓  ✓ ▓│ │▓  ✓ ▓│ │      │ │      │      │ Rotation 0°       │
│ ▸ Chapter 2  │  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘      │ Label [iii_____]  │
│              │     i        ii      iii       1        2          │                   │
│              │   (1)       (2)      (3)      (4)      (5)         │                   │
│              │  ┌──────┐ ┌──────┐  …                              │                   │
```

- The document view becomes a grid (`grid.cell`), keyboard-navigable as an ARIA grid
  (arrows, Space, Shift+arrows; accessibility.md §1). Under each page: its label and, when it
  differs, the physical number in parentheses (`caption`). Selected pages: 2 px
  `primary.main` border, 12 % `primary.main` fill and a check mark.
- Reordering by drag shows an insertion bar (3 px `primary.main`) between cells; Move
  left/right are the single-pointer alternative (accessibility.md §4). The live region
  announces "moved to position N" (J-PG-06).
- The sidebar keeps the tab's current panel (IA §4); the drawing shows Outline, which is
  the useful companion to the grid.
- Delete opens Confirm delete (W-23).

### W-09 Forms mode (IA §3)

```
│ [Select] | [Text field][Check box][Radio button][List box][Combo box][Button]         │
│ [Signature field] | [▾ Align] [Tab order] | [Flatten]                                │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Thumbnails   │     ┌──────────────────────────┐                  │ Text field        │
│              │     │ Name   ┌────────────┐ ①  │                  │ Name   [«name»__] │
│              │     │        └────────────┘    │                  │ Tooltip [_______] │
│              │     │ Agree  ☐ ②               │                  │ [ ] Required      │
│              │     │                          │                  │ [ ] Read-only     │
│              │     └──────────────────────────┘                  │ Format [▾ None]   │
│              │                                                   │ Calculate [▾ None]│
│              │                                                   │ Appearance …      │
```

- Fields in Forms mode are drawn as objects being designed: a dashed control-boundary outline
  and the field name on hover; the selected field has handles.
- Tab order: when on, each field shows its order number in a 20 px `primary.main` circle
  (①, ②) and the inspector lists the order with Move up / Move down buttons (the
  single-pointer alternative to dragging the badges).
- Format and Calculate offer only the built-in set (no scripts; CONSTITUTION 7).

### W-10 Secure mode (IA §3)

```
│ [Encrypt][Change permissions][Remove security] | [Mark for redaction]                │
│ [Apply redactions] [Scrub metadata] | [Sign][Certify] | [Signatures panel]           │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Signatures   │     ┌──────────────────────────┐                  │ Redaction mark    │
│ No signatures│     │ ████████ (marked)        │                  │ Page 2            │
│              │     │ text ███████ text        │                  │ Overlay text [__] │
│              │     │                          │                  │ [Remove mark]     │
│              │     └──────────────────────────┘                  │                   │
```

- Marked areas are drawn with a 2 px `error.main` outline and a diagonal hatch (not color
  alone) until applied; Apply redactions opens a confirmation (W-23 layout) whose wording
  comes from the redaction journeys.
- The drawing shows the Signatures panel (the sidebar keeps the tab's current panel, IA §4);
  its empty state is "No signatures".

### W-11 Find bar and Search results (IA §1, §4; J-RD-15, J-RD-31)

```
├──────────────┬───────────────────────────────────────────────────────────────────────┤
│ Search res…  │ [⌕ report________] 3 of 9 [‹][›] [Aa][ab|] ▬▬▬▭ [Cancel]       [✕]    │ find bar
│ 9 results    ├───────────────────────────────────────────────────────────────────────┤
│ Page 2       │            ┌──────────────────────────┐                               │
│  …annual «re…│            │  the ╔report╗ shows      │  current hit: primary outline │
│ Page 7       │            │  …   ┌report┐            │  other hits: fill + outline   │
│  …the «repo…»│            └──────────────────────────┘                               │
```

- The find bar is a `Paper` strip above the document view (not over it), `findbar` high:
  `TextField size="small"` 280 px, the count ("1 of 9", `aria-live="polite"`), previous/next
  `IconButton`s, two `ToggleButton`s (match case, whole word), a `LinearProgress` and
  `Cancel` only while a search runs, and a close button.
- Search results panel: before a search a hint to type a query; after a search with no hits
  "No results" in both the panel and the find bar count.

### W-12 Sidebar panels and empty states (IA §4)

```
┌ Outline ──────────────┐  ┌ Annotations ──────────┐  ┌ Attachments ───────────┐
│ Outline        [+][⋯] │  │ 0 annotations         │  │ Attachments            │
│                       │  │                       │  │                        │
│   No outline          │  │   No annotations      │  │   No attachments       │
│   [Add bookmark]      │  │                       │  │                        │
└───────────────────────┘  └───────────────────────┘  └────────────────────────┘
```

- Empty texts are `body2` in `text.secondary`, centred horizontally, 32 px below the panel
  header: "No outline" (with an `Add bookmark` button only when editing is allowed), "No
  annotations", "No attachments", "No layers", "No results", "No signatures".
- Lists: `List dense` with `ListItemButton`; the outline is an ARIA tree (`role="tree"`, for
  example MUI X `SimpleTreeView`, MIT); Annotations groups by page with `ListSubheader`.
- Attachments rows have name, size, description and page; no Open button and no double-click
  action (CONSTITUTION 7).
- Layers rows: `Switch` per group; a locked group shows a lock icon and a disabled switch with
  its reason.

### W-13 Inspector states (IA §5)

```
┌ Nothing selected ─────┐ ┌ Several selected ─────┐ ┌ Locked annotation ────┐
│ New highlights        │ │ 3 annotations         │ │ Square                │
│ Color   [■ ▾]         │ │ Color   [— mixed ▾]   │ │ {Position ⓘ}          │
│ Opacity [══●══]       │ │ Opacity [══●══]       │ │ {Size ⓘ}              │
│ (defaults for the     │ │ Author  [— mixed —]   │ │ Comment [_________]   │
│  active tool)         │ │                       │ │ ⓘ Locked: can't be    │
│                       │ │                       │ │   moved or deleted    │
└───────────────────────┘ └───────────────────────┘ └───────────────────────┘
```

- Groups (`subtitle2` headers, 16 px padding, 12 px between fields, labels above fields).
  Mixed values show "—" with an accessible name ending in "mixed".
- In Read mode the inspector is hidden by default; when shown it is read-only.

### W-14 Open failed (IA §7; J-RD-02, J-RD-05)

```
│                                                                                      │
│  ┌ Alert severity="error" (Snackbar, bottom-centre) ──────────────────────────────┐  │
│  │ ⨯ Couldn't open «notes.txt»: not a PDF.                              [✕]       │  │
│  └────────────────────────────────────────────────────────────────────────────────┘  │
```

- No tab is created; the rest of the window is unchanged. The message uses the journeys'
  terms ("not a PDF", "file too complex", "file not found") and no internal detail. The
  snackbar stays until dismissed (no auto-hide for errors) and is announced through
  `role="alert"`.

### W-15 Document failed after opening (IA §7)

```
├──────────────┬───────────────────────────────────────────────────────────────────────┤
│              │                                                                       │
│              │                     ⨯  This document can't be shown                   │
│              │             Page 1 is too complex to display (file too complex).      │
│              │                             [Close document]                          │
│              │                                                                       │
```

Only that tab shows the error view (icon, `h6` title, `body2` reason, one button), centred in
the document view; sidebar panels show their empty texts; other tabs keep working.

### W-16 Restricted (IA §3 notes, §7; J-RD-29, J-ANN-23, J-PG-15)

```
│ {Select} | {Highlight ⓘ}{Underline ⓘ}{Strikeout ⓘ} … {Ink ⓘ}   [Unlock with password] │
│ ⓘ The document's permissions don't allow adding comments.                            │
```

The disabled tools stay in place and focusable; a one-line reason (`caption`,
`text.secondary`) appears under the tool row once (not per tool), with the "Unlock with
password" button at the end of the row where the journeys allow it. Menu items show the same
reason as their secondary text.

### W-17 Notices (IA §7, §10)

```
│  ┌ Snackbar (bottom-left, above the status bar) ───────────────────────────────────┐ │
│  │ This document contains scripts. They were not run.                     [✕]      │ │
│  └─────────────────────────────────────────────────────────────────────────────────┘ │
```

- One `Snackbar` at a time, at most one action, auto-hide after 8 s except errors and
  notices with an action (they stay until acted on or dismissed; WCAG 2.2 SC 2.2.1).
- Notices: scripts present (once per document), "unsupported link", "unsupported action",
  link target missing, the full-screen offer from `/PageMode /FullScreen` (`[Full screen]`),
  the first-annotation author notice (author "Guest", `[Set name]`), no text under a markup
  drag. Wording beyond the quoted journey terms is our own.

### W-18 Password (IA §8; J-RD-04, J-RD-30)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Password required                              (h6) │
│ «report.pdf» is protected. Enter its password       │
│ to open it.                                         │
│ Password                                            │
│ [••••••••____________________________] [👁]          │
│ ⨯ incorrect password                  (error.main)  │
│                                   [Cancel] [Open]   │
└─────────────────────────────────────────────────────┘
```

- No `<form>`, no username field, `autocomplete="off"`, `type="password"` with a show/hide
  `IconButton`; paste allowed. The error line appears in place (`FormHelperText error`), the
  field keeps focus and is selected. Behind the dialog the tab shows no page content and no
  thumbnails (a neutral backdrop).

### W-19 Owner password unlock (IA §8; J-RD-29)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Unlock restricted actions                           │
│ This document doesn't allow: printing, copying      │
│ text. Enter the owner password to allow them for    │
│ this session.                                       │
│ Owner password [____________________________]       │
│                                 [Cancel] [Unlock]   │
└─────────────────────────────────────────────────────┘
```

### W-20 External link (IA §8; J-RD-17, J-RD-18)

```
┌ Dialog maxWidth="sm" ───────────────────────────────────────────────┐
│ Open this web address?                                              │
│ ┌ monospace, wraps, never truncated ──────────────────────────────┐ │
│ │ https://xn--exmple-cua.com/very/long/path?with=query&and=more…  │ │
│ │ (continues on the next lines, whole URL shown)                   │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│ Host: xn--exmple-cua.com                              (subtitle2)   │
│                                              [Cancel] [Open]        │
└─────────────────────────────────────────────────────────────────────┘
```

Cancel is the default (`autoFocus`). For `mailto:` with attachment parameters a line under
the URL says they were removed.

### W-21 Unsaved changes (IA §8; journey pending in #42)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Save changes?                                       │
│ «report.pdf» has changes that aren't saved. They    │
│ will be lost if you don't save them.                │
│ [Don't save]                    [Cancel] [Save]     │
└─────────────────────────────────────────────────────┘
```

The title is the term reader.md quotes ("save changes?"), so an absence check can't pass
because of different wording; tests also find the dialog by `data-testid="unsaved-changes"`.
`Save` is the default. `Don't save` sits apart on the left (destructive, `color="error"` text
button). Closing the browser tab with edits also triggers the browser's own leave-page prompt
(`beforeunload`), whose wording the app doesn't control.

### W-22 File changed on disk (IA §8; journey pending in #42)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ «report.pdf» changed on disk                        │
│ Another app changed this file. Reload it, or keep   │
│ your version and save it later.                     │
│                         [Reload] [Keep my version]  │
└─────────────────────────────────────────────────────┘
```

`Keep my version` is the default.

### W-23 Confirm delete (IA §8; J-PG-04, J-ANN-19, J-PG-14)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Delete 3 pages?                                     │
│ Pages 2, 3 and 7 will be removed. Earlier versions  │
│ of the file keep them after an incremental save;    │
│ use Save As with "remove earlier versions" to drop  │
│ them.                                               │
│                              [Cancel] [Delete]      │
└─────────────────────────────────────────────────────┘
```

Cancel is the default; Delete is `color="error"`. The other confirmations in IA §8 use the
same layout with their journey wording.

### W-24 Save failed (IA §8; J-OS-06)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Couldn't save «report.pdf»                          │
│ «typed reason in plain words». The original file    │
│ was not changed.                                    │
│                                            [OK]     │
└─────────────────────────────────────────────────────┘
```

### W-25 Print (IA §8; J-RD-21, J-ANN-25)

```
┌ Dialog maxWidth="sm" ──────────────────────────────────────────────────┐
│ Print                                                                  │
│ ┌ preview (engine-rendered, current sheet) ┐  Content                  │
│ │                                           │  (•) document only        │
│ │                                           │  ( ) document and annotations│
│ │                                           │  [ ] Comment contents on  │
│ │                                           │      extra pages          │
│ └───────────────────────────────────────────┘  Pages                    │
│        ‹  Sheet 1 of 2  ›                       (•) All  ( ) [2-4,7___]  │
│                                                Scale [Fit ▾]            │
│                                                Pages per sheet [2 ▾]    │
│                                                [x] Auto-rotate          │
│                                              [Cancel] [Print]           │
└────────────────────────────────────────────────────────────────────────┘
```

Print hands the engine-produced output to the browser's print dialog. Below `sm` the preview
moves above the options.

### W-26 Document properties (IA §8; J-RD-22)

A `Dialog` (`sm`) with `Tabs`: File (name, size, PDF version, page size, page count,
producer, creation and modification dates), Description (title, author, subject, keywords),
Security (encryption, permissions as a list with allowed / not allowed). Values are read-only
`body2`, labels `caption` in `text.secondary`, two columns (label 160 px). Escaped values
use monospace. `[Close]`.

### W-27 Preferences (IA §8)

```
┌ Dialog maxWidth="md" ─────────────────────────────────────────────────┐
│ Preferences                                                           │
│ ┌──────────────┬─────────────────────────────────────────────────────┐ │
│ │ ▓General▓    │ Appearance  (•) Light ( ) Dark ( ) Follow system     │ │
│ │ Viewing      │ [x] Restore open documents at launch                 │ │
│ │ Annotations  │ Author name [Guest____________]                      │ │
│ │ Printing     │                                                      │ │
│ │ Keyboard     │                                                      │ │
│ └──────────────┴─────────────────────────────────────────────────────┘ │
│                                                              [Close]  │
└───────────────────────────────────────────────────────────────────────┘
```

Sections: General (appearance, restore at launch, author name), Viewing (default zoom and
layout), Annotations (alignment guides when dragging), Printing (choose print content each
time), Keyboard (single-key shortcuts on/off). Changes apply at once; no OK button. A
vertical `Tabs` list on the left (180 px).

### W-28 Full screen and presentation (IA §9; J-RD-19)

```
Full screen                                   Presentation
┌──────────────────────────────────────┐      ┌──────────────────────────────────────┐
│ (pointer at top edge → row 2 slides  │      │■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■│
│  down over the page; nothing else)   │      │■■■■■■■ ┌────────────────┐ ■■■■■■■■■■■│
│        ┌──────────────────┐          │      │■■■■■■■ │   one page     │ ■■■■■■■■■■■│
│        │      page        │          │      │■■■■■■■ └────────────────┘ ■■■■■■■■■■■│
│        └──────────────────┘          │      │■■■■■■■■■■ end of document ■■■■■■■■■■■│
└──────────────────────────────────────┘      └──────────────────────────────────────┘
```

Both use the Fullscreen API. Dialogs, menus and snackbars render inside the full-screen
element. Presentation: black (`#000`) background, no chrome; the "end of document" hint is
`caption` text in `#ffffff` at 70 % opacity (≥ 4.5:1 on black) at the bottom centre on the
last page.

### W-29 Narrow windows (IA §10)

| Width | Layout |
|---|---|
| ≥ 1200 px (lg) | Sidebar and inspector docked (persistent `Drawer`s). |
| 900–1199 px | Both docked; view controls after zoom move into `[⋯ More]` in row 2; the tool row moves its tail tools into its More menu. |
| 600–899 px (sm–md) | Inspector becomes a `Drawer variant="temporary"` over the document view, opened by its toggle or by selecting an object; sidebar stays docked at min width. Menu buttons collapse into one `[☰ Menu]` button with the same groups as nested menus. |
| < 600 px (xs) | Sidebar also temporary. The mode `Tabs` become a `Select`. The status bar keeps the page field and zoom; layout moves into the View menu. |

At every width the document view keeps at least `docview.min`; at 200 % browser zoom on a
1280 px window (640 CSS px) every command stays reachable through the menus (accessibility.md
§3).

### W-30 Context menu (IA §6)

```
        ┌ Menu (at pointer) ─────────┐
        │ Copy                        │
        │ Highlight                   │
        │ Underline                   │
        │ Strikeout                   │
        │ Add note to selection       │
        └─────────────────────────────┘
```

`Menu` with `anchorReference="anchorPosition"`, dense items, keys shown as secondary text
only where shortcuts.md defines them; opened from the keyboard it anchors to the focused
object. Items per target are in IA §6.

### W-31 Edited state (IA §7; J-OS-01)

The tab shows an 8 px dot before the close button (the dot has `aria-label` "edited"; the
close button's name becomes "Close «name», edited"); `document.title` becomes
`• «name» — jpdf`; the status bar shows "Edited" (`caption`). All three clear after Save.

## 3. Reviewer checklist (AUTOMATION §5.4)

The audit's vision reviewer checks each screenshot for:
1. The areas of W-05 in the right order and the component named here (AppBar rows, Tabs,
   Drawers, status bar).
2. Sizes within ±2 px of §1.1 at 100 % zoom; all gaps on the 4 px grid; text in the §1.2
   variants.
3. Every empty, loading and error state shows text, as in W-02, W-04, W-12, W-14, W-15.
4. Contrast and focus ring as in §1.3, in both appearances; page content never recolored by
   the theme.
5. Disabled controls visible with their reason (W-16); dialogs with the default button
   named here.
6. Untrusted text plain, escaped, and middle-truncated (URLs never truncated).
