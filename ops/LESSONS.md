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

- Writing a local check that mirrors a judge script → match its Python regex semantics
  exactly (`\s` crosses newlines and includes U+001C–U+001F; pathlib globs match `.md`
  files and directories), and diff both side by side on probe trees before review
  ([journal](journal/2026-11-20.md), PR #27).

## Engine & security
- Parsing hostile input: per-object caps don't bound a document. Overlapping objects/xref
  sections make references re-parse the same bytes (quadratic time); several capped objects
  alive at once exceed 2 GiB → from the start, use a per-document lexing budget charged per
  byte, a per-document object budget, overlap rejection by byte span, one parsed object alive
  at a time, and compact xref entries ([journal](journal/2026-11-20.md), PR #24).
- Tests that edit generated PDFs: `String::from_utf8_lossy` changes byte lengths (binary header
  comment) and silently shifts offsets → edit `Vec<u8>` directly ([journal](journal/2026-11-20.md)).

## Process
- The guard blocks a Bash call whose text contains a GitHub CLI write it can't parse, even inside a
  heredoc or a chained command → write files with Write/Edit and run each CLI write as its own
  single command ([journal](journal/2026-11-20.md)).
- Any text naming the sealed exam manifest (even an issue body) is blocked by the guard, and any
  tool that reads it to "exclude" files would break CONSTITUTION 13 → refer to "the sealed
  corpus" only; overlap with it is the judge's concern ([journal](journal/2026-11-20.md)).
- Re-review after a push: `gh pr diff` can return the previous diff for a while → give the
  reviewer `git diff origin/main...HEAD` instead ([journal](journal/2026-11-20.md)).
- The guard blocks force pushes → once a branch is pushed, bring in main with `git merge
  origin/main`, never rebase ([journal](journal/2026-11-20.md)).
- Creating numbered issues that reference each other: re-creating a blocked issue after later
  ones gives it a later number → create dependencies first, then check numbers before writing "Depends on: #N"
  ([journal](journal/2026-11-20.md)).
