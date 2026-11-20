# Lessons learned (read at the start of every session; curated by retros, ≤ 60 entries)

Format: **situation → what to do** (link to the journal entry or PR). Refine existing entries
instead of adding near-duplicates.

## Tooling & environment
- Claude Code project settings and hooks apply **live** to running sessions → never write
  `.claude/settings.json` before its hook script exists and passes `--selftest` (bootstrap, 2026-11-20).
- macOS `/usr/bin/openssl` is LibreSSL and its default PKCS#12 output isn't readable by every
  tool → always pass `-keypbe AES-256-CBC -certpbe AES-256-CBC -macalg sha256` (bootstrap).
- pyHanko `SimpleSigner.load_pkcs12(file, passphrase=...)`: the 2nd positional argument is the
  CA chain, not the password (bootstrap).
- `reference.py` on the full smoke corpus takes ~4 min locally; one pdf.js file hangs MuPDF
  (> 60 s) → reference tools must be called with timeouts (bootstrap).

## Judge & CI
- MuPDF and Poppler agree (≤ 2% pixels differ at 72 dpi after 2× downscale) on ~78% of
  smoke pages; only those consensus pages are ratcheted (bootstrap calibration).
- A skipped required job counts as success on GitHub → the verdict job runs `if: always()` and
  fails unless every upstream job succeeded (bootstrap).

## Process
- The guard blocks a Bash call whose text contains a GitHub CLI write it can't parse, even inside a
  heredoc or a chained command → write files with Write/Edit and run each CLI write as its own
  single command ([journal](journal/2026-11-20.md)).
- Any text naming the sealed exam manifest (even an issue body) is blocked by the guard, and any
  tool that reads it to "exclude" files would break CONSTITUTION 13 → refer to "the sealed
  corpus" only; overlap with it is the judge's concern ([journal](journal/2026-11-20.md)).
- Creating numbered issues that reference each other: a blocked or failed create shifts later
  numbers → create dependencies first, then check numbers before writing "Depends on: #N"
  ([journal](journal/2026-11-20.md)).
