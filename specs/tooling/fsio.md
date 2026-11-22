# jpdf-fsio: non-blocking opens for local files

Area: tooling
ISO 32000-2 clauses: none (process hygiene)
Journeys: none
Status: implemented

## Behavior
`tools/fsio` (crate `jpdf-fsio`, `std` only, no dependencies) holds the one copy of the
per-target open flags that `jpdf-cli` and the development tools use when they read a local
file whose type they have just checked (#115 items 2 and 5, audit 0002).

- `OPEN_FLAGS: Option<i32>`: macOS `O_NONBLOCK` (`0x0004`; an open never acquires a
  controlling terminal there, so no `O_NOCTTY`); Linux on x86, x86_64, arm, aarch64, riscv64,
  powerpc64 and s390x `O_NONBLOCK | O_NOCTTY` (`0o4000 | 0o400`, the generic `<fcntl.h>`
  values); `None` everywhere else (mips, sparc, alpha and parisc use other values). One
  `cfg!` chain selects it, so the target list exists once.
- `open_nonblocking(path) -> io::Result<File>`: a read-only open with `OPEN_FLAGS` added (plain
  flags when `None`, and on non-unix targets). A FIFO with no writer then opens at once instead
  of blocking, and a terminal never becomes the controlling terminal. Regular-file reads are
  unchanged. The open follows symlinks and accepts any file type: every caller checks the
  opened handle (`File::metadata`, an `fstat`) and refuses anything but a regular file.

Callers (each keeps its own type checks, caps and errors). Every "doesn't block" for them, here
and in their specs, holds only where `OPEN_FLAGS` is set; elsewhere their opens are plain:
- `jpdf-cli` `read_limited` (specs/cli/jpdf-cli.md, "Reading FILE" step 2);
- `jpdf-arlington` `vendor.rs` `read_capped` (specs/tooling/arlington.md item 9 step 4);
- `jpdf-arlington` `read_dir` (the `--tsv` source; the opened handle must still be a regular
  file, `not a regular file` otherwise);
- `jpdf-corpus` `fetch.rs` `verify` (specs/tooling/corpus.md, the cached-file check);
- `jpdf-corpus` `manifest::load`, which also gains the opened-handle check: a FIFO or a
  directory is `<path>: not a regular file` (before: a FIFO blocked the open, a directory failed
  the read);
- `xtask` `specs.rs` `read_text` (specs/tooling/check-specs.md);
- `xtask` `web.rs` `read_results`, which also gains the opened-handle check: a FIFO or a
  directory at the results path is `<path>: not a regular file` (before: a FIFO blocked the
  open, a directory failed the read with the OS error). A missing file still means no results.

## Persistence
None.

## Undo
None.

## Errors
`open_nonblocking` returns the OS error unchanged (`NotFound`, `PermissionDenied`, …). Callers
map it to their own messages, as before.

## Limits
None of its own. Callers keep their caps (`take(cap + 1)` reads, size checks); this crate only
changes whether the open can block.

## Security
- Threat: a path swapped for a FIFO between a caller's type check and its open blocks a plain
  open forever (a hung judge run for `jpdf-cli`, which already opened this way; a hung local
  run for the tools, which didn't). Now every caller's open returns and its handle check
  refuses the FIFO.
- Arithmetic: none.
- What broken implementation still passes? A wrong `O_NONBLOCK` value on macOS or Linux
  x86_64/aarch64 hangs `open_never_blocks_on_a_fifo` (10 s timeout) and fails
  `flags_are_known_on_ci_targets` on macOS and Linux x86_64. A wrong `O_NOCTTY` value on Linux
  fails the flags test only, and an `open_nonblocking` that masks `O_NOCTTY` out of the flags
  it passes still passes (no test opens a terminal). A caller that drops its handle check
  is caught only in `read_results` and `manifest::load`, which have no pre-open type check, so
  their FIFO and directory tests reach the handle check. The other callers check the type before opening,
  so their FIFO tests stop there; dropping their handle check still passes (the swap between
  the two checks is a race no test stages deterministically; specs/cli/jpdf-cli.md lists the
  same for `read_limited`).
- What the tests can't see: the other Linux architectures in the list (CI runs x86_64 only);
  their values are the generic ones. On targets where `OPEN_FLAGS` is `None` the open of a
  FIFO still blocks: the FIFO tests are skipped there, and a FIFO swap between a caller's type
  check and its open still hangs it (`read_results` and `manifest::load`, which have no
  pre-open check, block on any FIFO there). The handle checks of `read_results` and
  `manifest::load` are tested only with a FIFO or directory already in place; an
  implementation that checks the path before opening instead would pass those tests too (the
  swap that tells them apart can't be staged deterministically).

## Accessibility
None.

## Platforms
macOS and Linux as above; other unix targets and Windows open with plain flags.

## Performance
One flag on an open; nothing measurable.

## Tests
- `tools/fsio/tests/open.rs`: `flags_are_known_on_ci_targets` (the exact values on macOS and
  Linux x86_64); `regular_files_read_normally` (200 000 bytes read back whole; a missing path is
  `NotFound`); `open_never_blocks_on_a_fifo` (a FIFO with no writer opens within 10 s and its
  handle is not a regular file; skipped where `OPEN_FLAGS` is `None`; on a timeout the
  cleanup opens the write end non-blocking, from a thread nobody waits for, so it can't
  hold up the failure report).
  The integration-test root forbids `unsafe_code` like the library.
- `xtask/src/web.rs` `reads_results_files`: a directory and a FIFO at the results path give
  `<path>: not a regular file` without blocking (the FIFO half only where `OPEN_FLAGS` is
  set).
- `tools/corpus/tests/corpus.rs` `manifest_load_refuses_non_regular_files`: the same for a
  manifest path (30 s timeout; the FIFO half only where `OPEN_FLAGS` is set).
- `tools/jpdf-cli/src/main.rs` `open_never_blocks_on_a_fifo` (now through the crate, skipped
  where `OPEN_FLAGS` is `None`, with the same non-blocking cleanup).
