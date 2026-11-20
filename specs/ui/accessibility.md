# Accessibility

Status: ready (M0, #10). Areas: [information-architecture.md](information-architecture.md).
Keys: [shortcuts.md](shortcuts.md).

Target: WCAG 2.2 level AA for the web app; the Apple HIG accessibility guidance for the macOS
app (VoiceOver, Full Keyboard Access, Increase Contrast, Reduce Motion, Reduce Transparency).
Every feature spec fills its `## Accessibility` section against this file.

## 1. Keyboard only

- **Every pointer action has a keyboard path.** Opening, navigating, selecting text, every
  command in every mode, panel actions, dialogs and the page organizer. Drawing tools: with
  the document view focused and a tool chosen, Return places the object with a default size at
  the keyboard caret position (the top-left of the visible page area for the first object),
  then arrows move it and Shift+Option+arrows (web Shift+Alt+arrows) resize it; the
  journeys that need a keyboard path name it (J-ANN-04, J-PG-06).
- **Focus order** follows the visual layout: tabs → main toolbar → mode tool row → sidebar →
  find bar → document view → inspector → status bar. macOS supports Full Keyboard Access; web
  provides a "skip to document" link as the first focusable element and landmark regions
  (`banner`, `navigation` for the sidebar, `main` for the document view, `complementary` for
  the inspector, `contentinfo` for the status bar).
- **Inside the document view**, Tab moves through the annotations, links and form fields of
  the current page in the engine's order (the structure tree order for tagged files, else the
  page's annotation order), then to the next page's (J-ANN-04).
- **Focus is always visible** (web: a focus ring with at least 3:1 contrast against both the
  page and the chrome; macOS: the system focus ring) and never hidden behind the toolbar or a
  panel (WCAG 2.2 SC 2.4.11).
- **Dialogs trap focus** and return it to the control that opened them; Esc cancels.
- **No keyboard trap** in the document view: Esc and Tab always lead out.

## 2. Screen readers (VoiceOver on macOS; VoiceOver, NVDA, JAWS on web)

- **Page text is exposed** from the engine: the text of the visible pages in reading order
  (structure tree for tagged files, else the engine's reading-order heuristic), as an
  accessible text layer (macOS: accessibility elements per text block; web: an off-screen,
  selectable text layer with roles). The native layer does not extract text itself
  (CONSTITUTION 6).
- **Every page** is a group labelled "Page N of M" plus its page label when it has one.
- **Annotations and links** are elements with role, type, author and text ("Note by Ann:
  check figure 2"); links announce their target as the engine's display string (J-RD-18).
- **Images** in the document are announced with their `/Alt` text when the file has one; an
  untagged figure is announced as "image" without invented text.
- **Controls** have labels in our own words, and toolbar icon buttons have accessible names
  and tooltips. Toggle buttons announce their state (on/off); disabled commands announce the
  engine's reason.
- **Live announcements** (macOS accessibility notifications; web `aria-live="polite"`) for:
  find results ("3 of 9"), page changes after a jump, page reorder results ("moved to position
  4", J-PG-06), job completion and failures (save, export), and errors. Not for every scroll.
- **Untrusted text** is announced as plain text, with control characters spelled out as the
  escaped display string (information-architecture.md §0 rule 3).

## 3. Visual

- **Contrast**: text at least 4.5:1, large text and UI component boundaries and icons at least
  3:1, in both light and dark appearance (WCAG 2.2 SC 1.4.3, 1.4.11). macOS uses system colors,
  which adapt to Increase Contrast; web defines its MUI palette for light and dark and its
  values are checked by an automated contrast test.
- **Dark mode**: the chrome follows the system appearance (or the app preference). Page content
  is never inverted by the chrome theme; only the explicit page tint changes it (J-RD-20).
- **Color is never the only signal**: annotation types have icons, search hits have an outline
  as well as a fill, errors have text, selected thumbnails have a border and a check mark.
- **Text size**: web works at 200% browser zoom and with 400% reflow of the chrome (not of PDF
  pages) without loss of commands (SC 1.4.4, 1.4.10); macOS honors the system text size where
  SwiftUI supports it.
- **Target size**: web interactive targets at least 24×24 CSS px (SC 2.5.8); macOS follows the
  HIG minimum control sizes.
- **Motion**: no animation is required to understand state; Reduce Motion (macOS) and
  `prefers-reduced-motion` (web) turn off animated scrolling and transitions. Page transitions
  from the file are always ignored (J-RD-19). Nothing flashes more than 3 times per second.
- **Transparency**: macOS Reduce Transparency removes sidebar and toolbar translucency.

## 4. Pointer and input

- Drags have a non-drag alternative (WCAG 2.2 SC 2.5.7): reorder pages with the move
  commands (J-PG-06), move and resize annotations with arrows (J-ANN-17), outline moves
  through cut and paste in the outline panel (J-PG-14).
- Pinch zoom and trackpad gestures always have menu and key equivalents (J-RD-08).
- Single-key shortcuts follow shortcuts.md rule 5 (SC 2.1.4).

## 5. Verification

- Web: the Playwright harness (#12) runs `axe-core` on the start screen, a document in each
  mode, each sidebar panel and each dialog, and fails on any violation of WCAG 2.2 A or AA
  rules; a keyboard-only Playwright pass opens a fixture, navigates, searches and adds a note
  without any pointer event.
- macOS: the XCUITest harness (#13) runs `performAccessibilityAudit()` on the same screens,
  and journeys drive controls through their accessibility identifiers and labels, which
  keeps those labels present.
- The audit's dogfooding pass (AUTOMATION §5.4) includes one VoiceOver walkthrough of a
  rotating journey and scores contrast and focus visibility from screenshots in both
  appearances.
