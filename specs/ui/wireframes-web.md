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
3. **Untrusted text** is everything that comes from a file: file names, the document title
   (J-RD-27), URLs and hosts, metadata, annotation text and authors, page labels, outline
   titles, layer names, attachment names, field names, signer names. It is drawn in `«…»`
   below, even where the drawing shows a plain example. It is always plain text, the engine's
   escaped display string, truncated in the middle (start and end visible) with the full
   escaped value in the tooltip / accessible description, except where IA §0 rule 3 forbids
   truncation (URLs and hosts wrap instead).
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
| `statusbar` | 32 px; its fields are dense `InputBase` inputs 28 px high (MUI's `size="small"` outlined fields are 40 px and don't fit) | the status bar |
| `findbar` | 44 px | the find bar |
| `sidebar.width` | 280 px default; min 200 px (fits a small thumbnail plus its label); max ⅓ of the window | left `Drawer` |
| `sidebar.tabs` | one row, 40 px high; seven icon-only `Tab`s with `minWidth: 36` px (7 × 36 = 252 px fits the 280 px default); narrower than that the strip scrolls (`variant="scrollable"`); names in tooltips and `aria-label` | panel `Tabs` |
| `inspector.width` | 300 px default; min 260 px; max 400 px | right `Drawer` |
| `docview.min` | 360 px | the drawers shrink to their minimum widths and then the breakpoints of W-29 apply before the document view gets narrower; below 600 px viewport width the document view takes the full width (pages may scroll in two dimensions, the SC 1.4.10 exception for content that needs it) |
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

| `action.selected` (selected tool, selected list row) | `primary.main` at 16 % with a 2 px `primary.main` bottom bar on tools | `primary.main` at 24 % with the same bar | the bar against the tinted button is 4.55:1 / 5.46:1 (≥ 3:1), so the state never relies on the tint alone (SC 1.4.11) |
| `action.disabled` (disabled icon and label) | `#5f5f5f` at 60 % | `#b0b0b0` at 60 % | disabled controls are exempt from contrast; they also carry the reason (W-16) |
| `info.main` / `warning.main` / `success.main` (Alert icons and borders) | `#0b6bcb` / `#9a5b00` / `#2e7d32` | `#7cc4fa` / `#ffb74d` / `#81c784` | ≥ 4.5:1 on paper and default; Alert text uses `text.primary` |

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
│ [▾ File] [▾ Edit] [▾ View] [▾ Go] [▾ Tools] [▾ Help]                                 │ main toolbar (no tab bar)
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

Shown only when there are no recent files at all; never an empty list box. When the browser
can't keep file handles (the no-handles case of J-RD-03), the W-01 list still shows the names
only (no size), and clicking an entry opens the file picker instead of the file; a `caption`
line under the list says so.

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
│ «big-manual.pdf» [✕]                                                                 │ tab bar
│ [▾ File]…[▾ Help] │ [Open][Save][Print] │ Read Annotate Edit Pages Forms Secure │ …   │ main toolbar
├──────────────┬───────────────────────────────────────────────────────┬───────────────┤
│ Thumbnails   │                ┌──────────────────┐                   │               │
│ ┌────┐       │                │ page 1 rendered  │                   │               │
│ │ 1  │       │                │                  │                   │               │
│ └────┘       │                └──────────────────┘                   │               │
│ ┌────┐       │                ┌──────────────────┐                   │               │
│ │░░░░│ 2     │                │░░░░░░░░░░░░░░░░░░│  placeholders of  │               │
│ └────┘       │                │░░░░░░░░░░░░░░░░░░│  the right size   │               │
├──────────────┴───────────────────────────────────────────────────────┴───────────────┤
│ — / 812     100% ▾    Single Page ▾   Loading… ▬▬▬▬▬▭▭▭▭ 38%  [Cancel]               │
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
│ «report.pdf» ● [✕] │ «b.pdf» [✕] │                                                   │ row 1: document tab bar
│ [▾ File][▾ Edit][▾ View][▾ Go][▾ Tools][▾ Help] | [Open][Save][Print] | [↶][↷]        │ row 2: main toolbar
│ | ▓Read▓ Annotate Edit Pages Forms Secure | [Select|Hand] | [◧][◨] [-][+] [‹][›]      │   (one row on screen;
│   [⟲ view][⟳ view] [⛶ Full screen][▶ Presentation] [Find]                            │    wrapped here for width)
├──────────────┬───────────────────────────────────────────────────────────────────────┤
│▤ ≣ ✎ 📎 ◫ ⌕ ✓│            ┌──────────────────────────┐                               │
│ Thumbnails   │            │                          │                               │
│ ┌────┐       │            │        page 11           │                               │
│ │ 10 │       │            │                          │                               │
│ └────┘       │            │                          │                               │
│ ┌────┐       │            └──────────────────────────┘                               │
│ │▓11▓│ ✓     │            ┌──────────────────────────┐                               │
│ └────┘       │            │        page 12           │                               │
├──────────────┴───────────────────────────────────────────────────────────────────────┤
│ [ 11 / 20 ]    [100% ▾]   [Single Page Continuous ▾]                        Edited   │ status bar
└──────────────────────────────────────────────────────────────────────────────────────┘
```

- Both rows sit in one `AppBar position="static"` (`background.paper`, bottom divider), in
  the focus order of accessibility.md §1 (tabs → main toolbar).
- Row 1, the document tab bar (`Tabs variant="scrollable"`): each tab shows the file name, or
  the document title when the file sets `/DisplayDocTitle` (J-RD-27; the file name stays in
  the tooltip), middle-truncated at 240 px, with an 8 px dot when edited. The close button is
  an `IconButton` placed **next to** the `Tab` element, not inside it (`role="tab"` can't
  contain a focusable control; axe `nested-interactive`); the tab bar is shown even with one document.
- Row 2, the main toolbar (`Toolbar`): the menu buttons (`Button` + `Menu`, IA §6, W-32)
  first; file actions; Undo / Redo (annotate.md's web toolbar rule); the mode switcher as
  `Tabs` (labels with icons, 40 px high, `primary.main` indicator); the Read-only Select /
  Hand `ToggleButtonGroup`; the "Highlight fields" `ToggleButton` (off by default for a newly
  opened file, J-FM-04); then sidebar and inspector toggles, zoom, back / forward, rotate
  view left / right, full screen, presentation (shortcuts.md names the toolbar as their web
  path) and Find. Below 1200 px window width everything after back / forward moves into a
  `[⋯ More]` menu (W-29). Inspector toggle is present in every mode.
- Mode tool row: hidden in Read (IA §3).
- Sidebar: persistent `Drawer`, panel `Tabs` with seven icon tabs in one row (Thumbnails,
  Outline, Annotations, Attachments, Layers, Search results, Signatures), always all seven, in
  this order (IA §4). Under the tabs: the panel title (`subtitle2`) and its actions. The
  drawer's inner edge is a resize handle (a 4 px `role="separator"` with
  `aria-valuenow`, arrows resize it); View > sidebar and inspector width presets are the
  single-pointer alternative (accessibility.md §4).
- Thumbnails: current page has a 2 px `primary.main` border; selected pages also show a check
  mark (accessibility.md §3). Page label under each (`caption`, «…», middle-truncated).
- Document view: `background.default` backdrop, pages centred, `gap.page` between them. It is
  a focusable `region` named "Document" (shortcuts.md rule 5).
- Status bar (`Toolbar` dense, `background.paper`, top divider, `caption` text): the page
  field (a dense outlined `InputBase`, 28 px high, 96 px wide for "11 / 20" and growing to
  220 px for the label plus "(11 of 20)"; a longer label is middle-truncated while the
  " (11 of 20)" part always stays visible; the full escaped label in the tooltip); the zoom
  and layout menus as dense `Select`s on `InputBase` (28 px). Layout options are J-RD-07's:
  "Single Page", "Single Page Continuous", "Two Pages", "Two Pages Continuous", and a "Show
  cover page" check item in the same menu. Trailing: job progress (when any) and the edited
  state.

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
│ [ 3 / 20 ]   [100% ▾]  [Single Page Continuous ▾]                           Edited   │
```

- Tool row: a second `Toolbar` (`toolrow`, `background.paper`, bottom divider). Tools are a
  single-selection `ToggleButtonGroup` (`size="small"`, icon + tooltip with the tool name and
  its key; the selected tool uses `action.selected`, §1.3); groups separated by vertical `Divider`s. `Shapes`, `Stamps`, `Signature` are
  split buttons: the button uses the last choice, the arrow opens a `Menu`. The quick
  controls (color, width, opacity) edit the active tool's defaults or the selection. `[⋯]`
  is the More menu (flatten annotations, export / import comments).
- When the row doesn't fit, tools from the end move into the More menu, never wrap to a
  third line; the select tool and the markup group never move.
- The first annotation shows the author notice as a `Snackbar` (W-17).

### W-07 Edit mode (IA §3; J-ED-01…J-ED-10)

```
│ [Select] | [Edit text][Add text][Add image][Link] | [▾ Watermark][▾ Header and footer]│
│ [▾ Bates numbers][▾ Background] | [Crop]                                             │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Thumbnails   │     ┌──────────────────────────┐                  │ Text              │
│              │     │  ┌──────────────────┐    │                  │ Font [▾ «Helv…»]  │
│              │     │  │ editing text|    │    │                  │ Size [11 ▾]       │
│              │     │  └──────────────────┘    │                  │ [B][I][U] [■]     │
│              │     └──────────────────────────┘                  │ Align [≡][≡][≡]   │
```

- The text box being edited has a 1 px `primary.main` outline and handles at the corners
  (8 × 8 px visual inside a 24 × 24 px hit area).
- Watermark, header and footer, Bates numbers and background are split buttons: the button
  opens their creator dialog (W-33) with a live preview, the arrow's menu has Add, Update and
  Remove (J-ED-06…J-ED-09; Remove asks first, W-23). Their inspector shows the settings of the
  open job.
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
│ [Find and mark] [Apply redactions] [Scrub metadata] | [Sign][Certify] | [Signatures] │
├──────────────┬───────────────────────────────────────────────────┬───────────────────┤
│ Signatures   │     ┌──────────────────────────┐                  │ Redaction mark    │
│ No signatures│     │ ████████ (marked)        │                  │ Page 2            │
│              │     │ text ███████ text        │                  │ Overlay text [__] │
│              │     │                          │                  │ [Remove mark]     │
│              │     └──────────────────────────┘                  │                   │
```

- Marks (J-SC-06) are drawn with an outline in the mark color plus a diagonal hatch (not
  color alone) until applied; the status bar shows J-SC-06's "4 marks; content is not removed
  until you apply redactions" while marks exist. Find and mark opens the find bar with a
  "Mark all results" button.
- Apply redactions opens a `Dialog` (`sm`) with J-SC-07's content: that the marked content is
  removed permanently when the file is saved, that the file will be rewritten without earlier
  versions, and the engine's list of other places that still contain the marked text (for
  example "Bookmarks: 1"), each row with a "Remove these too" check box, off by default;
  `[Cancel]` (default) and `[Apply]`.
- The drawing shows the Signatures panel (the sidebar keeps the tab's current panel, IA §4);
  its empty state is "No signatures".

### W-11 Find bar and Search results (IA §1, §4; J-RD-15, J-RD-31)

```
├──────────────┬───────────────────────────────────────────────────────────────────────┤
│ Search res…  │ [⌕ report____] 3 of 9 [‹][›] [Case Sensitive][Whole Words] ▬▭ [Cancel] [✕]│ find bar
│ 9 results    ├───────────────────────────────────────────────────────────────────────┤
│ Page 2       │            ┌──────────────────────────┐                               │
│  …annual «re…│            │  the ╔report╗ shows      │  current hit: primary outline │
│ Page 7       │            │  …   ┌report┐            │  other hits: fill + outline   │
│  …the «repo…»│            └──────────────────────────┘                               │
```

- The find bar is a `Paper` strip above the document view (not over it), `findbar` high:
  `TextField size="small"` 280 px, the count ("1 of 9", `aria-live="polite"`), previous/next
  `IconButton`s, two `ToggleButton`s labelled with J-RD-15's "Case Sensitive" and "Whole Words", a `LinearProgress` and
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
- One annotation selected: a "use as default" button at the end of the inspector (J-ANN-16).
  A value the engine rejects shows its typed error under that field (`FormHelperText error`)
  and the field keeps the previous value.
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
  snackbar stays until dismissed (no auto-hide for errors) and is announced politely
  (`role="status"`, accessibility.md §2).

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
the document view; sidebar panels show "Not available" (not their empty texts, which would
claim the file has no outline); other tabs keep working.

### W-16 Restricted (IA §3 notes, §7; J-RD-29, J-ANN-23, J-PG-15)

```
│ [Select] | {Highlight ⓘ}{Underline ⓘ}{Strikeout ⓘ} … {Ink ⓘ}   [Unlock with password] │
│ ⓘ The document's permissions don't allow adding comments.                            │
```

The select tool stays enabled (selecting text and filling forms aren't annotation
operations; IA §3 notes). The disabled tools stay in place and focusable; a one-line reason (`caption`,
`text.secondary`) appears under the tool row once (not per tool), with the "Unlock with
password" button at the end of the row where the journeys allow it. Menu items show the same
reason as their secondary text.

### W-17 Notices (IA §7, §10)

```
│  ┌ Snackbar (bottom-left, above the status bar) ───────────────────────────────────┐ │
│  │ This document contains scripts. They were not run.                     [✕]      │ │
│  └─────────────────────────────────────────────────────────────────────────────────┘ │
```

- One `Snackbar` at a time, at most one action. **No notice hides by itself**: each stays
  until dismissed, acted on, or replaced by a newer notice (which is then announced). This is
  the same rule on macOS; security notices (scripts, "unsupported link", "unsupported
  action") must never vanish unseen.
- Notices: scripts present (once per document, J-RD-28), "unsupported link", "unsupported
  action", "link target not found" (J-RD-17), copy not allowed by the permissions (J-RD-16),
  the full-screen offer from `/PageMode /FullScreen` (`[Full screen]`, J-RD-27), the
  first-annotation author notice (author "Guest", `[Set name]`), no text under a markup drag
  (J-ANN-01), annotations beyond the per-page limit not shown (J-ANN-24), the save notice of
  J-OS-01, and a skipped session file `«name»: file not found` (J-RD-24). Wording beyond the
  quoted journey terms is our own.

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
│ ⨯ incorrect password                  (error.main)  │
│                                 [Cancel] [Unlock]   │
└─────────────────────────────────────────────────────┘
```

The field follows W-18's rules (no `<form>`, `autocomplete="off"`, `type="password"`, paste
allowed, never stored or logged); a wrong password shows "incorrect password" in place
(J-RD-29). Unlock is the default.

### W-20 External link (IA §8; J-RD-17, J-RD-18)

```
┌ Dialog maxWidth="sm" ───────────────────────────────────────────────┐
│ Open this web address?                                              │
│ ┌ monospace, wraps, never truncated ──────────────────────────────┐ │
│ │ https://xn--exmple-cua.com/very/long/path?with=query&and=more&  │ │
│ │ page=2&ref=mail                                                  │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│ Host: xn--exmple-cua.com                              (subtitle2)   │
│                                              [Cancel] [Open]        │
└─────────────────────────────────────────────────────────────────────┘
```

Cancel is the default (`autoFocus`). The URL and the host are the engine's display strings
(CONSTITUTION 6, reader.md link policy); both wrap anywhere (`overflow-wrap: anywhere`) and are
never truncated, however long (a host can be 253 characters). For `mailto:` with attachment
parameters a line under the URL says they were removed.

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

`Keep my version` is the default and the dismiss action (Esc); it counts as IA §8's safe
dismiss button.

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
│ │                                           │  [ ] Print comment        │
│ │                                           │      contents on extra pages│
│ └───────────────────────────────────────────┘  Pages                    │
│        ‹  Sheet 1 of 2  ›                       (•) All ( ) Current      │
│                                                ( ) [2-4,7___]           │
│                                                ⨯ invalid page range     │
│                                                Scale [Fit ▾]            │
│                                                Pages per sheet [2 ▾]    │
│                                                [x] Auto-rotate          │
│                                              [Cancel] [Print]           │
└────────────────────────────────────────────────────────────────────────┘
```

Options are J-RD-21's: content, page range (all, current, a range such as "2-4,7"; an
invalid range such as "9-3" or "0" shows a hint under the field and disables Print), scale
(fit / actual size / custom %), pages per sheet (1, 2, 4), auto-rotate, and J-ANN-25's "print
comment contents on extra pages" check box (the visible label is that string, first letter
capitalized). Print hands the engine-produced output to the browser's print dialog. Below `sm`
the preview moves above the options.

### W-26 Document properties (IA §8; J-RD-22)

A `Dialog` (`sm`) with `Tabs`: File (file name, file size, page count, page size of the
current page, PDF version, creator, producer, created and modified dates in local time, tagged
yes/no, fast web view yes/no), Description (title, author, subject, keywords), Security
(encryption, permissions as a list with allowed / not allowed); these are J-RD-22's fields.
All values except the file size and page count come from the file and follow §0 rule 3. Values are read-only
`body2`, labels `caption` in `text.secondary`, two columns (label 160 px). Escaped values
use monospace. `[Close]`.

### W-27 Preferences (IA §8)

```
┌ Dialog maxWidth="md" ─────────────────────────────────────────────────┐
│ Preferences                                                           │
│ ┌──────────────┬─────────────────────────────────────────────────────┐ │
│ │ ▓General▓    │ Appearance  ( ) Light ( ) Dark (•) Follow system     │ │
│ │ Viewing      │ [x] Restore open documents at launch                 │ │
│ │ Annotations  │ Author name [Guest____________]                      │ │
│ │ Printing     │                                                      │ │
│ │ Forms        │                                                      │ │
│ │ Keyboard     │                                                      │ │
│ └──────────────┴─────────────────────────────────────────────────────┘ │
│                                                              [Close]  │
└───────────────────────────────────────────────────────────────────────┘
```

Sections: General (appearance, restore at launch, author name), Viewing (default zoom and
layout), Annotations (alignment guides when dragging), Printing (choose print content each
time), Forms (highlight fields in newly opened files, J-FM-04), Keyboard (single-key
shortcuts on/off). Changes apply at once; no OK button. A
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
| 900–1199 px | Both docked, shrinking toward their minimum widths so the document view keeps `docview.min` (200 + 260 + 360 = 820 px fits); the main toolbar's controls after back / forward move into `[⋯ More]`; the tool row moves its tail tools into its More menu. |
| 600–899 px (sm–md) | Inspector becomes a non-modal overlay over the right of the document view (a `Drawer variant="persistent"` positioned over the view: no backdrop, no focus trap), opened and closed only by its toggle, so selecting an object never moves focus away from the document view (arrow-key nudges keep working, shortcuts.md rule 6); sidebar stays docked at min width. Menu buttons collapse into one `[☰ Menu]` button with the same groups as nested menus. |
| < 600 px (xs) | Sidebar also temporary. The mode `Tabs` become a `Select`. The status bar keeps the page field and zoom; layout moves into the View menu. |

Down to 600 px the document view keeps at least `docview.min`; below that it takes the full
width (§1.1); at 200 % browser zoom on a
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

### W-32 Menus (IA §6)

```
[▾ File]
┌──────────────────────────────────────┐
│ New                              ▸   │
│ Merge files…                         │
│ Open…                        Ctrl+O  │
│ Open recent                      ▸   │
│ Close                                │
│ ──────────────────────────────────── │
│ Save                         Ctrl+S  │
│ Save as…                             │
│ Export pages as images…              │
│ Compress…                            │
│ ──────────────────────────────────── │
│ Print…                       Ctrl+P  │
│ Document properties                  │
└──────────────────────────────────────┘
```

- Menu buttons open MUI `Menu`s with dense `MenuItem`s: label (`body2`), key as secondary text
  (`caption`, `text.secondary`, right-aligned) only where shortcuts.md defines a web key
  ("Ctrl" means the platform's primary modifier, shortcuts.md rule 2), `▸` for submenus,
  `Divider` between groups. The groups and items are IA §6's table, in its order; File's
  "New ▸" submenu holds New blank document, New from images and New from copied pages
  (J-PG-01, J-OS-04, J-PG-13).
- An item the engine reports unavailable stays focusable (`aria-disabled`), dimmed, with the
  reason as a second line (`caption`). Items needing more input end in "…".

### W-33 Creator dialogs (IA §8 Creators)

```
┌ Dialog maxWidth="sm" (md for merge and split) ────────────────────────┐
│ Add watermark                                                    (h6) │
│ ┌ preview (engine-rendered page) ─────┐  Options                       │
│ │                                     │  Text  [«CONFIDENTIAL»_____]   │
│ │                                     │  Font  [▾ …]  Size [48 ▾]      │
│ │                                     │  Opacity [══●══]  Rotation [45]│
│ │                                     │  Position (•) on top of the    │
│ └─────────────────────────────────────┘    page content ( ) behind …   │
│      ‹  Page 1 of 20  ›                   Pages [all pages ▾]          │
│                                              [Cancel] [Apply]          │
└───────────────────────────────────────────────────────────────────────┘
```

One template for every creator in IA §8 (custom stamp, signature, watermark, header and
footer, Bates, background, page labels, split, merge, compress, export, new from images):
preview on the left (engine-rendered; omitted where there is nothing to preview), options on
the right in a single column (labels above fields, 16 px between groups), the action button
named after the action as the default, Cancel beside it. Option names are the journeys'
quoted strings. The per-creator details (signature draw / type / image tabs, the merge file
list with per-row errors, the compress result report, split modes) are drawn in #45.

### W-34 Save As (IA §6; J-OS-02)

```
┌ Dialog maxWidth="xs" ───────────────────────────────┐
│ Save as                                             │
│ [ ] Remove earlier versions                         │
│     Writes the file again from scratch; earlier     │
│     revisions are not kept.                         │
│ ⚠ (signed file) Signatures will no longer be valid. │
│                         [Cancel] [Choose location…] │
└─────────────────────────────────────────────────────┘
```

The browser's save picker can't hold our options, so a small dialog comes first: the "remove
earlier versions" check box, off by default (J-OS-02), with the signed-file warning line shown
only when it applies. `Choose location…` is the default. On a signed file, checking the box
first opens a W-23-style confirmation that the signatures will no longer verify, with Cancel
as the default (J-OS-02 error branch, IA §8); Cancel unchecks the box. Then the file-handle
adapter's save picker.

### W-35 Keyboard shortcuts list (IA §6 Help; shortcuts.md)

A `Dialog` (`md`) opened from Help or the "?" key (single key, shortcuts.md rule 5): a filter
field and a two-column table per context (shortcuts.md's tables: command, key), `subtitle2`
group headers, keys drawn as `<kbd>` chips (monospace, control-boundary outline). `[Close]`.

### W-36 Note popup (J-ANN-04)

```
        [note icon]
        ┌ Popper (paper, elevation 8, 280 × auto) ┐
        │ «Ann» · 20 Nov 2026 14:02     [⋯] [✕]   │
        │ [comment text____________________]      │
        │ [________________________________]      │
        └──────────────────────────────────────────┘
```

A non-modal `Popper` anchored next to its icon, kept inside the visible page area; the text
field gets focus when it opens; Esc or clicking outside closes it and keeps the text
(shortcuts.md rule 9). Author and date are untrusted text.

### W-37 Magnifier (IA §9; J-RD-25)

A 200 px circular loupe (2 px control-boundary ring, elevation 8) whose centre is 24 px
up and right of the pointer (flipped to stay inside the window near its edges), so the
pointer's hot spot is never covered,
showing the engine's rendering of the area under it at 4× the current zoom ("+" / "-" change
the factor), with the factor in a `caption` chip at the ring's bottom.

## 3. Reviewer checklist (AUTOMATION §5.4)

The audit's vision reviewer checks each screenshot for:
1. The areas of W-05 in the right order and the component named here (AppBar rows, Tabs,
   Drawers, status bar).
2. Screenshots are taken at device pixel ratio 1, window 1280 × 800, browser zoom 100 %. Sizes
   within ±2 px of §1.1; all gaps on the 4 px grid; text in the §1.2
   variants.
3. Every empty, loading and error state shows text, as in W-02, W-04, W-12, W-14, W-15.
4. Contrast and focus ring as in §1.3, in both appearances; page content never recolored by
   the theme.
5. Disabled controls visible with their reason (W-16); dialogs with the default button
   named here.
6. Untrusted text plain, escaped, and middle-truncated (URLs never truncated).
