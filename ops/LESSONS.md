# Lessons learned (read at the start of every session; curated by retros, ≤ 60 entries)

Format: **situation → what to do** (link to the journal entry or PR). Refine existing entries
instead of adding near-duplicates. Retro 0001 promoted the guard (force push, CLI writes),
review-diff, `ai-review` order, arithmetic/step-cap and conformance-suite lessons into
ops/TICK.md and specs/_TEMPLATE.md; they are no longer listed here.

## Tooling & environment
- Journal date and time → the machine's clock and the session's "today" are local (UTC+7); use `date -u`
  for the journal file name and the `HH:MM UTC` heading (the 2026-11-22.md "00:30 UTC" entry was really
  17:10 UTC on 11-21) ([journal](journal/2026-11-21.md), PR #123).
- Scratch files (probe trees, throwaway scripts, downloads) → put them in the session scratchpad
  directory, never `/tmp` (owner instruction, #28). `$TMPDIR` is unset in the sandbox (so
  `$TMPDIR/x` is `/x`, read-only): use the git-ignored `target/scratch/` of your worktree
  ([journal](journal/2026-11-21.md), PR #93).
- Claude Code project settings and hooks apply **live** to running sessions → never write
  `.claude/settings.json` before its hook script exists and passes `--selftest` (bootstrap, 2026-11-20).
- PKCS#12 fixtures on macOS: `/usr/bin/openssl` is LibreSSL → always pass `-keypbe AES-256-CBC
  -certpbe AES-256-CBC -macalg sha256`; pyHanko `SimpleSigner.load_pkcs12(file, passphrase=...)`
  takes the password by keyword (the 2nd positional argument is the CA chain) (bootstrap).
- Scripted multi-line replacements (python `str.replace`) after `cargo fmt` → they fail
  because rustfmt reflowed the text. Re-read the formatted lines first, and make the script
  assert every match before writing anything ([journal](journal/2026-11-21.md), PR #67).
- macOS: no `timeout` binary (exit 127), system `python3` lacks `tomllib` → use
  `/opt/homebrew/bin/python3.14 -I` for judge scripts; colima/docker mounts only `$HOME`, so put
  files for containers there, not in `/tmp` ([journal](journal/2026-11-20.md)).
- Mutation testing → `cargo mutants -p X` runs only X's own tests, so cross-crate coverage looks missing; add
  `--test-package` for the crates that exercise X (the whole workspace is too slow because of the bench tests).
  Use `--in-diff <range diff>`, set `TMPDIR=$PWD/target/scratch/…` (it is unset), and for jpdf-traits (about 26 s per
  mutant) narrow it with `-f` to the non-suite files ([journal](journal/2026-11-21.md), audit 0001, audit 0002).
- Judge-like memory checks: macOS RSS ≠ address space → run the Linux build in docker
  (`rust:<pinned>`) under `ulimit -v 2097152` ([journal](journal/2026-11-20.md)).
- `reference.py` on the full smoke corpus takes ~4 min locally; one pdf.js file hangs MuPDF
  (> 60 s) → reference tools must be called with timeouts (bootstrap).
- Timing tests with shell-script fake engines → on macOS the first exec of a freshly written
  script can take ~2 s; give timed assertions a warmup run and generous upper bounds; when a
  test needs the script to reach a point before a deadline, have it signal that point (atomic
  rename of a marker) and retry with a longer limit if it didn't (#75, PR #103); and
  test process-group kills only from Rust (a stray `kill -- -PGID` in the tool shell kills
  the session's shell) ([journal](journal/2026-11-21.md), PR #59).

- The guard blocks a Bash call whose text contains the word u‑n‑s‑a‑f‑e, even in a heredoc writing markdown →
  write such files with Write/Edit. Never `cat > file` without a heredoc (it waits on stdin until the 2 min
  timeout) ([journal](journal/2026-11-21.md), PR #104).

- Writing a guard/hook rule over untrusted command text (a hook timeout or exception fails open) → use a
  one-pass tokenizer, not nested-quantifier regexes, and give the self-test adversarial timing inputs that
  contain the trigger word so the fast path can't skip them ([journal](journal/2026-11-21.md), PR #119).

- `npx <tool>` in apps/web before `npm ci` → npx downloads whatever registry package has that name (it fetched
  an unrelated `biome@0.3.3`). Run `npm ci --ignore-scripts` in the worktree first and call `./node_modules/.bin/<tool>`
  ([journal](journal/2026-11-22.md), PR #121).

## Judge & CI
- MuPDF and Poppler agree (≤ 2% pixels differ at 72 dpi after 2× downscale) on ~78% of
  smoke pages; only those consensus pages are ratcheted (bootstrap calibration).
- A skipped required job counts as success on GitHub → the verdict job runs `if: always()` and
  fails unless every upstream job succeeded (bootstrap).
- main red with `judge-verdict` green, only `pages` failed (deploy-pages "No artifacts named
  github-pages") **and** `upload-pages-artifact` succeeded in the same attempt → infra, no code
  change. Don't rerun: one rerun worked (#39), another hit "Multiple artifacts named
  github-pages" and could never deploy (#59). The next push to main redeploys. If the upload
  step failed or was skipped, it's a real bug. Durable fix proposed in
  `ops/proposals/pages-deploy-retry.md` ([journal](journal/2026-11-21.md), PR #63).
- Writing a local check that mirrors a judge script → match its Python regex semantics
  exactly (`\s` crosses newlines and includes U+001C–U+001F; pathlib globs match `.md`
  files and directories), and diff both side by side on probe trees before review
  ([journal](journal/2026-11-20.md), PR #27).
- Adding a fuzz target → the judge's nightly step runs `cargo fuzz run` with no seeds (33
  edges from an empty corpus), so give each target a structure template like
  `jpdf_testgen::from_fuzz_chunks` (about 1000 edges), not just seeds in `xtask/src/fuzz.rs`
  ([journal](journal/2026-11-21.md), PR #58).

## Specs & journeys
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
- Writing signature or network journeys → give approval signatures a default change policy
  (shadow attacks are legal edits), keep certificate-derived URLs off loopback/private
  addresses, check web limits (CORS, mixed content, CSP, `fetch` redirect modes) and macOS ATS
  before promising a request, and draw the fixture PKI's expiry timeline before writing
  clock steps ([journal](journal/2026-11-20.md), PR #46).
- Writing a spec that restates journeys (UI spec, feature spec) → quote the journeys' exact UI
  strings (empty states, option names, page-field format, notice wording) instead of
  paraphrasing, and check new web keys against `specs/ui/shortcuts.md` rule 3 (the
  browser-reserved list) ([journal](journal/2026-11-20.md), PR #40).

## Apps (web, macOS)
- Writing a check that "nothing from set X reaches the artifact" (licenses, imports) → a text
  scan leaks through every indirection: paths into node_modules, `../` out of the scanned
  directory, symlinks (resolved from their real path), `#` subpath imports, bundler aliases.
  Fail closed on all of them up front, and prove the real property on the resolved graph:
  the bundler's module graph (#99), or for Rust `cargo tree --workspace -e normal,build -i
  <crate>` (a manifest grep misses `[dependencies.x]` tables and transitive paths)
  ([journal](journal/2026-11-21.md), PR #98, PR #102). Every decoding layer is a bypass too:
  JS escapes and line continuations, HTML entities, `%`, quotes of the other kind, line
  terminators other than LF (CR, U+2028/9), plus `data:`/`file:` modules and config files the
  tool loads on its own (tsconfig, PostCSS, up the tree). Allow-list characters and file
  shapes instead of decoding or deny-listing; the #112 review took 3 rounds
  ([journal](journal/2026-11-22.md), PR #121).
- Drawing wireframes or naming UI components → check each component's real constraints before
  writing sizes or behavior: MUI small fields are 40 px, `Tab` has `minWidth` 90, no focusable
  control inside `role="tab"`, temporary drawers are modal; SwiftUI `.disabled()` drops out of
  Full Keyboard Access, NSDocument autosave writes silently, the macOS split view has no
  overlay sidebar ([journal](journal/2026-11-20.md), PR #44).
- Choosing npm packages (web app, test tools) → run `npm run licenses` in apps/web on the
  candidate tree first: CONSTITUTION 10 has no build-only exemption, and Vite 8, ESLint,
  Babel-based plugins and axe-core all fail it ([journal](journal/2026-11-20.md), PR #49).
- Asserting styles in Playwright → use a `toHaveCSS` assertion, not your eye (MUI 9 ignores
  Typography `color`); after a pointer or focus action, await `el.getAnimations()` first (MUI
  transitions take 250 ms). Check a disabled control's hover and focused-hover states: an sx
  `&:hover` override ties with the theme's focus-visible rule and wins, so set only the
  property you mean ([journal](journal/2026-11-20.md), PR #49).
- macOS XCUITest → Automation Mode must work without a password: locally that needs the
  owner's sudo (#52), GitHub runners have it; `project-macos` skips PRs, so verify UI tests
  with `gh workflow run ci.yml --ref <branch>` before merging. Query SwiftUI elements by
  `accessibilityIdentifier` + `.element` (the title bar is a static text too; `firstMatch`
  failed with count 1), read static text from `value` when `label` is empty; check
  entitlements on Release only (Debug test builds inject read access to `/`)
  ([journal](journal/2026-11-20.md), PR #51).

## Engine & security
- Parsing hostile input: per-object caps don't bound a document (overlapping objects/xref
  sections re-parse the same bytes quadratically; several capped objects alive at once exceed
  2 GiB) → design it in from the start: charge a per-document lexing budget per byte, keep a per-document object budget, reject overlapping
  objects by byte span, keep one parsed object alive at a time, and store xref entries
  compactly ([journal](journal/2026-11-20.md), PR #24).
- Writing a conformance suite (details beyond the template's self-check) → lenient-read rules
  need a positive salvage check; every limit needs a per-axis/per-path boundary; compare whole
  results, not summaries (full outline segments, not count + bounding box); make tolerances
  relative or capped (an absolute 0.01 accepted any font matrix); count coverage from what
  the backend actually loaded (format, a reachable control path), never from fixture labels
  and never from the backend's pass/fail verdict (a failing backend would also fail
  coverage); check promised empty answers (`None`, empty lists) too; and keep toy
  implementations linear (an O(n) count inside a growth loop turned one test into 91 s).
  Decode options: a "flip the option, the result must differ" check proves only *different*
  (an error or a blank image differs too), so require fixtures where the explicit value overrides the
  implied one in every direction, and cap tolerances independently of the sample depth
  ([journal](journal/2026-11-21.md), PR #56, PR #67, PR #93, PR #123, audit 0001).
- Generating test vectors in code (bit writers, keystreams) → the toy that "checks" them often shares the same tables,
  so a typo passes both; verify once with an independent implementation (CPython zlib, LibreSSL) and pin a size +
  checksum fingerprint so any generator change gets re-verified ([journal](journal/2026-11-21.md), PR #110).
- Reading pinned data from a git cache → pinning ids is not integrity: git doesn't re-hash on
  read, replace refs remap blobs, and `git -C` walks up into the parent repo (or obeys an
  inherited `GIT_DIR`). Use `--git-dir`, `--no-replace-objects`, a scrubbed `GIT_*` env,
  `fsck --full` before reading, and `cat-file --batch-check` sizes before `--batch`
  ([journal](journal/2026-11-21.md), PR #62).
- Guarding "never write inside the repo" from a tool built in a worktree → the repo root
  is `.claude/worktrees/<x>`, which sits inside the main checkout. Refuse any path with an
  enclosing `.git`, check before creating directories, and re-check subdirectories for
  symlinks. Write new manifest/data parsers against the judge's real files first (smoke
  lists one blob twice) ([journal](journal/2026-11-21.md), PR #65).
- Proving "rejected before reading" (size, type) without measuring memory → make the test file mode 0200: an
  implementation that opens first fails with EACCES instead of the limit error. Skip only that half as root.
  FIFO/device inputs need a hang timeout in the test, not a plain `output()` ([journal](journal/2026-11-21.md), PR #104).
- Writing a "defaults fit the memory budget" check → summing the charged limits to exactly the 2 GiB cap proves
  nothing: list the uncharged terms (binary, stacks, xref table, one parse's objects, decoder slack) and require a
  reserve, or say plainly that there is none ([journal](journal/2026-11-21.md), PR #108, #109).
- Tests that edit generated PDFs: `String::from_utf8_lossy` changes byte lengths (binary header
  comment) and silently shifts offsets → edit `Vec<u8>` directly ([journal](journal/2026-11-20.md)).
- The workspace denies `as` casts in tests too → convert floats with a bit-setting helper
  instead of adding an `allow` ([journal](journal/2026-11-21.md), PR #71).

## Process
- Any text naming the sealed exam manifest (even an issue body) is blocked by the guard, and any
  tool that reads it to "exclude" files would break CONSTITUTION 13 → refer to "the sealed
  corpus" only; overlap with it is the judge's concern ([journal](journal/2026-11-20.md)).
- Branching from an unmerged PR branch → after it squash-merges, `git merge origin/main`
  gives add/add conflicts on the shared files; if main's tree equals the old PR head (`git
  diff <head> origin/main` empty), resolve with `git checkout --ours` ([journal](journal/2026-11-20.md), PR #32).
- Finding "the last X" from git (last audit, last retro) → `git log -1 --diff-filter=A` with an
  exact filename pattern (`[0-9][0-9][0-9][0-9].md`); the plain last-touch commit resets on any
  typo fix ([journal](journal/2026-11-21.md), PR #90).
- Writing a record (retro, audit, journal) that claims GitHub side effects (labels, new issues)
  → do them before the review, or the record is false when it merges ([journal](journal/2026-11-21.md), PR #90).
- Creating numbered issues that reference each other: re-creating a blocked issue after later
  ones gives it a later number → create dependencies first, then check numbers before writing "Depends on: #N"
  ([journal](journal/2026-11-20.md)).
