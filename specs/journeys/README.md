# User journeys (the scope)

Format (parsed by the judge, ops/judge/CONTRACT.md §4):

```
## J-AREA-NN: title
- milestone: M5
- platform: web, macos
- test: e2e.web.<id>, e2e.macos.<id>
- superseded_by: J-AREA-MM          (optional)

Steps: open … → … → save → reopen → … (and what MuPDF/Poppler must show)
```

Rules:
- **IDs are append-only.** Never delete a journey. Supersede it with one that covers at least
  the same steps.
- **Every journey needs E2E test IDs.** A journey passes when all of them pass in
  `cargo xtask ci`.
- **Milestone gates require** every journey assigned to that milestone (or earlier) to pass.
