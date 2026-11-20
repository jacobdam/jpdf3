//! Exit-code contract of `jpdf-cli` (ops/judge/CONTRACT.md §1). Spec: specs/cli/jpdf-cli.md.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

fn cli(args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_jpdf-cli"))
        .args(args)
        .output()
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Writes `bytes` to a fresh temp file and returns its path.
fn temp_file(bytes: &[u8]) -> io::Result<PathBuf> {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("jpdf-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{n}.pdf"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn minimal_pdf(pages: usize) -> Vec<u8> {
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            kids.join(" ")
        ),
    ];
    objs.extend((0..pages).map(|_| "<< /Type /Page /Parent 2 0 R >>".to_string()));
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref_at = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f\r\n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n\r\n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

#[test]
fn info_prints_page_count() -> io::Result<()> {
    for n in [1, 2, 7] {
        let f = temp_file(&minimal_pdf(n))?;
        let o = cli(&["info", f.to_str().unwrap()])?;
        assert_eq!(
            code(&o),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&o.stdout),
            format!("{{\"pages\": {n}}}\n")
        );
    }
    let f = temp_file(&minimal_pdf(1))?;
    let o = cli(&["info", f.to_str().unwrap(), "--password", "x"])?;
    assert_eq!(code(&o), Some(0));
    Ok(())
}

#[test]
fn every_other_contract_command_exits_3() -> io::Result<()> {
    let f = temp_file(&minimal_pdf(1))?;
    let f = f.to_str().unwrap();
    let cases: &[&[&str]] = &[
        &["render", f, "--page", "1", "--dpi", "72", "--out", "o.png"],
        &[
            "render",
            f,
            "--page",
            "1",
            "--dpi",
            "72",
            "--out",
            "o.png",
            "--password",
            "p",
        ],
        &["text", f, "--page", "1"],
        &["text", f, "--page", "1", "--password", "p"],
        &[
            "annotate",
            f,
            "--out",
            "o.pdf",
            "--page",
            "1",
            "--kind",
            "highlight",
            "--rect",
            "10,20.5,30,40",
            "--color",
            "255,255,0",
            "--save",
            "incremental",
        ],
        &["rewrite", f, "--out", "o.pdf", "--mode", "full"],
        &[
            "encrypt",
            f,
            "--out",
            "o.pdf",
            "--user-password",
            "u",
            "--owner-password",
            "o",
        ],
        &[
            "fill", f, "--out", "o.pdf", "--field", "a.b", "--value", "v",
        ],
        &[
            "redact", f, "--out", "o.pdf", "--page", "2", "--rect", "0,0,1,1",
        ],
        &[
            "sign",
            f,
            "--out",
            "o.pdf",
            "--p12",
            "k.p12",
            "--p12-password",
            "pw",
            "--field",
            "Sig1",
        ],
        &["verify-signatures", f],
        // Flags in any order, FILE last.
        &["text", "--page", "1", f],
    ];
    for args in cases {
        let o = cli(args)?;
        assert_eq!(code(&o), Some(3), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
        assert!(!o.stderr.is_empty(), "{args:?}");
    }
    Ok(())
}

#[test]
fn argument_errors_exit_2_with_empty_stdout() -> io::Result<()> {
    let cases: &[&[&str]] = &[
        &[],
        &["bogus", "f.pdf"],
        &["info"],
        &["info", "a.pdf", "b.pdf"],
        &["info", "a.pdf", "--password"],
        &["info", "a.pdf", "--page", "1"],
        &["info", "a.pdf", "--password", "a", "--password", "b"],
        &["render", "a.pdf", "--page", "1", "--dpi", "72"],
        &[
            "render", "a.pdf", "--page", "0", "--dpi", "72", "--out", "o",
        ],
        &[
            "render", "a.pdf", "--page", "-1", "--dpi", "72", "--out", "o",
        ],
        &["render", "a.pdf", "--page", "1", "--dpi", "0", "--out", "o"],
        &[
            "render", "a.pdf", "--page", "1", "--dpi", "99999", "--out", "o",
        ],
        &["text", "a.pdf", "--page", "x"],
        &[
            "annotate",
            "a.pdf",
            "--out",
            "o",
            "--page",
            "1",
            "--kind",
            "ink",
            "--rect",
            "0,0,1,1",
            "--color",
            "1,2,3",
            "--save",
            "incremental",
        ],
        &[
            "annotate",
            "a.pdf",
            "--out",
            "o",
            "--page",
            "1",
            "--kind",
            "highlight",
            "--rect",
            "0,0,1",
            "--color",
            "1,2,3",
            "--save",
            "incremental",
        ],
        &[
            "annotate",
            "a.pdf",
            "--out",
            "o",
            "--page",
            "1",
            "--kind",
            "highlight",
            "--rect",
            "0,0,1,NaN",
            "--color",
            "1,2,3",
            "--save",
            "incremental",
        ],
        &[
            "annotate",
            "a.pdf",
            "--out",
            "o",
            "--page",
            "1",
            "--kind",
            "highlight",
            "--rect",
            "0,0,1,1",
            "--color",
            "1,2,256",
            "--save",
            "incremental",
        ],
        &[
            "annotate",
            "a.pdf",
            "--out",
            "o",
            "--page",
            "1",
            "--kind",
            "highlight",
            "--rect",
            "0,0,1,1",
            "--color",
            "1,2,3",
            "--save",
            "full",
        ],
        &["rewrite", "a.pdf", "--out", "o", "--mode", "incremental"],
        &["encrypt", "a.pdf", "--out", "o", "--user-password", "u"],
        &["verify-signatures", "a.pdf", "--out", "o"],
    ];
    for args in cases {
        let o = cli(args)?;
        assert_eq!(code(&o), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
        assert!(!o.stderr.is_empty(), "{args:?}");
    }
    Ok(())
}

#[test]
fn bad_input_files_exit_2_or_3() -> io::Result<()> {
    let valid = minimal_pdf(3);
    let mut inputs: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"not a pdf at all".to_vec(),
        vec![0u8; 10_000],
        valid[..valid.len() / 2].to_vec(),
        valid[..valid.len() - 10].to_vec(),
    ];
    // An xref stream: valid PDF, not implemented yet.
    inputs.push(b"%PDF-1.7\n1 0 obj\n<</Type/XRef>>\nendobj\nstartxref\n9\n%%EOF\n".to_vec());
    for bytes in &inputs {
        let f = temp_file(bytes)?;
        let o = cli(&["info", f.to_str().unwrap()])?;
        assert!(
            matches!(code(&o), Some(2 | 3)),
            "{:?}: {o:?}",
            String::from_utf8_lossy(bytes)
        );
        assert!(o.stdout.is_empty());
    }
    let o = cli(&["info", "/nonexistent/jpdf/missing.pdf"])?;
    assert_eq!(code(&o), Some(2));
    let dir = std::env::temp_dir();
    let o = cli(&["info", dir.to_str().unwrap()])?;
    assert_eq!(code(&o), Some(2));
    Ok(())
}
