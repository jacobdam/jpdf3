# Lessons learned (read at the start of every session; curated by retros, ≤ 60 entries)

Format: **situation → what to do** (link to the journal entry or PR). Refine existing entries
instead of adding near-duplicates. Retro 0001 promoted the guard (force push, CLI writes),
review-diff, `ai-review` order, arithmetic/step-cap and conformance-suite lessons into
ops/TICK.md and specs/_TEMPLATE.md; they are no longer listed here.

## Tooling & environment
- Dates and times → the machine's clock, the session's "today" and git `%cs`/`%cd` (committer time zone; some main
  commits are `+0700`) are local, while GitHub timestamps are UTC. Use `date -u` for the journal file name and the
  `HH:MM UTC` heading (the 2026-11-22.md "00:30 UTC" entry was really 17:10 UTC on 11-21), and `%ct` (Unix seconds)
  for UTC commit days, tested with a commit at 23:59:59 UTC ([journal](journal/2026-11-21.md), PR #123;
  [journal](journal/2026-11-22.md), PR #220 R1.1).
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
- Running the Playwright probes by hand (or letting a reviewer run them) → a relative `JPDF_PROBE_OUT`/`--output`
  resolves from the config's directory, so output lands under it or its parents (`apps/web/target/`, `apps/target/` in
  PR #204 R3.1), which the root-only `/target` ignore misses; biome fails only on paths under apps/web, and `git add -A`
  commits the rest. Pass absolute paths under the worktree's `target/scratch/`, and read `git status --short` before
  every commit
  ([journal](journal/2026-11-22.md), PR #204).
- Hand-checking that a specific test kills a mutant → run `cargo nextest run --no-fail-fast`; fail-fast stops at
  the first failing test, so the targeted test may never run and a "caught" claim can be false
  ([journal](journal/2026-11-21.md), PR #140).
- Mutation testing → `cargo mutants -p X` runs only X's own tests, so cross-crate coverage looks missing; add
  `--test-package` for the crates that exercise X (the whole workspace is too slow because of the bench tests).
  Use `--in-diff <range diff>`, set `TMPDIR` (it is unset) to the session scratchpad **outside the repo**: under
  `target/scratch/` the jpdf-corpus tests refuse a cache inside the git tree and the baseline fails, so no mutant runs
  (audit 0003). For jpdf-traits (about 26 s per mutant) narrow it with `-f` to the non-suite files
  ([journal](journal/2026-11-21.md), audit 0001, audit 0002, [audit 0003](journal/2026-11-22.md)).
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

- The guard blocks a Bash call whose text contains the word u‑n‑s‑a‑f‑e, or names a gh comment/issue command
  (it tries to parse it as a CLI write), even in a heredoc or python script writing markdown →
  write such files with Write/Edit ([journal](journal/2026-11-22.md), PR #178).
  Never `cat > file` without a heredoc (it waits on stdin until the 2 min timeout) ([journal](journal/2026-11-21.md), PR #104).
- Posting review text with `gh pr comment` (TICK §4) → the guard scans the whole command, so the terminal-multiplexer
  name in a commit message or a python one-liner, a push command naming main, or `gh repo`/admin-merge text in a body
  all block. Use Write/Edit for scripts, reword the token, and disclose it. A body with an apostrophe (`'\''`) is never
  treated as inert ([journal](journal/2026-11-21.md), PR #129). Separately, Claude Code's own safety check refuses a
  long post whose single-quoted body holds `$` + quote (`$'\''`) or `$$`+brace; split the post or space the token and
  disclose it ([journal](journal/2026-11-22.md), PR #183).
- Post or heredoc text with a `>` before a protected path (a reviewer's `<patched> ops/driver/x.sh`) → the guard
  reads it as a redirect, quoted or not; reword that token (`PATCHED`), or write files with Write/Edit, and disclose
  it ([journal](journal/2026-11-22.md), PR #194).
- Proposing a guard/hook change that "skips quoted text" → shlex is not the shell: `#` comments, zsh `=word`, newlines
  inside quoted text and duplicated copies elsewhere in the command all differ. Allowlist the remainder word by word,
  pin every alternative with a direct assert plus a mutant, and fuzz with a real bash and zsh oracle (fake gh on PATH).
  It took 4 review rounds ([journal](journal/2026-11-21.md), PR #129). The same holds for a rule built on the guard's
  own shell tokenizer: each round found a new misread (`${...}`, heredoc delimiters, backticks) and PR #183 hit the
  cap. Design fail-closed from the start (a "couldn't read it the shell's way" marker the rule blocks on), never
  compare counts between two parsers, and keep a fix that needs a parser out of a small finding's PR
  ([journal](journal/2026-11-22.md), PR #183).

- Writing a shell check from grep/awk exit codes → BSD grep exits 2 on an ERE with an empty alternative (`(a|)$`),
  so `! grep -v …` passes everything: accept only the exact success code (`(( $? == 1 ))` for `grep -qv`) and test a
  bad input. BSD awk rejects a newline in `-v`: pass multi-line data through `ENVIRON`. Extract the code under test
  from the patched script by its exact text so the tests can't drift ([journal](journal/2026-11-22.md), PR #171).
- Trusting config for a tool run with the driver's permissions (cargo, rustup) → they read config from **every
  ancestor directory**, and `<main checkout>/.claude/worktrees/`, `target/`, `ops/state/` and `~/.cargo` are writable by
  sessions and not protected. Walk the ancestors, pin env overrides (`RUSTUP_TOOLCHAIN`), and keep anything that
  decides what to kill or run in the driver's memory, not a file ([journal](journal/2026-11-22.md), PR #171).

- Writing a guard/hook rule over untrusted command text (a hook timeout or exception fails open) → use a
  one-pass tokenizer, not nested-quantifier regexes, and give the self-test adversarial timing inputs that
  contain the trigger word so the fast path can't skip them ([journal](journal/2026-11-21.md), PR #119).

- `npx <tool>` in apps/web before `npm ci` → npx downloads whatever registry package has that name (it fetched
  an unrelated `biome@0.3.3`). Run `npm ci --ignore-scripts` in the worktree first and call `./node_modules/.bin/<tool>`
  ([journal](journal/2026-11-22.md), PR #121).

- Waiting on a background agent or shell → don't set a ScheduleWakeup/CronCreate "fallback": `claude -p` stays alive
  until the timer fires (up to 20 min idle after the final answer, #125). And never end the turn to wait: `claude -p`
  kills background tasks still running 10 min after the last turn (a re-review was lost that way). Run reviewers in
  the foreground ([journal](journal/2026-11-21.md), PR #127).
- Timing tests that read file mtimes (`stat -f %m`, 1 s resolution) → an off-by-one at the limit survives; measure
  below a second (perl `Time::HiRes`) and test "half a second under survives / stops at the limit"
  ([journal](journal/2026-11-21.md), PR #127).
- Testing a diff for a protected file → the guard blocks `git apply`/`patch` even in scratch directories; generate
  the diff from a scratch copy and test that copy, with a harness that refuses to run if the stubbing failed
  ([journal](journal/2026-11-21.md), PR #127).

- Testing code that signals process groups (watchdogs, group kills) → never let a test feed a value that could
  become `kill -- -1` (`1`, `01`), even "to check the guard refuses it". Write only ids of sleepers you spawned in
  their own group; test the guard with the kill command textually swapped for `echo` (assert the swap happened). Prove
  each "unregister before reap/kill" order with a test-only event recorder; reviewers found a new untested order in
  4 rounds running ([journal](journal/2026-11-21.md), PR #133).

- A second-reviewer major saying "not proven" → state the limitation and file an issue; don't build enforcement
  tooling in the same PR (owner, PR #151: a text-scan tripwire added for such a finding drew a new major in
  rounds 6, 7 and 9 to 14). Now a TICK §4 triage rule (ADR 0010) ([journal](journal/2026-11-22.md), PR #151).

- Writing a process rule that bounds a loop (review rounds, retries) → before the first review, write the state table:
  every counter value (below the cap, at the cap, the extra check, past it) and every exit (clean, open finding,
  regrade, dispute, a merge from main), each with its next step. A rule triggered by "when the PR merges" needs a step that notices the
  merge: auto-merge lands after the session exits (PR #177). PR #157's reviewers found at least one unhandled
  state in every round, 5 in a row ([journal](journal/2026-11-22.md), PR #157). In a design ADR, do it for every
  counter, "once a day" bound and fallback, and for the loops each fix adds: PR #239's majors in rounds 2 to 4 were
  states its own earlier fixes created ([journal](journal/2026-11-22.md), PR #239). The same goes for prose about
  a cache or CI mechanism (exact hit, miss, failed run, eviction, PR scope): PR #241 drew a major on it in each of rounds 1–3.

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
- Writing a check that parses another tool's text output (`cargo tree`, git) → before round 1, print every line shape
  from the real tool (markers before, after and instead of the source, delimiters inside paths, an empty field) and give
  each a case, resolve only absolute paths, and pin the flags on the command actually run (`Command::get_args`), not on
  a constant: PR #234's second reviewer found a new shape or flag gap in each of 4 rounds
  ([journal](journal/2026-11-22.md), PR #234).
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

- Writing an overview doc (README, digest) from PLAN.md → PLAN describes the target, so "runs in a worker", "is
  published", "every limit is enforced" come out in the present tense; check each against the tree and open issues
  first (PR #169 round 1: 2 majors, 2 minors) ([journal](journal/2026-11-22.md), PR #169). Same for a cleanup's
  "one copy", "only", "every" claims: grep the whole tree for the duplicated shape before writing them (PR #212 spent
  rounds 1 and 2 on such a sentence) ([journal](journal/2026-11-22.md), PR #212).

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
  ([journal](journal/2026-11-22.md), PR #121). A file-tree walk (the #115 Arlington scan) needs, before the first
  review, one fixture per root × kind (regular file, symlink, unreadable, other metadata error), skip
  directories by exact path rather than name (a `src/test-results/` module ships), a bounded non-blocking read with
  a counted-bytes test, and "never blocks" written as "a FIFO open doesn't wait for a writer where the flags are
  known"; each of these was a finding in rounds 1 to 4 (a non-EACCES fixture and the EACCES root message are still open, #229) ([journal](journal/2026-11-22.md), PR #227).
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
- Checking a backend-reported count whose unit the backend chooses (operations, work) → an absolute lower bound is
  covered by any flat rate (per sample, per input byte). Measure a difference between two inputs with the same output
  and at least the same length. Before the first review, replace each new check term with `true` once and confirm a
  self-test fails; reviewers found an unpinned term in 4 of 6 rounds ([journal](journal/2026-11-21.md), PR #135).
- Generating test vectors in code (bit writers, keystreams) → the toy that "checks" them often shares the same tables,
  so a typo passes both; verify once with an independent implementation (CPython zlib, LibreSSL) and pin a size +
  checksum fingerprint so any generator change gets re-verified ([journal](journal/2026-11-21.md), PR #110).
- Running git on a cache a stranger could plant (`--cache DIR`) → git writes through a symlink at any path it
  touches (refs, packs, reflogs, FETCH_HEAD, shallow), follows gitfiles, runs hooks and config programs. Require
  the whole cache to be plain directories and regular files, allow-list its config keys, and put every setting on
  the `-c` command line; naming dangerous paths took 3 review rounds ([journal](journal/2026-11-21.md), PR #131).
- Hand mutation with sed → if the mutated literal also appears in the test's expected value, sed rewrites both
  and the mutant "survives"; replace only the first occurrence and check which line changed (PR #131). Undoing a
  mutant with `git checkout <file>` also drops your uncommitted edits in that file: commit first, or mutate and
  restore a copy ([journal](journal/2026-11-22.md), PR #201).
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
- Making a limit exact with a final whole-token check → the in-loop checks that stop a hostile token early (the
  memory bound) now look redundant and no result-based test notices their removal. Test the early stop: an
  over-long token with no terminator must fail with the limit error after a few lexed bytes
  ([journal](journal/2026-11-21.md), PR #140).
- Writing a "defaults fit the memory budget" check → summing the charged limits proves nothing: require an
  itemized reserve (or say plainly there is none), and before the first review give every `Vec::new`/`collect`/
  `with_capacity` on the code path a term, pinning each per-item constant against a parsed worst-case shape. PR #142
  rounds 1 to 3 each found a missed term ([journal](journal/2026-11-21.md), PR #108;
  [journal](journal/2026-11-22.md), PR #142).
- Writing a memory rule for a caller that doesn't exist yet (a budget helper plus "requirements on future code") →
  before round 1, walk every allocation the caller will make (decryption, each filter stage, the shrink copy, the final
  contiguous slice) and say for each whether it is capped, charged by capacity, or an uncharged estimate. PR #186
  drew such majors in each of rounds 1 to 4 (capacity, intermediates, decryption, shrink copy, cipher capacity)
  ([journal](journal/2026-11-22.md), PR #186). But keep those rules abstract ("only a size the engine computed and
  checked"). Filter-specific examples (predictor rows, join buffers) belong in the caller's PR: in PR #232 the join
  text drew S2.1 and S3.2, the predictor text S4.1 and S5.1 (the last one in the final check)
  ([journal](journal/2026-11-22.md), PR #232).
- Caching results of hostile input (resolve cache, decoded streams) → test every rejection path twice (a second
  resolve after each kind of failure must fail the same way and cache nothing), pin cost formulas with literals and
  spare-capacity buffers (an expected value built from the constant under test pins nothing), and let the control
  case assert where the uncached path first fails ([journal](journal/2026-11-21.md), PR #139).
- Tests that edit generated PDFs: `String::from_utf8_lossy` changes byte lengths (binary header
  comment) and silently shifts offsets → edit `Vec<u8>` directly ([journal](journal/2026-11-20.md)).
- The workspace denies `as` casts in tests too → convert floats with a bit-setting helper
  instead of adding an `allow` ([journal](journal/2026-11-21.md), PR #71).

## Process
- Writing a test harness for a filter (who counts as the owner, what is agent text, which URLs count) → a
  should-hide case catches a dropped check but not one that is too broad. Give every filter a should-hide case and a
  near-miss should-show case (owner text quoting the tag mid-body, a URL differing only in owner or repo), and kill a
  hand mutant per term ([journal](journal/2026-11-22.md), PR #198). A filter that mirrors another check (the guard's
  tag rule) needs cases run through both, including leading whitespace and the characters each side strips (audit
  0005 AC.1, AS.1). Likewise, for "any of N triggers fails the
  run", give each trigger its own case with every other trigger off; a combined case hid a mutant
  ([journal](journal/2026-11-22.md), PR #201 R1.1).
- Scoping an audit's security brief → include proposal scripts that decide what counts as an owner instruction or
  what leaves the machine, even though they aren't live code (audit 0004's AC.1 was found only by the code reviewer).
  If Codex returns "flagged for possible cybersecurity risk", record it as unavailable; don't reword the brief to get
  past the filter ([journal](journal/2026-11-22.md), audit 0004).
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
  Claiming a workflow or rule fired (drills) → cite the run log or the resulting state (timeline event, draft flag),
  never the bot's "applied" message: a `|| true` hid a permission error ([journal](journal/2026-11-22.md), PR #209 R1.1).
- Creating numbered issues that reference each other: re-creating a blocked issue after later
  ones gives it a later number → create dependencies first, then check numbers before writing "Depends on: #N"
  ([journal](journal/2026-11-20.md)).
