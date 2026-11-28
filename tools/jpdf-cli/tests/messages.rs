//! The exact stderr line `jpdf-cli info` prints for an engine error (ops/judge/CONTRACT.md §1,
//! specs/cli/jpdf-cli.md): the `EngineError` text after a `jpdf-cli: ` prefix (#87 item 7).

use std::fs::{DirBuilder, OpenOptions};
use std::io;
use std::process::Command;

#[test]
fn empty_file_prints_the_engine_error_line() -> io::Result<()> {
    // A directory of our own under Cargo's per-target temporary directory (not the shared system
    // one). `create` fails if it already exists, so we never write into or remove someone
    // else's directory, and the file is created exclusively inside it.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("jpdf-cli-messages-{}", std::process::id()));
    DirBuilder::new().create(&dir)?;
    let path = dir.join("empty.pdf");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let o = Command::new(env!("CARGO_BIN_EXE_jpdf-cli"))
        .arg("info")
        .arg(&path)
        .output();
    std::fs::remove_file(&path)?;
    std::fs::remove_dir(&dir)?;
    let o = o?;
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty(), "{o:?}");
    assert_eq!(
        String::from_utf8_lossy(&o.stderr),
        "jpdf-cli: malformed PDF: startxref not found at byte 0\n"
    );
    Ok(())
}
