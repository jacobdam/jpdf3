//! Manifest format (specs/tooling/corpus.md, item 2): the judge's three-column TSV.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::Error;
use crate::sources::ALLOWED_HOSTS;

/// Largest manifest file we read.
pub const MAX_MANIFEST_BYTES: u64 = 32 << 20;
/// Most rows in one manifest.
pub const MAX_ROWS: usize = 200_000;
/// Longest URL.
pub const MAX_URL_BYTES: usize = 2048;
/// Largest file size a row may declare.
pub const MAX_ENTRY_BYTES: u64 = 64 << 20;

/// One manifest row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Git blob id, 40 lowercase hex digits.
    pub blob: String,
    pub size: u64,
    /// Unencoded `https://` URL on an allowed host.
    pub url: String,
}

/// A parsed manifest: header comments and rows, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Comment lines without the leading `#` and one following space.
    pub comments: Vec<String>,
    pub entries: Vec<Entry>,
}

/// Reads and parses a manifest file (read bounded at the cap + 1 byte).
pub fn load(path: &Path) -> Result<Manifest, Error> {
    let name = path.display().to_string();
    let io = |source| Error::Io {
        path: name.clone(),
        source,
    };
    let file = std::fs::File::open(path).map_err(io)?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(manifest_err(
            &name,
            0,
            format!("file is larger than {MAX_MANIFEST_BYTES} bytes"),
        ));
    }
    parse(&name, &bytes)
}

fn manifest_err(name: &str, line: usize, message: String) -> Error {
    Error::Manifest {
        manifest: name.to_string(),
        line,
        message,
    }
}

/// Parses manifest bytes; `name` is used in errors.
pub fn parse(name: &str, bytes: &[u8]) -> Result<Manifest, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| manifest_err(name, 0, format!("not UTF-8 ({e})")))?;
    let mut out = Manifest::default();
    let mut seen = HashMap::new();
    for (i, line) in text.split('\n').enumerate() {
        let lineno = i.saturating_add(1);
        let err = |m: String| manifest_err(name, lineno, m);
        if let Some(c) = line.strip_prefix('#') {
            out.comments
                .push(c.strip_prefix(' ').unwrap_or(c).to_string());
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [blob, size, url] = cols[..] else {
            return Err(err(format!(
                "expected 3 tab-separated columns, got {}",
                cols.len()
            )));
        };
        check_blob(blob).map_err(&err)?;
        let size = parse_size(size).map_err(&err)?;
        check_url(url).map_err(&err)?;
        // The judge's smoke manifest lists a few files under two names: the same blob and
        // size again is dropped; the same blob with another size is an error.
        match seen.get(blob) {
            Some(&first) if first == size => continue,
            Some(_) => return Err(err(format!("blob id {blob} listed with two sizes"))),
            None => {}
        }
        if out.entries.len() >= MAX_ROWS {
            return Err(err(format!("more than {MAX_ROWS} rows")));
        }
        seen.insert(blob, size);
        out.entries.push(Entry {
            blob: blob.to_string(),
            size,
            url: url.to_string(),
        });
    }
    Ok(out)
}

fn check_blob(blob: &str) -> Result<(), String> {
    if blob.len() == 40
        && blob
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(format!("bad blob id {blob:?} (40 lowercase hex digits)"))
    }
}

fn parse_size(size: &str) -> Result<u64, String> {
    let bad = || format!("bad size {size:?}");
    if size.is_empty() || !size.bytes().all(|b| b.is_ascii_digit()) || size.starts_with('0') {
        return Err(bad());
    }
    let n: u64 = size.parse().map_err(|_| bad())?;
    if n > MAX_ENTRY_BYTES {
        return Err(format!("size {n} is larger than {MAX_ENTRY_BYTES}"));
    }
    Ok(n)
}

/// Checks the URL rules: `https://<allowed host>/<path>`, ≤ 2048 bytes, no control chars.
pub fn check_url(url: &str) -> Result<(), String> {
    if url.len() > MAX_URL_BYTES {
        return Err(format!("URL longer than {MAX_URL_BYTES} bytes"));
    }
    if url.chars().any(char::is_control) {
        return Err("URL contains a control character".into());
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("URL {url:?} is not https://"))?;
    let (host, path) = rest
        .split_once('/')
        .ok_or_else(|| format!("URL {url:?} has no path"))?;
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(format!("host {host:?} is not allowed"));
    }
    if path.is_empty() {
        return Err(format!("URL {url:?} has no path"));
    }
    Ok(())
}

/// Percent-encodes a checked URL for fetching: in the path every byte except
/// `A–Z a–z 0–9 - _ . ~ / :` becomes `%XX` (the judge's `quote(url, safe=":/")`).
pub fn encode_url(url: &str) -> String {
    let Some(rest) = url.strip_prefix("https://") else {
        return url.to_string();
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut out = format!("https://{host}/");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/:".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Renders a manifest: `# ` comments, then rows.
pub fn write(m: &Manifest) -> String {
    let mut out = String::new();
    for c in &m.comments {
        out.push_str("# ");
        out.push_str(c);
        out.push('\n');
    }
    for e in &m.entries {
        out.push_str(&format!("{}\t{}\t{}\n", e.blob, e.size, e.url));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const B1: &str = "76e8430a4975ba221883c16a075a4e9fd28c66d0";
    const B2: &str = "9bd93307be15a053ae21638e1c9f5ec63cff8a23";

    fn row(blob: &str, size: &str, url: &str) -> String {
        format!("{blob}\t{size}\t{url}\n")
    }

    fn err_of(text: &str) -> (usize, String) {
        match parse("m.tsv", text.as_bytes()) {
            Err(Error::Manifest { line, message, .. }) => (line, message),
            other => panic!("{text:?}: {other:?}"),
        }
    }

    #[test]
    fn valid_manifest_round_trips() {
        let text = format!(
            "# header one\n# columns: x\n{}\n{}",
            row(
                B1,
                "328400",
                "https://raw.githubusercontent.com/o/r/c/a b.pdf"
            )
            .trim_end(),
            row(B2, "1", "https://gitlab.freedesktop.org/p/t/-/raw/c/x.PDF"),
        );
        let m = parse("m.tsv", text.as_bytes()).unwrap();
        assert_eq!(m.comments, ["header one", "columns: x"]);
        assert_eq!(m.entries.len(), 2);
        assert_eq!(m.entries[0].size, 328_400);
        assert_eq!(
            m.entries[1].url,
            "https://gitlab.freedesktop.org/p/t/-/raw/c/x.PDF"
        );
        assert_eq!(parse("m.tsv", write(&m).as_bytes()).unwrap(), m);
    }

    #[test]
    fn errors_name_the_line() {
        let ok = row(B1, "5", "https://raw.githubusercontent.com/o/r/c/f.pdf");
        let cases: [(String, &str); 13] = [
            (
                format!("{B1}\t5\n"),
                "expected 3 tab-separated columns, got 2",
            ),
            (
                format!("{ok}{B2}\t5\thttps://raw.githubusercontent.com/x\tz\n"),
                "got 4",
            ),
            (
                row(
                    &B1.to_uppercase(),
                    "5",
                    "https://raw.githubusercontent.com/a",
                ),
                "bad blob id",
            ),
            (
                row(&B1[..39], "5", "https://raw.githubusercontent.com/a"),
                "bad blob id",
            ),
            (
                row(B1, "0", "https://raw.githubusercontent.com/a"),
                "bad size",
            ),
            (
                row(B1, "05", "https://raw.githubusercontent.com/a"),
                "bad size",
            ),
            (
                row(B1, "+5", "https://raw.githubusercontent.com/a"),
                "bad size",
            ),
            (
                row(B1, "67108865", "https://raw.githubusercontent.com/a"),
                "larger than",
            ),
            (
                row(B1, "5", "http://raw.githubusercontent.com/a"),
                "not https://",
            ),
            (row(B1, "5", "https://evil.example/a"), "is not allowed"),
            (
                row(B1, "5", "https://raw.githubusercontent.com.evil/a"),
                "is not allowed",
            ),
            (
                row(B1, "5", "https://raw.githubusercontent.com/"),
                "has no path",
            ),
            (
                format!(
                    "{ok}{}",
                    row(B1, "6", "https://raw.githubusercontent.com/o/r/c/g.pdf")
                ),
                "listed with two sizes",
            ),
        ];
        for (text, want) in cases {
            let (line, message) = err_of(&text);
            assert!(message.contains(want), "{text:?}: {message}");
            assert_eq!(line, text.lines().count(), "{text:?}");
        }
        let (_, m) = err_of(&row(B1, "5", "https://raw.githubusercontent.com/a\u{7}"));
        assert!(m.contains("control"), "{m}");
        let long = format!("https://raw.githubusercontent.com/{}", "a".repeat(2048));
        let (_, m) = err_of(&row(B1, "5", &long));
        assert!(m.contains("longer than"), "{m}");
        match parse("m.tsv", b"\xff\n") {
            Err(Error::Manifest {
                line: 0, message, ..
            }) => assert!(message.contains("UTF-8")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn repeated_blob_with_the_same_size_is_dropped() {
        let text = format!(
            "{}{}",
            row(B1, "5", "https://raw.githubusercontent.com/o/r/c/f.pdf"),
            row(B1, "5", "https://raw.githubusercontent.com/o/r/c/copy.pdf")
        );
        let m = parse("m", text.as_bytes()).unwrap();
        assert_eq!(m.entries.len(), 1);
        assert!(m.entries[0].url.ends_with("/f.pdf"));
    }

    #[test]
    fn size_cap_is_inclusive() {
        let text = row(B1, "67108864", "https://raw.githubusercontent.com/a");
        assert_eq!(
            parse("m", text.as_bytes()).unwrap().entries[0].size,
            MAX_ENTRY_BYTES
        );
    }

    #[test]
    fn row_cap() {
        let mut text = String::new();
        for i in 0..=MAX_ROWS {
            text.push_str(&row(
                &format!("{i:040x}"),
                "1",
                "https://raw.githubusercontent.com/a",
            ));
        }
        let (line, m) = err_of(&text);
        assert_eq!(line, MAX_ROWS + 1);
        assert!(m.contains("more than 200000 rows"), "{m}");
        let at_cap: String = text
            .lines()
            .take(MAX_ROWS)
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(
            parse("m", at_cap.as_bytes()).unwrap().entries.len(),
            MAX_ROWS
        );
    }

    #[test]
    fn file_size_cap() {
        let dir = std::env::temp_dir().join(format!("jpdf-corpus-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("big.tsv");
        std::fs::write(
            &p,
            vec![b'#'; usize::try_from(MAX_MANIFEST_BYTES).unwrap() + 1],
        )
        .unwrap();
        match load(&p) {
            Err(Error::Manifest {
                line: 0, message, ..
            }) => assert!(message.contains("larger")),
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn encoding_matches_the_judges_quote() {
        // urllib.parse.quote(url, safe=":/") for each input.
        for (raw, want) in [
            (
                "https://raw.githubusercontent.com/o/r/c/test/a b.pdf",
                "https://raw.githubusercontent.com/o/r/c/test/a%20b.pdf",
            ),
            (
                "https://raw.githubusercontent.com/o/r/c/100%.pdf",
                "https://raw.githubusercontent.com/o/r/c/100%25.pdf",
            ),
            (
                "https://raw.githubusercontent.com/o/r/c/é#?&+=~_-.pdf",
                "https://raw.githubusercontent.com/o/r/c/%C3%A9%23%3F%26%2B%3D~_-.pdf",
            ),
            (
                "https://gitlab.freedesktop.org/p/t/-/raw/c/ISO 32000-2/x(1):.pdf",
                "https://gitlab.freedesktop.org/p/t/-/raw/c/ISO%2032000-2/x%281%29:.pdf",
            ),
        ] {
            assert_eq!(encode_url(raw), want);
        }
    }
}
