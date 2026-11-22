//! Fetch behavior with a curl stand-in, and the committed manifests
//! (specs/tooling/corpus.md, "Tests").
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jpdf_corpus::fetch::{Options, Report, fetch, select};
use jpdf_corpus::manifest::{self, Entry};
use jpdf_corpus::sha1::git_blob_id;
use jpdf_corpus::{Tier, sources};

#[cfg(test)]
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("corpus-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("serve")).unwrap();
    std::fs::create_dir_all(d.join("out")).unwrap();
    d
}

/// A curl stand-in: writes `<dir>/serve/<last URL segment>` to stdout, logs each argv line
/// to `<dir>/log`. Names starting with `fail` exit 22, `big` exit 63, `flaky` fail once, and
/// `flood` write without end (an old curl that ignores `--max-filesize` without a
/// Content-Length).
#[cfg(test)]
fn fake_curl(dir: &Path) -> PathBuf {
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> '{d}/log'
url=
while [ $# -gt 0 ]; do
  case "$1" in --url) url=$2; shift;; esac
  shift
done
name=${{url##*/}}
case "$name" in
  fail*) echo "could not resolve" >&2; exit 22;;
  big*) exit 63;;
  flaky*) if [ ! -f '{d}/flaked' ]; then touch '{d}/flaked'; exit 7; fi;;
  flood*) exec yes;;
esac
cat '{d}/serve/'"$name" || exit 22
"#,
        d = dir.display()
    );
    let p = dir.join("curl");
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(test)]
fn entry(name: &str, bytes: &[u8]) -> Entry {
    Entry {
        blob: git_blob_id(bytes),
        size: bytes.len() as u64,
        url: format!("https://raw.githubusercontent.com/o/r/c/{name}"),
    }
}

#[cfg(test)]
fn opts(curl: PathBuf) -> Options {
    Options {
        curl,
        jobs: 3,
        retry_delay: Duration::ZERO,
    }
}

#[test]
fn downloads_verifies_and_reuses() {
    let d = scratch("ok");
    let curl = fake_curl(&d);
    let files: [(&str, &[u8]); 3] = [
        ("a.pdf", b"%PDF-1.7 a"),
        ("b c.pdf", b"bb"),
        ("flaky.pdf", b"f"),
    ];
    let mut entries = Vec::new();
    for (name, bytes) in files {
        std::fs::write(d.join("serve").join(name.replace(' ', "%20")), bytes).unwrap();
        entries.push(entry(name, bytes));
    }
    let out = d.join("out");
    let r = fetch(&entries, &out, &opts(curl.clone()));
    assert_eq!((r.total, r.cached, r.downloaded), (3, 0, 3), "{r:?}");
    assert!(r.failed.is_empty() && r.integrity.is_empty(), "{r:?}");
    for e in &entries {
        let bytes = std::fs::read(out.join(format!("{}.pdf", e.blob))).unwrap();
        assert_eq!(git_blob_id(&bytes), e.blob);
    }
    let log = std::fs::read_to_string(d.join("log")).unwrap();
    assert_eq!(
        log.lines().count(),
        4,
        "flaky.pdf took two attempts:\n{log}"
    );
    for line in log.lines() {
        assert!(
            line.starts_with("-q --proto =https --proto-redir =https "),
            "{line}"
        );
        assert!(line.contains("--max-filesize "), "{line}");
        // Redirects fail instead of reaching an unchecked host; the body goes to stdout.
        assert!(line.contains(" --max-redirs 0 "), "{line}");
        assert!(!line.contains("--output"), "{line}");
    }
    assert!(
        log.contains("--url https://raw.githubusercontent.com/o/r/c/b%20c.pdf"),
        "{log}"
    );
    assert_eq!(r.summary(), "fetched 3/3 (0 cached, 0 failed, 0 integrity)");

    // Second run: everything verified from disk, curl never called.
    std::fs::remove_file(d.join("log")).unwrap();
    let r = fetch(&entries, &out, &opts(curl.clone()));
    assert_eq!((r.cached, r.downloaded), (3, 0), "{r:?}");
    assert!(!d.join("log").exists());

    // A corrupted cached file is downloaded again.
    std::fs::write(out.join(format!("{}.pdf", entries[0].blob)), b"%PDF-1.7 X").unwrap();
    let r = fetch(&entries, &out, &opts(curl));
    assert_eq!((r.cached, r.downloaded), (2, 1), "{r:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn wrong_bytes_are_integrity_failures() {
    let d = scratch("integrity");
    let curl = fake_curl(&d);
    // Served bytes differ from the manifest: same size, wrong hash; and wrong size.
    std::fs::write(d.join("serve/hash.pdf"), b"XX").unwrap();
    std::fs::write(d.join("serve/size.pdf"), b"XXX").unwrap();
    std::fs::write(d.join("serve/short.pdf"), b"X").unwrap();
    let entries = [
        entry("hash.pdf", b"YY"),
        entry("size.pdf", b"ZZ"),
        entry("big.pdf", b"WW"),
        entry("short.pdf", b"VV"),
    ];
    let out = d.join("out");
    let r = fetch(&entries, &out, &opts(curl));
    assert_eq!(r.integrity.len(), 4, "{r:?}");
    assert!(
        r.integrity
            .iter()
            .any(|m| m.contains("short.pdf: size is 1, expected 2")),
        "{r:?}"
    );
    assert!(
        r.integrity
            .iter()
            .any(|m| m.contains("hash.pdf: blob id is")),
        "{r:?}"
    );
    assert!(
        r.integrity
            .iter()
            .any(|m| m.contains("size.pdf: size is more than 2, expected 2")),
        "{r:?}"
    );
    assert!(
        r.integrity
            .iter()
            .any(|m| m.contains("big.pdf: curl exit Some(63)")),
        "{r:?}"
    );
    // No retries on integrity failures, and nothing is left behind.
    let log = std::fs::read_to_string(d.join("log")).unwrap();
    assert_eq!(log.lines().count(), 4, "{log}");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// A body without end is cut at the manifest size + 1 byte: an integrity failure, no hang,
/// nothing left behind (curl < 8.4 doesn't enforce `--max-filesize` without Content-Length).
#[test]
fn an_endless_body_is_cut_at_the_size() {
    let d = scratch("flood");
    let curl = fake_curl(&d);
    let entries = [entry("flood.pdf", b"%PDF-1.7 short")];
    let out = d.join("out");
    let start = std::time::Instant::now();
    let r = fetch(&entries, &out, &opts(curl));
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(r.integrity.len(), 1, "{r:?}");
    assert!(
        r.integrity[0]
            .contains("flood.pdf: size is more than 14, expected 14 (stopped after 15 bytes)"),
        "{r:?}"
    );
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    std::fs::remove_dir_all(&d).unwrap();
}

/// A FIFO (or any non-file) in place of a cached `<blob>.pdf` is not opened (that would
/// block forever); the file is downloaded again and replaces it.
#[test]
fn a_fifo_in_the_cache_is_replaced_not_read() {
    let d = scratch("fifo");
    let curl = fake_curl(&d);
    let bytes = b"%PDF-1.7 fifo";
    std::fs::write(d.join("serve/f.pdf"), bytes).unwrap();
    let entries = vec![entry("f.pdf", bytes)];
    let out = d.join("out");
    let dst = out.join(format!("{}.pdf", entries[0].blob));
    let st = std::process::Command::new("mkfifo")
        .arg(&dst)
        .status()
        .unwrap();
    assert!(st.success());
    let (tx, rx) = std::sync::mpsc::channel();
    let (e2, o2) = (entries.clone(), out.clone());
    std::thread::spawn(move || {
        let _ = tx.send(fetch(&e2, &o2, &opts(curl)));
    });
    let r = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("fetch blocked on the FIFO");
    assert_eq!((r.cached, r.downloaded), (0, 1), "{r:?}");
    assert!(std::fs::metadata(&dst).unwrap().is_file());
    assert_eq!(std::fs::read(&dst).unwrap(), bytes);
    std::fs::remove_dir_all(&d).unwrap();
}

/// `manifest::load` refuses a FIFO or a directory from the opened handle, and a FIFO with no
/// writer doesn't block the open (specs/tooling/fsio.md).
#[test]
fn manifest_load_refuses_non_regular_files() {
    let d = scratch("manifest-fifo");
    let dir = d.join("dir.tsv");
    std::fs::create_dir_all(&dir).unwrap();
    let mut refused = vec![dir];
    // Only where the open is non-blocking; a plain open of a FIFO blocks by design elsewhere.
    if jpdf_fsio::OPEN_FLAGS.is_some() {
        let fifo = d.join("fifo.tsv");
        let st = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(st.success());
        refused.push(fifo);
    }
    for p in refused {
        let (tx, rx) = std::sync::mpsc::channel();
        let p2 = p.clone();
        std::thread::spawn(move || {
            let _ = tx.send(manifest::load(&p2).map_err(|e| e.to_string()));
        });
        let r = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("manifest::load blocked");
        let e = r.unwrap_err();
        assert!(e.contains("not a regular file"), "{e}");
        assert!(e.contains(&p.display().to_string()), "{e}");
    }
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn download_failures_and_the_infra_threshold() {
    let d = scratch("fail");
    let curl = fake_curl(&d);
    let mut entries = Vec::new();
    for i in 0..198 {
        let name = format!("f{i}.pdf");
        let bytes = name.as_bytes().to_vec();
        std::fs::write(d.join("serve").join(&name), &bytes).unwrap();
        entries.push(entry(&name, &bytes));
    }
    entries.push(entry("fail1.pdf", b"1"));
    entries.push(entry("fail2.pdf", b"2"));
    let r = fetch(&entries, &d.join("out"), &opts(curl.clone()));
    assert_eq!((r.downloaded, r.failed.len()), (198, 2), "{r:?}");
    assert!(
        r.failed[0].contains("curl exit Some(22): could not resolve"),
        "{r:?}"
    );
    // 2 of 200 missing is max(1, 200/100) = 2: not yet an infrastructure failure.
    assert!(!r.is_infra_failure());
    entries.push(entry("fail3.pdf", b"3"));
    let r = fetch(&entries, &d.join("out"), &opts(curl));
    assert_eq!((r.cached, r.failed.len()), (198, 3), "{r:?}");
    assert!(r.is_infra_failure());
    // Three attempts per failing file (3 files × 3) in the second run.
    let log = std::fs::read_to_string(d.join("log")).unwrap();
    assert_eq!(
        log.lines().filter(|l| l.contains("/fail")).count(),
        6 + 9,
        "{log}"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn missing_curl_is_a_download_failure() {
    let d = scratch("nocurl");
    let r = fetch(
        &[entry("a.pdf", b"a")],
        &d.join("out"),
        &opts(d.join("no-such-curl")),
    );
    assert_eq!(r.failed.len(), 1, "{r:?}");
    assert!(r.failed[0].contains("cannot run"), "{r:?}");
    assert!(!r.is_infra_failure(), "1 of 1 is within max(1, 0)");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn committed_regress_manifest_matches_the_pinned_sources() {
    let m = manifest::load(&Tier::Regress.manifest()).unwrap();
    assert!(m.entries.len() > 5000, "{}", m.entries.len());
    let mut sorted = m.entries.clone();
    sorted.sort_by(|a, b| a.blob.cmp(&b.blob));
    assert_eq!(sorted, m.entries, "rows are sorted by blob id");
    for e in &m.entries {
        let src = sources::REGRESS
            .iter()
            .find(|s| e.url.starts_with(&format!("{}{}/", s.raw_prefix, s.commit)));
        assert!(src.is_some(), "{} is not from a pinned source", e.url);
        assert!(e.size <= sources::MAX_BUILD_FILE, "{}", e.url);
    }
    // Header source lines follow REGRESS order and their counts add up to the rows.
    let counts: Vec<(String, usize)> = m
        .comments
        .iter()
        .filter_map(|c| c.strip_prefix("source "))
        .map(|c| {
            let name = c.split(' ').next().unwrap().to_string();
            let n = c
                .split(" files=")
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap();
            (name, n.parse().unwrap())
        })
        .collect();
    let names: Vec<&str> = counts.iter().map(|(n, _)| n.as_str()).collect();
    let want: Vec<&str> = sources::REGRESS.iter().map(|s| s.name).collect();
    assert_eq!(names, want);
    assert_eq!(
        counts.iter().map(|(_, n)| n).sum::<usize>(),
        m.entries.len()
    );
    for s in sources::REGRESS {
        let prefix = format!("{}{}/", s.raw_prefix, s.commit);
        let rows = m
            .entries
            .iter()
            .filter(|e| e.url.starts_with(&prefix))
            .count();
        let header = counts.iter().find(|(n, _)| n == s.name).unwrap().1;
        assert_eq!(rows, header, "{}", s.name);
    }
    for s in sources::REGRESS {
        let line = format!("source {} {} {} files=", s.name, s.repo, s.commit);
        assert!(
            m.comments.iter().any(|c| c.starts_with(&line)),
            "header misses {}",
            s.name
        );
    }
    assert_eq!(
        m.comments.last().map(String::as_str),
        Some("columns: git_blob_sha1<TAB>size<TAB>url")
    );
}

#[test]
fn smoke_manifest_parses_with_the_same_rules() {
    let m = manifest::load(&Tier::Smoke.manifest()).unwrap();
    assert!(m.entries.len() > 1000, "{}", m.entries.len());
}

#[test]
fn sources_are_pinned_https_on_allowed_hosts() {
    let mut names = std::collections::HashSet::new();
    for s in sources::REGRESS {
        assert!(names.insert(s.name), "duplicate source name {}", s.name);
        assert!(
            s.name.bytes().all(|b| b.is_ascii_alphanumeric()),
            "{}",
            s.name
        );
        assert_eq!(s.commit.len(), 40, "{}", s.name);
        assert!(
            s.commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert!(s.repo.starts_with("https://"), "{}", s.name);
        manifest::check_url(&s.url("x.pdf")).unwrap();
        assert!(s.raw_prefix.ends_with('/'), "{}", s.name);
    }
}

#[test]
fn select_sorts_then_limits() {
    let rows = vec![
        entry("c.pdf", b"c"),
        entry("a.pdf", b"a"),
        entry("b.pdf", b"b"),
    ];
    let mut sorted: Vec<String> = rows.iter().map(|e| e.blob.clone()).collect();
    sorted.sort();
    let got: Vec<String> = select(rows.clone(), Some(2))
        .into_iter()
        .map(|e| e.blob)
        .collect();
    assert_eq!(got, sorted[..2]);
    assert_eq!(select(rows.clone(), None).len(), 3);
    assert_eq!(select(rows.clone(), Some(0)).len(), 0);
    assert_eq!(select(rows, Some(9)).len(), 3);
}

#[test]
fn exit_codes() {
    let r = |failed: usize, integrity: usize| Report {
        total: 200,
        failed: vec!["x".into(); failed],
        integrity: vec!["y".into(); integrity],
        ..Report::default()
    };
    assert_eq!(r(0, 0).exit_code(), 0);
    assert_eq!(r(2, 0).exit_code(), 0, "2 of 200 is within max(1, 2)");
    assert_eq!(r(3, 0).exit_code(), 75);
    assert_eq!(r(0, 1).exit_code(), 1);
    assert_eq!(r(3, 1).exit_code(), 1, "integrity wins over infrastructure");
}

#[test]
fn concurrent_fetches_of_one_tier_do_not_clash() {
    let d = scratch("concurrent");
    let curl = fake_curl(&d);
    let mut entries = Vec::new();
    for i in 0..40 {
        let name = format!("c{i}.pdf");
        std::fs::write(d.join("serve").join(&name), name.as_bytes()).unwrap();
        entries.push(entry(&name, name.as_bytes()));
    }
    let out = d.join("out");
    let reports: Vec<Report> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..3)
            .map(|_| s.spawn(|| fetch(&entries, &out, &opts(curl.clone()))))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for r in &reports {
        assert!(r.failed.is_empty() && r.integrity.is_empty(), "{r:?}");
    }
    let names: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 40, "no .part files left: {names:?}");
    std::fs::remove_dir_all(&d).unwrap();
}
