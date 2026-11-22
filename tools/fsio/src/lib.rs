//! jpdf-fsio — opens local files without blocking, for `jpdf-cli` and the development tools
//! (specs/tooling/fsio.md). The one place that knows the per-target `O_NONBLOCK` values.
#![forbid(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// The flags [`open_nonblocking`] adds to a read-only open, or `None` where we don't know the
/// values (the open then uses plain flags, and only the caller's `stat` checks guard against
/// FIFOs).
///
/// - macOS: `O_NONBLOCK` (0x0004). An open never acquires a controlling terminal there, so no
///   `O_NOCTTY` is needed.
/// - Linux architectures whose `<fcntl.h>` uses the generic values (`O_NONBLOCK` 0o4000,
///   `O_NOCTTY` 0o400). mips, sparc, alpha and parisc differ, so they get `None`.
pub const OPEN_FLAGS: Option<i32> = if cfg!(target_os = "macos") {
    Some(0x0004)
} else if cfg!(all(
    target_os = "linux",
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "powerpc64",
        target_arch = "s390x"
    )
)) {
    Some(0o4000 | 0o400)
} else {
    None
};

/// Opens `path` for reading with [`OPEN_FLAGS`]: opening a FIFO that has no writer then
/// returns at once instead of blocking, and a terminal swapped in never becomes the
/// controlling terminal. Neither flag changes reads from regular files.
///
/// The open follows symlinks and accepts any file type. Callers check the opened file
/// (`file.metadata()`, an `fstat`) and refuse anything but a regular file: a FIFO opened
/// this way would make reads fail with `WouldBlock` or return nothing.
pub fn open_nonblocking(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    if let Some(flags) = OPEN_FLAGS {
        std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, flags);
    }
    options.open(path)
}
