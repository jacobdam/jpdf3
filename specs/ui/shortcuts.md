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
   bind, and tests never press: Ctrl+T, Ctrl+N, Ctrl+W, Ctrl+Shift+T/N/W, Ctrl+Tab,
   Ctrl+Shift+Tab, Ctrl+PageUp/PageDown, Ctrl+1…9, Ctrl+L, Ctrl+D, Ctrl+H, Ctrl+J, Ctrl+R,
   Ctrl+Shift+R, F5, Ctrl+Shift+I/J/C, F12, F11, F6, F3, Ctrl+G, Ctrl+Shift+G, Alt+Left/Right,
   Alt+Home, Alt+D/E/F, Alt+1…8. Where macOS uses such a key, web uses the toolbar, the app
   menu or the alternative listed.
4. **Keys the web app takes over** from the browser, only while focus is inside the app and
   never in the browser's own UI: Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+P, Ctrl+F, Ctrl+=,
   Ctrl+-, Ctrl+0, Ctrl+A, Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y.
5. **Single-key shortcuts** (no modifier) act only while the document view or a sidebar panel
   has focus, never while a text field, the find bar or an editing box has focus, and can be
   turned off in Preferences (WCAG 2.2 SC 2.1.4).
6. **Context-sensitive keys.** Arrow keys move the selected object when an annotation, image,
   link or field is selected, and scroll / change pages otherwise. Delete acts on what has
   focus: the selected objects, thumbnails or outline entries.
7. **One meaning per key in a context.** The table below has no duplicate key within one
   context and platform; a new shortcut must be checked against the whole table and rule 3.
8. **Shortcuts are commands.** A shortcut calls the same command as its menu item or button;
   it never adds behavior of its own (CONSTITUTION 6: the command is an engine call).

## File and window

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Open | Cmd+O | Ctrl+O | J-RD-01 |
| Save | Cmd+S | Ctrl+S | J-OS-01 |
| Save As | Cmd+Shift+S | Ctrl+Shift+S | J-OS-02 |
| New blank document | Cmd+N | menu | J-PG-01 |
| Print | Cmd+P | Ctrl+P (also the toolbar button) | J-RD-21 |
| Document properties | Cmd+I | menu | J-RD-22 |
| Close tab | Cmd+W | the tab's close button | J-RD-23 |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab (also Cmd+Shift+] / Cmd+Shift+[) | Alt+PageDown / Alt+PageUp | J-RD-23 |
| Preferences | Cmd+, | menu | — |
| Keyboard shortcuts list | Help menu | ? (single key, rule 5) | — |
| Quit | Cmd+Q | — | — |

## Edit

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Undo | Cmd+Z | Ctrl+Z | J-ANN-20 |
| Redo | Cmd+Shift+Z | Ctrl+Shift+Z, Ctrl+Y | J-ANN-20 |
| Cut / Copy / Paste | Cmd+X / Cmd+C / Cmd+V | Ctrl+X / Ctrl+C / Ctrl+V | J-RD-16, J-ANN-17, J-PG-13 |
| Select all (text in the view; all items in a focused panel) | Cmd+A | Ctrl+A | J-RD-16, J-ANN-19 |
| Delete selection | Delete or Backspace | Delete (Backspace on macOS browsers) | J-ANN-17, J-PG-04 |
| Find | Cmd+F | Ctrl+F | J-RD-15 |
| Next / previous match (find field focused) | Return / Shift+Return; also Cmd+G / Cmd+Shift+G | Enter / Shift+Enter | J-RD-15 |
| Close the find bar | Esc | Esc | J-RD-15 |

## View

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Zoom in / out | Cmd+= / Cmd+- | Ctrl+= / Ctrl+- (also the toolbar) | J-RD-08 |
| Actual size | Cmd+0 | Ctrl+0 | J-RD-08 |
| Fit page | Cmd+9 | status-bar zoom menu | J-RD-08 |
| Fit width, fit height | View menu | status-bar zoom menu | J-RD-08 |
| Zoom with the wheel | Cmd+scroll, pinch | Ctrl+scroll, pinch | J-RD-08 |
| Rotate view right / left | Cmd+R / Cmd+L | toolbar | J-RD-09 |
| Show / hide sidebar | Ctrl+Cmd+S | toolbar | J-RD-10 |
| Sidebar panel 1…6 | Cmd+Option+1…6 | panel tabs (Tab, then arrow keys) | J-RD-10…J-RD-15 |
| Show / hide inspector | Cmd+Option+I | toolbar | — |
| Full screen | Ctrl+Cmd+F | toolbar (Fullscreen API) | J-RD-19 |
| Presentation | Cmd+Shift+F | toolbar | J-RD-19 |
| Magnifier on / off | ` (backtick, rule 5) | ` (backtick, rule 5) | J-RD-25 |
| Magnifier stronger / weaker | + / - while shown | + / - while shown | J-RD-25 |
| Leave full screen, presentation, magnifier | Esc | Esc | J-RD-19, J-RD-25 |

## Navigate (document view focused)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Next / previous page | Right / Left arrow, Page Down / Page Up | same | J-RD-06 |
| Scroll | Up / Down arrow, Space / Shift+Space | same | J-RD-06 |
| First / last page | Home / End (also Cmd+Up / Cmd+Down) | Home / End | J-RD-06 |
| Go to page (focus the page field) | Cmd+Option+G | G (rule 5) | J-RD-06 |
| Back / forward | Cmd+[ / Cmd+] | toolbar Back / Forward | J-RD-06, J-RD-17 |
| Move focus to the next / previous annotation or link on the page | Tab / Shift+Tab | Tab / Shift+Tab | J-ANN-04 |
| Activate the focused annotation or link | Return | Enter | J-ANN-04, J-RD-17 |
| Presentation: next / previous | Right arrow, Space, click / Left arrow | same | J-RD-19 |

## Tools (Annotate and Edit modes; rule 5)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Select tool | V | V | J-ANN-17 |
| Highlight | H | H | J-ANN-01 |
| Underline | U | U | J-ANN-02 |
| Note | N | N | J-ANN-04 |
| Text box | T | T | J-ANN-05 |
| Rectangle / ellipse | R / O | R / O | J-ANN-07 |
| Line / arrow | L / A | L / A | J-ANN-08 |
| Pen | P | P | J-ANN-11 |
| Eraser | E | E | J-ANN-12 |
| Constrain while drawing (square, circle, 45° angles) | Shift-drag | Shift-drag | J-ANN-07, J-ANN-08 |
| Finish a polygon / polyline | Return | Enter | J-ANN-09 |
| Cancel the current drawing; then return to the select tool; then clear the selection | Esc | Esc | J-ANN-09 |
| Nudge the selection 1 pt / 10 pt | arrows / Shift+arrows | same | J-ANN-17 |
| Add to the selection | Shift-click, Cmd-click | Shift-click, Ctrl-click | J-ANN-16 |

Tool keys apply in the mode that owns the tool; in other modes the key does nothing. G, `
and ? are the only single keys that work in every mode (still under rule 5). The Edit-mode tools have no single-key
shortcuts in this version.

## Pages (organizer and thumbnails focused)

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Select several / a range | Cmd-click / Shift-click | Ctrl-click / Shift-click | J-PG-04 |
| Move the selected pages right / left | Cmd+Option+→ / Cmd+Option+← | Ctrl+Shift+→ / Ctrl+Shift+← | J-PG-06 |
| Delete the selected pages | Delete | Delete | J-PG-04 |
| Copy pages | Cmd+C | Ctrl+C | J-PG-13 |

## Outline panel focused

| Command | macOS | Web | Journeys |
|---|---|---|---|
| Expand / collapse an entry | Right / Left arrow | same | J-RD-11 |
| Go to the entry's target | Return | Enter | J-RD-11 |
| Rename | double-click or context menu | same | J-PG-14 |
| Delete the entry | Delete | Delete | J-PG-14 |
