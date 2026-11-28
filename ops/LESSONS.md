# Lessons learned (read at the start of every session; curated by retros, ≤ 60 entries)

Format: **situation → what to do** (link to the journal entry or PR). Refine existing entries
instead of adding near-duplicates. Retro 0001 promoted the guard (force push, CLI writes),
review-diff, `ai-review` order, arithmetic/step-cap and conformance-suite lessons into
ops/TICK.md and specs/_TEMPLATE.md; they are no longer listed here.
Retro 0002 promoted the checker/filter/parser/loop-rule lessons into the specs/_TEMPLATE.md self-check and
dropped entries TICK §0 and §4 already state (background waits, "not proven" findings, post triggers).
Retro 0003 promoted the sizing-after-rustfmt lesson into ops/TICK.md §0 and the "walk a rule over GitHub state
through every case before round 1" lessons (PRs #289, #312, #479, #524, #528) into ops/TICK.md §3 step 2, and
dropped stale or restated entries (judge calibration, the skipped-job verdict, settings bootstrap, "last X" from git,
the #25 harness size formula, and the #317 filter-bucket test, which the template's "replace each term with `true`" states).

## Tooling & environment
- A merge of origin/main that brings a protected-path change from main → the guard blocks the merge, and once it
  stops on conflicts every Bash call (even `git add`/`git commit`); `git merge --abort`, then either
  `git merge origin/main -X theirs` and re-apply your side in a new commit, or set the conflicting record file to
  main's copy exactly (appended lines next to main's edits still conflict), push, `gh pr update-branch N`, and re-add
  your record lines after it ([journal](journal/2026-11-27/0048-353-dashboard-page.md),
  [journal](journal/2026-11-27/2300-347-design-window.md)).
- Dates and times → the machine's clock, the session's "today" and git `%cs`/`%cd` (committer time zone; some main
  commits are `+0700`) are local, while GitHub timestamps are UTC. Use `date -u` for the journal file name and the
  `HH:MM UTC` heading (the 2026-11-22.md "00:30 UTC" entry was really 17:10 UTC on 11-21), and `%ct` (Unix seconds)
  for UTC commit days, tested with a commit at 23:59:59 UTC ([journal](journal/2026-11-21.md), PR #123;
  [journal](journal/2026-11-22.md), PR #220 R1.1).
- Scratch files (probe trees, throwaway scripts, downloads) → put them in the session scratchpad
  directory, never `/tmp` (owner instruction, #28). `$TMPDIR` is unset in the sandbox (so
  `$TMPDIR/x` is `/x`, read-only): use the git-ignored `target/scratch/` of your worktree
  ([journal](journal/2026-11-21.md), PR #93).
- Scripted edits → after `cargo fmt`, multi-line `str.replace` scripts fail because rustfmt reflowed the text: re-read
  the formatted lines and make the script assert every match before writing ([journal](journal/2026-11-21.md),
  PR #67). An inline `python3 -c "..."` whose strings hold Markdown backticks → zsh runs them as command
  substitutions (an empty match still "succeeded" once and duplicated a line): write the script to a file with a
  quoted heredoc and run that ([journal](journal/2026-11-28/0617-236-memory-adr.md)).
- macOS/BSD shell → no `timeout` binary (exit 127); system `python3` lacks `tomllib`, so use
  `/opt/homebrew/bin/python3.14 -I` for judge scripts; colima/docker mounts only `$HOME`
  ([journal](journal/2026-11-20.md)); `cargo` may be off PATH (`export PATH=$HOME/.cargo/bin:$PATH`,
  [journal](journal/2026-11-25/1750-301-nightly-bench.md)). BSD grep exits 2 on an ERE with an empty alternative
  (`(a|)$`), so `! grep -v …` passes everything: accept only the exact success code (`(( $? == 1 ))` for `grep -qv`)
  and test a bad input; BSD awk rejects a newline in `-v`: pass multi-line data through `ENVIRON`. Extract the code
  under test from the patched script by its exact text so the tests can't drift ([journal](journal/2026-11-22.md), PR #171).
- Running the Playwright probes by hand (or letting a reviewer run them) → a relative `JPDF_PROBE_OUT`/`--output`
  resolves from the config's directory, so output lands under it or its parents (`apps/web/target/`, `apps/target/` in
  PR #204 R3.1), which the root-only `/target` ignore misses; biome fails only on paths under apps/web, and `git add -A`
  commits the rest. Pass absolute paths under the worktree's `target/scratch/`, and read `git status --short` before
  every commit ([journal](journal/2026-11-22.md), PR #204).
- Mutation testing → `cargo mutants -p X` runs only X's own tests, so cross-crate coverage looks missing; add
  `--test-package` for the crates that exercise X (the whole workspace is too slow because of the bench tests).
  Use `--in-diff <range diff>`, set `TMPDIR` (it is unset) to the session scratchpad **outside the repo**: under
  `target/scratch/` the jpdf-corpus tests refuse a cache inside the git tree and the baseline fails, so no mutant runs
  (audit 0003). For jpdf-traits (about 26 s per mutant) narrow it with `-f` to the non-suite files. Checking by hand
  that a test kills a mutant → `cargo nextest run --no-fail-fast` (fail-fast may never run the targeted test, PR #140).
  Hand mutants with sed: if the mutated literal also appears in the expected value, sed rewrites both, so replace only
  the first occurrence and check which line changed (PR #131); undoing with `git checkout <file>` drops your
  uncommitted edits (commit first, PR #201); a file restored with `mv` keeps its old mtime, so `touch` it before any
  live run ([journal](journal/2026-11-21.md), audits 0001–0003, [journal](journal/2026-11-24/0150-295-hold-skip.md)).
- Running judge tools locally → macOS RSS ≠ address space, so run memory checks on the Linux build in docker
  (`rust:<pinned>`) under `ulimit -v 2097152`; `reference.py` on the full smoke corpus takes ~4 min and one pdf.js
  file hangs MuPDF (> 60 s), so call reference tools with timeouts ([journal](journal/2026-11-20.md), bootstrap).
- Timing tests → with shell-script fake engines the first exec of a freshly written script on macOS can take ~2 s:
  give timed assertions a warmup run and generous upper bounds, and when a test needs the script to reach a point
  before a deadline, have it signal that point (atomic rename of a marker) and retry with a longer limit if it didn't
  (#75, PR #103). File mtimes (`stat -f %m`) have 1 s resolution, so an off-by-one at the limit survives: measure
  below a second (perl `Time::HiRes`) and test "half a second under survives / stops at the limit" (PR #127).
  Process groups: test group kills only from Rust (a stray `kill -- -PGID` in the tool shell kills the session's
  shell); never let a test feed a value that could become `kill -- -1` (`1`, `01`), even "to check the guard refuses
  it": write only ids of sleepers you spawned in their own group, test the guard with the kill command textually
  swapped for `echo` (assert the swap happened), and prove each "unregister before reap/kill" order with a test-only
  event recorder (PR #133) ([journal](journal/2026-11-21.md), PR #59).
- The guard blocks a Bash call whose text contains the word u‑n‑s‑a‑f‑e, or names a gh comment/issue command
  (it tries to parse it as a CLI write), even in a heredoc or python script writing markdown →
  write such files with Write/Edit ([journal](journal/2026-11-22.md), PR #178). The post triggers are listed in TICK
  §4's mechanics; the terminal-multiplexer name also blocks in a commit message or a python one-liner; a body with an apostrophe (`'\''`) is never treated as inert, and Claude Code's own check refuses a
  long post whose single-quoted body holds `$` + quote or `$$`+brace: split it or space the token and disclose it
  ([journal](journal/2026-11-21.md), PR #129; [journal](journal/2026-11-22.md), PR #183).
  Never `cat > file` without a heredoc (it waits on stdin until the 2 min timeout) ([journal](journal/2026-11-21.md), PR #104).
- Protected files outside the protected paths → the guard blocks `git apply`/`patch` of a protected file's diff even
  in scratch directories (generate the diff from a scratch copy and test that copy, with a harness that refuses to run
  if the stubbing failed, PR #127), and `cp` with a protected path as the source (write copies with Write or from the
  tool itself) ([journal](journal/2026-11-21.md), [journal](journal/2026-11-27/2223-303-admit.md)).
- Proposing a guard/hook change that "skips quoted text" → shlex is not the shell: `#` comments, zsh `=word`, newlines
  inside quoted text and duplicated copies elsewhere in the command all differ. Allowlist the remainder word by word,
  pin every alternative with a direct assert plus a mutant, and fuzz with a real bash and zsh oracle (fake gh on PATH)
  ([journal](journal/2026-11-21.md), PR #129). A rule built on the guard's own shell tokenizer drew a new misread each
  round (`${...}`, heredoc delimiters, backticks; PR #183 hit the cap): design it fail-closed (a "couldn't read it the
  shell's way" marker the rule blocks on), never compare counts between two parsers, and keep a parser-shaped fix out
  of a small finding's PR ([journal](journal/2026-11-22.md), PR #183). The hook fails open on a timeout or exception:
  use a one-pass tokenizer, not nested-quantifier regexes, and self-test adversarial timing inputs that contain the
  trigger word ([journal](journal/2026-11-21.md), PR #119).
- Trusting config for a tool run with the driver's permissions (cargo, rustup) → they read config from **every
  ancestor directory**, and `<main checkout>/.claude/worktrees/`, `target/`, `ops/state/` and `~/.cargo` are writable by
  sessions and not protected. Walk the ancestors, pin env overrides (`RUSTUP_TOOLCHAIN`), and keep anything that
  decides what to kill or run in the driver's memory, not a file ([journal](journal/2026-11-22.md), PR #171).
- Shell tests: GNU `touch -d '2 days 23 hours ago'` sets a future time, so an age-boundary case passes every mutant →
  use `touch -d "@$(( $(date +%s) - <seconds> ))"` and check it with `stat` once
  ([journal](journal/2026-11-25/1636-342-keep-open-worktrees.md)). Both forms are GNU-only: macOS `touch` rejects
  them, so a driver test (the driver runs on the Mac mini) must also run there. Use POSIX `TZ=UTC0 touch -t` with a
  stamp from `date -u -d @<epoch>` (GNU) or `date -u -r <epoch>` (BSD), in that order, and read each mtime back
  (`stat -c %Y`, then `stat -f %m`) so a date that didn't take stops the run, as
  ops/proposals/driver-keep-open-worktrees/test.sh's `aged` does; check on Linux with `colima start` and a
  `debian:trixie` container (#366, [journal](journal/2026-11-28/0041-366-touch-portable.md)).
- GitHub: during an outage `gh issue comment` fails with a bare "GraphQL: Something went wrong" while reads work →
  retry in one loop that first checks whether the comment already exists, so nothing is posted twice
  ([journal](journal/2026-11-25/1600-311-done-reply.md)).
- Running apps/web checks on the Linux host (no Node, Chromium exits 127, text boxes "hidden") → fetch the Node 24
  tarball and its SHASUMS256.txt from nodejs.org into `target/scratch/` and check the sum; `apt-get download` the
  libraries `ldd` reports missing plus `fonts-dejavu-core`, `dpkg-deb -x` them there, and run Playwright with
  `LD_LIBRARY_PATH` and a `FONTCONFIG_FILE` naming that font directory; without fonts every visibility assertion
  fails ([journal](journal/2026-11-25/1830-305-dashboard.md)).
- Running `cargo vet` on a crate the repo doesn't use yet (admission pre-checks) → `--locked` reads the repo's
  `imports.lock`, which vet prunes to the audits the current graph needs, so it reports every new crate unaudited; copy
  the store into a scratch probe and run vet without `--locked` there ([journal](journal/2026-11-27/2223-303-admit.md)).
- A change that makes an xtask command write under the main checkout (found from the git common dir) → every test, and every hand mutant you run, must run in a fresh `git init` repo with `GIT_CEILING_DIRECTORIES`, or it writes the real checkout's file; check the real file after mutants ([journal](journal/2026-11-28/0945-481-pick-log.md)).

## Judge & CI
- main red with `judge-verdict` green, only `pages` failed (deploy-pages "No artifacts named
  github-pages") **and** `upload-pages-artifact` succeeded in the same attempt → infra, no code
  change. Don't rerun: one rerun worked (#39), another hit "Multiple artifacts named
  github-pages" and could never deploy (#59). The next push to main redeploys. If the upload
  step failed or was skipped, it's a real bug. Durable fix proposed in
  `ops/proposals/pages-deploy-retry.md` ([journal](journal/2026-11-21.md), PR #63). Exception: deploy-pages got
  HTTP 500 and main's head has no newer push → `gh run rerun --failed` worked (run 45649071740)
  ([journal](journal/2026-11-25/1620-main-red-pages.md)). Any infra job red while a newer main `ci` run is in
  progress → don't `rerun --failed`: it queues in the main concurrency group, and a later push before it starts
  cancels it (run 45249100863 attempt 2 ran no job); read the newer run's result for the same job instead, and record
  the run's state after acting ([journal](journal/2026-11-23.md), PR #271).
- Mirroring or reading a judge script in a tool, check or page → match its Python regex semantics exactly (`\s`
  crosses newlines and includes U+001C–U+001F; pathlib globs match `.md` files and directories) and diff both side by
  side on probe trees before review (PR #27); a Rust check that mirrors a GitHub Actions `if:` expression folds case
  (`startsWith`/`contains` ignore it, PR #296 R1.1). Read the writer in ops/judge/ for every value each field can take,
  not one downloaded sample: verdict.py writes journey `pass: None` when a platform has no e2e results (push runs
  without macOS), and nightlies also run on `workflow_dispatch` (PR #370 R1.1, R1.6). A "rejects over the cap" test
  must use otherwise valid input, or the parse error passes it (R2.1) ([journal](journal/2026-11-20.md),
  [journal](journal/2026-11-24/0150-295-hold-skip.md), [journal](journal/2026-11-27/0106-306-judge-metrics.md)).
- Adding a fuzz target → the judge's nightly step runs `cargo fuzz run` with no seeds (33
  edges from an empty corpus), so give each target a structure template like
  `jpdf_testgen::from_fuzz_chunks` (about 1000 edges), not just seeds in `xtask/src/fuzz.rs`
  ([journal](journal/2026-11-21.md), PR #58).
- Proposing a `supply-chain/`, `deny.toml` or other pinned-config change → the judge's `policy` job copies those files from the base commit before it runs, so a PR's own copy is never checked: the owner's change lands on main first, and a dependent PR must be based on it (PR #444 R1.2, [journal](journal/2026-11-28/0304-304-backend-selection.md)).

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
  clock steps ([journal](journal/2026-11-20.md), PR #46). PKCS#12 fixtures on macOS: `/usr/bin/openssl` is LibreSSL,
  so pass `-keypbe AES-256-CBC -certpbe AES-256-CBC -macalg sha256`; pyHanko `SimpleSigner.load_pkcs12(file,
  passphrase=...)` takes the password by keyword (the 2nd positional argument is the CA chain) (bootstrap).
- Writing a spec that restates journeys (UI spec, feature spec) → quote the journeys' exact UI
  strings (empty states, option names, page-field format, notice wording) instead of
  paraphrasing, and check new web keys against `specs/ui/shortcuts.md` rule 3 (the
  browser-reserved list) ([journal](journal/2026-11-20.md), PR #40). A review fix that moves a journey check to new
  UI elements → check that each element is on screen at that step (specs/ui/information-architecture.md), and on
  macOS whether the system or the app draws it ([journal](journal/2026-11-27/1318-334-languages.md), R2.1, R3.1).
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
  candidate tree first: Vite 8, ESLint, Babel-based plugins and axe-core failed it before CONSTITUTION 10's
  test/build-tool list ([journal](journal/2026-11-20.md), PR #49). `npx <tool>` before `npm ci` downloads whatever
  registry package has that name (it fetched an unrelated `biome@0.3.3`): run `npm ci --ignore-scripts` in the
  worktree first and call `./node_modules/.bin/<tool>` ([journal](journal/2026-11-22.md), PR #121).
- Asserting styles in Playwright → use a `toHaveCSS` assertion, not your eye (MUI 9 ignores
  Typography `color`); after a pointer or focus action, await `el.getAnimations()` first (MUI
  transitions take 250 ms). Check a disabled control's hover and focused-hover states: an sx
  `&:hover` override ties with the theme's focus-visible rule and wins, so set only the
  property you mean ([journal](journal/2026-11-20.md), PR #49).
- macOS XCUITest → Automation Mode must work without a password: locally that needs the
  owner's sudo (#52), GitHub runners have it; `project-macos` skips PRs and runs no Rust tests
  (macOS-only Rust paths are checked locally only), so verify UI tests
  with `gh workflow run ci.yml --ref <branch>` before merging. Query SwiftUI elements by
  `accessibilityIdentifier` + `.element` (the title bar is a static text too; `firstMatch`
  failed with count 1), read static text from `value` when `label` is empty; check
  entitlements on Release only (Debug test builds inject read access to `/`)
  ([journal](journal/2026-11-20.md), PR #51).

## Engine & security
- A bounded-buffer claim ("at most cap + one chunk") → before round 1, state the allocation (reserve once, so capacity
  can't outgrow length), the failed-reservation path (no further heap allocation) and the cancel path; Codex found one
  of each in rounds 2–4 of PR #362 ([journal](journal/2026-11-26/2333-322-stderr-tail.md)).
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
  and at least the same length ([journal](journal/2026-11-21.md), PR #135).
- Generating test vectors or fixtures in code (bit writers, keystreams, edited PDFs) → the toy that "checks" them often
  shares the same tables, so a typo passes both: verify once with an independent implementation (CPython zlib,
  LibreSSL) and pin a size + checksum fingerprint so any generator change gets re-verified (PR #110). Edit generated
  PDFs as `Vec<u8>`: `String::from_utf8_lossy` changes byte lengths (binary header comment) and silently shifts
  offsets ([journal](journal/2026-11-21.md), [journal](journal/2026-11-20.md)).
- Running git on a cache a stranger could plant (`--cache DIR`), or reading pinned data from it → git writes through a
  symlink at any path it touches (refs, packs, reflogs, FETCH_HEAD, shallow), follows gitfiles, runs hooks and config
  programs: require the whole cache to be plain directories and regular files, allow-list its config keys, and put
  every setting on the `-c` command line (naming dangerous paths took 3 review rounds, PR #131). Pinning ids is not
  integrity: git doesn't re-hash on read, replace refs remap blobs, and `git -C` walks up into the parent repo (or
  obeys an inherited `GIT_DIR`); use `--git-dir`, `--no-replace-objects`, a scrubbed `GIT_*` env, `fsck --full` before
  reading, and `cat-file --batch-check` sizes before `--batch` (PR #62) ([journal](journal/2026-11-21.md)).
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
  ([journal](journal/2026-11-21.md), PR #140). The lexing budget is checked only before each token, so a parse that
  answers from its last token (a look-ahead that classifies bytes, an early `Unsupported` or `Malformed`) leaves those
  bytes unchecked: check the budget once before every answer, on all exit paths at once, with an exact-boundary test
  per path (PR #540 patched one path per round: S2.1, then S3.1,
  [journal](journal/2026-11-28/1448-78-lenient-read.md)).
- Writing a memory budget (a "defaults fit" check, or a rule for a caller that doesn't exist yet) → summing the
  charged limits proves nothing: require an itemized reserve (or say plainly there is none), and before round 1 walk
  every allocation on the path (`Vec::new`/`collect`/`with_capacity`, decryption, each filter stage, the shrink copy,
  the final contiguous slice), saying for each whether it is capped, charged by capacity, or an uncharged estimate,
  and pin each per-item constant against a parsed worst-case shape. PR #142 rounds 1 to 3 and PR #186 rounds 1 to 4
  each found a missed term. Keep a rule for a future caller abstract ("only a size the engine computed and checked"):
  filter-specific examples (predictor rows, join buffers) belong in the caller's PR, where PR #232's drew S2.1 to
  S5.1 ([journal](journal/2026-11-21.md), PR #108; [journal](journal/2026-11-22.md), PR #142, PR #186, PR #232).
- Caching results of hostile input (resolve cache, decoded streams) → test every rejection path twice (a second
  resolve after each kind of failure must fail the same way and cache nothing), pin cost formulas with literals and
  spare-capacity buffers (an expected value built from the constant under test pins nothing), and let the control
  case assert where the uncached path first fails ([journal](journal/2026-11-21.md), PR #139).
- Workspace lints in tests → clippy's `allow-expect-in-tests` covers `#[test]` functions only, not helpers of an
  integration test file, so write those helpers to return `io::Result` and use `?`
  ([journal](journal/2026-11-24/0309-246-issue-lint.md)); `as` casts are denied in tests too, so convert floats with a
  bit-setting helper instead of adding an `allow` ([journal](journal/2026-11-21.md), PR #71).
- A reviewer subagent that builds a scratch copy of the repo can leave stale binaries in the worktree's `target/`
  (`cannot run xtask`, fixture paths under the copy) → `cargo clean -p <crate>`, and tell reviewers to use their own
  `CARGO_TARGET_DIR` ([journal](journal/2026-11-24/0430-275-step6.md)).

## Process
- A test that passes for the wrong reason (beyond the template's "replace each term with `true`") → a loop that writes
  each bad case to one fixture path and runs a later step after the loop checks only the last case: run every step
  inside the loop (PR #506 R1.1); a near-miss fixture made with a broad string `replace` (an author login) can also
  change another field (a URL holding the login): replace the exact JSON field and check with a hand mutant
  (#373); a "step X fails → tool failure" test whose blocking state also breaks a later step passes without X's check:
  choose a state that fails only X, or have the function say which step failed (PR #351 R2.1); a claim that a fixture
  keeps N flags independent: compute each word's set of fixture items, opposites included, by script and run one swap
  mutant per pair (PR #393 took three rounds by eye) ([journal](journal/2026-11-28/0959-500-unknown-schema.md),
  [journal](journal/2026-11-27/0901-373-design-window.md), [journal](journal/2026-11-25/1750-301-nightly-bench.md),
  [journal](journal/2026-11-27/1501-390-status.md)).
- Writing a test harness for a filter or an issue-filing tool (beyond the template's self-check) → a filter that
  mirrors another check (the guard's tag rule) needs cases run through both, including leading whitespace and the
  characters each side strips (audit 0005 AC.1, AS.1). A fake CLI must run the script's real arguments (build the JSON
  from the requested `--json` fields and apply its `--jq` with jq), and every guard needs a case per command
  ([journal](journal/2026-11-23.md), PR #264 R1.1, R1.2). Assert each shown record's body and the exact output set,
  not membership (PR #265, CX.7). A "newest N by time" test needs a fixture where time order differs from id order,
  list order and reversed list order (PR #267). A tool that files or updates issues across nights (dedup by a key)
  needs a two-nights-over-the-cap test (a shared write cap spent on updates starves new findings: create first) and a
  finding whose label changes between nights (PR #384 R1.1, R1.2,
  [journal](journal/2026-11-27/1405-302-nightly-issues.md)).
- A scheduler or tool input built live from command output (git reads) and parsed later → put the filtering in a
  pure function over the command text with a fake reader, and decide per failure (bad bytes, oversize) whether it
  skips, exits 1 or exits 2, before round 1 (PR #285 R1.1: [journal](journal/2026-11-23/2225-243-step11.md)).
- Building something for the Pages site from CI data → the `project-linux` job that runs `cargo xtask ci linux` has
  no GitHub token and a shallow checkout (no `persist-credentials`, no `fetch-depth`): compute GitHub or git numbers
  in a session and commit them, as `ops/metrics/dashboard.json` does ([journal](journal/2026-11-25/1830-305-dashboard.md)).
- Deciding who wrote a GitHub record (`--author @me`, comment authors, a decision read from "a tagged comment") → the
  account is shared with the owner, so the author alone proves nothing and the tag alone proves nothing either (the
  repo is public and anyone can type it): agent records are tagged **and** by `jacobdam`, owner records untagged and
  by `jacobdam`; treat owner PRs in the owner feed (`is_pr`) separately ([journal](journal/2026-11-23.md), PR #269
  R1.2, R1.3; [journal](journal/2026-11-28/0555-461-undefer.md), PR #464 R1.1; PR #533 R1.1).
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
- Recording a batch of tool runs in an ADR (admission, benchmarks) → name the exact version and flags of each run and why it was chosen; a one-phrase rule ("the newest audited version", "the newest of the previous line") was false for some items and drew a major in rounds 1 and 2 of PR #444. Check each named version against the registry's version list before round 1 ([journal](journal/2026-11-28/0304-304-backend-selection.md)).
- Writing a record (retro, audit, journal) that claims GitHub side effects (labels, new issues)
  → do them before the review, or the record is false when it merges ([journal](journal/2026-11-21.md), PR #90).
  Claiming a workflow or rule fired (drills) → cite the run log or the resulting state (timeline event, draft flag),
  never the bot's "applied" message: a `|| true` hid a permission error ([journal](journal/2026-11-22.md), PR #209 R1.1).
  Filing numbered issues that reference each other: re-creating a blocked issue after later ones gives it a later
  number, so create dependencies first, then check numbers before writing "Depends on: #N" ([journal](journal/2026-11-20.md)).
- A behavior change stated as "only for case X" that comes from removing a fold → check every input that reached the
  old printing or exit path, not only the one the ADR names ([journal](journal/2026-11-27/1338-360-steps.md), PR #382
  R1.1).
- Lifting a skip label (`deferred`, `owner-todo`, `blocked`) from an untagged owner issue → it is back in the owner feed, so rule a picks it ahead of rule d and step 6. Say so in the lift comment, and don't promise a later step's pick ([journal](journal/2026-11-28/0555-461-undefer.md), PR #464 R1.2).
- A design that names a lint or gate scope ("the library code of every crate") → check it against the real cargo targets and features before round 1: `--lib` skips a bin-only crate, `--no-deps` is needed to exclude a path dependency, and a `cfg(feature)` gate on a module drops that crate's own tests unless it is `cfg(any(test, feature))` (PR #465 R1.2, R1.8, R2.1; [journal](journal/2026-11-28/0617-236-memory-adr.md)).
- A session that finds a tagged PR, worktree or issue minutes old (a Planner rerun, a "leftover" worktree, rule c's
  pick of a just-opened PR) → the owner's interactive session may still be working it: check for its locked
  worktree, `(owner's interactive session)` comments and the PR's age, and let it finish before acting; PR #521's
  Planner rerun started for an issue that run had just filed, and this retro's session first got rule f's
  `t/508-m0-close` and then rule c's PR #536, both of which the interactive session finished within 7 minutes
  ([journal](journal/2026-11-28/1029-planner-owner-run.md), [journal](journal/2026-11-28/1353-537-retro.md)).
- A reviewer's suggested wording for a nit or minor that lists cases ("a parked route issue, or …") → check it against the term it restates before applying it, or write the rule as the test itself ("a route step 1.2 wouldn't pick even without it"); PR #530's R3.2 wording drew the R4.1 major ([journal](journal/2026-11-28/1142-529-owner-feed-design.md)). Any final-check finding, even a nit, escalates to design, so fix wording before the final check runs (PR #528 R5.1). Rewrapping by hand in a fix drew nits in rounds 2 to 4 of PR #533: check every added line's width after each fix ([journal](journal/2026-11-28/1231-509-adr0019-text.md)).
- A process change justified by a cost (tokens, minutes, sessions) → measure the mechanism in the logs before writing the change: retro 0003's first draft moved the scheduler ahead of LESSONS to cut idle sessions' cost, but 153 of 271 idle sessions already ran it second and still used 169 k tokens (the fixed prompt is the floor), and the move skipped §1's `owner-todo` check (PR #538 R1.1, R1.2, [journal](journal/2026-11-28/1353-537-retro.md)).
- Fixing a false-statement finding after review round 4 (the post-cap diff) → remove or narrow the claim; a new sentence that adds a rule (S4.3's limit-precedence wording in PR #540) is a new untested claim, and any final-check finding escalates the PR to design (S5.1) ([journal](journal/2026-11-28/1448-78-lenient-read.md)).
- A test that claims something did not happen (an object not read, a step skipped) → assert a counter from the first draft (a `#[cfg(test)]` accessor over the lexing budget and span list), not a fixture that would fail if touched: an error-discarding read passes the fixture, and PR #542 spent rounds 2 to 4 on it. Write every coverage comment as the exact cases a test runs ("such as", never a list that reads as complete): a nit on such a parenthetical in the final check escalated the PR ([journal](journal/2026-11-28/1549-87-mutants.md)).
