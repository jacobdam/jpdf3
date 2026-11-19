#!/usr/bin/env python3
"""Judge self-test (AUTOMATION.md §6.2). PROTECTED. A failing self-test fails the verdict.

usage: selftest.py [--e2e --corpus DIR --manifest M --work DIR]

Unit checks always run. --e2e runs the real pipeline on a corpus sample with fake CLIs:
good (must pass what it implements), crash (must fail), leaky (cheating probes must fail).
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "lib"))
sys.path.insert(0, str(HERE))
import common as C  # noqa: E402
import verdict as V  # noqa: E402

FAILS: list[str] = []


def expect(cond: bool, what: str) -> None:
    print(("  ok   " if cond else "  FAIL ") + what)
    if not cond:
        FAILS.append(what)


def unit(tmp: Path) -> None:
    print("unit:")
    expect(C.classify(0, False) == "ok" and C.classify(2, False) == "error" and C.classify(3, False) == "unsupported", "exit code classes")
    expect(C.classify(101, False) == "crash" and C.classify(-11, False) == "crash" and C.classify(None, True) == "timeout", "panic/signal/timeout are crashes")
    sl = tmp / "sleep.sh"
    sl.write_text("#!/bin/sh\nsleep 5\n")
    sl.chmod(0o755)
    expect(C.run_limited([str(sl)], 0.5).status == "timeout", "run_limited enforces timeout")
    sg = tmp / "segv.sh"
    sg.write_text("#!/bin/sh\nkill -SEGV $$\n")
    sg.chmod(0o755)
    expect(C.run_limited([str(sg)], 5).status == "crash", "run_limited reports signals as crash")
    expect(C.run_limited([str(tmp / "nope")], 1).status == "missing", "missing binary is 'missing'")
    expect(C.text_f1("Hello world", "hello  WORLD") == 1.0 and C.text_f1("", "") == 1.0 and C.text_f1("a", "b") == 0.0, "text_f1")
    from PIL import Image
    w = tmp / "w.png"
    Image.new("RGB", (100, 100), "white").save(w)
    b = tmp / "b.png"
    im = Image.new("RGB", (100, 100), "white")
    for x in range(40):
        for y in range(100):
            im.putpixel((x, y), (0, 0, 0))
    im.save(b)
    big = tmp / "big.png"
    Image.new("RGB", (120, 100), "white").save(big)
    expect(C.diff_fraction(w, w) == 0.0, "identical images match")
    expect(C.diff_fraction(w, b) > 0.3, "different images differ")
    expect(C.diff_fraction(w, big) == 1.0, "size mismatch fails")
    expect(C.diff_fraction(w, tmp / "missing.png") == 1.0, "unreadable image fails")
    secret = b"TOPSECRETWORD"
    pdfish = b"%PDF-1.7\n1 0 obj<<>>stream\n" + zlib.compress(b"BT (" + secret + b") Tj ET") + b"\nendstream\n"
    expect(secret in C.raw_and_inflated(pdfish) and secret not in pdfish, "leak scan inflates streams")
    cur = {"files": {"a": True, "b": False}, "pages": {"a#1": {"match": True, "text_f1": 0.9}},
           "probes": {"p": True}, "journeys": {"J-X-01": True}, "tests_passed": 10}
    base = {"files": {"a": True, "b": True}, "pages": {"a#1": {"match": True, "text_f1": 0.95}},
            "probes": {"p": True}, "journeys": {"J-X-01": True}, "tests_passed": 11}
    reg = V.ratchet(cur, base)
    expect(any("b" in r and "open" in r for r in reg), "ratchet: file regression detected")
    expect(any("text score" in r for r in reg), "ratchet: text drop detected")
    expect(any("test count" in r for r in reg), "ratchet: test count drop detected")
    expect(V.ratchet(base, base) == [], "ratchet: identical is clean")
    merged = V.merge_best(base, cur)
    expect(merged["files"]["b"] and merged["pages"]["a#1"]["text_f1"] == 0.95 and merged["tests_passed"] == 11, "baseline keeps best-ever")
    jd, bd = tmp / "j", tmp / "jb"
    jd.mkdir()
    bd.mkdir()
    (bd / "a.md").write_text("## J-ANN-01: x\n- milestone: M5\n- test: t1\n\n## J-ANN-02: y\n- milestone: M5\n- test: t2\n")
    (jd / "a.md").write_text("## J-ANN-01: x\n- milestone: M5\n- test: t1\n")
    _, probs = V.grade_journeys(V.parse_journeys(jd), V.parse_journeys(bd), {"t1": "pass"})
    expect(any("J-ANN-02" in p and "deleted" in p for p in probs), "journeys are append-only")
    st, probs = V.grade_journeys(V.parse_journeys(bd), V.parse_journeys(bd), {"t1": "pass", "t2": "fail"})
    expect(st["J-ANN-01"]["pass"] and not st["J-ANN-02"]["pass"] and not probs, "journey pass requires all tests")
    ju = tmp / "junit.xml"
    ju.write_text('<testsuites><testsuite><testcase name="a"/><testcase name="b"><failure/></testcase>'
                  '<testcase name="c"><flakyFailure/></testcase><testcase name="d"><skipped/></testcase></testsuite></testsuites>')
    expect(V.parse_junit(ju) == {"passed": 2, "failed": 1, "flaky": 1, "present": True}, "junit parsing")
    policy_unit(tmp)


def policy_unit(tmp: Path) -> None:
    root = tmp / "tree"
    (root / "crates/x/src").mkdir(parents=True)
    (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "nightly"\n')
    (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/x"]\n[workspace.lints.rust]\nunsafe_code="warn"\n')
    (root / "crates/x/Cargo.toml").write_text('[package]\nname="x"\nversion="0.0.0"\n[lints.clippy]\nunwrap_used="allow"\n')
    (root / "crates/x/src/lib.rs").write_text('// no forbid here\npub fn f(){ unsafe { } }\n#[allow(clippy::unwrap_used)]\nfn g(){}\n#[ignore]\nfn t(){}\n'
                                              'fn ok(){ let s = "unsafe"; } // unsafe in comment\n')
    p = subprocess.run([sys.executable, str(HERE / "policy.py"), "--root", str(root)], capture_output=True, text=True)
    out = p.stdout
    expect(p.returncode == 1, "policy fails a bad tree")
    for needle in ("exact stable version", "unsafe_code must be", "exactly `[lints] workspace = true`",
                   "missing #![forbid(unsafe_code)]", "lib.rs:2: `unsafe`", "relaxing denied lint", "#[ignore] needs"):
        expect(needle in out, f"policy detects: {needle}")
    expect("lib.rs:7" not in out, "policy ignores `unsafe` in strings/comments")


def e2e(corpus: Path, manifest: Path, work: Path) -> None:
    print("e2e:")
    sample = work / "sample.tsv"
    lines = [l for l in manifest.read_text().splitlines() if l and not l.startswith("#")][:60]
    sample.write_text("\n".join(lines) + "\n")
    ref, probes = work / "ref", work / "probes"
    subprocess.run([sys.executable, str(HERE / "reference.py"), str(sample), str(corpus), str(ref), "--probes", str(probes)],
                   check=True, stderr=subprocess.DEVNULL)

    def run_mode(mode: str) -> dict:
        wrapper = work / f"fake-{mode}"
        wrapper.write_text(f"#!/bin/sh\nFAKE_MODE={mode} exec '{sys.executable}' '{HERE / 'selftest' / 'fake_cli.py'}' \"$@\"\n")
        wrapper.chmod(0o755)
        res, out = work / f"res-{mode}", work / f"out-{mode}"
        subprocess.run([sys.executable, str(HERE / "run_untrusted.py"), "--bin", str(wrapper), "--manifest", str(sample),
                        "--corpus", str(corpus), "--out", str(res), "--probes", str(probes)], check=True, stdout=subprocess.DEVNULL)
        subprocess.run([sys.executable, str(HERE / "verdict.py"), "--tier", "smoke", "--ref", str(ref), "--results", str(res),
                        "--corpus", str(corpus), "--probes", str(probes), "--baseline-status", "none-ever",
                        "--out", str(out)], stdout=subprocess.DEVNULL)
        return C.load(out / "report.json")

    good = run_mode("good")
    m = good["metrics"]
    expect(good["metrics"]["crashes"] == 0, "good fake: no crashes")
    expect(m["render_match"] >= 0.99, f"good fake: consensus pages match ({m['render_match']})")
    expect(m["open_rate"] >= 0.95, f"good fake: files open ({m['open_rate']})")
    expect(m["text_f1"] >= 0.99, f"good fake: text matches ({m['text_f1']})")
    for k in ("probe_rewrite", "probe_encrypt", "probe_decrypt", "probe_sign", "probe_verify"):
        expect(m.get(k, 0) >= 0.9, f"good fake: {k} ({m.get(k)})")
    expect(m.get("probe_annotate", 1) == 0 and m.get("probe_redact", 1) == 0, "unimplemented probes score 0")
    expect(good["pass"], "good fake: verdict passes")
    crash = run_mode("crash")
    expect(not crash["pass"] and crash["metrics"]["crashes"] > 0, "crash fake: verdict fails")
    leaky = run_mode("leaky")
    expect(leaky["metrics"].get("probe_rewrite", 1) == 0 and leaky["metrics"].get("probe_encrypt", 1) == 0, "leaky fake: cheating probes fail")
    # ratchet against the good run's baseline: the leaky run must regress
    out = work / "out-ratchet"
    subprocess.run([sys.executable, str(HERE / "verdict.py"), "--tier", "smoke", "--ref", str(ref), "--results", str(work / "res-leaky"),
                    "--corpus", str(corpus), "--probes", str(probes), "--baseline-status", "found",
                    "--baseline", str(work / "out-good" / "baseline.json"), "--out", str(out)], stdout=subprocess.DEVNULL)
    r = C.load(out / "report.json")
    expect(not r["pass"] and any("probe regressed" in x for x in r["regressions"]), "ratchet catches regression vs baseline")
    out = work / "out-missing"
    subprocess.run([sys.executable, str(HERE / "verdict.py"), "--tier", "smoke", "--ref", str(ref), "--results", str(work / "res-good"),
                    "--corpus", str(corpus), "--probes", str(probes), "--baseline-status", "missing", "--out", str(out)], stdout=subprocess.DEVNULL)
    expect(not C.load(out / "report.json")["pass"], "missing baseline fails (tamper guard)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--e2e", action="store_true")
    ap.add_argument("--corpus", type=Path)
    ap.add_argument("--manifest", type=Path, default=HERE / "corpus" / "smoke.tsv")
    ap.add_argument("--work", type=Path)
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as td:
        unit(Path(td))
        if a.e2e:
            work = a.work or Path(td) / "e2e"
            if work.exists():
                shutil.rmtree(work)
            work.mkdir(parents=True)
            e2e(a.corpus, a.manifest, work)
    print(f"judge selftest: {'PASS' if not FAILS else 'FAIL'} ({len(FAILS)} failures)")
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
