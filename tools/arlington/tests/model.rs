//! specs/tooling/arlington.md "Tests": parsing, checks, generation and pinning on our own
//! invented fixtures, and the vendored Arlington data in `tools/arlington/data` (ADR 0007).

use std::path::{Path, PathBuf};
use std::process::Command;

use jpdf_arlington::model::{
    Gate, Indirect, KeyName, Model, Required, Since, TypeAlt, ValueType, Version,
};
use jpdf_arlington::{Error, Source, check, fetch, generate, load, parse, sha1, vendor};

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
    // Owner condition on #60: the header names the copyright, the license and the commit.
    let real = generate::render(&m, &Source::Vendored(PathBuf::new()).describe());
    let header: String = real.lines().take(3).collect::<Vec<_>>().join("\n");
    for want in [
        "Copyright PDF Association, Inc.",
        "Apache License 2.0",
        fetch::COMMIT,
    ] {
        assert!(header.contains(want), "{want}: {header}");
    }
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
    // The hardened builder: no inherited GIT_DIR or global config can redirect these calls.
    let o = fetch::git_command()
        .arg("-C")
        .arg(dir)
        .args(args)
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
    std::fs::write(dir.join("LICENSE"), "fixture license\n").unwrap();
    std::fs::write(dir.join("NOTICE.txt"), "fixture notice\n").unwrap();
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

/// A bare cache, initialized like `ensure_pinned` does, holding the fixture commit, copied
/// in over the local transport that `git_command` itself refuses.
#[cfg(test)]
#[cfg(unix)]
fn fixture_cache(name: &str) -> (PathBuf, PathBuf, String) {
    let (dir, commit, _) = fixture_repo(name);
    let repo = scratch(&format!("{name}-cache")).join("c.git");
    let st = fetch::git_command()
        .args([
            "init",
            "--bare",
            "--quiet",
            "--template=",
            "--object-format=sha1",
        ])
        .arg(&repo)
        .status()
        .unwrap();
    assert!(st.success());
    let st = fetch::git_command()
        .args(["-c", "protocol.file.allow=always", "--git-dir"])
        .arg(&repo)
        .args(["fetch", "--quiet"])
        .arg(&dir)
        .arg(&commit)
        .status()
        .unwrap();
    assert!(st.success());
    (dir, repo, commit)
}

/// Never fetched: the commit is already in every cache these tests use.
const UNUSED_URL: &str = "https://example.invalid/unused.git";

#[cfg(test)]
#[cfg(unix)]
fn plant_hook(repo: &Path, marker: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let hooks = repo.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A hook planted in the cache never runs (audit 0002 on #82), though plain git runs it.
#[cfg(unix)]
#[test]
fn cache_hooks_never_run() {
    let (_, repo, commit) = fixture_cache("pin-hook");
    let marker = repo.with_file_name("hook-ran");
    plant_hook(&repo, &marker);
    // `update-ref` fires reference-transaction; the pin ref is new, so it runs.
    fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF).unwrap();
    let pinned = fetch::git_out(&repo, &["rev-parse", fetch::PIN_REF], None).unwrap();
    assert_eq!(String::from_utf8_lossy(&pinned).trim(), commit);
    assert!(!marker.exists(), "the planted hook ran");
    // Control: the hook is valid, and git without the hardening runs it.
    let st = Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_DIR")
        .arg("--git-dir")
        .arg(&repo)
        .args(["update-ref", "refs/control", &commit])
        .status()
        .unwrap();
    assert!(st.success());
    assert!(marker.exists(), "control: plain git should run the hook");
}

/// A cache's own config may hold only what `git init --bare` writes; links to another
/// object store or common directory are refused too.
#[cfg(unix)]
#[test]
fn caches_with_foreign_config_or_links_are_refused() {
    let (_, repo, commit) = fixture_cache("pin-config");
    fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF).unwrap();
    let config = std::fs::read(repo.join("config")).unwrap();
    for (key, value) in [
        ("core.hooksPath", "/somewhere"),
        ("core.HooksPath", "/somewhere"),
        ("credential.helper", "!touch x"),
        ("include.path", "/somewhere/config"),
        ("url.https://evil.example/.insteadOf", "https://github.com/"),
        ("core.alternateRefsCommand", "touch x"),
        ("remote.origin.url", "https://example.invalid/x.git"),
    ] {
        let st = Command::new("git")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env_remove("GIT_DIR")
            .arg("--git-dir")
            .arg(&repo)
            .args(["config", key, value])
            .status()
            .unwrap();
        assert!(st.success(), "{key}");
        let e = fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("is not a usable cache (its config sets") && e.contains("; delete it"),
            "{key}: {e}"
        );
        std::fs::write(repo.join("config"), &config).unwrap();
    }
    fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF).unwrap();
    for link in [
        "commondir",
        "objects/info/alternates",
        "objects/info/http-alternates",
    ] {
        let path = repo.join(link);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "/somewhere\n").unwrap();
        let e = fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(e.contains(&format!("(it has {link})")), "{link}: {e}");
        std::fs::remove_file(&path).unwrap();
    }
    fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF).unwrap();
}

/// The cache itself, or the directories git writes into, can't be links elsewhere: a
/// gitfile, a symlinked cache, symlinked `objects` or `refs`, or a symlink deeper down
/// (review of #131).
#[cfg(unix)]
#[test]
fn caches_that_point_elsewhere_are_refused() {
    let (_, foreign, commit) = fixture_cache("pin-elsewhere");
    let base = scratch("pin-elsewhere-links");
    let check = |repo: &Path, what: &str| {
        let e = fetch::ensure_pinned(repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("is not a directory (symlinks and gitfiles are refused)"),
            "{what}: {e}"
        );
        // Nothing was written into the foreign repository.
        let r = fetch::git_out(
            &foreign,
            &["rev-parse", "--verify", "--quiet", fetch::PIN_REF],
            None,
        );
        assert!(
            r.is_err(),
            "{what}: the pin ref reached the foreign repository"
        );
    };
    let gitfile = base.join("gitfile.git");
    std::fs::write(&gitfile, format!("gitdir: {}\n", foreign.display())).unwrap();
    check(&gitfile, "gitfile");
    let link = base.join("link.git");
    std::os::unix::fs::symlink(&foreign, &link).unwrap();
    check(&link, "symlinked cache");
    for sub in ["objects", "refs"] {
        let (_, repo, _) = fixture_cache(&format!("pin-elsewhere-{sub}"));
        let real = repo.with_file_name(format!("{sub}-real"));
        std::fs::rename(repo.join(sub), &real).unwrap();
        std::os::unix::fs::symlink(&real, repo.join(sub)).unwrap();
        let e = fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("symlinks and gitfiles are refused"),
            "{sub}: {e}"
        );
    }
    // Deeper links: git would write the pin ref or the fetched pack through them.
    for (i, sub) in ["refs/jpdf", "objects/pack", "logs"].iter().enumerate() {
        let (_, repo, _) = fixture_cache(&format!("pin-deep-{i}"));
        let target = repo.join(sub);
        let real = repo.with_file_name("deep-real");
        std::fs::create_dir_all(&real).unwrap();
        if target.exists() {
            std::fs::remove_dir_all(&target).unwrap();
        }
        std::os::unix::fs::symlink(&real, &target).unwrap();
        let e = fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("is not a regular file or directory (symlinks are refused)"),
            "{sub}: {e}"
        );
        assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0, "{sub}");
    }
    // Files git rewrites through a symlink: the reflog of the pin ref (written even with
    // `logAllRefUpdates` off, since it exists), `FETCH_HEAD` and `shallow`.
    for (i, file) in ["logs/refs/jpdf/pinned", "FETCH_HEAD", "shallow"]
        .iter()
        .enumerate()
    {
        let (_, repo, _) = fixture_cache(&format!("pin-file-{i}"));
        let victim = repo.with_file_name("victim");
        std::fs::write(&victim, "untouched\n").unwrap();
        let link = repo.join(file);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let e = fetch::ensure_pinned(&repo, UNUSED_URL, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("is not a regular file or directory (symlinks are refused)"),
            "{file}: {e}"
        );
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "untouched\n",
            "{file}"
        );
    }
    // Control: the foreign repository itself is a fine cache.
    fetch::ensure_pinned(&foreign, UNUSED_URL, &commit, fetch::PIN_REF).unwrap();
}

/// HTTPS is allowed (a typo in the protocol settings would refuse it): a fetch from a
/// closed local HTTPS port fails at the connection, not with "not allowed".
#[cfg(unix)]
#[test]
fn https_reaches_the_network_layer() {
    let (_, commit, _) = fixture_repo("pin-https");
    let repo = scratch("pin-https-cache").join("c.git");
    let e = fetch::ensure_pinned(&repo, "https://127.0.0.1:1/x.git", &commit, fetch::PIN_REF)
        .unwrap_err()
        .to_string();
    assert!(e.contains("git fetch") && !e.contains("not allowed"), "{e}");
    assert!(e.contains("unable to access"), "{e}");
}

/// HTTPS is the only transport: a local path, `file://` and `ext::` are refused by git.
#[cfg(unix)]
#[test]
fn only_https_is_fetched() {
    let (dir, commit, _) = fixture_repo("pin-proto");
    let cache = scratch("pin-proto-cache");
    for (i, url) in [
        dir.display().to_string(),
        format!("file://{}", dir.display()),
        format!("ext::sh -c touch% {}/ext-ran", cache.display()),
    ]
    .iter()
    .enumerate()
    {
        let repo = cache.join(format!("c{i}.git"));
        let e = fetch::ensure_pinned(&repo, url, &commit, fetch::PIN_REF)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("git fetch") && e.contains("not allowed"),
            "{url}: {e}"
        );
        // The cache was created (with an empty template) and holds nothing.
        assert!(!repo.join("hooks").exists(), "{url}");
        assert!(fetch::git_out(&repo, &["cat-file", "-e", &commit], None).is_err());
    }
    assert!(!cache.join("ext-ran").exists());
}

#[test]
fn the_pin_is_a_full_commit_and_tree_id() {
    for id in [
        fetch::COMMIT,
        fetch::TSV_TREE,
        fetch::LICENSE_BLOB,
        fetch::NOTICE_BLOB,
    ] {
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

/// The repository root (two levels above this crate).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The real vendored data matches the pins and loads offline (owner ruling on #60).
#[test]
fn vendored_data_matches_the_pins_and_loads() {
    let data = repo_root().join(vendor::DATA_DIR);
    let m = load(&Source::Vendored(data.clone())).unwrap();
    assert_eq!(m.objects.len(), 613);
    assert_eq!(m.key_count(), 3985);
    assert!(m.objects.contains_key("Catalog"));
    // The same files through the unchecked reader give the same model.
    assert_eq!(m, load(&Source::Dir(data.join("tsv"))).unwrap());
    assert_eq!(
        Source::Vendored(data).describe(),
        format!("commit {} (tsv/latest, vendored)", fetch::COMMIT)
    );
}

/// Our SHA-1 and git ids agree with git itself, across the padding boundaries.
#[test]
fn git_ids_agree_with_git() {
    let dir = scratch("hash-object");
    let mut inputs: Vec<Vec<u8>> = (0..=130u8).map(|n| vec![n; usize::from(n)]).collect();
    inputs.push(b"line\r\nwith CR\r\n".to_vec());
    for data in inputs {
        let f = dir.join("blob");
        std::fs::write(&f, &data).unwrap();
        let want = git(&dir, &["hash-object", "--no-filters", f.to_str().unwrap()]);
        assert_eq!(
            sha1::hex(&sha1::object_id("blob", &data)),
            want,
            "len {}",
            data.len()
        );
    }
    let (_, _, tree) = fixture_repo("hash-tree");
    let files = jpdf_arlington::read_dir(&fixtures()).unwrap();
    assert_eq!(sha1::hex(&sha1::tree_id(&files)), tree);
}

/// A vendored copy of the fixtures, written by `vendor_from` from the fixture repository,
/// and the pins it must match.
#[cfg(test)]
fn fixture_vendored(name: &str) -> (PathBuf, String, String, String) {
    let (repo, commit, tree) = fixture_repo(&format!("{name}-repo"));
    let license = git(&repo, &["rev-parse", "HEAD:LICENSE"]);
    let notice = git(&repo, &["rev-parse", "HEAD:NOTICE.txt"]);
    let out = scratch(name).join("data");
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    assert_eq!(
        vendor::vendor_from(&repo.join(".git"), &commit, pins, &out).unwrap(),
        3
    );
    (out, tree, license, notice)
}

#[test]
fn vendoring_writes_the_pinned_files_and_notices() {
    let (out, tree, license, notice) = fixture_vendored("vend-ok");
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    let files = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT).unwrap();
    assert_eq!(files, jpdf_arlington::read_dir(&fixtures()).unwrap());
    assert_eq!(
        std::fs::read_to_string(out.join("LICENSE")).unwrap(),
        "fixture license\n"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("NOTICE.txt")).unwrap(),
        "fixture notice\n"
    );
    assert_eq!(
        std::fs::read_to_string(out.join(".gitattributes")).unwrap(),
        vendor::GITATTRIBUTES
    );
    // The committed copy carries the same attributes file.
    assert_eq!(
        std::fs::read_to_string(repo_root().join(vendor::DATA_DIR).join(".gitattributes")).unwrap(),
        vendor::GITATTRIBUTES
    );
    // An existing target is never overwritten, and wrong pins write nothing.
    let (repo, commit, _) = fixture_repo("vend-again");
    let e = vendor::vendor_from(&repo.join(".git"), &commit, pins, &out).unwrap_err();
    assert!(e.to_string().contains("exists; delete it first"), "{e}");
    let fresh = scratch("vend-wrong").join("data");
    let wrong = vendor::Pins {
        license: &notice,
        ..pins
    };
    let e = vendor::vendor_from(&repo.join(".git"), &commit, wrong, &fresh).unwrap_err();
    assert!(e.to_string().contains("LICENSE is"), "{e}");
    assert!(!fresh.exists());
    // No staging directory is left behind, after success or failure.
    for parent in [out.parent().unwrap(), fresh.parent().unwrap()] {
        let names: Vec<_> = std::fs::read_dir(parent)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(
            names
                .iter()
                .all(|n| !n.to_string_lossy().contains(".staging-")),
            "{names:?}"
        );
    }
}

/// Every way a vendored copy can drift from the pins is refused.
#[test]
fn vendored_copies_that_drift_are_refused() {
    type Edit = fn(&Path);
    let cases: [(&str, Edit); 12] = [
        ("tsv is tree", |d| {
            let f = d.join("tsv/Leaf.tsv");
            let mut b = std::fs::read(&f).unwrap();
            b.push(b'\n');
            std::fs::write(f, b).unwrap();
        }),
        ("tsv is tree", |d| {
            std::fs::remove_file(d.join("tsv/Leaf.tsv")).unwrap()
        }),
        ("tsv is tree", |d| {
            std::fs::copy(d.join("tsv/Leaf.tsv"), d.join("tsv/Extra.tsv")).unwrap();
        }),
        ("tsv/notes.md is not a regular .tsv file", |d| {
            std::fs::write(d.join("tsv/notes.md"), "x").unwrap();
        }),
        ("tsv/sub.tsv is not a regular .tsv file", |d| {
            std::fs::create_dir(d.join("tsv/sub.tsv")).unwrap();
        }),
        ("LICENSE is blob", |d| {
            std::fs::write(d.join("LICENSE"), "other\n").unwrap()
        }),
        ("NOTICE.txt: No such file", |d| {
            std::fs::remove_file(d.join("NOTICE.txt")).unwrap()
        }),
        ("NOTICE.txt is blob", |d| {
            std::fs::write(d.join("NOTICE.txt"), "fixture notice, edited\n").unwrap()
        }),
        (".gitattributes differs", |d| {
            std::fs::write(d.join(".gitattributes"), "* text=auto\n").unwrap()
        }),
        ("unexpected entry \"README.md\"", |d| {
            std::fs::write(d.join("README.md"), "x").unwrap()
        }),
        (
            "tsv/.DS_Store is not a regular .tsv file (re-vendor or remove stray files)",
            |d| std::fs::write(d.join("tsv/.DS_Store"), "x").unwrap(),
        ),
        ("limit exceeded: Big.tsv is over 1048576 bytes", |d| {
            let f = std::fs::File::create(d.join("tsv/Big.tsv")).unwrap();
            f.set_len(parse::MAX_FILE_BYTES as u64 + 1).unwrap();
        }),
    ];
    for (i, (want, edit)) in cases.into_iter().enumerate() {
        let (out, tree, license, notice) = fixture_vendored(&format!("vend-drift-{i}"));
        let pins = vendor::Pins {
            tree: &tree,
            license: &license,
            notice: &notice,
        };
        edit(&out);
        let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
            .unwrap_err()
            .to_string();
        assert!(e.contains(want), "case {i}: {e}");
    }
}

/// A TSV of exactly the size cap is read (and then fails only the tree check).
#[test]
fn vendored_file_size_cap_is_inclusive() {
    let (out, tree, license, notice) = fixture_vendored("vend-cap");
    let f = std::fs::File::create(out.join("tsv/Big.tsv")).unwrap();
    f.set_len(parse::MAX_FILE_BYTES as u64).unwrap();
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
        .unwrap_err()
        .to_string();
    assert!(e.starts_with("vendored: tsv is tree"), "{e}");
}

#[cfg(unix)]
#[test]
fn vendored_symlinks_are_refused() {
    let (out, tree, license, notice) = fixture_vendored("vend-link");
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    let leaf = out.join("tsv/Leaf.tsv");
    let moved = out.parent().unwrap().join("Leaf.real");
    std::fs::rename(&leaf, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &leaf).unwrap();
    let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
        .unwrap_err()
        .to_string();
    assert!(e.contains("tsv/Leaf.tsv is not a regular .tsv file"), "{e}");

    let (out, ..) = fixture_vendored("vend-link-notice");
    let lic = out.join("LICENSE");
    let moved = out.parent().unwrap().join("LICENSE.real");
    std::fs::rename(&lic, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &lic).unwrap();
    let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
        .unwrap_err()
        .to_string();
    assert!(e.contains("LICENSE is not a regular file"), "{e}");

    let (out, ..) = fixture_vendored("vend-link-notice2");
    let n = out.join("NOTICE.txt");
    let moved = out.parent().unwrap().join("NOTICE.real");
    std::fs::rename(&n, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &n).unwrap();
    let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
        .unwrap_err()
        .to_string();
    assert!(e.contains("NOTICE.txt is not a regular file"), "{e}");

    // A symlinked tsv/ directory is refused even though its files are fine.
    let (out, ..) = fixture_vendored("vend-link-dir");
    let real = out.parent().unwrap().join("tsv-real");
    std::fs::rename(out.join("tsv"), &real).unwrap();
    std::os::unix::fs::symlink(&real, out.join("tsv")).unwrap();
    let e = vendor::load_checked(&out, pins, vendor::Caps::DEFAULT)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("is not a directory (symlinks are refused)"),
        "{e}"
    );
}

#[test]
fn cli_checks_the_vendored_copy_by_default() {
    let exe = env!("CARGO_BIN_EXE_jpdf-arlington");
    // The default copy is found from any directory, not only the checkout root.
    let ok = Command::new(exe)
        .arg("check")
        .current_dir(scratch("cli-cwd"))
        .output()
        .unwrap();
    assert_eq!(ok.status.code(), Some(0), "{ok:?}");
    assert!(
        String::from_utf8_lossy(&ok.stderr)
            .contains("vendored): 613 objects, 3985 keys, checks pass")
    );
    let (out, ..) = fixture_vendored("cli-vend");
    let bad = Command::new(exe)
        .args(["check", "--vendored"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1), "{bad:?}");
    // The fixture copy fails the real pins on its first check, the LICENSE blob id.
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("vendored: LICENSE is blob"),
        "{bad:?}"
    );
    // `vendor` refuses an existing target before touching the cache.
    let cache = scratch("cli-vend-cache").join("cache");
    let again = Command::new(exe)
        .args(["vendor", "--out"])
        .arg(&out)
        .arg("--cache")
        .arg(&cache)
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(1), "{again:?}");
    assert!(!cache.exists());
}

/// Owner ruling on #60: the data is for tests and tools only.
/// 1. Through the resolved dependency graph (`cargo tree`, normal and build edges, so
///    transitive paths and any manifest syntax count): no package under `crates/` or
///    `bindings/` depends on `jpdf-arlington`. Dev-dependencies are allowed.
/// 2. No shipped file names Arlington or is a copy of a vendored TSV ([`shipped_hits`]).
#[test]
fn no_shipped_crate_uses_the_arlington_data() {
    let out = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--workspace",
            "--target",
            "all",
            "--all-features",
            "--locked",
            "--offline",
            "-e",
            "normal,build",
            "--prefix",
            "none",
            "-i",
            "jpdf-arlington",
        ])
        .current_dir(repo_root())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let tree = String::from_utf8(out.stdout).unwrap();
    assert!(tree.contains("/tools/arlington)"), "{tree}");
    let root = repo_root().canonicalize().unwrap();
    let shipped: Vec<String> = ["crates", "bindings"]
        .iter()
        .map(|d| format!("{}/", root.join(d).display()))
        .collect();
    // Positive control: a known dependent shows up with the same path prefix.
    assert!(
        tree.contains(&format!("({})", root.join("xtask").display())),
        "{tree}"
    );
    for line in tree.lines() {
        assert!(
            !shipped.iter().any(|d| line.contains(d.as_str())),
            "shipped package depends on jpdf-arlington: {line}"
        );
    }

    let ids = vendored_ids();
    // 613 files; some share their contents, so 610 distinct blob ids.
    assert_eq!(ids.len(), 610);
    let (scanned, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
    for want in [
        "crates/jpdf-engine/src/lib.rs",
        "crates/jpdf-engine/Cargo.toml",
        "apps/web/vite.config.ts",
        "apps/web/index.html",
        "apps/web/src/main.tsx",
        "apps/macos/JPDF/StartView.swift",
    ] {
        assert!(scanned.contains(&root.join(want)), "{want} not scanned");
    }
    assert!(hits.is_empty(), "{hits:#?}");
}

/// Git blob ids of the vendored TSVs.
#[cfg(test)]
fn vendored_ids() -> std::collections::HashSet<[u8; 20]> {
    let tsv = repo_root().join(vendor::DATA_DIR).join("tsv");
    jpdf_arlington::read_dir(&tsv)
        .unwrap()
        .iter()
        .map(|(_, bytes)| sha1::object_id("blob", bytes))
        .collect()
}

/// Item 2 of [`no_shipped_crate_uses_the_arlington_data`] under `root`: returns the files
/// scanned and the findings. Scanned: every file under each `crates/*` directory (outside
/// its `tests` and `target` directories, so `build.rs`, `benches/` and data directories
/// count), and under `bindings/`, `apps/web` and `apps/macos` (outside `tests`, `Tests`,
/// `JPDFUITests`, `e2e`, `scripts`, `node_modules`, `dist` and `target` directories).
/// A finding is a file that contains "arlington" in any case or whose git blob id is one of
/// `ids`, any symlink or other non-regular entry (a link could reach the data from inside a
/// scanned tree), and any directory or file that can't be read. Every root except
/// `bindings/` must exist.
///
/// Not seen: an edited or embedded copy of a TSV (only whole-file blob ids are compared),
/// and a path to the data built outside the word match (`include_str!(concat!(…))` naming
/// `tools/` from a shipped crate); the dependency graph check doesn't see those either.
fn shipped_hits(
    root: &Path,
    ids: &std::collections::HashSet<[u8; 20]>,
    max_file_bytes: u64,
) -> (Vec<PathBuf>, Vec<String>) {
    /// Visits `p` without following symlinks; directories for which `skip` holds aren't
    /// entered.
    fn visit(
        p: &Path,
        skip: &dyn Fn(&Path) -> bool,
        files: &mut Vec<PathBuf>,
        hits: &mut Vec<String>,
    ) {
        let ft = match std::fs::symlink_metadata(p) {
            Ok(m) => m.file_type(),
            Err(e) => return hits.push(format!("{}: {e}", p.display())),
        };
        if ft.is_file() {
            return files.push(p.to_path_buf());
        }
        if !ft.is_dir() {
            return hits.push(format!("{}: not a regular file or directory", p.display()));
        }
        if skip(p) {
            return;
        }
        let entries = match std::fs::read_dir(p) {
            Ok(e) => e,
            Err(e) => return hits.push(format!("{}: {e}", p.display())),
        };
        for e in entries {
            match e {
                Ok(e) => visit(&e.path(), skip, files, hits),
                Err(e) => hits.push(format!("{}: {e}", p.display())),
            }
        }
    }
    let crates = root.join("crates");
    // Only a crate's own top-level `tests` and `target` (a `src/tests/` module ships).
    let crate_skip = |d: &Path| {
        d.parent().and_then(Path::parent) == Some(crates.as_path())
            && d.file_name().is_some_and(|n| n == "tests" || n == "target")
    };
    // Playwright's output directories (apps/web/.gitignore), at exactly these paths: a
    // `src/test-results/` module ships.
    let web_output = [
        root.join("apps/web/test-results"),
        root.join("apps/web/playwright-report"),
    ];
    let other_skip = |d: &Path| {
        d.file_name().is_some_and(|n| {
            [
                "tests",
                "Tests",
                "JPDFUITests",
                "e2e",
                "scripts",
                "node_modules",
                "dist",
                "target",
            ]
            .iter()
            .any(|s| n == *s)
        }) || d == web_output[0]
            || d == web_output[1]
    };
    let (mut files, mut hits) = (Vec::new(), Vec::new());
    // Every root, and `apps` above two of them, must be a real directory (not a file, not a
    // symlink), so nothing on the way to a root is followed.
    let real_dir = |p: &Path, hits: &mut Vec<String>| {
        let ok = std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
        if !ok {
            hits.push(format!(
                "{}: not a directory (symlinks are refused)",
                p.display()
            ));
        }
        ok
    };
    if real_dir(&crates, &mut hits) {
        visit(&crates, &crate_skip, &mut files, &mut hits);
    }
    let bindings = root.join("bindings");
    // Absent is fine; any other metadata error is a finding (through `real_dir`).
    let absent = std::fs::symlink_metadata(&bindings)
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
    if !absent && real_dir(&bindings, &mut hits) {
        visit(&bindings, &other_skip, &mut files, &mut hits);
    }
    if real_dir(&root.join("apps"), &mut hits) {
        for d in ["apps/web", "apps/macos"] {
            let dir = root.join(d);
            if real_dir(&dir, &mut hits) {
                visit(&dir, &other_skip, &mut files, &mut hits);
            }
        }
    }
    for f in &files {
        match read_scanned(f, max_file_bytes) {
            Ok(b) => {
                if String::from_utf8_lossy(&b)
                    .to_lowercase()
                    .contains("arlington")
                {
                    hits.push(format!("{}: names Arlington", f.display()));
                }
                if ids.contains(&sha1::object_id("blob", &b)) {
                    hits.push(format!("{}: a copy of a vendored TSV", f.display()));
                }
            }
            Err(e) => hits.push(format!("{}: {e}", f.display())),
        }
    }
    (files, hits)
}

/// Reads a scanned file: a non-blocking open where `jpdf_fsio::OPEN_FLAGS` is set (there a
/// FIFO swapped in after the walk can't block it), the opened handle must be a regular file, and a file over `max` bytes is an error, read
/// no further than `max + 1` bytes.
fn read_scanned(f: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    let file = jpdf_fsio::open_nonblocking(f)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    read_capped(file, max)
}

/// Reads `r` to the end, but never more than `max + 1` bytes; more than `max` is an error.
fn read_capped(r: impl std::io::Read, max: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut b = Vec::new();
    r.take(max.saturating_add(1)).read_to_end(&mut b)?;
    if u64::try_from(b.len()).unwrap_or(u64::MAX) > max {
        return Err(std::io::Error::other(format!("over {max} bytes")));
    }
    Ok(b)
}

/// The cap stops the read itself: an endless source fails with the cap error after
/// `max + 1` bytes (without the `take` this test never ends), and exactly `max` bytes pass.
#[test]
fn scan_reads_stop_one_byte_past_the_cap() {
    /// An endless source that counts the bytes it hands out.
    struct Counting(u64);
    impl std::io::Read for Counting {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            buf.fill(b'a');
            self.0 += u64::try_from(buf.len()).unwrap();
            Ok(buf.len())
        }
    }
    let mut src = Counting(0);
    let e = read_capped(&mut src, 1000).unwrap_err();
    assert_eq!(e.to_string(), "over 1000 bytes");
    assert_eq!(src.0, 1001, "read past max + 1");
    assert_eq!(read_capped(&[7u8; 1000][..], 1000).unwrap().len(), 1000);
    // The real cap: 64 MiB.
    assert_eq!(SCAN_MAX_FILE_BYTES, 64 * 1024 * 1024);
}

/// The per-file read cap of the real scan (no shipped file comes near it).
const SCAN_MAX_FILE_BYTES: u64 = 64 << 20;

/// [`shipped_hits`] on a fixture tree: a clean tree has no findings, and each way around
/// the audit 0002 version of the scan (#115 item 1) is found: a symlink in a crate's `src/`,
/// a TSV copied outside `src/` without the word, and the word in a crate's non-`src` files
/// or a `src/tests/` module. Skipped directories hold findings that must stay unseen.
#[test]
fn shipped_scan_finds_links_copies_and_names() {
    let tsv = repo_root()
        .join(vendor::DATA_DIR)
        .join("tsv/ArrayOfFunctions.tsv");
    let copy = std::fs::read(&tsv).unwrap();
    let ids = vendored_ids();
    // The copy must not contain the word, or the word match would find it instead.
    assert!(
        !String::from_utf8_lossy(&copy)
            .to_lowercase()
            .contains("arlington")
    );
    let tree = |name: &str| {
        let root = scratch(name);
        for (f, body) in [
            ("crates/a/Cargo.toml", "[package]\n"),
            ("crates/a/src/lib.rs", "//! a\n"),
            ("crates/a/tests/t.rs", "// arlington is fine in tests\n"),
            ("crates/a/target/x.rs", "// arlington in target\n"),
            ("apps/web/index.html", "<html></html>\n"),
            ("apps/web/node_modules/m/Arlington.txt", "arlington\n"),
            ("apps/web/e2e/x.ts", "// Arlington\n"),
            ("apps/web/src/main.tsx", "// main\n"),
            ("apps/web/test-results/r.txt", "arlington\n"),
            ("apps/web/playwright-report/r.html", "arlington\n"),
            ("apps/macos/App.swift", "// app\n"),
            ("apps/macos/JPDF/View.swift", "// view\n"),
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        std::fs::write(root.join("apps/web/node_modules/m/Catalog.tsv"), &copy).unwrap();
        std::os::unix::fs::symlink(&tsv, root.join("apps/web/node_modules/link")).unwrap();
        root
    };
    let clean = tree("scan-clean");
    let (files, hits) = shipped_hits(&clean, &ids, SCAN_MAX_FILE_BYTES);
    assert!(hits.is_empty(), "{hits:#?}");
    assert_eq!(files.len(), 6, "{files:#?}");

    type Plant = fn(&Path, &Path, &[u8]);
    let cases: [(&str, Plant, &str); 7] = [
        (
            "link",
            |r, tsv, _| std::os::unix::fs::symlink(tsv, r.join("crates/a/src/data.tsv")).unwrap(),
            "crates/a/src/data.tsv: not a regular file or directory",
        ),
        (
            "link-dir",
            |r, tsv, _| {
                std::os::unix::fs::symlink(tsv.parent().unwrap(), r.join("crates/b")).unwrap()
            },
            "crates/b: not a regular file or directory",
        ),
        (
            "copy",
            |r, _, b| {
                std::fs::create_dir_all(r.join("crates/a/data")).unwrap();
                std::fs::write(r.join("crates/a/data/x.tsv"), b).unwrap();
            },
            "crates/a/data/x.tsv: a copy of a vendored TSV",
        ),
        (
            "copy-web",
            |r, _, b| std::fs::write(r.join("apps/web/x.tsv"), b).unwrap(),
            "apps/web/x.tsv: a copy of a vendored TSV",
        ),
        (
            "word",
            |r, _, _| {
                std::fs::create_dir_all(r.join("crates/a/benches")).unwrap();
                std::fs::write(r.join("crates/a/benches/b.rs"), "// ARLINGTON\n").unwrap();
            },
            "crates/a/benches/b.rs: names Arlington",
        ),
        (
            "word-src-tests",
            |r, _, _| {
                std::fs::create_dir_all(r.join("crates/a/src/tests")).unwrap();
                std::fs::write(r.join("crates/a/src/tests/mod.rs"), "// Arlington\n").unwrap();
            },
            "crates/a/src/tests/mod.rs: names Arlington",
        ),
        (
            "word-src-test-results",
            |r, _, _| {
                std::fs::create_dir_all(r.join("apps/web/src/test-results")).unwrap();
                std::fs::write(r.join("apps/web/src/test-results/x.ts"), "// Arlington\n").unwrap();
            },
            "apps/web/src/test-results/x.ts: names Arlington",
        ),
    ];
    for (name, plant, want) in cases {
        let root = tree(&format!("scan-{name}"));
        plant(&root, &tsv, &copy);
        let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
        let want = format!(
            "{}: ",
            root.join(want.split(": ").next().unwrap()).display()
        ) + want.split_once(": ").unwrap().1;
        assert_eq!(hits, [want], "{name}");
        std::fs::remove_dir_all(root).unwrap();
    }
    // An unreadable directory or file is a finding, not skipped (as root, modes don't stop
    // reads, so that half is skipped).
    let root_user = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| o.stdout.starts_with(b"0\n"));
    if !root_user {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path, m: u32| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap()
        };
        for target in ["crates/a/src", "crates/a/src/lib.rs"] {
            let root = tree(&format!("scan-locked-{}", target.len()));
            let p = root.join(target);
            mode(&p, 0o000);
            let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
            mode(&p, 0o755);
            assert_eq!(hits.len(), 1, "{target}: {hits:#?}");
            assert!(
                hits[0].starts_with(&format!("{}: ", p.display())),
                "{target}: {hits:#?}"
            );
            std::fs::remove_dir_all(root).unwrap();
        }
        // A metadata error on `bindings` other than NotFound is a finding, not "absent": with
        // the fixture root unsearchable, every root (bindings included) is a finding.
        let root = tree("scan-root-locked");
        mode(&root, 0o600);
        let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
        mode(&root, 0o755);
        let refused = |r: &str| {
            format!(
                "{}: not a directory (symlinks are refused)",
                root.join(r).display()
            )
        };
        assert_eq!(
            hits,
            [refused("crates"), refused("bindings"), refused("apps")]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    // A regular file in place of each root, and a symlinked `apps`, are findings.
    for (i, r) in ["crates", "apps", "apps/web", "apps/macos"]
        .iter()
        .enumerate()
    {
        let root = tree(&format!("scan-root-file-{i}"));
        std::fs::remove_dir_all(root.join(r)).unwrap();
        std::fs::write(root.join(r), "placeholder\n").unwrap();
        let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
        assert_eq!(
            hits,
            [format!(
                "{}: not a directory (symlinks are refused)",
                root.join(r).display()
            )],
            "{r}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    let root = tree("scan-bindings-file");
    std::fs::write(root.join("bindings"), "placeholder\n").unwrap();
    let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
    assert_eq!(
        hits,
        [format!(
            "{}: not a directory (symlinks are refused)",
            root.join("bindings").display()
        )]
    );
    std::fs::remove_dir_all(root).unwrap();
    let root = tree("scan-apps-link");
    std::fs::rename(root.join("apps"), root.join("ui")).unwrap();
    std::os::unix::fs::symlink(root.join("ui"), root.join("apps")).unwrap();
    let (_, hits) = shipped_hits(&root, &ids, SCAN_MAX_FILE_BYTES);
    assert_eq!(
        hits,
        [format!(
            "{}: not a directory (symlinks are refused)",
            root.join("apps").display()
        )]
    );
    std::fs::remove_dir_all(root).unwrap();
    // The per-file cap at its exact boundary: the largest scanned fixture file
    // (`apps/web/index.html`, 14 bytes) passes at a cap of 14 and is a finding at 13.
    let (_, hits) = shipped_hits(&clean, &ids, 14);
    assert!(hits.is_empty(), "{hits:#?}");
    let (_, hits) = shipped_hits(&clean, &ids, 13);
    assert_eq!(
        hits,
        [format!(
            "{}: over 13 bytes",
            clean.join("apps/web/index.html").display()
        )]
    );
    // A missing app root is a finding; a missing bindings/ isn't.
    std::fs::remove_dir_all(clean.join("apps/macos")).unwrap();
    let (_, hits) = shipped_hits(&clean, &ids, SCAN_MAX_FILE_BYTES);
    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert!(hits[0].starts_with(&format!("{}: ", clean.join("apps/macos").display())));
    std::fs::remove_dir_all(clean).unwrap();
}

/// Every cap of the vendored loader at its exact boundary: at the cap the files load, one
/// below fails with the limit (small caps over the 3-file fixtures).
#[test]
fn vendored_caps_have_exact_boundaries() {
    let (out, tree, license, notice) = fixture_vendored("vend-caps");
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    let files = jpdf_arlington::read_dir(&fixtures()).unwrap();
    let total: usize = files.iter().map(|(_, b)| b.len()).sum();
    let biggest = files.iter().map(|(_, b)| b.len()).max().unwrap();
    // The notice files and .gitattributes share the per-file cap.
    let biggest = biggest
        .max("fixture license\n".len())
        .max(vendor::GITATTRIBUTES.len());
    let at = vendor::Caps {
        files: 3,
        file_bytes: biggest,
        total_bytes: total,
    };
    assert_eq!(vendor::load_checked(&out, pins, at).unwrap(), files);
    let over = |caps: vendor::Caps| match vendor::load_checked(&out, pins, caps) {
        Err(Error::Limit(m)) => m,
        other => panic!("{caps:?}: {other:?}"),
    };
    assert_eq!(over(vendor::Caps { files: 2, ..at }), "more than 2 files");
    assert_eq!(
        over(vendor::Caps {
            total_bytes: total - 1,
            ..at
        }),
        format!("more than {} bytes in all files", total - 1)
    );
    assert!(
        over(vendor::Caps {
            file_bytes: biggest - 1,
            ..at
        })
        .ends_with(&format!(" is over {} bytes", biggest - 1))
    );
    // A notice file over the cap: LICENSE is read first.
    assert_eq!(
        over(vendor::Caps {
            file_bytes: "fixture license\n".len() - 1,
            ..at
        }),
        "LICENSE is over 15 bytes"
    );
}

/// Upstream entries must be plain files (mode 100644): the vendored tree id assumes it.
#[cfg(unix)]
#[test]
fn pinned_loading_rejects_other_modes() {
    let (dir, _, _) = fixture_repo("pin-mode");
    use std::os::unix::fs::PermissionsExt as _;
    let leaf = dir.join(fetch::TSV_DIR).join("Leaf.tsv");
    std::fs::set_permissions(&leaf, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "--no-gpg-sign", "-m", "exec"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]);
    let tree = git(&dir, &["rev-parse", "HEAD:tsv/latest"]);
    let e = fetch::load_verified(&dir.join(".git"), &commit, &tree).unwrap_err();
    assert!(
        e.to_string()
            .contains("Leaf.tsv has mode 100755, expected 100644"),
        "{e}"
    );
}

/// Bad names are refused before anything is created.
#[test]
fn write_dir_checks_names_first() {
    let base = scratch("write-names");
    for bad in [
        "",
        "../x.tsv",
        "a/b.tsv",
        "a\\b.tsv",
        ".hidden.tsv",
        "notes.md",
    ] {
        let out = base.join("data");
        let files = vec![(bad.to_string(), b"x".to_vec())];
        let e = vendor::write_dir(&out, &files, &[])
            .unwrap_err()
            .to_string();
        assert!(e.contains("bad file name"), "{bad:?}: {e}");
        assert!(!out.exists(), "{bad:?}");
    }
    let out = base.join("data");
    let e = vendor::write_dir(&out, &[], &[("README", b"x".to_vec())])
        .unwrap_err()
        .to_string();
    assert!(e.contains("bad notice name"), "{e}");
    assert!(!out.exists());
}

/// The staging directory: removed after a failed load-back, and an existing one (another
/// run's) is refused and left alone.
#[test]
fn vendoring_cleans_up_only_its_own_staging() {
    let files = jpdf_arlington::read_dir(&fixtures()).unwrap();
    let (_, tree, license, notice) = fixture_vendored("stage-pins");
    let notices = vec![
        ("LICENSE", b"fixture license\n".to_vec()),
        ("NOTICE.txt", b"fixture notice\n".to_vec()),
    ];
    let pins = vendor::Pins {
        tree: &tree,
        license: &license,
        notice: &notice,
    };
    let base = scratch("stage");
    let out = base.join("data");
    let staged = base.join(format!("data.staging-{}", std::process::id()));
    // Load-back fails (wrong tree pin) after the staging directory was written.
    let wrong = vendor::Pins {
        tree: &license,
        ..pins
    };
    let e = vendor::vendor_files(&files, &notices, wrong, &out).unwrap_err();
    assert!(e.to_string().contains("tsv is tree"), "{e}");
    assert!(!out.exists() && !staged.exists());
    // Someone else's staging directory is refused and kept.
    std::fs::create_dir(&staged).unwrap();
    std::fs::write(staged.join("marker"), "keep").unwrap();
    let e = vendor::vendor_files(&files, &notices, pins, &out).unwrap_err();
    assert!(e.to_string().contains("exists; delete it first"), "{e}");
    assert!(staged.join("marker").is_file() && !out.exists());
    // Control: with the path free, the same call succeeds and leaves no staging behind.
    std::fs::remove_dir_all(&staged).unwrap();
    assert_eq!(
        vendor::vendor_files(&files, &notices, pins, &out).unwrap(),
        3
    );
    assert!(out.join("tsv/Leaf.tsv").is_file() && !staged.exists());
}
