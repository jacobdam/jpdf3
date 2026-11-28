//! specs/tooling/admit.md "Tests". Tool output fixtures are shaped like the real tools' output
//! (cargo 1.100, cargo-deny 0.21.0, cargo-vet 0.10.3, the judge's `check_deps`), trimmed to
//! what matters.

use super::*;

fn p(line: &str) -> Result<Options, String> {
    parse(
        &line
            .split(' ')
            .filter(|a| !a.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )
}

fn opts() -> Options {
    p("miniz_oxide@0.8.9 --work /w/probe").unwrap()
}

fn cap(code: i32, stdout: &str, stderr: &str) -> Captured {
    let (stdout, stderr) = (stdout.to_owned(), stderr.to_owned());
    Captured {
        code: Some(code),
        stdout,
        stderr,
    }
}

fn failed(c: &Checked) -> bool {
    matches!(c.outcome, Outcome::Failed(_))
}

fn pkg(name: &str, build_time: bool) -> Package {
    let (name, version, license) = (name.to_owned(), "1.0.0".to_owned(), Some("MIT".to_owned()));
    Package {
        name,
        version,
        license,
        build_time,
    }
}

fn lines(c: &Checked) -> String {
    c.details.join("\n")
}

// ---- parse ----

#[test]
fn parses_target_and_flags() {
    let (work, python) = (PathBuf::from("/w/probe"), "python3".to_owned());
    let (name, version) = ("miniz_oxide".to_owned(), "0.8.9".to_owned());
    assert_eq!(
        opts(),
        Options {
            name,
            version,
            work,
            features: Vec::new(),
            no_default_features: false,
            python
        }
    );
    let o = p("--features std,simd --python /opt/py zune-jpeg@0.5.0-rc.1+b.2 --no-default-features --work /w/x").unwrap();
    let got = (
        o.name.as_str(),
        o.version.as_str(),
        o.python.as_str(),
        o.work,
        o.features.join(" "),
        o.no_default_features,
    );
    assert_eq!(
        got,
        (
            "zune-jpeg",
            "0.5.0-rc.1+b.2",
            "/opt/py",
            PathBuf::from("/w/x"),
            "std simd".to_owned(),
            true
        )
    );
}

#[test]
fn feature_lists() {
    let f = |list: &str| parse(&["a@1.0.0", "--work", "/w", "--features", list].map(str::to_owned));
    let names = |list: &str| f(list).map(|o| o.features.join(" "));
    assert_eq!(names("a_b-c+d.E9").unwrap(), "a_b-c+d.E9");
    // At the limits: a 64-byte name, 32 names.
    let (at_len, at_count) = ("f".repeat(MAX_FEATURE), vec!["f"; MAX_FEATURES].join(","));
    assert_eq!(names(&at_len).unwrap(), at_len);
    assert_eq!(f(&at_count).unwrap().features.len(), MAX_FEATURES);
    let (past_len, past_count) = (
        "f".repeat(MAX_FEATURE + 1),
        vec!["f"; MAX_FEATURES + 1].join(","),
    );
    let bad = [
        "", "a,,b", "a,", ",a", "dep:x", "a/b", "a\"b", "a\\b", "a b", "é",
    ];
    for bad in bad.into_iter().chain([past_len.as_str(), &past_count]) {
        assert!(f(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn rejects_missing_repeated_and_unknown_arguments() {
    for args in [
        "--work /w",
        "a@1.0.0",
        "a@1.0.0 --work",
        "a@1.0.0 b@1.0.0 --work /w",
        "a --work /w",
        "a@1.0.0 --work /w --work /v",
        "a@1.0.0 --work /w --python p --python q",
        "a@1.0.0 --work /w --python",
        "a@1.0.0 --work /w --offline",
        "a@1.0.0 --work /w --features",
        "a@1.0.0 --work /w --features x --features y",
        "a@1.0.0 --work /w --no-default-features --no-default-features",
        "a@1.0.0 --work /w --all-features",
    ] {
        assert!(p(args).is_err(), "{args}");
    }
    let empty_python = ["a@1.0.0", "--work", "/w", "--python", ""].map(str::to_owned);
    assert!(parse(&empty_python).is_err());
}

#[test]
fn crate_names() {
    let n = |name: &str| parse(&[format!("{name}@1.0.0"), "--work".into(), "/w".into()]);
    for good in ["a", "A9", "a_b-c", &"a".repeat(MAX_NAME)] {
        assert!(n(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "9a",
        "_a",
        "a b",
        "a\"b",
        "a=b",
        "a.b",
        "a/b",
        "é",
        &"a".repeat(MAX_NAME + 1),
    ] {
        assert!(n(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn exact_versions_only() {
    let v = |version: &str| parse(&[format!("a@{version}"), "--work".into(), "/w".into()]);
    // At the limit: "1.0.0-" plus 58 letters is 64 bytes.
    let at = format!("1.0.0-{}", "a".repeat(MAX_VERSION - 6));
    for good in "0.0.0 1.2.3 10.20.30 1.0.0-rc.1 1.0.0+b.7 1.0.0-a-b+c-d"
        .split(' ')
        .chain([at.as_str()])
    {
        assert!(v(good).is_ok(), "{good}");
    }
    let past = format!("{at}a");
    let bad = "1 1.2 1.2.3.4 01.2.3 1.02.3 1.2.03 ^1.2.3 =1.2.3 ~1.2.3 1.2.* 1.2.3- 1.2.3+ 1.2.3+a+b \
               1.2.3-a_b 1.2.3-\" -1.2.3 1..3";
    for bad in bad.split(' ').chain(["", "1.2.3 ", &past]) {
        assert!(v(bad).is_err(), "{bad:?}");
    }
}

// ---- probe ----

#[test]
fn manifest_pins_the_one_dependency() {
    let want = "[package]\nname = \"jpdf-admit-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
                [dependencies.miniz_oxide]\nversion = \"=0.8.9\"\ndefault-features = true\nfeatures = []\n\n[workspace]\n";
    assert_eq!(manifest(&opts()), want);
    let o = p("miniz_oxide@0.8.9 --work /w --no-default-features --features std,a+b").unwrap();
    let want = want.replace(
        "default-features = true\nfeatures = []",
        "default-features = false\nfeatures = [\"std\", \"a+b\"]",
    );
    assert_eq!(manifest(&o), want);
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-admit-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    fs::canonicalize(d).unwrap()
}

#[test]
fn work_dir_rules() {
    use std::os::unix::fs::symlink;
    let base = scratch("work");
    let never = |_: &Path| false;
    assert_eq!(
        check_work(&base.join("new"), &never).unwrap(),
        base.join("new")
    );
    symlink(base.join("nowhere"), base.join("link")).unwrap();
    fs::write(base.join("file"), b"").unwrap();
    // Relative, existing (a directory, a dangling symlink), a missing or non-directory parent,
    // ending in `..`.
    for bad in [
        PathBuf::from("rel/new"),
        base.clone(),
        base.join("link"),
        base.join("missing/new"),
    ] {
        assert!(check_work(&bad, &never).is_err(), "{bad:?}");
    }
    for bad in [base.join("file/new"), base.join("new/..")] {
        assert!(check_work(&bad, &never).is_err(), "{bad:?}");
    }
    // A checkout at the parent, further up, or behind a symlinked parent.
    let (at, up) = (base.clone(), base.parent().unwrap().to_path_buf());
    assert!(check_work(&base.join("new"), &|a: &Path| a == at).is_err());
    assert!(check_work(&base.join("new"), &|a: &Path| a == up).is_err());
    fs::create_dir(base.join("real")).unwrap();
    symlink(base.join("real"), base.join("alias")).unwrap();
    let real = base.join("real");
    assert!(check_work(&base.join("alias/new"), &|a: &Path| a == real).is_err());
    assert_eq!(
        check_work(&base.join("alias/new"), &never).unwrap(),
        base.join("real/new")
    );
}

#[test]
fn has_git_sees_dirs_and_worktree_files() {
    let base = scratch("git");
    assert!(!has_git(&base));
    fs::create_dir_all(base.join("repo/.git")).unwrap();
    assert!(has_git(&base.join("repo")));
    fs::create_dir(base.join("wt")).unwrap();
    fs::write(base.join("wt/.git"), b"gitdir: x\n").unwrap();
    assert!(has_git(&base.join("wt")));
    // The live check refuses this repository's own tree.
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-probe");
    assert!(
        check_work(&here, &has_git)
            .unwrap_err()
            .contains("inside the git checkout")
    );
}

#[test]
fn write_probe_creates_new_files_only() {
    let base = scratch("write");
    let (repo, probe) = (base.join("repo"), base.join("probe"));
    fs::create_dir_all(repo.join("supply-chain")).unwrap();
    let copied: Vec<&str> = STORE.into_iter().chain([TOOLCHAIN]).collect();
    for (i, rel) in copied.iter().enumerate() {
        fs::write(repo.join(rel), format!("# {i} {rel}\n")).unwrap();
    }
    write_probe(&probe, &repo, &opts()).unwrap();
    assert_eq!(
        fs::read_to_string(probe.join("Cargo.toml")).unwrap(),
        manifest(&opts())
    );
    assert_eq!(fs::read(probe.join("src/lib.rs")).unwrap(), b"");
    // The toolchain and the three store files, each byte for byte.
    assert_eq!(copied.len(), 4);
    for rel in &copied {
        assert_eq!(
            fs::read(probe.join(rel)).unwrap(),
            fs::read(repo.join(rel)).unwrap(),
            "{rel}"
        );
    }
    // A second run into the same directory fails rather than reusing it; so does a missing file.
    assert!(write_probe(&probe, &repo, &opts()).is_err());
    for (i, rel) in copied.iter().enumerate() {
        let saved = fs::read(repo.join(rel)).unwrap();
        fs::remove_file(repo.join(rel)).unwrap();
        let next = base.join(format!("probe-missing-{i}"));
        assert!(write_probe(&next, &repo, &opts()).is_err(), "{rel}");
        fs::write(repo.join(rel), saved).unwrap();
    }
}

#[test]
fn commands_are_pinned() {
    let (probe, repo) = (Path::new("/w/p"), Path::new("/r"));
    let line = |c: Cmd| format!("{} {}", c.program, c.args.join(" "));
    assert_eq!(line(lock_cmd()), "cargo generate-lockfile --color never");
    assert_eq!(
        line(metadata_cmd()),
        "cargo metadata --format-version 1 --locked --color never"
    );
    assert_eq!(
        line(vet_cmd(probe)),
        "cargo vet --manifest-path /w/p/Cargo.toml --output-format json"
    );
    assert_eq!(
        vet_cmd(Path::new("/a b")).args.get(2).unwrap(),
        "/a b/Cargo.toml"
    );
    let deny =
        "cargo deny --color never --manifest-path /w/p/Cargo.toml --config /r/deny.toml check";
    assert_eq!(line(deny_cmd(probe, repo)), deny);
    let scan = unsafe_cmd("py", probe, repo);
    assert_eq!(scan.program, "py");
    assert_eq!(
        scan.args,
        ["-I", "-c", UNSAFE_SNIPPET, "/r/ops/judge", "/w/p"]
    );
    assert!(UNSAFE_SNIPPET.contains("policy.check_deps(Path(sys.argv[2]), r)"));
    // A path with a space stays one argument.
    assert_eq!(
        deny_cmd(Path::new("/a b"), repo).args.get(4).unwrap(),
        "/a b/Cargo.toml"
    );
}

// ---- cargo metadata ----

const META: &str = r#"{"packages":[
 {"name":"adler2","version":"2.0.1","license":"0BSD OR MIT OR Apache-2.0","source":"registry+https://github.com/rust-lang/crates.io-index","targets":[{"kind":["lib"]},{"kind":["bench"]}]},
 {"name":"jpdf-admit-probe","version":"0.0.0","license":null,"source":null,"targets":[{"kind":["lib"]}]},
 {"name":"libc","version":"0.2.170","license":"MIT OR Apache-2.0","source":"registry+https://github.com/rust-lang/crates.io-index","targets":[{"kind":["lib"]},{"kind":["custom-build"]}]},
 {"name":"serde_derive","version":"1.0.219","source":"registry+https://github.com/rust-lang/crates.io-index","targets":[{"kind":["proc-macro"]}]}
],"resolve":null,"version":1}"#;

#[test]
fn metadata_lists_third_party_packages() {
    let got = parse_metadata(META).unwrap();
    let text: Vec<String> = got
        .iter()
        .map(|p| format!("{}@{} {} {:?}", p.name, p.version, p.build_time, p.license))
        .collect();
    assert_eq!(
        text.join("\n"),
        "adler2@2.0.1 false Some(\"0BSD OR MIT OR Apache-2.0\")\nlibc@0.2.170 true Some(\"MIT OR Apache-2.0\")\n\
         serde_derive@1.0.219 true None"
    );
}

#[test]
fn metadata_shapes_it_does_not_know_are_errors() {
    let probe = r#"{"name":"jpdf-admit-probe","version":"0.0.0","source":null,"targets":[{"kind":["lib"]}]}"#;
    let dep =
        r#"{"name":"a","version":"1.0.0","source":"registry+x","targets":[{"kind":["lib"]}]}"#;
    let doc = |items: &[&str]| format!(r#"{{"packages":[{}]}}"#, items.join(","));
    assert!(parse_metadata(&doc(&[probe, dep])).is_ok());
    // A probe with no `source` key at all is the probe too.
    let bare = probe.replace(r#""source":null,"#, "");
    assert!(parse_metadata(&doc(&[&bare, dep])).is_ok());
    let edit = |from: &str, to: &str| {
        assert!(dep.contains(from), "{from}");
        doc(&[probe, &dep.replace(from, to)])
    };
    for bad in [
        "not json".to_owned(),
        r#"{"packages":{}}"#.to_owned(),
        doc(&[dep]),
        doc(&[probe]),
        doc(&[probe, probe, dep]),
        // A sourceless package that is not the probe, with no probe listed: only the name
        // decides that it isn't the probe.
        doc(&[&probe.replace("jpdf-admit-probe", "other"), dep]),
        edit(r#""source":"registry+x""#, r#""source":null"#),
        edit(r#","source":"registry+x""#, ""),
        edit(r#""source":"registry+x""#, r#""source":7"#),
        edit(r#""name":"a","#, ""),
        edit(r#""version":"1.0.0","#, ""),
        edit(r#","targets":[{"kind":["lib"]}]"#, ""),
        edit(r#"{"kind":["lib"]}"#, "{}"),
        edit(r#"["lib"]"#, "[1]"),
        edit(r#""source""#, r#""license":5,"source""#),
    ] {
        assert!(parse_metadata(&bad).is_err(), "{bad}");
    }
}

// ---- deny ----

const DENY_OK: &str = "advisories ok, bans ok, licenses ok, sources ok\n";
const DENY_FAIL: &str = "error[rejected]: failed to satisfy license requirements
   ┌─ /home/u/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ffmpeg-next-7.1.0/Cargo.toml:35:12
   │
35 │ license = \"WTFPL\"
warning[duplicate]: found 2 duplicate entries for crate 'shlex'
error[vulnerability]: something
   │
error[banned]: other
warning[x]: y
";

#[test]
fn deny_pass_and_rejection() {
    assert_eq!(
        deny_result(&cap(0, DENY_OK, ""), &[pkg("a", false)]),
        checked(Outcome::Ok, Vec::new())
    );
    let bad = deny_result(
        &cap(
            4,
            "advisories FAILED, bans ok, licenses FAILED, sources ok\n",
            DENY_FAIL,
        ),
        &[],
    );
    assert_eq!(bad.outcome, Outcome::Finding);
    assert_eq!(
        bad.details.join("\n"),
        "failed: advisories, licenses\nerror[rejected]: failed to satisfy license requirements\n  \
         ┌─ /home/u/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ffmpeg-next-7.1.0/Cargo.toml:35:12\n\
         error[vulnerability]: something\nerror[banned]: other"
    );
}

#[test]
fn deny_without_a_matching_summary_fails() {
    for c in [
        cap(1, "", "error: failed to load config\n"),
        cap(4, DENY_OK, DENY_FAIL),
        cap(
            0,
            "advisories ok, bans ok, licenses FAILED, sources ok\n",
            "",
        ),
        cap(0, "", ""),
        cap(4, "advisories ok, bans ok, licenses FAILED\n", DENY_FAIL),
        cap(
            4,
            "advisories FAILED, bans ok, sources FAILED, licenses ok\n",
            DENY_FAIL,
        ),
        cap(
            4,
            "advisories FAILED, bans ok, licenses failed, sources ok\n",
            DENY_FAIL,
        ),
        cap(
            4,
            "advisories ok, bans ok, licenses FAILED, sources ok, extra ok\n",
            DENY_FAIL,
        ),
    ] {
        assert!(failed(&deny_result(&c, &[])), "{c:?}");
    }
}

#[test]
fn deny_notes_ofl_licenses_whatever_the_result() {
    let mut font = pkg("font-data", true);
    font.license = Some("OFL-1.1 OR MIT".to_owned());
    let mut near = pkg("near", false);
    near.license = Some("OFL-1.0".to_owned());
    let packages = [pkg("a", false), font, near];
    let note = "note: font-data@1.0.0 is licensed OFL-1.1 OR MIT: OFL-1.1 is allowed for fonts only \
                (CONSTITUTION 10); check it by hand";
    let ok = deny_result(&cap(0, DENY_OK, ""), &packages);
    assert_eq!(ok, checked(Outcome::Ok, vec![note.to_owned()]));
    let summary = "advisories ok, bans ok, licenses FAILED, sources ok\n";
    let rejected = deny_result(&cap(4, summary, ""), &packages);
    assert_eq!(rejected.outcome, Outcome::Finding);
    assert_eq!(lines(&rejected), format!("failed: licenses\n{note}"));
    let broken = deny_result(&cap(1, "", "error: boom\n"), &packages);
    assert!(failed(&broken));
    assert_eq!(lines(&broken), note);
    // Any ASCII case counts, as cargo-deny reads license fields leniently.
    let mut lower = pkg("font-lower", false);
    lower.license = Some("ofl-1.1".to_owned());
    let c = deny_result(&cap(0, DENY_OK, ""), &[lower]);
    assert_eq!(
        lines(&c),
        "note: font-lower@1.0.0 is licensed ofl-1.1: OFL-1.1 is allowed for fonts only \
         (CONSTITUTION 10); check it by hand"
    );
}

// ---- the judge's dependency scan ----

const LIBC: &str = "dependency libc 0.2.170 contains `unsafe` (47 files) and is not allowlisted";

#[test]
fn judge_scan_results() {
    // The tomllib hint only when the quoted line names tomllib.
    let why = |c: Captured| match unsafe_result(&c).outcome {
        Outcome::Failed(w) => w,
        o => format!("not failed: {o:?}"),
    };
    let py = cap(1, "", "ModuleNotFoundError: No module named 'tomllib'\n");
    let hint = "judge dependency scan (needs Python 3.11+ for tomllib; see --python): exit 1: ";
    assert_eq!(
        why(py),
        format!("{hint}ModuleNotFoundError: No module named 'tomllib'")
    );
    let other = cap(1, "[]\n", "OSError: [Errno 13] Permission denied\n");
    assert_eq!(
        why(other),
        "judge dependency scan: exit 1: OSError: [Errno 13] Permission denied"
    );
    assert_eq!(
        unsafe_result(&cap(0, "[]\n", "")),
        checked(Outcome::Ok, Vec::new())
    );
    let hit = unsafe_result(&cap(0, &format!("[{LIBC:?}]\n"), ""));
    assert_eq!(hit, checked(Outcome::Finding, vec![LIBC.to_owned()]));
    for c in [
        cap(0, "[\"cargo metadata failed: boom\"]\n", ""),
        cap(1, "", "ModuleNotFoundError: No module named 'tomllib'\n"),
        cap(1, "[]\n", ""),
        cap(0, "", ""),
        cap(0, "{}\n", ""),
        cap(0, "[1]\n", ""),
        cap(0, "[]\nnot json\n", ""),
    ] {
        assert!(failed(&unsafe_result(&c)), "{c:?}");
    }
}

// ---- vet ----

/// cargo-vet 0.10.3's report for adler2 2.0.1 (exit 0), and for serde_derive 1.0.219 (exit
/// 255; `suggest` trimmed).
const VET_OK: &str = r#"{
  "conclusion": "success",
  "vetted_fully": [{"name": "adler2", "version": "2.0.1"}],
  "vetted_partially": [],
  "vetted_with_exemptions": []
}"#;
const VET_FAIL: &str = r#"{
  "conclusion": "fail (vetting)",
  "failures": [
    {"name": "proc-macro2", "version": "1.0.107", "missing_criteria": ["safe-to-deploy"]},
    {"name": "syn", "version": "2.0.119", "missing_criteria": ["safe-to-deploy", "safe-to-run"]}
  ],
  "suggest": {"suggestions": []}
}"#;
/// cargo-vet 0.10.3 with no store (`--store-path` to a missing directory), exit 255.
const VET_ERROR: &str = r#"{"error": {"message": "You must run 'cargo vet init' (store not found at /x)","severity": "error","causes": [],"labels": [],"related": []}}"#;

fn set(items: &[(&str, &str)]) -> Unvetted {
    let pairs = items
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()));
    Some(pairs.collect())
}

#[test]
fn vet_pass_and_rejection() {
    assert_eq!(
        vet_result(&cap(0, VET_OK, "")),
        (checked(Outcome::Ok, Vec::new()), set(&[]))
    );
    let (c, unvetted) = vet_result(&cap(255, VET_FAIL, ""));
    assert_eq!(c.outcome, Outcome::Finding);
    assert_eq!(
        lines(&c),
        "proc-macro2@1.0.107: no imported audit for safe-to-deploy\n\
         syn@2.0.119: no imported audit for safe-to-deploy, safe-to-run"
    );
    assert_eq!(
        unvetted,
        set(&[("proc-macro2", "1.0.107"), ("syn", "2.0.119")])
    );
    // Another `fail (...)`: a rejection, but vet didn't say what is unvetted.
    let violation = vet_result(&cap(255, r#"{"conclusion":"fail (violation)"}"#, ""));
    let want = checked(
        Outcome::Finding,
        vec!["conclusion: fail (violation)".to_owned()],
    );
    assert_eq!(violation, (want, None));
}

#[test]
fn vet_results_it_cannot_read_fail() {
    let why = "cargo vet: You must run 'cargo vet init' (store not found at /x)";
    assert_eq!(
        vet_result(&cap(255, VET_ERROR, "")),
        (failure(why.to_owned()), None)
    );
    let stderr = cap(101, "", "error: failed to run `cargo metadata`\n");
    assert_eq!(
        vet_result(&stderr),
        (
            failure(
                "cargo vet: no JSON report: exit 101: error: failed to run `cargo metadata`"
                    .to_owned()
            ),
            None
        )
    );
    for c in [
        cap(0, VET_FAIL, ""),
        cap(255, VET_OK, ""),
        cap(0, r#"{"conclusion":"fail (violation)"}"#, ""),
        cap(255, r#"{"conclusion":"fail (vetting)"}"#, ""),
        cap(255, r#"{"conclusion":"fail (vetting)","failures":{}}"#, ""),
        cap(255, r#"{"conclusion":"fail (vetting)","failures":[]}"#, ""),
        cap(
            255,
            r#"{"conclusion":"fail (vetting)","failures":[{"name":"a"}]}"#,
            "",
        ),
        cap(
            255,
            r#"{"conclusion":"fail (vetting)","failures":[{"version":"1.0.0"}]}"#,
            "",
        ),
        cap(255, r#"{"conclusion":"broken"}"#, ""),
        cap(255, r#"{"conclusion":"failed"}"#, ""),
        cap(0, r#"{"conclusion":7}"#, ""),
        cap(0, "", ""),
        cap(0, "success", ""),
    ] {
        let (r, unvetted) = vet_result(&c);
        assert!(failed(&r) && unvetted.is_none(), "{c:?}");
    }
}

// ---- build-time code ----

#[test]
fn build_time_needs_an_audit() {
    let macro2 = set(&[("proc-macro2", "1.0.0")]);
    // No build-time crate: nothing to check, whatever vet said (an unvetted plain crate is the
    // vet step's rejection, not this one's).
    for unvetted in [None, macro2.clone()] {
        assert_eq!(
            build_time_result(&[pkg("proc-macro2", false)], &unvetted),
            checked(Outcome::Ok, Vec::new())
        );
    }
    // Audited: vet passed, or the crate is not among its failures (the same name at another
    // version, or another name, doesn't count).
    for unvetted in [
        set(&[]),
        set(&[("proc-macro2", "2.0.0")]),
        set(&[("syn", "1.0.0")]),
    ] {
        let c = build_time_result(&[pkg("proc-macro2", true)], &unvetted);
        assert_eq!(
            c,
            checked(Outcome::Ok, vec!["proc-macro2@1.0.0: audited".to_owned()])
        );
    }
    let bad = build_time_result(
        &[
            pkg("serde_derive", true),
            pkg("proc-macro2", true),
            pkg("syn", false),
        ],
        &macro2,
    );
    assert_eq!(bad.outcome, Outcome::Finding);
    assert_eq!(
        lines(&bad),
        "serde_derive@1.0.0: audited\nproc-macro2@1.0.0: not audited"
    );
    // Vet didn't decide: a build-time crate can't be cleared.
    let unknown = build_time_result(&[pkg("a", false), pkg("proc-macro2", true)], &None);
    assert!(failed(&unknown));
    assert_eq!(lines(&unknown), "proc-macro2@1.0.0");
}

// ---- driver ----

/// A fake runner: answers by the command's first argument and records each call.
struct Fake {
    calls: Vec<String>,
    answers: Vec<(&'static str, Result<Captured, String>)>,
}

impl Fake {
    fn passing() -> Fake {
        let answers = vec![
            ("generate-lockfile", Ok(cap(0, "", ""))),
            ("metadata", Ok(cap(0, META, ""))),
            ("deny", Ok(cap(0, DENY_OK, ""))),
            ("-I", Ok(cap(0, "[]\n", ""))),
            ("vet", Ok(cap(0, VET_OK, ""))),
        ];
        Fake {
            calls: Vec::new(),
            answers,
        }
    }

    fn set(mut self, key: &str, answer: Result<Captured, String>) -> Fake {
        let slot = self.answers.iter_mut().find(|(k, _)| *k == key).unwrap();
        slot.1 = answer;
        self
    }

    fn drive(&mut self, write_ok: bool) -> (Log, Report) {
        let mut wrote = 0u32;
        let mut write = || {
            wrote = wrote.saturating_add(1);
            if write_ok {
                Ok(())
            } else {
                Err("write failed".to_owned())
            }
        };
        let mut run = |c: &Cmd| {
            let key = c.args.first().cloned().unwrap_or_default();
            self.calls.push(key.clone());
            let answer = self
                .answers
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, a)| a.clone());
            answer.unwrap_or_else(|| Err(format!("unexpected {key}")))
        };
        let r = drive(
            &opts(),
            Path::new("/w/p"),
            Path::new("/r"),
            &mut write,
            &mut run,
        );
        assert_eq!(wrote, 1);
        r
    }
}

const ALL: [&str; 5] = ["probe", "deny", "unsafe", "vet", "build-time"];
const CALLS: [&str; 5] = ["generate-lockfile", "metadata", "deny", "-I", "vet"];

fn steps_where(report: &Report, pick: fn(&Checked) -> bool) -> Vec<&'static str> {
    report
        .iter()
        .filter(|(_, c)| pick(c))
        .map(|(s, _)| *s)
        .collect()
}

/// vet's rejection of META's libc, a build-time crate.
fn vet_libc() -> Result<Captured, String> {
    let text = VET_FAIL
        .replace("proc-macro2", "libc")
        .replace("1.0.107", "0.2.170");
    Ok(cap(255, &text, ""))
}

#[test]
fn driver_runs_every_check_in_order() {
    let mut f = Fake::passing();
    let (log, report) = f.drive(true);
    assert_eq!(f.calls, CALLS);
    assert_eq!((log.names(), steps::exit_code(&log)), (ALL.to_vec(), 0));
    assert_eq!(steps_where(&report, |c| c.outcome == Outcome::Ok), ALL);
    assert_eq!(
        render(&opts(), &report, 0),
        "admit: probe: ok\nadmit: deny: ok\nadmit: unsafe: ok\nadmit: vet: ok\nadmit: build-time: ok\n  \
         libc@0.2.170: audited\n  serde_derive@1.0.219: audited\nadmit: miniz_oxide@0.8.9: admitted\n"
    );
}

#[test]
fn driver_keeps_going_after_a_rejection_or_failure() {
    // Each check rejects on its own, or with another; the others still run.
    let deny = Ok(cap(
        4,
        "advisories ok, bans ok, licenses FAILED, sources ok\n",
        DENY_FAIL,
    ));
    let scan = Ok(cap(0, &format!("[{LIBC:?}]\n"), ""));
    // syn is not build-time in META: vet rejects, build-time passes.
    let vet_syn = Ok(cap(255, &VET_FAIL.replace("proc-macro2", "syn-x"), ""));
    let both = Fake::passing()
        .set("deny", deny.clone())
        .set("-I", scan.clone());
    for (fake, rejected) in [
        (Fake::passing().set("deny", deny), "deny"),
        (Fake::passing().set("-I", scan), "unsafe"),
        (both, "deny, unsafe"),
        (Fake::passing().set("vet", vet_syn), "vet"),
        (Fake::passing().set("vet", vet_libc()), "vet, build-time"),
    ] {
        let mut f = fake;
        let (log, report) = f.drive(true);
        assert_eq!(
            (log.names(), f.calls.clone(), steps::exit_code(&log)),
            (ALL.to_vec(), CALLS.map(str::to_owned).to_vec(), 4),
            "{rejected}"
        );
        assert_eq!(
            steps_where(&report, |c| c.outcome == Outcome::Finding).join(", "),
            rejected
        );
        let text = render(&opts(), &report, 4);
        assert!(
            text.ends_with(&format!(
                "admit: miniz_oxide@0.8.9: rejected by {rejected}\n"
            )),
            "{text}"
        );
    }
    // A check that can't start fails its step only, and outranks another's rejection; vet
    // failing leaves build-time unable to clear META's build-time crates.
    let cannot = || Err("cannot run".to_owned());
    let scan_hit = Ok(cap(0, &format!("[{LIBC:?}]\n"), ""));
    for (fake, broken, rejected) in [
        (Fake::passing().set("deny", cannot()), "deny", ""),
        (Fake::passing().set("-I", cannot()), "unsafe", ""),
        (
            Fake::passing().set("deny", cannot()).set("-I", scan_hit),
            "deny",
            "unsafe",
        ),
        (Fake::passing().set("vet", cannot()), "vet, build-time", ""),
        (
            Fake::passing().set("vet", Ok(cap(255, VET_ERROR, ""))),
            "vet, build-time",
            "",
        ),
        (
            Fake::passing().set("-I", cannot()).set("vet", vet_libc()),
            "unsafe",
            "vet, build-time",
        ),
    ] {
        let mut f = fake;
        let (log, report) = f.drive(true);
        assert_eq!(
            (log.names(), f.calls.len(), steps::exit_code(&log)),
            (ALL.to_vec(), CALLS.len(), 1),
            "{broken}"
        );
        assert_eq!(steps_where(&report, failed).join(", "), broken);
        assert_eq!(
            steps_where(&report, |c| c.outcome == Outcome::Finding).join(", "),
            rejected
        );
        assert!(
            render(&opts(), &report, 1)
                .ends_with("admit: miniz_oxide@0.8.9: not decided: a step failed\n")
        );
    }
}

#[test]
fn driver_stops_when_the_probe_fails() {
    let lock_fail = Ok(cap(
        101,
        "",
        "error: no matching package named `nope` found\n",
    ));
    let meta_fail = Ok(cap(101, "", "error: the lock file needs to be updated\n"));
    let probe_only = r#"{"packages":[{"name":"jpdf-admit-probe","version":"0.0.0","source":null,"targets":[{"kind":["lib"]}]}]}"#;
    for (fake, write_ok, calls) in [
        (Fake::passing(), false, ""),
        (
            Fake::passing().set("generate-lockfile", lock_fail.clone()),
            true,
            "generate-lockfile",
        ),
        (
            Fake::passing().set("generate-lockfile", Err("no cargo".into())),
            true,
            "generate-lockfile",
        ),
        (
            Fake::passing().set("metadata", meta_fail),
            true,
            "generate-lockfile metadata",
        ),
        (
            Fake::passing().set("metadata", Err("no cargo".into())),
            true,
            "generate-lockfile metadata",
        ),
        (
            Fake::passing().set("metadata", Ok(cap(0, probe_only, ""))),
            true,
            "generate-lockfile metadata",
        ),
    ] {
        let mut f = fake;
        let (log, report) = f.drive(write_ok);
        assert_eq!(
            (f.calls.join(" "), log.names(), steps::exit_code(&log)),
            (calls.to_owned(), vec!["probe"], 1)
        );
        assert_eq!(steps_where(&report, failed), ["probe"]);
    }
    let (_, report) = Fake::passing()
        .set("generate-lockfile", lock_fail)
        .drive(true);
    assert_eq!(
        render(&opts(), &report, 1),
        "admit: probe: failed: cargo generate-lockfile: exit 101: error: no matching package named `nope` found\n\
         admit: miniz_oxide@0.8.9: not decided: a step failed\n"
    );
}

// ---- output ----

#[test]
fn failure_text_prefers_the_first_error_line() {
    let cargo = "    Updating crates.io index\nerror: failed to select a version for `a = \"=9.0.0\"`\n\
                 candidate versions found which didn't match: 1.0.0\nrequired by package `jpdf-admit-probe v0.0.0`\n";
    assert_eq!(
        cap(101, "", cargo).tail(),
        "exit 101: error: failed to select a version for `a = \"=9.0.0\"`"
    );
    let python = "Traceback (most recent call last):\n  File \"<string>\", line 4\nModuleNotFoundError: No module named 'tomllib'\n\n";
    assert_eq!(
        cap(1, "", python).tail(),
        "exit 1: ModuleNotFoundError: No module named 'tomllib'"
    );
    assert_eq!(cap(2, "", "").tail(), "exit 2: no output");
    let killed = Captured {
        code: None,
        stdout: String::new(),
        stderr: "x\n".to_owned(),
    };
    assert_eq!(killed.tail(), "exit signal: x");
}

#[test]
fn clean_escapes_controls_and_caps_length() {
    assert_eq!(clean("a\u{1b}[31mb\r\nc"), "a?[31mb??c");
    let marks = "\u{200E}\u{200F}\u{061C}";
    assert_eq!(
        clean(&format!("a{marks}b\u{200D}\u{061B}")),
        "a???b\u{200D}\u{061B}"
    );
    let hidden = "\u{2028}\u{2029}\u{202A}\u{202E}\u{2066}\u{2069}";
    assert_eq!(
        clean(&format!("a{hidden}b\u{2027}\u{202F}\u{2065}\u{206A}")),
        "a??????b\u{2027}\u{202F}\u{2065}\u{206A}"
    );
    let at = "x".repeat(MAX_LINE);
    assert_eq!(clean(&at), at);
    assert_eq!(clean(&format!("{at}y")), format!("{at}…"));
    // A multi-byte character that would cross the cap is cut whole.
    let near = "x".repeat(MAX_LINE - 1);
    assert_eq!(clean(&format!("{near}é")), format!("{near}…"));
}

#[test]
fn render_caps_details() {
    let at: Vec<String> = (0..MAX_DETAILS).map(|i| format!("d{i}")).collect();
    let text = |details: Vec<String>| {
        render(
            &opts(),
            &vec![("unsafe", checked(Outcome::Finding, details))],
            4,
        )
    };
    let full = text(at.clone());
    assert_eq!(full.lines().count(), MAX_DETAILS + 2);
    assert!(!full.contains("more"));
    let cut = text(at.into_iter().chain(["last".to_owned()]).collect());
    assert_eq!(cut.lines().count(), MAX_DETAILS + 3);
    assert!(cut.contains("  … 1 more\n") && !cut.contains("last"));
}
