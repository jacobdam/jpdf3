//! specs/tooling/fsio.md "Tests".
#![forbid(unsafe_code)]

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

#[cfg(test)]
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fsio-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The flags are known on the targets CI and the owner's machine run.
#[test]
fn flags_are_known_on_ci_targets() {
    if cfg!(target_os = "macos") {
        assert_eq!(jpdf_fsio::OPEN_FLAGS, Some(0x0004));
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
        // Every listed architecture, so the FIFO tests never skip on one of them.
        assert_eq!(jpdf_fsio::OPEN_FLAGS, Some(0o4000 | 0o400));
    }
}

/// A regular file opens and reads in full (the flags don't change regular-file reads).
#[test]
fn regular_files_read_normally() {
    let f = scratch("regular").join("data");
    let data: Vec<u8> = (0..=255u8).cycle().take(200_000).collect();
    fs::write(&f, &data).unwrap();
    let mut file = jpdf_fsio::open_nonblocking(&f).unwrap();
    assert!(file.metadata().unwrap().is_file());
    let mut got = Vec::new();
    file.read_to_end(&mut got).unwrap();
    assert_eq!(got, data);
    // A missing path is an ordinary open error.
    let e = jpdf_fsio::open_nonblocking(&f.with_file_name("missing")).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
}

/// Opening a FIFO with no writer blocks without `O_NONBLOCK`, so a wrong flag value hangs
/// here and fails the timeout. The opened handle reports a FIFO, which callers refuse.
/// Skipped where [`jpdf_fsio::OPEN_FLAGS`] is `None` (a plain open blocks there by design).
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn open_never_blocks_on_a_fifo() {
    if jpdf_fsio::OPEN_FLAGS.is_none() {
        return;
    }
    let fifo = scratch("fifo").join("fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = mpsc::channel();
    let path = fifo.clone();
    std::thread::spawn(move || {
        let kind = jpdf_fsio::open_nonblocking(&path)
            .and_then(|f| f.metadata())
            .map(|m| (m.is_file(), m.file_type()));
        let _ = tx.send(format!("{kind:?}"));
    });
    let got = rx.recv_timeout(Duration::from_secs(10));
    // On a timeout the reader thread stays blocked in its open; it ends with the test
    // process (nothing can unblock it once the FIFO is removed below).
    let _ = fs::remove_file(&fifo);
    let got = got.expect("open of a FIFO blocked");
    assert!(got.starts_with("Ok((false,"), "{got}");
}
