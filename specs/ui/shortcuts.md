# Keyboard shortcuts

Status: ready (M0, #10). Areas and commands: [information-architecture.md](information-architecture.md).
Accessibility rules for keys: [accessibility.md](accessibility.md).

The journeys are the source of truth for every key they name; this table collects them and
fills in the rest. A change here that contradicts a journey is a bug in this file.

## Rules

1. **macOS** follows the standard macOS keys (Apple HIG, Keyboard shortcuts) and the menu bar
   shows every shortcut next to its command.
2. **Web primary modifier.** "Ctrl" in the Web column means the platform's primary modifier:
   Cmd in browsers on macOS, Ctrl on Linux and Windows. CI runs Linux, so tests press Ctrl.
3. **Browser-reserved keys are never used on web** (reader.md shared rules). The app does not
   bind, and tests never press, any key Chrome, Firefox or Safari reserves on Linux, Windows
   or macOS. Known list (extend it when a new conflict is found):
   - primary modifier + T, N, W, Q, L, D, H, J, K, E, U, B, M, R, G, 1…9, Tab, PageUp,
     PageDown, `[`, `]`, `` ` ``, Left, Right (macOS browsers: Cmd+Left/Right are history);
   - primary modifier + Shift + T, N, W, R, G, A, B, M, P, O, S, I, J, C, Delete, Tab;
   - macOS browsers: Cmd+Option+Left/Right, Cmd+Y, Cmd+H, Cmd+M;
   - F1, F3, F5, F6, F7, F10, F11, F12;
   - Alt+Left/Right, Alt+Home, Alt+D/E/F, Alt+1…8.

   Where macOS uses such a key, web uses the toolbar, the app menu or the alternative listed.
4. **Keys the web app takes over** from the browser: Ctrl+O, Ctrl+S, Ctrl+P and Ctrl+F
   anywhere in the app; Ctrl+= (also Ctrl++ and numpad +), Ctrl+- (also numpad -) and Ctrl+0
   only while the document view has focus (elsewhere they stay the browser's zoom, which the
   200% zoom requirement relies on); Ctrl+A, Ctrl+X/C/V, Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y
   (Linux and Windows only) only while the document view or a panel list has focus. In text
   fields (find field, inspector fields, dialogs) every key keeps its native text-editing
   meaning, and undo acts on the field's text (annotate.md undo rule).
5. **Single-key shortcuts** (no modifier) act only while the document view has focus, never
   in a sidebar panel (letters there do type-ahead in lists and trees), a text field, the find
   bar or an editing box, and they can be turned off in Preferences (WCAG 2.2 SC 2.1.4). Keys
   are matched on `event.key` (web) / the typed character (macOS), case-insensitively, so they
   follow the keyboard layout. Web: the document view is a focusable `region` (not
   `role="application"`, which would hide the page text from browse-mode reading) with an
   accessible name and a description of its keys. Screen-reader users read the page text in
   browse mode and switch to focus mode (for example NVDA Insert+Space) to use the view's
   keys; every single-key command also has a toolbar or menu path.
6. **Context-sensitive keys.** In the modes where the selected object can be edited
   (Annotate, Edit, Forms), arrow keys move the selected annotation, image, link or field and
   Delete deletes the selection. In Read mode they never change the document: arrows scroll /
   change pages and Delete does nothing on the page. A form field focused for filling gets its
   native keys (arrows in radio groups and lists, typing in text fields). In panels, Delete
   acts on the focused thumbnails or outline entries. "Delete" in these tables means the
   ⌫ key (labelled delete on Mac keyboards, Backspace elsewhere) or Forward Delete (fn+⌫ on
   Macs, Delete elsewhere).
7. **One meaning per key in a context.** The table below has no duplicate key within one
   context and platform, except Esc, Return, Shift+arrows and Space, which follow the precedence
   in rules 9, 10 and 12;
   a new shortcut must be checked against the whole table and rule 3.
8. **Shortcuts are commands.** A shortcut calls the same command as its menu item or button;
   it never adds behavior of its own (CONSTITUTION 6: the command is an engine call).
9. **Esc** does the first that applies, innermost first: close a dialog (cancel) → leave a
   text editing box or text box (the typed text is kept) → close an open note popup (its text
   is kept) → cancel the drawing in progress → turn off the magnifier → with the find bar
   focused: cancel a running search (J-RD-31), else close the find bar → stop a load before its
   first page appears (information-architecture.md §7) → leave presentation → leave full screen
   → return from a drawing tool to the select tool → clear the selection. **Web full screen
   exception:** in Fullscreen-API mode (full screen and presentation) the browser always takes
   Esc first and exits; the app reacts to `fullscreenchange` (tests assert that event, not a
   keydown), and the inner steps are reached with each overlay's visible Cancel / Close.
10. **Return** (web Enter) in the document view does the first that applies: finish the
   polygon or polyline being drawn (J-ANN-09) → activate the focused annotation, link or field
   (J-ANN-04, J-RD-17) → with a drawing tool chosen and nothing focused, place a default-size
   object at the centre of the visible part of the current page (accessibility.md §1). Placing
   applies to note, text box, callout, the shapes, stamps and signatures; markup tools apply to
   a text selection instead, and ink and the eraser have no place action.
11. **Text formatting while editing.** macOS has a Format menu (Bold Cmd+B, Italic Cmd+I,
   Underline Cmd+U), enabled only while a text box or text editing box has focus. While it is
   enabled, the app removes Document Properties' Cmd+I key equivalent (the menu item stays),
   so Cmd+I has one owner at a time; an XCUITest checks both states.
12. **Shift+arrows and Space in the document view.** With an object selected in an editable
   mode, Shift+arrows nudge it by 10 pt; otherwise they select text. With an object focused
   (Tab) in an editable mode, Space adds it to or removes it from the selection; otherwise
   Space scrolls. Web has no formatting keys (Ctrl+B/I/U collide with rule 3 or browser defaults);
   formatting is in the inspector.

## File and window

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Open | Cmd+O | Ctrl+O | J-RD-01 |
| Save | Cmd+S | Ctrl+S | J-OS-01 |
| Save As | Cmd+Shift+S | menu (Ctrl+Shift+S is Firefox's screenshot key) | J-OS-02 |
| New blank document | Cmd+N | menu | J-PG-01 |
| Print | Cmd+P | Ctrl+P (also the toolbar button) | J-RD-21 |
| Document properties | Cmd+I (menu only while editing text, rule 11) | menu | J-RD-22 |
| Close tab | Cmd+W | the tab's close button | J-RD-23 |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab (also Cmd+Shift+] / Cmd+Shift+[) | Alt+PageDown / Alt+PageUp | J-RD-23 |
| Preferences | Cmd+, | menu | — |
| Keyboard shortcuts list | Help menu | ? (single key, rule 5) | — |
| Quit | Cmd+Q | — | — |

## Edit

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Undo | Cmd+Z | Ctrl+Z | J-ANN-20 |
| Redo | Cmd+Shift+Z | Ctrl+Shift+Z; Ctrl+Y on Linux and Windows | J-ANN-20 |
| Cut / Copy / Paste | Cmd+X / Cmd+C / Cmd+V | Ctrl+X / Ctrl+C / Ctrl+V | J-RD-16, J-ANN-17, J-PG-13 |
| Select all (text in the view; all items in a focused panel) | Cmd+A | Ctrl+A | J-RD-16, J-ANN-19 |
| Delete selection (rule 6) | Delete | Delete | J-ANN-17, J-PG-04 |
| Find | Cmd+F | Ctrl+F | J-RD-15 |
| Next / previous match (find field focused) | Return / Shift+Return; also Cmd+G / Cmd+Shift+G | Enter / Shift+Enter | J-RD-15 |
| Close the find bar | Esc (rule 9) | Esc (rule 9) | J-RD-15 |

## View

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Zoom in / out | Cmd+= / Cmd+- | Ctrl+= / Ctrl+- (rule 4 variants; also the toolbar) | J-RD-08 |
| Actual size | Cmd+0 | Ctrl+0 | J-RD-08 |
| Fit page | Cmd+9 | status-bar zoom menu | J-RD-08 |
| Fit width, fit height | View menu | status-bar zoom menu | J-RD-08 |
| Zoom with the wheel | Cmd+scroll, pinch | Ctrl+scroll, pinch | J-RD-08 |
| Rotate view right / left | Cmd+R / Cmd+L | toolbar | J-RD-09 |
| Show / hide sidebar | Ctrl+Cmd+S | toolbar | J-RD-10 |
| Sidebar panel: thumbnails, outline, annotations, attachments, layers, search results, signatures | Cmd+Option+1 … Cmd+Option+7, in that order | panel tabs (Tab, then arrow keys) | J-RD-10…J-RD-15 |
| Show / hide inspector | Cmd+Option+I | toolbar | — |
| Full screen | Ctrl+Cmd+F | toolbar (Fullscreen API) | J-RD-19 |
| Presentation | Cmd+Shift+F | toolbar | J-RD-19 |
| Magnifier on / off | M (rule 5) | M (rule 5) | J-RD-25 |
| Magnifier stronger / weaker | + / - while shown | + / - while shown | J-RD-25 |
| Leave full screen, presentation, magnifier | Esc (rule 9) | Esc (rule 9) | J-RD-19, J-RD-25 |

## Navigate (document view focused)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Next / previous page | Right / Left arrow, Page Down / Page Up | same | J-RD-06 |
| Scroll | Up / Down arrow, Space / Shift+Space | same | J-RD-06 |
| Select text by keyboard (text-select or select tool, no object selected): start at the beginning of the current page's text in the engine's reading order, or extend the current selection, by character / by line | Shift+Right/Left / Shift+Down/Up | same | J-RD-16, J-ANN-01 |
| Extend a text selection with the pointer | Shift-click | Shift-click | J-RD-16 |
| First / last page | Home / End (also Cmd+Up / Cmd+Down) | Home / End | J-RD-06 |
| Go to page (focus the page field) | Cmd+Option+G | G (rule 5) | J-RD-06 |
| Back / forward | Cmd+[ / Cmd+] | toolbar Back / Forward | J-RD-06, J-RD-17 |
| Move focus through the annotations, links and fields of the current page; after the last one Tab leaves the document view (accessibility.md §1) | Tab / Shift+Tab | Tab / Shift+Tab | J-ANN-04 |
| Activate the focused annotation or link | Return (rule 10) | Enter (rule 10) | J-ANN-04, J-RD-17 |
| Presentation: next / previous | Right arrow, Space, click / Left arrow | same | J-RD-19 |

## Tools (Annotate mode, plus the select tool in Edit; rule 5)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Select tool | V | V | J-ANN-17 |
| Highlight | H | H | J-ANN-01 |
| Underline | U | U | J-ANN-02 |
| Note | N | N | J-ANN-04 |
| Text box | T | T | J-ANN-05 |
| Rectangle / ellipse | R / O | R / O | J-ANN-07 |
| Line / arrow | L / A | L / A | J-ANN-08 |
| Ink | I | I | J-ANN-11 |
| Eraser | E | E | J-ANN-12 |
| Constrain while drawing (square, circle, 45° angles) | Shift-drag | Shift-drag | J-ANN-07, J-ANN-08 |
| Finish a polygon / polyline | Return (rule 10) | Enter (rule 10) | J-ANN-09 |
| Place a default-size object (drawing tool chosen, nothing focused) | Return (rule 10) | Enter (rule 10) | — |
| Resize the selection | the inspector's width and height fields (no key: web Alt+arrows are browser history) | same | J-ANN-17 |
| Cancel the current drawing; then return to the select tool; then clear the selection | Esc (rule 9) | Esc (rule 9) | J-ANN-09 |
| Nudge the selection 1 pt / 10 pt | arrows / Shift+arrows | same | J-ANN-17 |
| Add to the selection | Shift-click, Cmd-click; keyboard: Tab to the object, then Space | Shift-click, Ctrl-click; keyboard: Tab, then Space | J-ANN-16 |

Tool keys apply in the mode that owns the tool (V in Annotate and Edit, the others in
Annotate); in other modes the key does nothing. G, M and ? are the only single keys that work
in every mode (still under rule 5). The other Edit-mode tools have no single-key shortcuts in
this version.

## Pages (organizer and thumbnails focused)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Select several / a range | Cmd-click / Shift-click | Ctrl-click / Shift-click | J-PG-04 |
| Move focus in the grid / list | arrows | arrows | J-PG-06 |
| Toggle the focused page's selection / extend the range | Space / Shift+arrows | Space / Shift+arrows | J-PG-04, J-PG-13 |
| Move the selected pages right / left | Cmd+Option+→ / Cmd+Option+← | Ctrl+Shift+→ / Ctrl+Shift+← | J-PG-06 |
| Delete the selected pages | Delete | Delete | J-PG-04 |
| Copy pages | Cmd+C | Ctrl+C | J-PG-13 |

## Outline panel focused

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Expand / collapse an entry | Right / Left arrow | same | J-RD-11 |
| Go to the entry's target | Return | Enter | J-RD-11 |
| Rename | Cmd+Return, the Rename menu item, double-click or context menu | F2, double-click or context menu | J-PG-14 |
| Move an entry (no drag) | Cmd+X on the entry, then Cmd+V on the new parent (pasted as its last child) | Ctrl+X, then Ctrl+V | J-PG-14 |
| Delete the entry | Delete | Delete | J-PG-14 |
