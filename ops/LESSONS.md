# Lessons learned (read at the start of every session; curated by retros, ≤ 60 entries)

Format: **situation → what to do** (link to the journal entry or PR). Refine existing entries
instead of adding near-duplicates.

## Tooling & environment
- Scratch files (probe trees, throwaway scripts, downloads) → put them in the session scratchpad
  directory, never `/tmp` (owner instruction, #28).
- Claude Code project settings and hooks apply **live** to running sessions → never write
  `.claude/settings.json` before its hook script exists and passes `--selftest` (bootstrap, 2026-11-20).
- macOS `/usr/bin/openssl` is LibreSSL and its default PKCS#12 output isn't readable by every
  tool → always pass `-keypbe AES-256-CBC -certpbe AES-256-CBC -macalg sha256` (bootstrap).
- pyHanko `SimpleSigner.load_pkcs12(file, passphrase=...)`: the 2nd positional argument is the
  CA chain, not the password (bootstrap).
- macOS: no `timeout` binary (exit 127), system `python3` lacks `tomllib` → use
  `/opt/homebrew/bin/python3.14 -I` for judge scripts; colima/docker mounts only `$HOME`, so put
  files for containers there, not in `/tmp` ([journal](journal/2026-11-20.md)).
- Judge-like memory checks: macOS RSS ≠ address space → run the Linux build in docker
  (`rust:<pinned>`) under `ulimit -v 2097152` ([journal](journal/2026-11-20.md)).
- `reference.py` on the full smoke corpus takes ~4 min locally; one pdf.js file hangs MuPDF
  (> 60 s) → reference tools must be called with timeouts (bootstrap).

## Judge & CI
- MuPDF and Poppler agree (≤ 2% pixels differ at 72 dpi after 2× downscale) on ~78% of
  smoke pages; only those consensus pages are ratcheted (bootstrap calibration).
- A skipped required job counts as success on GitHub → the verdict job runs `if: always()` and
  fails unless every upstream job succeeded (bootstrap).

- main red with `judge-verdict` green, only `pages` failed (deploy-pages "No artifacts named
  github-pages") **and** `upload-pages-artifact` succeeded in the same attempt → infra: `gh run
  rerun --failed <id>`, no code change. If the upload step failed or was skipped, it's a real bug
  ([journal](journal/2026-11-20.md)).

- Writing a local check that mirrors a judge script → match its Python regex semantics
  exactly (`\s` crosses newlines and includes U+001C–U+001F; pathlib globs match `.md`
  files and directories), and diff both side by side on probe trees before review
  ([journal](journal/2026-11-20.md), PR #27).

- Writing journeys (append-only gates) → give every step one checkable result and a test
  mechanism Playwright/XCUITest can drive; never use browser-reserved keys (Ctrl+Tab/W/N/T/R,
  F11, Alt+Left); keep timing/fps targets in the bench, not E2E; web file handles go behind a
  test adapter; no "X or Y" outcomes; state the fixture and exact counts; every disabled UI
  control also gets a direct binding call expecting the engine's typed error; a security
  promise must hold for incremental saves (old revisions stay in the file); permission checks
  cover every path that takes pages out of a file (copy, extract, merge/insert source); assert
  a state only after the step that saves it; "remove what we added" needs a private marker
  ([journal](journal/2026-11-20.md), PR #30, PR #32, [PR #39](journal/2026-11-20.md)).

- Asserting with an external tool in a journey (qpdf, mutool, pdftotext, pdfimages, pyHanko) →
  check that the tool really prints that field: `qpdf --show-encryption` has no V or key
  length (use `qpdf --json --json-key=encrypt`); pyHanko decrypts but doesn't extract text
  (pipe its output to `pdftotext`); `qpdf --json` lists AcroForm fields once per widget
  ([journal](journal/2026-11-20.md), PR #43).

- Writing a spec that restates journeys (UI spec, feature spec) → quote the journeys' exact UI
  strings (empty states, option names, page-field format, notice wording) instead of
  paraphrasing, and check new web keys against `specs/ui/shortcuts.md` rule 3 (the
  browser-reserved list) ([journal](journal/2026-11-20.md), PR #40).
- Drawing wireframes or naming UI components → check each component's real constraints before
  writing sizes or behavior: MUI small fields are 40 px, `Tab` has `minWidth` 90, no focusable
  control inside `role="tab"`, temporary drawers are modal; SwiftUI `.disabled()` drops out of
  Full Keyboard Access, NSDocument autosave writes silently, the macOS split view has no
  overlay sidebar ([journal](journal/2026-11-20.md), PR #44).

- Choosing npm packages (web app, test tools) → run `npm run licenses` in apps/web on the
  candidate tree first: CONSTITUTION 10 has no build-only exemption, and Vite 8, ESLint,
  Babel-based plugins and axe-core all fail it. Check component facts with a Playwright
  `toHaveCSS` assertion, not by eye (MUI 9 ignores Typography `color`) ([journal](journal/2026-11-20.md), PR #49).

- Asserting computed styles in Playwright after a pointer or focus action → await
  `el.getAnimations()` to finish before `toHaveCSS`, or a 250 ms MUI transition makes the
  result depend on timing. Check a disabled control's hover and focused-hover states: an sx
  `&:hover` override ties with the theme's focus-visible rule and wins, so set only the
  property you mean ([journal](journal/2026-11-20.md), PR #49).

- macOS XCUITest → Automation Mode must work without a password: locally that needs the
  owner's sudo (#52), GitHub runners have it; `project-macos` skips PRs, so verify UI tests
  with `gh workflow run ci.yml --ref <branch>` before merging. Query SwiftUI elements by
  `accessibilityIdentifier` + `.element` (the title bar is a static text too; `firstMatch`
  failed with count 1), read static text from `value` when `label` is empty; check
  entitlements on Release only (Debug test builds inject read access to `/`)
  ([journal](journal/2026-11-20.md), PR #51).
- Adding a fuzz target → the judge's nightly step runs `cargo fuzz run` with no seeds (33
  edges from an empty corpus), so give each target a structure template like
  `jpdf_testgen::from_fuzz_chunks` (about 1000 edges), not just seeds in `xtask/src/fuzz.rs`
  ([journal](journal/2026-11-21.md), PR #58).

- Timing tests with shell-script fake engines → on macOS the first exec of a freshly written
  script can take ~2 s; give timed assertions a warmup run and generous upper bounds, and
  test process-group kills only from Rust (a stray `kill -- -PGID` in the tool shell kills
  the session's shell) ([journal](journal/2026-11-21.md), PR #59).

## Engine & security
- Parsing hostile input: per-object caps don't bound a document. Overlapping objects/xref
  sections make references re-parse the same bytes (quadratic time); several capped objects
  alive at once exceed 2 GiB → from the start, use a per-document lexing budget charged per
  byte, a per-document object budget, overlap rejection by byte span, one parsed object alive
  at a time, and compact xref entries ([journal](journal/2026-11-20.md), PR #24).
- Writing signature or network journeys → give approval signatures a default change policy
  (shadow attacks are legal edits), keep certificate-derived URLs off loopback/private
  addresses, check web limits (CORS, mixed content, CSP, `fetch` redirect modes) and macOS ATS
  before promising a request, and draw the fixture PKI's expiry timeline before writing
  clock steps ([journal](journal/2026-11-20.md), PR #46).
- Writing a conformance suite or any checker tested only by toys → for each check ask "what
  broken implementation still passes?": known answers must reach multi-block and padding
  boundaries (not just `""`/`"abc"`), lenient-read rules need a positive salvage check, every
  limit needs a per-axis/per-path boundary, and the spec must say what the suite cannot see
  (allocations) ([journal](journal/2026-11-21.md), PR #56).
- Reading pinned data from a git cache → pinning ids is not integrity: git doesn't re-hash on
  read, replace refs remap blobs, and `git -C` walks up into the parent repo (or obeys an
  inherited `GIT_DIR`). Use `--git-dir`, `--no-replace-objects`, a scrubbed `GIT_*` env,
  `fsck --full` before reading, and `cat-file --batch-check` sizes before `--batch`
  ([journal](journal/2026-11-21.md), PR #62).
- Tests that edit generated PDFs: `String::from_utf8_lossy` changes byte lengths (binary header
  comment) and silently shifts offsets → edit `Vec<u8>` directly ([journal](journal/2026-11-20.md)).

## Process
- The guard blocks a Bash call whose text contains a GitHub CLI write it can't parse, even inside a
  heredoc or a chained command → write files with Write/Edit and run each CLI write as its own
  single command ([journal](journal/2026-11-20.md)).
- Any text naming the sealed exam manifest (even an issue body) is blocked by the guard, and any
  tool that reads it to "exclude" files would break CONSTITUTION 13 → refer to "the sealed
  corpus" only; overlap with it is the judge's concern ([journal](journal/2026-11-20.md)).
- Finishing a PR → post the `ai-review` status (it is a required check) before enabling
  auto-merge and writing the journal; a session that ends early otherwise leaves the PR blocked
  ([journal](journal/2026-11-20.md), PR #30).
- Right after a push, `gh pr diff` and `gh pr view --json headRefOid` can return the previous
  commit for a while → give the reviewer `git diff origin/main...HEAD`, and post `ai-review`
  on `git rev-parse HEAD` after checking it matches `git ls-remote` ([journal](journal/2026-11-20.md)).
- Branching from an unmerged PR branch → after it squash-merges, `git merge origin/main`
  gives add/add conflicts on the shared files; if main's tree equals the old PR head (`git
  diff <head> origin/main` empty), resolve with `git checkout --ours` ([journal](journal/2026-11-20.md), PR #32).
- The guard blocks force pushes → once a branch is pushed, bring in main with `git merge
  origin/main`, never rebase ([journal](journal/2026-11-20.md)).
- Creating numbered issues that reference each other: re-creating a blocked issue after later
  ones gives it a later number → create dependencies first, then check numbers before writing "Depends on: #N"
  ([journal](journal/2026-11-20.md)).
