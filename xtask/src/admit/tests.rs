//! specs/tooling/admit.md "Tests". Tool output fixtures are shaped like the real tools' output
//! (cargo 1.100, cargo-deny 0.21.0, the judge's `check_deps`), trimmed to what matters.

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
            python
        }
    );
    let o = p("--python /opt/py zune-jpeg@0.5.0-rc.1+b.2 --work /w/x").unwrap();
    let got = (
        o.name.as_str(),
        o.version.as_str(),
        o.python.as_str(),
        o.work,
    );
    assert_eq!(
        got,
        (
            "zune-jpeg",
            "0.5.0-rc.1+b.2",
            "/opt/py",
            PathBuf::from("/w/x")
        )
    );
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
        "a@1.0.0 --work /w --features x",
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
                [dependencies]\nminiz_oxide = \"=0.8.9\"\n\n[workspace]\n";
    assert_eq!(manifest(&opts()), want);
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
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join(TOOLCHAIN), "[toolchain]\nchannel = \"1.2.3\"\n").unwrap();
    write_probe(&probe, &repo, &opts()).unwrap();
    assert_eq!(
        fs::read_to_string(probe.join("Cargo.toml")).unwrap(),
        manifest(&opts())
    );
    assert_eq!(fs::read(probe.join("src/lib.rs")).unwrap(), b"");
    assert_eq!(
        fs::read(probe.join(TOOLCHAIN)).unwrap(),
        fs::read(repo.join(TOOLCHAIN)).unwrap()
    );
    // A second run into the same directory fails rather than reusing it; so does a missing file.
    assert!(write_probe(&probe, &repo, &opts()).is_err());
    fs::remove_file(repo.join(TOOLCHAIN)).unwrap();
    assert!(write_probe(&base.join("probe2"), &repo, &opts()).is_err());
}

#[test]
fn commands_are_pinned() {
    let (probe, repo) = (Path::new("/w/p"), Path::new("/r"));
    let line = |c: Cmd| format!("{} {}", c.program, c.args.join(" "));
    assert_eq!(line(lock_cmd()), "cargo generate-lockfile --color never");
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
        deny_result(&cap(0, DENY_OK, "")),
        checked(Outcome::Ok, Vec::new())
    );
    let bad = deny_result(&cap(
        4,
        "advisories FAILED, bans ok, licenses FAILED, sources ok\n",
        DENY_FAIL,
    ));
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
        assert!(failed(&deny_result(&c)), "{c:?}");
    }
}

// ---- the judge's dependency scan ----

const LIBC: &str = "dependency libc 0.2.170 contains `unsafe` (47 files) and is not allowlisted";

#[test]
fn judge_scan_results() {
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
            ("deny", Ok(cap(0, DENY_OK, ""))),
            ("-I", Ok(cap(0, "[]\n", ""))),
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
        let mut wrote = 0;
        let mut write = || {
            wrote += 1;
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

const ALL: [&str; 3] = ["probe", "deny", "unsafe"];

fn steps_where(report: &Report, pick: fn(&Checked) -> bool) -> Vec<&'static str> {
    report
        .iter()
        .filter(|(_, c)| pick(c))
        .map(|(s, _)| *s)
        .collect()
}

#[test]
fn driver_runs_every_check_in_order() {
    let mut f = Fake::passing();
    let (log, report) = f.drive(true);
    assert_eq!(f.calls, ["generate-lockfile", "deny", "-I"]);
    assert_eq!((log.names(), steps::exit_code(&log)), (ALL.to_vec(), 0));
    assert_eq!(steps_where(&report, |c| c.outcome == Outcome::Ok), ALL);
    assert_eq!(
        render(&opts(), &report, 0),
        "admit: probe: ok\nadmit: deny: ok\nadmit: unsafe: ok\n\
         admit: miniz_oxide@0.8.9: admitted by the license and unsafe checks (cargo vet, build-time code and features: #413)\n"
    );
}

#[test]
fn driver_keeps_going_after_a_rejection_or_failure() {
    // Each check rejects on its own, or both; the other still runs.
    let deny = Ok(cap(
        4,
        "advisories ok, bans ok, licenses FAILED, sources ok\n",
        DENY_FAIL,
    ));
    let scan = Ok(cap(0, &format!("[{LIBC:?}]\n"), ""));
    let both = Fake::passing()
        .set("deny", deny.clone())
        .set("-I", scan.clone());
    for (fake, rejected) in [
        (Fake::passing().set("deny", deny), "deny"),
        (Fake::passing().set("-I", scan), "unsafe"),
        (both, "deny, unsafe"),
    ] {
        let mut f = fake;
        let (log, report) = f.drive(true);
        assert_eq!(
            (log.names(), f.calls.len(), steps::exit_code(&log)),
            (ALL.to_vec(), 3, 4),
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
    // A check that can't start fails its step only, and outranks the other's rejection.
    let scan_hit = Ok(cap(0, &format!("[{LIBC:?}]\n"), ""));
    for (fake, broken, rejected) in [
        (
            Fake::passing().set("deny", Err("cannot run".to_owned())),
            "deny",
            "",
        ),
        (
            Fake::passing().set("-I", Err("cannot run".to_owned())),
            "unsafe",
            "",
        ),
        (
            Fake::passing()
                .set("deny", Err("cannot run".to_owned()))
                .set("-I", scan_hit),
            "deny",
            "unsafe",
        ),
    ] {
        let mut f = fake;
        let (log, report) = f.drive(true);
        assert_eq!(
            (log.names(), steps::exit_code(&log)),
            (ALL.to_vec(), 1),
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
