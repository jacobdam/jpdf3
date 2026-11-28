//! The exact stderr line `jpdf-cli info` prints for an engine error (ops/judge/CONTRACT.md §1,
//! specs/cli/jpdf-cli.md): the `EngineError` text after a `jpdf-cli: ` prefix (#87 item 7).

use std::io;
use std::process::Command;

#[test]
fn empty_file_prints_the_engine_error_line() -> io::Result<()> {
    let dir = std::env::temp_dir().join(format!("jpdf-cli-messages-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("empty.pdf");
    std::fs::write(&path, b"")?;
    let o = Command::new(env!("CARGO_BIN_EXE_jpdf-cli"))
        .arg("info")
        .arg(&path)
        .output()?;
    std::fs::remove_dir_all(&dir)?;
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty(), "{o:?}");
    assert_eq!(
        String::from_utf8_lossy(&o.stderr),
        "jpdf-cli: malformed PDF: startxref not found at byte 0\n"
    );
    Ok(())
}
