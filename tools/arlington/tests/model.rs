//! specs/tooling/arlington.md "Tests": parsing, checks, generation and pinning on our own
//! invented fixtures (no Arlington data is committed).

use std::path::{Path, PathBuf};
use std::process::Command;

use jpdf_arlington::model::{
    Gate, Indirect, KeyName, Model, Required, Since, TypeAlt, ValueType, Version,
};
use jpdf_arlington::{Error, Source, check, fetch, generate, load, parse};

fn v(major: u8, minor: u8) -> Version {
    Version { major, minor }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/good")
}

#[cfg(test)]
fn good() -> Model {
    load(&Source::Dir(fixtures())).expect("fixtures load")
}

#[cfg(test)]
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn fixtures_parse_into_the_expected_model() {
    let m = good();
    assert_eq!(
        m.objects.keys().collect::<Vec<_>>(),
        ["ArrayOfLeaf", "Leaf", "Root"]
    );
    assert_eq!(m.key_count(), 8);
    let root = &m.objects["Root"];
    let mode = &root.keys[2];
    assert_eq!(mode.key, KeyName::Name("Mode".into()));
    assert_eq!(mode.types.len(), 2);
    assert_eq!(mode.types[0].ty.value, ValueType::Integer);
    assert_eq!(mode.types[1].ty.gates, vec![Gate::SinceVersion(v(1, 5))]);
    assert_eq!(mode.since, Since::Version(v(1, 2)));
    assert_eq!(mode.deprecated, Some(v(2, 0)));
    assert_eq!(mode.required, Required::If("fn:IsPresent(Kids)".into()));
    assert_eq!(mode.indirect, vec![Indirect::May, Indirect::MustBeDirect]);
    assert!(mode.inheritable);
    assert_eq!(mode.default_value, "0");
    assert_eq!(mode.possible_values, "[0,1];[Fast]");

    let ext = &root.keys[3];
    assert_eq!(
        ext.types[0].ty.gates,
        vec![Gate::IsPdfVersion(v(1, 7)), Gate::Extension("XY_1".into())]
    );
    assert_eq!(
        ext.types[1].links[0].gates,
        vec![Gate::SinceVersion(v(2, 0))]
    );
    assert_eq!(
        ext.since,
        Since::Expr("fn:Eval(fn:Extension(XY_1,1.7) || 2.0)".into())
    );
    assert_eq!(
        ext.indirect,
        vec![Indirect::Expr(
            "fn:MustBeIndirect(fn:BeforeVersion(2.0))".into()
        )]
    );
    assert_eq!(root.keys[4].key, KeyName::Wildcard);
    assert_eq!(
        root.keys[4].since,
        Since::Extension {
            name: "XY_1".into(),
            version: None
        }
    );
    let arr = &m.objects["ArrayOfLeaf"];
    assert_eq!(arr.keys[0].key, KeyName::Index(0));
    assert_eq!(arr.keys[1].key, KeyName::IndexRepeat(1));
    assert!(arr.keys[1].types[1].links.is_empty());
    let leaf = &m.objects["Leaf"].keys[0];
    assert_eq!(
        leaf.since,
        Since::Extension {
            name: "XY_1".into(),
            version: Some(v(1, 6))
        }
    );
    assert_eq!(leaf.special_case, "fn:Eval(@Size>0)");
}

/// Each broken model must fail with exactly the expected problem.
#[test]
fn checks_reject_inconsistent_models() {
    type Edit = fn(&mut Model);
    let cases: [(&str, Edit); 7] = [
        ("link to missing object Nope", |m| {
            m.objects.get_mut("Root").unwrap().keys[1].types[0].links[0].value = "Nope".into();
        }),
        ("duplicate key", |m| {
            let k = m.objects["Leaf"].keys[0].clone();
            m.objects.get_mut("Leaf").unwrap().keys.push(k);
        }),
        ("3 IndirectReference values for 2 types", |m| {
            m.objects.get_mut("Root").unwrap().keys[2]
                .indirect
                .push(Indirect::Must);
        }),
        ("deprecated in 1.1, before it appeared in 1.2", |m| {
            m.objects.get_mut("Root").unwrap().keys[2].deprecated = Some(v(1, 1));
        }),
        ("array has no link", |m| {
            m.objects.get_mut("Root").unwrap().keys[1].types[0]
                .links
                .clear();
        }),
        ("name cannot link to objects", |m| {
            let link = m.objects["Root"].keys[1].types[0].links[0].clone();
            m.objects.get_mut("Root").unwrap().keys[0].types[0]
                .links
                .push(link);
        }),
        ("type integer listed twice", |m| {
            let k = &mut m.objects.get_mut("Root").unwrap().keys[2];
            let alt: TypeAlt = k.types[0].clone();
            k.types[1] = alt;
        }),
    ];
    // Deprecated in the version that introduced the key is allowed (Arlington does it).
    let mut same = good();
    same.objects.get_mut("Root").unwrap().keys[2].deprecated = Some(v(1, 2));
    assert!(check::check(&same).is_ok());
    for (want, edit) in cases {
        let mut m = good();
        edit(&mut m);
        match check::check(&m) {
            Err(Error::Check(p)) => {
                assert_eq!(p.len(), 1, "{want}: {p:?}");
                assert!(p[0].ends_with(want), "{want}: {p:?}");
            }
            other => panic!("{want}: {other:?}"),
        }
    }
}

#[test]
fn bad_rows_name_the_file_line_and_column() {
    let dir = scratch("bad-rows");
    let h = parse::HEADER.join("\t");
    std::fs::write(
        dir.join("Foo.tsv"),
        format!(
            "{h}\nType\tname\t1.0\t\tTRUE\tFALSE\tFALSE\t\t\t\t\t\n\
             A\tnumbr\t1.0\t\tTRUE\tFALSE\tFALSE\t\t\t\t\t\n"
        ),
    )
    .unwrap();
    let e = load(&Source::Dir(dir)).unwrap_err().to_string();
    assert!(e.starts_with("Foo.tsv:3: Type: \"numbr\""), "{e}");
}

#[test]
fn read_dir_caps_file_size_before_reading() {
    let dir = scratch("big-file");
    let f = std::fs::File::create(dir.join("Big.tsv")).unwrap();
    f.set_len(parse::MAX_FILE_BYTES as u64 + 1).unwrap();
    assert!(matches!(
        jpdf_arlington::read_dir(&dir),
        Err(Error::Limit(_))
    ));
}

#[test]
fn generated_tables_are_deterministic_and_compile() {
    let m = good();
    let a = generate::render(&m, "fixtures");
    assert_eq!(a, generate::render(&good(), "fixtures"));
    assert!(a.starts_with("// @generated"));
    assert!(a.contains("pub const OBJECT_COUNT: usize = 3;"));
    assert!(a.contains("pub const KEY_COUNT: usize = 8;"));
    assert!(a.contains(
        "TypeAlt { gates: &[Gate::IsPdfVersion(Version { major: 1, minor: 7 }), \
         Gate::Extension(\"XY_1\")], ty: ValueType::Dictionary, \
         links: &[Link { gates: &[], object: \"Leaf\" }, ] }"
    ));
    // Control characters in the source line can't break out of the comment.
    assert!(!generate::render(&m, "x\nfn evil() {}").contains("\nfn evil"));

    // Compile the tables plus a test that reads them back.
    let dir = scratch("gen-compile");
    let src = format!(
        "{a}\n#[test]\nfn readback() {{\n\
         assert!(OBJECTS.windows(2).all(|w| w[0].name < w[1].name));\n\
         let root = OBJECTS.iter().find(|o| o.name == \"Root\").unwrap();\n\
         assert_eq!(root.keys.len(), 5);\n\
         assert_eq!(root.keys[2].required, Required::If(\"fn:IsPresent(Kids)\"));\n\
         assert_eq!(root.keys[2].indirect, &[Indirect::May, Indirect::MustBeDirect]);\n\
         assert_eq!(root.keys[2].deprecated, Some(Version {{ major: 2, minor: 0 }}));\n\
         assert_eq!(root.keys[4].key, KeyName::Wildcard);\n\
         assert_eq!(OBJECTS.iter().map(|o| o.keys.len()).sum::<usize>(), KEY_COUNT);\n}}\n"
    );
    std::fs::write(dir.join("tables.rs"), src).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let exe = dir.join("tables-test");
    let st = Command::new(&rustc)
        .args(["--edition", "2024", "--test", "-o"])
        .arg(&exe)
        .arg(dir.join("tables.rs"))
        .status()
        .unwrap();
    assert!(st.success(), "generated tables do not compile");
    assert!(Command::new(&exe).status().unwrap().success());
}

#[cfg(test)]
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {o:?}");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// A local repository with the fixtures under `tsv/latest`, standing in for the pinned one.
#[cfg(test)]
fn fixture_repo(name: &str) -> (PathBuf, String, String) {
    let dir = scratch(name);
    git(&dir, &["init", "-q"]);
    let tsv = dir.join(fetch::TSV_DIR);
    std::fs::create_dir_all(&tsv).unwrap();
    for (n, bytes) in jpdf_arlington::read_dir(&fixtures()).unwrap() {
        std::fs::write(tsv.join(n), bytes).unwrap();
    }
    std::fs::write(dir.join("README.md"), "outside tsv/latest\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "--no-gpg-sign", "-m", "fixtures"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]);
    let tree = git(&dir, &["rev-parse", "HEAD:tsv/latest"]);
    (dir, commit, tree)
}

#[test]
fn pinned_loading_checks_commit_and_tree() {
    let (dir, commit, tree) = fixture_repo("pin-repo");
    let files = fetch::load_verified(&dir.join(".git"), &commit, &tree).unwrap();
    assert_eq!(files, jpdf_arlington::read_dir(&fixtures()).unwrap());
    let model = parse::parse_files(&files).unwrap();
    assert_eq!(model, good());

    let e = fetch::load_verified(&dir.join(".git"), &commit, &commit).unwrap_err();
    assert!(e.to_string().contains("tree is"), "{e}");
    let other = "0000000000000000000000000000000000000000";
    assert!(fetch::load_verified(&dir.join(".git"), other, &tree).is_err());
    // A symbolic name that resolves to the commit is not the pin.
    let e = fetch::load_verified(&dir.join(".git"), "HEAD", &tree).unwrap_err();
    assert!(e.to_string().contains("pinned HEAD"), "{e}");
}

#[test]
fn pinned_loading_rejects_non_tsv_entries() {
    let (dir, _, _) = fixture_repo("pin-extra");
    std::fs::write(dir.join(fetch::TSV_DIR).join("notes.md"), "x\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "--no-gpg-sign", "-m", "extra"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]);
    let tree = git(&dir, &["rev-parse", "HEAD:tsv/latest"]);
    let e = fetch::load_verified(&dir.join(".git"), &commit, &tree).unwrap_err();
    assert!(e.to_string().contains("notes.md is not a .tsv file"), "{e}");
}

/// The id of `path` in HEAD, and the path of its loose object file.
#[cfg(test)]
fn loose(dir: &Path, path: &str) -> (String, PathBuf) {
    let id = git(dir, &["rev-parse", &format!("HEAD:{path}")]);
    let file = dir.join(".git/objects").join(&id[..2]).join(&id[2..]);
    assert!(file.is_file(), "{file:?}");
    (id, file)
}

#[test]
fn pinned_loading_ignores_replace_refs() {
    let (dir, commit, tree) = fixture_repo("pin-replace");
    let (leaf, _) = loose(&dir, "tsv/latest/Leaf.tsv");
    let (readme, _) = loose(&dir, "README.md");
    // `git replace` written by hand: Leaf.tsv's blob would read as README.md.
    let refs = dir.join(".git/refs/replace");
    std::fs::create_dir_all(&refs).unwrap();
    std::fs::write(refs.join(&leaf), format!("{readme}\n")).unwrap();
    let files = fetch::load_verified(&dir.join(".git"), &commit, &tree).unwrap();
    assert_eq!(files, jpdf_arlington::read_dir(&fixtures()).unwrap());
}

#[cfg(unix)]
#[test]
fn pinned_loading_detects_a_swapped_object_file() {
    let (dir, commit, tree) = fixture_repo("pin-swap");
    let (_, leaf) = loose(&dir, "tsv/latest/Leaf.tsv");
    let (_, readme) = loose(&dir, "README.md");
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&leaf, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::copy(&readme, &leaf).unwrap();
    let e = fetch::load_verified(&dir.join(".git"), &commit, &tree).unwrap_err();
    assert!(e.to_string().contains("git fsck"), "{e}");
}

#[test]
fn pinned_loading_checks_blob_sizes_before_reading() {
    let (dir, _, _) = fixture_repo("pin-big");
    let big = format!("{}\n", "x".repeat(parse::MAX_FILE_BYTES));
    std::fs::write(dir.join(fetch::TSV_DIR).join("Big.tsv"), big).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "--no-gpg-sign", "-m", "big"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]);
    let tree = git(&dir, &["rev-parse", "HEAD:tsv/latest"]);
    match fetch::load_verified(&dir.join(".git"), &commit, &tree) {
        // "(max …)" comes only from the `--batch-check` pass, before any blob is read.
        Err(Error::Limit(m)) => assert_eq!(m, "Big.tsv is 1048577 bytes (max 1048576)"),
        other => panic!("{other:?}"),
    }
}

/// A broken cache inside another repository is refused; git never walks up to the parent.
#[test]
fn a_broken_cache_is_refused_not_discovered_upwards() {
    let parent = scratch("pin-parent");
    git(&parent, &["init", "-q"]);
    let cache = parent.join("target/arlington");
    let repo = fetch::repo_path(&cache);
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("HEAD"), "").unwrap();
    let e = fetch::load_pinned(&cache).unwrap_err();
    assert!(e.to_string().contains("not a usable cache"), "{e}");
    assert!(!parent.join(".git/shallow").exists());
}

#[test]
fn the_pin_is_a_full_commit_and_tree_id() {
    for id in [fetch::COMMIT, fetch::TSV_TREE] {
        assert_eq!(id.len(), 40);
        assert!(
            id.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
}

#[test]
fn cli_exit_codes() {
    let exe = env!("CARGO_BIN_EXE_jpdf-arlington");
    let run = |args: &[&str]| Command::new(exe).args(args).output().unwrap();
    let f = fixtures();
    let f = f.to_str().unwrap();
    let ok = run(&["check", "--tsv", f]);
    assert_eq!(ok.status.code(), Some(0), "{ok:?}");
    assert!(String::from_utf8_lossy(&ok.stderr).contains("3 objects, 8 keys, checks pass"));

    let out = scratch("cli-gen").join("sub/tables.rs");
    let gen_run = run(&["generate", "--tsv", f, "--out", out.to_str().unwrap()]);
    assert_eq!(gen_run.status.code(), Some(0), "{gen_run:?}");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        generate::render(&good(), &Source::Dir(fixtures()).describe())
    );

    let missing = scratch("cli-missing").join("nope");
    assert_eq!(
        run(&["check", "--tsv", missing.to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(run(&["generate", "--tsv", f]).status.code(), Some(2));
    assert_eq!(run(&[]).status.code(), Some(2));
}
